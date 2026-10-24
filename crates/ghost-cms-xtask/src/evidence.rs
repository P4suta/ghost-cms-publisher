use std::{collections::BTreeMap, path::Path};

use anyhow::{Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const SLUG_UNWIND: u32 = 66;
const SLUG_COVERS: u32 = 2;
const DECISION_COVERS: u32 = 3;
const RELEASE_START_COVERS: u32 = 3;
const RELEASE_SLOT_COVERS: u32 = 2;
const REGISTRATION_COVERS: u32 = 3;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    date: String,
    source_sha256: String,
    manifest_sha256: String,
    kani: String,
    cbmc: String,
    image: String,
    target: String,
    slug_unwind: u32,
    results: BTreeMap<String, Counts>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Counts {
    checks: u32,
    failures: u32,
    covers: u32,
}

fn verify(receipt: &Receipt, source: &[u8], manifest: &[u8]) -> Result<()> {
    ensure!(
        hex::encode(Sha256::digest(source)) == receipt.source_sha256,
        "Policy source changed; proof evidence is stale"
    );
    ensure!(
        hex::encode(Sha256::digest(manifest)) == receipt.manifest_sha256,
        "Policy manifest changed; proof evidence is stale"
    );
    ensure!(
        !receipt.date.is_empty()
            && receipt.kani == "0.68.0"
            && receipt.cbmc == "6.11.0"
            && receipt.image
                == "dotfiles-proofs@sha256:8e8c3716a29370da1603df5251d7db4ea1e28479ef96265ae5ee9ce0cc46a42a"
            && receipt.target == "x86_64-unknown-linux-gnu"
            && receipt.slug_unwind == SLUG_UNWIND,
        "Unexpected proof configuration"
    );
    let expected = [
        (
            "accepted_slugs_are_portable_english_identifiers",
            SLUG_COVERS,
        ),
        (
            "publication_decisions_preserve_receipts_and_conflicts",
            DECISION_COVERS,
        ),
        (
            "release_start_is_future_aligned_and_after_registered_posts",
            RELEASE_START_COVERS,
        ),
        (
            "release_slots_are_monotonic_without_overflow",
            RELEASE_SLOT_COVERS,
        ),
        (
            "registration_decisions_preserve_completed_and_uncertain_writes",
            REGISTRATION_COVERS,
        ),
        (
            "video_shapes_preserve_positive_measurements",
            DECISION_COVERS,
        ),
        (
            "metadata_writes_never_unpublish_or_reschedule_posts",
            DECISION_COVERS,
        ),
    ];
    ensure!(
        receipt.results.len() == expected.len(),
        "Incomplete proof inventory"
    );
    for (name, covers) in expected {
        ensure!(
            receipt
                .results
                .get(name)
                .is_some_and(|r| r.checks > 0 && r.failures == 0 && r.covers == covers),
            "Missing or unsuccessful proof: {name}"
        );
    }
    Ok(())
}

/// Check the policy receipt against the repository's actual files.
///
/// # Errors
/// Refuses changed inputs, unexpected configurations, and unsuccessful verdicts.
pub fn run(root: &Path) -> Result<()> {
    let receipt = serde_json::from_slice(&std::fs::read(root.join("docs/proof-receipt.json"))?)?;
    verify(
        &receipt,
        &std::fs::read(root.join("crates/ghost-cms-policy/src/lib.rs"))?,
        &std::fs::read(root.join("crates/ghost-cms-policy/Cargo.toml"))?,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn changed_source_and_false_or_empty_verdicts_refuse_evidence() -> anyhow::Result<()> {
        let source = include_bytes!("../../ghost-cms-policy/src/lib.rs");
        let manifest = include_bytes!("../../ghost-cms-policy/Cargo.toml");
        let mut receipt: super::Receipt =
            serde_json::from_str(include_str!("../../../docs/proof-receipt.json"))?;
        super::verify(&receipt, source, manifest)?;
        assert!(super::verify(&receipt, b"changed", manifest).is_err());
        assert!(super::verify(&receipt, source, b"changed").is_err());
        let result = receipt
            .results
            .get_mut("accepted_slugs_are_portable_english_identifiers")
            .ok_or_else(|| anyhow::anyhow!("missing fixture verdict"))?;
        result.failures = 1;
        assert!(super::verify(&receipt, source, manifest).is_err());
        receipt.results.clear();
        assert!(super::verify(&receipt, source, manifest).is_err());
        Ok(())
    }
}
