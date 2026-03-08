use std::convert::Infallible;
use std::task::{Context, Poll};

use axum::http::{uri::PathAndQuery, Request, Uri};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
use tower::{Layer, Service};

/// Characters that must be percent-encoded inside a single URI path segment.
/// We start from NON_ALPHANUMERIC (encodes everything non-alphanumeric) and
/// then *remove* characters that are safe inside a path segment.
const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~')
    .remove(b'!')
    .remove(b'$')
    .remove(b'&')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')')
    .remove(b'*')
    .remove(b'+')
    .remove(b',')
    .remove(b';')
    .remove(b'=')
    .remove(b'@');

// Prefix constants removed — the middleware now detects decoded URIs by the
// characteristic `scheme:` + `//` pattern, regardless of path prefix.

/// Keywords that act as route segment boundaries (not parameter values).
const ROUTE_KEYWORDS: &[&str] = &[
    "namespaces",
    "datasets",
    "jobs",
    "sources",
    "tags",
    "versions",
    "runs",
    "fields",
    "facets",
    "start",
    "complete",
    "fail",
    "abort",
    "lineage",
    "column-lineage",
    "search",
    "stats",
    "events",
    "runlineage",
    "upstream",
    "healthcheck",
    "ping",
    "simple",
    "full",
    "v2beta",
    "lineage-events",
];

fn is_keyword(segment: &str) -> bool {
    ROUTE_KEYWORDS.contains(&segment) || segment == "api" || segment == "v1"
}

fn percent_encode_path_segment(value: &str) -> String {
    utf8_percent_encode(value, PATH_SEGMENT_ENCODE_SET).to_string()
}

/// Flush accumulated value parts into the result vector.
///
/// - Single-part values are pushed as-is (already correctly encoded or plain).
/// - Multi-part values are joined with `/` (reconstructing the decoded value)
///   and then percent-encoded to produce a single valid path segment.
fn flush_value_parts(value_parts: &mut Vec<&str>, result: &mut Vec<String>) {
    if value_parts.is_empty() {
        return;
    }
    if value_parts.len() == 1 {
        result.push(value_parts[0].to_string());
    } else {
        let joined = value_parts.join("/");
        result.push(percent_encode_path_segment(&joined));
    }
    value_parts.clear();
}

/// Re-encode a proxy-decoded URI path so Axum route matching works correctly.
///
/// When a reverse proxy decodes `%2F` → `/` and `%3A` → `:`, a single path
/// parameter like `hive%3A%2F%2Fhost%3A9083` becomes `hive://host:9083`, which
/// splits into multiple path segments and breaks routing.
///
/// This function detects decoded URI schemes by finding segments that end with
/// `:` (e.g. `hive:`, `s3:`, `s3a:`). It then accumulates subsequent segments
/// until it hits a known route keyword or end-of-path, joins them back into a
/// single value, and percent-encodes the result.
///
/// This approach works for **any** path prefix — it does not require `/api/v1/`
/// or `/api/v2beta/`. Already-encoded paths (`hive%3A%2F%2F`) have no segment
/// ending with `:`, so they pass through unchanged (idempotent).
///
/// Returns `None` if the path does not need modification.
fn normalize_path(path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('/').collect();
    let mut result: Vec<String> = Vec::new();
    let mut value_parts: Vec<&str> = Vec::new();
    let mut accumulating = false;

    for segment in &segments {
        if accumulating {
            if is_keyword(segment) {
                // Keyword boundary — flush accumulated decoded URI parts
                flush_value_parts(&mut value_parts, &mut result);
                accumulating = false;
                result.push(segment.to_string());
            } else {
                // Continue accumulating (empty from //, authority, path)
                value_parts.push(segment);
            }
        } else if segment.ends_with(':') && segment.len() > 1 {
            // Detected decoded URI scheme (e.g., "hive:", "s3:", "s3a:")
            accumulating = true;
            value_parts.push(segment);
        } else {
            result.push(segment.to_string());
        }
    }

    // Flush remaining accumulated parts at end of path
    flush_value_parts(&mut value_parts, &mut result);

    let new_path = result.join("/");
    if new_path != path {
        Some(new_path)
    } else {
        None
    }
}

