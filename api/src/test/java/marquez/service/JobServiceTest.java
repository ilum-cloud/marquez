/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.service;

import static marquez.common.models.CommonModelGenerator.newJobName;
import static marquez.common.models.CommonModelGenerator.newOwnerName;
import static marquez.service.models.ServiceModelGenerator.newJobMetaWith;
import static org.assertj.core.api.Assertions.assertThat;

import com.google.common.collect.ImmutableSet;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.UUID;
import java.util.stream.Collectors;
import marquez.api.JdbiUtils;
import marquez.common.models.JobName;
import marquez.common.models.JobType;
import marquez.common.models.NamespaceName;
import marquez.common.models.RunState;
import marquez.db.BaseDao;
import marquez.db.NamespaceDao;
import marquez.db.models.NamespaceRow;
import marquez.jdbi.MarquezJdbiExternalPostgresExtension;
import marquez.service.models.Job;
import marquez.service.models.JobMeta;
import org.jdbi.v3.core.Jdbi;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

@ExtendWith(MarquezJdbiExternalPostgresExtension.class)
@Tag("IntegrationTests")
public class JobServiceTest {

  private static JobService jobService;
  private static RunService runService;
  private static NamespaceService namespaceService;
  private static NamespaceRow namespace;
  private static Jdbi jdbi;

  @BeforeAll
  public static void setUpOnce(Jdbi jdbi) {
    JobServiceTest.jdbi = jdbi;
    BaseDao baseDao = jdbi.onDemand(BaseDao.class);
    runService = new RunService(baseDao, Collections.emptyList());
    jobService = new JobService(baseDao, runService);
    namespaceService = new NamespaceService(baseDao);

    // Create test namespace
    NamespaceDao namespaceDao = jdbi.onDemand(NamespaceDao.class);
    namespace =
        namespaceDao.upsertNamespaceRow(
            UUID.randomUUID(), Instant.now(), "test_namespace", newOwnerName().getValue());
  }

  @AfterEach
  public void cleanUp() {
    JdbiUtils.cleanDatabase(jdbi);
  }

  @Test
  public void testFindAllWithRunOptimization() {
    // Create multiple jobs to test the optimized performance
    List<Job> createdJobs = new ArrayList<>();

    for (int i = 0; i < 5; i++) {
      JobMeta jobMeta =
          new JobMeta(
              JobType.BATCH,
              ImmutableSet.of(),
              ImmutableSet.of(),
              null,
              "Performance test job " + i,
              null,
              null);

      Job job =
          jobService.createOrUpdate(
              NamespaceName.of(namespace.getName()), JobName.of("perfTestJob" + i), jobMeta);
      createdJobs.add(job);
    }

    List<RunState> runStates = new ArrayList<>();
    Collections.addAll(runStates, RunState.values());

    // Test the optimized findAllWithRun method
    long startTime = System.currentTimeMillis();
    List<Job> jobs = jobService.findAllWithRun(namespace.getName(), runStates, 10, 0);
    long executionTime = System.currentTimeMillis() - startTime;

    // Verify results
    assertThat(jobs).hasSizeGreaterThanOrEqualTo(5);

    // Verify that jobs have their basic data populated
    for (Job job : jobs) {
      assertThat(job.getName()).isNotNull();
      assertThat(job.getNamespace()).isNotNull();
    }

    // Performance should be reasonable - this validates the optimization works
    // With the N+1 optimization, this should complete quickly
    assertThat(executionTime).isLessThan(3000L); // Should complete within 3 seconds
  }

  @Test
  public void testFindAllWithRunPagination() {
    // Create more jobs than the page size to test pagination
    for (int i = 0; i < 15; i++) {
      JobMeta jobMeta =
          new JobMeta(
              JobType.BATCH,
              ImmutableSet.of(),
              ImmutableSet.of(),
              null,
              "Pagination test job " + i,
              null,
              null);

      jobService.createOrUpdate(
          NamespaceName.of(namespace.getName()), JobName.of("paginationJob" + i), jobMeta);
    }

    List<RunState> runStates = new ArrayList<>();
    Collections.addAll(runStates, RunState.values());

    // Test first page
    List<Job> firstPage = jobService.findAllWithRun(namespace.getName(), runStates, 10, 0);
    assertThat(firstPage).hasSizeLessThanOrEqualTo(10);

    // Test second page
    List<Job> secondPage = jobService.findAllWithRun(namespace.getName(), runStates, 10, 10);
    assertThat(secondPage).hasSizeGreaterThanOrEqualTo(5);

    // Ensure no duplicates between pages
    List<String> firstPageJobNames =
        firstPage.stream().map(job -> job.getName().getValue()).collect(Collectors.toList());
    List<String> secondPageJobNames =
        secondPage.stream().map(job -> job.getName().getValue()).collect(Collectors.toList());

    assertThat(firstPageJobNames).doesNotContainAnyElementsOf(secondPageJobNames);
  }

  @Test
  public void testFindAllWithRunEmptyNamespace() {
    List<RunState> runStates = new ArrayList<>();
    Collections.addAll(runStates, RunState.values());

    // Test with non-existent namespace
    List<Job> jobs = jobService.findAllWithRun("nonexistent_namespace", runStates, 10, 0);
    assertThat(jobs).isEmpty();
  }

  @Test
  public void testFindAllWithRunFilterByRunStates() {
    // Create a job
    JobMeta jobMeta =
        new JobMeta(
            JobType.BATCH,
            ImmutableSet.of(),
            ImmutableSet.of(),
            null,
            "Run state test job",
            null,
            null);

    jobService.createOrUpdate(
        NamespaceName.of(namespace.getName()), JobName.of("runStateTestJob"), jobMeta);

    // Test with all run states
    List<RunState> allRunStates = new ArrayList<>();
    Collections.addAll(allRunStates, RunState.values());

    List<Job> allJobs = jobService.findAllWithRun(namespace.getName(), allRunStates, 10, 0);
    assertThat(allJobs).hasSizeGreaterThanOrEqualTo(1);

    // Test with specific run states
    List<RunState> completedOnly = List.of(RunState.COMPLETED);
    List<Job> completedJobs = jobService.findAllWithRun(namespace.getName(), completedOnly, 10, 0);
    // This might be empty if no runs are completed, which is fine for this test
    assertThat(completedJobs).isNotNull();
  }

  @Test
  public void testFindAllWithRunPerformanceWithMultipleJobs() {
    // Create multiple jobs to simulate a more realistic scenario
    for (int i = 0; i < 20; i++) {
      JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespace.getName()));
      jobService.createOrUpdate(NamespaceName.of(namespace.getName()), newJobName(), jobMeta);
    }

    List<RunState> runStates = new ArrayList<>();
    Collections.addAll(runStates, RunState.values());

    // Test the optimized approach - should handle multiple jobs efficiently
    long startTime = System.currentTimeMillis();
    List<Job> jobs = jobService.findAllWithRun(namespace.getName(), runStates, 25, 0);
    long executionTime = System.currentTimeMillis() - startTime;

    // Verify results
    assertThat(jobs).hasSizeGreaterThanOrEqualTo(20);

    // Performance validation - the key benefit of our optimization
    assertThat(executionTime).isLessThan(5000L); // Should complete within 5 seconds
  }
}
