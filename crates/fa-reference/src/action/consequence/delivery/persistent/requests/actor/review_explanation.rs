//! Separately granted historical congress and effect explanation from one
//! ORIGINAL acknowledged owner. Replay consumes the existing durable machine;
//! this adapter never evaluates a replacement policy or manufactures a receipt.

use super::{
    ActorError, FileActorPort, FileActorSupervisor, FileRequestDisposition, JournalError,
    Knowledge, OwnerRef, UnknownReason,
};
use crate::Error;
use crate::action::consequence::congress::CongressPolicy;
use crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::delivery::persistent::observed::replay::{
    FileReviewEffectReplay, ReviewEffectAudit,
};
use crate::action::consequence::gate::containment::session::policy::{Evaluation, Policy};
use crate::action::consequence::gate::containment::session::transcript::CommitRecord;
use crate::action::consequence::oversight::ReviewWindow;
use crate::action::consequence::{Consequence, Decision};
use crate::action::{ActionState, ElapsedTick, ResolvedTarget, Scope};
use crate::reducer::Reduction;
use crate::round::Verdict;
use std::cell::Cell;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_REVIEW_EXPLANATION_QUERIES: usize = 1024;
pub const MAX_REVIEW_EXPLANATION_EVENTS: usize = MAX_JOURNAL_EVENTS;

/// Three cuts of the same acknowledged history. The beginning coordinates are
/// the original pre-vote anchor, the completed coordinates follow the actual
/// application (including refusal), and the ending coordinates include the
/// entire later suffix. Recorded ticks are never fresh clock observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileReviewExplanationBasis {
    pub scope: Scope,
    pub request: u64,
    pub attempt: u64,
    pub round: u64,
    pub begin_revision: u64,
    pub begin_control_sequence: u64,
    pub started_at: ElapsedTick,
    pub completed_revision: u64,
    pub completed_control_sequence: u64,
    pub completed_at: Option<ElapsedTick>,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub recorded_at: Option<ElapsedTick>,
}

/// Exact metadata from the original successful application, not a review key.
/// A replayable Continue can instead retain an application error such as Stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileReviewApplication {
    pub control_sequence: u64,
    pub input_revision: u64,
    pub consequence: Consequence,
}

/// A sealed projection over one immutable native replay. Only selected policy,
/// reducer, timing and effect observations escape; there is no whole archive,
/// action payload, helper input, source snapshot or live owner getter. The
/// selected witnesses in `evaluation()` are part of this explicit native
/// disclosure, as with the admission explanation; wire profiles can withhold
/// their values. Cloning shares the retained replay instead of copying inputs.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{FilePermit,
///     requests::actor::FileReviewExplanation};
/// fn no_approval(value: FileReviewExplanation) -> FilePermit { value }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileReviewExplanation;
/// fn no_helper_inputs(value: FileReviewExplanation) { value.archive(); }
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct FileReviewExplanation {
    basis: FileReviewExplanationBasis,
    replay: Rc<FileReviewEffectReplay>,
    completed_state: ActionState,
    ending_state: ActionState,
}

impl fmt::Debug for FileReviewExplanation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Do not disclose retained helper/payload bytes through derived Debug.
        f.debug_struct("FileReviewExplanation")
            .field("basis", &self.basis)
            .field("completed_state", &self.completed_state)
            .field("ending_state", &self.ending_state)
            .finish_non_exhaustive()
    }
}

