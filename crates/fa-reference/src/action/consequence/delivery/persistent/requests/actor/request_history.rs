//! Finite, separately granted navigation from an original external request to
//! its completed congresses. Discovery is historical metadata; the independent
//! explanation grant still owns every policy, vote, outcome and replay read.

use super::{
    ActorError, FileActorPort, FileActorSupervisor, JournalError, Knowledge, OwnerRef,
    UnknownReason,
};
use crate::Error;
use crate::action::Scope;
pub use crate::action::consequence::delivery::persistent::observed::replay::request_history::{
    FileRequestHistory, FileRequestHistoryBasis, FileRequestReviewCompletion, FileRequestReviewLink,
};
use crate::action::consequence::delivery::persistent::{
    MAX_JOURNAL_EVENTS, observed::FileOversight,
};
use std::cell::Cell;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_REQUEST_HISTORY_QUERIES: usize = 1024;
pub const MAX_REQUEST_HISTORY_EVENTS: usize = MAX_JOURNAL_EVENTS;
pub const MAX_REQUEST_HISTORY_REVIEWS: usize = MAX_JOURNAL_EVENTS / 2;

/// This capability discloses only original request/attempt/review relationships.
/// It does not imply access to the independent completed-review explanation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileRequestHistoryDisclosure {
    RecordedRequestRelationships,
}

pub type FileRequestHistoryObservation = Knowledge<FileRequestHistory, FileRequestHistoryBasis>;
pub type FileRequestHistoryResult = Result<FileRequestHistoryObservation, ActorError>;

struct Grant {
    remaining: Cell<usize>,
    max_history_events: usize,
    max_reviews: usize,
    disclosure: FileRequestHistoryDisclosure,
}

/// Noncloneable custody of the original finite metadata allowance. Dropping it
/// revokes all weak ports, including clones, without retaining the live owner.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorRequestHistoryGrant;
/// fn no_copy(grant: FileSupervisorRequestHistoryGrant) { grant.clone(); }
/// ```
#[must_use = "retain custody for the explicitly granted request relationships"]
pub struct FileSupervisorRequestHistoryGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorRequestHistoryGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.remaining.get()
    }
    pub fn max_history_events(&self) -> usize {
        self.grant.max_history_events
    }
    pub fn max_reviews(&self) -> usize {
        self.grant.max_reviews
    }
    pub fn disclosure(&self) -> FileRequestHistoryDisclosure {
        self.grant.disclosure
    }
    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorRequestHistoryGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorRequestHistoryGrant")
            .field("remaining_queries", &self.remaining_queries())
            .field("max_history_events", &self.max_history_events())
            .field("max_reviews", &self.max_reviews())
            .field("disclosure", &self.disclosure())
            .finish_non_exhaustive()
    }
}

/// Weak access to exactly one original acknowledged owner and disclosure grant.
/// Equal public scope is insufficient to adopt a replacement in the owner slot.
/// No raw journal, source, mutable host, review key or permit is exported.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorRequestHistoryPort;
/// fn no_authority(port: FileSupervisorRequestHistoryPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorRequestHistoryPort {
    owner: Weak<OwnerRef<FileOversight>>,
    issuer: Weak<()>,
    grant: Weak<Grant>,
    scope: Scope,
}

impl fmt::Debug for FileSupervisorRequestHistoryPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorRequestHistoryPort { .. }")
    }
}

impl FileActorSupervisor<FileOversight> {
    /// Provision metadata work once, without reading history, a source or clock.
    /// Every entered query reserves one complete bounded scan; the result is
    /// either complete or refused. No truncation or continuation hides a round.
    pub fn request_history_port(
        &self,
        queries: usize,
        max_history_events: usize,
        max_reviews: usize,
        disclosure: FileRequestHistoryDisclosure,
    ) -> Result<
        (
            FileSupervisorRequestHistoryPort,
            FileSupervisorRequestHistoryGrant,
        ),
        JournalError,
    > {
        if queries == 0 || max_history_events == 0 || max_reviews == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_REQUEST_HISTORY_QUERIES
            || max_history_events > MAX_REQUEST_HISTORY_EVENTS
            || max_reviews > MAX_REQUEST_HISTORY_REVIEWS
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
            max_history_events,
            max_reviews,
            disclosure,
        });
        Ok((
            FileSupervisorRequestHistoryPort {
                owner: Rc::downgrade(&self.owner),
                issuer: owner.host.journal_tail_identity(),
                grant: Rc::downgrade(&grant),
                scope: owner.host.actor_explanation_scope(),
            },
            FileSupervisorRequestHistoryGrant { grant },
        ))
    }
}

impl FileSupervisorRequestHistoryPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    /// Observe the original request map and completed-round relationships at
    /// one acknowledged cut. A known unadmitted request has no attempt or review;
    /// an unobserved request is Unknown. Unfinished starts are reported only as
    /// a historical count and are never offered as completed explanation links.
    ///
    /// This performs no disk reread or numerical replay. It leaves admission
    /// snapshots, publication keys, Unknown liability and every native budget
    /// untouched apart from this grant's query allowance. Accepted errors and
    /// output failure cannot refund that allowance.
    pub fn history(&self, request: u64) -> FileRequestHistoryResult {
        if request == 0 {
            return Err(ActorError::MalformedProposal);
        }
        let Some(grant) = self.grant.upgrade() else {
            return Ok(Knowledge::Withheld {
                authority_required: "supervisor_request_history",
            });
        };
        let Some(owner) = self.owner.upgrade() else {
            return Ok(unknown());
        };
        let Ok(owner) = owner.try_borrow() else {
            return Ok(unknown());
        };
        if owner.host.storage_failure().is_some()
            || !Weak::ptr_eq(&self.issuer, &owner.host.journal_tail_identity())
            || owner.host.actor_explanation_scope() != self.scope
        {
            return Ok(unknown());
        }
        let remaining = grant.remaining.get();
        if remaining == 0 {
            return Err(ActorError::Capacity);
        }
        grant.remaining.set(remaining - 1);
        match owner.host.capture_request_history(
            request,
            grant.max_history_events,
            grant.max_reviews,
        ) {
            Ok(Some(value)) => {
                let basis = value.basis();
                Ok(Knowledge::Known { value, basis })
            }
            Ok(None) => Ok(unknown()),
            Err(JournalError::Contract(Error::Limit | Error::Overflow)) => {
                Err(ActorError::Capacity)
            }
            Err(_) => Err(ActorError::Unavailable),
        }
    }
}

fn unknown() -> FileRequestHistoryObservation {
    Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    }
}

#[cfg(test)]
mod tests;
