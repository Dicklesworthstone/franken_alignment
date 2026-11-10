//! Separately granted, coherent pages of the original live publication journal.

use super::{
    ActorKnowledge, ActorUnknownReason, PublicationResponse, PublicationResult, PublicationTower,
    ReadArguments, ReasonCode, TowerAccess, TowerInstallError, TowerRequest,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileJournalTailResult, FileSupervisorJournalTailPort,
};
use crate::action::consequence::oversight::actor::ActorError;
use crate::address_resolver::Address;

/// A native journal observation bound to the registration and exact query that
/// produced it. Neither a public envelope edit nor an equal numeric owner can
/// turn this historical metadata into another run's history. It retains no
/// supervisor, source snapshot, numerical replay or effect capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedJournalTail {
    native: FileJournalTailResult,
    registration: Address,
    query_address: Address,
    query_operation_id: String,
    query_idempotency_key: Option<String>,
}

impl LinkedJournalTail {
    pub fn native(&self) -> &FileJournalTailResult {
        &self.native
    }

    pub fn registration(&self) -> &Address {
        &self.registration
    }

    pub fn query_address(&self) -> &Address {
        &self.query_address
    }

    pub fn query_operation_id(&self) -> &str {
        &self.query_operation_id
    }

    pub fn query_idempotency_key(&self) -> Option<&str> {
        self.query_idempotency_key.as_deref()
    }
}

impl PublicationTower {
    /// Install an independently retained native journal-disclosure grant.
    /// Actor intake, policy precheck, situation and explanation grants cannot
    /// create it. Replacement withdraws the old port and cursor first, even
    /// when the replacement belongs to a different original owner and refuses.
    pub fn set_journal_tail(
        &mut self,
        port: Option<FileSupervisorJournalTailPort>,
    ) -> Result<(), TowerInstallError> {
        self.journal_tail = None;
        self.journal_tail_cursor = None;
        if let Some(port) = port {
            if !port.belongs_to(self.wire.request_port()) {
                return Err(TowerInstallError::SnapshotMismatch);
            }
            self.journal_tail = Some(port);
        }
        Ok(())
    }

    pub(super) fn dispatch_journal_tail(
        &mut self,
        access: &TowerAccess,
        request: TowerRequest,
    ) -> PublicationResponse {
        // Shared address, tenant and registration checks precede this method.
        // Check the operator role before revealing the installed capability.
        if !access.operator {
            return self.withheld(&request);
        }
        if request.address.without_generation() != self.mount.without_generation() {
            return self.refusal(&request, ReasonCode::MissingRequiredObservation, true);
        }
        let selection = match request.arguments {
            ReadArguments::JournalTail { after_revision } => Ok(after_revision),
            ReadArguments::JournalTailPage { cursor } if cursor != 0 => Err(cursor),
            // Snapshot since_seq and native record revisions are different
            // coordinates. An implicit default must not silently convert them.
            _ => return self.refusal(&request, ReasonCode::IntegrityFailure, true),
        };
        let Some(port) = &self.journal_tail else {
            return self.refusal(&request, ReasonCode::CapabilityUnsupported, true);
        };
        let native = match selection {
            Ok(after_revision) => port.tail(after_revision),
            Err(cursor) => {
                let Some(original) = self
                    .journal_tail_cursor
                    .as_ref()
                    .filter(|original| original.id() == cursor)
                else {
                    // A forged/retired scalar cannot select another capture or
                    // retire the legitimate next page and its remaining quota.
                    return self
                        .journal_tail_response(&request, Err(ActorError::IdempotencyConflict));
                };
                port.continue_tail(original)
            }
        };
        let response = self.journal_tail_response(&request, native);
        // One exact native handle, shared across the original session's
        // reconnects. An output refusal cannot expose a partial Known page or
        // preserve a retry that would bypass the native single-use cursor.
        self.journal_tail_cursor = match &response.result {
            PublicationResult::JournalTail(linked) => match linked.native() {
                Ok(ActorKnowledge::Known { value, .. }) => value.continuation().cloned(),
                _ => None,
            },
            _ => None,
        };
        response
    }

    fn journal_tail_response(
        &self,
        request: &TowerRequest,
        native: FileJournalTailResult,
    ) -> PublicationResponse {
        let reason = match &native {
            Ok(ActorKnowledge::Known { .. }) => None,
            Ok(ActorKnowledge::Unknown {
                reason: ActorUnknownReason::ControllerUnavailable,
            })
            | Err(ActorError::Unavailable) => Some(ReasonCode::LowerLayerUnverified),
            Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_journal_tail",
            }) => Some(ReasonCode::DeniedByPolicy),
            Err(ActorError::Capacity) => Some(ReasonCode::BudgetExhausted),
            Ok(_) | Err(_) => Some(ReasonCode::IntegrityFailure),
        };
        self.bounded_response(self.response(
            request,
            PublicationResult::JournalTail(LinkedJournalTail {
                native,
                registration: self.mount.clone(),
                query_address: request.address.clone(),
                query_operation_id: request.operation_id.clone(),
                query_idempotency_key: request.idempotency_key.clone(),
            }),
            reason,
            true,
        ))
    }
}
