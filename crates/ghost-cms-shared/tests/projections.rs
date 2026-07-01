//! Tests for the neutral projections and diagnosis shared by the frontends.
#![allow(clippy::unwrap_used, reason = "tests panic on failure by design")]

use ghost_cms_core::domain::{Post, PostStatus};
use ghost_cms_core::publish::PublishOutcome;
use ghost_cms_shared::config::ConfigError;
use ghost_cms_shared::error::{Error, Remediation, diagnose};
use ghost_cms_shared::render::{outcome_line, post_rows, post_value};
use ghost_cms_shared::tag::{TagMeta, build_upsert};

fn post() -> Post {
    Post {
        id: "p1".to_owned(),
        title: Some("Hello".to_owned()),
        slug: "hello".to_owned(),
        status: PostStatus::Published,
        url: Some("https://x/h".to_owned()),
        updated_at: Some("2026-06-26".to_owned()),
        html: None,
    }
}

#[test]
fn outcome_line_formats_each_variant() {
    assert_eq!(
        outcome_line(&PublishOutcome::Created {
            id: "p1".to_owned(),
            url: Some("https://x".to_owned())
        }),
        "created p1  https://x"
    );
    assert_eq!(
        outcome_line(&PublishOutcome::SkippedUnchanged {
            id: "p1".to_owned()
        }),
        "unchanged p1 (skipped)"
    );
}

#[test]
fn post_value_uses_lowercase_status() {
    let v = post_value(&post());
    assert_eq!(v["status"], "published");
    assert_eq!(v["slug"], "hello");
}

#[test]
fn post_rows_default_missing_fields() {
    let mut p = post();
    p.title = None;
    p.updated_at = None;
    let rows = post_rows(std::slice::from_ref(&p));
    assert_eq!(rows[0].title, "(untitled)");
    assert_eq!(rows[0].updated, "-");
}

#[test]
fn build_upsert_parses_visibility() {
    let meta = TagMeta {
        visibility: Some("internal".to_owned()),
        ..TagMeta::default()
    };
    let input = build_upsert("rust", meta, None).unwrap();
    assert_eq!(input.name, "rust");
    assert_eq!(
        input.visibility,
        Some(ghost_cms_core::domain::TagVisibility::Internal)
    );
}

#[test]
fn build_upsert_rejects_bad_visibility() {
    let meta = TagMeta {
        visibility: Some("bogus".to_owned()),
        ..TagMeta::default()
    };
    assert!(matches!(
        build_upsert("rust", meta, None),
        Err(Error::InvalidValue {
            field: "visibility",
            ..
        })
    ));
}

#[test]
fn diagnose_maps_categories() {
    let d = diagnose(&Error::Config(ConfigError::MissingToken));
    assert_eq!(d.remediation, Remediation::SetToken);
    assert!(d.user_error);

    let d = diagnose(&Error::NotFound {
        resource: "tag",
        slug: "x".to_owned(),
        suggestion: None,
    });
    assert_eq!(d.remediation, Remediation::ListTags);

    let d = diagnose(&Error::NotFound {
        resource: "post",
        slug: "helo".to_owned(),
        suggestion: Some("hello".to_owned()),
    });
    assert!(d.summary.contains("did you mean `hello`?"));
}
