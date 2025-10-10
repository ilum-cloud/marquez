/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.api;

import static org.assertj.core.api.Assertions.assertThat;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.ArgumentMatchers.anyBoolean;
import static org.mockito.ArgumentMatchers.anyInt;
import static org.mockito.ArgumentMatchers.anyList;
import static org.mockito.ArgumentMatchers.anyString;
import static org.mockito.ArgumentMatchers.eq;
import static org.mockito.ArgumentMatchers.isNull;
import static org.mockito.Mockito.times;
import static org.mockito.Mockito.verify;
import static org.mockito.Mockito.when;

import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import com.google.common.collect.ImmutableSet;
import jakarta.ws.rs.core.Response;
import java.time.Instant;
import java.util.Arrays;
import java.util.List;
import java.util.UUID;
import marquez.api.FullSearchResource.FullSearchResults;
import marquez.api.models.SearchFilter;
import marquez.api.models.SimpleDataset;
import marquez.api.models.SimpleJob;
import marquez.api.models.SimpleSearchSort;
import marquez.common.models.DatasetId;
import marquez.common.models.DatasetName;
import marquez.common.models.DatasetType;
import marquez.common.models.Field;
import marquez.common.models.FieldName;
import marquez.common.models.JobId;
import marquez.common.models.JobName;
import marquez.common.models.JobType;
import marquez.common.models.NamespaceName;
import marquez.common.models.SourceName;
import marquez.db.FullSearchDao;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;

@ExtendWith(MockitoExtension.class)
class FullSearchResourceTest {

  @Mock private FullSearchDao fullSearchDao;

  private FullSearchResource fullSearchResource;

  private static final String TEST_QUERY = "analytics";
  private static final String TEST_NAMESPACE = "test_namespace";
  private static final int DEFAULT_LIMIT = 20;

  // Sample test data
  private SimpleDataset testDataset1;
  private SimpleDataset testDataset2;
  private SimpleJob testJob1;
  private SimpleJob testJob2;

  @BeforeEach
  void setUp() {
    fullSearchResource = new FullSearchResource(fullSearchDao);
    setupTestData();
  }

  private void setupTestData() {
    Instant now = Instant.now();
    NamespaceName namespace = NamespaceName.of(TEST_NAMESPACE);

    // Create sample fields for datasets
    List<Field> fields =
        List.of(
            new Field(FieldName.of("id"), "INTEGER", ImmutableSet.of(), "Primary key"),
            new Field(FieldName.of("name"), "VARCHAR", ImmutableSet.of(), "Name field"));

    // Create sample facets
    ImmutableMap<String, Object> facets =
        ImmutableMap.of(
            "schema", ImmutableMap.of("fields", Arrays.asList("id", "name")),
            "dataQuality", ImmutableMap.of("score", 0.95));

    // Create test datasets - now with fields and facets
    testDataset1 =
        new SimpleDataset(
            new DatasetId(namespace, DatasetName.of("analytics_data")),
            DatasetType.DB_TABLE,
            DatasetName.of("analytics_data"),
            DatasetName.of("public.analytics_data"),
            now,
            now,
            SourceName.of("test-source"),
            ImmutableSet.of(),
            null,
            null,
            "Test analytics dataset",
            UUID.randomUUID(),
            false,
            ImmutableList.copyOf(fields),
            facets);

    testDataset2 =
        new SimpleDataset(
            new DatasetId(namespace, DatasetName.of("user_analytics")),
            DatasetType.DB_TABLE,
            DatasetName.of("user_analytics"),
            DatasetName.of("public.user_analytics"),
            now,
            now,
            SourceName.of("test-source"),
            ImmutableSet.of(),
            null,
            null,
            "Test user analytics dataset",
            UUID.randomUUID(),
            false,
            ImmutableList.copyOf(fields),
            facets);

    // Create sample job facets
    ImmutableMap<String, Object> jobFacets =
        ImmutableMap.of(
            "spark", ImmutableMap.of("version", "3.1.0"),
            "ownership", ImmutableMap.of("team", "data-eng"));

    // Create test jobs - now with facets
    testJob1 =
        new SimpleJob(
            new JobId(namespace, JobName.of("analytics_job")),
            JobType.BATCH,
            JobName.of("analytics_job"),
            "analytics_job",
            null,
            null,
            now,
            now,
            null,
            "Test analytics job",
            UUID.randomUUID(),
            null,
            ImmutableSet.of(),
            jobFacets);

    testJob2 =
        new SimpleJob(
            new JobId(namespace, JobName.of("etl_analytics")),
            JobType.BATCH,
            JobName.of("etl_analytics"),
            "etl_analytics",
            null,
            null,
            now,
            now,
            null,
            "Test ETL analytics job",
            UUID.randomUUID(),
            null,
            ImmutableSet.of(),
            jobFacets);
  }

