use super::super::{FileActorPort, UnknownReason};
use super::*;
use crate::action::consequence::oversight::MAX_OBSERVED_ROUNDS;
use crate::action::consequence::oversight::learned_host::sidecar::workers::LearnedWorkerRound;

impl FileActorSupervisor<FileOversight> {
    /// Select one original, non-renewable ordinary held-review grant. Provision
    /// before exposing operator ports; this neither observes nor starts a round.
    pub fn convene_port(
        &self,
        queries: usize,
        operations: usize,
    ) -> Result<(FileSupervisorConvenePort, FileSupervisorConveneGrant), JournalError> {
        if queries == 0 || operations == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_CONVENE_QUERIES || operations > MAX_CONVENE_OPERATIONS {
            return Err(Error::Limit.into());
        }
        let mut owner = self
            .owner
            .try_borrow_mut()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        if owner
            .convening
            .as_ref()
            .is_some_and(|selected| selected.issued)
        {
            return Err(Error::Duplicate.into());
        }
        let planned = owner.convening.as_ref().map_or_else(
            || Rc::new(BTreeMap::new()),
            |selected| Rc::clone(&selected.planned),
        );
        let grant = Rc::new(Grant {
            queries: Cell::new(queries),
            operations: Cell::new(operations),
            records: RefCell::new(BTreeMap::new()),
            learned_offer: RefCell::new(None),
            planned: Rc::clone(&planned),
            scope: owner.host.actor_explanation_scope(),
            issuer: owner.host.journal_tail_identity(),
        });
        owner.convening = Some(Selection {
            grant: Rc::downgrade(&grant),
            issued: true,
            planned,
            rounds: BTreeSet::new(),
        });
        Ok((
            FileSupervisorConvenePort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope: grant.scope,
            },
            FileSupervisorConveneGrant { grant },
        ))
    }

    /// Selection remains live with zero unused operations so an already queued
    /// last operation can still be consumed. This never replenishes the grant.
    pub fn has_live_convening(&self) -> Result<bool, JournalError> {
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        let Some(grant) = owner
            .convening
            .as_ref()
            .and_then(|selected| selected.grant.upgrade())
        else {
            return Ok(false);
        };
        if !grant.healthy(&owner.host) {
            return Err(JournalError::Unavailable);
        }
        Ok(true)
    }

    /// Only the retained native wrapper supplies this weak custody marker.
    /// The exact held cut and next fixed round are checked before advertising
    /// an exception to the worker reservation. No work is performed.
    pub(crate) fn offer_native_convene(
        &self,
        request: u64,
        held_round: u64,
        next: LearnedWorkerRound,
        custody: Weak<()>,
    ) -> Result<(), JournalError> {
        self.offer_fixed_learned_convene(request, held_round, next, custody, LearnedOfferMode::NativeFixed)
    }

    /// An original process owner offers only its next fixed executable roster.
    /// Queueing may overlap direct-child cleanup; taking the ticket and every
    /// later Begin still require confirmed reaping by that same owner.
    pub(crate) fn offer_process_convene(
        &self,
        request: u64,
        held_round: u64,
        next: LearnedWorkerRound,
        custody: Weak<()>,
    ) -> Result<(), JournalError> {
        self.offer_fixed_learned_convene(request, held_round, next, custody, LearnedOfferMode::ProcessFixed)
    }

    fn offer_fixed_learned_convene(
        &self,
        request: u64,
        held_round: u64,
        next: LearnedWorkerRound,
        custody: Weak<()>,
        mode: LearnedOfferMode,
    ) -> Result<(), JournalError> {
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        let Some(grant) = owner
            .convening
            .as_ref()
            .and_then(|selected| selected.grant.upgrade())
        else {
            return Ok(());
        };
        if !grant.healthy(&owner.host) || custody.upgrade().is_none() {
            return Err(JournalError::Unavailable);
        }
        let (basis, blocked) = owner.host.learned_convene_status(request)?;
        let available = match mode {
            LearnedOfferMode::NativeFixed => owner.host.native_convene_round_available(next.round),
            LearnedOfferMode::ProcessFixed => !owner.host.convene_round_used(next.round),
            LearnedOfferMode::NativeReserved => false,
        };
        if blocked.is_some()
            || basis.held_round != held_round
            || !available
        {
            return Err(Error::WrongState.into());
        }
        if next.round == 0
            || next.evidence_root == [0; 32]
            || basis.recorded_at.is_none_or(|now| now >= next.window.commit_by)
            || next.window.commit_by >= next.window.reveal_by
            || next.window.reveal_by > basis.deadline
        {
            return Err(Error::InvalidInput.into());
        }
        *grant
            .learned_offer
            .try_borrow_mut()
            .map_err(|_| JournalError::Unavailable)? = Some(LearnedOffer {
            basis,
            round: next.round,
            commit_by: next.window.commit_by,
            custody,
            mode,
        });
        Ok(())
    }

    /// The same original native custodian may instead offer one separately
    /// prepaid unnumbered roster. The chosen ID must be fresh; this mode never
    /// gets the fixed successor's exception for a worker-leased round.
    pub(crate) fn offer_reserved_native_convene(
        &self,
        request: u64,
        held_round: u64,
        custody: Weak<()>,
    ) -> Result<(), JournalError> {
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        let Some(grant) = owner
            .convening
            .as_ref()
            .and_then(|selected| selected.grant.upgrade())
        else {
            return Ok(());
        };
        if !grant.healthy(&owner.host) || custody.upgrade().is_none() {
            return Err(JournalError::Unavailable);
        }
        let (basis, blocked) = owner.host.native_convene_status(request)?;
        if blocked.is_some() || basis.held_round != held_round {
            return Err(Error::WrongState.into());
        }
        *grant
            .learned_offer
            .try_borrow_mut()
            .map_err(|_| JournalError::Unavailable)? = Some(LearnedOffer {
            basis,
            round: 0,
            commit_by: basis.deadline,
            custody,
            mode: LearnedOfferMode::NativeReserved,
        });
        Ok(())
    }

    pub(crate) fn has_reserved_native_convene_offer(
        &self,
        request: u64,
    ) -> Result<bool, JournalError> {
        self.has_learned_convene_offer(request, None, LearnedOfferMode::NativeReserved)
    }

    pub(crate) fn has_fixed_native_convene_offer(
        &self,
        request: u64,
        round: u64,
    ) -> Result<bool, JournalError> {
        self.has_learned_convene_offer(request, Some(round), LearnedOfferMode::NativeFixed)
    }

    pub(crate) fn has_fixed_process_convene_offer(
        &self,
        request: u64,
        round: u64,
    ) -> Result<bool, JournalError> {
        self.has_learned_convene_offer(request, Some(round), LearnedOfferMode::ProcessFixed)
    }

    /// A spent grant remains eligible only for its already admitted operation.
    /// Fixed offers additionally bind that pending receipt to the exact slot.
    fn has_learned_convene_offer(
        &self,
        request: u64,
        fixed_round: Option<u64>,
        mode: LearnedOfferMode,
    ) -> Result<bool, JournalError> {
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        let Some(grant) = owner
            .convening
            .as_ref()
            .and_then(|selected| selected.grant.upgrade())
        else {
            return Ok(false);
        };
        if !grant.healthy(&owner.host) {
            return Err(JournalError::Unavailable);
        }
        let (basis, blocked) = owner.host.learned_convene_status(request)?;
        if blocked.is_some() {
            return Ok(false);
        }
        let offered = grant
            .learned_offer
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        let current = offered.as_ref().is_some_and(|offer| {
            let matches_slot = offer.mode == mode && match fixed_round {
                Some(round) => offer.mode.is_fixed() && offer.round == round,
                None => offer.mode == LearnedOfferMode::NativeReserved,
            };
            matches_slot
                && offer.basis == basis
                && offer.custody.upgrade().is_some()
                && basis.recorded_at.is_some_and(|now| now < offer.commit_by)
        });
        if !current {
            return Ok(false);
        }
        let records = grant
            .records
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        if let Some(round) = fixed_round {
            return Ok(match records.get(&round) {
                Some(record) => {
                    record.receipt.request.request == request
                        && record.receipt.outcome == FileConveneOutcome::Pending
                }
                None => grant.operations.get() > 0,
            });
        }
        Ok(grant.operations.get() > 0
            || records.values().any(|record| {
                record.receipt.request.request == request
                    && record.receipt.outcome == FileConveneOutcome::Pending
            }))
    }

    pub(crate) fn clear_native_convene(&self, request: u64) -> Result<(), JournalError> {
        self.clear_learned_convene(request, true)
    }

    pub(crate) fn clear_process_convene(&self, request: u64) -> Result<(), JournalError> {
        self.clear_learned_convene(request, false)
    }

    fn clear_learned_convene(&self, request: u64, native: bool) -> Result<(), JournalError> {
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        let Some(grant) = owner
            .convening
            .as_ref()
            .and_then(|selected| selected.grant.upgrade())
        else {
            return Ok(());
        };
        let mut offer = grant
            .learned_offer
            .try_borrow_mut()
            .map_err(|_| JournalError::Unavailable)?;
        if offer
            .as_ref()
            .is_some_and(|offered| {
                offered.basis.request == request
                    && (offered.mode != LearnedOfferMode::ProcessFixed) == native
            })
        {
            *offer = None;
        }
        Ok(())
    }

    /// Bounded observation-only allocator for EVERY initial or successor helper
    /// review. Opaque external request IDs do not own the native round namespace.
    /// Fixed schedules and failed/abandoned admitted IDs remain excluded after
    /// grant revocation. Callers with fixed IDs must use those exact IDs instead.
    pub fn next_review_round(&self, preferred: u64) -> Result<u64, JournalError> {
        if preferred == 0 {
            return Err(Error::InvalidInput.into());
        }
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        let planned = owner
            .convening
            .as_ref()
            .map_or(0, |selected| selected.planned.len());
        let mut round = preferred;
        for _ in 0..=MAX_OBSERVED_ROUNDS + MAX_CONVENE_OPERATIONS + planned {
            if !owner.host.convene_round_used(round)
                && owner.convening.as_ref().is_none_or(|selected| {
                    !selected.rounds.contains(&round) && !selected.planned.contains_key(&round)
                })
            {
                return Ok(round);
            }
            round = round.checked_add(1).ok_or(Error::Overflow)?;
        }
        Err(Error::Limit.into())
    }

    pub(crate) fn take_convene(
        &self,
        request: u64,
    ) -> Result<Option<FileConveneTicket>, JournalError> {
        if request == 0 {
            return Err(Error::InvalidInput.into());
        }
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        if owner.host.storage_failure().is_some() {
            return Err(JournalError::Unavailable);
        }
        let Some(grant) = owner
            .convening
            .as_ref()
            .and_then(|selected| selected.grant.upgrade())
        else {
            return Ok(None);
        };
        if !grant.healthy(&owner.host) {
            return Err(JournalError::Unavailable);
        }
        let mut records = grant
            .records
            .try_borrow_mut()
            .map_err(|_| JournalError::Unavailable)?;
        let Some(record) = records.values_mut().find(|record| {
            record.receipt.request.request == request
                && !record.taken
                && record.receipt.outcome == FileConveneOutcome::Pending
        }) else {
            return Ok(None);
        };
        record.taken = true;
        Ok(Some(FileConveneTicket {
            owner: Rc::downgrade(&self.owner),
            grant: Rc::downgrade(&grant),
            input: record.receipt.request,
            learned: record.learned.clone(),
            completed: false,
        }))
    }
}

