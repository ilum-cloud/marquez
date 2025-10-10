/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.api.models;

import java.time.Instant;
import lombok.NonNull;
import lombok.Value;
import marquez.common.models.DatasetName;
import marquez.common.models.JobName;
import marquez.common.models.NamespaceName;

/**
 * Represents a simplified search result for the simple search mechanism. Contains only essential
 * information with references to full objects for UI navigation.
 */
@Value
public class SimpleSearchResult {
  /** An {@code enum} used to determine the result type. */
  public enum ResultType {
    DATASET,
    JOB;
  }

  @NonNull ResultType type;
  @NonNull String name;
  @NonNull NamespaceName namespace;
  @NonNull Instant updatedAt;

  /**
   * Returns a new {@link SimpleSearchResult} object for a dataset.
   *
   * @param datasetName The name of the dataset.
   * @param namespace The namespace of the dataset.
   * @param updatedAt The updated timestamp of the dataset.
   * @return A {@link SimpleSearchResult} object for the dataset.
   */
  public static SimpleSearchResult newDatasetResult(
      @NonNull final DatasetName datasetName,
      @NonNull final NamespaceName namespace,
      @NonNull final Instant updatedAt) {
    return new SimpleSearchResult(ResultType.DATASET, datasetName.getValue(), namespace, updatedAt);
  }

  /**
   * Returns a new {@link SimpleSearchResult} object for a job.
   *
   * @param jobName The name of the job.
   * @param namespace The namespace of the job.
   * @param updatedAt The updated timestamp of the job.
   * @return A {@link SimpleSearchResult} object for the job.
   */
  public static SimpleSearchResult newJobResult(
      @NonNull final JobName jobName,
      @NonNull final NamespaceName namespace,
      @NonNull final Instant updatedAt) {
    return new SimpleSearchResult(ResultType.JOB, jobName.getValue(), namespace, updatedAt);
  }
}