  @Test
  void testFullSearch_withoutFilter() {
    // Setup
    List<SimpleDataset> mockDatasets = Arrays.asList(testDataset1, testDataset2);
    List<SimpleJob> mockJobs = Arrays.asList(testJob1, testJob2);

    when(fullSearchDao.countDatasets(TEST_QUERY, TEST_NAMESPACE)).thenReturn(5);
    when(fullSearchDao.countJobs(TEST_QUERY, TEST_NAMESPACE)).thenReturn(3);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(true),
            isNull()))
        .thenReturn(mockDatasets);
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(true),
            isNull()))
        .thenReturn(mockJobs);

    // Execute
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY,
            null,
            SimpleSearchSort.UPDATED_AT,
            DEFAULT_LIMIT,
            0,
            TEST_NAMESPACE,
            true,
            null);

    // Verify
    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getTotalCount()).isEqualTo(8); // 5 datasets + 3 jobs
    assertThat(results.getDatasets()).hasSize(2);
    assertThat(results.getJobs()).hasSize(2);
    assertThat(results.getDatasets()).containsExactly(testDataset1, testDataset2);
    assertThat(results.getJobs()).containsExactly(testJob1, testJob2);
  }

  @Test
  void testFullSearch_withDatasetFilter() {
    // Setup
    List<SimpleDataset> mockDatasets = Arrays.asList(testDataset1, testDataset2);

    when(fullSearchDao.countDatasets(TEST_QUERY, TEST_NAMESPACE)).thenReturn(5);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            eq(DEFAULT_LIMIT),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(true),
            isNull()))
        .thenReturn(mockDatasets);

    // Execute
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY,
            SearchFilter.DATASET,
            SimpleSearchSort.UPDATED_AT,
            DEFAULT_LIMIT,
            0,
            TEST_NAMESPACE,
            true,
            null);

    // Verify
    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getTotalCount()).isEqualTo(5); // Only dataset count
    assertThat(results.getDatasets()).hasSize(2);
    assertThat(results.getJobs()).isEmpty();

    // Verify no job queries were made
    verify(fullSearchDao, times(0)).countJobs(anyString(), anyString());
    verify(fullSearchDao, times(0))
        .searchJobs(anyString(), any(), anyInt(), anyInt(), anyString(), anyBoolean(), anyList());
  }

  @Test
  void testFullSearch_withJobFilter() {
    // Setup
    List<SimpleJob> mockJobs = Arrays.asList(testJob1, testJob2);

    when(fullSearchDao.countJobs(TEST_QUERY, TEST_NAMESPACE)).thenReturn(3);
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            eq(DEFAULT_LIMIT),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(true),
            isNull()))
        .thenReturn(mockJobs);

    // Execute
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY,
            SearchFilter.JOB,
            SimpleSearchSort.UPDATED_AT,
            DEFAULT_LIMIT,
            0,
            TEST_NAMESPACE,
            true,
            null);

    // Verify
    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getTotalCount()).isEqualTo(3); // Only job count
    assertThat(results.getDatasets()).isEmpty();
    assertThat(results.getJobs()).hasSize(2);

    // Verify no dataset queries were made
    verify(fullSearchDao, times(0)).countDatasets(anyString(), anyString());
    verify(fullSearchDao, times(0))
        .searchDatasets(
            anyString(), any(), anyInt(), anyInt(), anyString(), anyBoolean(), anyList());
  }

  @Test
  void testFullSearch_withFacetsDisabled() {
    // Setup
    List<SimpleDataset> mockDatasets = Arrays.asList(testDataset1, testDataset2);
    List<SimpleJob> mockJobs = Arrays.asList(testJob1, testJob2);

    when(fullSearchDao.countDatasets(TEST_QUERY, TEST_NAMESPACE)).thenReturn(5);
    when(fullSearchDao.countJobs(TEST_QUERY, TEST_NAMESPACE)).thenReturn(3);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(false),
            isNull()))
        .thenReturn(mockDatasets);
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(false),
            isNull()))
        .thenReturn(mockJobs);

    // Execute with facets disabled
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY,
            null,
            SimpleSearchSort.UPDATED_AT,
            DEFAULT_LIMIT,
            0,
            TEST_NAMESPACE,
            false,
            null);

    // Verify
    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getTotalCount()).isEqualTo(8);
    assertThat(results.getDatasets()).hasSize(2);
    assertThat(results.getJobs()).hasSize(2);

    // Verify facets parameter was passed as false
    verify(fullSearchDao)
        .searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(false),
            isNull());
    verify(fullSearchDao)
        .searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(false),
            isNull());
  }

  @Test
  void testFullSearch_withSpecificFacetNames() {
    // Setup
    List<String> facetNames = Arrays.asList("schema", "dataQuality");
    List<SimpleDataset> mockDatasets = Arrays.asList(testDataset1, testDataset2);
    List<SimpleJob> mockJobs = Arrays.asList(testJob1, testJob2);

    when(fullSearchDao.countDatasets(TEST_QUERY, TEST_NAMESPACE)).thenReturn(5);
    when(fullSearchDao.countJobs(TEST_QUERY, TEST_NAMESPACE)).thenReturn(3);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(true),
            eq(facetNames)))
        .thenReturn(mockDatasets);
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(true),
            eq(facetNames)))
        .thenReturn(mockJobs);

    // Execute with specific facet names
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY,
            null,
            SimpleSearchSort.UPDATED_AT,
            DEFAULT_LIMIT,
            0,
            TEST_NAMESPACE,
            true,
            facetNames);

    // Verify
    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getTotalCount()).isEqualTo(8);
    assertThat(results.getDatasets()).hasSize(2);
    assertThat(results.getJobs()).hasSize(2);

    // Verify facet names parameter was passed correctly
    verify(fullSearchDao)
        .searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(true),
            eq(facetNames));
    verify(fullSearchDao)
        .searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            anyInt(),
            eq(0),
            eq(TEST_NAMESPACE),
            eq(true),
            eq(facetNames));
  }

  @Test
  void testLimitDistribution_equalAvailability() {
    // Setup: Both types have plenty of data
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), eq(10), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testDataset1));
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), eq(10), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testJob1));

    // Execute
    fullSearchResource.fullSearch(
        TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 0, null, true, null);

    // Verify equal distribution: 10 datasets + 10 jobs = 20 total
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), eq(10), eq(0), isNull(), eq(true), isNull());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), eq(10), eq(0), isNull(), eq(true), isNull());
  }

  @Test
  void testLimitDistribution_fewJobs() {
    // Setup: Few jobs, many datasets
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(3);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), eq(17), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(Arrays.asList(testDataset1, testDataset2));
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), eq(3), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testJob1));

    // Execute
    fullSearchResource.fullSearch(
        TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 0, null, true, null);

    // Verify redistribution: 17 datasets + 3 jobs = 20 total
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), eq(17), eq(0), isNull(), eq(true), isNull());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), eq(3), eq(0), isNull(), eq(true), isNull());
  }

  @Test
  void testLimitDistribution_fewDatasets() {
    // Setup: Few datasets, many jobs
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(5);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), eq(5), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testDataset1));
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), eq(15), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(Arrays.asList(testJob1, testJob2));

    // Execute
    fullSearchResource.fullSearch(
        TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 0, null, true, null);

    // Verify redistribution: 5 datasets + 15 jobs = 20 total
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), eq(5), eq(0), isNull(), eq(true), isNull());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), eq(15), eq(0), isNull(), eq(true), isNull());
  }

  @Test
  void testLimitDistribution_noDatasets() {
    // Setup: No datasets, many jobs
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(0);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), eq(20), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(Arrays.asList(testJob1, testJob2));

    // Execute
    fullSearchResource.fullSearch(
        TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 0, null, true, null);

    // Verify all allocation goes to jobs: 0 datasets + 20 jobs = 20 total
    verify(fullSearchDao, times(0))
        .searchDatasets(
            anyString(), any(), anyInt(), anyInt(), anyString(), anyBoolean(), anyList());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), eq(20), eq(0), isNull(), eq(true), isNull());
  }

  @Test
  void testLimitDistribution_noJobs() {
    // Setup: Many datasets, no jobs
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(0);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), eq(20), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(Arrays.asList(testDataset1, testDataset2));

    // Execute
    fullSearchResource.fullSearch(
        TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 0, null, true, null);

    // Verify all allocation goes to datasets: 20 datasets + 0 jobs = 20 total
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), eq(20), eq(0), isNull(), eq(true), isNull());
    verify(fullSearchDao, times(0))
        .searchJobs(anyString(), any(), anyInt(), anyInt(), anyString(), anyBoolean(), anyList());
  }

  @Test
  void testLimitDistribution_oddLimit() {
    // Setup: Test odd limit distribution
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), eq(11), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testDataset1));
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), eq(10), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testJob1));

    // Execute with limit 21
    fullSearchResource.fullSearch(
        TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 21, 0, null, true, null);

    // Verify distribution: 11 datasets + 10 jobs = 21 total (remainder goes to datasets)
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), eq(11), eq(0), isNull(), eq(true), isNull());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), eq(10), eq(0), isNull(), eq(true), isNull());
  }

  @Test
  void testLimitDistribution_limitedBothTypes() {
    // Setup: Both types have limited data
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(8);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(7);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), eq(8), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testDataset1));
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), eq(7), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testJob1));

    // Execute with limit 20 (more than available)
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 0, null, true, null);

    // Verify we only get what's available: 8 datasets + 7 jobs = 15 total
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), eq(8), eq(0), isNull(), eq(true), isNull());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), eq(7), eq(0), isNull(), eq(true), isNull());

    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getTotalCount()).isEqualTo(15); // 8 + 7
  }

  @Test
  void testMaxLimitEnforcement() {
    // Setup
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(1000);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(1000);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), eq(50), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testDataset1));
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), eq(50), eq(0), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testJob1));

    // Execute with limit > MAX_LIMIT
    fullSearchResource.fullSearch(
        TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 150, 0, null, true, null);

    // Verify max limit is enforced: 50 datasets + 50 jobs = 100 total (MAX_LIMIT)
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), eq(50), eq(0), isNull(), eq(true), isNull());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), eq(50), eq(0), isNull(), eq(true), isNull());
  }

  @Test
  void testSortingParameter() {
    // Setup
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(10);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(10);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.NAME),
            anyInt(),
            eq(0),
            isNull(),
            eq(true),
            isNull()))
        .thenReturn(List.of(testDataset1));
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.NAME),
            anyInt(),
            eq(0),
            isNull(),
            eq(true),
            isNull()))
        .thenReturn(List.of(testJob1));

    // Execute with NAME sort
    fullSearchResource.fullSearch(TEST_QUERY, null, SimpleSearchSort.NAME, 20, 0, null, true, null);

    // Verify sort parameter is passed correctly
    verify(fullSearchDao)
        .searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.NAME),
            anyInt(),
            eq(0),
            isNull(),
            eq(true),
            isNull());
    verify(fullSearchDao)
        .searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.NAME),
            anyInt(),
            eq(0),
            isNull(),
            eq(true),
            isNull());
  }

  @Test
  void testEmptyResults() {
    // Setup - no results found
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(0);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(0);

    // Execute
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 0, null, true, null);

    // Verify
    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getTotalCount()).isZero();
    assertThat(results.getDatasets()).isEmpty();
    assertThat(results.getJobs()).isEmpty();

    // Verify no search queries were made since counts were 0
    verify(fullSearchDao, times(0))
        .searchDatasets(
            anyString(), any(), anyInt(), anyInt(), anyString(), anyBoolean(), anyList());
    verify(fullSearchDao, times(0))
        .searchJobs(anyString(), any(), anyInt(), anyInt(), anyString(), anyBoolean(), anyList());
  }

  @Test
  void testPagination_withOffset() {
    // Setup for testing pagination
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(100);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), eq(10), eq(5), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testDataset1));
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), eq(10), eq(5), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testJob1));

    // Execute with offset
    fullSearchResource.fullSearch(
        TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 10, null, true, null);

    // Verify offset is correctly distributed to both types
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), eq(10), eq(5), isNull(), eq(true), isNull());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), eq(10), eq(5), isNull(), eq(true), isNull());
  }

  @Test
  void testPagination_withLargeOffset() {
    // Setup for testing pagination with large offset that affects distribution
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(50);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(30);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), eq(10), eq(20), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testDataset1));
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), eq(10), eq(20), isNull(), eq(true), isNull()))
        .thenReturn(List.of(testJob1));

    // Execute with offset 40 (split as 20 for datasets, 20 for jobs)
    fullSearchResource.fullSearch(
        TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 40, null, true, null);

    // Verify offset is correctly distributed to both types
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), eq(10), eq(20), isNull(), eq(true), isNull());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), eq(10), eq(20), isNull(), eq(true), isNull());
  }

  @Test
  void testPagination_offsetWithDatasetFilter() {
    // Setup for testing offset with dataset filter
    List<SimpleDataset> mockDatasets = Arrays.asList(testDataset1, testDataset2);
    when(fullSearchDao.countDatasets(TEST_QUERY, TEST_NAMESPACE)).thenReturn(50);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            eq(DEFAULT_LIMIT),
            eq(10),
            eq(TEST_NAMESPACE),
            eq(true),
            isNull()))
        .thenReturn(mockDatasets);

    // Execute with dataset filter and offset
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY,
            SearchFilter.DATASET,
            SimpleSearchSort.UPDATED_AT,
            DEFAULT_LIMIT,
            10,
            TEST_NAMESPACE,
            true,
            null);

    // Verify
    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getDatasets()).hasSize(2);
    assertThat(results.getJobs()).isEmpty();

    // Verify offset is passed directly to dataset search when filtered
    verify(fullSearchDao)
        .searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            eq(DEFAULT_LIMIT),
            eq(10),
            eq(TEST_NAMESPACE),
            eq(true),
            isNull());
  }

  @Test
  void testPagination_offsetWithJobFilter() {
    // Setup for testing offset with job filter
    List<SimpleJob> mockJobs = Arrays.asList(testJob1, testJob2);
    when(fullSearchDao.countJobs(TEST_QUERY, TEST_NAMESPACE)).thenReturn(30);
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            eq(DEFAULT_LIMIT),
            eq(15),
            eq(TEST_NAMESPACE),
            eq(true),
            isNull()))
        .thenReturn(mockJobs);

    // Execute with job filter and offset
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY,
            SearchFilter.JOB,
            SimpleSearchSort.UPDATED_AT,
            DEFAULT_LIMIT,
            15,
            TEST_NAMESPACE,
            true,
            null);

    // Verify
    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getDatasets()).isEmpty();
    assertThat(results.getJobs()).hasSize(2);

    // Verify offset is passed directly to job search when filtered
    verify(fullSearchDao)
        .searchJobs(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.UPDATED_AT),
            eq(DEFAULT_LIMIT),
            eq(15),
            eq(TEST_NAMESPACE),
            eq(true),
            isNull());
  }

  @Test
  void testFacets_disabledWithSpecificFacetNames() {
    // Setup - test that facetNames is ignored (passed as null) when facets is false
    List<String> facetNames = Arrays.asList("schema", "ownership");
    List<SimpleDataset> mockDatasets = Arrays.asList(testDataset1);
    List<SimpleJob> mockJobs = Arrays.asList(testJob1);

    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(10);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(10);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), anyInt(), eq(0), isNull(), eq(false), isNull()))
        .thenReturn(mockDatasets);
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), anyInt(), eq(0), isNull(), eq(false), isNull()))
        .thenReturn(mockJobs);

    // Execute with facets=false but facetNames provided
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 0, null, false, facetNames);

    // Verify facets are disabled and facetNames is ignored (passed as null to DAO)
    verify(fullSearchDao)
        .searchDatasets(eq(TEST_QUERY), any(), anyInt(), eq(0), isNull(), eq(false), isNull());
    verify(fullSearchDao)
        .searchJobs(eq(TEST_QUERY), any(), anyInt(), eq(0), isNull(), eq(false), isNull());

    assertThat(response.getStatus()).isEqualTo(200);
  }

  @Test
  void testFacetNames_emptyList() {
    // Setup - test behavior with empty facet names list
    List<String> emptyFacetNames = List.of();
    List<SimpleDataset> mockDatasets = Arrays.asList(testDataset1);
    List<SimpleJob> mockJobs = Arrays.asList(testJob1);

    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(10);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(10);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY), any(), anyInt(), eq(0), isNull(), eq(true), eq(emptyFacetNames)))
        .thenReturn(mockDatasets);
    when(fullSearchDao.searchJobs(
            eq(TEST_QUERY), any(), anyInt(), eq(0), isNull(), eq(true), eq(emptyFacetNames)))
        .thenReturn(mockJobs);

    // Execute with empty facet names list
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 0, null, true, emptyFacetNames);

    // Verify empty facet names list is passed through
    verify(fullSearchDao)
        .searchDatasets(
            eq(TEST_QUERY), any(), anyInt(), eq(0), isNull(), eq(true), eq(emptyFacetNames));
    verify(fullSearchDao)
        .searchJobs(
            eq(TEST_QUERY), any(), anyInt(), eq(0), isNull(), eq(true), eq(emptyFacetNames));

    assertThat(response.getStatus()).isEqualTo(200);
  }

  @Test
  void testPaginationOffset_calculateOptimalLimitsEdgeCases() {
    // Test edge cases in pagination offset calculation

    // Case 1: Offset larger than total available data
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(5);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(3);

    // With offset 10 and total available data 8, should return empty results
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 10, null, true, null);

    // Should still return successful response with empty results
    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getTotalCount()).isEqualTo(8); // 5 datasets + 3 jobs
  }

  @Test
  void testComplexScenario_allParametersTogether() {
    // Test a complex scenario with all parameters
    List<String> facetNames = Arrays.asList("schema", "dataQuality", "ownership");
    List<SimpleDataset> mockDatasets = Arrays.asList(testDataset1);

    when(fullSearchDao.countDatasets(TEST_QUERY, TEST_NAMESPACE)).thenReturn(100);
    when(fullSearchDao.searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.NAME),
            eq(15),
            eq(25),
            eq(TEST_NAMESPACE),
            eq(true),
            eq(facetNames)))
        .thenReturn(mockDatasets);

    // Execute with all parameters: query, filter, sort, limit, offset, namespace, facets,
    // facetNames
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY,
            SearchFilter.DATASET,
            SimpleSearchSort.NAME,
            15,
            25,
            TEST_NAMESPACE,
            true,
            facetNames);

    // Verify all parameters are correctly passed
    verify(fullSearchDao)
        .searchDatasets(
            eq(TEST_QUERY),
            eq(SimpleSearchSort.NAME),
            eq(15),
            eq(25),
            eq(TEST_NAMESPACE),
            eq(true),
            eq(facetNames));

    // Verify no job search is performed due to dataset filter
    verify(fullSearchDao, times(0))
        .searchJobs(anyString(), any(), anyInt(), anyInt(), anyString(), anyBoolean(), anyList());
    verify(fullSearchDao, times(0)).countJobs(anyString(), anyString());

    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getDatasets()).hasSize(1);
    assertThat(results.getJobs()).isEmpty();
    assertThat(results.getTotalCount()).isEqualTo(100);
  }
}
