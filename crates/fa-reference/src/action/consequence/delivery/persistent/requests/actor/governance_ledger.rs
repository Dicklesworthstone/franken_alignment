//! Separately granted observation of the ORIGINAL credibility and Andon ledgers.
//!
//! This reads one acknowledged owner, not caller-supplied scores or a replayed
//! replacement. Counts describe the original evaluation protocol and labels;
//! they do not authenticate the evaluator, establish population error bounds,
//! qualify a deployment or change the committee's publication authority.

use super::{
    ActorError, FileActorPort, FileActorSupervisor, JournalError, Knowledge, OwnerRef,
    UnknownReason,
};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::FileOversight;
use crate::action::consequence::oversight::credibility::{
    CredibilityReport, EvaluationProtocol, MAX_EVALUATION_CASES, MemberMetrics,
    QualificationFailure,
};
use crate::action::{ElapsedTick, Scope};
use crate::round::MAX_MEMBERS;
use crate::round::review::andon::{LedgerIdentity, MAX_FALSE_HOLDS, MAX_WINDOWS, WindowAccounting};
use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;
use std::rc::{Rc, Weak};

pub const MAX_GOVERNANCE_LEDGER_QUERIES: usize = 1024;

/// One historical acknowledged cut. These coordinates retain their original
/// meanings: a journal revision is not a control sequence or an authority epoch,
/// and recorded time is not a fresh clock observation or current source lease.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileGovernanceLedgerBasis {
    pub scope: Scope,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
    pub recorded_at: Option<ElapsedTick>,
}

/// Original protocol accounting without helper questions, input contracts,
/// individual ground-truth labels, evidence roots or a promotion capability.
/// The native failures are preserved even when they prevent qualification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCredibilityLedgerSnapshot {
    protocol: EvaluationProtocol,
    revision: u64,
    policy_generation: u64,
    scoped_cases: usize,
    retained_cases: usize,
    pending_cases: usize,
    censored_cases: usize,
    violation_origins: u64,
    benign_origins: u64,
    lifetime_false_stops: u64,
    calibration_incident_open: bool,
    members: BTreeMap<String, MemberMetrics>,
    failures: Vec<QualificationFailure>,
}

impl FileCredibilityLedgerSnapshot {
    pub fn protocol(&self) -> &EvaluationProtocol {
        &self.protocol
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn policy_generation(&self) -> u64 {
        self.policy_generation
    }
    pub fn scoped_cases(&self) -> usize {
        self.scoped_cases
    }
    pub fn retained_cases(&self) -> usize {
        self.retained_cases
    }
    pub fn pending_cases(&self) -> usize {
        self.pending_cases
    }
    pub fn censored_cases(&self) -> usize {
        self.censored_cases
    }
    pub fn violation_origins(&self) -> u64 {
        self.violation_origins
    }
    pub fn benign_origins(&self) -> u64 {
        self.benign_origins
    }
    pub fn lifetime_false_stops(&self) -> u64 {
        self.lifetime_false_stops
    }
    pub fn calibration_incident_open(&self) -> bool {
        self.calibration_incident_open
    }
    pub fn members(&self) -> &BTreeMap<String, MemberMetrics> {
        &self.members
    }
    pub fn failures(&self) -> &[QualificationFailure] {
        &self.failures
    }

    fn from_native(report: CredibilityReport) -> Result<Self, ActorError> {
        // The original ledger already enforces these finite inventory limits.
        // Refuse an inconsistent source rather than truncate its accounting.
        if report.retained_cases > MAX_EVALUATION_CASES || report.members.len() > MAX_MEMBERS {
            return Err(ActorError::Capacity);
        }
        Ok(Self {
            protocol: report.protocol,
            revision: report.revision,
            policy_generation: report.policy_generation,
            scoped_cases: report.scoped_cases,
            retained_cases: report.retained_cases,
            pending_cases: report.pending_cases,
            censored_cases: report.censored_cases,
            violation_origins: report.violation_origins,
            benign_origins: report.benign_origins,
            lifetime_false_stops: report.lifetime_false_stops,
            calibration_incident_open: report.calibration_incident_open,
            members: report.members,
            failures: report.failures,
        })
    }
}

/// Complete original window accounting without raw false-hold confirmations.
/// Costs remain u128; reads cannot reset a window, forgive a charge, close an
/// incident or change the independent evaluation protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAndonLedgerSnapshot {
    identity: LedgerIdentity,
    windows: BTreeMap<u64, WindowAccounting>,
}