impl FileReviewExplanation {
    pub fn basis(&self) -> FileReviewExplanationBasis {
        self.basis
    }
    pub fn policy(&self) -> &Policy {
        &self.replay.review().anchor().policy.policy
    }
    pub fn evaluation(&self) -> &Evaluation {
        &self.replay.review().archive().policy.evaluation
    }
    pub fn congress(&self) -> &CongressPolicy {
        &self.replay.review().anchor().policy.congress
    }
    pub fn decision(&self) -> &Decision {
        &self.replay.review().archive().policy.decision
    }
    pub fn tally(&self) -> &Reduction {
        &self.replay.review().archive().policy.tally
    }
    pub fn missing(&self) -> &[String] {
        &self.replay.review().archive().policy.missing
    }
    pub fn abstained(&self) -> &[String] {
        &self.replay.review().archive().policy.abstained
    }
    pub fn commits(&self) -> &[CommitRecord] {
        &self.replay.review().archive().policy.transcript.commits
    }
    pub fn commit_times(&self) -> &[ElapsedTick] {
        &self.replay.review().archive().commit_times
    }
    /// Original acceptance order, without the retained reveal salts.
    pub fn reveals(&self) -> impl ExactSizeIterator<Item = (&str, Verdict, ElapsedTick)> {
        let archive = self.replay.review().archive();
        archive
            .policy
            .transcript
            .reveals
            .iter()
            .zip(&archive.reveal_times)
            .map(|(record, at)| (record.member.as_str(), record.verdict, *at))
    }
    pub fn window(&self) -> ReviewWindow {
        self.replay.review().anchor().window
    }
    pub fn input_revision(&self) -> u64 {
        self.replay.review().anchor().input_revision
    }
    pub fn evidence_root(&self) -> [u8; 32] {
        self.replay.review().anchor().policy.evidence_root
    }
    pub fn reveals_opened_at(&self) -> ElapsedTick {
        self.replay.review().archive().reveals_opened_at
    }
    pub fn review_completed_at(&self) -> ElapsedTick {
        self.replay.review().archive().completed_at
    }
    pub fn contradiction(&self) -> bool {
        self.replay.review().anchor().policy.contradiction
    }
    pub fn expected_control_sequence(&self) -> u64 {
        self.replay
            .review()
            .anchor()
            .policy
            .expected_control_sequence
    }
    pub fn proposal_policy_epoch(&self) -> u64 {
        self.replay
            .review()
            .anchor()
            .policy
            .action
            .spec()
            .policy_epoch
    }
    pub fn semantic_epoch(&self) -> u64 {
        self.replay.review().anchor().policy.snapshot_semantic_epoch
    }
    pub fn target(&self) -> Option<ResolvedTarget> {
        self.replay.review().anchor().policy.action.spec().target
    }
    pub fn units(&self) -> u64 {
        self.replay.review().anchor().policy.action.spec().units
    }
    pub fn payload_bytes(&self) -> usize {
        self.replay
            .review()
            .anchor()
            .policy
            .action
            .spec()
            .payload
            .len()
    }
    pub fn completed_state(&self) -> ActionState {
        self.completed_state
    }
    pub fn ending_state(&self) -> ActionState {
        self.ending_state
    }
    pub fn application(&self) -> Result<FileReviewApplication, Error> {
        match self.replay.review().application() {
            Ok(receipt) => Ok(FileReviewApplication {
                control_sequence: receipt.policy.control.sequence,
                input_revision: receipt.input_revision,
                consequence: receipt.policy.control.decision.consequence,
            }),
            Err(error) => Err(*error),
        }
    }
    /// Original selected-attempt linkage. Missing observations do not prove absence.
    pub fn effect(&self) -> &ReviewEffectAudit {
        self.replay.effect()
    }
}

pub type FileReviewExplanationObservation =
    Knowledge<FileReviewExplanation, FileReviewExplanationBasis>;
pub type FileReviewExplanationResult = Result<FileReviewExplanationObservation, ActorError>;

struct Grant {
    remaining: Cell<usize>,
    max_replay_events: usize,
}

/// Noncloneable host custody, independent of admission explanation, situation,
/// precheck and actor grants. Drop/revoke withdraws every weak port clone.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorReviewExplanationGrant;
/// fn no_copy(grant: FileSupervisorReviewExplanationGrant) { grant.clone(); }
/// ```
#[must_use = "retain this custodian for explicitly granted completed-review explanation"]
pub struct FileSupervisorReviewExplanationGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorReviewExplanationGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.remaining.get()
    }
    pub fn max_replay_events(&self) -> usize {
        self.grant.max_replay_events
    }
    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorReviewExplanationGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorReviewExplanationGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("max_replay_events", &self.max_replay_events())
            .finish_non_exhaustive()
    }
}

/// Weak access to one original owner and one shared finite disclosure allowance.
/// The port has no input, snapshot, time, mutable-host or authority method.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorReviewExplanationPort;
/// fn no_owner(port: FileSupervisorReviewExplanationPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorReviewExplanationPort {
    owner: Weak<OwnerRef<FileOversight>>,
    grant: Weak<Grant>,
    scope: Scope,
}

impl fmt::Debug for FileSupervisorReviewExplanationPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorReviewExplanationPort { .. }")
    }
}

impl FileActorSupervisor<FileOversight> {
    pub fn review_explanation_port(
        &self,
        queries: usize,
    ) -> Result<
        (
            FileSupervisorReviewExplanationPort,
            FileSupervisorReviewExplanationGrant,
        ),
        JournalError,
    > {
        self.review_explanation_port_with_budget(queries, MAX_REVIEW_EXPLANATION_EVENTS)
    }

