// Copyright 2024-2025 Ilum Labs LLC
// SPDX-License-Identifier: Apache-2.0

use sqlx::PgPool;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("failed to read migrations directory: {0}")]
    ReadDir(#[from] std::io::Error),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("invalid migration filename: {0}")]
    InvalidFilename(String),

    #[error("migration {version} ({name}) failed: {source}")]
    ExecutionFailed {
        version: String,
        name: String,
        source: sqlx::Error,
    },
}

pub struct MigrationRunner {
    migrations_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct MigrationFile {
    version: MigrationVersion,
    name: String,
    sql: String,
    checksum: String,
}

#[derive(Debug, Clone)]
enum MigrationVersion {
    Versioned(Vec<u64>),
    Repeatable(String),
}

impl MigrationVersion {
    fn parse(filename: &str) -> Option<Self> {
        if filename.starts_with("R__") {
            let name = filename.strip_prefix("R__")?.strip_suffix(".sql")?;
            Some(MigrationVersion::Repeatable(name.to_string()))
        } else if filename.starts_with('V') {
            let rest = filename.strip_prefix('V')?;
            let version_str = rest.split("__").next()?;
            let parts: Result<Vec<u64>, _> = version_str.split('.').map(|s| s.parse()).collect();
            Some(MigrationVersion::Versioned(parts.ok()?))
        } else {
            None
        }
    }

    fn version_key(&self) -> String {
        match self {
            MigrationVersion::Versioned(parts) => parts
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join("."),
            MigrationVersion::Repeatable(name) => format!("R__{}", name),
        }
    }
}

impl PartialEq for MigrationVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for MigrationVersion {}

impl PartialOrd for MigrationVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MigrationVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (MigrationVersion::Versioned(a), MigrationVersion::Versioned(b)) => {
                // Compare component by component: [76, 1] vs [76] -> [76, 1] > [76]
                let max_len = a.len().max(b.len());
                for i in 0..max_len {
                    let va = a.get(i).copied().unwrap_or(0);
                    let vb = b.get(i).copied().unwrap_or(0);
                    match va.cmp(&vb) {
                        Ordering::Equal => continue,
                        ord => return ord,
                    }
                }
                Ordering::Equal
            }
            // Versioned always comes before Repeatable
            (MigrationVersion::Versioned(_), MigrationVersion::Repeatable(_)) => Ordering::Less,
            (MigrationVersion::Repeatable(_), MigrationVersion::Versioned(_)) => Ordering::Greater,
            (MigrationVersion::Repeatable(a), MigrationVersion::Repeatable(b)) => a.cmp(b),
        }
    }
}

/// Computes a hex-encoded FNV-1a checksum of `data` (for `_marquez_migrations`).
fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write;
    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV prime
    }
    let mut s = String::with_capacity(16);
    let _ = write!(s, "{:016x}", hash);
    s
}

/// Computes a CRC32 checksum matching `java.util.zip.CRC32` (IEEE polynomial).
/// Used for Flyway-compatible `flyway_schema_history` entries.
fn flyway_crc32(data: &[u8]) -> i32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320; // IEEE polynomial (reversed)
            } else {
                crc >>= 1;
            }
        }
    }
    (crc ^ 0xFFFF_FFFF) as i32
}

/// Extracts a Flyway-style description from a migration filename.
/// E.g. `V1__initial_schema.sql` → `"initial schema"`
fn flyway_description(filename: &str) -> String {
    let without_ext = filename.strip_suffix(".sql").unwrap_or(filename);
    let desc = if let Some(pos) = without_ext.find("__") {
        &without_ext[pos + 2..]
    } else {
        without_ext
    };
    desc.replace('_', " ")
}

/// Java-only migrations that the Rust runner cannot execute directly.
/// We record them in `flyway_schema_history` so the Java/Flyway backend
/// recognizes them as already applied, and we execute any schema effects
/// (like view creation) that downstream code depends on.
struct JavaMigrationStub {
    version: &'static str,
    description: &'static str,
    script: &'static str,
    /// SQL to execute for schema effects (empty = data-only backfill, no-op).
    sql: &'static str,
}

