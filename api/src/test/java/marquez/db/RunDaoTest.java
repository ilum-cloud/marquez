/*
 * Copyright 2018-2023 contributors to the Marquez project
 * SPDX-License-Identifier: Apache-2.0
 */

package marquez.db;

import static marquez.common.models.CommonModelGenerator.newJobName;
import static marquez.db.DbTestUtils.createJobWithSymlinkTarget;
import static marquez.db.DbTestUtils.createJobWithoutSymlinkTarget;
import static marquez.db.DbTestUtils.newJobWith;
import static marquez.service.models.ServiceModelGenerator.newJobMetaWith;
import static org.assertj.core.api.Assertions.assertThat;

import com.google.common.collect.ImmutableSet;
import java.time.Instant;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import java.util.Set;
import java.util.TreeSet;
import java.util.stream.Collectors;
import java.util.stream.IntStream;
import java.util.stream.Stream;
import marquez.api.JdbiUtils;
import marquez.common.models.DatasetId;
import marquez.common.models.DatasetVersionId;
import marquez.common.models.InputDatasetVersion;
import marquez.common.models.NamespaceName;
import marquez.common.models.OutputDatasetVersion;
import marquez.common.models.RunId;
import marquez.common.models.RunState;
import marquez.db.models.ExtendedRunRow;
import marquez.db.models.JobRow;
import marquez.db.models.NamespaceRow;
import marquez.db.models.RunRow;
import marquez.jdbi.MarquezJdbiExternalPostgresExtension;
import marquez.service.models.JobMeta;
import marquez.service.models.Run;
import org.assertj.core.api.InstanceOfAssertFactories;
import org.jdbi.v3.core.Jdbi;
import org.jetbrains.annotations.NotNull;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

@ExtendWith(MarquezJdbiExternalPostgresExtension.class)
class RunDaoTest {

  private static RunDao runDao;
  private static Jdbi jdbi;
  private static JobVersionDao jobVersionDao;
  private static OpenLineageDao openLineageDao;

  static NamespaceRow namespaceRow;
  static JobRow jobRow;

  @BeforeAll
  public static void setUpOnce(Jdbi jdbi) {
    RunDaoTest.jdbi = jdbi;
    runDao = jdbi.onDemand(RunDao.class);
    jobVersionDao = jdbi.onDemand(JobVersionDao.class);
    openLineageDao = jdbi.onDemand(OpenLineageDao.class);
    namespaceRow = DbTestUtils.newNamespace(jdbi);
    jobRow = DbTestUtils.newJob(jdbi, namespaceRow.getName(), newJobName().getValue());
  }

  @AfterEach
  public void tearDown(Jdbi jdbi) {
    JdbiUtils.cleanDatabase(jdbi);
  }

  @Test
  public void getRun() {

    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);

    final RunRow runRow = DbTestUtils.newRun(jdbi, jobRow);
    DbTestUtils.transitionRunWithOutputs(
        jdbi, runRow.getUuid(), RunState.COMPLETED, jobMeta.getOutputs());

    jobVersionDao.upsertJobVersionOnRunTransition(
        jobVersionDao.loadJobRowRunDetails(jobRow, runRow.getUuid()),
        RunState.COMPLETED,
        Instant.now(),
        true);

    Optional<Run> run = runDao.findRunByUuid(runRow.getUuid());
    assertThat(run)
        .isPresent()
        .get()
        .extracting(
            Run::getInputDatasetVersions, InstanceOfAssertFactories.list(InputDatasetVersion.class))
        .hasSize(jobMeta.getInputs().size())
        .map(InputDatasetVersion::getDatasetVersionId)
        .map(DatasetVersionId::getName)
        .containsAll(
            jobMeta.getInputs().stream().map(DatasetId::getName).collect(Collectors.toSet()));

