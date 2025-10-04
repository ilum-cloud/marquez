/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.api.models;

import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSet;
import jakarta.annotation.Nullable;
import java.net.URL;
import java.time.Instant;
import java.util.Optional;
import java.util.UUID;
import lombok.EqualsAndHashCode;
import lombok.Getter;
import lombok.NonNull;
import lombok.ToString;
import marquez.common.models.JobId;
import marquez.common.models.JobName;
import marquez.common.models.JobType;
import marquez.common.models.NamespaceName;
import marquez.common.models.TagName;

/**
 * Simplified Job model for full search results with configurable facets. Contains essential job
 * information with optional facets based on API parameters.
 */
@EqualsAndHashCode
@ToString
public final class SimpleJob {
  @Getter private final JobId id;
  @Getter private final JobType type;
  @Getter private final JobName name;
  @Getter private final String simpleName;
  @Getter private final String parentJobName;
  @Getter private final UUID parentJobUuid;
  @Getter private final Instant createdAt;
  @Getter private final Instant updatedAt;
  @Getter private final NamespaceName namespace;
  @Nullable private final URL location;
  @Nullable private final String description;
  @Nullable private UUID currentVersion;
  @Getter @Nullable private ImmutableList<String> labels;
  @Getter @Nullable private final ImmutableSet<TagName> tags;
  @Getter @Nullable private final com.google.common.collect.ImmutableMap<String, Object> facets;

  public SimpleJob(
      @NonNull final JobId id,
      @NonNull final JobType type,
      @NonNull final JobName name,
      @NonNull String simpleName,
      @Nullable String parentJobName,
      @Nullable UUID parentJobUuid,
      @NonNull final Instant createdAt,
      @NonNull final Instant updatedAt,
      @Nullable final URL location,
      @Nullable final String description,
      @Nullable UUID currentVersion,
      @Nullable ImmutableList<String> labels,
      @Nullable final ImmutableSet<TagName> tags,
      @Nullable final com.google.common.collect.ImmutableMap<String, Object> facets) {
    this.id = id;
    this.type = type;
    this.name = name;
    this.simpleName = simpleName;
    this.parentJobName = parentJobName;
    this.parentJobUuid = parentJobUuid;
    this.createdAt = createdAt;
    this.updatedAt = updatedAt;
    this.namespace = id.getNamespace();
    this.location = location;
    this.description = description;
    this.currentVersion = currentVersion;
    this.labels = labels;
    this.tags = tags;
    this.facets = facets;
  }

  public Optional<URL> getLocation() {
    return Optional.ofNullable(location);
  }

  public Optional<String> getDescription() {
    return Optional.ofNullable(description);
  }

  public Optional<UUID> getCurrentVersion() {
    return Optional.ofNullable(currentVersion);
  }
}
