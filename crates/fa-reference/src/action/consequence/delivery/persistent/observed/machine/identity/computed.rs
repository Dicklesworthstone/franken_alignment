//! One original numerical execution path for live preparation and journal replay.
use super::{FileIdentityObservation, IdentityChallenge, IdentityEvent, IdentityOutcome, Machine, Transition};
use super::super::super::{Event, identity::computed::{ComputedIdentityEvent,
    FileComputedIdentityObservation, FileLearnedIdentityInput}, decoder::MAX_WITNESS_BYTES};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::identity::decoder::{
    DecoderIdentityMeasurement, DecoderIdentityProbe, IdentityProbeProgress, IdentityProbeWork};
use crate::action::consequence::activation::monitor::decoder::sampled::host::replay::error_tag;
use crate::action::consequence::delivery::persistent::codec::shared::Writer;
use crate::Error;

impl Machine {
    pub(in super::super::super) fn preflight_computed_identity(&self, check: u64,
        input: &FileLearnedIdentityInput) -> Result<(IdentityChallenge, DecoderIdentityProbe), Error>
    {
        if !self.clock_ready { return Err(Error::Incomplete); }
        let challenge = self.identity_challenge(check)?;
        let control = self.broker.inspect();
        if self.broker.identity_basis()? != challenge.basis()
            || control.sequence != challenge.control_sequence()
            || control.ledger.epoch != challenge.revocation_epoch()
            || self.broker.actor_revision() != challenge.actor_revision() { return Err(Error::Stale); }
        if control.suspended || self.broker.identity_installation(check)?.is_some() { return Err(Error::WrongState); }
        let report = self.broker.identity_report(check)?;
        if report.outcome != IdentityOutcome::Collecting || report.manifest.is_some()
            || !report.observations.is_empty() { return Err(Error::WrongState); }
        let probe = self.broker.hosted_learned_identity_probe(challenge.actor_revision(),
            challenge.passport(), input.measurement_sequence, input.budget)?;
        Ok((challenge, probe))
    }

    pub(super) fn apply_computed_identity(&mut self, event: &ComputedIdentityEvent)
        -> Result<Transition, Error>
    {
        event.validate()?;
        let (result, witness) = self.execute_computed_identity(event, || event.completed_at)?;
        if witness.as_slice() != event.witness.as_ref() { return Err(Error::Binding); }
        Ok(Transition::IdentityObserved(Box::new(result.observation)))
    }

    pub(in super::super::super) fn prepare_computed_identity<F>(&mut self,
        mut event: ComputedIdentityEvent, clock: F)
        -> Result<(ComputedIdentityEvent, FileComputedIdentityObservation), Error>
    where F: FnMut() -> ElapsedTick {
        let shape = Event::Identity(IdentityEvent::Computed(event.clone()));
        self.check_decoder_admission(&shape)?;
        self.check_consistency_route(&shape)?;
        let (result, witness) = self.execute_computed_identity(&event, clock)?;
        event.completed_at = result.completed_at;
        event.witness = witness.into();
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok((event, result))
    }

    fn execute_computed_identity<F>(&mut self, event: &ComputedIdentityEvent, mut clock: F)
        -> Result<(FileComputedIdentityObservation, Vec<u8>), Error>
    where F: FnMut() -> ElapsedTick {
        let (challenge, mut probe) = self.preflight_computed_identity(event.check, &event.input)?;
        // Reserve before executing the first token. The original passport bounds
        // the complete number of measurements, not a caller-selected subset.
        let mut measured = Vec::new();
        measured.try_reserve_exact(challenge.passport().anchors().len()).map_err(|_| Error::Limit)?;
        let mut observation = observed(self.apply_identity_inner(&IdentityEvent::Manifest(event.check,
            event.input.observed_manifest.clone(), event.started_at))?)?;
        let collecting = observation.measurement.as_ref()
            .is_ok_and(|report| report.outcome == IdentityOutcome::Collecting);
        let mut numerical_failure = None;
        if collecting {
            for _ in 0..probe.work().planned_tokens {
                match probe.advance() {
                    Ok(IdentityProbeProgress::Advanced) => {}
                    Ok(IdentityProbeProgress::Measured(frame)) => measured.push(*frame),
                    Ok(IdentityProbeProgress::Complete) => { numerical_failure = Some(Error::Binding); break; }
                    Err(error) => { numerical_failure = Some(error); break; }
                }
            }
            if numerical_failure.is_none() && !probe.complete() { numerical_failure = Some(Error::Incomplete); }
        }
        let completed_at = clock();
        // A backwards or interrupted clock cannot publish the private candidate.
        // The live wrapper is already poisoned before entering this execution.
        self.observe(completed_at)?;
        if collecting {
            let refusal = if completed_at >= challenge.deadline() { Some(Error::Stale) }
                else { numerical_failure };
            if let Some(error) = refusal {
                self.apply_identity(&IdentityEvent::Unavailable(challenge.basis()))?;
                observation = FileIdentityObservation { measurement: Err(error), containment: None };
            } else {
                for frame in &measured {
                    observation = observed(self.apply_identity_inner(&IdentityEvent::Anchor(event.check,
                        frame.anchor(), frame.source().clone(), completed_at))?)?;
                    if !observation.measurement.as_ref().is_ok_and(|report|
                        matches!(report.outcome, IdentityOutcome::Collecting | IdentityOutcome::Matched)) { break; }
                }
            }
        }
        let work = probe.work();
        let witness = witness(work, numerical_failure, &measured)?;
        Ok((FileComputedIdentityObservation { observation, work,
            started_at: event.started_at, completed_at }, witness))
    }
}

fn observed(value: Transition) -> Result<FileIdentityObservation, Error> {
    match value { Transition::IdentityObserved(result) => Ok(*result), _ => Err(Error::Binding) }
}

// Comparison material only. No decoder exists for these saved frames or work.
// Replay constructs every value above before it tests byte-for-byte equality.
fn witness(work: IdentityProbeWork, failure: Option<Error>, frames: &[DecoderIdentityMeasurement])
    -> Result<Vec<u8>, Error>
{
    let mut w = Writer::new(MAX_WITNESS_BYTES);
    w.raw(b"FALIDP\0\x01")?;
    for value in [work.planned_tokens, work.planned_scalar_products, work.entered_tokens,
        work.entered_scalar_product_bound, work.completed_tokens, work.completed_scalar_products] { w.u64(value)?; }
    w.count(work.measured_anchors)?; w.count(work.measurement_bytes)?;
    match failure { None => w.u8(0)?, Some(error) => { w.u8(1)?; w.u8(error_tag(error))?; } }
    w.count(frames.len())?;
    for frame in frames { w.u64(frame.anchor())?; w.blob(&frame.source().encode_initial(23)?)?; }
    Ok(w.finish())
}
