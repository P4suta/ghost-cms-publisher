# ghost-cms-publisher task entry points.
#
# Recipes that talk to Ghost are wrapped in `op run` so secrets come from
# 1Password (see .env.op) and never touch the repo. Dev recipes need no secrets.

# Inject GHOST_* from 1Password references in .env.op.
set positional-arguments
set shell := ["sh", "-cu"]

op := "op run --env-file .env.op --"

default:
    @just --list

# ----- build / test -----

build:
    cargo build --workspace --all-targets --locked

test:
    cargo nextest run --workspace --locked
    cargo test --doc --workspace --locked

# ----- quality gates -----

fmt:
    cargo fmt --all
    cargo sort --workspace --grouped
    taplo fmt
    typos --write-changes

lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo deny --offline --locked check bans licenses sources
    typos
    cargo machete
    cargo run -q --locked -p ghost-cms-xtask -- verify-evidence

clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# ----- authoring (local; no secrets) -----

# Scaffold a new post: just new "My Title" --tag Rust
new TITLE *ARGS:
    cargo run -q --locked -p ghost-cms-cli -- new "$@"

# Print a shell completion script: just completions powershell
completions SHELL:
    cargo run -q --locked -p ghost-cms-cli -- completions "$@"

# Generate man pages into a directory.
man OUT="man":
    cargo run -q --locked -p ghost-cms-cli -- man "$@"

# ----- Ghost operations (secrets via 1Password) -----

# Validate the token and print the site it points at.
whoami:
    {{ op }} cargo run -q -p ghost-cms-cli -- whoami

# Diagnose configuration and connectivity.
doctor:
    {{ op }} cargo run -q -p ghost-cms-cli -- doctor

# Check a collection without credentials or network access.
check *FILES:
    cargo run -q --locked -p ghost-cms-cli -- check "$@"

# Render a collection to a fresh local preview directory.
preview FILE OUTPUT:
    cargo run -q --locked -p ghost-cms-cli -- preview "$1" --output "$2"

# Check against the existing local RustSec advisory snapshot.
audit:
    cargo deny --offline --locked check advisories

# Rehearse publication and readback in a disposable Ghost container.
rehearse FILE *ARGS:
    cargo run -q --locked -p ghost-cms-xtask -- rehearse "$@"

# Prove publication decisions and English slug invariants in the bounded image.
proofs:
    dotfiles bounded run --image dotfiles-proofs@sha256:8e8c3716a29370da1603df5251d7db4ea1e28479ef96265ae5ee9ce0cc46a42a --project . -- cargo kani --manifest-path crates/ghost-cms-policy/Cargo.toml --output-format terse

# Create or update a post (pass extra flags after the file, e.g. --publish).
publish FILE *ARGS:
    {{ op }} cargo run -q --locked -p ghost-cms-cli -- publish "$@"

blog-list LIMIT="20":
    {{ op }} cargo run -q --locked -p ghost-cms-cli -- list --limit "$1"

get SLUG:
    {{ op }} cargo run -q --locked -p ghost-cms-cli -- get "$@"

# Open a post's URL in the browser.
open SLUG:
    {{ op }} cargo run -q --locked -p ghost-cms-cli -- open "$@"

upload-image PATH:
    {{ op }} cargo run -q --locked -p ghost-cms-cli -- upload-image "$@"

# Upload an image/media/file (auto-routed by type).
upload PATH *ARGS:
    {{ op }} cargo run -q --locked -p ghost-cms-cli -- upload "$@"

# Tag management: just tags list / tags set rust --description "…"
tags *ARGS:
    {{ op }} cargo run -q --locked -p ghost-cms-cli -- tags "$@"

# Watch blog/posts and auto-publish on save.
watch:
    {{ op }} cargo run -q -p ghost-cms-cli -- watch

# Run the MCP server locally (Claude Code launches it itself via .mcp.json).
mcp:
    {{ op }} cargo run -q -p ghost-cms-mcp

# ----- setup -----

# Install the CLI and MCP binaries onto PATH.
install:
    cargo install --path crates/ghost-cms-cli
    cargo install --path crates/ghost-cms-mcp

# Install git hooks.
hooks:
    lefthook install
