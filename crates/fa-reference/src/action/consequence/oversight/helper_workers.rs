//! Per-member helper workers feeding the ORIGINAL frozen congress.
//!
//! Each worker sees only its own exact view and has one bounded response slot.
//! The coordinator cannot accept replacement membership or skip a missing vote.
//! This profile uses round::commitment's non-cryptographic comparison oracle;
//! it does not authenticate a model, prove independent inference or mint rights.

pub mod io;
pub mod wire;

use super::{MAX_COMMITTEE_BYTES, ObservedReview, ObservedSession};
use crate::action::ElapsedTick;
use crate::evidence_view::EvidenceViewManifest;
use crate::reducer::MAX_VOTES;
use crate::round::{Digest, Verdict, commitment};
use crate::Error;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

pub const MAX_WORKER_SALT_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelperLimits {
    pub members: usize,
    pub input_bytes: usize,
    pub salt_bytes: usize,
}

impl Default for HelperLimits {
    fn default() -> Self {
        Self { members: MAX_VOTES, input_bytes: MAX_COMMITTEE_BYTES, salt_bytes: MAX_WORKER_SALT_BYTES }
    }
}

/// A single authorized input, never the entire committee or a frozen action's
/// private policy witnesses. Cloning this immutable request copies no authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HelperRequest {
    round: u64,
    member: String,
    evidence_root: [u8; 32],
    view: EvidenceViewManifest,
}

impl HelperRequest {
    pub fn round(&self) -> u64 { self.round }
    pub fn member(&self) -> &str { &self.member }
    pub fn evidence_root(&self) -> &[u8; 32] { &self.evidence_root }
    pub fn view(&self) -> &EvidenceViewManifest { &self.view }