/// All Java-only Flyway migrations, in version order.
/// Values taken directly from each class's `getVersion()`, `getDescription()`,
/// and `getChecksum()` (all return null → we store NULL).
const JAVA_MIGRATION_STUBS: &[JavaMigrationStub] = &[
    JavaMigrationStub {
        version: "44.1",
        description: "UpdateRunsWithJobUUID",
        script: "marquez.db.migrations.V44_1__UpdateRunsWithJobUUID",
        sql: "",
    },
    JavaMigrationStub {
        version: "44.2",
        description: "BackfillAirflowParentRuns",
        script: "marquez.db.migrations.V44_2__BackfillAirflowParentRuns",
        sql: "",
    },
    JavaMigrationStub {
        version: "44.3",
        description: "BackfillJobsWithParents",
        script: "marquez.db.migrations.V44_3_BackfillJobsWithParents",
        sql: "",
    },
    JavaMigrationStub {
        version: "56.1",
        description: "CreateFacetViews",
        script: "marquez.db.migrations.V56_1__FacetViews",
        sql: "", // V57.2 replaces these views, so skip executing V56.1's SQL
    },
    JavaMigrationStub {
        version: "57.2",
        description: "BackFillFacets",
        script: "marquez.db.migrations.V57_1__BackfillFacets",
        sql: "CREATE OR REPLACE VIEW job_facets_view AS SELECT * FROM job_facets;\
              CREATE OR REPLACE VIEW run_facets_view AS SELECT * FROM run_facets;\
              CREATE OR REPLACE VIEW dataset_facets_view AS SELECT * FROM dataset_facets;",
    },
    JavaMigrationStub {
        version: "66.3",
        description: "BackfillJobFacetsWithJobVersion",
        script: "marquez.db.migrations.V66_3_JobFacetsBackfillJobVersion",
        sql: "",
    },
    JavaMigrationStub {
        version: "67.2",
        description:
            "Back fill job_uuid and is_current_job_version in job_versions_io_mapping table",
        script: "marquez.db.migrations.V67_2_JobVersionsIOMappingBackfillJob",
        sql: "",
    },
];

impl MigrationRunner {
    pub fn new(migrations_dir: impl AsRef<Path>) -> Self {
        Self {
            migrations_dir: migrations_dir.as_ref().to_path_buf(),
        }
    }

    pub async fn run_all(&self, pool: &PgPool) -> Result<(), MigrationError> {
        let migrations = self.load_migrations()?;

        self.ensure_tracking_table(pool).await?;
        self.ensure_flyway_table(pool).await?;

        // Separate versioned and repeatable
        let (mut versioned, repeatable): (Vec<_>, Vec<_>) = migrations
            .into_iter()
            .partition(|m| matches!(m.version, MigrationVersion::Versioned(_)));

        versioned.sort_by(|a, b| a.version.cmp(&b.version));

        // Match Flyway behavior: sort repeatable migrations alphabetically
        let mut repeatable = repeatable;
        repeatable.sort_by(|a, b| a.version.cmp(&b.version));

        // Apply versioned migrations in order, skipping already-applied
        let applied = self.get_applied_versions(pool).await?;
        for migration in &versioned {
            let key = migration.version.version_key();
            if applied.contains(&key) {
                tracing::debug!(version = %key, name = %migration.name, "skipping already-applied migration");
                continue;
            }
            tracing::info!(version = %key, name = %migration.name, "applying migration");
            self.apply_migration(pool, migration).await?;
        }

        // Record Java-only migrations and apply their schema effects (e.g. views)
        self.apply_java_migration_stubs(pool).await?;

        // Apply repeatable migrations only when checksum changes
        for migration in &repeatable {
            let key = migration.version.version_key();
            if self
                .is_repeatable_current(pool, &key, &migration.checksum)
                .await?
            {
                tracing::debug!(name = %key, "skipping unchanged repeatable migration");
                continue;
            }
            tracing::info!(name = %key, "applying repeatable migration");
            self.apply_migration(pool, migration).await?;
        }

        Ok(())
    }

