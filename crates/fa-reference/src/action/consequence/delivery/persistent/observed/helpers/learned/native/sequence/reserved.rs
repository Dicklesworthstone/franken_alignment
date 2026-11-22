//! Separately paid explicit successors for a one-round learned stream recipe.
//! Fixed refinement slots retain their original distinct ordering and windows.
use super::*;
use crate::action::consequence::delivery::persistent::requests::actor::FileConveneBasis;
use crate::action::consequence::delivery::stream::ReleaseFrame;
use crate::action::consequence::oversight::learned_host::text::stream::Release;

pub(super) fn supported(host: &FileOversight) -> bool {
    host.machine.native_learned_convening_supported()
        && host.machine.learned_contract().is_some_and(|config| {
            config.requires_sidecar()
                && config.required_policy_source().is_some()
                && config.text_stream_profile().is_some()
                && config.required_pre_output_forecast().is_none()
                && config.required_owned_pre_output_forecast().is_none()
        })
}

/// All remaining owners leave the sequence before any fallible acquisition.
/// Only successful native Begin returns unused owners to that same sequence.
pub(in crate::action::consequence::delivery::persistent::observed) struct ReservedNativeSuccessor {
    sidecar: FileLearnedSidecar,
    members: BTreeMap<String, NativeReviewMember>,
    remaining: VecDeque<BTreeMap<String, NativeReviewMember>>,
}

impl FileNativeSidecarSequence {
    pub(in crate::action::consequence::delivery::persistent::observed) fn reserved_successor_available(
        &self,
    ) -> bool {
        self.status == NativeReviewStatus::Finished
            && self.polls < self.limits.polls
            && self.active.is_some()
            && self.future.is_empty()
            && self.index + 1 == self.rounds.len()
            && !self.convene_rosters.is_empty()
            && matches!(self.history.last(),
                Some(FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. })
                    if receipt.policy.control.decision.consequence == Consequence::HoldEffect)
    }

    pub(in crate::action::consequence::delivery::persistent::observed) fn check_reserved_successor(
        &self,
        host: &FileOversight,
        expected: &FileConveneBasis,
    ) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) || !supported(host) {
            return Err(Error::Binding.into());
        }
        if !self.reserved_successor_available() {
            return Err(Error::WrongState.into());
        }
        let previous = self.active.as_ref().ok_or(Error::Incomplete)?;
        if previous.sidecar.attempt() != expected.attempt
            || previous.sidecar.input_revision() != expected.input_revision
            || self.round().round != expected.held_round
            || host.current_learned_sidecar(&previous.sidecar)? != &self.input
            || self.input.action() != host.request_action(expected.request)?
        {
            return Err(Error::Stale.into());
        }
        if !matches!(self.history.last(),
            Some(FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. })
                if receipt.policy.control.attempt == expected.attempt
                    && receipt.policy.control.sequence == expected.control_sequence
                    && receipt.policy.control.binding.round == expected.held_round)
        {
            return Err(Error::Stale.into());
        }
        self.check_reserved_stream(host)
    }

    fn check_reserved_stream(&self, host: &FileOversight) -> Result<(), JournalError> {
        // The historical request retry shortcut cannot establish today's
        // original generation/audience cut. Derive its exact current action.
        let action = self.input.action();
        let frame = ReleaseFrame::decode(&action.spec().payload)?;
        let current = host.machine.broker.learned_stream_spec(
            if frame.is_finish() {
                Release::Finish
            } else {
                Release::Message
            },
            action.spec().deadline,
        )?;
        if &current != action.spec() {
            return Err(Error::Stale.into());
        }
        Ok(())
    }

    pub(in crate::action::consequence::delivery::persistent::observed) fn take_reserved_successor(
        &mut self,
        host: &FileOversight,
        expected: &FileConveneBasis,
        round: u64,
    ) -> Result<ReservedNativeSuccessor, JournalError> {
        self.check_reserved_successor(host, expected)?;
        if round == 0 || host.convene_round_used(round) || self.records.contains_key(&round) {
            return Err(Error::Duplicate.into());
        }
        self.revision = self.revision.checked_add(1).ok_or(Error::Overflow)?;
        self.status = NativeReviewStatus::Failed;
        self.failure = Some(Error::Incomplete.into());
        let mut remaining = std::mem::take(&mut self.convene_rosters);
        let members = remaining.pop_front().ok_or(Error::Incomplete)?;
        let previous = self.active.take().ok_or(Error::Incomplete)?;
        self.records.insert(
            round,
            NativeSequenceRecord {
                started: false,
                polls: 0,
                interrupted: false,
                members: members.keys().map(|name| (name.clone(), None)).collect(),
                workers: BTreeMap::new(),
                completed: false,
            },
        );
        Ok(ReservedNativeSuccessor {
            sidecar: previous.sidecar,
            members,
            remaining,
        })
    }

    pub(in crate::action::consequence::delivery::persistent::observed) fn begin_reserved_successor(
        &mut self,
        host: &mut FileOversight,
        selected: ReservedNativeSuccessor,
        round: LearnedWorkerRound,
        snapshot: Snapshot,
    ) -> Result<(), JournalError> {
        if !Rc::ptr_eq(&self.issuer, &host.issuer) {
            return Err(Error::Binding.into());
        }
        if self.status != NativeReviewStatus::Failed
            || self.active.is_some()
            || !self.future.is_empty()
            || !self.convene_rosters.is_empty()
            || self.polls >= self.limits.polls
            || self.rounds.len() >= MAX_SIDECAR_REFINEMENT_ROUNDS
        {
            return Err(Error::WrongState.into());
        }
        let record = self.records.get(&round.round).ok_or(Error::Missing)?;
        if record.started
            || record.members.keys().ne(selected.members.keys())
            || host.current_learned_sidecar(&selected.sidecar)? != &self.input
        {
            return Err(Error::Stale.into());
        }
        self.check_reserved_stream(host)?;
        self.rounds.try_reserve(1).map_err(|_| Error::Limit)?;
        let active = match host.begin_native_sidecar_review(
            host.revision(),
            selected.sidecar,
            round,
            selected.members,
            self.limits,
            snapshot,
        ) {
            Ok(active) => active,
            Err(error) => {
                self.failure = Some(error.clone());
                return Err(error);
            }
        };
        self.records
            .get_mut(&round.round)
            .expect("selected original roster")
            .capture(&active);
        self.index = self.rounds.len();
        self.rounds.push(round);
        self.active = Some(active);
        self.convene_rosters = selected.remaining;
        self.status = NativeReviewStatus::Running;
        self.failure = None;
        Ok(())
    }
}
