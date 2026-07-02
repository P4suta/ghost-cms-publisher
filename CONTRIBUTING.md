# Contributing

Local setup, quality gates, and how releases work.

## Development setup

Tooling is pinned with [mise](https://mise.jdx.dev/) and tasks run through
[just](https://github.com/casey/just):

```sh
mise install     # installs just, lefthook, typos, taplo, nextest, deny, ...
just hooks       # installs the git hooks (lefthook)
just build
just test
```

`ghost-cms-core` holds all Ghost/Admin-API logic; `ghost-cms-shared` is the app
layer; `ghost-cms-cli` and `ghost-cms-mcp` are thin I/O adapters. Please keep new
logic in the layer where it belongs.

## Quality gates

The same checks run locally (via lefthook + `just`) and in CI. Before opening a
PR, make sure these pass:

```sh
just lint    # fmt --check, clippy -D warnings, cargo-deny, typos, machete
just test    # nextest + doctests
```

CI additionally enforces: `taplo fmt --check`, `cargo sort --check`, an MSRV
build (Rust 1.95), `cargo doc` with `-D warnings`, and the test suite on Linux,
macOS, and Windows. Run the individual commands from the plan if a CI job fails
on a platform you don't have.

### Snapshot tests

CLI output is covered by [insta](https://insta.rs/) snapshots. If you change
rendered output intentionally:

```sh
cargo insta review   # accept or reject the new snapshots
```

## Commit messages

We use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`,
`fix:`, `docs:`, `ci:`, …); see `committed.toml` for the allowed types. The
`commit-msg` hook and the `commitlint` CI job enforce this. PR titles must also
follow the convention — they become the squash-merge commit and feed the
changelog.

## Releases

Releases are automated with [release-plz](https://release-plz.dev/): merged
Conventional Commits accumulate into a "Release PR" that bumps the version and
updates `CHANGELOG.md`. Merging that PR (after adding the `release: approved`
label) tags `v{version}`, which triggers the release workflow to build and
publish the cross-platform binaries. No crate is published to crates.io.
