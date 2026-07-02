# ghost-cms-publisher task entry points.
#
# Recipes that talk to Ghost are wrapped in `op run` so secrets come from
# 1Password (see .env.op) and never touch the repo. Dev recipes need no secrets.

# Inject GHOST_* from 1Password references in .env.op.
op := "op run --env-file .env.op --"

default:
    @just --list

# ----- build / test -----

build:
    cargo build --workspace --all-targets

test:
    cargo nextest run --workspace || cargo test --workspace
    cargo test --doc --workspace

# ----- quality gates -----

fmt:
    cargo fmt --all
    cargo sort --workspace --grouped
    taplo fmt
    typos --write-changes

lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo deny check advisories bans licenses sources
    typos
    cargo machete

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# ----- authoring (local; no secrets) -----

# Scaffold a new post: just new "My Title" --tag Rust
new TITLE *ARGS:
    cargo run -q -p ghost-cms-cli -- new {{TITLE}} {{ARGS}}

# Print a shell completion script: just completions powershell
completions SHELL:
    cargo run -q -p ghost-cms-cli -- completions {{SHELL}}

# Generate man pages into a directory.
man OUT="man":
    cargo run -q -p ghost-cms-cli -- man {{OUT}}

# ----- Ghost operations (secrets via 1Password) -----

# Validate the token and print the site it points at.
whoami:
    {{op}} cargo run -q -p ghost-cms-cli -- whoami

# Diagnose configuration and connectivity.
doctor:
    {{op}} cargo run -q -p ghost-cms-cli -- doctor

# Dry-run a post (no writes).
preview FILE:
    {{op}} cargo run -q -p ghost-cms-cli -- publish {{FILE}} --dry-run

# Create or update a post (pass extra flags after the file, e.g. --publish).
publish FILE *ARGS:
    {{op}} cargo run -q -p ghost-cms-cli -- publish {{FILE}} {{ARGS}}

blog-list LIMIT="20":
    {{op}} cargo run -q -p ghost-cms-cli -- list --limit {{LIMIT}}

get SLUG:
    {{op}} cargo run -q -p ghost-cms-cli -- get {{SLUG}}

# Open a post's URL in the browser.
open SLUG:
    {{op}} cargo run -q -p ghost-cms-cli -- open {{SLUG}}

upload-image PATH:
    {{op}} cargo run -q -p ghost-cms-cli -- upload-image {{PATH}}

# Upload an image/media/file (auto-routed by type).
upload PATH *ARGS:
    {{op}} cargo run -q -p ghost-cms-cli -- upload {{PATH}} {{ARGS}}

# Tag management: just tags list / tags set rust --description "…"
tags *ARGS:
    {{op}} cargo run -q -p ghost-cms-cli -- tags {{ARGS}}

# Watch blog/posts and auto-publish on save.
watch:
    {{op}} cargo run -q -p ghost-cms-cli -- watch

# Run the MCP server locally (Claude Code launches it itself via .mcp.json).
mcp:
    {{op}} cargo run -q -p ghost-cms-mcp

# ----- setup -----

# Install the CLI and MCP binaries onto PATH.
install:
    cargo install --path crates/ghost-cms-cli
    cargo install --path crates/ghost-cms-mcp

# Install git hooks.
hooks:
    lefthook install
