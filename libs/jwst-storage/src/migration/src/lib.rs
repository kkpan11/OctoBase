pub use sea_orm_migration::prelude::*;

mod m20220101_000001_initial_blob_table;
mod m20220101_000002_initial_doc_table;
mod m20230321_000001_blob_optimized_table;
mod m20230614_000001_initial_bucket_blob_table;
mod m20230626_023319_doc_guid;
mod m20230814_061223_initial_diff_log_table;
mod m20231124_082401_blob_deleted_at;
mod schemas;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_initial_blob_table::Migration),
            Box::new(m20220101_000002_initial_doc_table::Migration),
            Box::new(m20230321_000001_blob_optimized_table::Migration),
            Box::new(m20230614_000001_initial_bucket_blob_table::Migration),
            Box::new(m20230626_023319_doc_guid::Migration),
            Box::new(m20230814_061223_initial_diff_log_table::Migration),
            Box::new(m20231124_082401_blob_deleted_at::Migration),
        ]
    }
}
