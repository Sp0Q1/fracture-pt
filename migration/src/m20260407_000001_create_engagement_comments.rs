use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum EngagementComments {
    Table,
    Id,
    Pid,
    EngagementId,
    UserId,
    Content,
    CreatedAt,
    UpdatedAt,
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
                    .table(EngagementComments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EngagementComments::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(EngagementComments::Pid).uuid().not_null())
                    .col(
                        ColumnDef::new(EngagementComments::EngagementId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EngagementComments::UserId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EngagementComments::Content)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EngagementComments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(EngagementComments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-engagement_comments-engagement_id")
                            .from(EngagementComments::Table, EngagementComments::EngagementId)
                            .to(Engagements::Table, Engagements::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-engagement_comments-user_id")
                            .from(EngagementComments::Table, EngagementComments::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-engagement_comments-pid")
                    .table(EngagementComments::Table)
                    .col(EngagementComments::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-engagement_comments-engagement_id")
                    .table(EngagementComments::Table)
                    .col(EngagementComments::EngagementId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(EngagementComments::Table).to_owned())
            .await
    }
}
