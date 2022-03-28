//! Iris tools library — migrate / project helpers for CLI and embedders (e.g. farm-migrate).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod migrate;
pub mod project;

pub use iris::{collect_schema_paths, load_schema_document, table_name_class_hints};
pub use migrate::{ApplyReport, migrate_apply, migrate_plan, migrate_run, migrate_verify};
pub use project::{load_project, load_project_optional, load_project_required, read_schema};
