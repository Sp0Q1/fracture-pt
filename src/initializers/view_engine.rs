use std::collections::HashMap;

use async_trait::async_trait;
use axum::{Extension, Router as AxumRouter};
use fluent_templates::{ArcLoader, FluentLoader};
use fracture_core::views::sri::{register_sri_function, SriIndex};
use loco_rs::{
    app::{AppContext, Initializer},
    controller::views::{engines, ViewEngine},
    Error, Result,
};
use tracing::info;

/// Tera filter: parse a JSON string into a Tera Value so templates can
/// access nested fields. Used by `jobs/org_run_show.html` to render parsed
/// per-tool result summaries (ip_enum, port_scan, etc.) without each tool
/// needing its own controller route.
fn from_json_filter(
    value: &tera::Value,
    _args: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    match value {
        tera::Value::Null => Ok(tera::Value::Null),
        tera::Value::String(s) => serde_json::from_str(s)
            .map_err(|e| tera::Error::msg(format!("from_json: invalid JSON ({e})"))),
        other => Err(tera::Error::msg(format!(
            "from_json: expected a string, got {other}"
        ))),
    }
}

const I18N_DIR: &str = "assets/i18n";
const I18N_SHARED: &str = "assets/i18n-shared.ftl";
const STATIC_DIR: &str = "assets/static";
const STATIC_URL: &str = "/static";

pub struct TemplateInitializer;

#[async_trait]
impl Initializer for TemplateInitializer {
    fn name(&self) -> String {
        "view-engine".to_string()
    }

    async fn after_routes(&self, router: AxumRouter, _ctx: &AppContext) -> Result<AxumRouter> {
        // Compute SRI hashes for every static asset once at boot. Templates
        // call `{{ sri(path='/static/foo.css') }}` instead of carrying
        // hand-maintained literals that drift out of sync with the file.
        let sri_index = SriIndex::from_directory(std::path::Path::new(STATIC_DIR), STATIC_URL)
            .map_err(|e| Error::string(&format!("SRI index build failed: {e}")))?;
        info!(entries = sri_index.len(), "SRI index built");

        let tera_engine = if std::path::Path::new(I18N_DIR).exists() {
            let arc = std::sync::Arc::new(
                ArcLoader::builder(&I18N_DIR, unic_langid::langid!("en-GB"))
                    .shared_resources(Some(&[I18N_SHARED.into()]))
                    .customize(|bundle| bundle.set_use_isolating(false))
                    .build()
                    .map_err(|e| Error::string(&e.to_string()))?,
            );
            info!("locales loaded");

            let sri = sri_index;
            engines::TeraView::build()?.post_process(move |tera| {
                tera.register_function("t", FluentLoader::new(arc.clone()));
                register_sri_function(tera, sri.clone());
                tera.register_filter("from_json", from_json_filter);
                fracture_core::register_templates(tera)
                    .map_err(|e| loco_rs::Error::string(&e.to_string()))?;
                Ok(())
            })?
        } else {
            let sri = sri_index;
            engines::TeraView::build()?.post_process(move |tera| {
                register_sri_function(tera, sri.clone());
                tera.register_filter("from_json", from_json_filter);
                fracture_core::register_templates(tera)
                    .map_err(|e| loco_rs::Error::string(&e.to_string()))?;
                Ok(())
            })?
        };

        Ok(router.layer(Extension(ViewEngine::from(tera_engine))))
    }
}
