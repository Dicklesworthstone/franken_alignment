//! Reuse original review completion, disclosure planning and application.
use super::{Machine, Transition, Event, DecoderEvent, LearnedEvent, SidecarEvent};
use super::super::super::super::decoder::learned::sidecar::FileLearnedSidecarFinish;
use super::super::super::super::super::codec::shared::Writer;
use crate::action::{ActionState, ElapsedTick};
use crate::action::consequence::activation::monitor::decoder::sampled::host::replay::error_tag;
use crate::action::consequence::oversight::{replay::ObservedDecisionArchive,
    sidecar::SidecarRefinementOutcome};
use crate::{Error, Snapshot};
use std::rc::Rc;

impl Machine {
    pub(super) fn apply_learned_sidecar_finish(&mut self, event: &SidecarEvent) -> Result<Transition, Error> {
        let SidecarEvent::Finish { attempt, actor_revision, input_revision, round,
            allow_refinement, snapshot, expected } = event else { return Err(Error::Binding); };
        let result = self.execute_learned_sidecar_finish(*attempt, *actor_revision,
            *input_revision, *round, *allow_refinement, snapshot)?;
        if self.learned_sidecar_finish_witness(*attempt, &result)?.as_slice() != expected.as_ref() {
            return Err(Error::Binding);
        }
        Ok(Transition::LearnedSidecarFinished(Box::new(result)))
    }

    pub(in super::super::super::super) fn prepare_learned_sidecar_finish(&mut self, attempt: u64,
        actor_revision: u64, input_revision: u64, round: u64, allow_refinement: bool,
        snapshot: Snapshot) -> Result<(SidecarEvent, FileLearnedSidecarFinish), Error>
    {
        let shape = Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(SidecarEvent::Finish {
            attempt, actor_revision, input_revision, round, allow_refinement,
            snapshot: snapshot.clone(), expected: Rc::from(&b""[..]),
        })));
        self.check_decoder_admission(&shape)?;
        self.check_consistency_route(&shape)?;
        let result = self.execute_learned_sidecar_finish(attempt, actor_revision,
            input_revision, round, allow_refinement, &snapshot)?;
        let expected = self.learned_sidecar_finish_witness(attempt, &result)?.into();
        self.requests.refresh(&self.broker.inspect())?;
        self.bootstrap = None;
        Ok((SidecarEvent::Finish { attempt, actor_revision, input_revision, round,
            allow_refinement, snapshot, expected }, result))
    }

    fn execute_learned_sidecar_finish(&mut self, attempt: u64, actor_revision: u64,
        input_revision: u64, round: u64, allow_refinement: bool, snapshot: &Snapshot)
        -> Result<FileLearnedSidecarFinish, Error>
    {
        let original = self.checked_learned_sidecar(attempt)?;
        if original.actor_revision() != actor_revision || original.input_revision() != input_revision {
            return Err(Error::Stale);
        }
        let session = self.sessions.get(&round).ok_or(Error::Missing)?;
        if session.0 != attempt { return Err(Error::Binding); }
        if session.1.inputs() != original.round().input() { return Err(Error::Stale); }
        let now = self.now()?;
        let review = self.sessions.get_mut(&round).ok_or(Error::Missing)?.1.finish(now)?;
        self.sessions.remove(&round);
        let archive = review.replay_archive();
        let original = self.learned.as_mut().ok_or(Error::Incomplete)?.sidecars.get_mut(&attempt).ok_or(Error::Missing)?;
        let outcome = if allow_refinement { Some(self.broker.refine_learned_sidecar(original, &review)?) }
            else { None };
        if let Some(SidecarRefinementOutcome::Refined { group, .. }) = &outcome {
            let input_revision = original.input_revision();
            // The richer packet must also satisfy every independently required
            // source contract before this private candidate can be persisted.
            self.checked_learned_sidecar(attempt)?;
            return Ok(FileLearnedSidecarFinish::Refined { group: *group, input_revision, archive });
        }
        let current = original.round().input().clone();
        let receipt = self.broker.apply_review(review, Some(&current), snapshot);
        Ok(FileLearnedSidecarFinish::Applied { outcome, receipt, archive })
    }

    // Comparison bytes only. The original archive and packet are reconstructed
    // from source and original accepted events before this equality is checked.
    fn learned_sidecar_finish_witness(&self, attempt: u64, result: &FileLearnedSidecarFinish)
        -> Result<Vec<u8>, Error>
    {
        let mut w = Writer::new(super::super::super::super::decoder::MAX_WITNESS_BYTES);
        w.raw(b"FALSFIN\x01")?;
        write_archive(&mut w, result.archive())?;
        let original = self.learned.as_ref().ok_or(Error::Incomplete)?.sidecars.get(&attempt).ok_or(Error::Missing)?;
        w.u64(original.input_revision())?; w.blob(original.round().payload())?;
        let work = original.round().work();
        w.count(work.rounds)?; w.count(work.residual_bytes)?; w.count(work.committee_bytes)?;
        match result {
            FileLearnedSidecarFinish::Refined { group, input_revision, .. } => {
                w.u8(0)?; super::super::super::super::decoder::learned::sidecar::write_group(&mut w, *group)?;
                w.u64(*input_revision)?;
            }
            FileLearnedSidecarFinish::Applied { outcome, receipt, .. } => {
                w.u8(1)?;
                match outcome {
                    None => w.u8(0)?,
                    Some(SidecarRefinementOutcome::Final) => w.u8(1)?,
                    Some(SidecarRefinementOutcome::Missing { members }) => { w.u8(2)?; write_names(&mut w, members)?; }
                    Some(SidecarRefinementOutcome::BudgetExhausted { members }) => { w.u8(3)?; write_names(&mut w, members)?; }
                    Some(SidecarRefinementOutcome::Unresolved { members }) => { w.u8(4)?; write_names(&mut w, members)?; }
                    Some(SidecarRefinementOutcome::Refined { .. }) => return Err(Error::Binding),
                }
                match receipt {
                    Err(error) => { w.u8(0)?; w.u8(error_tag(*error))?; }
                    Ok(receipt) => {
                        w.u8(1)?; let control = &receipt.policy.control;
                        w.u64(control.sequence)?; w.u64(control.attempt)?;
                        w.raw(&control.decision.consequence.encode())?;
                        w.u8(state_tag(control.before))?; w.u8(state_tag(control.after))?;
                        w.u64(control.refunded_units)?; w.count(control.stopped.len())?;
                        for attempt in &control.stopped { w.u64(*attempt)?; }
                        w.u64(receipt.policy.snapshot_semantic_epoch)?;
                    }
                }
            }
        }
        Ok(w.finish())
    }
}

