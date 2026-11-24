//! Native calibration findings from one original acknowledged owner.
//!
//! These are the incidents already selected by the original immutable evaluation
//! and Andon contracts, not caller-authored risk scores or a second detector.
//! Reading them spends the SAME finite disclosure grant as ledger, performs
//! no journal write and cannot change a helper, policy, budget or incident.
//! Full FA-125 standing queries remain explicitly unknown until their native
//! sources and predicates are implemented; no empty list stands in for them.

use super::{
    ActorError, FileGovernanceLedgerBasis, FileGovernanceLedgerSnapshot,
    FileSupervisorGovernanceLedgerPort, Knowledge,
};
use crate::action::consequence::oversight::credibility::EvaluationProtocol;
use crate::governance_assurance::{RiskTheaterFamily, RiskTheaterFinding};
use crate::observation_algebra::{
    Knowledge as QueryKnowledge, UnknownReason as QueryUnknownReason,
};
use crate::round::review::andon::{LedgerIdentity, WindowAccounting};
use std::collections::BTreeMap;

/// The original evaluation protocol's lifetime false-stop predicate. The
/// counter includes actual stopped rounds with independent Benign labels; it
/// is not an estimate of a population false-positive rate. Its threshold is
/// the protocol's original budget, never a threshold supplied with this query.
///
/// This snapshot does not disclose individual case labels or invent round
/// supports absent from the original report. Andon incident supports, when
/// that subsystem is installed, are available separately below.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileCredibilityCalibrationSnapshot {
    protocol: EvaluationProtocol,
    revision: u64,
    policy_generation: u64,
    lifetime_false_stops: u64,
    calibration_incident_open: bool,
}

impl FileCredibilityCalibrationSnapshot {
    pub fn protocol(&self) -> &EvaluationProtocol {
        &self.protocol
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn policy_generation(&self) -> u64 {
        self.policy_generation
    }
    pub fn lifetime_false_stops(&self) -> u64 {
        self.lifetime_false_stops
    }
    pub fn calibration_incident_open(&self) -> bool {
        self.calibration_incident_open
    }
}

/// Complete open-incident projection over the original registered Andon
/// windows. An installed source with an empty incidents map is a known empty
/// result ONLY for this exact source cut and predicate. It does not establish
/// that the control system has no risk-theater findings.
///
/// Window identities, occurrence-time bounds, immutable budgets, full-width
/// costs, the opening round and every retained supporting round are preserved.
/// Closed historical windows remain visible: crossing into a new occurrence
/// window cannot forgive an old false stop or close its native incident.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAndonCalibrationFindings {
    identity: LedgerIdentity,
    inspected_windows: usize,
    incidents: BTreeMap<u64, WindowAccounting>,
}

impl FileAndonCalibrationFindings {
    pub fn identity(&self) -> LedgerIdentity {
        self.identity
    }
    pub fn inspected_windows(&self) -> usize {
        self.inspected_windows
    }
    pub fn incidents(&self) -> &BTreeMap<u64, WindowAccounting> {
        &self.incidents
    }
}

/// Sealed findings captured through the original supervisor-only ledger grant.
/// The immutable source cut applies to BOTH native subqueries. None means
/// the corresponding source is not installed in that acknowledged owner; a
/// consumer must expose that query as unknown, never an empty clean finding.
///
/// The risk_theater map explicitly covers all six FULL FA-125 queries. They
/// remain unknown, including Andon closure modes, even when the separate native
/// budget-exhaustion predicate is observable. This prevents a useful partial
/// integration from silently becoming a complete governance assurance claim.
///
/// ```compile_fail,E0451
/// use fa_reference::action::consequence::delivery::persistent::requests::actor::FileGovernanceFindingsSnapshot;
/// fn forge(original: &FileGovernanceFindingsSnapshot) -> FileGovernanceFindingsSnapshot {
///     FileGovernanceFindingsSnapshot {
///         basis: original.basis(), credibility: None, andon: None,
///         risk_theater: Default::default(),
///     }
/// }
/// ```
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::{
///     FilePermit, requests::actor::FileGovernanceFindingsSnapshot,
/// };
/// fn no_permission(findings: FileGovernanceFindingsSnapshot) -> FilePermit { findings }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileGovernanceFindingsSnapshot {
    basis: FileGovernanceLedgerBasis,
    credibility: Option<FileCredibilityCalibrationSnapshot>,
    andon: Option<FileAndonCalibrationFindings>,
    risk_theater: BTreeMap<RiskTheaterFamily, QueryKnowledge<Vec<RiskTheaterFinding>>>,
}

