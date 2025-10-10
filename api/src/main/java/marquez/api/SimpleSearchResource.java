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
import marquez.api.models.SimpleSearchResult;
import marquez.api.models.SimpleSearchSort;
import marquez.db.SimpleSearchDao;

@Slf4j
@Path("/api/v1/search/simple")
public class SimpleSearchResource {
  private static final String DEFAULT_SORT = "updated_at";
  private static final String DEFAULT_LIMIT = "20";
  private static final int MIN_LIMIT = 1;
  private static final int MAX_LIMIT = 100;

  private final SimpleSearchDao simpleSearchDao;

  public SimpleSearchResource(@NonNull final SimpleSearchDao simpleSearchDao) {
    this.simpleSearchDao = simpleSearchDao;
  }

  /**
   * Simple search endpoint that performs name-based search across datasets and jobs without
   * requiring external dependencies like OpenSearch/Elasticsearch. Returns simplified results with
   * references to full objects for UI navigation. Results are sorted by updated_at DESC (newest
   * first) by default for better user experience.
   *
   * @param query Search query string (supports prefix/postfix matching)
   * @param filter Optional filter to restrict results to datasets or jobs only
   * @param sort Sort order for results (name or updated_at)
   * @param limit Maximum number of results to return
   * @param namespace Optional namespace filter
   * @return Response containing simple search results
   */
  @Timed
  @ResponseMetered
  @ExceptionMetered
  @GET
  @Produces(MediaType.APPLICATION_JSON)
  public Response simpleSearch(
      @QueryParam("q") @NotBlank String query,
      @QueryParam("filter") @Nullable SearchFilter filter,
      @QueryParam("sort") @DefaultValue(DEFAULT_SORT) SimpleSearchSort sort,
      @QueryParam("limit") @DefaultValue(DEFAULT_LIMIT) @Min(MIN_LIMIT) int limit,
      @QueryParam("namespace") @Nullable String namespace) {

    // Enforce maximum limit for performance
    final int actualLimit = Math.min(limit, MAX_LIMIT);

    final List<SimpleSearchResult> searchResults =
        simpleSearchDao.simpleSearch(query, filter, sort, actualLimit, namespace);

    return Response.ok(new SimpleSearchResults(searchResults)).build();
  }

  /** Wrapper for {@link SimpleSearchResult}s which also contains metadata. */
  @ToString
  public static final class SimpleSearchResults {
    @Getter private final int totalCount;
    @Getter private final List<SimpleSearchResult> results;

    @JsonCreator
    public SimpleSearchResults(@NonNull final List<SimpleSearchResult> results) {
      this.totalCount = results.size();
      this.results = results;
    }
  }
}
