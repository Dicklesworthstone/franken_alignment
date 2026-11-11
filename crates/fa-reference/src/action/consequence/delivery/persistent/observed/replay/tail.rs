//! Redacted journal metadata from the ORIGINAL acknowledged live owner.
//! No event payload is cloned, encoded, replayed or exposed by this projection.

use super::super::{
    BaseEvent, Event, FileOversight, JournalError,
    decoder::{
        DecoderEvent,
        learned::{LearnedEvent, sidecar::SidecarEvent},
    },
    source::SourceEvent,
};
use super::situation::{FileSituationBasis, FileSituationOrigin};
use crate::Error;
use std::rc::{Rc, Weak};

/// Closed metadata labels, not claims about a transition's result. In particular,
/// Publish/Reconcile describe recorded operations; they are not execution or
/// nonexecution receipts. The original outcome and explanation APIs own those
/// observations. Nested policy/model data and helper decisions remain withheld.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileJournalEventKind {
    Time,
    Propose,
    SubmitRequest,
    Inputs,
    InputsUnavailable,
    BeginReview,
    Commit,
    OpenReveals,
    Reveal,
    FinishReview,
    Authorize,
    RequestHuman,
    HumanDecision,
    RevokeHumans,
    Dispatch,
    Publish,
    Reconcile,
    Seal,
    AbandonUnknown,
    Cancel,
    Fence,
    Sweep,
    Stop,
    StopProgress,
    ReplacePolicy,
    NarrowAuthority,
    ReserveRecovery,
    ActorState,
    ActorCheckpoint,
    ActorReset,
    PublicationGuard,
    PublishChecked,
    SourceEnabled,
    SourceObserved,
    SourceWithdrawn,
    SourceReplaced,
    CredentialGuard,
    PublishCredentialed,
    CredentialRotate,
    CredentialRevoke,
    PolicyCampaign,
    StreamBootstrap,
    GeneratedStreamBootstrap,
    Identity,
    DecoderEnabled,
    Tokenizer,
    TextIntent,
    DecoderStep,
    DecoderResume,
    DecoderCheckpoint,
    DecoderStopPolicy,
    Generation,
    GenerationBegin,
    GenerationAdvance,
    GenerationCancel,
    LearnedEnabled,
    LearnedStepBegin,
    LearnedStep,
    LearnedResume,
    LearnedMessageContinuation,
    LearnedSidecarPrepare,
    LearnedSidecarBegin,
    LearnedSidecarFinish,
    LearnedCheckpoint,
    Credibility,
    Consistency,
    Mediation,
    PublicationWitness,
    TextMessage,
}

