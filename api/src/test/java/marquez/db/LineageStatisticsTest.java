package marquez.db;

import static org.assertj.core.api.Assertions.assertThat;

import java.time.Instant;
import java.util.Collections;
import java.util.UUID;
import marquez.jdbi.MarquezJdbiExternalPostgresExtension;
import marquez.service.models.LineageEvent;
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

    Dataset outputDataset =
        new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());

    LineageTestUtils.createLineageRow(
        openLineageDao,
        producerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.emptyList(),
        Collections.singletonList(outputDataset));

    assertLineageStatistics(namespace, datasetName, 1, 0, "[]", "[\"" + namespace + "\"]");

    Dataset inputDataset =
        new Dataset(namespace, datasetName, LineageTestUtils.newDatasetFacet());

    LineageTestUtils.createLineageRow(
        openLineageDao,
        consumerJobName,
        "COMPLETE",
        JobFacet.builder().build(),
        Collections.singletonList(inputDataset),
        Collections.emptyList());

    assertLineageStatistics(namespace, datasetName, 1, 1, "[\"" + namespace + "\"]", "[\"" + namespace + "\"]");
  }

  private void assertLineageStatistics(
      String namespace, String datasetName, int expectedInEdges, int expectedOutEdges, String expectedConsumingNamespaces, String expectedProducingNamespaces) {

    UUID datasetUuid =
        datasetDao.getUuid(namespace, datasetName).get().getUuid();

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
                      "SELECT * FROM dataset_facets " +
                      "WHERE name = :facetName AND dataset_uuid = :datasetUuid " +
                      "ORDER BY created_at DESC LIMIT 1")
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
                  .findOne().orElse(null));
    }
}