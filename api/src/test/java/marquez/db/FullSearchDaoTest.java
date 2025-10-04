/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.db;

import static org.assertj.core.api.Assertions.assertThat;

import java.util.Arrays;
import java.util.List;
import marquez.api.models.SimpleDataset;
import marquez.api.models.SimpleJob;
import marquez.api.models.SimpleSearchSort;
import marquez.jdbi.MarquezJdbiExternalPostgresExtension;
import org.jdbi.v3.core.Jdbi;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

/** The test suite for {@link FullSearchDao}. */
@Tag("DataAccessTests")
@ExtendWith(MarquezJdbiExternalPostgresExtension.class)
class FullSearchDaoTest {

  static final int LIMIT = 20;
  static final int NUM_OF_JOBS = 3;

  static FullSearchDao fullSearchDao;

  @BeforeAll
  static void setUpOnce(final Jdbi jdbi) {
    fullSearchDao = jdbi.onDemand(FullSearchDao.class);

    // Create test datasets with different names for testing
    DbTestUtils.newDataset(jdbi, "analytics_dataset");
    DbTestUtils.newDataset(jdbi, "user_analytics");
    DbTestUtils.newDataset(jdbi, "sales_data");
    DbTestUtils.newDataset(jdbi, "customer_info");
    DbTestUtils.newDataset(jdbi, "namespace1", "analytics_subset");
    DbTestUtils.newDataset(jdbi, "namespace2", "other_data");

    // Create test jobs
    DbTestUtils.newJobs(jdbi, NUM_OF_JOBS);

    // Create additional jobs with specific names for testing
    DbTestUtils.newJob(jdbi, "analytics_job");
    DbTestUtils.newJob(jdbi, "etl_job");
    DbTestUtils.newJob(jdbi, "namespace1", "analytics_processor");
  }