impl FileAndonLedgerSnapshot {
    pub fn identity(&self) -> LedgerIdentity {
        self.identity
    }
    pub fn windows(&self) -> &BTreeMap<u64, WindowAccounting> {
        &self.windows
    }
}

/// Sealed observations taken under one immutable original-owner borrow. None
/// identifies an uninstalled subsystem in THIS complete acknowledged owner;
/// it does not describe another owner, unseen history or global absence.
///
/// ```compile_fail,E0451
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileGovernanceLedgerSnapshot;
/// fn forge(original: &FileGovernanceLedgerSnapshot) -> FileGovernanceLedgerSnapshot {
///     FileGovernanceLedgerSnapshot {
///         basis: original.basis(), credibility: None, andon: None,
///     }
/// }
/// ```
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileGovernanceLedgerSnapshot,
/// };
/// fn no_authority(snapshot: FileGovernanceLedgerSnapshot) -> FilePermit { snapshot }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileGovernanceLedgerSnapshot {
    basis: FileGovernanceLedgerBasis,
    credibility: Option<FileCredibilityLedgerSnapshot>,
    andon: Option<FileAndonLedgerSnapshot>,
}

impl FileGovernanceLedgerSnapshot {
    pub fn basis(&self) -> FileGovernanceLedgerBasis {
        self.basis
    }
    pub fn credibility(&self) -> Option<&FileCredibilityLedgerSnapshot> {
        self.credibility.as_ref()
    }
    pub fn andon(&self) -> Option<&FileAndonLedgerSnapshot> {
        self.andon.as_ref()
    }
}

pub type FileGovernanceLedgerObservation =
    Knowledge<FileGovernanceLedgerSnapshot, FileGovernanceLedgerBasis>;
pub type FileGovernanceLedgerResult = Result<FileGovernanceLedgerObservation, ActorError>;

struct Grant {
    remaining: Cell<usize>,
}

/// Noncloneable custody of a finite disclosure allowance. Dropping this grant
/// revokes all weak ports without retaining the original authority lock.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorGovernanceLedgerGrant;
/// fn no_copy(grant: FileSupervisorGovernanceLedgerGrant) { grant.clone(); }
/// ```
#[must_use = "retain custody for the explicitly granted governance ledger disclosure"]
pub struct FileSupervisorGovernanceLedgerGrant {
    grant: Rc<Grant>,
}

impl FileSupervisorGovernanceLedgerGrant {
    pub fn remaining_queries(&self) -> usize {
        self.grant.remaining.get()
    }
    pub fn revoke(self) {
        drop(self);
    }
}

impl fmt::Debug for FileSupervisorGovernanceLedgerGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileSupervisorGovernanceLedgerGrant")
            .field("remaining_queries", &self.remaining_queries())
            .finish_non_exhaustive()
    }
}

/// Weak, supervisor-only access to original acknowledged governance accounting.
/// Actor, precheck and other observation grants cannot provision this role.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::{
///     observed::FileOversight, requests::actor::FileActorPort,
/// };
/// fn no_self_grant(actor: FileActorPort<FileOversight>) { actor.governance_ledger_port(1); }
/// ```
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileSupervisorGovernanceLedgerPort;
/// fn no_owner(port: FileSupervisorGovernanceLedgerPort) { port.host_mut(); }
/// ```
#[derive(Clone)]
pub struct FileSupervisorGovernanceLedgerPort {
    owner: Weak<OwnerRef<FileOversight>>,
    issuer: Weak<()>,
    scope: Scope,
    grant: Weak<Grant>,
}

impl fmt::Debug for FileSupervisorGovernanceLedgerPort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FileSupervisorGovernanceLedgerPort { .. }")
    }
}

