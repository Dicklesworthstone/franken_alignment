//! Congress-reviewed action lifecycle over the existing in-memory authority.
//!
//! This owner never exposes the underlying authority or its unchecked permit.
//! Reviews bind exact action, policy, snapshot epoch and declared evidence bytes
//! before commitments. No external effect or cryptographic guarantee is added.

use super::{
    ActionState, ElapsedTick, FrozenAction, Inspection, Permit, Purpose, ReferenceAuthority, Scope,
    TrustedOutcome,
};
use crate::round::review::{CongressReview, ExactStatus, ReviewDecision, ReviewPolicy};
use crate::round::{Digest, MAX_FIELD_LEN, Verdict};
use crate::{Error, Judgment, ReadWitness, Snapshot};
use std::collections::BTreeMap;

struct RegisteredReview {
    congress: CongressReview,
    judgment: Judgment,
}

/// One-use token that cannot be constructed from an empirical tally.
///
/// ```compile_fail,E0599
/// use fa_reference::action::congress::ReviewedPermit;
/// fn duplicate(permit: ReviewedPermit) { let _copy = permit.clone(); }
/// ```
///
/// ```compile_fail,E0451
/// use fa_reference::action::{Permit, congress::ReviewedPermit};
/// fn forge(inner: Permit) -> ReviewedPermit { ReviewedPermit { inner } }
/// ```
#[derive(Debug)]
pub struct ReviewedPermit {
    inner: Permit,
}

/// Trusted bootstrap of a bounded reference domain, not a production broker.
/// Clock, snapshot authenticity, actual evidence capture and exact-validator
/// status remain trusted caller facts. There is no mutable inner-authority API.
///
/// ```compile_fail,E0599
/// use fa_reference::action::congress::CongressAuthority;
/// fn branch(authority: CongressAuthority) { let _copy = authority.clone(); }
/// ```
pub struct CongressAuthority {
    authority: ReferenceAuthority,
    policy: ReviewPolicy,
    reviews: BTreeMap<u64, RegisteredReview>,
}

impl CongressAuthority {
    pub fn new(
        scope: Scope,
        total: u64,
        max_attempts: usize,
        policy: ReviewPolicy,
    ) -> Result<Self, Error> {
        Ok(Self {
            authority: ReferenceAuthority::new(scope, total, max_attempts)?,
            policy,
            reviews: BTreeMap::new(),
        })
    }

    pub fn observe_time(&mut self, elapsed: ElapsedTick) -> Result<(), Error> {
        self.authority.observe_time(elapsed)
    }

    /// Freeze all review inputs before accepting a commitment. The complete
    /// length-framed challenge must fit the round's 4096-byte reference profile.
    /// Every fallible preflight precedes insertion into the authority.
    pub fn propose(
        &mut self,
        id: u64,
        action: FrozenAction,
        snapshot: &Snapshot,
        evidence: &[u8],
    ) -> Result<(), Error> {
        self.authority.validate_current(&action)?;
        let challenge = binding(id, &action, &self.policy, snapshot.semantic_epoch, evidence)?;
        let judgment = Judgment::capture(snapshot, action.spec().required_witnesses.clone())?;
        let congress = CongressReview::new(self.policy.clone(), id, &challenge)?;
        self.authority.propose(id, action)?;
        self.reviews.insert(id, RegisteredReview { congress, judgment });
        Ok(())
    }

    pub fn prepare(&mut self, id: u64) -> Result<(), Error> {
        self.authority.prepare(id)
    }

    pub fn begin_review(&mut self, id: u64) -> Result<(), Error> {
        self.authority.begin_review(id)
    }

    /// Inspection does not allow changing the policy or replacing the transcript.
    pub fn review(&self, id: u64) -> Result<&CongressReview, Error> {
        Ok(&self.reviews.get(&id).ok_or(Error::Missing)?.congress)
    }

    pub fn commit(&mut self, id: u64, member: &str, digest: Digest) -> Result<(), Error> {
        self.authority.attempt_at(id, ActionState::Reviewing)?;
        self.reviews
            .get_mut(&id)
            .ok_or(Error::Missing)?
            .congress
            .commit(member, digest)
    }

    pub fn open_reveals(&mut self, id: u64) -> Result<(), Error> {
        self.authority.attempt_at(id, ActionState::Reviewing)?;
        self.reviews
            .get_mut(&id)
            .ok_or(Error::Missing)?
            .congress
            .open_reveals()
    }

    pub fn reveal(&mut self, id: u64, member: &str, verdict: Verdict, salt: &[u8]) -> Result<(), Error> {
        self.authority.attempt_at(id, ActionState::Reviewing)?;
        self.reviews
            .get_mut(&id)
            .ok_or(Error::Missing)?
            .congress
            .reveal(member, verdict, salt)
    }

    /// A hold returns Incomplete without reserving rights; exact disqualification
    /// returns Binding. Call `review().evaluate()` for the diagnostic predicate.
    pub fn authorize(
        &mut self,
        id: u64,
        snapshot: &Snapshot,
        evidence: &[u8],
        exact: ExactStatus,
    ) -> Result<ReviewedPermit, Error> {
        let attempt = self.authority.attempt_at(id, ActionState::Reviewing)?;
        self.check_review(id, &attempt.action, snapshot, evidence, exact)?;
        let judgment = &self.reviews.get(&id).ok_or(Error::Missing)?.judgment;
        let inner = self.authority.authorize(id, judgment, snapshot)?;
        Ok(ReviewedPermit { inner })
    }

