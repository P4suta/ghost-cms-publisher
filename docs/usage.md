# Publish a collection

Validate and review a complete collection before connecting it to Ghost.
The CLI and MCP server share the same preparation and publication core.

## Prepare locally

```sh
ghost-cms check blog/posts/
ghost-cms preview blog/posts/ --output preview
```

These commands need no Ghost URL, credentials, or network connection.
Open `preview/index.html` to browse the articles and their copied media.
The output directory must be new.
The preview contains a JSON validation report and works through `file://` links.
Add `--json` for structured command output.

A directory input selects its immediate Markdown files.
Pass files or directories together to check a larger collection.
Validation rejects duplicate slugs, empty bodies, malformed front matter, missing local files, missing article targets, and missing heading fragments.
Code examples never count as links or raw markup.
Remote links remain external dependencies.
Offline validation doesn't certify their availability or historical accuracy.

## Write a post

```markdown
---
title: "Aomori art and history"
slug: aomori-art-and-history
status: draft
language: zh-Hans
tags: [Aomori, Travel]
feature_image: ../assets/aomori.jpg
feature_image_alt: "The white museum building and its green courtyard"
feature_image_caption: "Aomori Museum of Art and its courtyard."
excerpt: "Art, archaeology, and a walk through Aomori."
---

## Getting there

Body written in **GitHub-Flavored Markdown**.

![Aomori waterfront](../assets/aomori.jpg)

[Another trip](another-trip.md#route)
```

Use unique English slugs with lowercase ASCII letters, digits, and single internal hyphens.
A slug must contain a letter and fit within 64 bytes.
Titles and bodies can use any language.
The optional `language` field sets the preview's HTML language.
`feature_image_alt` supplies the cover description for screen readers, and `feature_image_caption` preserves its caption and attribution.
Both appear in the local preview and affect the publication identity when edited.
Omit `tags` or use `tags: []` when creating posts without tags.
Publication states are `draft`, `published`, and `scheduled`.
Scheduled posts require an RFC 3339 `published_at` value.

Local article links must point to another member of the prepared collection.
They become `/{slug}/` links with native GitHub Flavored Markdown heading fragments.
Percent-encoded Unicode paths and duplicate heading identifiers work in both previews and publication.
Set `--route-template '/articles/{slug}/'` consistently on check, preview, and publish when Ghost uses custom routing.
The template must stay on the configured site.
The publisher rejects network-path URLs and control characters.

Local images, audio, videos, and other attachments share content-based upload identities.
Local videos require FFmpeg and ffprobe on `PATH` to measure their display size and duration and create a frame thumbnail.
This preparation preserves the video bytes and audio.
The publisher verifies the saved native video metadata after Ghost imports the content.

For posts already created through `schedule drafts`, preview a metadata repair before applying it.
Use a new private report path for each command.

```sh
ghost-cms video repair --receipt drafts.json --report video-preview.json
ghost-cms video repair --receipt drafts.json --report video-repair.json --apply
ghost-cms video verify --report video-repair.json
```

