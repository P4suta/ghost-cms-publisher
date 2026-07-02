# Changelog

All notable changes to ghost-cms-publisher are recorded in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Releases are cut by [release-plz](https://release-plz.dev/); this file is
maintained automatically from Conventional Commit messages once the release
automation is activated.

## [Unreleased]

### Changed

- Updated dependencies to current major versions: comrak 0.52, jsonwebtoken 10
  (pure-Rust `rust_crypto` backend), rmcp 2.0, toml 1, sha2 0.11, inquire 0.9,
  and notify-debouncer-mini 0.7, plus grouped minor/patch bumps. No user-facing
  behavior changes; MSRV remains 1.95.

## [0.1.0] - 2026-07-02

### Added

- `ghost-cms` CLI: publish a `frontmatter + Markdown` file to a Ghost blog,
  idempotently by slug (create / update / skip) with `updated_at` conflict
  detection, plus `list`, `get`, `new`, `watch`, `tags`, `upload`, `doctor`,
  `whoami`, `open`, `init`, `completions`, and `man`.
- `ghost-cms-mcp`: a stdio Model Context Protocol server exposing the same
  operations (except delete) as tools.
- Authentication via a Ghost **Staff Access Token**, minting a short-lived
  per-request Admin API JWT — works on Ghost(Pro) plans without Custom
  Integrations.
- Automatic local image upload with CDN URL rewriting, and GFM-to-HTML
  rendering.

[Unreleased]: https://github.com/P4suta/ghost-cms-publisher/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/P4suta/ghost-cms-publisher/releases/tag/v0.1.0
