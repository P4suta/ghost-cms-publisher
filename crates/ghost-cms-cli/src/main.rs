//! ghost-cms: a thoroughly ergonomic CLI for publishing Markdown to Ghost.
//!
//! This binary is a user-facing CLI, so writing to stdout is the whole point —
//! the print lints are relaxed here (but kept strict in the libraries).
#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "the CLI's job is to render results to the user's terminal"
)]
#![allow(
    clippy::redundant_pub_crate,
    reason = "binary crate: pub(crate) is the honest visibility, and this nursery lint directly conflicts with rustc's unreachable_pub"
)]

mod cli;
mod command;
mod commands;
mod ctx;
mod editor;
mod error;
mod output;
mod pick;
mod ui;

use clap::Parser;
use ghost_cms_shared::config::{self, Overrides};

use crate::cli::{Cli, Command};
use crate::command::Command as _;
use crate::ctx::Ctx;
use crate::output::apply_color;

fn init_tracing(verbose: u8, quiet: bool) {
    let fallback = if quiet {
        "error"
    } else {
        match verbose {
            0 => "warn",
            1 => "info",
            2 => "debug",
            _ => "trace",
        }
    };
    let filter = tracing_subscriber::EnvFilter::try_from_env("GHOST_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(fallback));
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .try_init();
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    let cli = Cli::parse();
    apply_color(cli.color.into());
    init_tracing(cli.verbose, cli.quiet);

    let overrides = Overrides {
        api_url: cli.api_url,
        token: cli.token,
        accept_version: cli.accept_version,
        blog_dir: cli.blog_dir,
    };
    let json = cli.json;
    let quiet = cli.quiet;

    // Completions/man need the clap factory, not config or a client.
    match cli.command {
        Command::Completions(args) => {
            commands::meta::print_completions::<Cli>(args.shell);
            Ok(())
        },
        Command::Man(args) => commands::meta::generate_man::<Cli>(&args.out_dir),
        command => {
            let ctx = Ctx {
                json,
                quiet,
                settings: config::resolve(overrides),
            };
            dispatch(command, &ctx).await
        },
    }
}

/// Dispatch a non-meta command against the resolved context.
async fn dispatch(command: Command, ctx: &Ctx) -> miette::Result<()> {
    match command {
        Command::Init(a) => a.run(ctx).await,
        Command::Login(a) => a.run(ctx).await,
        Command::Doctor(a) => a.run(ctx).await,
        Command::Whoami(a) => a.run(ctx).await,
        Command::New(a) => a.run(ctx).await,
        Command::Publish(a) => a.run(ctx).await,
        Command::List(a) => a.run(ctx).await,
        Command::Get(a) => a.run(ctx).await,
        Command::Open(a) => a.run(ctx).await,
        Command::Edit(a) => a.run(ctx).await,
        Command::Delete(a) => a.run(ctx).await,
        Command::UploadImage(a) => a.run(ctx).await,
        Command::Upload(a) => a.run(ctx).await,
        Command::Tags(a) => a.run(ctx).await,
        Command::Watch(a) => a.run(ctx).await,
        Command::Completions(_) | Command::Man(_) => {
            unreachable!("meta commands are handled before dispatch")
        },
    }
}