    /// Reference protocol only, not a collision-resistant commitment.
    pub fn commitment(&self, verdict: Verdict, salt: &[u8]) -> Result<Digest, Error> {
        commitment(self.round, &self.member, &self.evidence_root, verdict, salt)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperPhase {
    AwaitCommit,
    CommitQueued,
    AwaitReveal,
    ReadyReveal,
    RevealQueued,
    Complete,
    Failed,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelperFailure {
    Disconnected,
    CommitDeadline,
    RevealDeadline,
    Rejected(Error),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HelperStatus {
    pub phase: HelperPhase,
    pub committed: bool,
    pub revealed: bool,
    pub failure: Option<HelperFailure>,
}

enum Reply {
    Commit(Digest),
    Reveal { verdict: Verdict, salt: Vec<u8> },
}

struct Slot {
    status: HelperStatus,
    pending: Option<Reply>,
}

impl Slot {
    fn fail(&mut self, failure: HelperFailure) {
        // No later transport event rewrites a previously accepted vote.
        if matches!(self.status.phase, HelperPhase::Complete | HelperPhase::Failed | HelperPhase::Closed) { return; }
        self.pending = None;
        self.status.phase = HelperPhase::Failed;
        self.status.failure = Some(failure);
    }
}

/// Provision exactly one of these to each worker. There is no Clone, member
/// selector, peer-vote inspection, session getter or conversion to a broker.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::oversight::{OversightBroker, helper_workers::HelperPort};
/// fn elevate(worker: HelperPort) -> OversightBroker { worker }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::helper_workers::HelperPort;
/// fn copy(worker: HelperPort) { let _other = worker.clone(); }
/// ```
pub struct HelperPort {
    request: HelperRequest,
    slot: Rc<RefCell<Slot>>,
    salt_limit: usize,
}

impl fmt::Debug for HelperPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HelperPort").field("member", &self.request.member()).finish_non_exhaustive()
    }
}

impl HelperPort {
    pub fn request(&self) -> &HelperRequest { &self.request }
    pub fn phase(&self) -> HelperPhase {
        self.slot.try_borrow().map_or(HelperPhase::Closed, |slot| slot.status.phase)
    }

    /// The coordinator supplies receipt time; a helper cannot backdate a reply.
    pub fn submit_commitment(&self, digest: Digest) -> Result<(), Error> {
        let mut slot = self.slot.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if slot.status.phase != HelperPhase::AwaitCommit { return Err(Error::WrongState); }
        slot.pending = Some(Reply::Commit(digest));
        slot.status.phase = HelperPhase::CommitQueued;
        Ok(())
    }

    pub fn reveal(&self, verdict: Verdict, salt: &[u8]) -> Result<(), Error> {
        if salt.len() > self.salt_limit { return Err(Error::Limit); }
        let mut slot = self.slot.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if slot.status.phase != HelperPhase::ReadyReveal { return Err(Error::WrongState); }
        slot.pending = Some(Reply::Reveal { verdict, salt: salt.to_vec() });
        slot.status.phase = HelperPhase::RevealQueued;
        Ok(())
    }

    pub fn disconnect(&self) {
        if let Ok(mut slot) = self.slot.try_borrow_mut() { slot.fail(HelperFailure::Disconnected); }
    }
}

impl Drop for HelperPort {
    fn drop(&mut self) { self.disconnect(); }
}

/// Owns the one original session, not a replacement reducer. Returned ports are
/// preassigned to the complete frozen roster and are never reissued or swapped.
/// Clocks, provider identity, process containment and scheduling belong to the host.
pub struct HelperRound {
    session: ObservedSession,
    slots: BTreeMap<String, Rc<RefCell<Slot>>>,
    elapsed: ElapsedTick,
    revealing: bool,
    finished: bool,
}

impl HelperRound {
    pub fn new(session: ObservedSession, limits: HelperLimits) -> Result<(Self, BTreeMap<String, HelperPort>), Error> {
        if limits.members == 0 || limits.input_bytes == 0 || limits.salt_bytes == 0 { return Err(Error::InvalidInput); }
        if limits.members > MAX_VOTES || limits.input_bytes > MAX_COMMITTEE_BYTES
            || limits.salt_bytes > MAX_WORKER_SALT_BYTES { return Err(Error::Limit); }
        let (round, evidence_root) = session.worker_identity()?;
        if session.inputs().views().len() > limits.members { return Err(Error::Limit); }
        // Validate the entire retained logical closure before cloning any views.
        // Its existing accounting includes member names, profile and input bytes.
        if session.inputs().logical_bytes() > limits.input_bytes { return Err(Error::Limit); }
        let mut slots = BTreeMap::new();
        let mut ports = BTreeMap::new();
        for (member, view) in session.inputs().views() {
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
        let elapsed = session.elapsed();
        Ok((Self { session, slots, elapsed, revealing: false, finished: false }, ports))
    }

    pub fn elapsed(&self) -> ElapsedTick { self.elapsed }

    /// Supervisor-only health information. It is not a vote or permission.
    pub fn statuses(&self) -> BTreeMap<String, HelperStatus> {
        self.slots.iter().map(|(member, slot)| (member.clone(), slot.borrow().status)).collect()
    }

    /// At most one queued reply per frozen member is consumed per invocation.
    /// No helper-supplied time, extra roster member, deadline extension or early
    /// permissive finish can enter the original congress through this operation.
    pub fn advance(&mut self, now: ElapsedTick) -> Result<(), Error> {
        if self.finished { return Err(Error::WrongState); }
        if now < self.elapsed { return Err(Error::Stale); }
        self.elapsed = now;
        let window = self.session.window();
        for (member, shared) in &self.slots {
            let mut slot = shared.borrow_mut();
            if matches!(slot.status.phase, HelperPhase::Complete | HelperPhase::Failed | HelperPhase::Closed) { continue; }
            if !slot.status.committed && now >= window.commit_by {
                slot.fail(HelperFailure::CommitDeadline);
                continue;
            }
            if now >= window.reveal_by {
                slot.fail(HelperFailure::RevealDeadline);
                continue;
            }
            let Some(reply) = slot.pending.take() else { continue; };
            match reply {
                Reply::Commit(digest) => match self.session.commit_from_worker(member, digest, now) {
                    Ok(()) => { slot.status.committed = true; slot.status.phase = HelperPhase::AwaitReveal; }
                    Err(error) => slot.fail(HelperFailure::Rejected(error)),
                },
                Reply::Reveal { verdict, salt } => match self.session.reveal(member, verdict, &salt, now) {
                    Ok(()) => { slot.status.revealed = true; slot.status.phase = HelperPhase::Complete; }
                    Err(error) => slot.fail(HelperFailure::Rejected(error)),
                },
            }
        }
        if !self.revealing {
            let all_committed = self.slots.values().all(|slot| slot.borrow().status.committed);
            if all_committed || now >= window.commit_by {
                self.session.open_reveals(now)?;
                self.revealing = true;
                for shared in self.slots.values() {
                    let mut slot = shared.borrow_mut();
                    if slot.status.phase == HelperPhase::AwaitReveal {
                        slot.status.phase = HelperPhase::ReadyReveal;
                    }
                }
            }
        }
        Ok(())
    }

    /// Returns the original authority-bound review, with missing workers still
    /// in its denominator. The broker must apply it against fresh current state.
    /// A premature attempt retains the round and its pending obligations.
    pub fn finish(&mut self, now: ElapsedTick) -> Result<ObservedReview, Error> {
        self.advance(now)?;
        match self.session.finish(now) {
            Ok(review) => { self.finished = true; self.close_ports(); Ok(review) }
            Err(Error::Incomplete) => Err(Error::Incomplete),
            Err(error) => { self.finished = true; self.close_ports(); Err(error) }
        }
    }

    fn close_ports(&self) {
        for shared in self.slots.values() {
            let mut slot = shared.borrow_mut();
            slot.pending = None;
            if !matches!(slot.status.phase, HelperPhase::Complete | HelperPhase::Failed) {
                slot.status.phase = HelperPhase::Closed;
            }
        }
    }
}

impl Drop for HelperRound {
    fn drop(&mut self) { self.close_ports(); }
}
