//! `doctor` — diagnose configuration and connectivity.

use ghost_cms_shared::error::Error;
use ghost_cms_shared::text::mask_token;
use owo_colors::{OwoColorize, Stream};

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::to_report;

/// Diagnose configuration and connectivity.
#[derive(Debug, clap::Args)]
pub(crate) struct DoctorArgs {}

impl Command for DoctorArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let s = &ctx.settings;

        println!("configuration");
        let cfg_path = s
            .config_path
            .as_ref()
            .map_or_else(|| "(none)".to_owned(), |p| p.display().to_string());
        println!("    config file     {cfg_path}");
        check(
            s.api_url.is_some(),
            "site URL",
            &format!(
                "{} [{}]",
                s.api_url.as_deref().unwrap_or("(unset)"),
                s.api_url_source
            ),
        );
        check(
            s.token.is_some(),
            "token",
            &format!("{} [{}]", mask_token(s.token.as_deref()), s.token_source),
        );
        println!(
            "    accept-version  {} [{}]",
            s.accept_version, s.accept_version_source
        );
        let blog_ok = s.blog_dir.is_dir();
        check(
            blog_ok,
            "blog dir",
            &format!(
                "{} ({})",
                s.blog_dir.display(),
                if blog_ok { "exists" } else { "missing" }
            ),
        );

        println!();
        println!("connectivity");
        connectivity(ctx).await;
        Ok(())
    }
}

/// A green ✓ or red ✗ check mark.
fn mark(ok: bool) -> String {
    let glyph = if ok { "✓" } else { "✗" };
    if ok {
        glyph
            .if_supports_color(Stream::Stdout, |t| t.green().to_string())
            .to_string()
    } else {
        glyph
            .if_supports_color(Stream::Stdout, |t| t.red().to_string())
            .to_string()
    }
}

/// Print one checklist line.
fn check(ok: bool, label: &str, detail: &str) {
    println!("  {} {label:<15} {detail}", mark(ok));
}

async fn connectivity(ctx: &Ctx) {
    if ctx.settings.api_url.is_none() || ctx.settings.token.is_none() {
        check(
            false,
            "admin api",
            "skipped — run `ghost-cms init` to configure URL + token",
        );
        return;
    }
    let client = match ctx.client() {
        Ok(c) => c,
        Err(e) => {
            check(false, "admin api", &e.to_string());
            return;
        },
    };
    let sp = ctx.spinner("contacting Ghost…");
    let res = client.site().get().await;
    sp.finish_and_clear();
    match res {
        Ok(site) => check(true, "admin api", &format!("{} ({})", site.title, site.url)),
        Err(e) => check(false, "admin api", &to_report(Error::Core(e)).to_string()),
    }
}
