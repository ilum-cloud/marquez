/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.db.mappers;

import static marquez.db.Columns.stringOrThrow;
import static marquez.db.Columns.timestampOrThrow;

import java.sql.ResultSet;
import java.sql.SQLException;
import lombok.NonNull;
import marquez.api.models.SimpleSearchResult;
import marquez.common.models.DatasetName;
import marquez.common.models.JobName;
import marquez.common.models.NamespaceName;
import marquez.db.Columns;
import org.jdbi.v3.core.mapper.RowMapper;
import org.jdbi.v3.core.statement.StatementContext;

/** Convert a search results to a {@link SimpleSearchResult}. */
public final class SimpleSearchResultMapper implements RowMapper<SimpleSearchResult> {
  @Override
  public SimpleSearchResult map(@NonNull ResultSet results, @NonNull StatementContext context)
      throws SQLException {
    final SimpleSearchResult.ResultType type =
        SimpleSearchResult.ResultType.valueOf(stringOrThrow(results, Columns.TYPE));
    switch (type) {
      case DATASET:
        return SimpleSearchResult.newDatasetResult(
            DatasetName.of(stringOrThrow(results, Columns.NAME)),
            NamespaceName.of(stringOrThrow(results, Columns.NAMESPACE_NAME)),
            timestampOrThrow(results, Columns.UPDATED_AT));
      case JOB:
        return SimpleSearchResult.newJobResult(
            JobName.of(stringOrThrow(results, Columns.NAME)),
            NamespaceName.of(stringOrThrow(results, Columns.NAMESPACE_NAME)),
            timestampOrThrow(results, Columns.UPDATED_AT));
      default:
        throw new IllegalArgumentException(String.format("search type '%s' not supported", type));
    }
  }
}