impl FileActorSupervisor<FileOversight> {
    /// Provision one separate shared allowance without reading ledger contents,
    /// consuming an admission snapshot, observing time or writing a journal.
    pub fn governance_ledger_port(
        &self,
        queries: usize,
    ) -> Result<
        (
            FileSupervisorGovernanceLedgerPort,
            FileSupervisorGovernanceLedgerGrant,
        ),
        JournalError,
    > {
        if queries == 0 {
            return Err(Error::InvalidInput.into());
        }
        if queries > MAX_GOVERNANCE_LEDGER_QUERIES {
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
        });
        Ok((
            FileSupervisorGovernanceLedgerPort {
                owner: Rc::downgrade(&self.owner),
                issuer: owner.host.journal_tail_identity(),
                scope: owner.host.actor_explanation_scope(),
                grant: Rc::downgrade(&grant),
            },
            FileSupervisorGovernanceLedgerGrant { grant },
        ))
    }
}

impl FileSupervisorGovernanceLedgerPort {
    pub(crate) fn belongs_to(&self, actor: &FileActorPort<FileOversight>) -> bool {
        Weak::ptr_eq(&self.owner, &actor.owner) && self.scope == actor.scope
    }

    pub(crate) fn capability_snapshot(&self) -> super::FileOperatorCapabilityObservation {
        super::capabilities::observe(
            &self.owner,
            &self.grant,
            self.scope,
            "supervisor_governance_ledger",
            |grant| {
                let budget = super::FileOperatorCapabilityBudget::new(&[(
                    "remaining_queries",
                    grant.remaining.get(),
                )]);
                Some((self.issuer.clone(), budget))
            },
        )
    }

    /// Capture complete original protocol and window accounting. Each entered
    /// query spends once before fallible native work, including explicit absence
    /// and repeated reads. Clones share spending; output failure cannot refund
    /// it. Revocation, a lost/replaced/faulted owner or a conflicting borrow enters
    /// no query and never falls back to an earlier captured result.
    pub fn ledger(&self) -> FileGovernanceLedgerResult {
        let Some(grant) = self.grant.upgrade() else {
            return Ok(Knowledge::Withheld {
                authority_required: "supervisor_governance_ledger",
            });
        };
        let unknown = || {
            Ok(Knowledge::Unknown {
                reason: UnknownReason::ControllerUnavailable,
            })
        };
        let Some(owner) = self.owner.upgrade() else {
            return unknown();
        };
        let Ok(owner) = owner.try_borrow() else {
            return unknown();
        };
        let host = &owner.host;
        if host.storage_failure().is_some()
            || host.actor_explanation_scope() != self.scope
            || !Weak::ptr_eq(&self.issuer, &host.journal_tail_identity())
        {
            return unknown();
        }
        let remaining = grant.remaining.get();
        if remaining == 0 {
            return Err(ActorError::Capacity);
        }
        grant.remaining.set(remaining - 1);

        // All following reads share this immutable original-owner borrow. No
        // callback, source, filesystem operation or reconstructed owner occurs.
        let snapshot = host.inspect();
        if snapshot.revision != host.revision() {
            return unknown();
        }
        let basis = FileGovernanceLedgerBasis {
            scope: self.scope,
            journal_revision: snapshot.revision,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch,
            recorded_at: snapshot.control.ledger.elapsed,
        };
        let credibility = match host.credibility_protocol() {
            Ok(None) => None,
            Ok(Some(protocol)) => {
                let Ok(report) = host.credibility_report() else {
                    return unknown();
                };
                if &report.protocol != protocol {
                    return unknown();
                }
                Some(FileCredibilityLedgerSnapshot::from_native(report)?)
            }
            Err(_) => return unknown(),
        };
        let andon = match host.andon_config() {
            Ok(None) => None,
            Ok(Some(config)) => {
                let Ok(ledger) = host.andon_ledger() else {
                    return unknown();
                };
                if credibility.is_none() || ledger.identity() != config.identity() {
                    return unknown();
                }
                if ledger.windows().len() > MAX_WINDOWS
                    || ledger.windows().values().any(|window| {
                        window
                            .incident
                            .as_ref()
                            .is_some_and(|incident| incident.rounds.len() > MAX_FALSE_HOLDS)
                    })
                {
                    return Err(ActorError::Capacity);
                }
                Some(FileAndonLedgerSnapshot {
                    identity: ledger.identity(),
                    windows: ledger.windows().clone(),
                })
            }
            Err(_) => return unknown(),
        };
        Ok(Knowledge::Known {
            value: FileGovernanceLedgerSnapshot {
                basis,
                credibility,
                andon,
            },
            basis,
        })
    }
}
