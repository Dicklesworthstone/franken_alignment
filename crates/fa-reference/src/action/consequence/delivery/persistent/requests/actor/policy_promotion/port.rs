use super::super::{FileActorPort, FileActorSupervisor, JournalError, UnknownReason};
use super::*;
use crate::Error;
use crate::action::consequence::delivery::persistent::FileDeliverySnapshot;

impl FileActorSupervisor<FileOversight> {
    /// Delegate one independently approved native campaign. Provisioning checks
    /// the original issuer and complete current corpus without reading source,
    /// clock or journal bytes. It does not create/approve a campaign or consume
    /// an admission snapshot. A stale, revoked or already used key refuses.
    pub fn policy_promotion_port(
        &self,
        permit: FilePolicyPromotionPermit,
        queries: usize,
        max_replay_events: usize,
    ) -> Result<
        (
            FileSupervisorPolicyPromotionPort,
            FileSupervisorPolicyPromotionGrant,
        ),
        JournalError,
    > {
        if queries == 0 || max_replay_events == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_POLICY_PROMOTION_QUERIES
            || max_replay_events > MAX_POLICY_PROMOTION_REPLAY_EVENTS
        {
            return Err(Error::Limit.into());
        }
        let owner = self
            .owner
            .try_borrow()
            .map_err(|_| JournalError::Unavailable)?;
        owner.host.policy_promotion_preflight(&permit)?;
        let review = owner.host.policy_promotion_review(&permit)?;
        let scope = owner.host.actor_explanation_scope();
        let grant = Rc::new(Grant {
            candidate_policy_generation: review.report().candidate_policy().generation(),
            permit,
            queries: Cell::new(queries),
            operations: Cell::new(1),
            max_replay_events,
            record: RefCell::new(None),
        });
        Ok((
            FileSupervisorPolicyPromotionPort {
                owner: Rc::downgrade(&self.owner),
                grant: Rc::downgrade(&grant),
                scope,
                issuer: owner.host.journal_tail_identity(),
            },
            FileSupervisorPolicyPromotionGrant { grant },
        ))
    }
}

impl FileSupervisorPolicyPromotionPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Observe only the selected native campaign. Every entered read spends a
    /// query; the plan reserves neither the original key nor an operation ID.
    /// Selecting another campaign discloses nothing about owner availability.
    pub fn next(&self, campaign: u64, operation: u64) -> FilePolicyPromotionPlanResult {
        if campaign == 0 || operation == 0 {
            return Err(FilePolicyPromotionError::MalformedRequest);
        }
        let Some(grant) = self.grant.upgrade() else {
            return Ok(withheld());
        };
        if campaign != grant.permit.campaign() {
            return Ok(withheld());
        }
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(owner) = owner.try_borrow() else {
            return Ok(unknown());
        };
        if !self.healthy(&owner.host) {
            return Ok(unknown());
        }
        let remaining = grant.queries.get();
        if remaining == 0 {
            return Err(FilePolicyPromotionError::Capacity);
        }
        let record = grant
            .record
            .try_borrow()
            .map_err(|_| FilePolicyPromotionError::Unavailable)?;
        grant.queries.set(remaining - 1);
        let state = owner.host.inspect();
        let basis = basis(&owner.host, self.scope, campaign, &state)?;
        let block_reason = if state.stop.is_some() {
            Some(FilePolicyPromotionBlockReason::Stopped)
        } else if let Some(record) = record.as_ref() {
            Some(if record.request.operation == operation {
                FilePolicyPromotionBlockReason::OperationAlreadyUsed
            } else {
                FilePolicyPromotionBlockReason::OperationLimit
            })
        } else {
            blocked(&owner.host, &grant, &state)?
        };
        Ok(Knowledge::Known {
            value: FilePolicyPromotionPlan {
                basis,
                operation,
                candidate_policy_generation: grant.candidate_policy_generation,
                block_reason,
            },
            basis,
        })
    }

    /// Enter the original durable promotion once under the full expected cut.
    /// The native gate checks the exact approval, complete corpus and candidate
    /// again. Failure or unwind after entry never refunds work or repeats the
    /// transition. A healthy exact retry keeps its original acknowledgment even
    /// after later work; current native dispositions determine driver cleanup.
    pub fn promote(&self, input: FilePolicyPromotionRequest) -> FilePolicyPromotionResult {
        FilePolicyPromotionRequest::new(input.operation, input.expected)?;
        let Some(grant) = self.grant.upgrade() else {
            return Ok(withheld());
        };
        if input.expected.campaign != grant.permit.campaign() {
            return Ok(withheld());
        }
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(mut owner) = owner.try_borrow_mut() else {
            return Ok(unknown());
        };
        if !self.healthy(&owner.host) {
            return Ok(unknown());
        }
        let mut record = grant
            .record
            .try_borrow_mut()
            .map_err(|_| FilePolicyPromotionError::Unavailable)?;
        if let Some(record) = record.as_ref() {
            if record.request.operation != input.operation {
                return Err(FilePolicyPromotionError::Capacity);
            }
            if record.request != input {
                return Err(FilePolicyPromotionError::IdempotencyConflict);
            }
            return record
                .result
                .clone()
                .unwrap_or(Err(FilePolicyPromotionError::Unavailable));
        }
        let before = owner.host.inspect();
        let before_basis = basis(&owner.host, self.scope, grant.permit.campaign(), &before)?;
        if input.expected != before_basis {
            return Err(FilePolicyPromotionError::StalePredecessor);
        }
        if let Some(reason) = blocked(&owner.host, &grant, &before)? {
            return Err(match reason {
                FilePolicyPromotionBlockReason::StaleCampaign => {
                    FilePolicyPromotionError::StalePredecessor
                }
                FilePolicyPromotionBlockReason::ReplayLimit
                | FilePolicyPromotionBlockReason::OperationLimit => {
                    FilePolicyPromotionError::Capacity
                }
                FilePolicyPromotionBlockReason::OperationAlreadyUsed => {
                    FilePolicyPromotionError::IdempotencyConflict
                }
                _ => FilePolicyPromotionError::Unsupported,
            });
        }
        grant.operations.set(0);
        *record = Some(Record {
            request: input,
            result: None,
        });
        owner.snapshot = None;
        let result = owner
            .host
            .promote_policy_campaign(before_basis.journal_revision, &grant.permit)
            .map_err(native_error)
            .and_then(|receipt| {
                let after = owner.host.inspect();
                let after_basis = basis(&owner.host, self.scope, grant.permit.campaign(), &after)?;
                let change = receipt.change();
                if receipt.update().operation() != before_basis.campaign
                    || receipt.update().expected_control_sequence() != before_basis.control_sequence
                    || receipt.update().expected_authority_epoch() != before_basis.authority_epoch
                    || receipt.update().policy().generation() != grant.candidate_policy_generation
                    || change.previous_generation != before_basis.policy_generation
                    || change.generation != after_basis.policy_generation
                    || change.sequence != after_basis.control_sequence
                    || change.revocation_floor != after_basis.authority_epoch
                    || before_basis.journal_revision.checked_add(1)
                        != Some(after_basis.journal_revision)
                    || before_basis.control_sequence.checked_add(1)
                        != Some(after_basis.control_sequence)
                    || before_basis.authority_epoch.checked_add(1)
                        != Some(after_basis.authority_epoch)
                    || after_basis.policy_generation != grant.candidate_policy_generation
                    || after_basis.recorded_at != before_basis.recorded_at
                    || after.control.ledger.charged != before.control.ledger.charged
                    || before
                        .control
                        .ledger
                        .available
                        .checked_add(change.refunded_units)
                        != Some(after.control.ledger.available)
                    || before
                        .control
                        .ledger
                        .reserved
                        .checked_sub(change.refunded_units)
                        != Some(after.control.ledger.reserved)
                    || after.control.ceiling != before.control.ceiling
                    || after.executions != before.executions
                    || after.payload != before.payload
                    || after.target != before.target
                    || after.dispatcher_epoch != before.dispatcher_epoch
                    || after.control.suspended != before.control.suspended
                    || after.stop != before.stop
                {
                    return Err(FilePolicyPromotionError::Unavailable);
                }
                Ok(Knowledge::Known {
                    value: FilePolicyPromotionReceipt {
                        before: before_basis,
                        after: after_basis,
                        operation: input.operation,
                        candidate_policy_generation: grant.candidate_policy_generation,
                        cancelled: change.cancelled.clone(),
                        refunded_units: change.refunded_units,
                    },
                    basis: after_basis,
                })
            });
        record
            .as_mut()
            .expect("entered original policy promotion")
            .result = Some(result.clone());
        result
    }

    fn healthy(&self, host: &FileOversight) -> bool {
        host.storage_failure().is_none()
            && host.actor_explanation_scope() == self.scope
            && Weak::ptr_eq(&self.issuer, &host.journal_tail_identity())
    }
}