impl FileJournalEventKind {
    /// Validate a closed wire label without accepting debug names or caller text.
    pub fn from_label(label: &str) -> Option<Self> {
        Some(match label {
            "time" => Self::Time,
            "propose" => Self::Propose,
            "submit_request" => Self::SubmitRequest,
            "inputs" => Self::Inputs,
            "inputs_unavailable" => Self::InputsUnavailable,
            "begin_review" => Self::BeginReview,
            "commit" => Self::Commit,
            "open_reveals" => Self::OpenReveals,
            "reveal" => Self::Reveal,
            "finish_review" => Self::FinishReview,
            "authorize" => Self::Authorize,
            "request_human" => Self::RequestHuman,
            "human_decision" => Self::HumanDecision,
            "revoke_humans" => Self::RevokeHumans,
            "dispatch" => Self::Dispatch,
            "publish" => Self::Publish,
            "reconcile" => Self::Reconcile,
            "seal" => Self::Seal,
            "abandon_unknown" => Self::AbandonUnknown,
            "cancel" => Self::Cancel,
            "fence" => Self::Fence,
            "sweep" => Self::Sweep,
            "stop" => Self::Stop,
            "stop_progress" => Self::StopProgress,
            "replace_policy" => Self::ReplacePolicy,
            "narrow_authority" => Self::NarrowAuthority,
            "reserve_recovery" => Self::ReserveRecovery,
            "actor_state" => Self::ActorState,
            "actor_checkpoint" => Self::ActorCheckpoint,
            "actor_reset" => Self::ActorReset,
            "publication_guard" => Self::PublicationGuard,
            "publish_checked" => Self::PublishChecked,
            "source_enabled" => Self::SourceEnabled,
            "source_observed" => Self::SourceObserved,
            "source_withdrawn" => Self::SourceWithdrawn,
            "source_replaced" => Self::SourceReplaced,
            "credential_guard" => Self::CredentialGuard,
            "publish_credentialed" => Self::PublishCredentialed,
            "credential_rotate" => Self::CredentialRotate,
            "credential_revoke" => Self::CredentialRevoke,
            "policy_campaign" => Self::PolicyCampaign,
            "stream_bootstrap" => Self::StreamBootstrap,
            "generated_stream_bootstrap" => Self::GeneratedStreamBootstrap,
            "identity" => Self::Identity,
            "decoder_enabled" => Self::DecoderEnabled,
            "tokenizer" => Self::Tokenizer,
            "text_intent" => Self::TextIntent,
            "decoder_step" => Self::DecoderStep,
            "decoder_resume" => Self::DecoderResume,
            "decoder_checkpoint" => Self::DecoderCheckpoint,
            "decoder_stop_policy" => Self::DecoderStopPolicy,
            "generation" => Self::Generation,
            "generation_begin" => Self::GenerationBegin,
            "generation_advance" => Self::GenerationAdvance,
            "generation_cancel" => Self::GenerationCancel,
            "learned_enabled" => Self::LearnedEnabled,
            "learned_step_begin" => Self::LearnedStepBegin,
            "learned_step" => Self::LearnedStep,
            "learned_resume" => Self::LearnedResume,
            "learned_message_continuation" => Self::LearnedMessageContinuation,
            "learned_sidecar_prepare" => Self::LearnedSidecarPrepare,
            "learned_sidecar_begin" => Self::LearnedSidecarBegin,
            "learned_sidecar_finish" => Self::LearnedSidecarFinish,
            "learned_checkpoint" => Self::LearnedCheckpoint,
            "credibility" => Self::Credibility,
            "consistency" => Self::Consistency,
            "mediation" => Self::Mediation,
            "publication_witness" => Self::PublicationWitness,
            "text_message" => Self::TextMessage,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Time => "time",
            Self::Propose => "propose",
            Self::SubmitRequest => "submit_request",
            Self::Inputs => "inputs",
            Self::InputsUnavailable => "inputs_unavailable",
            Self::BeginReview => "begin_review",
            Self::Commit => "commit",
            Self::OpenReveals => "open_reveals",
            Self::Reveal => "reveal",
            Self::FinishReview => "finish_review",
            Self::Authorize => "authorize",
            Self::RequestHuman => "request_human",
            Self::HumanDecision => "human_decision",
            Self::RevokeHumans => "revoke_humans",
            Self::Dispatch => "dispatch",
            Self::Publish => "publish",
            Self::Reconcile => "reconcile",
            Self::Seal => "seal",
            Self::AbandonUnknown => "abandon_unknown",
            Self::Cancel => "cancel",
            Self::Fence => "fence",
            Self::Sweep => "sweep",
            Self::Stop => "stop",
            Self::StopProgress => "stop_progress",
            Self::ReplacePolicy => "replace_policy",
            Self::NarrowAuthority => "narrow_authority",
            Self::ReserveRecovery => "reserve_recovery",
            Self::ActorState => "actor_state",
            Self::ActorCheckpoint => "actor_checkpoint",
            Self::ActorReset => "actor_reset",
            Self::PublicationGuard => "publication_guard",
            Self::PublishChecked => "publish_checked",
            Self::SourceEnabled => "source_enabled",
            Self::SourceObserved => "source_observed",
            Self::SourceWithdrawn => "source_withdrawn",
            Self::SourceReplaced => "source_replaced",
            Self::CredentialGuard => "credential_guard",
            Self::PublishCredentialed => "publish_credentialed",
            Self::CredentialRotate => "credential_rotate",
            Self::CredentialRevoke => "credential_revoke",
            Self::PolicyCampaign => "policy_campaign",
            Self::StreamBootstrap => "stream_bootstrap",
            Self::GeneratedStreamBootstrap => "generated_stream_bootstrap",
            Self::Identity => "identity",
            Self::DecoderEnabled => "decoder_enabled",
            Self::Tokenizer => "tokenizer",
            Self::TextIntent => "text_intent",
            Self::DecoderStep => "decoder_step",
            Self::DecoderResume => "decoder_resume",
            Self::DecoderCheckpoint => "decoder_checkpoint",
            Self::DecoderStopPolicy => "decoder_stop_policy",
            Self::Generation => "generation",
            Self::GenerationBegin => "generation_begin",
            Self::GenerationAdvance => "generation_advance",
            Self::GenerationCancel => "generation_cancel",
            Self::LearnedEnabled => "learned_enabled",
            Self::LearnedStepBegin => "learned_step_begin",
            Self::LearnedStep => "learned_step",
            Self::LearnedResume => "learned_resume",
            Self::LearnedMessageContinuation => "learned_message_continuation",
            Self::LearnedSidecarPrepare => "learned_sidecar_prepare",
            Self::LearnedSidecarBegin => "learned_sidecar_begin",
            Self::LearnedSidecarFinish => "learned_sidecar_finish",
            Self::LearnedCheckpoint => "learned_checkpoint",
            Self::Credibility => "credibility",
            Self::Consistency => "consistency",
            Self::Mediation => "mediation",
            Self::PublicationWitness => "publication_witness",
            Self::TextMessage => "text_message",
        }
    }
}

/// One acknowledged event position and directly recorded, nonsecret links.
/// A journal revision is an ordinal, never a control sequence or timestamp.
/// `request` always means an external actor request; human approval IDs and
/// operation IDs are deliberately not relabeled as actor requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileJournalTailEvent {
    revision: u64,
    kind: FileJournalEventKind,
    attempt: Option<u64>,
    request: Option<u64>,
    round: Option<u64>,
}

