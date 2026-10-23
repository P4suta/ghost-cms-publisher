# Local verification

Checked on 2026-10-24 with Rust 1.99.1 and the tools in `mise.toml`.
These results cover the implementation associated with [issue 27](https://github.com/P4suta/ghost-cms-publisher/issues/27).

## Executable contracts

The workspace tests cover front matter, offline collection preparation, CLI output, HTTP requests, preview cleanup, media changes, and upload retry.
They also check destination-scoped receipts, corrupt state, concurrent state access, explicit remote overrides, and invalid batches before network access.
All 85 workspace tests passed, followed by the workspace documentation tests.
The scheduling fixtures cover metadata corrections before freezing, foreign slug and page collisions, changed remote revisions, interrupted writes, retry adoption, and preservation of completed publication times.
The editor readback regression rejects empty native containers and ignores them beside a complete HTML card.
Native text nested inside links and documents containing only native media or code cards remain accepted.
The route regression preserves root, nested, and Unicode article paths while rejecting network-path URLs and control characters before publication.
Rendering fixtures cover visible image/video/audio captions, escaped labels, paired posters, authored bookmark descriptions and thumbnails, inline-link preservation, ordinary-link fallbacks, and literal Markdown examples.
The Starter size fixture accepts 5,000,000 bytes and rejects 5,000,001 bytes before network access.
The scaffold regression checks quoted multilingual metadata, paths with spaces, overwrite refusal, and path traversal rejection.
The actual `just new` recipe also produced a valid draft with a quoted Japanese title, a comma-containing tag, and a directory containing spaces.

The checked commands are:

```sh
mise x -- just lint
mise x -- just test
mise x -- cargo +1.95.0 check --workspace --all-targets --locked
mise x -- env RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
mise x -- taplo fmt --check
mise x -- cargo sort --workspace --grouped --check
mise x -- just --fmt --check
dotfiles repository check
```

The Rust 1.95.0 check covers the oldest supported compiler version.
Local runtime tests use macOS on Apple Silicon.
Linux and Windows runtime checks remain the existing CI matrix's responsibility.

## Policy proofs

Kani 0.68.0 with the C Bounded Model Checker (CBMC) 6.11.0 proved the production policy crate as Linux amd64.
The image was `dotfiles-proofs@sha256:8e8c3716a29370da1603df5251d7db4ea1e28479ef96265ae5ee9ce0cc46a42a`.
The runner bounded each invocation to 2 GiB memory, no extra swap, two CPUs, 256 processes, and 900 seconds.
All five harnesses completed successfully within those limits.

| Harness | Domain | Result |
| --- | --- | --- |
| `publication_decisions_preserve_receipts_and_conflicts` | All remote states, content states, and force values | 57 checks, zero failures, three covers satisfied |
| `accepted_slugs_are_portable_english_identifiers` | Arbitrary byte arrays and lengths 0–64, unwind 66 | 130 checks, zero failures, two covers satisfied |
| `release_start_is_future_aligned_and_after_registered_posts` | Arbitrary unsigned epoch values and the last completed time | 64 checks, zero failures, three covers satisfied |
| `release_slots_are_monotonic_without_overflow` | Arbitrary unsigned start times and slot indexes | 39 checks, zero failures, two covers satisfied |
| `registration_decisions_preserve_completed_and_uncertain_writes` | All local entry and remote observation variants | 69 checks, zero failures, three covers satisfied |

The policy source SHA-256 is `622ad3f5f399f3c47149006c65fb6849d1b810f614c4cf2cd0718c616a8f81d5`.
The policy manifest SHA-256 is `82189adef6e96c3e8238684f7e29e02d1f373d3c13d31a8eab7ee49319a0fde4`.
The machine-readable [receipt](proof-receipt.json) binds the results to those inputs and verifier settings.
The fast gate rejects a stale source or manifest, failed verdict, unexpected configuration, or incomplete inventory.
Its regression test plants changed source and false or empty results and confirms rejection.
The filesystem port has a native adapter in the shared crate.
Its failure fixture injects a final preview identity error and requires cleanup of the owned output directory.
Receipt validation checks recorded evidence.
It can't replace running the prover after a policy change.

These proofs establish the decision and slug properties within their stated domains.
They assume the adapter accurately observes remote IDs, revisions, and content identities.
They don't prove Markdown/YAML parsers, operating-system persistence and locking, HTTP servers, or Ghost's content conversion.
Known-answer fixtures and the real Ghost rehearsal check those boundaries.
Changes to parser versions, rendering rules, persistence adapters, or supported Ghost behavior require renewed boundary checks.

## Dependencies and credentials

The complete dependency policy check passed with a clean RustSec database at revision `f7b2f6bc663452200bbe639ebb75e0c762b3422b`.
The audit configuration used the repository policy with only `advisories.db-path` changed to an isolated database directory.

```sh
mise x -- cargo deny -L error --config /path/to/isolated-deny.toml --offline --locked check advisories bans licenses sources
```

The Comrak configuration omits unused CLI and syntax-highlighting features.
The YAML parser now uses deserialize-only serde-saphyr 2.0.0.
The lockfile updates the affected TLS/HTTP dependencies and a yanked spin release.
The bundled trust store resolves to `webpki-roots` 1.0.10, which removes the root covered by [RUSTSEC-2026-0381](https://rustsec.org/advisories/RUSTSEC-2026-0381.html).
The previous 1.0.8 lockfile failed this advisory check with the same database before the update.
The `rmcp` crate and its macros resolve to 2.2.0, including the fix described in the [client transport advisory](https://github.com/advisories/GHSA-9g45-5xwm-f3wc).
The enabled features use the stdio server transport.
A real stdio session completed initialization, listed nine tools, rejected an invalid slug, and shut down successfully on input closure.
The session used dummy credentials and an unused loopback destination.
The existing RSA advisory exception remains narrow: authentication uses HS256 and never calls RSA operations.
This result describes the recorded advisory snapshot rather than an online refresh.
Gitleaks 8.30.1 found no credentials in the workspace's Rust source and tests.

## Ghost integration boundary

The owned loopback rehearsal uses Ghost 6.72.0 at the immutable image named in the xtask.
A real multilingual fixture preserved visible text, literal HTML code, a pipe table, and a linked Chinese heading, then skipped an unchanged second publication.
The publisher imports normal content through Ghost's HTML conversion without a complete-body HTML-card wrapper.
The rehearsal reads the stored Lexical document and requires native editor blocks alongside any individual HTML cards.
It also checks returned content, local media URLs, and article fragments before repeating publication.
It removes its container, volumes, temporary credentials, and network on success or failure.
The implementation never reads the production Ghost credential source during a rehearsal.

The archive snapshot used for this check passed the same rehearsal with 125 draft posts and 1,067 uploaded local media URLs.
Readback preserved caption text, bookmark cards, media player kinds, and poster counts alongside text, code, tables, and linked fragments.
Every post skipped during an unchanged second publication.
The scheduling rehearsal verified all 125 scheduled posts, repeated the same registration without changes, and observed a real scheduled-to-published transition.
Scheduling writes changed status and publication time without selecting a newsletter.
The owned container and network were absent after completion.
This checks returned Ghost HTML and HTTP resources rather than a production theme's presentation or a browser's decoder.
Inspection of the pinned image's `ghost_head` helper confirmed that normal helper output includes the packaged card assets without requiring a native bookmark node.
