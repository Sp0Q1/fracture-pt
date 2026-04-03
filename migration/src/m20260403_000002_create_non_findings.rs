use super::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum NonFindings {
    Table,
    Id,
    Pid,
    OrgId,
    EngagementId,
    CreatedByUserId,
    Title,
    Content,
    SortOrder,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Organizations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Engagements {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(NonFindings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NonFindings::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(NonFindings::Pid).uuid().not_null())
                    .col(ColumnDef::new(NonFindings::OrgId).integer().not_null())
                    .col(
                        ColumnDef::new(NonFindings::EngagementId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(NonFindings::CreatedByUserId)
                            .integer()
                            .null(),
                    )
                    .col(ColumnDef::new(NonFindings::Title).string().not_null())
                    .col(
                        ColumnDef::new(NonFindings::Content)
                            .text()
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(NonFindings::SortOrder)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(NonFindings::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(NonFindings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-non_findings-org_id")
                            .from(NonFindings::Table, NonFindings::OrgId)
                            .to(Organizations::Table, Organizations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-non_findings-engagement_id")
                            .from(NonFindings::Table, NonFindings::EngagementId)
                            .to(Engagements::Table, Engagements::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-non_findings-created_by_user_id")
                            .from(NonFindings::Table, NonFindings::CreatedByUserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-non_findings-pid")
                    .table(NonFindings::Table)
                    .col(NonFindings::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-non_findings-org_id-engagement_id")
                    .table(NonFindings::Table)
                    .col(NonFindings::OrgId)
                    .col(NonFindings::EngagementId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(NonFindings::Table).to_owned())
            .await?;
        Ok(())
    }
}