fn basis(
    host: &FileOversight,
    scope: Scope,
    campaign: u64,
    state: &FileDeliverySnapshot,
) -> Result<FilePolicyPromotionBasis, FilePolicyPromotionError> {
    Ok(FilePolicyPromotionBasis {
        scope,
        campaign,
        journal_revision: state.revision,
        control_sequence: state.control.sequence,
        authority_epoch: state.control.ledger.epoch,
        policy_generation: host.current_policy().map_err(native_error)?.generation(),
        recorded_at: state.control.ledger.elapsed,
    })
}

fn blocked(
    host: &FileOversight,
    grant: &Grant,
    state: &FileDeliverySnapshot,
) -> Result<Option<FilePolicyPromotionBlockReason>, FilePolicyPromotionError> {
    use FilePolicyPromotionBlockReason::*;
    if state.stop.is_some() {
        return Ok(Some(Stopped));
    }
    if host
        .file_source_status()
        .is_some_and(|source| source.interrupted)
    {
        return Ok(Some(SourceUnavailable));
    }
    if host.decoder_required() || host.action_consistency_required() {
        return Ok(Some(NumericalReplayUnsupported));
    }
    if let Err(error) = host.policy_promotion_preflight(&grant.permit) {
        return match error {
            JournalError::Contract(Error::Stale) => Ok(Some(StaleCampaign)),
            JournalError::Contract(Error::WrongState | Error::Missing | Error::Incomplete) => {
                Ok(Some(CampaignNotApproved))
            }
            other => Err(native_error(other)),
        };
    }
    let events = usize::try_from(state.revision)
        .ok()
        .and_then(|n| n.checked_add(1));
    if events.is_none_or(|events| events > grant.max_replay_events) {
        return Ok(Some(ReplayLimit));
    }
    if grant.operations.get() == 0 {
        return Ok(Some(OperationLimit));
    }
    Ok(None)
}

fn native_error(error: JournalError) -> FilePolicyPromotionError {
    match error {
        JournalError::Contract(Error::Limit | Error::Overflow) => {
            FilePolicyPromotionError::Capacity
        }
        JournalError::Contract(Error::Stale) => FilePolicyPromotionError::StalePredecessor,
        JournalError::Contract(Error::WrongState | Error::Missing | Error::Incomplete) => {
            FilePolicyPromotionError::Unsupported
        }
        _ => FilePolicyPromotionError::Unavailable,
    }
}
fn unknown<T>() -> Knowledge<T, FilePolicyPromotionBasis> {
    Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }
}
fn withheld<T>() -> Knowledge<T, FilePolicyPromotionBasis> {
    Knowledge::Withheld {
        authority_required: "supervisor_policy_promotion",
    }
}
