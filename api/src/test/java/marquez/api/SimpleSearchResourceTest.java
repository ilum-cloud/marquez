/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.ArgumentMatchers.isNull;
import static org.mockito.Mockito.mock;
import static org.mockito.Mockito.when;

import com.google.common.collect.ImmutableList;
import io.dropwizard.testing.junit5.DropwizardExtensionsSupport;
import io.dropwizard.testing.junit5.ResourceExtension;
import jakarta.ws.rs.core.Response;
import java.time.Instant;
import java.util.List;
import marquez.api.models.SearchFilter;
import marquez.api.models.SimpleSearchResult;
import marquez.api.models.SimpleSearchSort;
import marquez.common.models.DatasetName;
import marquez.common.models.JobName;
import marquez.common.models.NamespaceName;
import marquez.db.SimpleSearchDao;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

@ExtendWith(DropwizardExtensionsSupport.class)
class SimpleSearchResourceTest {

  private static final SimpleSearchDao simpleSearchDao = mock(SimpleSearchDao.class);
  private static final ResourceExtension UNDER_TEST =
      ResourceExtension.builder().addResource(new SimpleSearchResource(simpleSearchDao)).build();

  @Test
  void testSimpleSearch_basic() {
    // Given
    final String query = "test";
    final List<SimpleSearchResult> mockResults =
        ImmutableList.of(
            SimpleSearchResult.newDatasetResult(
                DatasetName.of("test_dataset"), NamespaceName.of("test_namespace"), Instant.now()),
            SimpleSearchResult.newJobResult(
                JobName.of("test_job"), NamespaceName.of("test_namespace"), Instant.now()));

    when(simpleSearchDao.simpleSearch(
            eq(query), isNull(), eq(SimpleSearchSort.UPDATED_AT), eq(20), isNull()))
        .thenReturn(mockResults);

    // When
    final Response response =
        UNDER_TEST.target("/api/v1/search/simple").queryParam("q", query).request().get();

    // Then
    assertThat(response.getStatus()).isEqualTo(200);
    final SimpleSearchResource.SimpleSearchResults results =
        response.readEntity(SimpleSearchResource.SimpleSearchResults.class);
    assertThat(results.getTotalCount()).isEqualTo(2);
    assertThat(results.getResults()).hasSize(2);
    assertThat(results.getResults().get(0).getType())
        .isEqualTo(SimpleSearchResult.ResultType.DATASET);
    assertThat(results.getResults().get(1).getType()).isEqualTo(SimpleSearchResult.ResultType.JOB);
  }

  @Test
  void testSimpleSearch_withFilter() {
    // Given
    final String query = "test";
    final List<SimpleSearchResult> mockResults =
        ImmutableList.of(
            SimpleSearchResult.newDatasetResult(
                DatasetName.of("test_dataset"), NamespaceName.of("test_namespace"), Instant.now()));

    when(simpleSearchDao.simpleSearch(
            eq(query), eq(SearchFilter.DATASET), eq(SimpleSearchSort.UPDATED_AT), eq(20), isNull()))
        .thenReturn(mockResults);

    // When
    final Response response =
        UNDER_TEST
            .target("/api/v1/search/simple")
            .queryParam("q", query)
            .queryParam("filter", "dataset")
            .request()
            .get();

    // Then
    assertThat(response.getStatus()).isEqualTo(200);
    final SimpleSearchResource.SimpleSearchResults results =
        response.readEntity(SimpleSearchResource.SimpleSearchResults.class);
    assertThat(results.getTotalCount()).isEqualTo(1);
    assertThat(results.getResults()).hasSize(1);
    assertThat(results.getResults().get(0).getType())
        .isEqualTo(SimpleSearchResult.ResultType.DATASET);
  }

  @Test
  void testSimpleSearch_withLimit() {
    // Given
    final String query = "test";
    final int limit = 5;
    final List<SimpleSearchResult> mockResults =
        ImmutableList.of(
            SimpleSearchResult.newDatasetResult(
                DatasetName.of("test_dataset"), NamespaceName.of("test_namespace"), Instant.now()));

    when(simpleSearchDao.simpleSearch(
            eq(query), isNull(), eq(SimpleSearchSort.UPDATED_AT), eq(limit), isNull()))
        .thenReturn(mockResults);

    // When
    final Response response =
        UNDER_TEST
            .target("/api/v1/search/simple")
            .queryParam("q", query)
            .queryParam("limit", limit)
            .request()
            .get();

    // Then
    assertThat(response.getStatus()).isEqualTo(200);
  }

  @Test
  void testSimpleSearch_enforcesMaxLimit() {
    // Given
    final String query = "test";
    final int requestedLimit = 200; // Above max limit of 100
    final List<SimpleSearchResult> mockResults = ImmutableList.of();

    when(simpleSearchDao.simpleSearch(
            eq(query),
            isNull(),
            eq(SimpleSearchSort.UPDATED_AT),
            eq(100), // Should be capped at 100
            isNull()))
        .thenReturn(mockResults);

    // When
    final Response response =
        UNDER_TEST
            .target("/api/v1/search/simple")
            .queryParam("q", query)
            .queryParam("limit", requestedLimit)
            .request()
            .get();

    // Then
    assertThat(response.getStatus()).isEqualTo(200);
  }

  @Test
  void testSimpleSearch_withNamespace() {
    // Given
    final String query = "test";
    final String namespace = "test_namespace";
    final List<SimpleSearchResult> mockResults = ImmutableList.of();

    when(simpleSearchDao.simpleSearch(
            eq(query), isNull(), eq(SimpleSearchSort.UPDATED_AT), eq(20), eq(namespace)))
        .thenReturn(mockResults);

    // When
    final Response response =
        UNDER_TEST
            .target("/api/v1/search/simple")
            .queryParam("q", query)
            .queryParam("namespace", namespace)
            .request()
            .get();

    // Then
    assertThat(response.getStatus()).isEqualTo(200);
  }
}
