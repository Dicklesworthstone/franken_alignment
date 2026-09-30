//! Acknowledged actual-model measurements, recomputed rather than imported on replay.
mod preparation;
pub(in super::super) use preparation::IdentityComputation;
use super::{Event, FileIdentityChallenge, FileIdentityObservation, FileIdentityObserver,
    FileOversight, IdentityEvent, JournalError, Machine, ModelManifest, Transition, journal};
use super::super::decoder::MAX_WITNESS_BYTES;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::identity::{decoder::IdentityProbeWork, wire};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use crate::action::consequence::delivery::persistent::{JournalFailure, JournalIo,
    codec::shared::{Reader, Writer}};
use crate::Error;
use std::rc::Rc;

/// All stimuli come from the existing challenge; the model comes from the owner.
/// Manifest fields remain independently observed data, not computed attestation.
#[derive(Clone, Debug)]
pub struct FileLearnedIdentityInput {
    pub measurement_sequence: u64,
    pub budget: DecoderBudget,
    pub observed_manifest: ModelManifest,
}

/// Returned only after canonical acknowledgment. Matching still needs the
/// separate original identity installation, congress, and both effect keys.
/// Work excludes reconstruction of the preceding journal and filesystem costs.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::delivery::persistent::observed::identity::FileComputedIdentityObservation;
/// fn authorize(observed: FileComputedIdentityObservation) -> Permit { observed }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileComputedIdentityObservation {
    pub observation: FileIdentityObservation,
    pub work: IdentityProbeWork,
    pub started_at: ElapsedTick,
    pub completed_at: ElapsedTick,
}

#[derive(Clone)]
pub(in super::super) struct ComputedIdentityEvent {
    pub check: u64,
    pub input: FileLearnedIdentityInput,
    pub started_at: ElapsedTick,
    pub completed_at: ElapsedTick,
    pub witness: Rc<[u8]>,
}
impl ComputedIdentityEvent {
    pub(in super::super) fn validate(&self) -> Result<(), Error> {
        if self.check == 0 || self.input.measurement_sequence == 0 { return Err(Error::InvalidInput); }
        if self.input.budget.scalar_products > MAX_DECODER_PRODUCTS { return Err(Error::Limit); }
        if self.completed_at < self.started_at { return Err(Error::Stale); }
        wire::decode_manifest(&wire::encode_manifest(&self.input.observed_manifest))?;
        Ok(())
    }
}

impl FileIdentityObserver {
    /// Execute the complete bounded stimulus roster against THIS learned owner,
    /// then record its original identity observations in one acknowledged event.
    /// The challenge must already have begun and withdrawn prior eligibility.
    /// No cached frame, surrogate model, anchor subset or verdict is accepted.
    ///
    /// Clock is sampled before and after original inference. Crossing the native
    /// deadline withdraws the check instead of installing a timely-looking prefix.
    /// This synchronous whole-roster operation is NOT preemptible. The existing
    /// token-stepped learned_decoder_probe remains available for cooperative use.
    ///
    /// A persisted failure consumes this challenge's computed operation. Errors
    /// or unwinds after admission poison the owner; no candidate observation is
    /// released and recovery withdraws the old challenge. Exact replay recomputes
    /// the original model, frames and work before comparing the saved witness.
    pub fn observe_computed_learned<F>(&self, host: &mut FileOversight, revision: u64,
        challenge: &FileIdentityChallenge, input: FileLearnedIdentityInput, mut clock: F)
        -> Result<FileComputedIdentityObservation, JournalError>
    where F: FnMut() -> ElapsedTick {
        self.check(host, challenge)?;
        if revision != host.revision() { return Err(Error::Stale.into()); }
        wire::decode_manifest(&wire::encode_manifest(&input.observed_manifest))?;
        // Preflight creates no session, executes no token, and writes nothing.
        host.machine.preflight_computed_identity(challenge.id(), &input)?;
        if host.events.len() >= host.profile.delivery.limits.events { return Err(Error::Limit.into()); }
        host.events.try_reserve(1).map_err(|_| Error::Limit)?;
        let started_at = clock();
        if started_at < host.inspect().control.ledger.elapsed.ok_or(Error::Incomplete)? {
            return Err(Error::Stale.into());
        }
        let event = ComputedIdentityEvent { check: challenge.id(), input, started_at,
            completed_at: started_at, witness: Rc::from(&b""[..]) };
        event.validate()?;
        host.check_source_admission(&Event::Identity(IdentityEvent::Computed(event.clone())))?;
        let mut candidate = Machine::replay(&host.profile, &host.events)?;
        host.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: std::io::ErrorKind::Other, replacement_may_be_visible: false });
        let (event, result) = candidate.prepare_computed_identity(event, &mut clock)?;
        let event = Event::Identity(IdentityEvent::Computed(event));
        let bytes = journal::encode_appended(&host.profile, host.store.identity(), &host.events, &event)?;
        host.persist_candidate(event, bytes, candidate, Transition::Unit)?;
        Ok(result)
    }
}

pub(in super::super) fn write(w: &mut Writer, event: &ComputedIdentityEvent) -> Result<(), Error> {
    event.validate()?;
    if event.witness.is_empty() { return Err(Error::Incomplete); }
    if event.witness.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
    w.u64(event.check)?; w.u64(event.input.measurement_sequence)?;
    w.u64(event.input.budget.scalar_products)?;
    w.blob(&wire::encode_manifest(&event.input.observed_manifest))?;
    w.u64(event.started_at.0)?; w.u64(event.completed_at.0)?; w.blob(&event.witness)
}
pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<ComputedIdentityEvent, Error> {
    let check = r.u64()?; let measurement_sequence = r.u64()?;
    let budget = DecoderBudget { scalar_products: r.u64()? };
    let observed_manifest = wire::decode_manifest(r.blob(wire::MANIFEST_BYTES)?)?;
    let started_at = ElapsedTick(r.u64()?); let completed_at = ElapsedTick(r.u64()?);
    let witness = Rc::from(r.blob(MAX_WITNESS_BYTES)?);
    let event = ComputedIdentityEvent { check,
        input: FileLearnedIdentityInput { measurement_sequence, budget, observed_manifest },
        started_at, completed_at, witness };
    event.validate()?;
    if event.witness.is_empty() { return Err(Error::Incomplete); }
    Ok(event)
}
