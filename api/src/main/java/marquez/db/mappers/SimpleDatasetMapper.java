/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.db.mappers;

import static marquez.db.Columns.booleanOrDefault;
import static marquez.db.Columns.stringArrayOrThrow;
import static marquez.db.Columns.stringOrNull;
import static marquez.db.Columns.stringOrThrow;
import static marquez.db.Columns.timestampOrNull;
import static marquez.db.Columns.timestampOrThrow;
import static marquez.db.Columns.uuidOrNull;
import static marquez.db.mappers.MapperUtils.toFacetsOrNull;

import com.fasterxml.jackson.core.type.TypeReference;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSet;
import java.sql.ResultSet;
import java.sql.SQLException;
import java.util.List;
import java.util.stream.Collectors;
import lombok.NonNull;
import lombok.extern.slf4j.Slf4j;
import marquez.api.models.SimpleDataset;
import marquez.common.Utils;
import marquez.common.models.DatasetId;
import marquez.common.models.DatasetName;
import marquez.common.models.DatasetType;
import marquez.common.models.Field;
import marquez.common.models.NamespaceName;
import marquez.common.models.SourceName;
import marquez.common.models.TagName;
import marquez.db.Columns;
import org.jdbi.v3.core.mapper.RowMapper;
import org.jdbi.v3.core.statement.StatementContext;

/** A {@link RowMapper} for {@link SimpleDataset}. */
@Slf4j
public final class SimpleDatasetMapper implements RowMapper<SimpleDataset> {
  @Override
  public SimpleDataset map(@NonNull ResultSet results, @NonNull StatementContext context)
      throws SQLException {
    return new SimpleDataset(
        new DatasetId(
            NamespaceName.of(stringOrThrow(results, Columns.NAMESPACE_NAME)),
            DatasetName.of(stringOrThrow(results, Columns.NAME))),
        DatasetType.valueOf(stringOrThrow(results, Columns.TYPE)),
        DatasetName.of(stringOrThrow(results, Columns.NAME)),
        DatasetName.of(stringOrThrow(results, Columns.PHYSICAL_NAME)),
        timestampOrThrow(results, Columns.CREATED_AT),
        timestampOrThrow(results, Columns.UPDATED_AT),
        SourceName.of(stringOrThrow(results, Columns.SOURCE_NAME)),
        toTagSet(results, "tags"),
        timestampOrNull(results, Columns.LAST_MODIFIED_AT),
        stringOrNull(results, Columns.LIFECYCLE_STATE),
        stringOrNull(results, Columns.DESCRIPTION),
        uuidOrNull(results, Columns.CURRENT_VERSION_UUID),
        booleanOrDefault(results, Columns.IS_DELETED, false),
        toFieldsOrNull(results, "fields"),
        toFacetsOrNull(results, "facets"));
  }

  private ImmutableSet<TagName> toTagSet(ResultSet results, String column) throws SQLException {
    if (results.getObject(column) == null) {
      return ImmutableSet.of();
    }
    List<String> tagList = stringArrayOrThrow(results, column);
    return ImmutableSet.copyOf(tagList.stream().map(TagName::of).collect(Collectors.toSet()));
  }

  private ImmutableList<Field> toFieldsOrNull(ResultSet results, String column)
      throws SQLException {
    final String fieldsAsString = stringOrNull(results, column);
    if (fieldsAsString == null) {
      return null;
    }
    try {
      return ImmutableList.copyOf(
          Utils.fromJson(fieldsAsString, new TypeReference<List<Field>>() {}));
    } catch (Exception e) {
      log.error("Failed to parse fields: {}", fieldsAsString, e);
      return null;
    }
  }
}
