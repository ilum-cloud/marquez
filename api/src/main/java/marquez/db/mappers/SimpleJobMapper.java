/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.db.mappers;

import static marquez.db.Columns.stringArrayOrThrow;
import static marquez.db.Columns.stringOrNull;
import static marquez.db.Columns.stringOrThrow;
import static marquez.db.Columns.timestampOrThrow;
import static marquez.db.Columns.urlOrNull;
import static marquez.db.Columns.uuidOrNull;
import static marquez.db.mappers.MapperUtils.toFacetsOrNull;

import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSet;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.util.List;
import java.util.stream.Collectors;
import lombok.NonNull;
import marquez.api.models.SimpleJob;
import marquez.common.models.JobId;
import marquez.common.models.JobName;
import marquez.common.models.JobType;
import marquez.common.models.NamespaceName;
import marquez.common.models.TagName;
import marquez.db.Columns;
import org.jdbi.v3.core.mapper.RowMapper;
import org.jdbi.v3.core.statement.StatementContext;

/** A {@link RowMapper} for {@link SimpleJob}. */
public final class SimpleJobMapper implements RowMapper<SimpleJob> {
  @Override
  public SimpleJob map(@NonNull ResultSet results, @NonNull StatementContext context)
      throws SQLException {
    return new SimpleJob(
        new JobId(
            NamespaceName.of(stringOrThrow(results, Columns.NAMESPACE_NAME)),
            JobName.of(stringOrThrow(results, Columns.NAME))),
        JobType.valueOf(stringOrThrow(results, Columns.TYPE)),
        JobName.of(stringOrThrow(results, Columns.NAME)),
        stringOrThrow(results, Columns.SIMPLE_NAME),
        stringOrNull(results, Columns.PARENT_JOB_NAME),
        uuidOrNull(results, Columns.PARENT_JOB_UUID),
        timestampOrThrow(results, Columns.CREATED_AT),
        timestampOrThrow(results, Columns.UPDATED_AT),
        urlOrNull(results, Columns.LOCATION),
        stringOrNull(results, Columns.DESCRIPTION),
        uuidOrNull(results, Columns.CURRENT_VERSION_UUID),
        toStringList(results, "labels"),
        toTagSet(results, "tags"),
        toFacetsOrNull(results, "facets"));
  }

  private ImmutableList<String> toStringList(ResultSet results, String column) throws SQLException {
    if (results.getObject(column) == null) {
      return ImmutableList.of();
    }
    return ImmutableList.copyOf(stringArrayOrThrow(results, column));
  }

  private ImmutableSet<TagName> toTagSet(ResultSet results, String column) throws SQLException {
    if (results.getObject(column) == null) {
      return ImmutableSet.of();
    }
    List<String> tagList = stringArrayOrThrow(results, column);
    return ImmutableSet.copyOf(tagList.stream().map(TagName::of).collect(Collectors.toSet()));
  }
}
