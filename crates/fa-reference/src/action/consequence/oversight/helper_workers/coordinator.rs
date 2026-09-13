//! Shared bounded worker-slot coordination. It forwards to the original native
//! session or a durable adapter; it NEVER reduces votes or creates a review.
//! Backend failure is not a missing vote and cannot trigger a permissive fallback.
use super::{HelperFailure, HelperLimits, HelperPhase, HelperPort, HelperRequest, HelperStatus,
    MAX_COMMITTEE_BYTES, MAX_VOTES, MAX_WORKER_SALT_BYTES, Reply, Slot};
use crate::action::ElapsedTick;
use crate::action::consequence::oversight::{CommitteeInput, ReviewWindow};
use crate::round::{Digest, Verdict};
use crate::Error;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub(crate) enum AdvanceError<E> { Protocol(Error), Backend(E) }

/// A crate-owned bridge, not an extension point for caller-written evaluators.
/// Only accepted original transitions may return Ok. For a durable adapter that
/// means its commit barrier completed before a slot changes phase.
pub(crate) trait Session {
    type Failure;
    fn observe(&mut self, now: ElapsedTick) -> Result<(), Self::Failure>;
    fn commit(&mut self, member: &str, digest: Digest, now: ElapsedTick) -> Result<(), AdvanceError<Self::Failure>>;
    fn open(&mut self, now: ElapsedTick) -> Result<(), AdvanceError<Self::Failure>>;
    fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8], now: ElapsedTick) -> Result<(), AdvanceError<Self::Failure>>;
}

pub(crate) struct Coordinator {
    slots: BTreeMap<String, Rc<RefCell<Slot>>>,
    window: ReviewWindow,
    elapsed: ElapsedTick,
    revealing: bool,
    closed: bool,
}

impl Coordinator {
    pub(crate) fn new(round: u64, evidence_root: [u8; 32], inputs: &CommitteeInput,
        window: ReviewWindow, elapsed: ElapsedTick, limits: HelperLimits)
        -> Result<(Self, BTreeMap<String, HelperPort>), Error>
    {
        if limits.members == 0 || limits.input_bytes == 0 || limits.salt_bytes == 0 { return Err(Error::InvalidInput); }
        if limits.members > MAX_VOTES || limits.input_bytes > MAX_COMMITTEE_BYTES
            || limits.salt_bytes > MAX_WORKER_SALT_BYTES { return Err(Error::Limit); }
        if inputs.views().len() > limits.members || inputs.logical_bytes() > limits.input_bytes { return Err(Error::Limit); }
        let mut slots = BTreeMap::new();
        let mut ports = BTreeMap::new();
        for (member, view) in inputs.views() {
            let slot = Rc::new(RefCell::new(Slot {
                status: HelperStatus { phase: HelperPhase::AwaitCommit, committed: false, revealed: false, failure: None },
                pending: None,
            }));
            ports.insert(member.clone(), HelperPort {
                request: HelperRequest { round, member: member.clone(), evidence_root, view: view.clone() },
                slot: Rc::clone(&slot), salt_limit: limits.salt_bytes,
            });
            slots.insert(member.clone(), slot);
        }
        Ok((Self { slots, window, elapsed, revealing: false, closed: false }, ports))
    }

    pub(crate) fn elapsed(&self) -> ElapsedTick { self.elapsed }
    pub(crate) fn statuses(&self) -> BTreeMap<String, HelperStatus> {
        self.slots.iter().map(|(member, slot)| (member.clone(), slot.borrow().status)).collect()
    }

    /// Preserve the original one-reply-per-member iteration and deadline rules.
    /// A protocol rejection marks only that slot; a backend failure escapes to
    /// the owner, which must stop I/O. Successful earlier commits are not undone.
    pub(crate) fn advance<S: Session>(&mut self, session: &mut S, now: ElapsedTick)
        -> Result<(), AdvanceError<S::Failure>>
    {
        if self.closed { return Err(AdvanceError::Protocol(Error::WrongState)); }
        if now < self.elapsed { return Err(AdvanceError::Protocol(Error::Stale)); }
        self.elapsed = now;
        session.observe(now).map_err(AdvanceError::Backend)?;
        for (member, shared) in &self.slots {
            let mut slot = shared.borrow_mut();
            if matches!(slot.status.phase, HelperPhase::Complete | HelperPhase::Failed | HelperPhase::Closed) { continue; }
            if !slot.status.committed && now >= self.window.commit_by {
                slot.fail(HelperFailure::CommitDeadline);
                continue;
            }
            if now >= self.window.reveal_by {
                slot.fail(HelperFailure::RevealDeadline);
                continue;
            }
            let Some(reply) = slot.pending.take() else { continue; };
            match reply {
                Reply::Commit(digest) => match session.commit(member, digest, now) {
                    Ok(()) => { slot.status.committed = true; slot.status.phase = HelperPhase::AwaitReveal; }
                    Err(AdvanceError::Protocol(error)) => slot.fail(HelperFailure::Rejected(error)),
                    Err(AdvanceError::Backend(error)) => return Err(AdvanceError::Backend(error)),
                },
                Reply::Reveal { verdict, salt } => match session.reveal(member, verdict, &salt, now) {
                    Ok(()) => { slot.status.revealed = true; slot.status.phase = HelperPhase::Complete; }
                    Err(AdvanceError::Protocol(error)) => slot.fail(HelperFailure::Rejected(error)),
                    Err(AdvanceError::Backend(error)) => return Err(AdvanceError::Backend(error)),
                },
            }
        }
        if !self.revealing {
            let all_committed = self.slots.values().all(|slot| slot.borrow().status.committed);
            if all_committed || now >= self.window.commit_by {
                session.open(now)?;
                self.revealing = true;
                for shared in self.slots.values() {
                    let mut slot = shared.borrow_mut();
                    if slot.status.phase == HelperPhase::AwaitReveal { slot.status.phase = HelperPhase::ReadyReveal; }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
        for shared in self.slots.values() {
            let mut slot = shared.borrow_mut();
            slot.pending = None;
            if !matches!(slot.status.phase, HelperPhase::Complete | HelperPhase::Failed) { slot.status.phase = HelperPhase::Closed; }
        }
    }
}
impl Drop for Coordinator {
    fn drop(&mut self) { self.close(); }
}
