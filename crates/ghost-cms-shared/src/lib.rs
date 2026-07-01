//! Shared application layer for the ghost-cms frontends.
//!
//! `ghost-cms-core` owns the Ghost protocol and pure domain; the CLI and MCP
//! frontends own presentation (color, tables, miette, MCP error codes). This
//! crate sits between them and holds everything they would otherwise duplicate:
//!
//! - [`config`] — layered configuration resolution with provenance.
//! - [`paths`] — the state-cache and relative-path conventions.
//! - [`media`] — MIME guessing and upload-endpoint detection.
//! - [`upload`] — read-and-upload orchestration.
//! - [`render`] — neutral JSON / detail / row / one-line projections.
//! - [`error`] — one frontend error type and its diagnosis.
//! - [`tag`] — tag upsert assembly.
//! - [`text`] — slugify and token masking.
//!
//! It depends only on `ghost-cms-core`; it pulls in no UI, miette, or MCP types.

pub mod config;
pub mod error;
pub mod media;
pub mod paths;
pub mod render;
pub mod tag;
pub mod text;
pub mod upload;