    /// Provisioning reads no source, clock or history and writes no event.
    /// Every entered query spends once, including an unknown result after
    /// admission. The complete acknowledged history must fit this event bound.
    pub fn review_explanation_port_with_budget(
        &self,
        queries: usize,
        max_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorReviewExplanationPort,
            FileSupervisorReviewExplanationGrant,
        ),
        JournalError,
    > {
        if queries == 0 || max_replay_events == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_REVIEW_EXPLANATION_QUERIES
            || max_replay_events > MAX_REVIEW_EXPLANATION_EVENTS
        {
            return Err(Error::Limit.into());
        }
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        let grant = Rc::new(Grant {
            remaining: Cell::new(queries),
            max_replay_events,
        });
        Ok((
            FileSupervisorReviewExplanationPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope: owner.host.actor_explanation_scope(),
            },
            FileSupervisorReviewExplanationGrant { grant },
        ))
    }
}

impl FileSupervisorReviewExplanationPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Explain the selected original completed round only if it belongs to this
    /// exact durable external request. A matching numeric attempt or a different
    /// request's valid round cannot substitute. The original cursor reconstructs
    /// congress, actual application and effect linkage in ONE complete pass.
    ///
    /// Unknown covers missing/unadmitted requests, unfinished rounds, unsupported
    /// numerical history, replay bound exhaustion and reconstruction failure.
    /// It is never proved absence, permission, an application retry or a source
    /// refresh. All queries share custody and accounting across port clones;
    /// later encoding/output failure does not refund the entered query.
    pub fn explain_review(&self, request: u64, round: u64) -> FileReviewExplanationResult {
        if request == 0 || round == 0 {
            return Err(ActorError::MalformedProposal);
        }
        let Some(grant) = self.grant.upgrade() else {
            return Ok(Knowledge::Withheld {
                authority_required: "supervisor_review_explanation",
            });
        };
        let unknown = || Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        };
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(owner) = owner.try_borrow() else {
            return Ok(unknown());
        };
        if owner.host.storage_failure().is_some() {
            return Ok(unknown());
        }
        let remaining = grant.remaining.get();
        if remaining == 0 {
            return Err(ActorError::Capacity);
        }
        grant.remaining.set(remaining - 1);
        if owner.host.revision() > grant.max_replay_events as u64
            || owner.host.actor_explanation_scope() != self.scope
            || owner.host.decoder_required()
            || owner.host.learned_action_consistency_required()
        {
            return Ok(unknown());
        }
        let Ok(status) = owner.host.request_status(request) else {
            return Ok(unknown());
        };
        let FileRequestDisposition::Admitted { attempt, stage } = status.disposition else {
            return Ok(unknown());
        };
        let Ok(action) = owner.host.request_action(request) else {
            return Ok(unknown());
        };
        let Ok(mut cursor) = owner.host.begin_review_replay(round) else {
            return Ok(unknown());
        };
        if cursor.advance(0, grant.max_replay_events).is_err() {
            return Ok(unknown());
        }
        let Ok(replay) = cursor.finish_with_effects() else {
            return Ok(unknown());
        };
        let review = replay.review();
        let anchor = review.anchor();
        let completed = review.completed_snapshot();
        let ending = review.journal_snapshot();
        let Some(&completed_state) = completed.control.ledger.stages.get(&attempt) else {
            return Ok(unknown());
        };
        if status.request != request
            || anchor.policy.attempt != attempt
            || anchor.policy.round != round
            || &anchor.policy.action != action
            || action.spec().scope != self.scope
            || review.begin_revision() == 0
            || review.begin_revision() > completed.revision
            || completed.revision > ending.revision
            || ending.revision != owner.host.revision()
            || ending.control.ledger.stages.get(&attempt) != Some(&stage)
            || replay.effect().attempt() != attempt
            || replay.effect().stage_at_review() != completed_state
            || replay.effect().stage_at_end() != stage
        {
            return Ok(unknown());
        }
        let basis = FileReviewExplanationBasis {
            scope: self.scope,
            request,
            attempt,
            round,
            begin_revision: review.begin_revision(),
            begin_control_sequence: anchor.policy.expected_control_sequence,
            started_at: anchor.started_at,
            completed_revision: completed.revision,
            completed_control_sequence: completed.control.sequence,
            completed_at: completed.control.ledger.elapsed,
            journal_revision: ending.revision,
            control_sequence: ending.control.sequence,
            recorded_at: ending.control.ledger.elapsed,
        };
        Ok(Knowledge::Known {
            value: FileReviewExplanation {
                basis,
                replay: Rc::new(replay),
                completed_state,
                ending_state: stage,
            },
            basis,
        })
    }
}

#[cfg(test)]
mod tests;
