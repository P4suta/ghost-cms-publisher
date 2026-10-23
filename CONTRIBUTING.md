# Contributing

Use the exact tools in `mise.toml` and the command surface in `Justfile`.

```sh
mise install
mise x -- just hooks
mise x -- just build
mise x -- just lint
mise x -- just test
```

## Boundaries

`ghost-cms-policy` holds dependency-free publication decisions and slug validation.
`ghost-cms-core` prepares collections and implements Ghost API operations behind an HTTP transport.
`ghost-cms-shared` supplies application projections and configuration.
Its native filesystem adapter implements the core's file capability without introducing filesystem effects into the core.
The CLI and MCP binaries own user interaction and process exits.
`ghost-cms-xtask` verifies proof identities and rehearses publication against a disposable local Ghost instance.

Keep Markdown authoritative and preserve literal code during rendering.
Validate a complete batch before network operations.
Keep state scoped to its normalized destination and retain Ghost's conflict token on updates.
See the [publication ADR](docs/adr/prepare-publications-before-remote-writes.md) and [usage guide](docs/usage.md).

## Verification

`just lint` checks formatting, Clippy, dependency policy, spelling, unused dependencies, and the source-bound proof receipt.
`just test` runs the workspace tests through Nextest and then checks documentation tests.
`just audit` checks the existing local RustSec advisory snapshot.
Update that snapshot separately when an online advisory refresh is intended.

The additional CI gates use `taplo fmt --check`, `cargo sort --workspace --grouped --check`, Rust 1.95, documentation warnings, and Linux/macOS/Windows tests.
The local rehearsal requires Docker and the pinned Ghost image in the xtask.
It creates an owned network and container, binds Ghost only on loopback, uses temporary credentials and storage, checks returned content and local media, repeats publication, and removes its resources.
SMTP points to an unused container-local port.
The container has its own bridge network; normal Ghost startup may contact its upstream services.
No production Ghost configuration is read by this command.

```sh
mise x -- just rehearse blog/posts/
mise x -- just proofs
```

`just proofs` uses the configured bounded proof image with Kani 0.68.0 and CBMC 6.11.0.
The owner-side runner applies memory, CPU, process, and deadline limits.
A limit stop is inconclusive.
The [proof receipt](docs/proof-receipt.json) records the measured results for the exact policy source and manifest.
`verify-evidence` checks that receipt's identities and inventory; it does not run Kani or independently certify the recorded verdict.
When the policy changes, run both proofs and update the receipt with their actual results before relying on it.
The receipt regression test rejects changed source, changed configuration inputs, failed verdicts, and an empty inventory.
Parser, filesystem, HTTP, and Ghost conversion behavior use fixtures and real integration checks rather than a theorem about those external systems.

The generated README and license files come from `repository.toml` through `dotfiles repository generate`.
Keep detailed documentation in `docs/`.

## Contributions and releases

Use Conventional Commits and keep each pull request within its linked issue.
Snapshot changes require an intentional review through `cargo insta review`.
The existing release-plz workflow proposes version updates and release publication remains protected by the repository's release approval flow.
No crate in this workspace is published to crates.io.
