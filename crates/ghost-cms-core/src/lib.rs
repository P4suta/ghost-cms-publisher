//! ghost-core: publish Markdown to a Ghost blog via the Admin API.
//!
//! The crate is split into small, independently testable modules:
//!
//! - [`client`] — an async client over the Ghost Admin API, generic over an
//!   [`transport::HttpTransport`] so it can be unit-tested without a network.
//! - [`frontmatter`] / [`markdown`] — parse `blog/posts/*.md` into a typed
//!   front matter plus GFM-rendered HTML.
//! - [`domain`] — the typed request/response DTOs.
//! - [`publish`] — the idempotent create-or-update orchestration.
//!
//! Token parsing/JWT signing, content hashing, and image uploading are internal
//! implementation details. Authentication uses a Staff Access Token rather than
//! a Custom Integration, so it works on Ghost Pro plans where custom
//! integrations are unavailable.

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
