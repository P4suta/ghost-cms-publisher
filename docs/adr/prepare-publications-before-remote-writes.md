---
date: 2026-10-23
---

# Prepare publications before remote writes

## Status

Proposed for the local publication workflow.

## Context

Collection imports need validated metadata, complete media, working article links, and a preview before connecting to Ghost.
The previous publisher uploads images before deciding whether a post needs an update and keys receipts only by slug.
Those choices repeat uploads and can mistake another site's receipt for the current post.

## Decision

Prepare a collection locally, validate every member, and share that prepared representation between checking, previews, and publishing.
Rewrite local article links using the collection's English slugs and use native GFM heading identifiers.
Use lowercase English slugs of at most 64 bytes as portable publication identifiers.
Keep media hashes in the local content identity, independent of uploaded URLs.
Scope receipts and uploaded-media records to the normalized Admin API URL.
Skip a post only when its content, remote ID, and remote revision match the saved receipt.
Refuse remote edits unless the caller passes `--force`.
Persist state atomically after each successful external step and preserve conflict tokens on updates.
Send prepared HTML as a Ghost HTML card so Ghost preserves native heading identifiers, code, and tables.

The effect-free [policy crate](../../crates/ghost-cms-policy/src/lib.rs) owns the publication decision and slug contract.
Filesystem reads, Markdown parsing, hashing, HTTP, and persistence remain explicit external boundaries.
The core declares a filesystem port, and the shared application crate supplies its native adapter.
Prepared collections carry that capability through media reads, preview writes, atomic state replacement, and exclusive state leases.

## Consequences

Offline preparation needs no URL, token, or network access.
A collection with invalid local inputs fails before the first remote write.
Ghost still controls its own HTML-to-Lexical conversion and configured public routes.
The local Markdown remains the editable source; Ghost presents the imported body as an HTML card.
Unmarked HTML conversion was rejected after the real Ghost rehearsal removed a linked Chinese heading identifier.
The default article route is `/{slug}/`; custom Ghost routing requires a matching route template.
Previews and state writes refuse existing output collisions or concurrent publication rather than replacing unrelated files.

Checking individual posts during a batch was rejected because it discovers late local failures after earlier remote changes.
Hashing uploaded payloads was rejected because CDN URLs change independently of source content.
Discarding remote revision information was rejected because local caches cannot establish that a remote post stayed unchanged.
