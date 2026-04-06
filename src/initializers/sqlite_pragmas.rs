use async_trait::async_trait;
use axum::Router as AxumRouter;
use loco_rs::{
    app::{AppContext, Initializer},
    Result,
};
use sea_orm::ConnectionTrait;

/// Initializer that sets SQLite performance pragmas on startup.
///
/// - WAL mode: allows concurrent readers during writes
/// - busy_timeout: wait up to 5s instead of failing immediately on lock contention
/// - synchronous=NORMAL: safe with WAL, better write performance
pub struct SqlitePragmasInitializer;

#[async_trait]
impl Initializer for SqlitePragmasInitializer {
    fn name(&self) -> String {
        "sqlite-pragmas".to_string()
    }

    async fn before_run(&self, _app_context: &AppContext) -> Result<()> {
        Ok(())
    }

    async fn after_routes(&self, router: AxumRouter, ctx: &AppContext) -> Result<AxumRouter> {
        // Only apply to SQLite databases
        let db_url = ctx.config.database.uri.as_str();
        if db_url.starts_with("sqlite") {
            let pragmas = [
                "PRAGMA journal_mode=WAL",
                "PRAGMA busy_timeout=5000",
                "PRAGMA synchronous=NORMAL",
            ];
            for pragma in &pragmas {
                if let Err(e) = ctx.db.execute_unprepared(pragma).await {
                    tracing::warn!(pragma = %pragma, error = %e, "Failed to set SQLite pragma");
                } else {
                    tracing::info!(pragma = %pragma, "SQLite pragma set");
                }
            }
        }
        Ok(router)
    }
}
