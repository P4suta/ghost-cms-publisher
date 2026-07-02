//! Tag upsert assembly shared by the CLI `tags set` and MCP `ghost_set_tag`.

use ghost_cms_core::domain::{
    CodeInjection, OpenGraph, SeoMeta, TagUpsertInput, TagVisibility, TwitterCard,
};

use crate::error::{Error, Result};

/// Optional tag metadata gathered by a frontend.
///
/// Image fields must already be resolved to URLs (via
/// [`crate::upload::upload_if_local`]).
#[derive(Debug, Default)]
pub struct TagMeta {
    /// Display name (falls back to the existing name, then the slug).
    pub name: Option<String>,
    /// Description.
    pub description: Option<String>,
    /// Feature image URL.
    pub feature_image: Option<String>,
    /// Accent color (hex).
    pub accent_color: Option<String>,
    /// Visibility (`public`/`internal`).
    pub visibility: Option<String>,
    /// Canonical URL.
    pub canonical_url: Option<String>,
    /// SEO meta title.
    pub meta_title: Option<String>,
    /// SEO meta description.
    pub meta_description: Option<String>,
    /// Open Graph image URL.
    pub og_image: Option<String>,
    /// Open Graph title.
    pub og_title: Option<String>,
    /// Open Graph description.
    pub og_description: Option<String>,
    /// Twitter image URL.
    pub twitter_image: Option<String>,
    /// Twitter title.
    pub twitter_title: Option<String>,
    /// Twitter description.
    pub twitter_description: Option<String>,
    /// HTML injected into the tag archive `<head>`.
    pub codeinjection_head: Option<String>,
    /// HTML injected before the tag archive `</body>`.
    pub codeinjection_foot: Option<String>,
}

/// Assemble a [`TagUpsertInput`] for `slug` from `meta`.
///
/// The name falls back to `existing_name`, then the slug.
///
/// # Errors
/// Returns [`Error::InvalidValue`] if `visibility` is not `public`/`internal`.
pub fn build_upsert(
    slug: &str,
    meta: TagMeta,
    existing_name: Option<&str>,
) -> Result<TagUpsertInput> {
    let name = meta
        .name
        .or_else(|| existing_name.map(str::to_owned))
        .unwrap_or_else(|| slug.to_owned());

    let visibility = match meta.visibility {
        Some(v) => Some(
            v.parse::<TagVisibility>()
                .map_err(|()| Error::InvalidValue {
                    field: "visibility",
                    value: v,
                })?,
        ),
        None => None,
    };

    Ok(TagUpsertInput {
        name,
        slug: Some(slug.to_owned()),
        description: meta.description,
        feature_image: meta.feature_image,
        visibility,
        accent_color: meta.accent_color,
        updated_at: None,
        seo: SeoMeta {
            meta_title: meta.meta_title,
            meta_description: meta.meta_description,
            canonical_url: meta.canonical_url,
        },
        open_graph: OpenGraph {
            og_image: meta.og_image,
            og_title: meta.og_title,
            og_description: meta.og_description,
        },
        twitter: TwitterCard {
            twitter_image: meta.twitter_image,
            twitter_title: meta.twitter_title,
            twitter_description: meta.twitter_description,
        },
        code_injection: CodeInjection {
            codeinjection_head: meta.codeinjection_head,
            codeinjection_foot: meta.codeinjection_foot,
        },
    })
}