/// Tower [`Layer`] that re-encodes proxy-decoded special characters in URI
/// paths before they reach the Axum router.
#[derive(Clone)]
pub struct NormalizeUriLayer;

impl<S> Layer<S> for NormalizeUriLayer {
    type Service = NormalizeUriService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        NormalizeUriService { inner }
    }
}

/// Tower [`Service`] created by [`NormalizeUriLayer`].
#[derive(Clone)]
pub struct NormalizeUriService<S> {
    inner: S,
}

impl<S, B> Service<Request<B>> for NormalizeUriService<S>
where
    S: Service<Request<B>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        let uri = req.uri();
        if let Some(new_path) = normalize_path(uri.path()) {
            tracing::info!(
                original_path = uri.path(),
                normalized_path = %new_path,
                "NormalizeUri: re-encoding decoded path"
            );

            // Rebuild the URI with the normalized path, preserving query string
            let new_pq = if let Some(q) = uri.query() {
                format!("{}?{}", new_path, q)
            } else {
                new_path
            };

            if let Ok(pq) = new_pq.parse::<PathAndQuery>() {
                let mut parts = uri.clone().into_parts();
                parts.path_and_query = Some(pq);
                if let Ok(new_uri) = Uri::from_parts(parts) {
                    *req.uri_mut() = new_uri;
                }
            }
        }

        self.inner.call(req)
    }
}

/// Allow `axum::serve(listener, NormalizeUriService<Router>)` by implementing
/// the make-service pattern: clone ourselves for each incoming connection.
impl<'a, S> Service<axum::serve::IncomingStream<'a, tokio::net::TcpListener>>
    for NormalizeUriService<S>
