//! Bounded source-checked sidecar evidence for the ORIGINAL helper congress.
//! Helpers see coarse checked learned state by default. An explicit abstention may
//! purchase one predeclared exact residual for a NEW round under the same action.
//! Missing workers, exhausted budgets and absent residuals never become consent.
use super::{CommitteeContract, CommitteeInput, ObservedReview, action_frame};
use crate::Error;
use crate::action::FrozenAction;
use crate::action::consequence::activation::probe::learned::{CheckedLearnedKv, KvGroup};
use crate::evidence_view::{AuthorizationProjection, EvidencePartView, EvidenceViewManifest,
    OriginalIdentity, RedactionMetadata, WindowMetadata};
use crate::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

const DOMAIN: &[u8; 8] = b"FASIDE\0\x01";
pub const MAX_SIDECAR_REFINEMENT_ROUNDS: usize = 32;
pub const MAX_SIDECAR_PRIORITY_GROUPS: usize = 256;
pub const MAX_SIDECAR_ACCUMULATED_BYTES: usize = 8 * 1_048_576;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SidecarIdentity {
    /// Original evidence object within the action tenant. Caller authentication
    /// of this identity remains outside this reference builder.
    pub object_id: u64,
    pub generation: u64,
    /// Frozen transform identifier for the checked learned representation.
    pub transform_id: u64,
}
impl SidecarIdentity {
    fn check(self) -> Result<(), Error> {
        if self.object_id == 0 || self.transform_id == 0 { return Err(Error::InvalidInput); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SidecarCongressBudget {
    /// Coarse round plus later refined input versions.
    pub rounds: usize,
    /// Purchased residual bytes once, before helper replication.
    pub residual_bytes: usize,
    /// Sum of the existing CommitteeInput logical-byte accounting for every
    /// generated input version, including per-helper replication.
    pub committee_bytes: usize,
}
impl Default for SidecarCongressBudget {
    fn default() -> Self {
        Self { rounds: MAX_SIDECAR_REFINEMENT_ROUNDS, residual_bytes: MAX_SIDECAR_ACCUMULATED_BYTES,
            committee_bytes: MAX_SIDECAR_ACCUMULATED_BYTES }
    }
}
impl SidecarCongressBudget {
    fn check(self) -> Result<(), Error> {
        if self.rounds == 0 || self.residual_bytes == 0 || self.committee_bytes == 0 {
            return Err(Error::InvalidInput);
        }
        if self.rounds > MAX_SIDECAR_REFINEMENT_ROUNDS
            || self.residual_bytes > MAX_SIDECAR_ACCUMULATED_BYTES
            || self.committee_bytes > MAX_SIDECAR_ACCUMULATED_BYTES
        { return Err(Error::Limit); }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SidecarCongressWork {
    pub rounds: usize,
    pub residual_bytes: usize,
    pub committee_bytes: usize,
}

/// The exact helper-visible sidecar bytes plus the validated whole-input object.
/// This is evidence only; it contains no round verdict, permit or dispatch key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SidecarCommitteeRound {
    input: CommitteeInput,
    payload: Rc<[u8]>,
    selected: Vec<KvGroup>,
    work: SidecarCongressWork,
}
impl SidecarCommitteeRound {
    pub fn input(&self) -> &CommitteeInput { &self.input }
    pub fn payload(&self) -> &[u8] { &self.payload }
    pub fn selected_groups(&self) -> &[KvGroup] { &self.selected }
    pub fn work(&self) -> SidecarCongressWork { self.work }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidecarRefinementOutcome {
    /// Every worker supplied a substantive verdict. The caller may apply THIS
    /// review through the original broker; this type does not apply it itself.
    Final,
    /// Missing transport/protocol evidence is not a request for more sidecar data.
    Missing { members: Vec<String> },
    /// A fresh whole-input version was generated with exactly one newly purchased
    /// residual. The caller must record it and start a distinct round.
    Refined { group: KvGroup, round: SidecarCommitteeRound },
    /// A refinement is still requested, but the frozen resource contract cannot
    /// buy another complete input. This is a hold, never a silent skip.
    BudgetExhausted { members: Vec<String> },
    /// Helpers asked for refinement but no predeclared retained residual remains.
    Unresolved { members: Vec<String> },
}

/// Stateful planner for one action's helper-visible sidecar sequence. It never
/// owns an OversightBroker or mutates authority; all round/apply operations remain
/// the existing broker's responsibility. The priority order is frozen up front.
#[derive(Clone, Debug)]
pub struct SidecarCongressPlan {
    source: CheckedLearnedKv,
    identity: SidecarIdentity,
    priority: Vec<KvGroup>,
    selected: BTreeSet<KvGroup>,
    budget: SidecarCongressBudget,
    work: SidecarCongressWork,
    last: Option<CommitteeInput>,
}
impl SidecarCongressPlan {
    pub fn new(source: CheckedLearnedKv, identity: SidecarIdentity, priority: Vec<KvGroup>,
        budget: SidecarCongressBudget) -> Result<Self, Error>
    {
        identity.check()?; budget.check()?;
        if priority.len() > MAX_SIDECAR_PRIORITY_GROUPS { return Err(Error::Limit); }
        let mut unique = BTreeSet::new();
        for group in &priority {
            if !unique.insert(*group) { return Err(Error::Duplicate); }
            // Refinement can only purchase bytes already retained and checked by
            // the source owner. There is no later hidden raw-state fallback.
            source.residual_bytes(*group)?;
        }
        Ok(Self { source, identity, priority, selected: BTreeSet::new(), budget,
            work: SidecarCongressWork::default(), last: None })
    }

    pub fn source(&self) -> &CheckedLearnedKv { &self.source }
    pub fn identity(&self) -> SidecarIdentity { self.identity }
    pub fn selected_groups(&self) -> impl Iterator<Item = KvGroup> + '_ { self.selected.iter().copied() }
    pub fn work(&self) -> SidecarCongressWork { self.work }

    /// Generate exactly one coarse input. Calling it twice cannot manufacture a
    /// fresh review basis or reset refinement accounting.
    pub fn initial(&mut self, action: &FrozenAction, contract: &CommitteeContract)
        -> Result<SidecarCommitteeRound, Error>
    {
        if self.last.is_some() { return Err(Error::WrongState); }
        let round = self.prepare_round(action, contract, &self.selected, 0)?;
        if !fits(round.work, self.budget) { return Err(Error::Limit); }
        self.publish_round(round.clone());
        Ok(round)
    }

    /// Examine a completed but UNAPPLIED original review. Binding to the exact
    /// last input prevents a review over older/coarser bytes from purchasing new
    /// evidence for a different action or revision.
    pub fn refine_after(&mut self, review: &ObservedReview, action: &FrozenAction,
        contract: &CommitteeContract) -> Result<SidecarRefinementOutcome, Error>
    {
        let last = self.last.as_ref().ok_or(Error::WrongState)?;
        if review.inputs() != last || review.inputs().action() != action { return Err(Error::Binding); }
        if !review.missing().is_empty() {
            return Ok(SidecarRefinementOutcome::Missing { members: review.missing().to_vec() });
        }
        if review.abstained().is_empty() { return Ok(SidecarRefinementOutcome::Final); }
        let members = review.abstained().to_vec();
        if self.work.rounds >= self.budget.rounds {
            return Ok(SidecarRefinementOutcome::BudgetExhausted { members });
        }
        let Some(group) = self.priority.iter().copied().find(|group| !self.selected.contains(group)) else {
            return Ok(SidecarRefinementOutcome::Unresolved { members });
        };
        let residual = self.source.residual_bytes(group)?;
        let mut selected = self.selected.clone(); selected.insert(group);
        let round = self.prepare_round(action, contract, &selected, residual.len())?;
        if !fits(round.work, self.budget) {
            return Ok(SidecarRefinementOutcome::BudgetExhausted { members });
        }
        self.selected = selected;
        self.publish_round(round.clone());
        Ok(SidecarRefinementOutcome::Refined { group, round })
    }

    fn publish_round(&mut self, round: SidecarCommitteeRound) {
        self.work = round.work;
        self.last = Some(round.input);
    }

    fn prepare_round(&self, action: &FrozenAction, contract: &CommitteeContract,
        selected: &BTreeSet<KvGroup>, added_residual_bytes: usize) -> Result<SidecarCommitteeRound, Error>
    {
        let payload = encode_payload(&self.source, selected)?;
        let input = committee_input(action, contract, self.identity, &payload)?;
        let work = SidecarCongressWork {
            rounds: self.work.rounds.checked_add(1).ok_or(Error::Overflow)?,
            residual_bytes: self.work.residual_bytes.checked_add(added_residual_bytes).ok_or(Error::Overflow)?,
            committee_bytes: self.work.committee_bytes.checked_add(input.logical_bytes()).ok_or(Error::Overflow)?,
        };
        Ok(SidecarCommitteeRound { input, payload: payload.into(), selected: selected.iter().copied().collect(), work })
    }
}
fn fits(work: SidecarCongressWork, budget: SidecarCongressBudget) -> bool {
    work.rounds <= budget.rounds && work.residual_bytes <= budget.residual_bytes
        && work.committee_bytes <= budget.committee_bytes
}

fn encode_payload(source: &CheckedLearnedKv, selected: &BTreeSet<KvGroup>) -> Result<Vec<u8>, Error> {
    let base = source.encode_base()?;
    let mut length = 24_usize.checked_add(base.len()).ok_or(Error::Overflow)?;
    for group in selected {
        let residual = source.residual_bytes(*group)?;
        length = length.checked_add(8).and_then(|n| n.checked_add(residual.len())).ok_or(Error::Overflow)?;
    }
    if length > crate::full_input::MAX_SUBMITTED_BYTES { return Err(Error::Limit); }
    let mut out = Vec::new(); out.try_reserve_exact(length).map_err(|_| Error::Limit)?;
    out.extend_from_slice(DOMAIN);
    out.extend_from_slice(&(base.len() as u64).to_be_bytes());
    out.extend_from_slice(&(selected.len() as u64).to_be_bytes());
    out.extend_from_slice(&base);
    for group in selected {
        let residual = source.residual_bytes(*group)?;
        out.extend_from_slice(&(residual.len() as u64).to_be_bytes());
        out.extend_from_slice(residual);
    }
    if out.len() != length { return Err(Error::Binding); }
    Ok(out)
}

fn committee_input(action: &FrozenAction, contract: &CommitteeContract, identity: SidecarIdentity,
    payload: &[u8]) -> Result<CommitteeInput, Error>
{
    let original = OriginalIdentity { tenant_id: action.spec().scope.tenant,
        object_id: identity.object_id, generation: identity.generation };
    let frame = action_frame(action);
    let mut views = BTreeMap::new();
    for (member, helper) in contract.members() {
        let total = frame.len().checked_add(payload.len()).and_then(|n| n.checked_add(helper.question().len())).ok_or(Error::Overflow)?;
        let mut bytes = Vec::new(); bytes.try_reserve_exact(total).map_err(|_| Error::Limit)?;
        bytes.extend_from_slice(&frame); let evidence_start = bytes.len();
        bytes.extend_from_slice(payload); let evidence_end = bytes.len();
        bytes.extend_from_slice(helper.question()); let end = bytes.len();
        let input = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: evidence_start } },
            SubmittedPart { kind: PartKind::Evidence { source_id: identity.object_id, transform_id: identity.transform_id },
                span: ByteSpan { start: evidence_start, end: evidence_end } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: evidence_end, end } },
        ], Vec::new())?;
        let manifest = EvidenceViewManifest::new(input, AuthorizationProjection {
            projection_id: helper.projection_id(), policy_epoch: action.spec().policy_epoch,
            projected_originals: vec![original],
        }, vec![EvidencePartView { input_part_index: 1, original, transform_id: identity.transform_id,
            redaction: RedactionMetadata::None, window: WindowMetadata {
                original_byte_len: payload.len() as u64, window_start: 0, window_len: payload.len() as u64,
                truncated: false,
            } }])?;
        if views.insert(member.clone(), manifest).is_some() { return Err(Error::Duplicate); }
    }
    CommitteeInput::capture(action, contract, views)
}
