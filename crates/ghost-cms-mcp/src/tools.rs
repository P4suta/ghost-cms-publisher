//! The Ghost MCP tools — thin adapters over the shared application layer.

use ghost_cms_core::domain::PostStatus;
use ghost_cms_core::frontmatter::FrontMatter;
use ghost_cms_core::publish::{self, PublishOptions};
use ghost_cms_shared::error::require_post_by_slug;
use ghost_cms_shared::media::UploadKind;
use ghost_cms_shared::tag::{TagMeta, build_upsert};
use ghost_cms_shared::{paths, render, upload};
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, tool, tool_router};

use crate::args::{
    ListArgs, PublishInlineArgs, PublishMarkdownArgs, SetTagArgs, SlugArgs, UploadAnyArgs,
    UploadArgs,
};
use crate::error::{IntoMcp, parse_status};
use crate::server::GhostServer;

/// Join non-empty lines, or a placeholder when there are none.
fn lines_or(lines: &[String], empty: &str) -> String {
    if lines.is_empty() {
        empty.to_owned()
    } else {
        lines.join("\n")
    }
}

// `vis` makes the generated `tool_router()` reachable from the `#[tool_handler]`
// impl in server.rs (a different module in this crate).
#[tool_router(vis = "pub(crate)")]
impl GhostServer {
    /// Build a server from a client and blog directory.
    pub(crate) fn new(client: ghost_cms_core::Ghost, blog_dir: std::path::PathBuf) -> Self {
        Self {
            client: std::sync::Arc::new(client),
            blog_dir,
        }
    }