where
    S: Clone + Send + 'static,
{
    type Response = Self;
    type Error = Infallible;
    type Future = std::future::Ready<Result<Self, Infallible>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(
        &mut self,
        _: axum::serve::IncomingStream<'a, tokio::net::TcpListener>,
    ) -> Self::Future {
        std::future::ready(Ok(self.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoded_hive_namespace_is_re_encoded() {
        let path =
            "/api/v1/namespaces/hive://ilum-hive-metastore:9083/datasets/default.campaign_events";
        let result = normalize_path(path).expect("should normalize");
        assert_eq!(
            result,
            "/api/v1/namespaces/hive%3A%2F%2Filum-hive-metastore%3A9083/datasets/default.campaign_events"
        );
    }

    #[test]
    fn already_encoded_path_is_unchanged() {
        let path = "/api/v1/namespaces/hive%3A%2F%2Filum-hive-metastore%3A9083/datasets/default.campaign_events";
        assert!(normalize_path(path).is_none());
    }

    #[test]
    fn simple_path_is_unchanged() {
        let path = "/api/v1/namespaces/my-namespace/datasets/my-dataset";
        assert!(normalize_path(path).is_none());
    }

    #[test]
    fn non_api_path_is_unchanged() {
        let path = "/some/random/path";
        assert!(normalize_path(path).is_none());
    }

    #[test]
    fn healthcheck_is_unchanged() {
        assert!(normalize_path("/healthcheck").is_none());
    }

    #[test]
    fn decoded_namespace_with_jobs() {
        let path = "/api/v1/namespaces/hive://host:9083/jobs/my-job";
        let result = normalize_path(path).expect("should normalize");
        assert_eq!(
            result,
            "/api/v1/namespaces/hive%3A%2F%2Fhost%3A9083/jobs/my-job"
        );
    }

    #[test]
    fn multiple_decoded_params() {
        let path = "/api/v1/namespaces/s3://bucket/path/datasets/schema://db:1234/table";
        let result = normalize_path(path).expect("should normalize");
        assert_eq!(
            result,
            "/api/v1/namespaces/s3%3A%2F%2Fbucket%2Fpath/datasets/schema%3A%2F%2Fdb%3A1234%2Ftable"
        );
    }

    #[test]
    fn decoded_namespace_with_runs() {
        let path = "/api/v1/namespaces/hive://host:9083/jobs/my-job/runs";
        let result = normalize_path(path).expect("should normalize");
        assert_eq!(
            result,
            "/api/v1/namespaces/hive%3A%2F%2Fhost%3A9083/jobs/my-job/runs"
        );
    }

    #[test]
    fn decoded_namespace_with_dataset_versions() {
        let path = "/api/v1/namespaces/hive://host:9083/datasets/my-ds/versions";
        let result = normalize_path(path).expect("should normalize");
        assert_eq!(
            result,
            "/api/v1/namespaces/hive%3A%2F%2Fhost%3A9083/datasets/my-ds/versions"
        );
    }

    #[test]
    fn decoded_namespace_with_field_tags() {
        let path = "/api/v1/namespaces/hive://host:9083/datasets/my-ds/fields/col1/tags/pii";
        let result = normalize_path(path).expect("should normalize");
        assert_eq!(
            result,
            "/api/v1/namespaces/hive%3A%2F%2Fhost%3A9083/datasets/my-ds/fields/col1/tags/pii"
        );
    }

    #[test]
    fn s3_namespace_is_re_encoded() {
        let path = "/api/v1/namespaces/s3://my-bucket/datasets/my-dataset";
        let result = normalize_path(path).expect("should normalize");
        assert_eq!(
            result,
            "/api/v1/namespaces/s3%3A%2F%2Fmy-bucket/datasets/my-dataset"
        );
    }

    #[test]
    fn v2beta_path_is_unchanged() {
        let path = "/api/v2beta/search/jobs";
        assert!(normalize_path(path).is_none());
    }

    #[test]
    fn lineage_events_path_is_unchanged() {
        let path = "/api/v1/stats/lineage-events";
        assert!(normalize_path(path).is_none());
    }

    #[test]
    fn proxy_prefixed_decoded_path_is_re_encoded() {
        let path = "/api/dev/reactive/lineage/namespaces/hive://host:9083/datasets/ds/lineage";
        let result = normalize_path(path).expect("should normalize");
        assert_eq!(
            result,
            "/api/dev/reactive/lineage/namespaces/hive%3A%2F%2Fhost%3A9083/datasets/ds/lineage"
        );
    }

    #[test]
    fn already_encoded_proxy_path_is_unchanged() {
        let path =
            "/api/dev/reactive/lineage/namespaces/hive%3A%2F%2Fhost%3A9083/datasets/ds/lineage";
        assert!(normalize_path(path).is_none());
    }

    #[test]
    fn non_api_path_with_no_scheme_is_unchanged() {
        let path = "/other/path/to/resource";
        assert!(normalize_path(path).is_none());
    }

    #[test]
    fn proxy_prefixed_s3a_dataset_is_re_encoded() {
        let path = "/api/dev/reactive/lineage/namespaces/file/datasets/s3a://ilum-files/loki_cluster_seed.json/lineage";
        let result = normalize_path(path).expect("should normalize");
        assert_eq!(
            result,
            "/api/dev/reactive/lineage/namespaces/file/datasets/s3a%3A%2F%2Filum-files%2Floki_cluster_seed.json/lineage"
        );
    }

    #[tokio::test]
    async fn middleware_normalizes_decoded_path_for_router() {
        use axum::body::Body;
        use axum::routing::get;
        use axum::Router;
        use tower::{Layer, ServiceExt};

        // NormalizeUriLayer must wrap the Router from outside (not via Router::layer())
        // because Router::layer() applies middleware after route matching.
        let router = Router::new().route(
            "/api/v1/namespaces/{namespace}/datasets",
            get(|| async { "ok" }),
        );
        let app = NormalizeUriLayer.layer(router);

        let req = Request::builder()
            .uri("/api/v1/namespaces/hive://host:9083/datasets")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
    }

    #[tokio::test]
    async fn middleware_normalizes_proxy_prefixed_path_for_router() {
        use axum::body::Body;
        use axum::routing::get;
        use axum::Router;
        use tower::{Layer, ServiceExt};

        let router = Router::new().route(
            "/api/dev/reactive/lineage/namespaces/{namespace}/datasets/{dataset}/lineage",
            get(|| async { "ok" }),
        );
        let app = NormalizeUriLayer.layer(router);

        let req = Request::builder()
            .uri("/api/dev/reactive/lineage/namespaces/hive://host:9083/datasets/default.customers/lineage")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status().as_u16(), 200);
    }
}
