---
date: 2026-10-23
---

# Stage drafts before registering release plans

## Status

Accepted for collection republication.

## Context

A collection needs complete remote drafts and a reviewable release order before publication times are assigned.
Recomputing every time during a retry can move posts that Ghost already scheduled or published.
A response can also disappear after Ghost accepted a request.

## Decision

Keep draft preparation, plan creation, registration, and readback as separate CLI operations.
Bind each receipt to the normalized Admin API destination, expected public URL, source files, media identities, remote IDs, and remote content.
Check every post and static-page slug before creating the first draft.
Adopt existing posts only through matching local receipts and revisions.
Support explicit restaging of local corrections while collection membership stays fixed.
Use a matching newer publication-cache revision to recover a correction whose draft receipt was not saved.
Freeze source identities when creating the release plan.
Import ordinary content into editable Ghost blocks instead of a single HTML card.
Use Ghost-compatible image, video, audio, and bookmark markup for conversion.
Read back the saved editor document and refuse an entire collection article stored only as HTML cards.

Freeze explicit ordered slug groups into UTC timestamps.
Use the next whole hour at least thirty minutes ahead and ten-minute intervals between groups.
Allow one group to contain simultaneous posts.
Write pending intent before each scheduling request and read back its result before recording completion.
On retry, adopt a matching uncertain registration and keep every completed time.
Move expired unfinished entries into future slots after the registered prefix.
Send only status, publication time, and the observed revision during registration.
Omit newsletter selection so publication affects the website only.

The [policy crate](../../crates/ghost-cms-policy/src/lib.rs) owns time arithmetic and registration decisions.
The [schedule adapter](../../crates/ghost-cms-core/src/schedule.rs) validates remote observations and persists progress through the existing filesystem port.
The bounded proofs quantify the admitted integer and enum domains.
HTTP, hashing, Ghost conversion, and background execution remain external trust boundaries covered by fixtures and the disposable Ghost rehearsal.

## Consequences

The computer can stop after registration because Ghost owns the actual publication schedule.
Local source changes or remote edits require review before further writes.
A request whose result cannot be reconciled stays pending rather than triggering an unconditional replacement.
Receipts contain local paths and remote post identities and belong outside version control.
Removing a source tag does not authorize deleting an unrelated site tag.

Scheduling complete post payloads was rejected because it can replace content during a time-only operation.
A client-side publishing daemon was rejected because it requires a continuously running computer.
Reassigning the complete batch on every run was rejected because it changes completed registrations.
The API offers revision checks for individual writes, so batch preflight is not a remote transaction.