  @Test
  void testSearchDatasets_empty() {
    // Test with empty query
    List<SimpleDataset> datasets =
        fullSearchDao.searchDatasets("", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(datasets).isNotEmpty();
  }

  @Test
  void testSearchDatasets_withQuery() {
    List<SimpleDataset> datasets =
        fullSearchDao.searchDatasets(
            "analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(datasets)
        .isNotEmpty()
        .allMatch(dataset -> dataset.getName().getValue().toLowerCase().contains("analytics"));
  }

  @Test
  void testSearchDatasets_withNamespace() {
    List<SimpleDataset> datasets =
        fullSearchDao.searchDatasets("", SimpleSearchSort.NAME, LIMIT, 0, "namespace1", true, null);
    assertThat(datasets)
        .isNotEmpty()
        .allMatch(dataset -> dataset.getNamespace().getValue().equals("namespace1"));
  }

  @Test
  void testSearchDatasets_sortByUpdatedAt() {
    List<SimpleDataset> datasets =
        fullSearchDao.searchDatasets("", SimpleSearchSort.UPDATED_AT, LIMIT, 0, null, true, null);
    assertThat(datasets).isNotEmpty();
    // Verify datasets are sorted by updated_at DESC (most recent first)
    for (int i = 1; i < datasets.size(); i++) {
      assertThat(datasets.get(i - 1).getUpdatedAt())
          .isAfterOrEqualTo(datasets.get(i).getUpdatedAt());
    }
  }

  @Test
  void testSearchDatasets_sortByName() {
    List<SimpleDataset> datasets =
        fullSearchDao.searchDatasets("", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(datasets).isNotEmpty();
    // Verify datasets are sorted by name ASC
    for (int i = 1; i < datasets.size(); i++) {
      assertThat(datasets.get(i - 1).getName().getValue())
          .isLessThanOrEqualTo(datasets.get(i).getName().getValue());
    }
  }

  @Test
  void testSearchDatasets_withLimit() {
    List<SimpleDataset> datasets =
        fullSearchDao.searchDatasets("", SimpleSearchSort.NAME, 2, 0, null, true, null);
    assertThat(datasets).hasSize(2);
  }

  @Test
  void testSearchJobs_empty() {
    // Test with empty query
    List<SimpleJob> allJobs =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(allJobs).isNotEmpty();
  }

  @Test
  void testSearchJobs_withQuery() {
    List<SimpleJob> jobs =
        fullSearchDao.searchJobs("analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(jobs)
        .isNotEmpty()
        .allMatch(job -> job.getName().getValue().toLowerCase().contains("analytics"));
  }

  @Test
  void testSearchJobs_withNamespace() {
    List<SimpleJob> jobs =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, LIMIT, 0, "namespace1", true, null);
    assertThat(jobs)
        .isNotEmpty()
        .allMatch(job -> job.getNamespace().getValue().equals("namespace1"));
  }

  @Test
  void testSearchJobs_sortByUpdatedAt() {
    List<SimpleJob> jobs =
        fullSearchDao.searchJobs("", SimpleSearchSort.UPDATED_AT, LIMIT, 0, null, true, null);
    assertThat(jobs).isNotEmpty();
    // Verify jobs are sorted by updated_at DESC (most recent first)
    for (int i = 1; i < jobs.size(); i++) {
      assertThat(jobs.get(i - 1).getUpdatedAt()).isAfterOrEqualTo(jobs.get(i).getUpdatedAt());
    }
  }

  @Test
  void testSearchJobs_sortByName() {
    List<SimpleJob> jobs =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(jobs).isNotEmpty();
    // Verify jobs are sorted by name ASC
    for (int i = 1; i < jobs.size(); i++) {
      assertThat(jobs.get(i - 1).getName().getValue())
          .isLessThanOrEqualTo(jobs.get(i).getName().getValue());
    }
  }

  @Test
  void testSearchJobs_withLimit() {
    List<SimpleJob> jobs =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, 2, 0, null, true, null);
    assertThat(jobs).hasSize(2);
    // Also test with different limit
    List<SimpleJob> morJobs =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, 5, 0, null, true, null);
    assertThat(morJobs).hasSizeGreaterThanOrEqualTo(jobs.size());
  }

  @Test
  void testCountDatasets() {
    int count = fullSearchDao.countDatasets("analytics", null);
    assertThat(count).isGreaterThan(0);
  }

  @Test
  void testCountJobs() {
    int count = fullSearchDao.countJobs("analytics", null);
    assertThat(count).isGreaterThan(0);
  }

  @Test
  void testCaseInsensitiveSearch() {
    // Test case insensitive search for datasets
    List<SimpleDataset> upperDatasets =
        fullSearchDao.searchDatasets(
            "ANALYTICS", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    List<SimpleDataset> lowerDatasets =
        fullSearchDao.searchDatasets(
            "analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    List<SimpleDataset> mixedDatasets =
        fullSearchDao.searchDatasets(
            "Analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(upperDatasets).hasSameSizeAs(lowerDatasets).hasSameSizeAs(mixedDatasets);

    // Test case insensitive search for jobs
    List<SimpleJob> upperJobs =
        fullSearchDao.searchJobs("ANALYTICS", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    List<SimpleJob> lowerJobs =
        fullSearchDao.searchJobs("analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    List<SimpleJob> mixedJobs =
        fullSearchDao.searchJobs("Analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(upperJobs).hasSameSizeAs(lowerJobs).hasSameSizeAs(mixedJobs);
  }

  @Test
  void testPartialMatching() {
    // Test prefix matching
    List<SimpleDataset> prefixDatasets =
        fullSearchDao.searchDatasets("analy", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(prefixDatasets)
        .isNotEmpty()
        .allMatch(dataset -> dataset.getName().getValue().toLowerCase().contains("analy"));

    // Test suffix matching
    List<SimpleDataset> suffixDatasets =
        fullSearchDao.searchDatasets("data", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(suffixDatasets)
        .isNotEmpty()
        .allMatch(dataset -> dataset.getName().getValue().toLowerCase().contains("data"));

    // Test infix matching for jobs
    List<SimpleJob> infixJobs =
        fullSearchDao.searchJobs("job", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(infixJobs)
        .isNotEmpty()
        .allMatch(job -> job.getName().getValue().toLowerCase().contains("job"));
  }

  @Test
  void testNonExistentSearch() {
    // Test searching for something that doesn't exist
    List<SimpleDataset> datasets =
        fullSearchDao.searchDatasets(
            "nonexistent_dataset_xyz", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(datasets).isEmpty();

    List<SimpleJob> jobs =
        fullSearchDao.searchJobs(
            "nonexistent_job_xyz", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(jobs).isEmpty();
  }

  @Test
  void testCountMethods() {
    // Test count methods
    int datasetCount = fullSearchDao.countDatasets("analytics", null);
    int jobCount = fullSearchDao.countJobs("analytics", null);

    assertThat(datasetCount).isGreaterThan(0);
    assertThat(jobCount).isGreaterThan(0);
  }

  @Test
  void testNamespaceFiltering() {
    // Test with a namespace that doesn't exist
    List<SimpleDataset> datasets =
        fullSearchDao.searchDatasets(
            "", SimpleSearchSort.NAME, LIMIT, 0, "nonexistent_namespace", true, null);
    assertThat(datasets).isEmpty();

    List<SimpleJob> jobs =
        fullSearchDao.searchJobs(
            "", SimpleSearchSort.NAME, LIMIT, 0, "nonexistent_namespace", true, null);
    assertThat(jobs).isEmpty();
  }

  @Test
  void testOffset() {
    // Test offset with datasets
    List<SimpleDataset> allDatasets =
        fullSearchDao.searchDatasets(
            "analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);

    if (allDatasets.size() > 1) {
      List<SimpleDataset> offsetDatasets =
          fullSearchDao.searchDatasets(
              "analytics", SimpleSearchSort.NAME, LIMIT, 1, null, true, null);
      assertThat(offsetDatasets).hasSizeLessThanOrEqualTo(allDatasets.size() - 1);

      // Verify that the first dataset in offset results is different from the first in non-offset
      // results
      if (!offsetDatasets.isEmpty() && allDatasets.size() > 1) {
        assertThat(offsetDatasets.get(0).getId()).isNotEqualTo(allDatasets.get(0).getId());
      }
    }

    // Test offset with jobs
    List<SimpleJob> allJobs =
        fullSearchDao.searchJobs("analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);

    if (allJobs.size() > 1) {
      List<SimpleJob> offsetJobs =
          fullSearchDao.searchJobs("analytics", SimpleSearchSort.NAME, LIMIT, 1, null, true, null);
      assertThat(offsetJobs).hasSizeLessThanOrEqualTo(allJobs.size() - 1);

      // Verify that the first job in offset results is different from the first in non-offset
      // results
      if (!offsetJobs.isEmpty() && allJobs.size() > 1) {
        assertThat(offsetJobs.get(0).getId()).isNotEqualTo(allJobs.get(0).getId());
      }
    }
  }

  @Test
  void testOffsetPagination() {
    // Test multiple pages with datasets
    List<SimpleDataset> page1 =
        fullSearchDao.searchDatasets("", SimpleSearchSort.NAME, 2, 0, null, true, null);
    List<SimpleDataset> page2 =
        fullSearchDao.searchDatasets("", SimpleSearchSort.NAME, 2, 2, null, true, null);

    if (page1.size() == 2 && !page2.isEmpty()) {
      // Ensure no overlap between pages
      assertThat(page1).doesNotContainAnyElementsOf(page2);
    }

    // Test multiple pages with jobs
    List<SimpleJob> jobPage1 =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, 2, 0, null, true, null);
    List<SimpleJob> jobPage2 =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, 2, 2, null, true, null);

    if (jobPage1.size() == 2 && !jobPage2.isEmpty()) {
      // Ensure no overlap between pages
      assertThat(jobPage1).doesNotContainAnyElementsOf(jobPage2);
    }
  }

  @Test
  void testIncludeFacets_false() {
    // Test with includeFacets = false for datasets
    List<SimpleDataset> datasetsWithoutFacets =
        fullSearchDao.searchDatasets(
            "analytics", SimpleSearchSort.NAME, LIMIT, 0, null, false, null);
    assertThat(datasetsWithoutFacets).isNotEmpty();

    // When includeFacets is false, facets should be empty or null
    for (SimpleDataset dataset : datasetsWithoutFacets) {
      if (dataset.getFacets() != null) {
        assertThat(dataset.getFacets()).isEmpty();
      }
    }

    // Test with includeFacets = false for jobs
    List<SimpleJob> jobsWithoutFacets =
        fullSearchDao.searchJobs("analytics", SimpleSearchSort.NAME, LIMIT, 0, null, false, null);
    assertThat(jobsWithoutFacets).isNotEmpty();

    // When includeFacets is false, facets should be empty or null
    for (SimpleJob job : jobsWithoutFacets) {
      if (job.getFacets() != null) {
        assertThat(job.getFacets()).isEmpty();
      }
    }
  }

  @Test
  void testIncludeFacets_true() {
    // Test with includeFacets = true for datasets
    List<SimpleDataset> datasetsWithFacets =
        fullSearchDao.searchDatasets("", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(datasetsWithFacets).isNotEmpty();

    // Verify that basic dataset properties are always present
    for (SimpleDataset dataset : datasetsWithFacets) {
      assertThat(dataset.getName()).isNotNull();
      assertThat(dataset.getNamespace()).isNotNull();
      assertThat(dataset.getCreatedAt()).isNotNull();
      assertThat(dataset.getUpdatedAt()).isNotNull();
    }

    // Test with includeFacets = true for jobs
    List<SimpleJob> jobsWithFacets =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    assertThat(jobsWithFacets).isNotEmpty();

    // Verify that basic job properties are always present
    for (SimpleJob job : jobsWithFacets) {
      assertThat(job.getName()).isNotNull();
      assertThat(job.getNamespace()).isNotNull();
      assertThat(job.getCreatedAt()).isNotNull();
      assertThat(job.getUpdatedAt()).isNotNull();
    }
  }

  @Test
  void testSpecificFacetNames() {
    // Test with specific facet names for datasets
    List<String> specificFacets = Arrays.asList("schema", "dataSource");
    List<SimpleDataset> datasetsWithSpecificFacets =
        fullSearchDao.searchDatasets(
            "", SimpleSearchSort.NAME, LIMIT, 0, null, true, specificFacets);
    assertThat(datasetsWithSpecificFacets).isNotEmpty();

    // All datasets should have the basic properties regardless of facet filtering
    for (SimpleDataset dataset : datasetsWithSpecificFacets) {
      assertThat(dataset.getName()).isNotNull();
      assertThat(dataset.getNamespace()).isNotNull();
    }

    // Test with specific facet names for jobs
    List<SimpleJob> jobsWithSpecificFacets =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, LIMIT, 0, null, true, specificFacets);
    assertThat(jobsWithSpecificFacets).isNotEmpty();

    // All jobs should have the basic properties regardless of facet filtering
    for (SimpleJob job : jobsWithSpecificFacets) {
      assertThat(job.getName()).isNotNull();
      assertThat(job.getNamespace()).isNotNull();
    }
  }

  @Test
  void testEmptyFacetNamesList() {
    // Test with empty facet names list (should include all facets when includeFacets=true)
    List<SimpleDataset> datasetsEmptyFacets =
        fullSearchDao.searchDatasets(
            "", SimpleSearchSort.NAME, LIMIT, 0, null, true, java.util.Collections.emptyList());
    List<SimpleDataset> datasetsNullFacets =
        fullSearchDao.searchDatasets("", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);

    // Both should return the same results when includeFacets is true
    assertThat(datasetsEmptyFacets).hasSameSizeAs(datasetsNullFacets);

    // Test with jobs
    List<SimpleJob> jobsEmptyFacets =
        fullSearchDao.searchJobs(
            "", SimpleSearchSort.NAME, LIMIT, 0, null, true, java.util.Collections.emptyList());
    List<SimpleJob> jobsNullFacets =
        fullSearchDao.searchJobs("", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);

    // Both should return the same results when includeFacets is true
    assertThat(jobsEmptyFacets).hasSameSizeAs(jobsNullFacets);
  }

  @Test
  void testLargeOffset() {
    // Test with offset larger than result set
    List<SimpleDataset> allDatasets =
        fullSearchDao.searchDatasets(
            "analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    int totalCount = allDatasets.size();

    if (totalCount > 0) {
      List<SimpleDataset> offsetDatasets =
          fullSearchDao.searchDatasets(
              "analytics", SimpleSearchSort.NAME, LIMIT, totalCount + 10, null, true, null);
      assertThat(offsetDatasets).isEmpty();
    }

    // Test with jobs
    List<SimpleJob> allJobs =
        fullSearchDao.searchJobs("analytics", SimpleSearchSort.NAME, LIMIT, 0, null, true, null);
    int totalJobCount = allJobs.size();

    if (totalJobCount > 0) {
      List<SimpleJob> offsetJobs =
          fullSearchDao.searchJobs(
              "analytics", SimpleSearchSort.NAME, LIMIT, totalJobCount + 10, null, true, null);
      assertThat(offsetJobs).isEmpty();
    }
  }

  @Test
  void testCombinedParameters() {
    // Test combining offset, includeFacets, and facetNames for datasets
    List<SimpleDataset> combinedResults =
        fullSearchDao.searchDatasets(
            "analytics",
            SimpleSearchSort.NAME,
            5,
            1,
            null,
            true,
            java.util.Collections.singletonList("schema"));

    if (!combinedResults.isEmpty()) {
      assertThat(combinedResults).hasSizeLessThanOrEqualTo(5);
      assertThat(combinedResults)
          .allMatch(dataset -> dataset.getName().getValue().toLowerCase().contains("analytics"));
    }

    // Test combining offset, includeFacets, and facetNames for jobs
    List<SimpleJob> combinedJobResults =
        fullSearchDao.searchJobs(
            "analytics",
            SimpleSearchSort.NAME,
            5,
            1,
            null,
            true,
            java.util.Collections.singletonList("documentation"));

    if (!combinedJobResults.isEmpty()) {
      assertThat(combinedJobResults).hasSizeLessThanOrEqualTo(5);
      assertThat(combinedJobResults)
          .allMatch(job -> job.getName().getValue().toLowerCase().contains("analytics"));
    }
  }

  @Test
  void testFacetsWithNamespaceFiltering() {
    // Test facets with namespace filtering for datasets
    List<SimpleDataset> namespacedDatasets =
        fullSearchDao.searchDatasets(
            "",
            SimpleSearchSort.NAME,
            LIMIT,
            0,
            "namespace1",
            true,
            java.util.Collections.singletonList("dataSource"));

    for (SimpleDataset dataset : namespacedDatasets) {
      assertThat(dataset.getNamespace().getValue()).isEqualTo("namespace1");
    }

    // Test facets with namespace filtering for jobs
    List<SimpleJob> namespacedJobs =
        fullSearchDao.searchJobs(
            "",
            SimpleSearchSort.NAME,
            LIMIT,
            0,
            "namespace1",
            true,
            java.util.Collections.singletonList("documentation"));

    for (SimpleJob job : namespacedJobs) {
      assertThat(job.getNamespace().getValue()).isEqualTo("namespace1");
    }
  }
}
