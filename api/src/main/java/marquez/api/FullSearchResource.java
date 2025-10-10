/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.api;

import com.codahale.metrics.annotation.ExceptionMetered;
import com.codahale.metrics.annotation.ResponseMetered;
import com.codahale.metrics.annotation.Timed;
import com.fasterxml.jackson.annotation.JsonCreator;
import jakarta.annotation.Nullable;
import jakarta.inject.Inject;
import jakarta.validation.constraints.Min;
import jakarta.validation.constraints.NotBlank;
import jakarta.ws.rs.DefaultValue;
import jakarta.ws.rs.GET;
import jakarta.ws.rs.Path;
import jakarta.ws.rs.Produces;
import jakarta.ws.rs.QueryParam;
import jakarta.ws.rs.core.MediaType;
import jakarta.ws.rs.core.Response;
import java.util.List;
import lombok.Getter;
import lombok.NonNull;
import lombok.ToString;
import lombok.extern.slf4j.Slf4j;
import marquez.api.models.SearchFilter;
import marquez.api.models.SimpleDataset;
import marquez.api.models.SimpleJob;
import marquez.api.models.SimpleSearchSort;
import marquez.db.FullSearchDao;

@Slf4j
@Path("/api/v1/search/full")
public class FullSearchResource {
  private static final String DEFAULT_SORT = "updated_at";
  private static final String DEFAULT_LIMIT = "20";
  private static final String DEFAULT_OFFSET = "0";
  private static final int MIN_LIMIT = 1;
  private static final int MAX_LIMIT = 100;
  private static final int MIN_OFFSET = 0;

  private final FullSearchDao fullSearchDao;

  @Inject
  public FullSearchResource(@NonNull final FullSearchDao fullSearchDao) {
    this.fullSearchDao = fullSearchDao;
  }

  /**
   * Full search endpoint that performs name-based search across datasets and jobs and returns
   * Dataset and Job objects with configurable facets. This endpoint supports facet collection
   * control for performance optimization while maintaining access to metadata when needed. Results
   * are sorted by updated_at DESC (newest first) by default for better user experience.
   *
   * @param query Search query string (supports prefix/postfix matching)
   * @param filter Optional filter to restrict results to datasets or jobs only
   * @param sort Sort order for results (name or updated_at)
   * @param limit Maximum number of results to return
   * @param offset Number of results to skip for pagination
   * @param namespace Optional namespace filter
   * @param facets Whether to include facets in the response (default: true)
   * @param facetNames List of specific facet names to include (default: all facets)
   * @return Response containing full search results with configurable facets
   */
  @Timed
  @ResponseMetered
  @ExceptionMetered
  @GET
  @Produces(MediaType.APPLICATION_JSON)
  public Response fullSearch(
      @QueryParam("q") @NotBlank String query,
      @QueryParam("filter") @Nullable SearchFilter filter,
      @QueryParam("sort") @DefaultValue(DEFAULT_SORT) SimpleSearchSort sort,
      @QueryParam("limit") @DefaultValue(DEFAULT_LIMIT) @Min(MIN_LIMIT) int limit,
      @QueryParam("offset") @DefaultValue(DEFAULT_OFFSET) @Min(MIN_OFFSET) int offset,
      @QueryParam("namespace") @Nullable String namespace,
      @QueryParam("facets") @DefaultValue("true") boolean facets,
      @QueryParam("facetNames") @Nullable List<String> facetNames) {

    // Enforce maximum limit for performance
    final int actualLimit = Math.min(limit, MAX_LIMIT);

    List<SimpleDataset> datasets = List.of();
    List<SimpleJob> jobs = List.of();
    int totalDatasetCount = 0;
    int totalJobCount = 0;

    // Calculate smart limits for each type based on filter and available data
    int datasetLimit;
    int jobLimit;
    int datasetOffset = offset;
    int jobOffset = offset;

    if (filter == SearchFilter.DATASET) {
      // Only datasets requested
      datasetLimit = actualLimit;
      jobLimit = 0;
    } else if (filter == SearchFilter.JOB) {
      // Only jobs requested
      datasetLimit = 0;
      jobLimit = actualLimit;
    } else {
      // Both datasets and jobs requested - need smart distribution
      // First, get counts to understand availability
      totalDatasetCount = fullSearchDao.countDatasets(query, namespace);
      totalJobCount = fullSearchDao.countJobs(query, namespace);

      // Calculate optimal distribution
      int[] offsets = calculateOptimalLimits(offset, totalDatasetCount, totalJobCount);
      int[] limits =
          calculateOptimalLimits(
              actualLimit, totalDatasetCount - offsets[0], totalJobCount - offsets[1]);
      datasetLimit = limits[0];
      jobLimit = limits[1];
      datasetOffset = offsets[0];
      jobOffset = offsets[1];
    }

    // Retrieve datasets if needed
    if (datasetLimit > 0) {
      datasets =
          fullSearchDao.searchDatasets(
              query,
              sort,
              datasetLimit,
              datasetOffset,
              namespace,
              facets,
              facets ? facetNames : null);
      if (filter != null) {
        totalDatasetCount = fullSearchDao.countDatasets(query, namespace);
      }
    }

    // Retrieve jobs if needed
    if (jobLimit > 0) {
      jobs =
          fullSearchDao.searchJobs(
              query, sort, jobLimit, jobOffset, namespace, facets, facets ? facetNames : null);
      if (filter != null) {
        totalJobCount = fullSearchDao.countJobs(query, namespace);
      }
    }

    final int totalCount = totalDatasetCount + totalJobCount;
    return Response.ok(new FullSearchResults(totalCount, datasets, jobs)).build();
  }

