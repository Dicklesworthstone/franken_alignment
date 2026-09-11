//! Mandatory model-liveness lane over the existing authority and effect ledger.
//! The separate observer is a trusted measurement ingress, not authenticated
//! hardware or a remote identity service. No signature or native inference is
//! supplied. Mismatch latches before an observer can discard its returned report.

use super::OversightBroker;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::{FrameIdentity, SourceFrame};
use crate::action::consequence::activation::identity::{AnchorObservation, ModelManifest, ModelPassport};
use crate::Error;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

pub const MAX_IDENTITY_CHECKS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityPolicy {
    pub observer_id: u64,
    pub timeout_ticks: u64,
    pub validity_ticks: u64,
    pub max_checks: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityMismatch {
    Manifest,
    CaptureContract { anchor: u64, frame: FrameIdentity, dimensions: usize },
    Anchor { anchor: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityOutcome {
    Collecting,
    Matched,
    Mismatch(IdentityMismatch),
    Unavailable,
    Expired,
}

#[derive(Debug)]
struct Binding {
    issuer: Rc<()>,
    id: u64,
    basis: u64,
    sequence: u64,
    epoch: u64,
    actor_revision: u64,
    started: ElapsedTick,
    deadline: ElapsedTick,
    valid_until: ElapsedTick,
    passport: Rc<ModelPassport>,
}

/// Immutable registered stimuli and context; cloning does not copy authority.
#[derive(Clone, Debug)]
pub struct IdentityChallenge { binding: Rc<Binding> }

impl IdentityChallenge {
    pub fn id(&self) -> u64 { self.binding.id }
    pub fn basis(&self) -> u64 { self.binding.basis }
    pub fn control_sequence(&self) -> u64 { self.binding.sequence }
    pub fn revocation_epoch(&self) -> u64 { self.binding.epoch }
    pub fn actor_revision(&self) -> u64 { self.binding.actor_revision }
    pub fn passport(&self) -> &ModelPassport { &self.binding.passport }
    pub fn started_at(&self) -> ElapsedTick { self.binding.started }
    pub fn deadline(&self) -> ElapsedTick { self.binding.deadline }
    pub fn valid_until(&self) -> ElapsedTick { self.binding.valid_until }
}

/// Historical observation, not a currentness certificate or a live capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityReport {
    pub check: u64,
    pub basis: u64,
    pub observer: u64,
    pub passport: u64,
    pub passport_generation: u64,
    pub control_sequence: u64,
    pub revocation_epoch: u64,
    pub actor_revision: u64,
    pub started_at: ElapsedTick,
    pub deadline: ElapsedTick,
    pub valid_until: ElapsedTick,
    pub completed_at: Option<ElapsedTick>,
    pub manifest: Option<ModelManifest>,
    pub observations: BTreeMap<u64, AnchorObservation>,
    pub outcome: IdentityOutcome,
}

/// The numerical result and the actual control transition, never a permit.
///
/// ```compile_fail,E0308
/// use fa_reference::action::Permit;
/// use fa_reference::action::consequence::oversight::identity::IdentityInstallation;
/// fn elevate(result: IdentityInstallation) -> Permit { result }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentityInstallation {
    pub report: IdentityReport,
    pub sequence: u64,
    pub revocation_floor: u64,
    pub cancelled: Vec<u64>,
    pub refunded_units: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdentityStatus {
    Unconfigured,
    Missing,
    Pending { check: u64 },
    Expired { check: u64 },
    Stale { check: u64 },
    Matching { check: u64, valid_until: ElapsedTick },
    Mismatch { check: u64 },
}

#[derive(Debug)]
struct Entry {
    binding: Rc<Binding>,
    report: IdentityReport,
    installation: Option<IdentityInstallation>,
}

#[derive(Debug)]
struct State {
    issuer: Rc<()>,
    passport: Rc<ModelPassport>,
    policy: IdentityPolicy,
    elapsed: Option<ElapsedTick>,
    basis: u64,
    active: Option<u64>,
    live: Option<u64>,
    mismatch: Option<u64>,
    sequences: BTreeMap<u64, u64>,
    entries: BTreeMap<u64, Entry>,
}

impl State {
    fn matches(&self, challenge: &IdentityChallenge) -> Result<(), Error> {
        if !Rc::ptr_eq(&self.issuer, &challenge.binding.issuer) { return Err(Error::Binding); }
        let entry = self.entries.get(&challenge.id()).ok_or(Error::Missing)?;
        if !Rc::ptr_eq(&entry.binding, &challenge.binding) { return Err(Error::Binding); }
        Ok(())
    }
    fn observe_time(&mut self, now: ElapsedTick) -> Result<(), Error> {
        if self.elapsed.is_some_and(|previous| now < previous) { return Err(Error::Stale); }
        self.elapsed = Some(now);
        Ok(())
    }
    fn observing(&mut self, challenge: &IdentityChallenge, now: ElapsedTick) -> Result<(), Error> {
        self.matches(challenge)?;
        if self.active != Some(challenge.id())
            || self.entries[&challenge.id()].report.outcome != IdentityOutcome::Collecting
        { return Err(Error::WrongState); }
        self.observe_time(now)?;
        if now >= challenge.deadline() {
            let entry = self.entries.get_mut(&challenge.id()).expect("validated check");
            entry.report.outcome = IdentityOutcome::Expired;
            entry.report.completed_at = Some(now);
            self.active = None;
            return Err(Error::Stale);
        }
        Ok(())
    }
    fn fail(&mut self, id: u64, mismatch: IdentityMismatch, now: ElapsedTick) {
        let entry = self.entries.get_mut(&id).expect("validated check");
        entry.report.outcome = IdentityOutcome::Mismatch(mismatch);
        entry.report.completed_at = Some(now);
        self.mismatch = Some(id);
        self.active = None;
        self.live = None;
    }
}

#[derive(Debug)]
pub(super) struct IdentityGate { state: Rc<RefCell<State>> }

/// Separately provisioned measurement ingress, deliberately not Clone. It can
/// supply observations, not a claimed Passed result, effect permit or refund.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::oversight::identity::IdentityObserver;
/// fn duplicate(observer: IdentityObserver) { let _copy = observer.clone(); }
/// ```
#[derive(Debug)]
pub struct IdentityObserver { state: Rc<RefCell<State>> }

impl IdentityObserver {
    pub fn observe_manifest(
        &self, challenge: &IdentityChallenge, manifest: ModelManifest, now: ElapsedTick,
    ) -> Result<IdentityReport, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.observing(challenge, now)?;
        if state.entries[&challenge.id()].report.manifest.is_some() { return Err(Error::Duplicate); }
        let changed = &manifest != state.passport.manifest();
        state.entries.get_mut(&challenge.id()).expect("validated check").report.manifest = Some(manifest);
        if changed { state.fail(challenge.id(), IdentityMismatch::Manifest, now); }
        Ok(state.entries[&challenge.id()].report.clone())
    }

    pub fn observe_anchor(
        &self, challenge: &IdentityChallenge, anchor: u64, source: &SourceFrame, now: ElapsedTick,
    ) -> Result<IdentityReport, Error> {
        let mut state = self.state.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.observing(challenge, now)?;
        let entry = &state.entries[&challenge.id()];
        if entry.report.manifest.is_none() { return Err(Error::Incomplete); }
        if entry.report.observations.contains_key(&anchor) { return Err(Error::Duplicate); }
        let registered = state.passport.anchors().get(&anchor).ok_or(Error::Missing)?;
        if source.identity().sequence <= state.sequences.get(&anchor).copied().unwrap_or(0) {
            return Err(Error::Stale);
        }
        let compared = registered.compare(source);
        match compared {
            Err(Error::Binding) => state.fail(challenge.id(), IdentityMismatch::CaptureContract {
                anchor, frame: source.identity(), dimensions: source.dimensions(),
            }, now),
            Err(error) => return Err(error),
            Ok(observation) => {
                let outside = observation.outside() != 0;
                state.sequences.insert(anchor, source.identity().sequence);
                state.entries.get_mut(&challenge.id()).expect("validated check")
                    .report.observations.insert(anchor, observation);
                if outside {
                    state.fail(challenge.id(), IdentityMismatch::Anchor { anchor }, now);
                } else if state.entries[&challenge.id()].report.observations.len() == state.passport.anchors().len() {
                    let entry = state.entries.get_mut(&challenge.id()).expect("validated check");
                    entry.report.outcome = IdentityOutcome::Matched;
                    entry.report.completed_at = Some(now);
                    state.active = None;
                }
            }
        }
        Ok(state.entries[&challenge.id()].report.clone())
    }
}

impl OversightBroker {
    pub fn enable_identity_checks(
        &mut self, passport: ModelPassport, policy: IdentityPolicy,
    ) -> Result<IdentityObserver, Error> {
        if self.identity.is_some() { return Err(Error::Duplicate); }
        if !self.inputs.is_empty() || !self.started_rounds.is_empty() || self.inspect().sequence != 0 {
            return Err(Error::WrongState);
        }
        if policy.observer_id == 0 || policy.timeout_ticks == 0 || policy.max_checks == 0
            || policy.timeout_ticks > policy.validity_ticks
        { return Err(Error::InvalidInput); }
        if policy.max_checks > MAX_IDENTITY_CHECKS { return Err(Error::Limit); }
        let actor = self.delivery.controller().actor().profile();
        let manifest = passport.manifest();
        if manifest.tenant != self.scope.tenant || manifest.model_generation != actor.model_generation
            || manifest.host_generation != actor.host_generation
            || manifest.tokenizer_generation != actor.tokenizer_generation
        { return Err(Error::Binding); }
        let state = Rc::new(RefCell::new(State {
            issuer: Rc::clone(&self.issuer), passport: Rc::new(passport), policy, elapsed: None,
            basis: 0, active: None, live: None, mismatch: None, sequences: BTreeMap::new(),
            entries: BTreeMap::new(),
        }));
        self.identity = Some(IdentityGate { state: Rc::clone(&state) });
        Ok(IdentityObserver { state })
    }

    pub fn identity_basis(&self) -> Result<u64, Error> {
        let state = self.identity.as_ref().ok_or(Error::Incomplete)?.state.try_borrow()
            .map_err(|_| Error::WrongState)?;
        Ok(state.basis)
    }

    /// Beginning any admitted check removes the prior live basis. Capacity
    /// failure after valid context also leaves it removed, never quietly reused.
    /// Pending reviews and human keys are invalidated; reservations are not.
    pub fn begin_identity_check(
        &mut self, id: u64, expected_sequence: u64, expected_actor_revision: u64,
    ) -> Result<IdentityChallenge, Error> {
        let shared = Rc::clone(&self.identity.as_ref().ok_or(Error::Incomplete)?.state);
        let mut state = shared.try_borrow_mut().map_err(|_| Error::WrongState)?;
        let inspection = self.inspect();
        if inspection.sequence != expected_sequence || self.actor_revision() != expected_actor_revision {
            return Err(Error::Stale);
        }
        if inspection.suspended || state.mismatch.is_some() || state.active.is_some() {
            return Err(Error::WrongState);
        }
        if id == 0 { return Err(Error::InvalidInput); }
        if state.entries.contains_key(&id) { return Err(Error::Duplicate); }
        let now = inspection.ledger.elapsed.ok_or(Error::Incomplete)?;
        let deadline = ElapsedTick(now.0.checked_add(state.policy.timeout_ticks).ok_or(Error::Overflow)?);
        let valid_until = ElapsedTick(now.0.checked_add(state.policy.validity_ticks).ok_or(Error::Overflow)?);
        let basis = state.basis.checked_add(1).ok_or(Error::Overflow)?;
        state.observe_time(now)?;
        self.invalidate_identity_inputs()?;
        state.basis = basis;
        state.live = None;
        if state.entries.len() >= state.policy.max_checks { return Err(Error::Limit); }
        let binding = Rc::new(Binding {
            issuer: Rc::clone(&self.issuer), id, basis, sequence: inspection.sequence,
            epoch: inspection.ledger.epoch, actor_revision: expected_actor_revision,
            started: now, deadline, valid_until, passport: Rc::clone(&state.passport),
        });
        let report = IdentityReport {
            check: id, basis, observer: state.policy.observer_id, passport: state.passport.id(),
            passport_generation: state.passport.generation(), control_sequence: inspection.sequence,
            revocation_epoch: inspection.ledger.epoch, actor_revision: expected_actor_revision,
            started_at: now, deadline, valid_until, completed_at: None, manifest: None,
            observations: BTreeMap::new(), outcome: IdentityOutcome::Collecting,
        };
        state.entries.insert(id, Entry { binding: Rc::clone(&binding), report, installation: None });
        state.active = Some(id);
        Ok(IdentityChallenge { binding })
    }

    /// Explicit capture loss closes an incomplete challenge but never clears a
    /// mismatch latch. Fresh evidence plus a fresh congress is needed afterward.
    pub fn identity_unavailable(&mut self, expected_basis: u64) -> Result<u64, Error> {
        let shared = Rc::clone(&self.identity.as_ref().ok_or(Error::Incomplete)?.state);
        let mut state = shared.try_borrow_mut().map_err(|_| Error::WrongState)?;
        if state.basis != expected_basis { return Err(Error::Stale); }
        let basis = state.basis.checked_add(1).ok_or(Error::Overflow)?;
        let now = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        state.observe_time(now)?;
        self.invalidate_identity_inputs()?;
        if let Some(id) = state.active.take() {
            let entry = state.entries.get_mut(&id).expect("active check");
            entry.report.outcome = IdentityOutcome::Unavailable;
            entry.report.completed_at = Some(now);
        }
        state.basis = basis;
        state.live = None;
        Ok(basis)
    }

    pub fn expire_identity_check(&mut self, challenge: &IdentityChallenge) -> Result<(), Error> {
        let shared = Rc::clone(&self.identity.as_ref().ok_or(Error::Incomplete)?.state);
        let mut state = shared.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.matches(challenge)?;
        let now = self.inspect().ledger.elapsed.ok_or(Error::Incomplete)?;
        if now < challenge.deadline() { return Err(Error::WrongState); }
        match state.observing(challenge, now) {
            Err(Error::Stale) if state.entries[&challenge.id()].report.outcome == IdentityOutcome::Expired => Ok(()),
            result => result,
        }
    }

    /// A mismatch is already a shared fail-closed condition before this call.
    /// Here it becomes the ORIGINAL authority's suspension/revocation transaction,
    /// not an endpoint nonexecution claim or a new rights ledger.
    pub fn apply_identity_check(
        &mut self, challenge: &IdentityChallenge, expected_sequence: u64, expected_epoch: u64,
    ) -> Result<IdentityInstallation, Error> {
        let shared = Rc::clone(&self.identity.as_ref().ok_or(Error::Incomplete)?.state);
        let mut state = shared.try_borrow_mut().map_err(|_| Error::WrongState)?;
        state.matches(challenge)?;
        let entry = &state.entries[&challenge.id()];
        if entry.installation.is_some() { return Err(Error::Duplicate); }
        let report = entry.report.clone();
        let inspection = self.inspect();
        if inspection.sequence != expected_sequence || inspection.ledger.epoch != expected_epoch {
            return Err(Error::Stale);
        }
        let now = inspection.ledger.elapsed.ok_or(Error::Incomplete)?;
        if now < report.completed_at.ok_or(Error::Incomplete)? { return Err(Error::Stale); }
        state.observe_time(now)?;
        let (sequence, revocation_floor, cancelled, refunded_units) = match report.outcome {
            IdentityOutcome::Mismatch(_) if state.mismatch == Some(challenge.id()) => {
                let result = self.delivery.fence_identity(expected_sequence, expected_epoch)?;
                for slot in self.inputs.values_mut() { slot.approved = None; }
                result
            }
            IdentityOutcome::Matched => {
                if state.mismatch.is_some() || state.active.is_some() { return Err(Error::WrongState); }
                if report.basis != state.basis || report.control_sequence != expected_sequence
                    || report.revocation_epoch != expected_epoch || report.actor_revision != self.actor_revision()
                    || now >= report.valid_until || inspection.suspended
                { return Err(Error::Stale); }
                state.live = Some(challenge.id());
                (expected_sequence, expected_epoch, Vec::new(), 0)
            }
            _ => return Err(Error::Incomplete),
        };
        let installation = IdentityInstallation { report, sequence, revocation_floor, cancelled, refunded_units };
        state.entries.get_mut(&challenge.id()).expect("validated check").installation = Some(installation.clone());
        Ok(installation)
    }

    pub fn identity_report(&self, id: u64) -> Result<IdentityReport, Error> {
        let state = self.identity.as_ref().ok_or(Error::Incomplete)?.state.try_borrow()
            .map_err(|_| Error::WrongState)?;
        Ok(state.entries.get(&id).ok_or(Error::Missing)?.report.clone())
    }

    /// Recover a retained installation receipt, not a live approval or a refund.
    pub fn identity_installation(&self, id: u64) -> Result<Option<IdentityInstallation>, Error> {
        let state = self.identity.as_ref().ok_or(Error::Incomplete)?.state.try_borrow()
            .map_err(|_| Error::WrongState)?;
        Ok(state.entries.get(&id).ok_or(Error::Missing)?.installation.clone())
    }

    pub fn identity_status(&self) -> Result<IdentityStatus, Error> {
        let Some(gate) = &self.identity else { return Ok(IdentityStatus::Unconfigured); };
        let state = gate.state.try_borrow().map_err(|_| Error::WrongState)?;
        if let Some(check) = state.mismatch { return Ok(IdentityStatus::Mismatch { check }); }
        let inspection = self.inspect();
        let now = inspection.ledger.elapsed.ok_or(Error::Incomplete)?;
        if let Some(check) = state.active {
            return Ok(if now >= state.entries[&check].binding.deadline {
                IdentityStatus::Expired { check }
            } else { IdentityStatus::Pending { check } });
        }
        let Some(check) = state.live else { return Ok(IdentityStatus::Missing); };
        let binding = &state.entries[&check].binding;
        if now >= binding.valid_until { return Ok(IdentityStatus::Expired { check }); }
        if state.elapsed.is_some_and(|observed| now < observed)
            || binding.basis != state.basis || binding.epoch != inspection.ledger.epoch
            || inspection.suspended
        { return Ok(IdentityStatus::Stale { check }); }
        Ok(IdentityStatus::Matching { check, valid_until: binding.valid_until })
    }

    pub(super) fn check_identity(&self) -> Result<(), Error> {
        match self.identity_status()? {
            IdentityStatus::Unconfigured | IdentityStatus::Matching { .. } => Ok(()),
            IdentityStatus::Mismatch { .. } => Err(Error::Binding),
            IdentityStatus::Expired { .. } | IdentityStatus::Stale { .. } => Err(Error::Stale),
            _ => Err(Error::Incomplete),
        }
    }

    fn invalidate_identity_inputs(&mut self) -> Result<(), Error> {
        if self.inputs.values().any(|slot| slot.revision == u64::MAX) { return Err(Error::Overflow); }
        for slot in self.inputs.values_mut() {
            slot.revision += 1;
            slot.approved = None;
        }
        Ok(())
    }
}
