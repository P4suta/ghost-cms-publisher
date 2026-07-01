//! `upload` / `upload-image` — upload a file and print its CDN URL.

use std::path::PathBuf;

use ghost_cms_shared::media::UploadKind;
use ghost_cms_shared::upload::upload;

use crate::command::Command;
use crate::ctx::Ctx;
use crate::error::Friendly;

/// Which upload endpoint to target (CLI flag mirror of [`UploadKind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum UploadKindArg {
    /// Route by file extension.
    Auto,
    /// Force the images endpoint.
    Image,
    /// Force the media endpoint (audio/video).
    Media,
    /// Force the files endpoint (any file).
    File,
}

impl From<UploadKindArg> for UploadKind {
    fn from(k: UploadKindArg) -> Self {
        match k {
            UploadKindArg::Auto => Self::Auto,
            UploadKindArg::Image => Self::Image,
            UploadKindArg::Media => Self::Media,
            UploadKindArg::File => Self::File,
        }
    }
}

/// Upload an image, media file, or arbitrary file by kind.
#[derive(Debug, clap::Args)]
pub(crate) struct UploadArgs {
    /// Path to the file.
    #[arg(value_hint = clap::ValueHint::FilePath)]
    path: PathBuf,
    /// Which endpoint to target.
    #[arg(long, value_enum, default_value_t = UploadKindArg::Auto)]
    kind: UploadKindArg,
}

impl Command for UploadArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let sp = ctx.spinner("uploading…");
        let url = upload(&client, &self.path, self.kind.into())
            .await
            .friendly();
        sp.finish_and_clear();
        println!("{}", url?);
        Ok(())
    }
}

/// Upload an image file and print its CDN URL.
#[derive(Debug, clap::Args)]
pub(crate) struct UploadImageArgs {
    /// Path to the image.
    #[arg(value_hint = clap::ValueHint::FilePath)]
    path: PathBuf,
}

impl Command for UploadImageArgs {
    async fn run(self, ctx: &Ctx) -> miette::Result<()> {
        let client = ctx.client()?;
        let sp = ctx.spinner("uploading…");
        let url = upload(&client, &self.path, UploadKind::Image)
            .await
            .friendly();
        sp.finish_and_clear();
        println!("{}", url?);
        Ok(())
    }
}
