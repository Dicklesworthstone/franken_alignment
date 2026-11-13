use super::super::{FileActorPort, UnknownReason};
use super::*;
use crate::action::consequence::oversight::MAX_OBSERVED_ROUNDS;

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
        if owner.convening.is_some() {
            return Err(Error::Duplicate.into());
        }
        let grant = Rc::new(Grant {
            queries: Cell::new(queries),
            operations: Cell::new(operations),
            records: RefCell::new(BTreeMap::new()),
            scope: owner.host.actor_explanation_scope(),
            issuer: owner.host.journal_tail_identity(),
        });
        owner.convening = Some(Selection {
            grant: Rc::downgrade(&grant),
            rounds: BTreeSet::new(),
        });
        Ok((
            FileSupervisorConvenePort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
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

    /// Bounded observation-only allocator for EVERY initial or successor helper
    /// review. Opaque external request IDs do not own the native round namespace.
    /// Failed/abandoned admitted IDs remain excluded after grant revocation.
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
        let mut round = preferred;
        for _ in 0..=MAX_OBSERVED_ROUNDS + MAX_CONVENE_OPERATIONS {
            if !owner.host.convene_round_used(round)
                && owner
                    .convening
                    .as_ref()
                    .is_none_or(|selected| !selected.rounds.contains(&round))
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
            completed: false,
        }))
    }
}

impl FileSupervisorConvenePort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner)
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
        let Ok((basis, native_block)) = owner.host.convene_status(request) else {
            return Ok(Knowledge::Unknown {
                reason: UnknownReason::OutcomeUnknown,
            });
        };
        let block_reason = block(&owner.host, &grant, &records, round, native_block);
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
        let (basis, native_block) = owner
            .host
            .convene_status(input.request)
            .map_err(|_| FileConveneError::Unsupported)?;
        if input.expected != basis {
            return Err(FileConveneError::StalePredecessor);
        }
        if let Some(reason) = block(&owner.host, &grant, &records, input.round, native_block) {
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
) -> Option<FileConveneBlockReason> {
    use FileConveneBlockReason::*;
    if records.contains_key(&round) || host.convene_round_used(round) {
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