impl FileSupervisorConvenePort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Spend one query on a bounded read of the exact held predecessor. A Known
    /// actionable plan reserves neither its round ID nor an operation slot.
    pub fn next(&self, request: u64, round: u64) -> FileConvenePlanResult {
        if request == 0 || round == 0 {
            return Err(FileConveneError::MalformedRequest);
        }
        let Some(grant) = self.grant.upgrade() else {
            return Ok(withheld());
        };
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(owner) = owner.try_borrow() else {
            return Ok(unknown());
        };
        if !grant.healthy(&owner.host) {
            return Ok(unknown());
        }
        let remaining = grant.queries.get();
        if remaining == 0 {
            return Err(FileConveneError::Capacity);
        }
        let records = grant
            .records
            .try_borrow()
            .map_err(|_| FileConveneError::Unavailable)?;
        grant.queries.set(remaining - 1);
        let Ok((basis, native_block, learned_offer)) = grant.status(&owner.host, request, round) else {
            return Ok(Knowledge::Unknown {
                reason: UnknownReason::OutcomeUnknown,
            });
        };
        let block_reason = block(
            &owner.host,
            &grant,
            &records,
            round,
            native_block,
            learned_offer.as_ref(),
        );
        Ok(Knowledge::Known {
            value: FileConvenePlan {
                basis,
                round,
                block_reason,
            },
            basis,
        })
    }

    /// Admit one explicit successor into the original finite queue. Nothing in
    /// this method reads a source/clock or starts helpers. Exact retries, also
    /// after progress, observe the same Pending/Opened/Failed/Abandoned record.
    pub fn convene(&self, input: FileConveneRequest) -> FileConveneResult {
        let Some(grant) = self.grant.upgrade() else {
            return Ok(withheld());
        };
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(mut owner) = owner.try_borrow_mut() else {
            return Ok(unknown());
        };
        if !grant.healthy(&owner.host) {
            return Ok(unknown());
        }
        let mut records = grant
            .records
            .try_borrow_mut()
            .map_err(|_| FileConveneError::Unavailable)?;
        if let Some(record) = records.get(&input.round) {
            if record.receipt.request != input {
                return Err(FileConveneError::IdempotencyConflict);
            }
            return Ok(Knowledge::Known {
                value: record.receipt.clone(),
                basis: input.expected,
            });
        }
        let (basis, native_block, learned_offer) = grant
            .status(&owner.host, input.request, input.round)
            .map_err(|_| FileConveneError::Unsupported)?;
        if input.expected != basis {
            return Err(FileConveneError::StalePredecessor);
        }
        if let Some(reason) = block(
            &owner.host,
            &grant,
            &records,
            input.round,
            native_block,
            learned_offer.as_ref(),
        ) {
            return Err(match reason {
                FileConveneBlockReason::RoundAlreadyUsed => FileConveneError::IdempotencyConflict,
                FileConveneBlockReason::RoundLimit
                | FileConveneBlockReason::OperationLimit
                | FileConveneBlockReason::PendingOperation => FileConveneError::Capacity,
                FileConveneBlockReason::StaleReview => FileConveneError::StalePredecessor,
                _ => FileConveneError::Unsupported,
            });
        }
        let selection = owner
            .convening
            .as_mut()
            .ok_or(FileConveneError::Unavailable)?;
        if !Weak::ptr_eq(&selection.grant, &self.grant) || selection.rounds.contains(&input.round) {
            return Err(FileConveneError::IdempotencyConflict);
        }
        if records.len() >= MAX_CONVENE_OPERATIONS
            || selection.rounds.len() >= MAX_CONVENE_OPERATIONS
        {
            return Err(FileConveneError::Capacity);
        }
        grant.operations.set(grant.operations.get() - 1);
        selection.rounds.insert(input.round);
        let receipt = FileConveneReceipt {
            request: input,
            outcome: FileConveneOutcome::Pending,
            opened: None,
        };
        records.insert(
            input.round,
            Record {
                receipt: receipt.clone(),
                taken: false,
                learned: learned_offer,
            },
        );
        Ok(Knowledge::Known {
            value: receipt,
            basis,
        })
    }
}

