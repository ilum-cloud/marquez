package marquez.db;

import static org.assertj.core.api.Assertions.assertThat;

import java.time.Instant;
import java.util.Collections;
import java.util.UUID;
import marquez.jdbi.MarquezJdbiExternalPostgresExtension;
import marquez.service.models.LineageEvent.Dataset;
import marquez.service.models.LineageEvent.JobFacet;
import org.jdbi.v3.core.Jdbi;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.postgresql.util.PGobject;

@ExtendWith(MarquezJdbiExternalPostgresExtension.class)
public class LineageStatisticsTest {

  private static DatasetFacetsDao datasetFacetsDao;
  private static OpenLineageDao openLineageDao;
  private static DatasetDao datasetDao;
  private Jdbi jdbi;

  @BeforeAll
  public static void setUpOnce(Jdbi jdbi) {
    datasetFacetsDao = jdbi.onDemand(DatasetFacetsDao.class);
    openLineageDao = jdbi.onDemand(OpenLineageDao.class);
    datasetDao = jdbi.onDemand(DatasetDao.class);
  }

  @BeforeEach
  public void setup(Jdbi jdbi) {
    this.jdbi = jdbi;
  }

  @Test
  public void testLineageStatistics() {
    String namespace = "lineage_stats_test_" + UUID.randomUUID();
    String datasetName = "the_dataset";
    String producerJobName = "producer_job";
    String consumerJobName = "consumer_job";

    Dataset outputDataset = new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());

    LineageTestUtils.createLineageRow(
        openLineageDao,
        producerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.emptyList(),
        Collections.singletonList(outputDataset));

    assertLineageStatistics(namespace, datasetName, 1, 0, "[]", "[\"" + namespace + "\"]");

