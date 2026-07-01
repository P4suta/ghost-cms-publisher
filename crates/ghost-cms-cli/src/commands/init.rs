//! `init` / `login` — guided setup that validates and saves configuration.

use std::path::PathBuf;

use ghost_cms_core::{Config, Ghost};
use ghost_cms_shared::config::{CONFIG_FILE, FileConfig, write_config};
use miette::IntoDiagnostic;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;

/// Guided first-time setup (URL + token), validated and saved.
#[derive(Debug, clap::Args)]
pub(crate) struct InitArgs {}

impl Command for InitArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        wizard(ctx, false).await
    }
}

/// Set or update the Staff Access Token.
#[derive(Debug, clap::Args)]
pub(crate) struct LoginArgs {}

impl Command for LoginArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        wizard(ctx, true).await
    }
}

/// Run the wizard. With `token_only` (the `login` command) it keeps the existing
/// site URL and only (re)captures the token.
async fn wizard(ctx: &Ctx, token_only: bool) -> miette::Result<()> {
    let api_url = resolve_url(ctx, token_only)?;
    let token = inquire::Password::new("Staff Access Token (id:secret)")
        .without_confirmation()
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .prompt()
        .into_diagnostic()?;

    validate(ctx, &api_url, &token).await?;

    if !token_only {
        save_site_config(ctx, &api_url)?;
    }
    choose_token_storage(ctx, &api_url, &token)
}

fn resolve_url(ctx: &Ctx, token_only: bool) -> miette::Result<String> {
    if token_only {
        return ctx.settings.api_url.clone().ok_or_else(|| {
            miette::miette!(
                help = "Run `ghost-cms init` first.",
                "no site URL configured"
            )
        });
    }
    let default = ctx.settings.api_url.clone().unwrap_or_default();
    let url = inquire::Text::new("Ghost site URL")
        .with_initial_value(&default)
        .prompt()
        .into_diagnostic()?;
    Ok(url.trim().to_owned())
}

async fn validate(ctx: &Ctx, url: &str, token: &str) -> miette::Result<()> {
    let cfg = Config::new(
        url.to_owned(),
        token.to_owned(),
        ctx.settings.accept_version.clone(),
    );
    let client = Ghost::new(&cfg).friendly()?;
    let sp = ctx.spinner("validating…");
    let site = client.site().get().await.friendly();
    sp.finish_and_clear();
    let site = site?;
    ctx.success(&format!("authenticated to {} ({})", site.title, site.url));
    Ok(())
}

fn save_site_config(ctx: &Ctx, url: &str) -> miette::Result<()> {
    let path = PathBuf::from(CONFIG_FILE);
    let cfg = FileConfig {
        api_url: Some(url.to_owned()),
        accept_version: Some(ctx.settings.accept_version.clone()),
        blog_dir: Some(ctx.settings.blog_dir.display().to_string()),
    };
    write_config(&path, &cfg).into_diagnostic()?;
    ctx.success(&format!("saved {}", path.display()));
    Ok(())
}

/// Where the user wants the freshly-captured token to live.
#[derive(Debug, Clone, Copy)]
enum TokenStorage {
    /// Keep it in a secret manager / shell environment (recommended).
    EnvOrManager,
    /// Persist it to a git-ignored `.env` file.
    LocalEnv,
}

impl std::fmt::Display for TokenStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::EnvOrManager => "Use 1Password / env yourself (recommended)",
            Self::LocalEnv => "Save to a local .env (plaintext, git-ignored)",
        };
        f.write_str(text)
    }
}

fn choose_token_storage(ctx: &Ctx, url: &str, token: &str) -> miette::Result<()> {
    let choice = inquire::Select::new(
        "Where should the token live?",
        vec![TokenStorage::EnvOrManager, TokenStorage::LocalEnv],
    )
    .prompt()
    .into_diagnostic()?;

    match choice {
        TokenStorage::LocalEnv => {
            std::fs::write(
                ".env",
                format!("GHOST_ADMIN_API_URL={url}\nGHOST_STAFF_TOKEN={token}\n"),
            )
            .into_diagnostic()?;
            ctx.success(".env written — make sure it is git-ignored");
        },
        TokenStorage::EnvOrManager => {
            ctx.note("Provide the token via your shell or 1Password, e.g.:");
            println!("  export GHOST_ADMIN_API_URL='{url}'");
            println!("  export GHOST_STAFF_TOKEN='<id:secret>'");
        },
    }
    Ok(())
}