    #[tool(description = "Validate the Ghost staff token and return site title, URL and version.")]
    async fn ghost_whoami(&self) -> Result<CallToolResult, ErrorData> {
        let site = self.client.site().get().await.mcp()?;
        let version = site.version.as_deref().unwrap_or("unknown");
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "{} ({}) [Ghost {}]",
            site.title, site.url, version
        ))]))
    }

    #[tool(
        description = "Idempotently publish/update a Ghost post from a Markdown file (frontmatter-driven). Looks up by slug; creates or updates with updated_at conflict detection."
    )]
    async fn ghost_publish_markdown(
        &self,
        Parameters(args): Parameters<PublishMarkdownArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let opts = PublishOptions {
            dry_run: args.dry_run,
            force: args.force,
            status_override: parse_status(args.status.as_deref())?,
            allow_raw_html: false,
            state_path: Some(paths::state_path(&self.blog_dir)),
        };
        let file = paths::resolve(&self.blog_dir, &args.file);
        let outcome = publish::publish_file(self.client.as_ref(), &file, &opts)
            .await
            .mcp()?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            render::outcome_line(&outcome),
        )]))
    }

    #[tool(
        description = "Publish/update a Ghost post from inline Markdown (no file). Idempotent by slug."
    )]
    async fn ghost_publish_inline(
        &self,
        Parameters(args): Parameters<PublishInlineArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let status = parse_status(args.status.as_deref())?.unwrap_or(PostStatus::Draft);
        let mut front = FrontMatter::new(args.title, args.slug);
        front.status = status;
        front.tags = args.tags;
        let opts = PublishOptions {
            dry_run: args.dry_run,
            state_path: Some(paths::state_path(&self.blog_dir)),
            ..PublishOptions::default()
        };
        let outcome = publish::publish_post(
            self.client.as_ref(),
            front,
            &args.markdown,
            &self.blog_dir,
            &opts,
        )
        .await
        .mcp()?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            render::outcome_line(&outcome),
        )]))
    }

    #[tool(description = "List recent posts (status, slug, updated_at, title).")]
    async fn ghost_list_posts(
        &self,
        Parameters(args): Parameters<ListArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let posts = self
            .client
            .posts()
            .list(args.limit, args.page)
            .await
            .mcp()?;
        let lines: Vec<String> = render::post_rows(&posts)
            .iter()
            .map(|r| {
                format!(
                    "{:<9} {:<28} {}  {}",
                    r.status.to_string(),
                    r.slug,
                    r.updated,
                    r.title
                )
            })
            .collect();
        Ok(CallToolResult::success(vec![ContentBlock::text(lines_or(
            &lines,
            "(no posts)",
        ))]))
    }

    #[tool(description = "Get one post by slug; returns metadata and HTML as JSON.")]
    async fn ghost_get_post(
        &self,
        Parameters(args): Parameters<SlugArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let post = require_post_by_slug(self.client.as_ref(), &args.slug)
            .await
            .mcp()?;
        Ok(CallToolResult::success(vec![ContentBlock::text(
            render::post_value(&post).to_string(),
        )]))
    }

    #[tool(description = "Upload an image file and return its Ghost CDN URL.")]
    async fn ghost_upload_image(
        &self,
        Parameters(args): Parameters<UploadArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = paths::resolve(&self.blog_dir, &args.path);
        let url = upload::upload(self.client.as_ref(), &path, UploadKind::Image)
            .await
            .mcp()?;
        Ok(CallToolResult::success(vec![ContentBlock::text(url)]))
    }

    #[tool(
        description = "Upload an image, media (audio/video), or arbitrary file. kind: auto|image|media|file. Returns the CDN URL."
    )]
    async fn ghost_upload(
        &self,
        Parameters(args): Parameters<UploadAnyArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let path = paths::resolve(&self.blog_dir, &args.path);
        let kind = UploadKind::parse(args.kind.as_deref().unwrap_or("auto"));
        let url = upload::upload(self.client.as_ref(), &path, kind)
            .await
            .mcp()?;
        Ok(CallToolResult::success(vec![ContentBlock::text(url)]))
    }

    #[tool(description = "List tags with post counts.")]
    async fn ghost_list_tags(
        &self,
        Parameters(args): Parameters<ListArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let tags = self.client.tags().list(args.limit, args.page).await.mcp()?;
        let lines: Vec<String> = render::tag_rows(&tags)
            .iter()
            .map(|r| format!("{:<24} {} (posts: {})", r.slug, r.name, r.posts))
            .collect();
        Ok(CallToolResult::success(vec![ContentBlock::text(lines_or(
            &lines,
            "(no tags)",
        ))]))
    }

    #[tool(
        description = "Create or update a tag's metadata (upsert by slug). Image fields accept local paths or URLs."
    )]
    async fn ghost_set_tag(
        &self,
        Parameters(args): Parameters<SetTagArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let existing = self.client.tags().find_by_slug(&args.slug).await.mcp()?;
        let base = &self.blog_dir;
        let feature_image = upload::upload_if_local(self.client.as_ref(), base, args.feature_image)
            .await
            .mcp()?;
        let og_image = upload::upload_if_local(self.client.as_ref(), base, args.og_image)
            .await
            .mcp()?;
        let twitter_image = upload::upload_if_local(self.client.as_ref(), base, args.twitter_image)
            .await
            .mcp()?;

        let meta = TagMeta {
            name: args.name,
            description: args.description,
            feature_image,
            accent_color: args.accent_color,
            visibility: args.visibility,
            canonical_url: args.canonical_url,
            meta_title: args.meta_title,
            meta_description: args.meta_description,
            og_image,
            og_title: args.og_title,
            og_description: args.og_description,
            twitter_image,
            twitter_title: args.twitter_title,
            twitter_description: args.twitter_description,
            codeinjection_head: None,
            codeinjection_foot: None,
        };
        let input =
            build_upsert(&args.slug, meta, existing.as_ref().map(|t| t.name.as_str())).mcp()?;

        let tag = match &existing {
            Some(t) => {
                let updated_at = t.updated_at.clone().ok_or_else(|| {
                    ErrorData::internal_error("existing tag is missing updated_at".to_owned(), None)
                })?;
                self.client.tags().update(&t.id, &input, &updated_at).await
            },
            None => self.client.tags().create(&input).await,
        }
        .mcp()?;

        let verb = if existing.is_some() {
            "updated"
        } else {
            "created"
        };
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "{verb} tag '{}'",
            tag.slug
        ))]))
    }
}
