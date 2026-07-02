# ghost-cms-publisher

[![CI](https://github.com/P4suta/ghost-cms-publisher/actions/workflows/ci.yml/badge.svg)](https://github.com/P4suta/ghost-cms-publisher/actions/workflows/ci.yml)
[![CodeQL](https://github.com/P4suta/ghost-cms-publisher/actions/workflows/codeql.yml/badge.svg)](https://github.com/P4suta/ghost-cms-publisher/actions/workflows/codeql.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![MSRV](https://img.shields.io/badge/rust-1.95%2B-orange.svg)](rust-toolchain.toml)

Publish Markdown to a [Ghost](https://ghost.org/) blog from your terminal — or
from an AI assistant over the [Model Context Protocol](https://modelcontextprotocol.io/).

It authenticates with a **Staff Access Token**, so it works even on Ghost(Pro)
plans where Custom Integrations (and therefore Content/Admin API keys) are not
available. The same token drives the full Admin API: create, update, publish,
list, delete, and image upload.

- **`ghost-cms`** — a CLI that turns a `frontmatter + Markdown` file into a post,
  idempotently (look up by slug, then create or update with `updated_at`
  conflict detection).
- **`ghost-cms-mcp`** — a thin stdio MCP server exposing the same operations as
  tools, so an assistant can publish on request.

Both share one library (`ghost-cms-core`); the binaries are thin I/O adapters.

## How it authenticates

Ghost's Admin API takes a short-lived JWT (HS256). The signing key is the
**hex-decoded** secret half of a `{id}:{secret}` Staff Access Token; the `id`
becomes the JWT `kid`, and the payload fixes `aud = "/admin/"` with a 5-minute
expiry. `ghost-cms-core` mints a fresh JWT per request — you only ever handle
the token string.

Get a token from **Ghost Admin → Settings → Staff → (your user) → Staff Access
Token**. It is equivalent to your account; treat it like a password.

## Install

### Download a prebuilt binary

Each [release](https://github.com/P4suta/ghost-cms-publisher/releases) ships an
archive per platform:

| Platform            | Archive                                                        |
| ------------------- | ------------------------------------------------------------- |
| Linux (x86-64)      | `ghost-cms-publisher-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`  |
| macOS (Apple Silicon) | `ghost-cms-publisher-vX.Y.Z-aarch64-apple-darwin.tar.gz`    |
| Windows (x86-64)    | `ghost-cms-publisher-vX.Y.Z-x86_64-pc-windows-msvc.zip`       |

Each archive contains both binaries (`ghost-cms` and `ghost-cms-mcp`), shell
completions, and man pages. Verify the download against `SHA256SUMS` (attached to
the release), then extract and put the binaries on your `PATH`.

### Build from source

Requires a recent stable Rust toolchain (see `rust-toolchain.toml`; MSRV 1.95).

```sh
cargo install --path crates/ghost-cms-cli   # installs `ghost-cms`
cargo install --path crates/ghost-cms-mcp   # installs `ghost-cms-mcp`
```

## Configure

The fastest path is the guided wizard:

```sh
ghost-cms init     # prompts for URL + token, validates, and saves non-secret config
```

`init` writes a `ghost-cms.toml` (URL, accept-version, blog dir — **never the
token**) and lets you choose how to provide the token. Settings resolve with the
precedence **flag > environment variable > `ghost-cms.toml` > default**, and
`ghost-cms doctor` shows exactly where each value comes from plus a connectivity
check.

Configuration values:

| Variable / key         | Required | Example                       |
| ---------------------- | -------- | ----------------------------- |
| `GHOST_ADMIN_API_URL`  | yes      | `https://your-blog.ghost.io`  |
| `GHOST_STAFF_TOKEN`    | yes      | `{id}:{secret}`               |
| `GHOST_ACCEPT_VERSION` | no       | `v5.0` (default)              |
| `GHOST_BLOG_DIR`       | no       | `blog` (default)              |

**Never commit your token.** Copy `.env.example` to `.env` (git-ignored) for
local use, or keep it in a secret manager. This repo ships a [1Password](https://1password.com/)
flow: put the values behind the `op://` references in `.env.op`, then run any
command through `op run --env-file .env.op -- …` (the `Justfile` recipes do this
for you).

## Usage

Scaffold a post (slug is derived from the title, frontmatter is pre-filled, and
the file opens in `$EDITOR`):

```sh
ghost-cms new "My first post" --tag Rust --tag Ghost
```

…or write one by hand under `blog/posts/`:

```markdown
---
title: "My first post"
slug: my-first-post
status: draft          # draft | published | scheduled
tags: [Rust, Ghost]
excerpt: "A short summary for listings."
canonical_url: https://github.com/you/your-repo/blob/main/README.md
# All of the following are optional:
featured: true
visibility: public               # public | members | paid
og_image: ../assets/social.png   # local path → uploaded, or an absolute URL
og_title: "My first post"
og_description: "What people see when it's shared."
twitter_image: ../assets/social.png
codeinjection_head: "<style>.kg-card { margin: 2rem 0; }</style>"
authors: [you@example.com]       # extra co-authors, resolved by email
---

# Hello

Body written in **GitHub-Flavored Markdown**.
```

Every key under "optional" maps to a Ghost post field; image fields (`feature_image`,
`og_image`, `twitter_image`) accept a local path that is uploaded on publish.

Then:

```sh
ghost-cms whoami                          # validate the token, print the site
ghost-cms publish blog/posts/my-first-post.md --dry-run   # plan only
ghost-cms publish blog/posts/my-first-post.md             # create/update (draft)
ghost-cms publish blog/posts/my-first-post.md --publish --open   # publish & open
ghost-cms publish blog/posts/                             # publish every *.md in a dir
ghost-cms list                            # bordered table (or --json)
ghost-cms get my-first-post --json
ghost-cms edit my-first-post              # open the local file in $EDITOR
ghost-cms open my-first-post              # open the published URL in the browser
ghost-cms upload blog/assets/clip.mp4     # auto-routes image/media/file by type
ghost-cms upload doc.pdf --kind file
ghost-cms watch                           # auto-publish posts as you save them
```

### Tags

Manage tag archive pages — set descriptions, colors, feature images, and SEO/social cards:

```sh
ghost-cms tags list
ghost-cms tags get rust
ghost-cms tags set rust --description "Posts about Rust." \
  --accent-color '#CE412B' --feature-image blog/assets/rust.png
ghost-cms tags delete obsolete --yes
```

`tags set` is an upsert by slug; image flags accept a local path (uploaded) or a URL.

Publishing is **idempotent**: the slug is the key. Re-running with unchanged
content is a no-op; a changed body updates the existing post. Local image paths
in the body and `feature_image` are uploaded and rewritten to the Ghost CDN URL.

Global flags work on every command: `--json` (machine-readable output),
`--quiet`, `--verbose` (repeatable), and `--color auto|always|never` (also
honors `NO_COLOR` and non-TTY pipes). Invalid frontmatter is reported with the
exact file, line, and column underlined.

### Shell completions & man pages

```sh
ghost-cms completions powershell  > ghost-cms.ps1   # or bash/zsh/fish/elvish
ghost-cms man ./man                                 # render man pages
```

### Just recipes

If you have [`just`](https://github.com/casey/just) and the 1Password CLI:

```sh
just whoami
just preview blog/posts/my-first-post.md   # dry run
just publish blog/posts/my-first-post.md
just blog-list
```

## Use from an AI assistant (MCP)

`ghost-cms-mcp` speaks MCP over stdio. Register it (example for Claude Code) via
`.mcp.json` — see the committed template, which injects secrets through
`op run` so nothing sensitive lives in the repo. Exposed tools:

- `ghost_whoami`
- `ghost_publish_markdown` (by file path; honors all extended frontmatter)
- `ghost_publish_inline` (title + slug + Markdown, no file)
- `ghost_list_posts`
- `ghost_get_post`
- `ghost_list_tags`
- `ghost_set_tag` (upsert tag metadata by slug)
- `ghost_upload_image`
- `ghost_upload` (image / media / file, auto-routed)

`stdout` is reserved for the JSON-RPC channel; all logs go to `stderr`. Deletion
is intentionally **not** exposed over MCP — destructive actions stay in the CLI.

## Notes & limits

- Markdown is rendered to HTML and sent with `?source=html`; Ghost converts it
  to its native Lexical format.
- Ghost(Pro) **Starter** caps request bodies at 5 MB; oversized posts/images are
  rejected locally before any network call.
- The Content API is not used (its key needs a Custom Integration); reads go
  through the Admin API instead.

## Development

```sh
just lint    # fmt-check + clippy (-D warnings) + cargo-deny + typos + machete
just test    # unit + wiremock integration tests
```

Library code returns `Result`; binaries own the `anyhow`/exit boundary. See
[CONTRIBUTING.md](CONTRIBUTING.md) for setup, the full quality gates, snapshot
tests, and the release flow.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