    fn load_migrations(&self) -> Result<Vec<MigrationFile>, MigrationError> {
        let mut migrations = Vec::new();

        for entry in std::fs::read_dir(&self.migrations_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Skip non-.sql files (e.g., V57__readme.md)
            if path.extension().and_then(|e| e.to_str()) != Some("sql") {
                continue;
            }

            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| MigrationError::InvalidFilename(format!("{:?}", path)))?;

            let version = MigrationVersion::parse(filename)
                .ok_or_else(|| MigrationError::InvalidFilename(filename.to_string()))?;

            let sql = std::fs::read_to_string(&path)?;
            let checksum = sha256_hex(sql.as_bytes());

            migrations.push(MigrationFile {
                version,
                name: filename.to_string(),
                sql,
                checksum,
            });
        }

        Ok(migrations)
    }

    async fn ensure_tracking_table(&self, pool: &PgPool) -> Result<(), MigrationError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS _marquez_migrations (
                version     TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                checksum    TEXT NOT NULL DEFAULT '',
                applied_at  TIMESTAMPTZ NOT NULL DEFAULT now()
            )
            "#,
        )
        .execute(pool)
        .await?;

        // Add checksum column if upgrading from older schema that lacked it
        sqlx::query(
            r#"
            ALTER TABLE _marquez_migrations
            ADD COLUMN IF NOT EXISTS checksum TEXT NOT NULL DEFAULT ''
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Creates the Flyway schema history table so the Java backend can recognize
    /// migrations applied by the Rust backend. This enables switching between
    /// backends against the same database.
    async fn ensure_flyway_table(&self, pool: &PgPool) -> Result<(), MigrationError> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS flyway_schema_history (
                installed_rank INT NOT NULL,
                version        VARCHAR(50),
                description    VARCHAR(200) NOT NULL,
                type           VARCHAR(20) NOT NULL,
                script         VARCHAR(1000) NOT NULL,
                checksum       INT,
                installed_by   VARCHAR(100) NOT NULL,
                installed_on   TIMESTAMP NOT NULL DEFAULT now(),
                execution_time INT NOT NULL,
                success        BOOLEAN NOT NULL,
                CONSTRAINT flyway_schema_history_pk PRIMARY KEY (installed_rank)
            )
            "#,
        )
        .execute(pool)
        .await?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS flyway_schema_history_s_idx \
             ON flyway_schema_history (success)",
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Records Java-only Flyway migrations in `flyway_schema_history` and
    /// executes any schema effects (view creation) they carry. This allows the
    /// Java backend to start without attempting to re-run these migrations.
    async fn apply_java_migration_stubs(&self, pool: &PgPool) -> Result<(), MigrationError> {
        for stub in JAVA_MIGRATION_STUBS {
            // Skip if already recorded
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM flyway_schema_history WHERE version = $1)",
            )
            .bind(stub.version)
            .fetch_one(pool)
            .await?;

            if exists {
                continue;
            }

            // Execute schema effects (e.g. CREATE OR REPLACE VIEW)
            if !stub.sql.is_empty() {
                for stmt in split_statements(stub.sql) {
                    let trimmed = stmt.trim();
                    if !trimmed.is_empty() {
                        sqlx::query(trimmed).execute(pool).await?;
                    }
                }
                tracing::info!(
                    version = %stub.version,
                    description = %stub.description,
                    "applied Java migration stub (schema effects)"
                );
            } else {
                tracing::info!(
                    version = %stub.version,
                    description = %stub.description,
                    "recorded Java migration stub (data-only, skipped)"
                );
            }

            // Record in flyway_schema_history with type=JDBC, checksum=NULL
            // (matching Java's BaseJavaMigration.getChecksum() returning null)
            let next_rank: i32 = sqlx::query_scalar::<_, Option<i32>>(
                "SELECT COALESCE(MAX(installed_rank), 0) + 1 FROM flyway_schema_history",
            )
            .fetch_one(pool)
            .await?
            .unwrap_or(1);

            sqlx::query(
                r#"
                INSERT INTO flyway_schema_history
                    (installed_rank, version, description, type, script,
                     checksum, installed_by, execution_time, success)
                VALUES ($1, $2, $3, 'JDBC', $4, NULL, 'marquez-rs', 0, true)
                "#,
            )
            .bind(next_rank)
            .bind(stub.version)
            .bind(stub.description)
            .bind(stub.script)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    async fn get_applied_versions(&self, pool: &PgPool) -> Result<Vec<String>, MigrationError> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT version FROM _marquez_migrations WHERE version NOT LIKE 'R__%'")
                .fetch_all(pool)
                .await?;
        Ok(rows.into_iter().map(|(v,)| v).collect())
    }

    /// Returns true if the repeatable migration's stored checksum matches.
    async fn is_repeatable_current(
        &self,
        pool: &PgPool,
        key: &str,
        checksum: &str,
    ) -> Result<bool, MigrationError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT checksum FROM _marquez_migrations WHERE version = $1")
                .bind(key)
                .fetch_optional(pool)
                .await?;
        Ok(row.is_some_and(|(stored,)| stored == checksum))
    }

    async fn apply_migration(
        &self,
        pool: &PgPool,
        migration: &MigrationFile,
    ) -> Result<(), MigrationError> {
        let key = migration.version.version_key();
        let start = std::time::Instant::now();

        // Split SQL into individual statements (respects dollar-quoting)
        let statements = split_statements(&migration.sql);

        let mut tx = pool.begin().await?;

        for stmt in &statements {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }
            sqlx::query(trimmed).execute(&mut *tx).await.map_err(|e| {
                MigrationError::ExecutionFailed {
                    version: key.clone(),
                    name: migration.name.clone(),
                    source: e,
                }
            })?;
        }

        // Upsert the tracking record (_marquez_migrations)
        sqlx::query(
            r#"
            INSERT INTO _marquez_migrations (version, name, checksum, applied_at)
            VALUES ($1, $2, $3, now())
            ON CONFLICT (version) DO UPDATE SET checksum = $3, applied_at = now()
            "#,
        )
        .bind(&key)
        .bind(&migration.name)
        .bind(&migration.checksum)
        .execute(&mut *tx)
        .await
        .map_err(|e| MigrationError::ExecutionFailed {
            version: key.clone(),
            name: migration.name.clone(),
            source: e,
        })?;

        let elapsed_ms = start.elapsed().as_millis() as i32;

        // Also record in flyway_schema_history for Java backend compatibility
        let flyway_version: Option<&str> = match &migration.version {
            MigrationVersion::Versioned(_) => Some(key.as_str()),
            MigrationVersion::Repeatable(_) => None,
        };
        // Flyway uses "SQL" for both versioned and repeatable migrations;
        // repeatable migrations are distinguished by a NULL version column.
        let flyway_type = "SQL";
        let description = flyway_description(&migration.name);
        let flyway_checksum = flyway_crc32(migration.sql.as_bytes());

        // Get next installed_rank
        let next_rank: i32 = sqlx::query_scalar::<_, Option<i32>>(
            "SELECT COALESCE(MAX(installed_rank), 0) + 1 FROM flyway_schema_history",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| MigrationError::ExecutionFailed {
            version: key.clone(),
            name: migration.name.clone(),
            source: e,
        })?
        .unwrap_or(1);

        sqlx::query(
            r#"
            INSERT INTO flyway_schema_history
                (installed_rank, version, description, type, script,
                 checksum, installed_by, execution_time, success)
            VALUES ($1, $2, $3, $4, $5, $6, 'marquez-rs', $7, true)
            ON CONFLICT (installed_rank) DO NOTHING
            "#,
        )
        .bind(next_rank)
        .bind(flyway_version)
        .bind(&description)
        .bind(flyway_type)
        .bind(&migration.name)
        .bind(flyway_checksum)
        .bind(elapsed_ms)
        .execute(&mut *tx)
        .await
        .map_err(|e| MigrationError::ExecutionFailed {
            version: key.clone(),
            name: migration.name.clone(),
            source: e,
        })?;

        tx.commit().await?;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Dollar-quote-aware SQL statement splitter
