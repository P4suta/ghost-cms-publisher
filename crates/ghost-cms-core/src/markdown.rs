//! Split a post file into front matter + body, and render GFM to HTML.

use crate::error::{CoreError, Result};
use crate::frontmatter::FrontMatter;

/// The front-matter delimiter line.
const DELIM: &str = "---";

/// A post file split into its typed front matter and Markdown body.
#[derive(Debug)]
pub struct ParsedPost {
    /// Parsed and validated front matter.
    pub front: FrontMatter,
    /// The Markdown body following the front-matter block.
    pub body_md: String,
}

/// Convert a `serde_yaml_ng` error into a [`CoreError`], attaching the absolute
/// byte offset within the original file when a location is available.
fn frontmatter_syntax_error(e: &serde_yaml_ng::Error, yaml_start: usize) -> CoreError {
    e.location().map_or_else(
        || CoreError::FrontMatter(format!("invalid YAML frontmatter: {e}")),
        |loc| CoreError::FrontMatterSyntax {
            message: e.to_string(),
            offset: yaml_start + loc.index(),
            line: loc.line(),
            column: loc.column(),
        },
    )
}

/// Byte offsets of the closing delimiter within the post-open slice.
struct Close {
    yaml_end: usize,
    body_start: usize,
}

/// Locate the closing `---` line (a line whose trimmed content is exactly `---`).
fn find_closing(s: &str) -> Option<Close> {
    let mut offset = 0usize;
    for line in s.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']).trim_end();
        if trimmed == DELIM {
            return Some(Close {
                yaml_end: offset,
                body_start: offset + line.len(),
            });
        }
        offset += line.len();
    }
    None
}

/// Parse a post file: a leading `---` YAML block, a closing `---`, then body.
///
/// # Errors
/// Returns [`CoreError::FrontMatter`] when the block is missing, unterminated,
/// or the YAML fails to deserialize / validate.
pub fn parse(input: &str) -> Result<ParsedPost> {
    let normalized = input.strip_prefix('\u{feff}').unwrap_or(input);
    let trimmed = normalized.trim_start();
    let after_open = trimmed.strip_prefix(DELIM).ok_or_else(|| {
        CoreError::FrontMatter("missing frontmatter block (file must start with `---`)".to_owned())
    })?;
    let after_open = after_open.strip_prefix('\r').unwrap_or(after_open);
    let after_open = after_open
        .strip_prefix('\n')
        .ok_or_else(|| CoreError::FrontMatter("malformed frontmatter opening line".to_owned()))?;

    let close = find_closing(after_open).ok_or_else(|| {
        CoreError::FrontMatter("unterminated frontmatter block (missing closing `---`)".to_owned())
    })?;

    let yaml = &after_open[..close.yaml_end];
    let body = &after_open[close.body_start..];

    // `after_open` is a suffix of `input`, so its start offset is the difference
    // in lengths; `yaml` begins at the same offset.
    let yaml_start = input.len() - after_open.len();
    let front: FrontMatter =
        serde_yaml_ng::from_str(yaml).map_err(|e| frontmatter_syntax_error(&e, yaml_start))?;
    front.validate()?;

    Ok(ParsedPost {
        front,
        body_md: body.trim_start_matches(['\r', '\n']).to_owned(),
    })
}

/// Render a GFM Markdown body to HTML.
///
/// `allow_raw_html` controls whether inline/raw HTML in the source is passed
/// through (`true`) or escaped (`false`, the safe default).
#[must_use]
pub fn render_html(body_md: &str, allow_raw_html: bool) -> String {
    let mut options = comrak::Options::default();
    options.extension.table = true;
    options.extension.strikethrough = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.render.r#unsafe = allow_raw_html;
    comrak::markdown_to_html(body_md, &options)
}

#[cfg(test)]
mod tests {
    use super::{parse, render_html};

    #[test]
    fn parses_frontmatter_and_body() {
        let input = "---\ntitle: Hello\nslug: hello\ntags: [Rust, CLI]\n---\n\n# Body\n\nText.\n";
        let parsed = parse(input).unwrap();
        assert_eq!(parsed.front.title, "Hello");
        assert_eq!(parsed.front.slug, "hello");
        assert_eq!(parsed.front.tags, vec!["Rust".to_owned(), "CLI".to_owned()]);
        assert!(parsed.body_md.starts_with("# Body"));
    }

    #[test]
    fn rejects_missing_block() {
        assert!(parse("no frontmatter here").is_err());
    }

    #[test]
    fn rejects_unknown_key() {
        let input = "---\ntitle: X\nslug: x\nbogus: 1\n---\nbody\n";
        assert!(parse(input).is_err());
    }

    #[test]
    fn syntax_error_carries_offset_into_original_file() {
        use crate::error::CoreError;
        // Bad indentation on line 3 of the YAML block (line 4 of the file).
        let input = "---\ntitle: X\n  bad: : :\nslug: x\n---\nbody\n";
        match parse(input) {
            Err(CoreError::FrontMatterSyntax { offset, line, .. }) => {
                assert!(line >= 1);
                // Offset points within the original input, past the opening `---\n`.
                assert!(offset >= 4 && offset <= input.len(), "offset={offset}");
            },
            other => panic!("expected FrontMatterSyntax, got {other:?}"),
        }
    }

    #[test]
    fn parses_extended_fields() {
        let input = "---\ntitle: X\nslug: x\nfeatured: true\nvisibility: members\nog_title: OG\nauthors: [a@example.com]\n---\nbody\n";
        let parsed = parse(input).unwrap();
        assert_eq!(parsed.front.featured, Some(true));
        assert_eq!(
            parsed.front.visibility,
            Some(crate::domain::Visibility::Members)
        );
        assert_eq!(parsed.front.og_title.as_deref(), Some("OG"));
        assert_eq!(parsed.front.authors, vec!["a@example.com".to_owned()]);
    }

    #[test]
    fn rejects_bad_visibility() {
        let input = "---\ntitle: X\nslug: x\nvisibility: bogus\n---\nbody\n";
        assert!(parse(input).is_err());
    }

    #[test]
    fn renders_gfm_table_and_tasklist() {
        let html = render_html("| a | b |\n|---|---|\n| 1 | 2 |\n\n- [x] done\n", false);
        assert!(html.contains("<table>"));
        assert!(html.contains("checkbox"));
    }

    #[test]
    fn escapes_raw_html_when_unsafe_disabled() {
        let html = render_html("<script>alert(1)</script>", false);
        assert!(!html.contains("<script>"));
    }
}