    /// Re-evaluate against current inputs before the one-use dispatch transition.
    /// A failed check leaves the reservation held, never refunded as nonexecution.
    pub fn dispatch(
        &mut self,
        permit: &ReviewedPermit,
        final_action: &FrozenAction,
        snapshot: &Snapshot,
        evidence: &[u8],
        exact: ExactStatus,
    ) -> Result<(), Error> {
        self.check_review(permit.inner.attempt, final_action, snapshot, evidence, exact)?;
        self.authority.dispatch(&permit.inner, final_action, snapshot)
    }

    pub fn cancel(&mut self, id: u64) -> Result<(), Error> {
        self.authority.cancel(id)
    }

    pub fn deny(&mut self, id: u64) -> Result<(), Error> {
        self.authority.deny(id)
    }

    pub fn mark_unknown(&mut self, id: u64) -> Result<(), Error> {
        self.authority.mark_unknown(id)
    }

    pub fn record_trusted_outcome(&mut self, id: u64, outcome: TrustedOutcome) -> Result<(), Error> {
        self.authority.record_trusted_outcome(id, outcome)
    }

    pub fn mark_irrecoverable(&mut self, id: u64) -> Result<(), Error> {
        self.authority.mark_irrecoverable(id)
    }

    pub fn revoke_epoch(&mut self) -> Result<(), Error> {
        self.authority.revoke_epoch()
    }

    pub fn inspect(&self) -> Inspection {
        self.authority.inspect()
    }

    fn check_review(
        &self,
        id: u64,
        action: &FrozenAction,
        snapshot: &Snapshot,
        evidence: &[u8],
        exact: ExactStatus,
    ) -> Result<(), Error> {
        let registered = self.reviews.get(&id).ok_or(Error::Missing)?;
        let current = binding(id, action, &self.policy, snapshot.semantic_epoch, evidence)?;
        if current.as_slice() != registered.congress.transcript().evidence_root() {
            return Err(Error::Binding);
        }
        if !registered.judgment.valid_at(snapshot)? {
            return Err(Error::Binding);
        }
        match registered.congress.evaluate(exact)? {
            ReviewDecision::Ready(_) => Ok(()),
            ReviewDecision::Hold(_) => Err(Error::Incomplete),
            ReviewDecision::Disqualified => Err(Error::Binding),
        }
    }
}

/// Exact reference preimage, not a canonical wire format or digest. No field is
/// summarized with the transcript's non-cryptographic hash. Large challenges
/// refuse rather than silently narrowing the helper's declared dependencies.
fn binding(
    id: u64,
    action: &FrozenAction,
    policy: &ReviewPolicy,
    semantic_epoch: u64,
    evidence: &[u8],
) -> Result<Vec<u8>, Error> {
    let mut out = Binding(Vec::new());
    out.bytes(b"fa-reference/action/congress/v1")?;
    out.word(id)?;
    let spec = action.spec();
    out.word(u64::from(spec.version))?;
    for value in [
        spec.scope.tenant,
        spec.scope.principal,
        spec.scope.run,
        spec.scope.branch,
        spec.scope.authority,
    ] {
        out.word(value)?;
    }
    out.word(match spec.scope.purpose {
        Purpose::Effect => 1,
        Purpose::Experiment => 2,
    })?;
    let target = spec.target.ok_or(Error::Incomplete)?;
    for value in [
        target.adapter,
        target.object,
        target.contract_version,
        target.expected_version,
        target.generation,
        spec.policy_epoch,
        spec.deadline.0,
        spec.units,
        semantic_epoch,
    ] {
        out.word(value)?;
    }
    out.bytes(&spec.payload)?;
    out.word(spec.required_witnesses.len() as u64)?;
    for witness in &spec.required_witnesses {
        match witness {
            ReadWitness::Exact { key, value } => {
                out.word(1)?;
                out.word(*key)?;
                out.word(u64::from(value.is_some()))?;
                if let Some(value) = value {
                    out.bytes(value)?;
                }
            }
            ReadWitness::EmptyRange { start, end } => {
                out.word(2)?;
                out.word(*start)?;
                out.word(*end)?;
            }
        }
    }
    let requirements = policy.requirements();
    for value in [
        requirements.caps.per_member,
        requirements.caps.per_cohort,
        requirements.min_permit_weight,
        requirements.max_hold_weight,
        requirements.min_permit_cohorts as u64,
    ] {
        out.word(value)?;
    }
    out.word(policy.members().len() as u64)?;
    for member in policy.members() {
        out.bytes(member.id.as_bytes())?;
        out.bytes(member.cohort.as_bytes())?;
        out.word(member.weight)?;
    }
    out.bytes(evidence)?;
    Ok(out.0)
}

struct Binding(Vec<u8>);

impl Binding {
    fn word(&mut self, value: u64) -> Result<(), Error> {
        self.bytes(&value.to_be_bytes())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), Error> {
        let end = self
            .0
            .len()
            .checked_add(8)
            .and_then(|len| len.checked_add(value.len()))
            .ok_or(Error::Limit)?;
        if end > MAX_FIELD_LEN {
            return Err(Error::Limit);
        }
        self.0.extend_from_slice(&(value.len() as u64).to_be_bytes());
        self.0.extend_from_slice(value);
        Ok(())
    }
}
