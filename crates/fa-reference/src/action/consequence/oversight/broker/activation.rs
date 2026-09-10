//! Optional mandatory activation lane in the existing owning oversight broker.
//! Current capture metadata and values remain trusted host inputs. This is a
//! fixed linear tripwire, not an activation classifier claiming harmful intent.

use super::OversightBroker;
use crate::action::ActionState;
use crate::action::consequence::activation::SourceFrame;
use crate::action::consequence::activation::monitor::{MonitorOutcome, RefinementMonitor, RefinementReport};
use crate::action::consequence::activation::probe::SCORE_WORDS;
use crate::Error;
use std::collections::BTreeMap;
use std::rc::Rc;

const MAX_CAPTURES: usize = 512;
const MAX_RETAINED_SCORE_WORDS: usize = 1_048_576;

#[derive(Debug)]
struct CaptureRecord {
    input_revision: u64,
    actor_revision: u64,
    policy_epoch: u64,
    report: Rc<RefinementReport>,
}

#[derive(Debug)]
pub(super) struct ActivationState {
    monitor: RefinementMonitor,
    stream: u64,
    max_captures: usize,
    captures: usize,
    last_sequence: u64,
    score_words: usize,
    records: BTreeMap<u64, CaptureRecord>,
}

impl OversightBroker {
    /// Freeze this requirement before any proposal. No disable, weight-edit or
    /// budget-widening operation is exposed. The configured stream must belong
    /// to this actor's trusted capture boundary; this is not authentication.
    pub fn enable_activation_tripwire(
        &mut self, monitor: RefinementMonitor, stream: u64, max_captures: usize,
    ) -> Result<(), Error> {
        if self.activation.is_some() { return Err(Error::Duplicate); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() || self.inspect().sequence != 0 {
            return Err(Error::WrongState);
        }
        if stream == 0 || max_captures == 0 { return Err(Error::InvalidInput); }
        if max_captures > MAX_CAPTURES { return Err(Error::Limit); }
        if monitor.profile().model_generation != self.delivery.controller().actor().profile().model_generation {
            return Err(Error::Binding);
        }
        self.activation = Some(ActivationState { monitor, stream, max_captures, captures: 0,
            last_sequence: 0, score_words: 0, records: BTreeMap::new() });
        Ok(())
    }

    pub fn activation_tripwire_required(&self) -> bool { self.activation.is_some() }

    /// The host retains both expected revisions while capturing. This profile
    /// observes the last token of the supplied full-prefix actor state only.
    /// Each admitted capture advances the input-basis revision, so old reviews
    /// and human requests cannot approve a different activation observation.
    ///
    /// After context validation, failures deliberately leave the action held:
    /// invalidate the old observation BEFORE capacity checks or numerical work.
    /// Call input_revision() after an error to obtain the new basis revision.
    pub fn record_activation(
        &mut self, attempt: u64, expected_input_revision: u64,
        expected_actor_revision: u64, source: &SourceFrame,
    ) -> Result<Rc<RefinementReport>, Error> {
        let state = self.activation.as_ref().ok_or(Error::Incomplete)?;
        let slot = self.inputs.get(&attempt).ok_or(Error::Missing)?;
        if slot.revision != expected_input_revision || self.actor_revision() != expected_actor_revision {
            return Err(Error::Stale);
        }
        if slot.current.is_none() { return Err(Error::Incomplete); }
        let inspection = self.inspect();
        if inspection.suspended
            || !matches!(inspection.ledger.stages.get(&attempt), Some(ActionState::Reviewing | ActionState::Authorized))
        {
            return Err(Error::WrongState);
        }
        if slot.action.spec().policy_epoch != inspection.ledger.epoch
            || inspection.ledger.elapsed.ok_or(Error::Incomplete)? >= slot.action.spec().deadline
        {
            return Err(Error::Stale);
        }
        let last_position = self.delivery.controller().actor().next_position().checked_sub(1).ok_or(Error::Incomplete)?;
        let identity = source.identity();
        if identity.profile != state.monitor.profile() || identity.stream != state.stream
            || identity.profile.tenant != slot.action.spec().scope.tenant
            || identity.position != last_position || source.dimensions() != state.monitor.dimensions()
        {
            return Err(Error::Binding);
        }
        if identity.sequence <= state.last_sequence { return Err(Error::Stale); }
        let revision = slot.revision.checked_add(1).ok_or(Error::Overflow)?;
        let slot = self.inputs.get_mut(&attempt).expect("validated input slot");
        slot.revision = revision;
        slot.approved = None;
        let state = self.activation.as_mut().expect("configured activation lane");
        state.records.remove(&attempt);
        state.last_sequence = identity.sequence;
        if state.captures >= state.max_captures { return Err(Error::Limit); }
        state.captures += 1;
        let report = state.monitor.analyze(source)?;
        let words = report.steps().iter().map(|step| step.observations.len() * 2 * SCORE_WORDS).sum::<usize>();
        let retained = state.score_words.checked_add(words).ok_or(Error::Limit)?;
        if retained > MAX_RETAINED_SCORE_WORDS { return Err(Error::Limit); }
        let report = Rc::new(report);
        state.records.insert(attempt, CaptureRecord { input_revision: revision,
            actor_revision: expected_actor_revision, policy_epoch: inspection.ledger.epoch,
            report: Rc::clone(&report) });
        state.score_words = retained;
        Ok(report)
    }

    /// Report a capture outage even when no valid SourceFrame could be built.
    /// Reservations and external effects are never refunded by observation loss.
    pub fn activation_unavailable(&mut self, attempt: u64, expected_revision: u64) -> Result<u64, Error> {
        let state = self.activation.as_mut().ok_or(Error::Incomplete)?;
        let slot = self.inputs.get_mut(&attempt).ok_or(Error::Missing)?;
        if slot.revision != expected_revision { return Err(Error::Stale); }
        if !state.records.contains_key(&attempt) { return Ok(slot.revision); }
        let revision = slot.revision.checked_add(1).ok_or(Error::Overflow)?;
        state.records.remove(&attempt);
        slot.revision = revision;
        slot.approved = None;
        Ok(revision)
    }

    /// Historical numerical evidence; presence is not a freshness certificate.
    /// Every positive effect boundary calls check_activation again.
    pub fn activation_report(&self, attempt: u64) -> Result<Option<&RefinementReport>, Error> {
        let state = self.activation.as_ref().ok_or(Error::Incomplete)?;
        self.inputs.get(&attempt).ok_or(Error::Missing)?;
        Ok(state.records.get(&attempt).map(|record| record.report.as_ref()))
    }

    pub(super) fn check_activation(&self, attempt: u64) -> Result<(), Error> {
        let Some(state) = &self.activation else { return Ok(()); };
        let slot = self.inputs.get(&attempt).ok_or(Error::Missing)?;
        let record = state.records.get(&attempt).ok_or(Error::Incomplete)?;
        if record.input_revision != slot.revision || record.actor_revision != self.actor_revision()
            || record.policy_epoch != self.inspect().ledger.epoch
        {
            return Err(Error::Stale);
        }
        if record.report.outcome() != MonitorOutcome::NoAlarm { return Err(Error::Incomplete); }
        Ok(())
    }
}