The preview performs no remote writes.
The applied repair uploads missing frame thumbnails and updates only native video metadata using the current revision.
It preserves body text, captions, authored custom posters, video URLs, tags, covers, and publication times.
Its report includes editor backups and records pending intent before each post update.
An interrupted run keeps its incomplete report.
Repeat with a fresh report path to verify completed changes and repair unfinished cards.
Keep the original release plan as its historical baseline and use `video verify` for the amended editor documents.
The publisher uploads attachments to Ghost's image, media, or file endpoint as appropriate.
The conservative upload and request ceiling is 5,000,000 bytes.
The Starter plan and free trials impose a [5 MB media limit](https://ghost.org/help/media-file-size-limits/).
Prepare files below 5,000,000 bytes to leave room for either interpretation.
The publisher checks files again before uploading, so changing media after validation produces an error.
It never compresses or overwrites source media.

## Captions and link cards

Write image captions as native Markdown alternative text.
The publisher keeps that text on the image and displays it below the figure.
Use a descriptive link label for a local video or audio file.
It becomes a browser-native player with the same visible caption and a label for screen readers.

```markdown
![The harbor at sunset](../assets/harbor.jpg)

[Waves along the coast](../assets/coast.mp4)

[Bells in the temple garden](../assets/bells.m4a)
```

A standalone image immediately followed by video links with matching caption prefixes becomes a video poster.
Ordered clips remain separate native players, with the poster on the first clip.
Inline prose around media remains in its original order.
Empty image descriptions remain possible for intentionally decorative images.
Collection authors should review their own caption requirements.

A standalone website or collection-article link with a descriptive label becomes a bookmark card.
An immediately following single-paragraph blockquote supplies the authored description.
The Markdown link title can also supply a description.
Matching image-and-link pairs for YouTube or Vimeo can supply a thumbnail.
Inline links, URL-only labels, generic map labels, and unsupported schemes keep ordinary link behavior.
Preparation uses authored metadata and never fetches destination websites.

```markdown
[Museum visitor guide](https://example.com/museum)

> Architecture, exhibitions, and directions supplied by the museum.
```

The generated bookmark structure follows [Ghost's theme content contract](https://docs.ghost.org/themes/content#bookmark-card).
Cards, captions, and players use the same renderer in local previews and publication.
The source files remain native Markdown.

## Markup and new posts

Native Markdown is the default.
Use `--allow-raw-html` for intentional HTML such as remote embeds.
Raw HTML resources must use absolute URLs.
Local resources and `srcset` need native Markdown or explicit preparation.
Only the publisher can supply Ghost card delimiter comments.
Ghost converts prepared HTML into its editable paragraphs, headings, lists, and cards through the [HTML import endpoint](https://docs.ghost.org/admin-api/posts/creating-a-post#source-html).
The publisher doesn't wrap the complete article in an HTML card.
Collection readback checks include the saved editor document.
Edit the original Markdown and republish it to change the body.

`ghost-cms new "My trip" --slug my-japan-trip --tag Travel` scaffolds a draft and opens the editor.
The command escapes the title and tags as YAML strings.
The command refuses an existing destination file.

Optional front matter includes `canonical_url`, `featured`, `visibility`, `authors`, search metadata, social metadata, and code injection fields.
See [FrontMatter](../crates/ghost-cms-core/src/frontmatter.rs) for the checked schema.

## Connect a site

```sh
ghost-cms init
ghost-cms doctor
ghost-cms whoami
ghost-cms publish blog/posts/ --dry-run
ghost-cms publish blog/posts/
```

`init` stores non-secret configuration and prompts for the credential source.
Configuration resolves as flags, then environment variables, then `ghost-cms.toml`, then defaults.
Set `GHOST_ADMIN_API_URL` to the site's URL and `GHOST_STAFF_TOKEN` to its `{id}:{hex-secret}` Staff Access Token.
An integration key uses the same token format where the plan supports integrations.
The client mints a short-lived HS256 JSON Web Token for each request.
Treat the token as an account credential and keep it outside version control.

`GHOST_ACCEPT_VERSION` defaults to `v5.0` and `GHOST_BLOG_DIR` defaults to `blog`.
The site URL can include a subdirectory or the existing `/ghost/api/admin/` suffix.
The client refuses redirects to keep authenticated requests at the configured destination.
The repository's remote `just` recipes use the optional 1Password CLI flow in `.env.op`.
Direct CLI commands also work with environment variables or another secret manager.

For a local `.env`, put `GHOST_ADMIN_API_URL` and `GHOST_STAFF_TOKEN` in the ignored file and restrict its permissions.
The CLI doesn't load `.env` automatically.
Build without credentials, then let Just supply the environment only to the finished binary.

```sh
chmod 600 .env
mise x -- cargo build --locked -p ghost-cms-cli
mise x -- just --dotenv-path .env --command target/debug/ghost-cms whoami
```

Hosted Ghost sites commonly use an API domain such as `https://example.ghost.io` even when the public site uses a custom domain.
Use that API domain to avoid authenticated redirects.

`--dry-run` validates locally and then reads Ghost to show the proposed action.
It requires credentials and never writes posts, uploads media, or creates state.
Use `check` for an offline inspection.
An invalid collection fails before the first network request, including when an earlier member is valid.

All exported archive candidates use `status: draft`.
Publishing without a status flag follows the front matter.
Use `--publish` only when the content should become public.

## Resume and conflicts

Publication state records local content, the remote post ID and revision, and uploaded media URLs for each destination site.
An unchanged local post skips only when Ghost still matches its saved ID and revision.
Changing a local image changes the post's identity even if its path stays the same.
A later post failure doesn't remove completed media uploads.
The publisher reuses those uploads on retry.
Writes use Ghost's `updated_at` conflict token.
A remote edit refuses publication until reviewed or explicitly overwritten with `--force`.

State writes use atomic replacement and an operating-system file lock.
An operating-system lock refuses simultaneous writes to the same state file.
Corrupt or unsupported state produces an error without replacing the file.
The publisher keeps legacy unscoped receipts.
They can't support an unchanged skip.
An absent receipt causes the normal slug-based create-or-update decision.

## Schedule a collection

Prepare drafts on the expected public site, inspect them in the production theme, then freeze and register a release plan.
The draft command refuses existing post and static-page slugs that its receipts don't own.
It validates the whole collection before its first remote write.
Schedule commands use native Markdown and the default `/{slug}/` article route.
Repeat `drafts` with the same receipt to upload local corrections before creating the release plan.
Collection membership stays fixed, and remote edits still require review.
An interrupted correction can resume through its newer publication-cache revision.

```sh
ghost-cms schedule drafts blog/posts/ --site-url https://example.com/ --receipt private/drafts.json --require-feature-images
ghost-cms schedule plan --drafts private/drafts.json --order release-order.json --output private/release.json
ghost-cms schedule apply --plan private/release.json
ghost-cms schedule verify --plan private/release.json
```

The order file is a JSON array of slug groups.
Every draft must appear exactly once.
Posts in the same group share a time.

```json
[["first-trip"], ["coastal-route", "coastal-museum"], ["travel-index"]]
```

The first slot is a whole UTC hour at least thirty minutes after plan creation starts.
Later groups follow at ten-minute intervals, including overnight.
UTC whole hours also fall on whole hours in Japan Standard Time.
The plan assigns new publication dates and adds no republication notice to the body.
Scheduling uses [Ghost's publication API](https://docs.ghost.org/admin-api/posts/scheduling-a-post).
The command omits newsletter selection, so the API [doesn't send email](https://docs.ghost.org/admin-api/posts/sending-a-post).

Keep the receipt, its adjacent publication cache, the release plan, and the source collection together.
These files record paths and post identities and should stay outside version control.
Repeat `apply` after an interruption.
It reconciles pending writes, retains scheduled or published entries, and moves expired unfinished entries to future slots after existing registrations.
Source edits, changed remote content, mismatched times, and lost identities stop registration.
Restaging drafts doesn't change a release plan that already exists.
Use `--json` on any schedule command for structured output.

The optional `--require-feature-images` check requires a cover, a description for screen readers, and a caption on every collection member.
Ordinary publication continues to support posts without covers.

## Other commands and MCP

Use `list`, `get`, `edit`, and `open` to inspect or open posts.
Use `upload` for a single attachment and `tags` to manage tag metadata.
`watch` publishes local files as they change.
`completions` and `man` generate command documentation.

Install the MCP binary with `mise x -- cargo install --locked --path crates/ghost-cms-mcp`.
The repository's `.mcp.json` template launches it over stdio through 1Password.
MCP publication uses the same local preparation, media cache, and conflict checks.
The MCP server requires Ghost configuration at startup.
Offline check and preview are CLI commands.
Standard output carries MCP messages and standard error carries diagnostics.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for development gates and the disposable local Ghost rehearsal.
