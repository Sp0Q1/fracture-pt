#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::doc_markdown,
    clippy::field_reassign_with_default
)]

pub mod app;
pub mod controllers;
pub mod initializers;
pub mod mailers;
pub mod models;
pub mod services;
pub mod views;
pub mod workers;

pub use fracture_core::{require_role, require_user};
