/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.db;

import static org.assertj.core.api.Assertions.assertThat;

import java.util.List;
import java.util.Map;
import java.util.stream.Collectors;
import lombok.NonNull;
import marquez.api.models.SearchFilter;
import marquez.api.models.SimpleSearchResult;
import marquez.api.models.SimpleSearchSort;
import marquez.jdbi.MarquezJdbiExternalPostgresExtension;
import org.jdbi.v3.core.Jdbi;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

/** The test suite for {@link SimpleSearchDao}. */
@Tag("DataAccessTests")
@ExtendWith(MarquezJdbiExternalPostgresExtension.class)
public class SimpleSearchDaoTest {

  static final int LIMIT = 25;
  static final int NUM_OF_JOBS = 2;
  static final int NUM_OF_DATASETS = 12;

  static SimpleSearchDao simpleSearchDao;

  @BeforeAll
  public static void setUpOnce(final Jdbi jdbi) {
    simpleSearchDao = jdbi.onDemand(SimpleSearchDao.class);

    // Set up test data similar to SearchDaoTest
    DbTestUtils.newDataset(jdbi, "simple_search_dataset_0");
    DbTestUtils.newDataset(jdbi, "simple_search_dataset_1");
    DbTestUtils.newDataset(jdbi, "simple_search_dataset_2");

    DbTestUtils.newDataset(jdbi, "namespace1", "simpleDatasetA");
    DbTestUtils.newDataset(jdbi, "namespace1", "simpleDatasetB");
    DbTestUtils.newDataset(jdbi, "namespace2", "simpleDatasetC");

    // Create some jobs for testing
    DbTestUtils.newJobs(jdbi, NUM_OF_JOBS);
  }

  @Test
  public void testSimpleSearch_basic() {
    final String query = "simple";
    final List<SimpleSearchResult> results = simpleSearchDao.simpleSearch(query, LIMIT);

    // Ensure search results contain datasets and jobs with "simple" in name
    assertThat(results).isNotEmpty();
    assertThat(results).allMatch(result -> result.getName().toLowerCase().contains("simple"));
  }

  @Test
  public void testSimpleSearch_withFilter() {
    final String query = "simple";
    final List<SimpleSearchResult> datasetResults =
        simpleSearchDao.simpleSearch(query, SearchFilter.DATASET, LIMIT);

    // Ensure filtered results contain only datasets
    assertThat(datasetResults).isNotEmpty();
    assertThat(datasetResults)
        .allMatch(result -> result.getType() == SimpleSearchResult.ResultType.DATASET);
  }

  @Test
  public void testSimpleSearch_withNamespace() {
    final String query = "simpleDataset";
    final String namespace = "namespace1";
    final List<SimpleSearchResult> results =
        simpleSearchDao.simpleSearch(
            query, SearchFilter.DATASET, SimpleSearchSort.UPDATED_AT, LIMIT, namespace);

    // Ensure results are from the specified namespace
    assertThat(results).hasSize(2);
    assertThat(results).allMatch(result -> result.getNamespace().getValue().equals(namespace));
    assertThat(results).extracting("name").contains("simpleDatasetA", "simpleDatasetB");
  }

  @Test
  public void testSimpleSearch_sortByName() {
    final String query = "simple_search_dataset";
    final List<SimpleSearchResult> results =
        simpleSearchDao.simpleSearch(query, null, SimpleSearchSort.NAME, LIMIT, null);

    // Ensure results are sorted by name
    assertThat(results).hasSize(3);
    final List<String> names =
        results.stream().map(SimpleSearchResult::getName).collect(Collectors.toList());
    assertThat(names).isSorted();
  }

  @Test
  public void testSimpleSearch_sortByUpdatedAt() {
    final String query = "simple_search_dataset";
    final List<SimpleSearchResult> results =
        simpleSearchDao.simpleSearch(query, null, SimpleSearchSort.UPDATED_AT, LIMIT, null);

    // Ensure results are sorted by updated_at descending (newest first)
    assertThat(results).hasSize(3);
    for (int i = 0; i < results.size() - 1; i++) {
      assertThat(results.get(i).getUpdatedAt()).isAfterOrEqualTo(results.get(i + 1).getUpdatedAt());
    }
  }

  @Test
  public void testSimpleSearch_limitRespected() {
    final String query = "test"; // This should match many results
    final int smallLimit = 5;
    final List<SimpleSearchResult> results =
        simpleSearchDao.simpleSearch(query, null, SimpleSearchSort.UPDATED_AT, smallLimit, null);

    // Ensure limit is respected
    assertThat(results).hasSizeLessThanOrEqualTo(smallLimit);
  }

  @Test
  public void testSimpleSearch_noResults() {
    final String query = "nonexistent_query_string";
    final List<SimpleSearchResult> results = simpleSearchDao.simpleSearch(query, LIMIT);

    assertThat(results).isEmpty();
  }

  @Test
  public void testSimpleSearch_partialMatch() {
    final String query = "Dataset"; // Should match datasets with "Dataset" in name
    final List<SimpleSearchResult> results =
        simpleSearchDao.simpleSearch(query, SearchFilter.DATASET, LIMIT);

    // Ensure partial matching works (case insensitive)
    assertThat(results).isNotEmpty();
    assertThat(results).allMatch(result -> result.getName().toLowerCase().contains("dataset"));
  }

  /** Returns search results grouped by {@link SimpleSearchResult.ResultType}. */
  private Map<SimpleSearchResult.ResultType, List<SimpleSearchResult>> groupResultsByType(
      @NonNull List<SimpleSearchResult> results) {
    return results.stream().collect(Collectors.groupingBy(SimpleSearchResult::getType));
  }
}
