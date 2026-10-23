//! Effect-free publication decisions.

#![no_std]
#![deny(clippy::panic, clippy::unwrap_used, clippy::expect_used)]

const MAX_SLUG_BYTES: usize = 64;
const RELEASE_LEAD_SECONDS: u64 = 1_800;
const RELEASE_INTERVAL_SECONDS: u64 = 600;
const HOUR_SECONDS: u64 = 3_600;

/// Durable progress before observing Ghost.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum RegistrationState {
    /// No request has been attempted.
    Planned,
    /// A request may have reached Ghost.
    Pending,
    /// Matching registration was read back.
    Registered,
}

/// Remote state relative to the immutable draft and requested time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum RegistrationRemote {
    /// Draft identity, content, and revision match.
    Draft,
    /// Scheduled or published identity, content, and time match.
    Scheduled,
    /// An identity, content, revision, status, or time differs.
    Changed,
}

/// Permitted next step for a release entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationDecision {
    /// Register an unchanged draft.
    Register,
    /// Retain a matching scheduled or published post.
    Keep,
    /// Refuse an inconsistent observation.
    Conflict,
}

/// Decide whether a release entry can be written, retained, or refused.
#[must_use]
pub const fn registration_decision(
    state: RegistrationState,
    remote: RegistrationRemote,
) -> RegistrationDecision {
    match (state, remote) {
        (RegistrationState::Planned | RegistrationState::Pending, RegistrationRemote::Draft) => {
            RegistrationDecision::Register
        },
        (
            RegistrationState::Pending | RegistrationState::Registered,
            RegistrationRemote::Scheduled,
        ) => RegistrationDecision::Keep,
        _ => RegistrationDecision::Conflict,
    }
}

/// A checked UTC publication time in whole seconds since the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseSlot(u64);

impl ReleaseSlot {
    /// Seconds since the Unix epoch.
    #[must_use]
    pub const fn seconds(self) -> u64 {
        self.0
    }
}

/// A requested publication time cannot be represented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingError {
    /// Arithmetic would exceed the timestamp domain.
    Overflow,
}

/// Choose a whole hour at least thirty minutes ahead and ten minutes after any registered post.
///
/// # Errors
/// Returns an overflow error when no representable slot satisfies the constraints.
pub fn first_release_slot(
    now: u64,
    last_registered: Option<u64>,
) -> Result<ReleaseSlot, TimingError> {
    let mut lower = now
        .checked_add(RELEASE_LEAD_SECONDS)
        .ok_or(TimingError::Overflow)?;
    if let Some(last) = last_registered {
        let after = last
            .checked_add(RELEASE_INTERVAL_SECONDS)
            .ok_or(TimingError::Overflow)?;
        if after > lower {
            lower = after;
        }
    }
    let remainder = lower % HOUR_SECONDS;
    let seconds = if remainder == 0 {
        lower
    } else {
        lower
            .checked_add(HOUR_SECONDS - remainder)
            .ok_or(TimingError::Overflow)?
    };
    Ok(ReleaseSlot(seconds))
}

/// Advance a checked start by a number of ten-minute intervals.
///
/// # Errors
/// Returns an overflow error when the resulting timestamp is not representable.
pub fn release_slot(start: ReleaseSlot, index: u32) -> Result<ReleaseSlot, TimingError> {
    start
        .0
        .checked_add(u64::from(index) * RELEASE_INTERVAL_SECONDS)
        .map(ReleaseSlot)
        .ok_or(TimingError::Overflow)
}

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
    use super::{Content, Decision, Remote, decide, first_release_slot, release_slot, valid_slug};

    #[kani::proof]
    fn registration_decisions_preserve_completed_and_uncertain_writes() {
        use super::{
            RegistrationDecision as Decision, RegistrationRemote as Remote,
            RegistrationState as State, registration_decision,
        };
        let state: State = kani::any();
        let remote: Remote = kani::any();
        let decision = registration_decision(state, remote);
        assert_eq!(
            decision == Decision::Register,
            remote == Remote::Draft && state != State::Registered
        );
        assert_eq!(
            decision == Decision::Keep,
            remote == Remote::Scheduled && state != State::Planned
        );
        if state == State::Registered {
            assert_ne!(decision, Decision::Register);
        }
        if remote == Remote::Changed {
            assert_eq!(decision, Decision::Conflict);
        }
        kani::cover!(decision == Decision::Register);
        kani::cover!(decision == Decision::Keep);
        kani::cover!(decision == Decision::Conflict);
    }

    #[kani::proof]
    fn release_start_is_future_aligned_and_after_registered_posts() {
        let now: u64 = kani::any();
        let last: Option<u64> = kani::any();
        if let Ok(slot) = first_release_slot(now, last) {
            let time = slot.seconds();
            assert_eq!(time % 3_600, 0);
            assert!(time >= now);
            assert!(time - now >= 1_800);
            if let Some(last) = last {
                assert!(time >= last);
                assert!(time - last >= 600);
            }
            let mut lower = now + 1_800;
            if let Some(last) = last {
                lower = lower.max(last + 600);
            }
            assert!(time - lower < 3_600);
            kani::cover!(last.is_none());
            kani::cover!(last.is_some());
        }
        kani::cover!(first_release_slot(now, last).is_err());
    }

    #[kani::proof]
    fn release_slots_are_monotonic_without_overflow() {
        let now: u64 = kani::any();
        let index: u32 = kani::any();
        if let Ok(first) = first_release_slot(now, None) {
            if let Ok(slot) = release_slot(first, index) {
                assert!(slot.seconds() >= first.seconds());
                assert_eq!(slot.seconds() - first.seconds(), u64::from(index) * 600);
                kani::cover!(index > 0);
            }
            kani::cover!(release_slot(first, index).is_err());
        }
    }

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
    use super::{ReleaseSlot, first_release_slot, release_slot, valid_slug};

    #[test]
    fn release_times_keep_headroom_and_preserve_registered_slots() {
        assert_eq!(
            first_release_slot(3_600, None).map(ReleaseSlot::seconds),
            Ok(7_200)
        );
        assert_eq!(
            first_release_slot(5_400, None).map(ReleaseSlot::seconds),
            Ok(7_200)
        );
        assert_eq!(
            first_release_slot(5_401, None).map(ReleaseSlot::seconds),
            Ok(10_800)
        );
        let first = first_release_slot(0, Some(10_800));
        assert_eq!(first.map(ReleaseSlot::seconds), Ok(14_400));
        assert_eq!(
            first
                .and_then(|s| release_slot(s, 2))
                .map(ReleaseSlot::seconds),
            Ok(15_600)
        );
        assert!(first_release_slot(u64::MAX, None).is_err());
        assert!(first_release_slot(0, Some(u64::MAX)).is_err());
    }

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
