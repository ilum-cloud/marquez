use axum::extract::{FromRequestParts, Query};
use axum::http::request::Parts;
use axum::http::Request;
use serde::Deserialize;

/// A dedicated extractor for `FullSearchParams` that manually handles repeated
/// `facetNames` query params (e.g. `?facetNames=version&facetNames=storage`).
///
/// Axum's `Query<T>` uses `serde_urlencoded` which rejects duplicate keys at
/// the format level — before any field-level `deserialize_with` runs. This
/// extractor works around that by extracting `facetNames` from the raw query
/// string, then deserializing the remaining params normally.
pub struct JsonQueryFullSearch(pub FullSearchParams);

impl<S: Send + Sync> FromRequestParts<S> for JsonQueryFullSearch {
    type Rejection = crate::error::AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let raw = parts.uri.query().unwrap_or("");

        let facet_names: Vec<String> = form_urlencoded::parse(raw.as_bytes())
            .filter(|(k, _)| k == "facetNames")
            .map(|(_, v)| v.into_owned())
            .collect();

        let filtered: String = form_urlencoded::parse(raw.as_bytes())
            .filter(|(k, _)| k != "facetNames")
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let mut params: FullSearchParams = serde_urlencoded::from_str(&filtered)
            .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;

        params.facet_names = facet_names;
        Ok(JsonQueryFullSearch(params))
    }
}

/// A wrapper around Axum's `Query<T>` that converts query-string parse failures
/// into JSON `AppError::BadRequest` responses instead of Axum's default
/// `text/plain` rejections (which break the Java/Spring proxy).
pub struct JsonQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for JsonQuery<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = crate::error::AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match Query::<T>::from_request_parts(parts, state).await {
            Ok(Query(value)) => Ok(JsonQuery(value)),
            Err(rejection) => Err(crate::error::AppError::BadRequest(rejection.body_text())),
        }
    }
}

/// A wrapper around Axum's `Json<T>` that converts body parse failures
/// into `AppError::UnprocessableEntity` (422) instead of Axum's default 400.
/// Matches Dropwizard's behavior of returning 422 for `@Valid`/`@NotNull` violations.
pub struct ValidJson<T>(pub T);

impl<S, T> axum::extract::FromRequest<S> for ValidJson<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = crate::error::AppError;

    async fn from_request(
        req: Request<axum::body::Body>,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(v)) => Ok(ValidJson(v)),
            Err(e) => Err(crate::error::AppError::BadRequest(e.body_text())),
        }
    }
}

/// A wrapper around Axum's `Path<T>` that converts path parse failures
/// into JSON `AppError` responses instead of Axum's default plain-text.
/// Returns 404 for path param parse failures (matching Jersey behavior for
/// invalid UUIDs, etc.).
pub struct JsonPath<T>(pub T);

impl<S, T> FromRequestParts<S> for JsonPath<T>
where
    T: serde::de::DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = crate::error::AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Path(value)) => Ok(JsonPath(value)),
            Err(rejection) => Err(crate::error::AppError::NotFound(rejection.body_text())),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    #[serde(default, deserialize_with = "reject_negative_optional")]
    pub limit: Option<i32>,
    #[serde(default, deserialize_with = "reject_negative_optional")]
    pub offset: Option<i32>,
}

impl PaginationParams {
    pub fn limit(&self) -> i32 {
        self.limit.unwrap_or(100).clamp(0, 1000)
    }
    pub fn offset(&self) -> i32 {
        self.offset.unwrap_or(0).max(0)
    }
}

fn reject_negative_optional<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<i32> = Option::deserialize(deserializer)?;
    if let Some(v) = opt {
        if v < 0 {
            return Err(serde::de::Error::custom(format!(
                "value must be non-negative, got {v}"
            )));
        }
    }
    Ok(opt)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchParams {
    pub q: String,
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i32>,
    pub namespace: Option<String>,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsParams {
    pub period: String,
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsParams {
    pub before: Option<String>,
    pub after: Option<String>,
    pub sort_direction: Option<String>,
    #[serde(default, deserialize_with = "reject_negative_optional")]
    pub limit: Option<i32>,
    #[serde(default, deserialize_with = "reject_negative_optional")]
    pub offset: Option<i32>,
}

impl EventsParams {
    pub fn limit(&self) -> i32 {
        self.limit.unwrap_or(100).clamp(1, 1000)
    }
    pub fn offset(&self) -> i32 {
        self.offset.unwrap_or(0).max(0)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LineageParams {
    pub node_id: String,
    pub depth: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnLineageParams {
    pub node_id: String,
    pub depth: Option<i32>,
    pub with_downstream: Option<bool>,
    pub created_at_until: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpstreamRunParams {
    pub run_id: uuid::Uuid,
    pub depth: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct RunTransitionParams {
    pub at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullSearchParams {
    pub q: String,
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<i32>,
    pub namespace: Option<String>,
    pub offset: Option<i32>,
    pub facets: Option<bool>,
    #[serde(default)]
    pub facet_names: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobListParams {
    #[serde(default, deserialize_with = "reject_negative_optional")]
    pub limit: Option<i32>,
    #[serde(default, deserialize_with = "reject_negative_optional")]
    pub offset: Option<i32>,
    #[serde(default)]
    pub last_run_states: Vec<String>,
}

impl JobListParams {
    pub fn limit(&self) -> i32 {
        self.limit.unwrap_or(100).clamp(1, 1000)
    }
    pub fn offset(&self) -> i32 {
        self.offset.unwrap_or(0).max(0)
    }
}

/// A dedicated extractor for `JobListParams` that manually handles repeated
/// `lastRunStates` query params (e.g. `?lastRunStates=RUNNING&lastRunStates=COMPLETED`).
///
/// Same pattern as `JsonQueryFullSearch` — `serde_urlencoded` rejects duplicate keys.
pub struct JsonQueryJobList(pub JobListParams);

impl<S: Send + Sync> FromRequestParts<S> for JsonQueryJobList {
    type Rejection = crate::error::AppError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let raw = parts.uri.query().unwrap_or("");

        let last_run_states: Vec<String> = form_urlencoded::parse(raw.as_bytes())
            .filter(|(k, _)| k == "lastRunStates")
            .map(|(_, v)| v.into_owned())
            .collect();

        let filtered: String = form_urlencoded::parse(raw.as_bytes())
            .filter(|(k, _)| k != "lastRunStates")
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let mut params: JobListParams = serde_urlencoded::from_str(&filtered)
            .map_err(|e| crate::error::AppError::BadRequest(e.to_string()))?;

        params.last_run_states = last_run_states;
        Ok(JsonQueryJobList(params))
    }
}

#[derive(Debug, Deserialize)]
pub struct FacetParams {
    #[serde(rename = "type")]
    pub type_: String,
}
