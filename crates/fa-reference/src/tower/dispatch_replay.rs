//! Branch-only admission, execution and retry ownership for paired replay.

use super::*;
use crate::Error;
use crate::event_journal::JournalEventKind;

pub const MAX_REPLAY_RECORDS: usize = 32;
pub const MAX_REPLAY_RETAINED_BYTES: usize = 8 * 1_048_576;
pub const MAX_REPLAY_OPERATIONS: usize = 64;

pub(super) struct ReplayRegistration {
    basis: Basis,
    source_address: Address,
    source_basis: Basis,
    record: ObservedReplayRecord,
}

pub(super) struct CachedReplay {
    request: TowerRequest,
    response: TowerResponse,
}

impl TowerBuilder {
    /// Install the actual capability-free context owned by this branch. The
    /// host binds its numeric tenant/run to the Tower's display domain; requests
    /// cannot supply a context or mint one from a purpose tag.
    pub fn enable_replay(&mut self, cx: ExperimentCx) -> Result<(), TowerInstallError> {
        let JournalScope::Branch { branch_id, .. } = self.journal.scope() else {
            return Err(TowerInstallError::ReplayContextRequired);
        };
        if cx.branch_id() != *branch_id
            || cx.tenant_id() == 0
            || cx.run_id() == 0
            || cx.epoch() != self.snapshot_basis.policy_epoch()
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        if self.replay_context.is_some() {
            return Err(TowerInstallError::DuplicateObject);
        }
        self.replay_context = Some(cx);
        Ok(())
    }

    /// Register a branch-owned artifact and its separately retained historical
    /// source. Their cuts are different clock domains: the source is never
    /// relabeled as the branch's cut merely to satisfy object-import checks.
    pub fn insert_replay(
        &mut self,
        address: Address,
        basis: Basis,
        source_address: Address,
        source_basis: Basis,
        record: ObservedReplayRecord,
    ) -> Result<(), TowerInstallError> {
        validate_address(&address)?;
        validate_address(&source_address)?;
        let cx = self
            .replay_context
            .as_ref()
            .ok_or(TowerInstallError::ReplayContextRequired)?;
        let native = record.native_scope();
        if address.kind != Noun::Branch
            || source_address.tenant != self.journal.tenant()
            || !matches!(source_address.kind, Noun::Closure | Noun::Receipt)
            || (source_address.kind == Noun::Receipt && !record.has_receipt())
            || source_address.generation != Some(source_basis.semantic_generation())
            || source_basis.policy_epoch() != record.historical_policy_epoch()
            || cx.tenant_id() != native.tenant
            || cx.run_id() != native.run
            || cx.branch_id() == native.branch
        {
            return Err(TowerInstallError::InvalidReplaySource);
        }
        let key = address.without_generation();
        let Some((generation, object_basis)) = self.objects.get(&key) else {
            return Err(TowerInstallError::MissingSource);
        };
        if basis != *object_basis
            || basis.control_seq() != self.snapshot_basis.control_seq()
            || address.generation != Some(*generation)
        {
            return Err(TowerInstallError::SnapshotMismatch);
        }
        if self.replays.contains_key(&key) {
            return Err(TowerInstallError::DuplicateObject);
        }
        let retained_bytes = self
            .replay_retained_bytes
            .checked_add(record.retained_bytes())
            .ok_or(TowerInstallError::LimitExceeded)?;
        if self.replays.len() >= MAX_REPLAY_RECORDS || retained_bytes > MAX_REPLAY_RETAINED_BYTES {
            return Err(TowerInstallError::LimitExceeded);
        }
        self.replays.insert(
            key,
            ReplayRegistration {
                basis,
                source_address,
                source_basis,
                record,
            },
        );
        self.replay_retained_bytes = retained_bytes;
        Ok(())
    }
}