fn block(
    host: &FileOversight,
    grant: &Grant,
    records: &BTreeMap<u64, Record>,
    round: u64,
    native: Option<FileConveneBlockReason>,
    learned_offer: Option<&LearnedOffer>,
) -> Option<FileConveneBlockReason> {
    use FileConveneBlockReason::*;
    if grant.planned.get(&round).is_some_and(|request| {
        learned_offer.is_none_or(|offer| {
            !offer.mode.is_fixed() || offer.basis.request != *request
        })
    }) {
        return Some(RoundAlreadyUsed);
    }
    if records.contains_key(&round)
        || (learned_offer.is_none_or(|offer| offer.mode != LearnedOfferMode::NativeFixed)
            && host.convene_round_used(round))
    {
        return Some(RoundAlreadyUsed);
    }
    if let Some(reason) = native {
        return Some(reason);
    }
    if grant.operations.get() == 0 {
        return Some(OperationLimit);
    }
    if records
        .values()
        .any(|record| record.receipt.outcome == FileConveneOutcome::Pending)
    {
        return Some(PendingOperation);
    }
    None
}
fn unknown<T>() -> Knowledge<T, FileConveneBasis> {
    Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }
}
fn withheld<T>() -> Knowledge<T, FileConveneBasis> {
    Knowledge::Withheld {
        authority_required: "supervisor_convene",
    }
}