// ─────────────────────────────────────────────────────────────────────────────

/// Splits SQL text into individual statements, respecting:
/// - Dollar-quoted strings (`$$`, `$func$`, `$tag$`)
/// - Single-quoted strings (with `''` escape)
/// - Line comments (`--`)
/// - Block comments (`/* ... */`, nestable)
///
/// Semicolons inside any of the above are NOT treated as statement terminators.
fn split_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = sql.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // Dollar-quote start: $$ or $tag$
        if ch == '$' {
            if let Some(tag) = try_parse_dollar_tag(&chars, i) {
                let tag_str: String = tag.iter().collect();
                current.push_str(&tag_str);
                i += tag.len();

                // Scan for matching close tag
                while i < len {
                    if chars[i] == '$' {
                        if let Some(close_tag) = try_parse_dollar_tag(&chars, i) {
                            let close_str: String = close_tag.iter().collect();
                            if close_str == tag_str {
                                current.push_str(&close_str);
                                i += close_tag.len();
                                break;
                            }
                        }
                    }
                    current.push(chars[i]);
                    i += 1;
                }
                continue;
            }
        }

        // Single-quoted string
        if ch == '\'' {
            current.push(ch);
            i += 1;
            while i < len {
                current.push(chars[i]);
                if chars[i] == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        current.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Line comment: --
        if ch == '-' && i + 1 < len && chars[i + 1] == '-' {
            while i < len && chars[i] != '\n' {
                current.push(chars[i]);
                i += 1;
            }
            continue;
        }

        // Block comment: /* ... */ (nestable)
        if ch == '/' && i + 1 < len && chars[i + 1] == '*' {
            current.push(chars[i]);
            current.push(chars[i + 1]);
            i += 2;
            let mut depth = 1;
            while i < len && depth > 0 {
                if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
                    depth += 1;
                    current.push(chars[i]);
                    current.push(chars[i + 1]);
                    i += 2;
                } else if chars[i] == '*' && i + 1 < len && chars[i + 1] == '/' {
                    depth -= 1;
                    current.push(chars[i]);
                    current.push(chars[i + 1]);
                    i += 2;
                } else {
                    current.push(chars[i]);
                    i += 1;
                }
            }
            continue;
        }

        // Statement terminator
        if ch == ';' {
            if !current.trim().is_empty() {
                statements.push(current.clone());
            }
            current.clear();
            i += 1;
            continue;
        }

        current.push(ch);
        i += 1;
    }

    // Trailing content without final semicolon
    if !current.trim().is_empty() {
        statements.push(current);
    }

    statements
}