impl FileGovernanceFindingsSnapshot {
    pub fn basis(&self) -> FileGovernanceLedgerBasis {
        self.basis
    }
    pub fn credibility(&self) -> Option<&FileCredibilityCalibrationSnapshot> {
        self.credibility.as_ref()
    }
    pub fn andon(&self) -> Option<&FileAndonCalibrationFindings> {
        self.andon.as_ref()
    }
    pub fn risk_theater(
        &self,
    ) -> &BTreeMap<RiskTheaterFamily, QueryKnowledge<Vec<RiskTheaterFinding>>> {
        &self.risk_theater
    }

    fn from_ledger(snapshot: FileGovernanceLedgerSnapshot) -> Self {
        let credibility = snapshot
            .credibility
            .map(|ledger| FileCredibilityCalibrationSnapshot {
                protocol: ledger.protocol,
                revision: ledger.revision,
                policy_generation: ledger.policy_generation,
                lifetime_false_stops: ledger.lifetime_false_stops,
                calibration_incident_open: ledger.calibration_incident_open,
            });
        let andon = snapshot.andon.map(|ledger| FileAndonCalibrationFindings {
            identity: ledger.identity,
            inspected_windows: ledger.windows.len(),
            incidents: ledger
                .windows
                .into_iter()
                .filter(|(_, window)| window.incident.is_some())
                .collect(),
        });
        Self {
            basis: snapshot.basis,
            credibility,
            andon,
            risk_theater: RiskTheaterFamily::ALL
                .into_iter()
                .map(|family| {
                    (
                        family,
                        QueryKnowledge::Unknown {
                            reason: QueryUnknownReason::LowerLayerUnverified,
                        },
                    )
                })
                .collect(),
        }
    }
}

pub type FileGovernanceFindingsObservation =
    Knowledge<FileGovernanceFindingsSnapshot, FileGovernanceLedgerBasis>;
pub type FileGovernanceFindingsResult = Result<FileGovernanceFindingsObservation, ActorError>;

impl FileSupervisorGovernanceLedgerPort {
    /// Observe original calibration incidents with one original ledger query.
    /// Clones, Ledger reads and Findings reads all share the SAME allowance.
    /// A lost, replaced, borrowed or storage-faulted owner supplies no cached
    /// fallback; it remains unknown without spending an unentered query.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::delivery::persistent::{
    ///     observed::FileOversight, requests::actor::FileActorPort,
    /// };
    /// fn no_actor_query(actor: FileActorPort<FileOversight>) { actor.findings(); }
    /// ```
    pub fn findings(&self) -> FileGovernanceFindingsResult {
        Ok(match self.ledger()? {
            Knowledge::Known { value, basis } => Knowledge::Known {
                value: FileGovernanceFindingsSnapshot::from_ledger(value),
                basis,
            },
            Knowledge::Pending { request } => Knowledge::Pending { request },
            Knowledge::Unknown { reason } => Knowledge::Unknown { reason },
            Knowledge::Withheld { authority_required } => {
                Knowledge::Withheld { authority_required }
            }
            Knowledge::Stale {
                generation,
                current,
            } => Knowledge::Stale {
                generation,
                current,
            },
            Knowledge::Absent {
                closed_domain,
                frontier,
            } => Knowledge::Absent {
                closed_domain,
                frontier,
            },
        })
    }
}
