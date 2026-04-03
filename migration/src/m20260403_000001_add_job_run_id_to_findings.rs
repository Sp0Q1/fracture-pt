use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Findings {
    Table,
    JobRunId,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite cannot add FK constraints via ALTER TABLE.
        // Add the column without FK — integrity is enforced at the app level.
        manager
            .alter_table(
                Table::alter()
                    .table(Findings::Table)
                    .add_column(ColumnDef::new(Findings::JobRunId).integer().null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-findings-job_run_id")
                    .table(Findings::Table)
                    .col(Findings::JobRunId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Findings::Table)
                    .drop_column(Findings::JobRunId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
