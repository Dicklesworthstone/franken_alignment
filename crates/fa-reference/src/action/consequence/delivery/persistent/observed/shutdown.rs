//! Coordinate terminal shutdown of a fixed set of ORIGINAL durable owners.
//! A campaign owns evidence, not those owners or a second authority ledger.
//! Per-file acknowledged cuts are not a distributed simultaneous-stop claim.

mod campaign;
mod canonical;
mod plan;
pub mod coordinator;
pub use plan::{FileShutdownMember, MAX_SHUTDOWN_PLAN_BYTES};
pub use campaign::FileShutdownCampaign;

use super::{FileOversight, FileOversightProfile, FileStopSweep, JournalError, journal};
use crate::action::{ElapsedTick, Scope};
use crate::action::consequence::delivery::{StopProgress, StopRequest};
use crate::action::consequence::delivery::fleet::FleetScope;
use crate::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const MAX_SHUTDOWN_DOMAINS: usize = 64;
pub const MAX_SHUTDOWN_ATTEMPTS: usize = 512;
pub const MAX_SHUTDOWN_HEAD_BYTES: usize = 32 * 1024 * 1024;

/// Operator-retained registration. Native capture records an acknowledged owner;
/// a restored registration must still be compared with its actual domain.
/// The exact history prefix binds guards and source identity as well as counters.
/// Clones share historical bytes; they retain no owner, file lock or live keys.
#[derive(Clone)]
pub struct FileShutdownDomain {
    id: u64,
    directory: PathBuf,
    profile: Rc<FileOversightProfile>,
    anchor: Rc<[u8]>,
    revision: usize,
}
impl FileShutdownDomain {
    pub fn id(&self) -> u64 { self.id }
    pub fn directory(&self) -> &Path { &self.directory }
    pub fn scope(&self) -> Scope { self.profile.delivery.scope }
    pub fn clock_domain(&self) -> u64 { self.profile.delivery.clock_domain }
    pub fn registered_revision(&self) -> u64 { self.revision as u64 }
    pub fn retained_anchor_bytes(&self) -> usize { self.anchor.len() }
}
impl fmt::Debug for FileShutdownDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileShutdownDomain").field("id", &self.id)
            .field("scope", &self.scope()).field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}

impl FileOversight {
    /// Supervisor registration; an actor port cannot obtain this history. This
    /// neither reserves rights nor stops admission. Include EVERY intended
    /// domain in the plan, including a registered domain that later goes offline.
    pub fn shutdown_domain(&self, id: u64) -> Result<FileShutdownDomain, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if id == 0 { return Err(Error::InvalidInput.into()); }
        let anchor = journal::encode(&self.profile, self.store.identity(), &self.events)?;
        if anchor.len() > MAX_SHUTDOWN_HEAD_BYTES { return Err(Error::Limit.into()); }
        Ok(FileShutdownDomain { id, directory: self.store.identity().to_path_buf(),
            profile: Rc::new(self.profile.clone()), anchor: anchor.into(), revision: self.events.len() })
    }
}