    Dataset inputDataset = new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());

    LineageTestUtils.createLineageRow(
        openLineageDao,
        consumerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.singletonList(inputDataset),
        Collections.emptyList());

    assertLineageStatistics(
        namespace, datasetName, 1, 1, "[\"" + namespace + "\"]", "[\"" + namespace + "\"]");
  }

  @Test
  public void testLineageStatisticsWithMultipleVersions() {
    String namespace = "lineage_stats_test_v_" + UUID.randomUUID();
    String datasetName = "the_dataset";
    String producerJobName = "producer_job";
    String consumerJobName = "consumer_job";

    Dataset outputDataset = new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());

    // 1. Create dataset version 1
    LineageTestUtils.createLineageRow(
        openLineageDao,
        producerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.emptyList(),
        Collections.singletonList(outputDataset));

    assertLineageStatistics(namespace, datasetName, 1, 0, "[]", "[\"" + namespace + "\"]");

    // 2. Consume dataset version 1
    Dataset inputDataset = new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());

    LineageTestUtils.createLineageRow(
        openLineageDao,
        consumerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.singletonList(inputDataset),
        Collections.emptyList());

    assertLineageStatistics(
        namespace, datasetName, 1, 1, "[\"" + namespace + "\"]", "[\"" + namespace + "\"]");

    // 3. Create dataset version 2 (by changing facets or just sending a new event, likely creating
    // new version)
    // To ensure new version, we add a description
    Dataset outputDatasetV2 =
        new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());
    outputDatasetV2.getFacets().getDocumentation().setDescription("v2 description");

    LineageTestUtils.createLineageRow(
        openLineageDao,
        producerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.emptyList(),
        Collections.singletonList(outputDatasetV2));

    // Stats for V2 should be: 1 producer, 1 consumer.
    // The consumer job that consumed V1 is still the current version of that job,
    // and it consumes this dataset (by UUID). Lineage stats are at Dataset level.
    // Note: assertLineageStatistics retrieves the *latest* facet (which is for V2 now)
    assertLineageStatistics(
        namespace, datasetName, 1, 1, "[\"" + namespace + "\"]", "[\"" + namespace + "\"]");

    // 4. Consume dataset version 2
    Dataset inputDatasetV2 =
        new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());
    inputDatasetV2.getFacets().getDocumentation().setDescription("v2 description");

    LineageTestUtils.createLineageRow(
        openLineageDao,
        consumerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.singletonList(inputDatasetV2),
        Collections.emptyList());

    assertLineageStatistics(
        namespace, datasetName, 1, 1, "[\"" + namespace + "\"]", "[\"" + namespace + "\"]");
  }

  @Test
  public void testLineageStatisticsModification() {
    String namespace = "lineage_stats_mod_" + UUID.randomUUID();
    String datasetName = "the_dataset";
    String producerJobName = "producer_job";
    String consumerJobName = "consumer_job";

    Dataset outputDataset = new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());
    Dataset inputDataset = new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());

    // 1. Initial State: Producer -> Dataset -> Consumer
    LineageTestUtils.createLineageRow(
        openLineageDao,
        producerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.emptyList(),
        Collections.singletonList(outputDataset));

    LineageTestUtils.createLineageRow(
        openLineageDao,
        consumerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.singletonList(inputDataset),
        Collections.emptyList());

    assertLineageStatistics(
        namespace, datasetName, 1, 1, "[\"" + namespace + "\"]", "[\"" + namespace + "\"]");

    // 2. Modify Consumer Job to NOT use the dataset anymore (e.g. use a different input)
    String otherDatasetName = "other_dataset";
    Dataset otherInputDataset =
        new Dataset(namespace, otherDatasetName, LineageTestUtils.newDatasetFacet());

    // Create a new version of consumer job that uses 'other_dataset' instead
    LineageTestUtils.createLineageRow(
        openLineageDao,
        consumerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.singletonList(otherInputDataset),
        Collections.emptyList());

    // Now 'the_dataset' should have 0 consumers (outEdges) because the consumer job's current
    // version doesn't use it.
    // However, the producer job still produces it.
    assertLineageStatistics(namespace, datasetName, 1, 0, "[]", "[\"" + namespace + "\"]");
  }

  @Test
  public void testComplexLineage() {
    String namespace = "complex_lineage_" + UUID.randomUUID();
    String datasetName = "hub_dataset";
    String producer1 = "producer_1";
    String producer2 = "producer_2";
    String consumer1 = "consumer_1";
    String consumer2 = "consumer_2";

    Dataset dataset = new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());

    // Producer 1
    LineageTestUtils.createLineageRow(
        openLineageDao,
        producer1,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.emptyList(),
        Collections.singletonList(dataset));

    assertLineageStatistics(namespace, datasetName, 1, 0, "[]", "[\"" + namespace + "\"]");

    // Producer 2
    LineageTestUtils.createLineageRow(
        openLineageDao,
        producer2,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.emptyList(),
        Collections.singletonList(dataset));

    // Should have 2 inEdges (producers)
    assertLineageStatistics(namespace, datasetName, 2, 0, "[]", "[\"" + namespace + "\"]");

    // Consumer 1
    LineageTestUtils.createLineageRow(
        openLineageDao,
        consumer1,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.singletonList(dataset),
        Collections.emptyList());

    assertLineageStatistics(
        namespace, datasetName, 2, 1, "[\"" + namespace + "\"]", "[\"" + namespace + "\"]");

    // Consumer 2
    LineageTestUtils.createLineageRow(
        openLineageDao,
        consumer2,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.singletonList(dataset),
        Collections.emptyList());

    assertLineageStatistics(
        namespace, datasetName, 2, 2, "[\"" + namespace + "\"]", "[\"" + namespace + "\"]");
  }

  private void assertLineageStatistics(
      String namespace,
      String datasetName,
      int expectedInEdges,
      int expectedOutEdges,
      String expectedConsumingNamespaces,
      String expectedProducingNamespaces) {

    UUID datasetUuid = datasetDao.getUuid(namespace, datasetName).get().getUuid();

    DatasetFacetsDao.DatasetFacetRow facetRow = getDatasetFacet(datasetUuid, "lineageStatistics");

    assertThat(facetRow).isNotNull();
    PGobject facetJson = facetRow.facet();
    String json = facetJson.getValue();

    assertThat(json).contains("\"lineageStatistics\": {");
    assertThat(json).contains("\"inEdges\": " + expectedInEdges);
    assertThat(json).contains("\"outEdges\": " + expectedOutEdges);
  }

  private DatasetFacetsDao.DatasetFacetRow getDatasetFacet(UUID datasetUuid, String facetName) {
    return jdbi.withHandle(
        h ->
            h.createQuery(
                    "SELECT * FROM dataset_facets "
                        + "WHERE name = :facetName AND dataset_uuid = :datasetUuid "
                        + "ORDER BY created_at DESC LIMIT 1")
                .bind("facetName", facetName)
                .bind("datasetUuid", datasetUuid)
                .map(
                    rv ->
                        new DatasetFacetsDao.DatasetFacetRow(
                            rv.getColumn("created_at", Instant.class),
                            rv.getColumn("dataset_uuid", UUID.class),
                            rv.getColumn("dataset_version_uuid", UUID.class),
                            rv.getColumn("run_uuid", UUID.class),
                            rv.getColumn("lineage_event_time", Instant.class),
                            rv.getColumn("lineage_event_type", String.class),
                            rv.getColumn("type", DatasetFacetsDao.Type.class),
                            rv.getColumn("name", String.class),
                            rv.getColumn("facet", PGobject.class)))
                .findOne()
                .orElse(null));
  }
}
