/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.db;

import jakarta.annotation.Nullable;
import java.util.List;
import marquez.api.models.SimpleDataset;
import marquez.api.models.SimpleJob;
import marquez.api.models.SimpleSearchSort;
import marquez.db.mappers.SimpleDatasetMapper;
import marquez.db.mappers.SimpleJobMapper;
import org.jdbi.v3.sqlobject.SqlObject;
import org.jdbi.v3.sqlobject.config.RegisterRowMapper;
import org.jdbi.v3.sqlobject.statement.SqlQuery;

/** The DAO for full search functionality returning simplified Dataset and Job objects. */
@RegisterRowMapper(SimpleDatasetMapper.class)
@RegisterRowMapper(SimpleJobMapper.class)
public interface FullSearchDao extends SqlObject {

  /**
   * Counts the total number of datasets matching the search criteria.
   *
   * @param query Query string to match against names (supports prefix/postfix matching).
   * @param namespace Optional namespace filter.
   * @return The total count of matching datasets.
   */
  @SqlQuery(
      """
              SELECT COUNT(*)
              FROM datasets_view d
              WHERE (d.namespace_name = :namespace OR :namespace IS NULL)
                AND (d.name ILIKE '%' || COALESCE(:query, '') || '%')
                AND d.is_deleted = false""")
  int countDatasets(String query, @Nullable String namespace);

  /**
   * Counts the total number of jobs matching the search criteria.
   *
   * @param query Query string to match against names (supports prefix/postfix matching).
   * @param namespace Optional namespace filter.
   * @return The total count of matching jobs.
   */
  @SqlQuery(
      """
              SELECT COUNT(*)
              FROM jobs_view j
              WHERE (j.namespace_name = :namespace OR :namespace IS NULL)
                AND (j.name ILIKE '%' || COALESCE(:query, '') || '%')
                AND j.symlink_target_uuid IS NULL""")
  int countJobs(String query, @Nullable String namespace);

  /**
   * Performs a full search for datasets by name, returning Dataset objects with configurable
   * facets.
   *
   * @param query Query string to match against names (supports prefix/postfix matching).
   * @param sort Sort order for results (by name or updated_at).
   * @param limit Maximum number of results to return.
   * @param offset Number of results to skip for pagination.
   * @param namespace Optional namespace filter.
   * @param includeFacets Whether to include facets in the response.
   * @param facetNames List of specific facet names to include (null for all).
   * @return A list of {@link SimpleDataset} objects.
   */
  @SqlQuery(
      """
              WITH facets_t AS (
                  SELECT df.dataset_version_uuid,
                         df.facet,
                         df."name",
                         df.created_at,
                         rank() OVER (PARTITION BY df.dataset_version_uuid, "name"
                                      ORDER BY created_at DESC) AS r
                  FROM dataset_facets AS df
                  WHERE (df.type ILIKE 'dataset' OR df.type ILIKE 'unknown' OR df.type ILIKE 'input')
                    AND (:includeFacets = true)
                    AND (CARDINALITY(COALESCE(:facetNames, ARRAY[]::text[])) = 0 OR df.name = ANY(COALESCE(:facetNames, ARRAY[]::text[])))
                    AND df.dataset_uuid IN (
                        SELECT uuid
                        FROM datasets_view
                        WHERE (namespace_name = :namespace OR :namespace IS NULL)
                          AND (name ILIKE '%' || COALESCE(:query, '') || '%')
                          AND is_deleted = false
                        ORDER BY
                          CASE WHEN :sort::text = 'UPDATED_AT' THEN updated_at END DESC,
                          CASE WHEN :sort::text = 'NAME' THEN name END
                        LIMIT :limit OFFSET :offset
                    )
              )
              SELECT d.uuid,
                     d.type,
                     d.created_at,
                     d.updated_at,
                     d.namespace_name,
                     d.name,
                     d.physical_name,
                     d.source_name,
                     d.description,
                     d.current_version_uuid,
                     d.last_modified_at,
                     d.is_deleted,
                     dv.lifecycle_state,
                     dv.fields,
                     COALESCE(t.tags, ARRAY[]::text[]) AS tags,
                     CASE
                       WHEN :includeFacets = false THEN '[]'::jsonb
                       ELSE COALESCE(f.facets, '[]'::jsonb)
                     END AS facets
              FROM datasets_view d
              LEFT JOIN dataset_versions dv ON d.current_version_uuid = dv.uuid
              LEFT JOIN (
                  SELECT ARRAY_AGG(t.name) AS tags, m.dataset_uuid
                  FROM tags AS t
                           INNER JOIN datasets_tag_mapping AS m ON m.tag_uuid = t.uuid
                  GROUP BY m.dataset_uuid
              ) t ON t.dataset_uuid = d.uuid
              LEFT JOIN (
                  SELECT df.dataset_version_uuid,
                         JSONB_AGG(df.facet ORDER BY df.created_at DESC) AS facets
                  FROM facets_t AS df
                  WHERE r = 1
                  GROUP BY df.dataset_version_uuid
              ) f ON f.dataset_version_uuid = d.current_version_uuid
              WHERE (d.namespace_name = :namespace OR :namespace IS NULL)
                AND (d.name ILIKE '%' || COALESCE(:query, '') || '%')
                AND d.is_deleted = false
              ORDER BY
                CASE WHEN :sort::text = 'UPDATED_AT' THEN d.updated_at END DESC,
                CASE WHEN :sort::text = 'NAME' THEN d.name END
              LIMIT :limit OFFSET :offset""")
  List<SimpleDataset> searchDatasets(
      String query,
      SimpleSearchSort sort,
      int limit,
      int offset,
      @Nullable String namespace,
      boolean includeFacets,
      @Nullable java.util.List<String> facetNames);