/// Fixed denominator and bounded work registration. No method adds/removes a
/// domain after construction. The operation identifies an ORIGINAL StopRequest
/// in each domain; it supplies no cross-process ordering or authentication.
#[derive(Clone, Debug)]
pub struct FileShutdownPlan {
    operation: u64,
    scope: FleetScope,
    domains: Rc<[FileShutdownDomain]>,
    max_attempts: usize,
    max_head_bytes: usize,
}
impl FileShutdownPlan {
    pub fn new(operation: u64, mut domains: Vec<FileShutdownDomain>, max_attempts: usize,
        max_head_bytes: usize) -> Result<Self, Error>
    {
        if operation == 0 || domains.is_empty() || max_attempts == 0 || max_head_bytes == 0 {
            return Err(Error::InvalidInput);
        }
        if domains.len() > MAX_SHUTDOWN_DOMAINS || max_attempts > MAX_SHUTDOWN_ATTEMPTS
            || max_head_bytes > MAX_SHUTDOWN_HEAD_BYTES { return Err(Error::Limit); }
        let mut bytes = 0_usize;
        for (index, domain) in domains.iter().enumerate() {
            if domains[..index].iter().any(|previous| previous.id == domain.id
                || previous.directory == domain.directory || previous.scope() == domain.scope())
            { return Err(Error::Duplicate); }
            bytes = bytes.checked_add(domain.anchor.len()).ok_or(Error::Limit)?;
        }
        if bytes > max_head_bytes { return Err(Error::Limit); }
        domains.sort_by_key(|domain| domain.id);
        let scope = FleetScope::Domains(domains.iter().map(|domain| domain.id).collect());
        Ok(Self { operation, scope, domains: domains.into(), max_attempts, max_head_bytes })
    }
    pub fn operation(&self) -> u64 { self.operation }
    pub fn scope(&self) -> &FleetScope { &self.scope }
    pub fn domains(&self) -> &[FileShutdownDomain] { &self.domains }
    pub fn max_attempts(&self) -> usize { self.max_attempts }
    pub fn max_head_bytes(&self) -> usize { self.max_head_bytes }
    pub fn start(&self) -> FileShutdownCampaign { FileShutdownCampaign::new(self.clone()) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileShutdownSource { AcknowledgedOwner, CanonicalImage }

/// The original sweep at its own journal cut, including every refused outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownDrain {
    pub journal_revision: u64,
    pub sweep: FileStopSweep,
}

/// Evidence at one local cut. Dispatches are reported since this domain's
/// REGISTRATION, not falsely ordered against a fleet-wide wall-clock instant.
/// A drained domain can still have charged, already executed effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownObservation {
    pub source: FileShutdownSource,
    pub journal_revision: u64,
    pub control_sequence: u64,
    pub authority_epoch: u64,
    pub executions: u64,
    pub stop: Option<StopProgress>,
    /// Latest native sweep observed by this campaign, not an invented receipt.
    /// Canonical inspection reconstructs it; a live-only inspection may lack it.
    pub last_drain: Option<FileShutdownDrain>,
    pub dispatches_since_registration: Vec<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileShutdownStep {
    InspectOwner,
    InspectCanonical,
    Stop(StopRequest),
    Drain { clock_domain: u64, at: ElapsedTick },
    Unavailable,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileShutdownResult {
    /// Installed before entering work. A caught unwind cannot become success.
    Interrupted,
    Observed(Box<FileShutdownObservation>),
    Refused(JournalError),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownAttempt {
    pub domain: u64,
    pub step: FileShutdownStep,
    pub result: FileShutdownResult,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownDomainReport {
    pub domain: u64,
    pub scope: Scope,
    pub clock_domain: u64,
    pub registered_revision: u64,
    /// Historical successful observation is retained even after a later failure.
    pub last_observation: Option<FileShutdownObservation>,
    /// An unavailable/error/interrupted later attempt invalidates aggregate success.
    pub latest_succeeded: bool,
}

/// Report data cannot substitute for a native endpoint receipt or effect key.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::delivery::persistent::observed::shutdown::FileShutdownReport;
/// use fa_reference::action::Permit;
/// fn grant(report: FileShutdownReport) -> Permit { report }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileShutdownReport {
    pub operation: u64,
    pub domains: Vec<FileShutdownDomainReport>,
    pub attempts: Vec<FileShutdownAttempt>,
    pub retained_head_bytes: usize,
}
impl FileShutdownReport {
    pub fn unobserved_stops(&self) -> Vec<u64> {
        self.domains.iter().filter(|domain| !self.stopped(domain)).map(|domain| domain.domain).collect()
    }
    /// All registered domains, not merely those that responded. These are the
    /// last local observations, NOT a simultaneous snapshot or remote halt proof.
    pub fn all_observed_stopped(&self) -> bool {
        !self.domains.is_empty() && self.domains.iter().all(|domain| self.stopped(domain))
    }
    pub fn all_observed_drained(&self) -> bool {
        self.all_observed_stopped() && self.domains.iter().all(|domain| {
            domain.last_observation.as_ref().and_then(|observation| observation.stop.as_ref())
                .is_some_and(StopProgress::drained)
        })
    }
    fn stopped(&self, domain: &FileShutdownDomainReport) -> bool {
        domain.latest_succeeded && domain.last_observation.as_ref()
            .and_then(|observation| observation.stop.as_ref())
            .is_some_and(|stop| stop.receipt.request().operation == self.operation)
    }
}

#[cfg(test)]
mod tests;
