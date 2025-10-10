/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.api.models;

import com.google.common.collect.ImmutableSet;
import jakarta.annotation.Nullable;
import java.time.Instant;
import java.util.Optional;
import java.util.UUID;
import lombok.EqualsAndHashCode;
import lombok.Getter;
import lombok.NonNull;
import lombok.ToString;
import marquez.common.models.DatasetId;
import marquez.common.models.DatasetName;
import marquez.common.models.DatasetType;
import marquez.common.models.NamespaceName;
import marquez.common.models.SourceName;
import marquez.common.models.TagName;

/**
 * Simplified Dataset model for full search results with configurable facets. Contains essential
 * dataset information with optional facets and fields based on API parameters.
 */
@EqualsAndHashCode
@ToString
public final class SimpleDataset {
  @Getter private final DatasetId id;
  @Getter private final DatasetType type;
  @Getter private final DatasetName name;
  @Getter private final DatasetName physicalName;
  @Getter private final Instant createdAt;
  @Getter private final Instant updatedAt;
  @Getter private final NamespaceName namespace;
  @Getter private final SourceName sourceName;
  @Getter private final ImmutableSet<TagName> tags;
  @Nullable private final Instant lastModifiedAt;
  @Nullable private final String lastLifecycleState;
  @Nullable private final String description;
  @Nullable private final UUID currentVersion;
  @Getter private final boolean isDeleted;

  @Getter @Nullable
  private final com.google.common.collect.ImmutableList<marquez.common.models.Field> fields;

  @Getter @Nullable private final com.google.common.collect.ImmutableMap<String, Object> facets;

  public SimpleDataset(
      @NonNull final DatasetId id,
      @NonNull final DatasetType type,
      @NonNull final DatasetName name,
      @NonNull final DatasetName physicalName,
      @NonNull final Instant createdAt,
      @NonNull final Instant updatedAt,
      @NonNull final SourceName sourceName,
      @Nullable final ImmutableSet<TagName> tags,
      @Nullable final Instant lastModifiedAt,
      @Nullable final String lastLifecycleState,
      @Nullable final String description,
      @Nullable final UUID currentVersion,
      boolean isDeleted,
      @Nullable final com.google.common.collect.ImmutableList<marquez.common.models.Field> fields,
      @Nullable final com.google.common.collect.ImmutableMap<String, Object> facets) {
    this.id = id;
    this.type = type;
    this.name = name;
    this.physicalName = physicalName;
    this.createdAt = createdAt;
    this.updatedAt = updatedAt;
    this.namespace = id.getNamespace();
    this.sourceName = sourceName;
    this.tags = (tags == null) ? ImmutableSet.of() : tags;
    this.lastModifiedAt = lastModifiedAt;
    this.lastLifecycleState = lastLifecycleState;
    this.description = description;
    this.currentVersion = currentVersion;
    this.isDeleted = isDeleted;
    this.fields = fields;
    this.facets = facets;
  }

  public Optional<Instant> getLastModifiedAt() {
    return Optional.ofNullable(lastModifiedAt);
  }

  public Optional<String> getDescription() {
    return Optional.ofNullable(description);
  }

  public Optional<String> getLastLifecycleState() {
    return Optional.ofNullable(lastLifecycleState);
  }

  public Optional<UUID> getCurrentVersion() {
    return Optional.ofNullable(currentVersion);
  }
}
