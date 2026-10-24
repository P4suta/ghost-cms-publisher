---
date: 2026-10-24
---

# Prepare video metadata before native card publication

## Status

Proposed.

## Context

Ghost's HTML importer accepts a video URL without inspecting the uploaded file.
The editor displays its empty selector when both thumbnail fields are empty.
Missing dimensions also prevent a portrait video from keeping its measured proportions.
A rendered video element and a successful file request don't establish a populated editor card.

## Decision

Measure local video dimensions and duration through the shared native filesystem adapter.
Decode a frame with FFmpeg and create a thumbnail without rewriting the source video.
Keep the generated asset bound to the original video digest and verify that digest before upload.
Limit thumbnail dimensions without enlarging smaller frames.
Refuse invalid, missing, or changed measurements and decoding failures.
Keep remote video links as ordinary links when no local measurements exist.

Import normal Markdown content through the existing HTML boundary.
Supply the thumbnail, dimensions, and duration that Ghost's HTML importer understands.
Then fill the complete native video metadata through a revision-checked editor update.
Require exact editor readback and unchanged content outside the declared video metadata fields.
Preserve authored custom posters, captions, source URLs, publication dates, covers, and tags.
Permit an unchanged known publication state or Ghost's scheduled-to-published transition.

Repair existing posts only through their destination-bound draft ownership receipt and content-based upload cache.
Plan the complete owned collection before a remote write.
Upload only missing generated frame images and never replace an existing video URL.
Write a private backup and pending intent before each editor update.
Read back the result before recording completion.
Keep the original release plan as its historical baseline and use the repair report to verify the amended editor documents.
Require a fresh report path for another run.
An unchanged repeat performs no post update.

The [video contract](../../crates/ghost-cms-core/src/video.rs) owns document validation and metadata changes.
The [native adapter](../../crates/ghost-cms-shared/src/filesystem/video.rs) supplies decoder observations.
The [policy crate](../../crates/ghost-cms-policy/src/lib.rs) checks positive measurements and publication transitions.
The [boundary rehearsal](../../crates/ghost-cms-xtask/src/video.rs) exercises preview, repair, repeat, and changed-content refusal with real Ghost storage.

## Consequences

Local video preparation requires FFmpeg and ffprobe on the command's path.
Video-free collections keep their existing runtime requirements.
The process adapter trusts the installed decoder to report measurements and decode the frame correctly.
The proofs cover the deterministic measurement and publication decisions, while known-answer cases and the real Ghost rehearsal check JSON and API behavior.
They don't prove a decoder, a theme, or browser playback support for every source codec.
Changing a video's codec remains a separate operation with its own source and quality checks.

Checking only the HTML result misses the editor's empty-card state.
Replacing a whole post during repair risks losing later text edits and publication settings.
Re-encoding the source doesn't supply the card's missing metadata.
Keeping a private repair report adds local state, but it preserves backups and makes partial or stale results visible.
