/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.db;

import jakarta.annotation.Nullable;
import java.util.List;
import marquez.api.models.SearchFilter;
import marquez.api.models.SimpleSearchResult;
import marquez.api.models.SimpleSearchSort;
import marquez.db.mappers.SimpleSearchResultMapper;
import org.jdbi.v3.sqlobject.SqlObject;
import org.jdbi.v3.sqlobject.config.RegisterRowMapper;
import org.jdbi.v3.sqlobject.statement.SqlQuery;

/** The DAO for simple search functionality. */
@RegisterRowMapper(SimpleSearchResultMapper.class)
public interface SimpleSearchDao extends SqlObject {

  default List<SimpleSearchResult> simpleSearch(String query, int limit) {
    return simpleSearch(query, null, SimpleSearchSort.UPDATED_AT, limit, null);
  }

  default List<SimpleSearchResult> simpleSearch(String query, SearchFilter filter, int limit) {
    return simpleSearch(query, filter, SimpleSearchSort.UPDATED_AT, limit, null);
  }

  default List<SimpleSearchResult> simpleSearch(
      String query, SearchFilter filter, SimpleSearchSort sort, int limit) {
    return simpleSearch(query, filter, sort, limit, null);
  }

  /**
   * Performs a simple search across datasets and jobs by name. Optimized for performance with
   * proper indexes and streaming results. Results are sorted by updated_at DESC (newest first) by
   * default.
   *
   * @param query Query string to match against names (supports prefix/postfix matching).
   * @param filter Optional filter to restrict results to datasets or jobs only.
   * @param sort Sort order for results (by name or updated_at).
   * @param limit Maximum number of results to return.
   * @param namespace Optional namespace filter.
   * @return A list of {@link SimpleSearchResult} objects.
   */
  @SqlQuery(
      """
          SELECT type, name, namespace_name, updated_at
          FROM (
            SELECT 'DATASET' AS type,
                   d.name,
                   d.namespace_name,
                   d.updated_at
            FROM datasets_view AS d
            WHERE (d.namespace_name = :namespace OR CAST(:namespace AS TEXT) IS NULL)
              AND (d.name ILIKE '%' || CAST(:query AS TEXT) || '%')
              AND d.is_deleted = false
            UNION ALL
            SELECT 'JOB' AS type,
                   j.name,
                   j.namespace_name,
                   j.updated_at
            FROM jobs_view AS j
            WHERE (j.namespace_name = :namespace OR CAST(:namespace AS TEXT) IS NULL)
              AND (j.name ILIKE '%' || CAST(:query AS TEXT) || '%')
              AND j.symlink_target_uuid IS NULL
          ) AS results
          WHERE (CAST(:filter AS TEXT) IS NULL OR UPPER(type) = UPPER(CAST(:filter AS TEXT)))
          ORDER BY
            CASE WHEN UPPER(CAST(:sort AS TEXT)) = 'UPDATED_AT' THEN updated_at END DESC,
            CASE WHEN UPPER(CAST(:sort AS TEXT)) = 'NAME' THEN name END ASC
          LIMIT :limit""")
  List<SimpleSearchResult> simpleSearch(
      String query,
      @Nullable SearchFilter filter,
      SimpleSearchSort sort,
      int limit,
      @Nullable String namespace);
}