  /**
   * Calculates optimal limits for datasets and jobs to distribute the total limit as equally as
   * possible while ensuring we return up to the limit if data is available.
   *
   * @param totalLimit Total number of results allowed
   * @param availableDatasets Number of datasets available
   * @param availableJobs Number of jobs available
   * @return Array with [datasetLimit, jobLimit]
   */
  private int[] calculateOptimalLimits(int totalLimit, int availableDatasets, int availableJobs) {
    // If no data available, return zeros
    if (availableDatasets == 0 && availableJobs == 0) {
      return new int[] {0, 0};
    }

    // If only one type has data, allocate all to that type
    if (availableDatasets == 0) {
      return new int[] {0, Math.min(totalLimit, availableJobs)};
    }
    if (availableJobs == 0) {
      return new int[] {Math.min(totalLimit, availableDatasets), 0};
    }

    // Both types have data - distribute as equally as possible
    int baseAllocation = totalLimit / 2;
    int remainder = totalLimit % 2;

    int datasetLimit = Math.min(baseAllocation, availableDatasets);
    int jobLimit = Math.min(baseAllocation, availableJobs);

    // Distribute remainder and any unused allocation
    int unusedFromDatasets = Math.max(0, baseAllocation - availableDatasets);
    int unusedFromJobs = Math.max(0, baseAllocation - availableJobs);

    // Give remainder to datasets first (arbitrary choice)
    if (remainder > 0 && datasetLimit < availableDatasets) {
      datasetLimit = Math.min(datasetLimit + remainder, availableDatasets);
      remainder = Math.max(0, remainder - (datasetLimit - baseAllocation));
    }

    // Redistribute unused allocation from one type to the other
    if (unusedFromDatasets > 0 && jobLimit < availableJobs) {
      jobLimit = Math.min(jobLimit + unusedFromDatasets, availableJobs);
    }
    if (unusedFromJobs > 0 && datasetLimit < availableDatasets) {
      datasetLimit = Math.min(datasetLimit + unusedFromJobs, availableDatasets);
    }

    // Give any remaining remainder to jobs if datasets couldn't use it
    if (remainder > 0 && jobLimit < availableJobs) {
      jobLimit = Math.min(jobLimit + remainder, availableJobs);
    }

    return new int[] {datasetLimit, jobLimit};
  }

  /** Wrapper for full search results containing simplified Dataset and Job objects. */
  @ToString
  public static final class FullSearchResults {
    @Getter private final int totalCount;
    @Getter private final List<SimpleDataset> datasets;
    @Getter private final List<SimpleJob> jobs;

    @JsonCreator
    public FullSearchResults(
        int totalCount,
        @NonNull final List<SimpleDataset> datasets,
        @NonNull final List<SimpleJob> jobs) {
      this.totalCount = totalCount;
      this.datasets = datasets;
      this.jobs = jobs;
    }
  }
}