impl FileJournalTailEvent {
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn kind(&self) -> FileJournalEventKind {
        self.kind
    }
    pub fn attempt(&self) -> Option<u64> {
        self.attempt
    }
    pub fn request(&self) -> Option<u64> {
        self.request
    }
    pub fn round(&self) -> Option<u64> {
        self.round
    }
}

/// Internal immutable metadata only; no raw event or authoritative machine is
/// retained. Native grants alone issue pages from this capture.
pub(in crate::action::consequence::delivery::persistent) struct FileJournalTailCapture {
    pub basis: FileSituationBasis,
    pub after_revision: u64,
    pub events: Vec<FileJournalTailEvent>,
}

impl FileOversight {
    /// Bind the actual native incarnation, including replacement through a
    /// trusted gateway's mutable host slot. An equal scope cannot adopt a grant.
    pub(in crate::action::consequence::delivery::persistent) fn journal_tail_identity(
        &self,
    ) -> Weak<()> {
        Rc::downgrade(&self.issuer)
    }

    pub(in crate::action::consequence::delivery::persistent) fn capture_journal_tail(
        &self,
        after_revision: u64,
    ) -> Result<Option<FileJournalTailCapture>, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        if self.events.len() > super::super::super::MAX_JOURNAL_EVENTS {
            return Err(Error::Limit.into());
        }
        let revision = self.revision();
        // Nothing beyond this acknowledged cut has been observed. A future
        // request cannot turn into an empty, supposedly complete journal page.
        if after_revision > revision {
            return Ok(None);
        }
        let start = usize::try_from(after_revision).map_err(|_| Error::Limit)?;
        let count = self.events.len().checked_sub(start).ok_or(Error::Binding)?;
        let mut events = Vec::new();
        events.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        for (index, event) in self.events[start..].iter().enumerate() {
            let event_revision = after_revision
                .checked_add(index as u64)
                .and_then(|before| before.checked_add(1))
                .ok_or(Error::Overflow)?;
            events.push(project(event_revision, event)?);
        }
        // Inspect the small control projection, not snapshot()'s copied payload.
        // Every event and this basis are read under the same owner borrow.
        let control = self.machine.broker.inspect();
        Ok(Some(FileJournalTailCapture {
            basis: FileSituationBasis {
                scope: self.profile.delivery.scope,
                journal_revision: revision,
                control_sequence: control.sequence,
                recorded_at: control.ledger.elapsed,
                origin: FileSituationOrigin::AcknowledgedOwner,
                anchored_through: None,
            },
            after_revision,
            events,
        }))
    }
}

