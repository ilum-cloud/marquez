use chrono::{DateTime, Utc};
use uuid::Uuid;

fn new_id() -> u32 {
    rand::random_range(0..u32::MAX)
}

pub fn new_namespace_name() -> String {
    format!("s3://test_namespace{}", new_id())
}

pub fn new_owner_name() -> String {
    format!("test_owner{}", new_id())
}

pub fn new_source_name() -> String {
    format!("test_source{}", new_id())
}

pub fn new_dataset_name() -> String {
    format!("test_dataset{}", new_id())
}

pub fn new_job_name() -> String {
    format!("test_job{}", new_id())
}

pub fn new_field_name() -> String {
    format!("test_field{}", new_id())
}

pub fn new_tag_name() -> String {
    format!("test_tag{}", new_id())
}

pub fn new_run_id() -> Uuid {
    Uuid::new_v4()
}

pub fn new_timestamp() -> DateTime<Utc> {
    Utc::now()
}

pub fn new_description() -> String {
    format!("test_description{}", new_id())
}

pub fn new_connection_url() -> String {
    format!("postgresql://localhost:5432/test{}", new_id())
}

pub fn new_source_type() -> String {
    "POSTGRESQL".to_string()
}

pub fn new_connection_url_raw() -> String {
    format!("postgresql://localhost:5432/test{}", new_id())
}

pub fn new_checksum() -> String {
    format!("checksum_{}", new_id())
}

pub fn new_physical_name() -> String {
    format!("public.test_table{}", new_id())
}

pub fn new_external_id() -> String {
    format!("ext_{}", Uuid::new_v4())
}

pub fn new_version() -> Uuid {
    Uuid::new_v4()
}

pub fn new_dataset_type() -> String {
    "DB_TABLE".to_string()
}

pub fn new_job_type() -> String {
    "BATCH".to_string()
}