/// Tries to parse a dollar-quote tag at position `pos`.
/// A tag is `$` + optional identifier (alphanumeric/underscore) + `$`.
fn try_parse_dollar_tag(chars: &[char], pos: usize) -> Option<Vec<char>> {
    if pos >= chars.len() || chars[pos] != '$' {
        return None;
    }

    let mut tag = vec!['$'];
    let mut j = pos + 1;

    while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
        tag.push(chars[j]);
        j += 1;
    }

    if j < chars.len() && chars[j] == '$' {
        tag.push('$');
        Some(tag)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Version parsing ───

    #[test]
    fn test_parse_versioned() {
        let v = MigrationVersion::parse("V1__initial_schema.sql").unwrap();
        assert!(matches!(v, MigrationVersion::Versioned(ref parts) if parts == &[1]));

        let v = MigrationVersion::parse("V2.1__alter.sql").unwrap();
        assert!(matches!(v, MigrationVersion::Versioned(ref parts) if parts == &[2, 1]));

        let v = MigrationVersion::parse("V55.3__add_run_facets.sql").unwrap();
        assert!(matches!(v, MigrationVersion::Versioned(ref parts) if parts == &[55, 3]));
    }

    #[test]
    fn test_parse_repeatable() {
        let v = MigrationVersion::parse("R__Datasets_view.sql").unwrap();
        assert!(matches!(v, MigrationVersion::Repeatable(ref name) if name == "Datasets_view"));
    }

    #[test]
    fn test_parse_non_sql_skipped_by_extension_check() {
        // MigrationVersion::parse only looks at V/R prefix and version number.
        // Non-SQL files (like V57__readme.md) are filtered out in load_migrations()
        // by checking the file extension, not during version parsing.
        // The .md file would parse as V57, but load_migrations skips it.
        let v = MigrationVersion::parse("V57__readme.md");
        assert!(v.is_some()); // parse itself doesn't filter on extension
    }

    // ─── Version ordering ───

    #[test]
    fn test_version_ordering() {
        let v1 = MigrationVersion::Versioned(vec![1]);
        let v2 = MigrationVersion::Versioned(vec![2]);
        let v2_1 = MigrationVersion::Versioned(vec![2, 1]);
        let v3 = MigrationVersion::Versioned(vec![3]);
        let v55_3 = MigrationVersion::Versioned(vec![55, 3]);
        let v76 = MigrationVersion::Versioned(vec![76]);
        let v76_1 = MigrationVersion::Versioned(vec![76, 1]);
        let r = MigrationVersion::Repeatable("test".to_string());

        assert!(v1 < v2);
        assert!(v2 < v2_1);
        assert!(v2_1 < v3);
        assert!(v55_3 < v76);
        assert!(v76 < v76_1);
        assert!(v76_1 < r);
    }

    #[test]
    fn test_full_sort_order() {
        let mut versions = vec![
            MigrationVersion::Repeatable("Runs_view".to_string()),
            MigrationVersion::Versioned(vec![76, 1]),
            MigrationVersion::Versioned(vec![2, 1]),
            MigrationVersion::Versioned(vec![1]),
            MigrationVersion::Versioned(vec![55, 3]),
            MigrationVersion::Versioned(vec![2]),
            MigrationVersion::Versioned(vec![55, 1]),
            MigrationVersion::Repeatable("Datasets_view".to_string()),
            MigrationVersion::Versioned(vec![76]),
            MigrationVersion::Versioned(vec![3]),
        ];
        versions.sort();
        let expected = vec![
            MigrationVersion::Versioned(vec![1]),
            MigrationVersion::Versioned(vec![2]),
            MigrationVersion::Versioned(vec![2, 1]),
            MigrationVersion::Versioned(vec![3]),
            MigrationVersion::Versioned(vec![55, 1]),
            MigrationVersion::Versioned(vec![55, 3]),
            MigrationVersion::Versioned(vec![76]),
            MigrationVersion::Versioned(vec![76, 1]),
            MigrationVersion::Repeatable("Datasets_view".to_string()),
            MigrationVersion::Repeatable("Runs_view".to_string()),
        ];
        assert_eq!(versions, expected);
    }

    // ─── Version key ───

    #[test]
    fn test_version_key() {
        let v = MigrationVersion::Versioned(vec![2, 1]);
        assert_eq!(v.version_key(), "2.1");

        let v = MigrationVersion::Versioned(vec![55, 3]);
        assert_eq!(v.version_key(), "55.3");

        let r = MigrationVersion::Repeatable("Datasets_view".to_string());
        assert_eq!(r.version_key(), "R__Datasets_view");
    }

    // ─── Statement splitting ───

    #[test]
    fn test_split_simple() {
        let sql = "CREATE TABLE foo (id INT);\nCREATE TABLE bar (id INT);";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("foo"));
        assert!(stmts[1].contains("bar"));
    }

    #[test]
    fn test_split_dollar_func_quoting() {
        let sql = r#"
CREATE OR REPLACE FUNCTION write_run_uuid()
RETURNS trigger
LANGUAGE plpgsql AS
$func$
BEGIN
    NEW.run_uuid := NEW.run_id::uuid;
    RETURN NEW;
END
$func$;

CREATE TRIGGER lineage_events_run_uuid
    BEFORE INSERT ON lineage_events
    FOR EACH ROW
EXECUTE PROCEDURE write_run_uuid();
"#;
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2);
        // The function body (with its semicolons) stays in one statement
        assert!(stmts[0].contains("$func$"));
        assert!(stmts[0].contains("NEW.run_uuid := NEW.run_id::uuid;"));
        assert!(stmts[1].contains("CREATE TRIGGER"));
    }

    #[test]
    fn test_split_double_dollar_quoting() {
        let sql = r#"
DO
$$
DECLARE
    x boolean;
BEGIN
    SELECT TRUE INTO x;
    IF x THEN
        ALTER TABLE t ALTER COLUMN c TYPE VARCHAR;
    END IF;
END
$$ LANGUAGE plpgsql;
"#;
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("ALTER TABLE"));
    }

    #[test]
    fn test_split_single_quoted_string() {
        let sql = "INSERT INTO foo VALUES ('hello; world');";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 1);
        assert!(stmts[0].contains("hello; world"));
    }

    #[test]
    fn test_split_escaped_quotes() {
        let sql = "INSERT INTO foo VALUES ('it''s a test;');";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_split_line_comment() {
        let sql = "-- comment;\nCREATE TABLE foo (id INT); -- trailing\nCREATE TABLE bar (id INT);";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn test_split_block_comment() {
        let sql = "/* SPDX; License */\n\nCREATE TABLE foo (id INT);";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_split_empty() {
        assert!(split_statements("").is_empty());
        assert!(split_statements("   \n  ").is_empty());
    }

    #[test]
    fn test_split_no_trailing_semicolon() {
        let sql = "CREATE TABLE foo (id INT)";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 1);
    }

    #[test]
    fn test_split_rewrite_function_pattern() {
        // Exercises the R__Jobs_view_and_rewrite_function.sql pattern
        let sql = r#"
CREATE OR REPLACE VIEW jobs_view AS SELECT * FROM jobs;

CREATE OR REPLACE FUNCTION rewrite_jobs_fqn_table() RETURNS TRIGGER AS
$$
DECLARE
    job_uuid uuid;
BEGIN
    INSERT INTO jobs (uuid, type) SELECT NEW.uuid, NEW.type
    ON CONFLICT (namespace_uuid, name)
        DO UPDATE SET updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS update_symlinks ON jobs_view;

CREATE TRIGGER update_symlinks
    INSTEAD OF UPDATE OR INSERT
    ON jobs_view
    FOR EACH ROW
EXECUTE FUNCTION rewrite_jobs_fqn_table();
"#;
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 4);
        assert!(stmts[0].contains("CREATE OR REPLACE VIEW"));
        assert!(stmts[1].contains("FUNCTION rewrite_jobs_fqn_table"));
        assert!(stmts[1].contains("ON CONFLICT"));
        assert!(stmts[2].contains("DROP TRIGGER"));
        assert!(stmts[3].contains("CREATE TRIGGER"));
    }

    // ─── Checksum ───

    #[test]
    fn test_checksum_deterministic() {
        let a = sha256_hex(b"hello world");
        let b = sha256_hex(b"hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn test_checksum_different_for_different_input() {
        let a = sha256_hex(b"hello");
        let b = sha256_hex(b"world");
        assert_ne!(a, b);
    }

    // ─── Flyway CRC32 ───

    #[test]
    fn test_flyway_crc32_known_value() {
        // CRC32 of empty string is 0
        assert_eq!(flyway_crc32(b""), 0);
        // CRC32 of "123456789" is 0xCBF43926 = 3421780262 → as i32: -873187034
        assert_eq!(flyway_crc32(b"123456789"), -873187034_i32);
    }

    #[test]
    fn test_flyway_crc32_deterministic() {
        let a = flyway_crc32(b"CREATE TABLE foo (id INT);");
        let b = flyway_crc32(b"CREATE TABLE foo (id INT);");
        assert_eq!(a, b);
    }

    // ─── Flyway description ───

    #[test]
    fn test_flyway_description() {
        assert_eq!(
            flyway_description("V1__initial_schema.sql"),
            "initial schema"
        );
        assert_eq!(
            flyway_description("V76.1__BackfillLineageStatistics.sql"),
            "BackfillLineageStatistics"
        );
        assert_eq!(flyway_description("R__Datasets_view.sql"), "Datasets view");
    }
}