fn project(revision: u64, event: &Event) -> Result<FileJournalTailEvent, Error> {
    use FileJournalEventKind as Kind;
    let mut row = FileJournalTailEvent {
        revision,
        kind: Kind::Time,
        attempt: None,
        request: None,
        round: None,
    };
    row.kind = match event {
        Event::Core(core) => match core {
            BaseEvent::Time(_) => Kind::Time,
            BaseEvent::Propose(attempt, ..) => {
                row.attempt = Some(*attempt);
                Kind::Propose
            }
            BaseEvent::SubmitRequest(request, ..) => {
                row.request = Some(*request);
                Kind::SubmitRequest
            }
            BaseEvent::Publish(attempt) => {
                row.attempt = Some(*attempt);
                Kind::Publish
            }
            BaseEvent::Reconcile(attempt) => {
                row.attempt = Some(*attempt);
                Kind::Reconcile
            }
            BaseEvent::Seal(attempt) => {
                row.attempt = Some(*attempt);
                Kind::Seal
            }
            BaseEvent::Cancel(attempt) => {
                row.attempt = Some(*attempt);
                Kind::Cancel
            }
            BaseEvent::AbandonUnknown(request) => {
                row.attempt = Some(request.attempt);
                Kind::AbandonUnknown
            }
            BaseEvent::Fence => Kind::Fence,
            BaseEvent::Sweep => Kind::Sweep,
            BaseEvent::Stop(_) => Kind::Stop,
            BaseEvent::StopProgress(_) => Kind::StopProgress,
            BaseEvent::ReplacePolicy(_) => Kind::ReplacePolicy,
            BaseEvent::ReserveRecovery(_) => Kind::ReserveRecovery,
            // These base shapes are forbidden by this original journal profile.
            BaseEvent::Review(_)
            | BaseEvent::Authorize(..)
            | BaseEvent::Dispatch(..)
            | BaseEvent::ActivateCredibility(_)
            | BaseEvent::WithdrawCredibility(_) => return Err(Error::Binding),
        },
        Event::Inputs(attempt, ..) => {
            row.attempt = Some(*attempt);
            Kind::Inputs
        }
        Event::InputsUnavailable(attempt, ..) => {
            row.attempt = Some(*attempt);
            Kind::InputsUnavailable
        }
        Event::Begin(attempt, round, ..) => {
            row.attempt = Some(*attempt);
            row.round = Some(*round);
            Kind::BeginReview
        }
        Event::Commit(round, ..) => {
            row.round = Some(*round);
            Kind::Commit
        }
        Event::OpenReveals(round) => {
            row.round = Some(*round);
            Kind::OpenReveals
        }
        Event::Reveal(round, ..) => {
            row.round = Some(*round);
            Kind::Reveal
        }
        Event::Finish(round, ..) => {
            row.round = Some(*round);
            Kind::FinishReview
        }
        Event::Authorize(attempt, ..) => {
            row.attempt = Some(*attempt);
            Kind::Authorize
        }
        Event::RequestHuman(_, attempt, ..) => {
            row.attempt = Some(*attempt);
            Kind::RequestHuman
        }
        Event::Human(..) => Kind::HumanDecision,
        Event::RevokeHumans => Kind::RevokeHumans,
        Event::Dispatch(attempt, ..) => {
            row.attempt = Some(*attempt);
            Kind::Dispatch
        }
        Event::ActorState(_) => Kind::ActorState,
        Event::NarrowAuthority(_) => Kind::NarrowAuthority,
        Event::ActorCheckpoint(..) => Kind::ActorCheckpoint,
        Event::ActorReset(..) => Kind::ActorReset,
        Event::PublicationGuard => Kind::PublicationGuard,
        Event::PublishChecked(attempt, ..) => {
            row.attempt = Some(*attempt);
            Kind::PublishChecked
        }
        Event::Source(source) => match source {
            SourceEvent::Enable(_) => Kind::SourceEnabled,
            SourceEvent::Observe(..) => Kind::SourceObserved,
            SourceEvent::Withdraw => Kind::SourceWithdrawn,
            SourceEvent::Replace(_) => Kind::SourceReplaced,
        },
        Event::CredentialGuard(_) => Kind::CredentialGuard,
        Event::PublishCredentialed(attempt, ..) => {
            row.attempt = Some(*attempt);
            Kind::PublishCredentialed
        }
        Event::CredentialRotate(_) => Kind::CredentialRotate,
        Event::CredentialRevoke(_) => Kind::CredentialRevoke,
        Event::Campaign(_) => Kind::PolicyCampaign,
        Event::StreamBootstrap(_) => Kind::StreamBootstrap,
        Event::GeneratedStreamBootstrap(_) => Kind::GeneratedStreamBootstrap,
        Event::Identity(_) => Kind::Identity,
        Event::Decoder(decoder) => match decoder {
            DecoderEvent::Learned(learned) => match learned {
                LearnedEvent::Enable(_) => Kind::LearnedEnabled,
                LearnedEvent::Begin(_) => Kind::LearnedStepBegin,
                LearnedEvent::Step { .. } => Kind::LearnedStep,
                LearnedEvent::Resume { .. } => Kind::LearnedResume,
                LearnedEvent::ContinueMessage(_) => Kind::LearnedMessageContinuation,
                LearnedEvent::Checkpoint(_) => Kind::LearnedCheckpoint,
                LearnedEvent::Sidecar(sidecar) => match sidecar {
                    SidecarEvent::Prepare { attempt, .. } => {
                        row.attempt = Some(*attempt);
                        Kind::LearnedSidecarPrepare
                    }
                    SidecarEvent::Begin { attempt, .. } => {
                        row.attempt = Some(*attempt);
                        Kind::LearnedSidecarBegin
                    }
                    SidecarEvent::Finish { attempt, round, .. } => {
                        row.attempt = Some(*attempt);
                        row.round = Some(*round);
                        Kind::LearnedSidecarFinish
                    }
                },
            },
            DecoderEvent::Enable(_) => Kind::DecoderEnabled,
            DecoderEvent::Tokenizer(_) => Kind::Tokenizer,
            DecoderEvent::TextIntent(_) => Kind::TextIntent,
            DecoderEvent::Step(..) => Kind::DecoderStep,
            DecoderEvent::Resume { .. } => Kind::DecoderResume,
            DecoderEvent::Checkpoint(..) => Kind::DecoderCheckpoint,
            DecoderEvent::StopPolicy(_) => Kind::DecoderStopPolicy,
            DecoderEvent::Generate(..) => Kind::Generation,
            DecoderEvent::BeginGeneration(_) => Kind::GenerationBegin,
            DecoderEvent::AdvanceGeneration { .. } => Kind::GenerationAdvance,
            DecoderEvent::CancelGeneration { .. } => Kind::GenerationCancel,
        },
        Event::Credibility(_) => Kind::Credibility,
        Event::Consistency(_) => Kind::Consistency,
        Event::Mediation(_) => Kind::Mediation,
        Event::PublicationWitness(_) => Kind::PublicationWitness,
        Event::TextMessage(request, _) => {
            row.request = Some(request.request);
            Kind::TextMessage
        }
    };
    Ok(row)
}
