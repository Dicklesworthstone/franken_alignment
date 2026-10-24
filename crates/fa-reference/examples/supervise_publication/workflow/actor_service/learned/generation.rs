//! Cooperative original prompt and non-forecast generation, without a new source.
//! Forecasted sampling remains exclusively in prediction::Session.
use super::{FileOversight, GenerationStatus, debug};
use super::prediction::MAX_REPLAY_EVENTS_PER_QUANTUM;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::{
    FileLearnedIntentPreparation, FileLearnedReplayContinuation,
    FileLearnedStepPreparation, FileLearnedStepPreparationStatus,
};
use fa_reference::action::consequence::oversight::learned_host::HostedLearnedInspection;

enum Preparation {
    Intent(FileLearnedIntentPreparation),
    Completion(FileLearnedStepPreparation),
}

/// Only private historical replay custody. No cloned model, live authority,
/// source writer, forecast or sampler is constructed by this scheduler.
/// The first operation starts cold without taking the live control replay cache.
/// Later operations move original acknowledged predecessors through their tails.
#[derive(Default)]
pub(super) struct Session {
    expected: Option<HostedLearnedInspection>,
    preparation: Option<Preparation>,
    carry: Option<FileLearnedReplayContinuation>,
    interrupted: bool,
}
impl Session {
    pub(super) fn new() -> Self { Self::default() }

    /// Return only this session's acknowledged historical custody when generic
    /// generation ends or hands sampling to the original forecast stepper.
    pub(super) fn retain_for_control(mut self, host: &mut FileOversight) -> Result<(), String> {
        if self.interrupted || self.preparation.is_some() {
            return Err("original generation replay is incomplete or interrupted".into());
        }
        let current = host.learned_generation_inspection().map_err(debug)?;
        if current.pending.is_some()
            || self.expected.as_ref().is_some_and(|expected| expected != &current.numerical) {
            return Err("original generation predecessor changed before replay custody transfer".into());
        }
        if let Some(carry) = self.carry.take() {
            let revision = host.revision();
            carry.retain_for_control(host, revision).map_err(debug)?;
        }
        Ok(())
    }

    /// At most 32 original historical reducer calls and ONE finalizer. Return
    /// after the intent ACK, before its numerical operation, even on a warm tail.
    /// The returned count excludes that finalizer and any previously verified
    /// prefix. One numerical operation and filesystem I/O remain synchronous.
    ///
    /// Fresh policy, time and stop processing belong to the existing outer
    /// operator loop. catch_up and both finalizers still enforce the original
    /// source, pause, capacity, owner, predecessor and forecast admission rules.
    /// No mutable host borrow survives a quantum.
    pub(super) fn advance_quantum(&mut self, host: &mut FileOversight) -> Result<usize, String> {
        if self.interrupted { return Err("original generation finalization was interrupted".into()); }
        let current = host.learned_generation_inspection().map_err(debug)?;
        if !current.numerical.status.is_active() || current.paused || !host.clock_ready() {
            return Err("original generation is not active under a current clock".into());
        }
        // Even an existing forecast cannot be driven through this generic lane.
        // The actual raw/owned handle must retain request, deadline and custody.
        if host.action_consistency_required() && current.numerical.status == GenerationStatus::Generating {
            return Err("forecasted output requires its original forecast stepper".into());
        }
        if self.expected.as_ref().is_some_and(|expected| expected != &current.numerical) {
            return Err("original generation predecessor changed between replay quanta".into());
        }
        let revision = host.revision();
        if self.preparation.is_none() {
            // A consuming preparation refusal cannot discard its provenance and
            // then silently retry cold. An unwind leaves this latch set as well.
            self.interrupted = true;
            let actor = current.numerical.actor_revision;
            let position = current.numerical.position;
            self.preparation = Some(match (current.pending.is_some(), self.carry.take()) {
                (false, Some(carry)) => Preparation::Intent(
                    carry.prepare_intent(host, revision, actor, position).map_err(debug)?),
                (false, None) => Preparation::Intent(
                    host.prepare_learned_step_intent(revision, actor, position).map_err(debug)?),
                (true, Some(carry)) => Preparation::Completion(
                    carry.prepare_completion(host, revision, actor, position).map_err(debug)?),
                (true, None) => Preparation::Completion(
                    host.prepare_learned_step_completion(revision, actor, position).map_err(debug)?),
            });
            self.expected = Some(current.numerical);
            self.interrupted = false;
        }
        let (before, progress) = match self.preparation.as_mut().expect("original preparation") {
            Preparation::Intent(task) => {
                let before = task.progress().replayed_events;
                (before, task.catch_up(host, revision, before, MAX_REPLAY_EVENTS_PER_QUANTUM).map_err(debug)?)
            }
            Preparation::Completion(task) => {
                let before = task.progress().replayed_events;
                (before, task.catch_up(host, revision, before, MAX_REPLAY_EVENTS_PER_QUANTUM).map_err(debug)?)
            }
        };
        let replayed = progress.replayed_events.checked_sub(before).ok_or("invalid original replay cursor")?;
        if progress.status != FileLearnedStepPreparationStatus::Ready { return Ok(replayed); }
        // The original finalizer performs its own full live preflight again.
        // Do not clear the latch on an outer storage/admission error or unwind.
        self.interrupted = true;
        match self.preparation.take().expect("verified original preparation") {
            Preparation::Intent(task) => {
                self.carry = Some(task.finish_with_continuation(host).map_err(debug)?);
            }
            Preparation::Completion(task) => {
                let (result, carry) = task.finish_with_continuation(host).map_err(debug)?;
                self.expected = Some(host.learned_generation_inspection().map_err(debug)?.numerical);
                self.carry = Some(carry);
                // A native refusal is acknowledged, but cannot be retried by
                // rebuilding an earlier quiet candidate inside this session.
                result.map_err(debug)?;
            }
        }
        self.interrupted = false;
        Ok(replayed)
    }
}
