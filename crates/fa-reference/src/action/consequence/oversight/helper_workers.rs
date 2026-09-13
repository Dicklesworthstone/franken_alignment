//! Per-member helper workers feeding the ORIGINAL frozen congress.
//!
//! Each worker sees only its own exact view and has one bounded response slot.
//! The coordinator cannot accept replacement membership or skip a missing vote.
//! This profile uses round::commitment's non-cryptographic comparison oracle;
//! it does not authenticate a model, prove independent inference or mint rights.

pub mod io;
pub mod wire;
pub(crate) mod coordinator;

use super::{MAX_COMMITTEE_BYTES, ObservedReview, ObservedSession};
use crate::action::ElapsedTick;
use crate::evidence_view::EvidenceViewManifest;
use crate::reducer::MAX_VOTES;
use crate::round::{Digest, Verdict, commitment};
use crate::Error;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt;
use std::rc::Rc;
use coordinator::{AdvanceError, Coordinator, Session};

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
    coordinator: Coordinator,
}

impl HelperRound {
    pub fn new(session: ObservedSession, limits: HelperLimits) -> Result<(Self, BTreeMap<String, HelperPort>), Error> {
        if limits.members == 0 || limits.input_bytes == 0 || limits.salt_bytes == 0 { return Err(Error::InvalidInput); }
        if limits.members > MAX_VOTES || limits.input_bytes > MAX_COMMITTEE_BYTES
            || limits.salt_bytes > MAX_WORKER_SALT_BYTES { return Err(Error::Limit); }
        let (round, root) = session.worker_identity()?;
        let (coordinator, ports) = Coordinator::new(round, root, session.inputs(), session.window(), session.elapsed(), limits)?;
        Ok((Self { session, coordinator }, ports))
    }

    pub fn elapsed(&self) -> ElapsedTick { self.coordinator.elapsed() }

    /// Supervisor-only health information. It is not a vote or permission.
    pub fn statuses(&self) -> BTreeMap<String, HelperStatus> { self.coordinator.statuses() }

    /// At most one queued reply per frozen member is consumed per invocation.
    /// The original session still checks each commitment, reveal and cutoff.
    pub fn advance(&mut self, now: ElapsedTick) -> Result<(), Error> {
        self.coordinator.advance(&mut NativeSession(&mut self.session), now).map_err(native_error)
    }

    /// Returns the original authority-bound review, with missing workers still
    /// in its denominator. The broker must apply it against fresh current state.
    /// A premature attempt retains the round and its pending obligations.
    pub fn finish(&mut self, now: ElapsedTick) -> Result<ObservedReview, Error> {
        self.advance(now)?;
        match self.session.finish(now) {
            Ok(review) => { self.coordinator.close(); Ok(review) }
            Err(Error::Incomplete) => Err(Error::Incomplete),
            Err(error) => { self.coordinator.close(); Err(error) }
        }
    }
}

struct NativeSession<'a>(&'a mut ObservedSession);
impl Session for NativeSession<'_> {
    type Failure = Infallible;
    // Preserve the existing memory-only adapter: receipt time is observed by
    // its ORIGINAL commit/open/reveal methods, not a new authority operation.
    fn observe(&mut self, _now: ElapsedTick) -> Result<(), Infallible> { Ok(()) }
    fn commit(&mut self, member: &str, digest: Digest, now: ElapsedTick) -> Result<(), AdvanceError<Infallible>> {
        self.0.commit_from_worker(member, digest, now).map_err(AdvanceError::Protocol)
    }
    fn open(&mut self, now: ElapsedTick) -> Result<(), AdvanceError<Infallible>> {
        self.0.open_reveals(now).map_err(AdvanceError::Protocol)
    }
    fn reveal(&mut self, member: &str, verdict: Verdict, salt: &[u8], now: ElapsedTick) -> Result<(), AdvanceError<Infallible>> {
        self.0.reveal(member, verdict, salt, now).map_err(AdvanceError::Protocol)
    }
}
fn native_error(error: AdvanceError<Infallible>) -> Error {
    match error { AdvanceError::Protocol(error) => error, AdvanceError::Backend(impossible) => match impossible {} }
}
