//! `whoami` — validate the token and identify the site.

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;
use crate::output::SiteView;

/// Validate the Staff Access Token and print the site it points at.
#[derive(Debug, clap::Args)]
pub(crate) struct WhoamiArgs {}

impl Command for WhoamiArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let sp = ctx.spinner("checking token…");
        let site = client.site().get().await.friendly();
        sp.finish_and_clear();
        ctx.emit(&SiteView(&site?));
        Ok(())
    }
}