  /**
   * Performs a full search for jobs by name, returning Job objects with configurable facets.
   *
   * @param query Query string to match against names (supports prefix/postfix matching).
   * @param sort Sort order for results (by name or updated_at).
   * @param limit Maximum number of results to return.
   * @param offset Number of results to skip for pagination.
   * @param namespace Optional namespace filter.
   * @param includeFacets Whether to include facets in the response.
   * @param facetNames List of specific facet names to include (null for all).
   * @return A list of {@link SimpleJob} objects.
   */
  @SqlQuery(
      """
              WITH jobs_view_page AS (
                  SELECT *
                  FROM jobs_view AS j
                  WHERE (j.namespace_name = :namespace OR :namespace IS NULL)
                    AND (j.name ILIKE '%' || COALESCE(:query, '') || '%')
                    AND j.symlink_target_uuid IS NULL
              ),
              job_versions_temp AS (
                  SELECT *
                  FROM job_versions AS jv
                  WHERE (:namespace IS NULL OR jv.namespace_name = :namespace)
                    AND (:includeFacets = true)
              ),
              facets_temp AS (
                  SELECT
                      run_uuid,
                      JSON_AGG(e.facet ORDER BY e.lineage_event_time ASC) AS facets
                  FROM (
                      SELECT
                          jf.run_uuid,
                          jf.facet,
                          jf.lineage_event_time
                      FROM job_facets_view AS jf
                      INNER JOIN job_versions_temp jv2
                          ON jv2.latest_run_uuid = jf.run_uuid
                      INNER JOIN jobs_view_page j2
                          ON j2.current_version_uuid = jv2.uuid
                      WHERE (:includeFacets = true)
                        AND (CARDINALITY(COALESCE(:facetNames, ARRAY[]::text[])) = 0 OR jf.name = ANY(COALESCE(:facetNames, ARRAY[]::text[])))
                      ORDER BY jf.lineage_event_time ASC
                  ) e
                  GROUP BY e.run_uuid
              ),
              job_tags AS (
                  SELECT
                      j.uuid,
                      ARRAY_AGG(t.name) as tags
                  FROM jobs j
                  INNER JOIN jobs_tag_mapping jtm ON jtm.job_uuid = j.uuid
                  INNER JOIN tags t ON jtm.tag_uuid = t.uuid
                  WHERE (:namespace IS NULL OR j.namespace_name = :namespace)
                  GROUP BY j.uuid
              )
              SELECT
                  j.uuid,
                  j.type,
                  j.created_at,
                  j.updated_at,
                  j.namespace_name,
                  j.name,
                  j.simple_name,
                  j.parent_job_name,
                  j.parent_job_uuid,
                  j.current_location AS location,
                  j.description,
                  j.current_version_uuid,
                  COALESCE(jt.tags, ARRAY[]::text[]) AS tags,
                  ARRAY[]::text[] AS labels,
                  CASE
                    WHEN :includeFacets = false THEN '[]'::json
                    ELSE COALESCE(f.facets, '[]'::json)
                  END AS facets
              FROM jobs_view_page j
              LEFT OUTER JOIN job_versions_temp AS jv
                  ON jv.uuid = j.current_version_uuid
              LEFT OUTER JOIN facets_temp AS f
                  ON f.run_uuid = jv.latest_run_uuid
              LEFT OUTER JOIN job_tags jt
                  ON j.uuid = jt.uuid
              ORDER BY
                CASE WHEN :sort::text = 'UPDATED_AT' THEN j.updated_at END DESC,
                CASE WHEN :sort::text = 'NAME' THEN j.name END
              LIMIT :limit OFFSET :offset""")
  List<SimpleJob> searchJobs(
      String query,
      SimpleSearchSort sort,
      int limit,
      int offset,
      @Nullable String namespace,
      boolean includeFacets,
      @Nullable java.util.List<String> facetNames);
}
