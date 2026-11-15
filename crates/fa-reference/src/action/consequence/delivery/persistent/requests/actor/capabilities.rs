//! Read only the surviving original grant's finite counters. These observations
//! describe local custody, not current execution eligibility or a signed profile.
//! No adapter below performs replay, allocates an operation or spends a query.

use super::{Knowledge, OwnerRef, UnknownReason};
use crate::action::Scope;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use std::collections::BTreeMap;
use std::rc::Weak;

/// The original owner observed beside a process-local grant. Journal revision
/// is provenance only: grants and their counters are not durable journal data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileOperatorCapabilityBasis {
    pub scope: Scope,
    pub journal_revision: u64,
}

/// Only explicitly named original grant limits are disclosed. A missing entry
/// is not an unlimited allowance, and a positive counter is not permission to
/// execute. The operation's own source, epoch, predecessor and work checks apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileOperatorCapabilityBudget {
    limits: BTreeMap<&'static str, usize>,
}

impl FileOperatorCapabilityBudget {
    pub fn limits(&self) -> &BTreeMap<&'static str, usize> {
        &self.limits
    }

    pub(super) fn new(limits: &[(&'static str, usize)]) -> Self {
        Self {
            limits: limits.iter().copied().collect(),
        }
    }

    pub(super) fn with(mut self, name: &'static str, value: Option<usize>) -> Self {
        if let Some(value) = value {
            self.limits.insert(name, value);
        }
        self
    }
}

pub type FileOperatorCapabilityObservation =
    Knowledge<FileOperatorCapabilityBudget, FileOperatorCapabilityBasis>;

/// Grant upgrade precedes owner access, matching the native disclosure law.
/// Both handles are dropped before returning, so a snapshot cannot retain the
/// custodian, owner, lock or original incarnation. No mutable borrow is taken.
pub(super) fn observe<G>(
    owner: &Weak<OwnerRef<FileOversight>>,
    grant: &Weak<G>,
    scope: Scope,
    authority_required: &'static str,
    read: impl FnOnce(&G) -> Option<(Weak<()>, FileOperatorCapabilityBudget)>,
) -> FileOperatorCapabilityObservation {
    let Some(grant) = grant.upgrade() else {
        return Knowledge::Withheld { authority_required };
    };
    let unknown = || Knowledge::Unknown {
        reason: UnknownReason::ControllerUnavailable,
    };
    let Some(owner) = owner.upgrade() else {
        return unknown();
    };
    let Ok(owner) = owner.try_borrow() else {
        return unknown();
    };
    if owner.host.storage_failure().is_some() || owner.host.actor_explanation_scope() != scope {
        return unknown();
    }
    let Some((issuer, value)) = read(&grant) else {
        return unknown();
    };
    if !Weak::ptr_eq(&issuer, &owner.host.journal_tail_identity()) {
        return unknown();
    }
    Knowledge::Known {
        value,
        basis: FileOperatorCapabilityBasis {
            scope,
            journal_revision: owner.host.revision(),
        },
    }
}
