//! Original acknowledged request/round navigation. This reads retained metadata
//! and original learned completion observations without reconstructing a model,
//! cloning helper inputs or rerunning the authority machine.

use super::{
    DecoderEvent, Event, FileLearnedSidecarFinish, FileOversight, JournalError, LearnedEvent,
    SidecarEvent,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::{
    MAX_JOURNAL_EVENTS, requests::FileRequestDisposition,
};
use crate::action::{ActionState, ElapsedTick, Scope};
use std::collections::{BTreeMap, BTreeSet};

/// Exact original coordinates for one acknowledged request-history observation.
/// request_generation is request-local and may be zero. The journal revision,
/// authority sequence, epoch and elapsed observation are separate coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileRequestHistoryBasis {
    pub scope: Scope,
    pub request: u64,
    pub request_generation: u64,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
    pub recorded_at: Option<ElapsedTick>,
}

/// Completion category only. ApplicationAttempted includes rejected original
/// applications; neither variant asserts a verdict, approval or external effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileRequestReviewCompletion {
    ApplicationAttempted,
    EvidenceRefined,
}

/// A directly observed completed review for the selected original attempt.
/// The independent explanation capability must still validate and disclose it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRequestReviewLink {
    round: u64,
    begin_revision: u64,
    completed_revision: u64,
    completion_kind: FileRequestReviewCompletion,
}

impl FileRequestReviewLink {
    pub fn round(&self) -> u64 {
        self.round
    }
    pub fn begin_revision(&self) -> u64 {
        self.begin_revision
    }
    pub fn completed_revision(&self) -> u64 {
        self.completed_revision
    }
    pub fn completion_kind(&self) -> FileRequestReviewCompletion {
        self.completion_kind
    }
}

/// A complete bounded relationship projection, not a replayable journal or a
/// current eligibility result. Entries follow original completion order.
/// Incomplete reviews are starts without a recorded completion at this cut;
/// they may already be abandoned and do not claim a currently active worker.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{FilePermit,
///     requests::actor::FileRequestHistory};
/// fn no_approval(history: FileRequestHistory) -> FilePermit { history }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRequestHistory {
    basis: FileRequestHistoryBasis,
    attempt: Option<u64>,
    stage: Option<ActionState>,
    incomplete_reviews: usize,
    reviews: Vec<FileRequestReviewLink>,
}

impl FileRequestHistory {
    pub fn basis(&self) -> FileRequestHistoryBasis {
        self.basis
    }
    pub fn admitted(&self) -> bool {
        self.attempt.is_some()
    }
    pub fn attempt(&self) -> Option<u64> {
        self.attempt
    }
    pub fn stage(&self) -> Option<ActionState> {
        self.stage
    }
    pub fn incomplete_reviews(&self) -> usize {
        self.incomplete_reviews
    }
    pub fn reviews(&self) -> &[FileRequestReviewLink] {
        &self.reviews
    }
}

impl FileOversight {
    pub(in crate::action::consequence::delivery::persistent) fn capture_request_history(
        &self,
        request: u64,
        max_history_events: usize,
        max_reviews: usize,
    ) -> Result<Option<FileRequestHistory>, JournalError> {
        if self.fault.is_some() {
            return Err(JournalError::Unavailable);
        }
        if self.events.len() > max_history_events || self.events.len() > MAX_JOURNAL_EVENTS {
            return Err(Error::Limit.into());
        }
        let status = match self.request_status(request) {
            Ok(status) => status,
            Err(JournalError::Contract(Error::Missing)) => return Ok(None),
            Err(error) => return Err(error),
        };
        if status.request != request || request == 0 {
            return Err(Error::Binding.into());
        }
        let control = self.machine.broker.inspect();
        let scope = self.profile.delivery.scope;
        let (attempt, stage) = match status.disposition {
            FileRequestDisposition::NotAdmitted(_) => (None, None),
            FileRequestDisposition::Admitted { attempt, stage } => {
                if attempt == 0
                    || control.ledger.stages.get(&attempt) != Some(&stage)
                    || self.request_action(request)?.spec().scope != scope
                {
                    return Err(Error::Binding.into());
                }
                (Some(attempt), Some(stage))
            }
        };
        let basis = FileRequestHistoryBasis {
            scope,
            request,
            request_generation: status.generation,
            journal_revision: self.revision(),
            control_sequence: control.sequence,
            authority_epoch: control.ledger.epoch,
            recorded_at: control.ledger.elapsed,
        };
        let mut history = FileRequestHistory {
            basis,
            attempt,
            stage,
            incomplete_reviews: 0,
            reviews: Vec::new(),
        };
        let Some(selected) = attempt else {
            return Ok(Some(history));
        };
        let mut begins = BTreeMap::new();
        let mut completed = BTreeSet::new();
        for (index, event) in self.events.iter().enumerate() {
            let revision = u64::try_from(index)
                .map_err(|_| Error::Overflow)?
                .checked_add(1)
                .ok_or(Error::Overflow)?;
            if let Event::Begin(attempt, round, ..) = event {
                if *attempt == 0
                    || *round == 0
                    || begins.insert(*round, (*attempt, revision)).is_some()
                {
                    return Err(Error::Binding.into());
                }
            }
            let Some(round) = super::finish_round(event) else {
                continue;
            };
            let &(reviewed_attempt, begin_revision) = begins.get(&round).ok_or(Error::Binding)?;
            if round == 0 || begin_revision >= revision || !completed.insert(round) {
                return Err(Error::Binding.into());
            }
            // A learned Finish carries the attempt as well as the round. Check
            // that direct link even for an unrelated review, before filtering.
            let completion_kind = match event {
                Event::Finish(..) => FileRequestReviewCompletion::ApplicationAttempted,
                Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(
                    SidecarEvent::Finish {
                        attempt,
                        actor_revision,
                        input_revision,
                        round,
                        ..
                    },
                ))) => {
                    if *attempt != reviewed_attempt {
                        return Err(Error::Binding.into());
                    }
                    // Borrow the acknowledged result. Do not clone its archive,
                    // infer refinement from allow_refinement, or replay numerics.
                    let outcome = self.machine.learned_sidecar_outcome(*round)?;
                    if outcome.attempt != *attempt
                        || outcome.round != *round
                        || outcome.actor_revision != *actor_revision
                        || outcome.input_revision != *input_revision
                    {
                        return Err(Error::Binding.into());
                    }
                    match &outcome.result {
                        FileLearnedSidecarFinish::Applied { .. } => {
                            FileRequestReviewCompletion::ApplicationAttempted
                        }
                        FileLearnedSidecarFinish::Refined { .. } => {
                            FileRequestReviewCompletion::EvidenceRefined
                        }
                    }
                }
                _ => return Err(Error::Binding.into()),
            };
            if reviewed_attempt == selected {
                if history.reviews.len() >= max_reviews {
                    return Err(Error::Limit.into());
                }
                history.reviews.try_reserve(1).map_err(|_| Error::Limit)?;
                history.reviews.push(FileRequestReviewLink {
                    round,
                    begin_revision,
                    completed_revision: revision,
                    completion_kind,
                });
            }
        }
        history.incomplete_reviews = begins
            .iter()
            .filter(|(round, (attempt, _))| *attempt == selected && !completed.contains(*round))
            .count();
        Ok(Some(history))
    }
}
