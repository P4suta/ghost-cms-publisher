//! Shared application layer between `ghost-cms-core` and the CLI/MCP frontends.
//!
//! - [`config`] — layered configuration resolution with provenance.
//! - [`paths`] — the state-cache and relative-path conventions.
//! - [`media`] — MIME guessing and upload-endpoint detection.
//! - [`upload`] — read-and-upload orchestration.
//! - [`render`] — neutral JSON / detail / row / one-line projections.
//! - [`error`] — one frontend error type and its diagnosis.
//! - [`tag`] — tag upsert assembly.
//! - [`text`] — slugify and token masking.

pub mod config;
pub mod error;
pub mod media;
pub mod paths;
pub mod render;
pub mod tag;
pub mod text;
pub mod upload;
