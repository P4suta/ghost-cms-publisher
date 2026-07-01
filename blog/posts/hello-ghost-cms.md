---
title: "Hello, ghost-cms-publisher"
slug: hello-ghost-cms-publisher
status: draft
tags: [Meta, Rust]
excerpt: "A sample post showing the front matter schema and GFM rendering."
meta_description: "Sample post for ghost-cms-publisher."
# Optional extended fields (all may be omitted):
featured: false
visibility: public
---

# Hello

This is a sample post. The block above is the **front matter**; everything below
is the body, written in GitHub-Flavored Markdown and rendered to HTML on publish.

## What the front matter controls

| Field           | Purpose                                  |
| --------------- | ---------------------------------------- |
| `title`         | Post title (required)                    |
| `slug`          | Stable idempotency key (required)        |
| `status`        | `draft`, `published`, or `scheduled`     |
| `tags`          | Resolved or created by name              |
| `canonical_url` | Point search engines at the original     |

## GFM features

- [x] tables
- [x] task lists
- [x] ~~strikethrough~~ and autolinks like https://ghost.org/
- [ ] anything you don't write

Publishing this file is idempotent: run it once to create the draft, edit and
run again to update it. Unchanged content is skipped.
