/// Legacy scan_jobs model — kept only for schema compatibility.
/// The scan_jobs table is replaced by job_definitions + job_runs.
pub use super::_entities::scan_jobs::{ActiveModel, Entity};

impl sea_orm::ActiveModelBehavior for ActiveModel {}