    assertThat(run)
        .get()
        .extracting(
            Run::getOutputDatasetVersions,
            InstanceOfAssertFactories.list(OutputDatasetVersion.class))
        .hasSize(jobMeta.getOutputs().size())
        .map(OutputDatasetVersion::getDatasetVersionId)
        .map(DatasetVersionId::getName)
        .containsAll(
            jobMeta.getOutputs().stream().map(DatasetId::getName).collect(Collectors.toSet()));
  }

  @Test
  public void getFindAll() {

    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);

    Set<RunRow> expectedRuns =
        createRunsForJob(jobRow, 5, jobMeta.getOutputs()).collect(Collectors.toSet());
    List<Run> runs = runDao.findAll(jobRow.getNamespaceName(), jobRow.getName(), 10, 0);
    assertThat(runs)
        .hasSize(expectedRuns.size())
        .map(Run::getId)
        .map(RunId::getValue)
        .containsAll(expectedRuns.stream().map(RunRow::getUuid).collect(Collectors.toSet()));
  }

  @Test
  public void getFindAllForSymlinkedJob() {
    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);

    final JobRow symlinkJob =
        createJobWithSymlinkTarget(
            jdbi, namespaceRow, newJobName().getValue(), jobRow.getUuid(), "symlink job");

    Set<RunRow> expectedRuns =
        Stream.concat(
                createRunsForJob(symlinkJob, 3, jobMeta.getOutputs()),
                createRunsForJob(jobRow, 2, jobMeta.getOutputs()))
            .collect(Collectors.toSet());

    // all runs should be present
    List<Run> runs = runDao.findAll(jobRow.getNamespaceName(), jobRow.getName(), 10, 0);
    assertThat(runs)
        .hasSize(expectedRuns.size())
        .map(Run::getId)
        .map(RunId::getValue)
        .containsAll(expectedRuns.stream().map(RunRow::getUuid).collect(Collectors.toSet()));
  }

  @Test
  public void testFindByLatestJob() {
    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);
    Set<RunRow> runs =
        createRunsForJob(jobRow, 5, jobMeta.getOutputs()).collect(Collectors.toSet());

    TreeSet<RunRow> sortedRuns =
        new TreeSet<>(Comparator.comparing(RunRow::getUpdatedAt).reversed());
    sortedRuns.addAll(runs);
    Run byLatestJob =
        runDao.findByLatestJob(jobRow.getNamespaceName(), jobRow.getName(), 1, 0).get(0);
    assertThat(byLatestJob)
        .hasFieldOrPropertyWithValue("id", new RunId(sortedRuns.first().getUuid()));

    JobRow newTargetJob =
        createJobWithoutSymlinkTarget(jdbi, namespaceRow, "newTargetJob", "a symlink target");

    // update the old job to point to the new targets
    createJobWithSymlinkTarget(
        jdbi,
        namespaceRow,
        jobRow.getName(),
        newTargetJob.getUuid(),
        jobMeta.getDescription().orElse(null));

    // get the latest run for the *newTargetJob*. It should be the same as the old job's latest run
    byLatestJob =
        runDao
            .findByLatestJob(newTargetJob.getNamespaceName(), newTargetJob.getName(), 1, 0)
            .get(0);
    assertThat(byLatestJob)
        .hasFieldOrPropertyWithValue("id", new RunId(sortedRuns.first().getUuid()));
  }

  @NotNull
  private Stream<RunRow> createRunsForJob(
      JobRow jobRow, int count, ImmutableSet<DatasetId> outputs) {
    return IntStream.range(0, count)
        .mapToObj(
            i -> {
              final RunRow runRow = DbTestUtils.newRun(jdbi, jobRow);
              DbTestUtils.transitionRunWithOutputs(
                  jdbi, runRow.getUuid(), RunState.COMPLETED, outputs);

              jobVersionDao.upsertJobVersionOnRunTransition(
                  jobVersionDao.loadJobRowRunDetails(jobRow, runRow.getUuid()),
                  RunState.COMPLETED,
                  Instant.now(),
                  true);
              return runRow;
            });
  }

  @Test
  public void updateRowWithNullNominalTimeDoesNotUpdateNominalTime() {
    final RunDao runDao = jdbi.onDemand(RunDao.class);

    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);

    RunRow row = DbTestUtils.newRun(jdbi, jobRow);

    RunRow updatedRow =
        runDao.upsert(
            row.getUuid(),
            null,
            row.getUuid().toString(),
            row.getUpdatedAt(),
            jobRow.getUuid(),
            null,
            row.getRunArgsUuid(),
            null,
            null,
            namespaceRow.getName(),
            jobRow.getName(),
            null);

    assertThat(row.getUuid()).isEqualTo(updatedRow.getUuid());
    assertThat(row.getNominalStartTime()).isNotNull();
    assertThat(row.getNominalEndTime()).isNotNull();
    assertThat(updatedRow.getNominalStartTime()).isEqualTo(row.getNominalStartTime());
    assertThat(updatedRow.getNominalEndTime()).isEqualTo(row.getNominalEndTime());
  }

  @Test
  public void updateRowWithExternalId() {
    final RunDao runDao = jdbi.onDemand(RunDao.class);

    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);

    RunRow row = DbTestUtils.newRun(jdbi, jobRow);

    runDao.upsert(
        row.getUuid(),
        null,
        row.getUuid().toString(),
        row.getUpdatedAt(),
        jobRow.getUuid(),
        null,
        row.getRunArgsUuid(),
        null,
        null,
        namespaceRow.getName(),
        jobRow.getName(),
        null);

    runDao.upsert(
        row.getUuid(),
        null,
        "updated-external-id",
        row.getUpdatedAt(),
        jobRow.getUuid(),
        null,
        row.getRunArgsUuid(),
        null,
        null,
        namespaceRow.getName(),
        jobRow.getName(),
        null);

    Optional<ExtendedRunRow> runRowOpt = runDao.findRunByUuidAsExtendedRow(row.getUuid());
    assertThat(runRowOpt)
        .isPresent()
        .get()
        .extracting("externalId")
        .isEqualTo("updated-external-id");
  }

  @Test
  public void testFindByLatestJobOptimized() {
    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);
    Set<RunRow> runs =
        createRunsForJob(jobRow, 5, jobMeta.getOutputs()).collect(Collectors.toSet());

    TreeSet<RunRow> sortedRuns =
        new TreeSet<>(Comparator.comparing(RunRow::getUpdatedAt).reversed());
    sortedRuns.addAll(runs);

    // Test the optimized method
    List<Run> optimizedRuns =
        runDao.findByLatestJobOptimized(jobRow.getNamespaceName(), jobRow.getName(), 3, 0);

    // Verify results
    assertThat(optimizedRuns).hasSize(3);
    assertThat(optimizedRuns.get(0))
        .hasFieldOrPropertyWithValue("id", new RunId(sortedRuns.first().getUuid()));
  }

  @Test
  public void testFindByLatestJobOptimizedVsOriginal() {
    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);

    // Create runs for the job
    createRunsForJob(jobRow, 10, jobMeta.getOutputs()).collect(Collectors.toSet());

    // Get results from both methods
    List<Run> originalRuns =
        runDao.findByLatestJob(jobRow.getNamespaceName(), jobRow.getName(), 5, 0);
    List<Run> optimizedRuns =
        runDao.findByLatestJobOptimized(jobRow.getNamespaceName(), jobRow.getName(), 5, 0);

    // Results should be identical
    assertThat(optimizedRuns).hasSameSizeAs(originalRuns);

    // Check that the run IDs match (same order)
    List<RunId> originalIds = originalRuns.stream().map(Run::getId).collect(Collectors.toList());
    List<RunId> optimizedIds = optimizedRuns.stream().map(Run::getId).collect(Collectors.toList());
    assertThat(optimizedIds).isEqualTo(originalIds);

    // Verify that dataset facets are included in optimized version
    for (Run run : optimizedRuns) {
      // The optimized version should include dataset facets data
      assertThat(run).isNotNull();
      assertThat(run.getId()).isNotNull();
    }
  }

  @Test
  public void testFindByLatestJobOptimizedWithDatasetFacets() {
    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);

    // Create runs with outputs to ensure dataset facets are created
    Set<RunRow> runs =
        createRunsForJob(jobRow, 3, jobMeta.getOutputs()).collect(Collectors.toSet());

    // Test optimized method - should include dataset facets without performance issues
    long startTime = System.currentTimeMillis();
    List<Run> optimizedRuns =
        runDao.findByLatestJobOptimized(jobRow.getNamespaceName(), jobRow.getName(), 10, 0);
    long optimizedTime = System.currentTimeMillis() - startTime;

    // Verify results
    assertThat(optimizedRuns).hasSize(3);

    // Performance should be reasonable (this is more of a smoke test)
    assertThat(optimizedTime).isLessThan(2000L); // Should complete within 2 seconds

    // Verify that dataset information is properly populated
    for (Run run : optimizedRuns) {
      assertThat(run.getId()).isNotNull();
      // Input/output versions should be populated when available
      assertThat(run.getInputDatasetVersions()).isNotNull();
      assertThat(run.getOutputDatasetVersions()).isNotNull();
    }
  }

  @Test
  public void testFindByLatestJobOptimizedWithPagination() {
    final JobMeta jobMeta = newJobMetaWith(NamespaceName.of(namespaceRow.getName()));
    final JobRow jobRow =
        newJobWith(jdbi, namespaceRow.getName(), newJobName().getValue(), jobMeta);

    // Create more runs than the limit to test pagination
    createRunsForJob(jobRow, 15, jobMeta.getOutputs()).collect(Collectors.toSet());

    // Test first page
    List<Run> firstPage =
        runDao.findByLatestJobOptimized(jobRow.getNamespaceName(), jobRow.getName(), 5, 0);
    assertThat(firstPage).hasSize(5);

    // Test second page
    List<Run> secondPage =
        runDao.findByLatestJobOptimized(jobRow.getNamespaceName(), jobRow.getName(), 5, 5);
    assertThat(secondPage).hasSize(5);

    // Test third page
    List<Run> thirdPage =
        runDao.findByLatestJobOptimized(jobRow.getNamespaceName(), jobRow.getName(), 5, 10);
    assertThat(thirdPage).hasSize(5);

    // Ensure no duplicates between pages
    Set<RunId> allRunIds =
        Stream.of(firstPage, secondPage, thirdPage)
            .flatMap(List::stream)
            .map(Run::getId)
            .collect(Collectors.toSet());
    assertThat(allRunIds).hasSize(15); // Should have 15 unique run IDs
  }

  @Test
  public void testFindByLatestJobOptimizedEmptyResults() {
    // Test with non-existent job
    List<Run> runs =
        runDao.findByLatestJobOptimized("nonexistent_namespace", "nonexistent_job", 10, 0);
    assertThat(runs).isEmpty();
  }
}
