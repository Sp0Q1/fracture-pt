use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Findings {
    Table,
    JobRunId,
}

#[derive(DeriveIden)]
enum JobRuns {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Findings::Table)
                    .add_column(ColumnDef::new(Findings::JobRunId).integer().null())
                    .add_foreign_key(
                        TableForeignKey::new()
                            .name("fk-findings-job_run_id")
                            .from_tbl(Findings::Table)
                            .from_col(Findings::JobRunId)
                            .to_tbl(JobRuns::Table)
                            .to_col(JobRuns::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
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
