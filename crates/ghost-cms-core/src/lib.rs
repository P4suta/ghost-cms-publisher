//! ghost-core: publish Markdown to a Ghost blog via the Admin API.
//!
//! Modules:
//! - [`client`] — async Admin API client, generic over [`transport::HttpTransport`].
//! - [`frontmatter`] / [`markdown`] — parse `blog/posts/*.md` into typed front matter plus GFM HTML.
//! - [`domain`] — request/response DTOs.
//! - [`publish`] — idempotent create-or-update orchestration.
//!
//! Auth uses a Staff Access Token (works on Ghost Pro, where custom
//! integrations are unavailable).

#![allow(
    clippy::redundant_pub_crate,
    reason = "pub(crate) is the honest visibility for items in private modules; this nursery lint conflicts with rustc's unreachable_pub"
)]

pub mod client;
pub mod config;
pub mod constants;
pub mod domain;
pub mod error;
pub mod frontmatter;
pub mod markdown;
pub mod media;
pub mod publish;
pub mod transport;

pub(crate) mod auth;
pub(crate) mod hash;
pub(crate) mod images;

pub use client::Ghost;
pub use config::Config;
pub use error::{ApiError, CoreError, Operation, Resource, Result};
pub use publish::{PlannedAction, PublishOptions, PublishOutcome, publish_file, publish_post};
