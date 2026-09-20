//! Replayable original inputs for final-cut publication validation (FA-062).
//!
//! These packets are observations, not approvals. Decoding always reconstructs
//! the existing validated types; a binding recaptures witnesses from the ORIGINAL
//! image, never from the most recently received image. Adapter/helper authenticity
//! and storage integrity remain host assumptions.

mod codec;
pub mod producer;
#[cfg(test)]
mod tests;

use crate::action::FrozenAction;
use crate::action::consequence::delivery::publication_gate::PublicationInputs;
use crate::action::consequence::oversight::publication::PublicationJudgment;
use crate::full_input::{ActualHelperInput, OpaqueJudgment};
use crate::product_frontier::{FrontierStage, ProductFrontiers, TrustedClosingMarker};
use crate::witness::{AdapterDomainInput, SnapshotEntry, WitnessJudgment, WitnessRequest, WitnessSnapshot, MAX_WITNESSES};
use crate::Error;

/// Legacy FAPWIN01/FAPWEV01 wire ceiling. Longer admitted prefixes use version
/// two and constant-span reconstruction; native sequence counters remain u64.
pub const MAX_REPLAY_PREFIX: u64 = 4_096;
pub const MAX_PUBLICATION_PACKET_BYTES: usize = 3 * 1_048_576;

/// Exactly the snapshot and closing observation consumed by witness validation.
/// Nonterminal prefixes and unrelated projections cannot establish absence and
/// are deliberately not exported as a reusable full ProductFrontiers history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileWitnessInput {
    snapshot: WitnessSnapshot,
    keys: Vec<u64>,
    admitted_close: Option<TrustedClosingMarker>,
}

impl FileWitnessInput {
    pub fn new(
        revision: u64, control_cut: u64, semantic_epoch: u64,
        domain: AdapterDomainInput, entries: Vec<SnapshotEntry>, frontiers: &ProductFrontiers,
    ) -> Result<Self, Error> {
        if entries.len() > crate::witness::MAX_SNAPSHOT_ENTRIES { return Err(Error::Limit); }
        let mut keys: Vec<_> = entries.iter().map(SnapshotEntry::key).collect();
        keys.sort_unstable();
        let snapshot = WitnessSnapshot::new(revision, control_cut, semantic_epoch, domain, entries)?;
        let admitted_close = frontiers.closing_marker(domain.domain().projection());
        Ok(Self { snapshot, keys, admitted_close })
    }

    pub fn snapshot(&self) -> &WitnessSnapshot { &self.snapshot }
    pub fn admitted_close(&self) -> Option<TrustedClosingMarker> { self.admitted_close }

    /// Rebuild only the closed authenticated projection used by the existing
    /// witness engine. Missing closure remains missing even with a snapshot's
    /// asserted Closed marker; it must never be inferred from that assertion.
    pub fn witness_frontiers(&self) -> Result<ProductFrontiers, Error> {
        replay_compact_frontiers(self.admitted_close)
    }
}

fn replay_frontiers(close: Option<TrustedClosingMarker>) -> Result<ProductFrontiers, Error> {
    if close.is_some_and(|marker| marker.final_sequence > MAX_REPLAY_PREFIX) { return Err(Error::Limit); }
    replay_compact_frontiers(close)
}

// The archive retains an independently ADMITTED close, not merely the snapshot's
// asserted marker. Reconstruct just that already closed authenticated projection.
fn replay_compact_frontiers(close: Option<TrustedClosingMarker>) -> Result<ProductFrontiers, Error> {
    let mut frontiers = ProductFrontiers::new(1, 1)?;
    if let Some(marker) = close {
        if marker.final_sequence != 0 {
            frontiers.accept_contiguous(marker.key, FrontierStage::Authenticated, 1, marker.final_sequence)?;
        }
        frontiers.record_close(marker)?;
    }
    Ok(frontiers)
}

/// Owned current observations. An absent lane is retained as absent, not treated
/// as a request to remove that lane from an already reviewed binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePublicationInputs {
    structured: Option<FileWitnessInput>,
    opaque: Option<ActualHelperInput>,
}

impl FilePublicationInputs {
    pub fn new(structured: Option<FileWitnessInput>, opaque: Option<ActualHelperInput>) -> Self {
        Self { structured, opaque }
    }
    pub fn structured(&self) -> Option<&FileWitnessInput> { self.structured.as_ref() }
    pub fn opaque(&self) -> Option<&ActualHelperInput> { self.opaque.as_ref() }

    fn requires_compact_prefix(&self) -> bool {
        self.structured.as_ref().and_then(|input| input.admitted_close)
            .is_some_and(|marker| marker.final_sequence > MAX_REPLAY_PREFIX)
    }

    pub fn materialize(&self) -> Result<PublicationInputs, Error> {
        Ok(PublicationInputs {
            structured: self.structured.as_ref().map(|input| {
                input.witness_frontiers().map(|frontiers| (input.snapshot.clone(), frontiers))
            }).transpose()?,
            opaque: self.opaque.clone(),
        })
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> { codec::encode_inputs(self) }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> { codec::decode_inputs(bytes) }
}

/// Original capture recipe. Replay recaptures its exact dependencies from this
/// retained image, not an asserted success bit, serialized permit or latest input.
/// The journal consumer binds it to its own controller-produced frozen action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePublicationEvidence {
    original: FilePublicationInputs,
    requests: Vec<WitnessRequest>,
}

impl FilePublicationEvidence {
    pub fn new(original: FilePublicationInputs, requests: Vec<WitnessRequest>) -> Result<Self, Error> {
        if requests.len() > MAX_WITNESSES { return Err(Error::Limit); }
        if original.structured.is_none() && !requests.is_empty() { return Err(Error::Binding); }
        if original.structured.is_none() && original.opaque.is_none() { return Err(Error::Incomplete); }
        if let Some(input) = &original.structured {
            WitnessJudgment::capture(&input.snapshot, &input.witness_frontiers()?, requests.clone())?;
        }
        Ok(Self { original, requests })
    }

    pub fn original(&self) -> &FilePublicationInputs { &self.original }
    pub fn requests(&self) -> &[WitnessRequest] { &self.requests }

    /// A trusted assertion that these observations supported review of this
    /// exact action. This operation does not run a helper or mint effect rights.
    pub fn capture_for(&self, action: FrozenAction) -> Result<PublicationJudgment, Error> {
        let structured = self.original.structured.as_ref().map(|input| {
            WitnessJudgment::capture(&input.snapshot, &input.witness_frontiers()?, self.requests.clone())
        }).transpose()?;
        let opaque = self.original.opaque.as_ref().map(|input| OpaqueJudgment::capture(input, b""));
        PublicationJudgment::bind(action, structured, opaque)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> { codec::encode_evidence(self) }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> { codec::decode_evidence(bytes) }
}
