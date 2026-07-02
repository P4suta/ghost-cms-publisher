//! Small string helpers shared by the frontends.

use std::time::{SystemTime, UNIX_EPOCH};

/// Derive an ASCII kebab-case slug from a title.
///
/// Falls back to `post-<unix_seconds>` when the title has no ASCII alphanumerics.
#[must_use]
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
            prev_dash = false;
        } else if !out.is_empty() && !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let slug = out.trim_matches('-').to_owned();
    if slug.is_empty() {
        format!("post-{}", unix_secs())
    } else {
        slug
    }
}

/// Mask a secret for display, keeping a short recognizable prefix.
#[must_use]
pub fn mask_token(token: Option<&str>) -> String {
    match token {
        Some(t) if t.len() > 6 => format!("{}…", &t[..6]),
        Some(_) => "******".to_owned(),
        None => "(unset)".to_owned(),
    }
}

/// The Levenshtein edit distance between two strings (counted in `char`s).
#[must_use]
pub fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.chars().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// The candidate closest to `target` by edit distance.
///
/// Returns `None` when nothing is within a typo-sized threshold.
#[must_use]
pub fn nearest<'a, I>(candidates: I, target: &str) -> Option<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let threshold = (target.chars().count() / 3).max(2);
    candidates
        .into_iter()
        .map(|c| (levenshtein(c, target), c))
        .filter(|(distance, _)| *distance <= threshold)
        .min_by_key(|(distance, _)| *distance)
        .map(|(_, candidate)| candidate.to_owned())
}

/// Seconds since the Unix epoch (0 if the clock predates 1970).
fn unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::{mask_token, nearest, slugify};

    #[test]
    fn slugify_kebabs_ascii() {
        assert_eq!(slugify("Hello, World!"), "hello-world");
        assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
        assert_eq!(slugify("Rust + Ghost"), "rust-ghost");
    }

    #[test]
    fn slugify_falls_back_for_non_ascii() {
        assert!(slugify("テスト記事").starts_with("post-"));
    }

    #[test]
    fn mask_keeps_prefix() {
        assert_eq!(mask_token(Some("64abcdef0011")), "64abcd…");
        assert_eq!(mask_token(Some("short")), "******");
        assert_eq!(mask_token(None), "(unset)");
    }

    #[test]
    fn nearest_suggests_close_typos() {
        let slugs = ["hello-world", "getting-started", "about"];
        assert_eq!(nearest(slugs, "helo-world").as_deref(), Some("hello-world"));
        assert_eq!(nearest(slugs, "abuot").as_deref(), Some("about"));
    }

    #[test]
    fn nearest_ignores_distant_input() {
        let slugs = ["hello-world", "getting-started"];
        assert_eq!(nearest(slugs, "xyz"), None);
        let empty: [&str; 0] = [];
        assert_eq!(nearest(empty, "anything"), None);
    }
}