fn write_archive(w: &mut Writer, archive: &ObservedDecisionArchive) -> Result<(), Error> {
    w.u32(archive.version)?; w.blob(&archive.policy.to_bytes()?)?;
    super::super::super::super::views::write(w, archive.inputs.views())?;
    w.u64(archive.input_revision)?;
    for tick in [archive.window.commit_by, archive.window.reveal_by, archive.started_at,
        archive.reveals_opened_at, archive.completed_at] { w.u64(tick.0)?; }
    for times in [&archive.commit_times, &archive.reveal_times] {
        w.count(times.len())?;
        for ElapsedTick(tick) in times { w.u64(*tick)?; }
    }
    Ok(())
}
fn write_names(w: &mut Writer, names: &[String]) -> Result<(), Error> {
    w.count(names.len())?;
    for name in names { w.blob(name.as_bytes())?; }
    Ok(())
}
fn state_tag(state: ActionState) -> u8 {
    match state {
        ActionState::Proposed => 0, ActionState::Prepared => 1, ActionState::Reviewing => 2,
        ActionState::Authorized => 3, ActionState::Dispatching => 4, ActionState::Confirmed => 5,
        ActionState::Denied => 6, ActionState::Cancelled => 7, ActionState::Unknown => 8,
        ActionState::ConfirmedNotExecuted => 9, ActionState::IrrecoverablyUnknown => 10,
    }
}
