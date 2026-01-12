package marquez.db;

import static org.assertj.core.api.Assertions.assertThat;

import java.time.Instant;
import java.util.UUID;
import marquez.api.JdbiUtils;
import marquez.common.models.DatasetType;
import marquez.db.models.DatasetFieldRow;
import marquez.db.models.DatasetRow;
import marquez.db.models.NamespaceRow;
import marquez.db.models.SourceRow;
import marquez.jdbi.MarquezJdbiExternalPostgresExtension;
import org.jdbi.v3.core.Jdbi;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;

@ExtendWith(MarquezJdbiExternalPostgresExtension.class)
class DatasetFieldDaoTest {

  private static final String NAMESPACE = "test_namespace";
  private static final String OWNER = "owner";
  private static final String SOURCE = "test_source";
  private static final String SOURCE_CONNECTION_URL = "jdbc:postgresql://localhost:5432/test";
  private static final String DATASET = "test_dataset";
  private static final String PHYSICAL_NAME = "physical_name";
  private static final String DESCRIPTION = "description";
  private static final String FIELD_NAME = "test_field";

  private static DatasetFieldDao datasetFieldDao;
  private static DatasetDao datasetDao;
  private static NamespaceDao namespaceDao;
  private static SourceDao sourceDao;

  @BeforeAll
  public static void setUpOnce(Jdbi jdbi) {
    datasetFieldDao = jdbi.onDemand(DatasetFieldDao.class);
    datasetDao = jdbi.onDemand(DatasetDao.class);
    namespaceDao = jdbi.onDemand(NamespaceDao.class);
    sourceDao = jdbi.onDemand(SourceDao.class);
  }

  @AfterEach
  public void tearDown(Jdbi jdbi) {
    JdbiUtils.cleanDatabase(jdbi);
  }

  @Test
  void testUpsertDuplicateFieldsWithNullType(Jdbi jdbi) {
    NamespaceRow namespace =
        namespaceDao.upsertNamespaceRow(UUID.randomUUID(), Instant.now(), NAMESPACE, OWNER);

    SourceRow source =
        sourceDao.upsert(
            UUID.randomUUID(), "POSTGRES", Instant.now(), SOURCE, SOURCE_CONNECTION_URL);

    DatasetRow dataset =
        datasetDao.upsert(
            UUID.randomUUID(),
            DatasetType.DB_TABLE,
            Instant.now(),
            namespace.getUuid(),
            namespace.getName(),
            source.getUuid(),
            source.getName(),
            DATASET,
            PHYSICAL_NAME,
            DESCRIPTION,
            false);

    UUID datasetUuid = dataset.getUuid();
    String fieldType = null;

    DatasetFieldRow row1 =
        datasetFieldDao.upsert(
            UUID.randomUUID(), Instant.now(), FIELD_NAME, fieldType, DESCRIPTION, datasetUuid);

    DatasetFieldRow row2 =
        datasetFieldDao.upsert(
            UUID.randomUUID(),
            Instant.now(),
            FIELD_NAME,
            fieldType,
            "updated description",
            datasetUuid);

    Integer fieldCount =
        jdbi.withHandle(
            h ->
                h.createQuery(
                        "SELECT count(*) FROM dataset_fields WHERE dataset_uuid = :datasetUuid AND name = :name")
                    .bind("datasetUuid", datasetUuid)
                    .bind("name", FIELD_NAME)
                    .mapTo(Integer.class)
                    .one());

    assertThat(fieldCount).isEqualTo(1);
    assertThat(row1.getUuid()).isEqualTo(row2.getUuid());
    assertThat(row1.getType()).isEqualTo("UNKNOWN");
  }
}