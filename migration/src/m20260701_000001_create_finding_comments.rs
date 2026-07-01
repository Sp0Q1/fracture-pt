use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum FindingComments {
    Table,
    Id,
    Pid,
    FindingId,
    UserId,
    Content,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Findings {
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
                    .table(FindingComments::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FindingComments::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(FindingComments::Pid).uuid().not_null())
                    .col(ColumnDef::new(FindingComments::FindingId).integer().not_null())
                    .col(ColumnDef::new(FindingComments::UserId).integer().not_null())
                    .col(ColumnDef::new(FindingComments::Content).text().not_null())
                    .col(
                        ColumnDef::new(FindingComments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FindingComments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-finding_comments-finding_id")
                            .from(FindingComments::Table, FindingComments::FindingId)
                            .to(Findings::Table, Findings::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk-finding_comments-user_id")
                            .from(FindingComments::Table, FindingComments::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-finding_comments-finding_id")
                    .table(FindingComments::Table)
                    .col(FindingComments::FindingId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx-finding_comments-pid")
                    .table(FindingComments::Table)
                    .col(FindingComments::Pid)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(FindingComments::Table).to_owned())
            .await?;
        Ok(())
    }
}
