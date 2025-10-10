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
class FullSearchResourceSimpleTest {

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

    // Create test datasets - simplified without facets and lineage
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
            null, // fields
            null); // facets

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
            null, // fields
            null); // facets

    // Create test jobs - simplified without facets and inputs/outputs
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
            null); // facets

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
            null); // facets
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
            TEST_QUERY, SimpleSearchSort.UPDATED_AT, DEFAULT_LIMIT, 0, TEST_NAMESPACE, true, null))
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
            TEST_QUERY, SimpleSearchSort.UPDATED_AT, DEFAULT_LIMIT, 0, TEST_NAMESPACE, true, null))
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
  void testOffsetPagination() {
    // Setup
    when(fullSearchDao.countDatasets(TEST_QUERY, null)).thenReturn(50);
    when(fullSearchDao.countJobs(TEST_QUERY, null)).thenReturn(30);
    when(fullSearchDao.searchDatasets(
            TEST_QUERY, SimpleSearchSort.UPDATED_AT, 10, 3, null, true, null))
        .thenReturn(List.of(testDataset1));
    when(fullSearchDao.searchJobs(TEST_QUERY, SimpleSearchSort.UPDATED_AT, 10, 2, null, true, null))
        .thenReturn(List.of(testJob1));

    // Execute with offset of 5
    Response response =
        fullSearchResource.fullSearch(
            TEST_QUERY, null, SimpleSearchSort.UPDATED_AT, 20, 5, null, true, null);

    // Verify offset and limit are passed correctly to both queries
    verify(fullSearchDao)
        .searchDatasets(TEST_QUERY, SimpleSearchSort.UPDATED_AT, 10, 3, null, true, null);
    verify(fullSearchDao)
        .searchJobs(TEST_QUERY, SimpleSearchSort.UPDATED_AT, 10, 2, null, true, null);

    assertThat(response.getStatus()).isEqualTo(200);
    FullSearchResults results = (FullSearchResults) response.getEntity();
    assertThat(results.getDatasets()).containsExactly(testDataset1);
    assertThat(results.getJobs()).containsExactly(testJob1);
    assertThat(results.getTotalCount()).isEqualTo(80); // 50 datasets + 30 jobs
  }

  @Test
  void testOffsetWithDatasetFilter() {
    // Setup
    List<SimpleDataset> mockDatasets = Arrays.asList(testDataset1, testDataset2);

    when(fullSearchDao.countDatasets(TEST_QUERY, TEST_NAMESPACE)).thenReturn(25);
    when(fullSearchDao.searchDatasets(
            TEST_QUERY, SimpleSearchSort.UPDATED_AT, DEFAULT_LIMIT, 10, TEST_NAMESPACE, true, null))
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
    assertThat(results.getTotalCount()).isEqualTo(25); // Only dataset count
    assertThat(results.getDatasets()).hasSize(2);
    assertThat(results.getJobs()).isEmpty();

    // Verify offset is passed correctly and no job queries were made
    verify(fullSearchDao)
        .searchDatasets(
            TEST_QUERY, SimpleSearchSort.UPDATED_AT, DEFAULT_LIMIT, 10, TEST_NAMESPACE, true, null);
    verify(fullSearchDao, times(0)).countJobs(anyString(), anyString());
    verify(fullSearchDao, times(0))
        .searchJobs(anyString(), any(), anyInt(), anyInt(), anyString(), anyBoolean(), anyList());
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
  void testEmptyResults() {
    // Setup
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
}