impl ReferenceTower {
    pub(super) fn dispatch_replay(
        &mut self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> TowerResponse {
        // Admission always precedes record, cache, and original history lookup.
        if !access.operator || !access.full_domain(self.snapshot.journal.tenant()) {
            return self.withheld(&request);
        }
        if !access.allowed_purposes.contains(&PurposeTag::Experiment) {
            return self.refusal(&request, ReasonCode::PurposeMismatch, true);
        }
        let JournalScope::Branch { branch_id, .. } = self.snapshot.journal.scope() else {
            return self.refusal(&request, ReasonCode::PurposeMismatch, true);
        };
        let branch_id = *branch_id;
        let Some(cx) = self.snapshot.replay_context.as_ref() else {
            return self.refusal(&request, ReasonCode::PurposeMismatch, true);
        };
        if cx.branch_id() != branch_id || cx.epoch() != self.snapshot.snapshot_basis.policy_epoch()
        {
            return self.refusal(&request, ReasonCode::PurposeMismatch, true);
        }
        let Some(key) = request.idempotency_key.as_ref() else {
            return self.refusal(&request, ReasonCode::IntegrityFailure, true);
        };
        let edits = match &request.arguments {
            ReadArguments::None => &[][..],
            ReadArguments::Replay { interventions } => interventions.as_slice(),
            _ => return self.refusal(&request, ReasonCode::IntegrityFailure, true),
        };
        if replay::validate_edits(edits).is_err() {
            return self.refusal(&request, ReasonCode::BudgetExhausted, true);
        }
        // Cached outcomes belong to this owned Tower instance and the complete
        // original request. Reauthorization happens above on every retry.
        if let Some(cached) = self.snapshot.replay_cache.get(key) {
            return if cached.request == request {
                cached.response.clone()
            } else {
                self.refusal(&request, ReasonCode::IntegrityFailure, true)
            };
        }
        if self.snapshot.remaining_read_units == 0
            || self.snapshot.replay_cache.len() >= MAX_REPLAY_OPERATIONS
            || self.snapshot.journal.len() >= self.snapshot.limits.max_journal_events
            || self.snapshot.journal.last_control_seq() == u64::MAX
        {
            return self.refusal(&request, ReasonCode::BudgetExhausted, true);
        }
        let Some(registration) = self
            .snapshot
            .replays
            .get(&request.address.without_generation())
        else {
            return self.refusal(&request, ReasonCode::ReplayClosureUnavailable, true);
        };
        if access
            .scope
            .is_authorized_for(&registration.source_address)
            .is_err()
        {
            return self.withheld(&request);
        }
        if let Some(result) = requested_generation(&request.address, registration.basis) {
            return self.response(&request, result, 0, true);
        }

        // A real baseline replay and paired experiment now consumes one finite
        // logical operation unit. Semantic failures and output failures retain
        // that expenditure and the exact retry outcome.
        self.snapshot.remaining_read_units -= 1;
        let execution = registration
            .record
            .run(cx, edits, registration.source_basis);
        let outcome = match execution {
            Ok(source) => TowerRehearsalOutcome {
                rehearsal: true,
                branch_id,
                source_address: registration.source_address.clone(),
                source_basis: registration.source_basis,
                source,
                _seal: RehearsalSeal,
            },
            Err(error) => {
                let reason = match error {
                    Error::Limit | Error::Overflow => ReasonCode::BudgetExhausted,
                    Error::Missing | Error::Incomplete => ReasonCode::ReplayClosureUnavailable,
                    _ => ReasonCode::IntegrityFailure,
                };
                let response = self.replay_failure(&request, reason);
                return self.retain_replay(request, response);
            }
        };
        let digest = match wire::replay_summary_digest(&outcome) {
            Ok(digest) => digest,
            Err(error) => {
                let response = self.replay_failure(&request, encoding_reason(error));
                return self.retain_replay(request, response);
            }
        };

        // The event records the actual paired result, not a fabricated new
        // congress consequence. Only the isolated branch journal can receive it.
        let appended = self.snapshot.journal.emit_transition(
            Vec::new(),
            vec![request.address.clone(), outcome.source_address.clone()],
            JournalEventKind::BranchOutcome {
                branch_id,
                summary_digest: digest,
            },
            digest,
            1,
            0,
        );
        let sequence = match appended {
            Ok(event) => event.control_seq,
            Err(_) => {
                let response = self.replay_failure(&request, ReasonCode::IntegrityFailure);
                return self.retain_replay(request, response);
            }
        };
        let old_basis = self.snapshot.snapshot_basis;
        let mut identity = old_basis.identity();
        identity.control_seq = sequence;
        self.snapshot.snapshot_basis =
            Basis::new(identity, old_basis.claim_class(), old_basis.proofs());
        let result = Knowledge::known(TowerValue::Replay(outcome), self.snapshot.snapshot_basis);
        let mut response = self.response(&request, result, 1, true);
        response.envelope.retry_safe = true;
        response.envelope.source_frontiers = Knowledge::known(
            self.snapshot.frontiers.clone(),
            self.snapshot.snapshot_basis,
        );
        if let Err(error) = response.to_json() {
            // The completed branch outcome remains recorded. An exact retry
            // repeats the refusal without another experiment or another event.
            response = self.replay_failure(&request, encoding_reason(error));
        }
        self.retain_replay(request, response)
    }

    fn replay_failure(&self, request: &TowerRequest, reason: ReasonCode) -> TowerResponse {
        let mut response = self.response(
            request,
            Knowledge::unknown(UnknownReason::Incomplete),
            1,
            true,
        );
        response.envelope.status = ResponseStatus::Error;
        response.envelope.reason_code = Some(reason);
        response.envelope.retry_safe = true;
        response
    }

    fn retain_replay(&mut self, request: TowerRequest, response: TowerResponse) -> TowerResponse {
        let key = request
            .idempotency_key
            .as_ref()
            .expect("admitted replay key")
            .clone();
        self.snapshot.replay_cache.insert(
            key,
            CachedReplay {
                // Retain bounded content, not caller-owned spare capacities.
                request: request.clone(),
                response: response.clone(),
            },
        );
        response
    }
}

fn encoding_reason(error: TowerEncodingError) -> ReasonCode {
    match error {
        TowerEncodingError::OutputLimit => ReasonCode::BudgetExhausted,
        TowerEncodingError::InvalidKnowledge => ReasonCode::IntegrityFailure,
    }
}
