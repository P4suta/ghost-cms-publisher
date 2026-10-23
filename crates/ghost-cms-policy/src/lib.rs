//! Effect-free publication decisions.

#![no_std]
#![deny(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

const MAX_SLUG_BYTES: usize = 64;

/// State of a remote post relative to the destination's receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Remote {
    /// The slug does not exist.
    Absent,
    /// The post has no revision for conflict detection.
    MissingRevision,
    /// A revision exists but no receipt exists.
    Unrecorded,
    /// The remote ID and revision match the receipt.
    MatchesReceipt,
    /// The ID or revision changed since the receipt.
    Changed,
}

/// Local content identity relative to the receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Content {
    /// Source and media hashes match.
    Same,
    /// Source or media changed, or no receipt exists.
    Different,
}

/// A complete publication decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// No remote post exists.
    Create,
    /// Local content and the remote receipt both match.
    Skip,
    /// Update using the observed remote revision.
    Update,
    /// The remote object changed since the last successful write.
    RemoteChanged,
    /// The remote post lacks its required conflict token.
    MissingRevision,
}

/// Decide using only the caller's observations.
#[must_use]
pub const fn decide(remote: Remote, content: Content, force: bool) -> Decision {
    match remote {
        Remote::Absent => Decision::Create,
        Remote::MissingRevision => Decision::MissingRevision,
        Remote::Changed if !force => Decision::RemoteChanged,
        Remote::MatchesReceipt if matches!(content, Content::Same) && !force => Decision::Skip,
        Remote::Unrecorded | Remote::MatchesReceipt | Remote::Changed => Decision::Update,
    }
}

/// Check an English slug of at most 64 bytes with single internal hyphens.
#[must_use]
pub fn valid_slug(bytes: &[u8]) -> bool {
    if bytes.is_empty() || bytes.len() > MAX_SLUG_BYTES {
        return false;
    }
    let mut letter = false;
    let mut previous_hyphen = true;
    for &byte in bytes {
        match byte {
            b'a'..=b'z' => {
                letter = true;
                previous_hyphen = false;
            },
            b'0'..=b'9' => previous_hyphen = false,
            b'-' if !previous_hyphen => previous_hyphen = true,
            _ => return false,
        }
    }
    letter && !previous_hyphen
}

#[cfg(kani)]
mod proofs {
    use super::{Content, Decision, Remote, decide, valid_slug};

    #[kani::proof]
    fn publication_decisions_preserve_receipts_and_conflicts() {
        let remote: Remote = kani::any();
        let content: Content = kani::any();
        let force: bool = kani::any();
        let decision = decide(remote, content, force);
        assert_eq!(decision == Decision::Create, remote == Remote::Absent);
        if decision == Decision::Skip {
            assert!(remote == Remote::MatchesReceipt && content == Content::Same && !force);
        }
        if remote == Remote::Changed && !force {
            assert_eq!(decision, Decision::RemoteChanged);
        }
        if decision == Decision::Update {
            assert!(
                matches!(remote, Remote::Unrecorded | Remote::MatchesReceipt)
                    || (remote == Remote::Changed && force)
            );
        }
        kani::cover!(decision == Decision::Skip);
        kani::cover!(decision == Decision::Update);
        kani::cover!(decision == Decision::RemoteChanged);
    }

    #[kani::proof]
    #[kani::unwind(66)]
    fn accepted_slugs_are_portable_english_identifiers() {
        let bytes: [u8; 64] = kani::any();
        let len: usize = kani::any();
        kani::assume(len <= 64);
        let slug = &bytes[..len];
        if valid_slug(slug) {
            assert!(!slug.is_empty());
            assert!(slug[0] != b'-' && slug[len - 1] != b'-');
            let mut letter = false;
            let mut previous = false;
            for &byte in slug {
                assert!(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
                assert!(!(previous && byte == b'-'));
                previous = byte == b'-';
                letter |= byte.is_ascii_lowercase();
            }
            assert!(letter);
            kani::cover!(len > 1);
        }
        kani::cover!(!valid_slug(slug));
    }
}

#[cfg(test)]
mod tests {
    use super::valid_slug;

    #[test]
    fn slug_boundaries() {
        assert!(valid_slug(b"chinese-travel-guide-2026"));
        for bad in [
            b"".as_slice(),
            b"123",
            b"../draft",
            b"-a",
            b"a-",
            b"a--b",
            b"A",
            b"a b",
        ] {
            assert!(!valid_slug(bad));
        }
        assert!(!valid_slug(&[b'a'; 65]));
    }
}
