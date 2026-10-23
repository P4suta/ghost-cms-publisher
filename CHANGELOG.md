# Changelog

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning
follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Maintained by
[release-plz](https://release-plz.dev/) from Conventional Commit messages.

## [Unreleased]

### Added

- Offline `check` and `preview` commands for complete Markdown collections.
- Visible captions from Markdown image text and media labels, native video/audio players, and paired video posters.
- Bookmark cards from standalone descriptive links, with authored descriptions and ordinary-link fallbacks.
- Shared validation for article links, heading fragments, local media, duplicate slugs, and publication metadata before network operations.
- Source-bound policy proofs and a disposable Ghost publication rehearsal with content and media readback.

### Changed

- Require English slugs with lowercase letters, digits, single internal hyphens, and at most 64 bytes.
- Preserve heading identifiers, tables, and code through a Ghost HTML card, with local Markdown as the editable source.
- Replace the YAML parser with deserialize-only serde-saphyr and remove unused Comrak features.
- Scope publication receipts and uploaded media to each destination, write state atomically, and refuse simultaneous state writes.

### Fixed

- Update the bundled TLS trust store to webpki-roots 1.0.10 for RUSTSEC-2026-0381.
- Update `rmcp` and its macros to 2.2.0 for the disclosed HTTP client transport issue.
- Refuse invalid batches before the first network request and remote edits before uploads.
- Reuse completed media uploads after a failed post write and skip unchanged local media.
- Preserve quoted multilingual scaffold metadata and refuse escaping or existing destination paths.
- Apply the Starter-compatible upload ceiling as 5,000,000 bytes.
- Reject article route templates that escape the configured site or contain control characters.
- Update affected TLS/HTTP dependencies and replace a yanked transitive release.

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
