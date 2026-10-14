//! Fresh, bounded file intake for the ORIGINAL mediation observer.
//! Trusted directories and independently provisioned expectations are required.
//! This detects source loss/substitution at explicit checks, not all OS bypasses.
mod publication;
pub use publication::{TopologyPublication, TopologyPublicationReport};

use super::{FileMediationObserver, FileMediationUpdate, FileOversight, JournalError};
use super::super::{JournalFailure, JournalIo};
use crate::action::consequence::delivery::TopologyChange;
use crate::action::consequence::mediation::CutCheck;
use crate::perimeter_inventory::{InventoryLoadError, MAX_INVENTORY_BYTES};
use crate::perimeter_inventory::topology::{LoadedAuthorityTopology, TopologySourceLimits, MAX_TOPOLOGY_BYTES};
use crate::Error;
use std::{fmt, fs::{self, File, Metadata}, io::{self, Read}, path::{Component, Path, PathBuf}};
use std::os::unix::fs::MetadataExt;

/// Reserve both full bounded inputs and their one-byte EOF probes BEFORE I/O.
/// Logical read admission, not bytes actually transferred or time spent.
pub const TOPOLOGY_REFRESH_READ_BYTES: u64 = (MAX_TOPOLOGY_BYTES + MAX_INVENTORY_BYTES + 2) as u64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TopologySourceWork {
    pub checks: u64,
    pub reserved_read_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TopologySourceRefusal {
    /// Installed before work; retained when an unwind prevents a complete result.
    Interrupted,
    BudgetExhausted,
    CheckNotAcknowledged,
    Input(InventoryLoadError),
    /// Includes metadata in unselected families, not merely selected graph edges.
    ChangedInput,
    /// Recovery, prior loss or a native fence requires explicit newer evidence.
    CoverageUnavailable,
    /// The ORIGINAL checker returned a refusal, bypass or unreachable sink.
    NotCertified,
}

/// Native check and withdrawal results, not a permit or a remote attestation.
/// An inner refusal means withdrawal is acknowledged or coverage was already
/// unavailable. An outer JournalError can leave the owner quarantined instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopologyRefresh {
    pub journal_revision: u64,
    pub work: TopologySourceWork,
    pub checked: Option<Result<CutCheck, Error>>,
    pub refusal: Option<TopologySourceRefusal>,
    pub withdrawal: Option<TopologyChange>,
}

/// Exclusive process-local custody of the original observer. No observer getter,
/// owner borrow, raw descriptor or clone is exposed. Dropping does not renew work
/// or restore a withdrawn cut. All reads use these fixed paths and expectations.
/// Scheduling checks at admission/publication is the supervisor's obligation;
/// this is not a background watcher or a new mandatory deployment profile.
///
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::FileTopologySource;
/// fn duplicate(source: FileTopologySource) { let _ = source.clone(); }
/// ```
#[must_use = "retain the original source custodian while its checks are required"]
pub struct FileTopologySource {
    observer: FileMediationObserver,
    topology_path: PathBuf,
    inventory_path: PathBuf,
    expected: LoadedAuthorityTopology,
    limits: TopologySourceLimits,
    work: TopologySourceWork,
    last_operation: u64,
    last_rebind: Option<FileMediationUpdate>,
    last_refusal: Option<TopologySourceRefusal>,
    last_check: Option<Result<CutCheck, Error>>,
}
impl fmt::Debug for FileTopologySource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileTopologySource").field("work", &self.work)
            .field("last_operation", &self.last_operation).finish_non_exhaustive()
    }
}

impl FileMediationObserver {
    /// Bind independently selected complete input data to an already enabled
    /// native graph. Consumes separate observer custody, without reading files,
    /// changing the journal, certifying a cut or issuing an actor capability.
    pub fn bind_topology_source(self, host: &FileOversight, revision: u64,
        topology_path: PathBuf, inventory_path: PathBuf, expected: LoadedAuthorityTopology)
        -> Result<FileTopologySource, JournalError>
    {
        self.check_owner(host)?;
        if revision != host.revision() { return Err(Error::Stale.into()); }
        for path in [&topology_path, &inventory_path] {
            if !path.is_absolute() || path.file_name().is_none()
                || path.components().any(|c| matches!(c, Component::ParentDir)) {
                return Err(Error::InvalidInput.into());
            }
        }
        if topology_path == inventory_path { return Err(Error::Binding.into()); }
        let current = host.mediation_snapshot()?;
        if &current.graph != expected.graph() { return Err(Error::Binding.into()); }
        if !current.available { return Err(Error::Incomplete.into()); }
        let limits = expected.source_limits();
        Ok(FileTopologySource { observer: self, topology_path, inventory_path, expected, limits,
            work: TopologySourceWork::default(), last_operation: 0, last_rebind: None,
            last_refusal: None, last_check: None })
    }
}

impl FileTopologySource {
    pub fn work(&self) -> TopologySourceWork { self.work }
    pub fn limits(&self) -> TopologySourceLimits { self.limits }
    pub fn expected(&self) -> &LoadedAuthorityTopology { &self.expected }
    /// Keep source refusal distinct from a subsequent failed durable withdrawal.
    pub fn last_refusal(&self) -> Option<&TopologySourceRefusal> { self.last_refusal.as_ref() }
    pub fn last_check(&self) -> Option<&Result<CutCheck, Error>> { self.last_check.as_ref() }

    /// Read BOTH files anew, compare the entire loaded contract, then invoke the
    /// original durable independent checker. No self-derived expected profile,
    /// cached-input fallback or parser-supplied certificate is accepted.
    /// Fresh operation IDs increase within this source lifetime; stale caller
    /// predecessors and foreign owners refuse before reads or eligibility loss.
    pub fn refresh(&mut self, host: &mut FileOversight, revision: u64, operation: u64)
        -> Result<TopologyRefresh, JournalError>
    {
        self.refresh_with(host, revision, operation, read_regular)
    }

    /// Explicit independently provisioned replacement, not automatic adoption of
    /// changed file contents. New graph AND inventory generations are mandatory.
    /// Commit the original topology revocation before accepting the expectation;
    /// a subsequent fresh refresh must certify it. Work/paths/limits never reset.
    /// Exact acknowledged rebind retries return the original native receipt.
    pub fn rebind(&mut self, host: &mut FileOversight, revision: u64, operation: u64,
        expected: LoadedAuthorityTopology) -> Result<Option<TopologyChange>, JournalError>
    {
        self.observer.check_owner(host)?;
        if let Some(previous) = &self.last_rebind {
            if previous.operation == operation {
                if expected != self.expected { return Err(Error::Binding.into()); }
                return self.observer.update(host, revision, previous);
            }
        }
        self.check_call(host, revision, operation)?;
        if expected.source_limits() != self.limits
            || expected.check_edge_budget() != self.expected.check_edge_budget() {
            return Err(Error::Binding.into());
        }
        let before = self.expected.graph().spec();
        let after = expected.graph().spec();
        if after.generation <= before.generation || after.inventory_generation <= before.inventory_generation {
            return Err(Error::Stale.into());
        }
        let update = FileMediationUpdate { operation, expected_generation: before.generation,
            expected_authority_epoch: host.inspect().control.ledger.epoch,
            next: Some(expected.graph().clone()) };
        let change = self.observer.update(host, revision, &update)?;
        self.last_operation = operation;
        self.last_rebind = Some(update);
        self.expected = expected;
        Ok(change)
    }

    fn check_call(&self, host: &FileOversight, revision: u64, operation: u64) -> Result<(), JournalError> {
        self.observer.check_owner(host)?;
        if revision != host.revision() || operation <= self.last_operation { return Err(Error::Stale.into()); }
        if &host.mediation_snapshot()?.graph != self.expected.graph() { return Err(Error::Binding.into()); }
        Ok(())
    }

    // Private fault seam. Public callers cannot supply bytes, readers or success.
    fn refresh_with(&mut self, host: &mut FileOversight, revision: u64, operation: u64,
        mut read: impl FnMut(&Path, usize) -> Result<Vec<u8>, InventoryLoadError>)
        -> Result<TopologyRefresh, JournalError>
    {
        self.check_call(host, revision, operation)?;
        self.last_operation = operation;
        self.last_refusal = Some(TopologySourceRefusal::Interrupted);
        self.last_check = None;
        if !host.mediation_snapshot()?.available {
            self.last_refusal = Some(TopologySourceRefusal::CoverageUnavailable);
            return Ok(self.report(host, None, self.last_refusal.clone(), None));
        }
        let mut pending = PendingSource { host, armed: true };
        let result = self.refresh_checked(&mut *pending.host, operation, &mut read);
        pending.armed = result.is_err();
        result
    }

    fn refresh_checked(&mut self, host: &mut FileOversight, operation: u64,
        read: &mut impl FnMut(&Path, usize) -> Result<Vec<u8>, InventoryLoadError>)
        -> Result<TopologyRefresh, JournalError>
    {
        let admitted = self.work.checks < self.limits.checks
            && self.work.reserved_read_bytes.checked_add(TOPOLOGY_REFRESH_READ_BYTES)
                .is_some_and(|bytes| bytes <= self.limits.read_bytes);
        if !admitted {
            return self.withdraw(host, operation, TopologySourceRefusal::BudgetExhausted, None);
        }
        self.work.checks += 1;
        self.work.reserved_read_bytes += TOPOLOGY_REFRESH_READ_BYTES;
        // A caught unwind during read/parse cannot expose yesterday's permitting
        // state. Only this exclusive call can remove its own temporary latch.
        quarantine(host);
        let loaded = (|| {
            let topology = read(&self.topology_path, MAX_TOPOLOGY_BYTES)?;
            let inventory = read(&self.inventory_path, MAX_INVENTORY_BYTES)?;
            LoadedAuthorityTopology::from_json_bytes(&topology, &inventory)
        })();
        host.fault = None;
        let refusal = match loaded {
            Err(error) => Some(TopologySourceRefusal::Input(error)),
            Ok(loaded) if loaded != self.expected => Some(TopologySourceRefusal::ChangedInput),
            Ok(_) => None,
        };
        if let Some(refusal) = refusal { return self.withdraw(host, operation, refusal, None); }
        // PendingSource restores quarantine on any unwind through verification
        // or withdrawal, including before the native storage latch is installed.
        let epoch = host.inspect().control.ledger.epoch;
        let revision = host.revision();
        let checked = self.observer.certify(host, revision, epoch,
            self.expected.proposal(), self.expected.check_edge_budget());
        let checked = match checked {
            Ok(checked) => checked,
            Err(error) => {
                self.last_refusal = Some(TopologySourceRefusal::CheckNotAcknowledged);
                quarantine(host); return Err(error);
            }
        };
        self.last_check = Some(checked.clone());
        if matches!(&checked, Ok(CutCheck::Verified(_))) {
            self.last_refusal = None;
            Ok(self.report(host, Some(checked), None, None))
        } else {
            self.withdraw(host, operation, TopologySourceRefusal::NotCertified, Some(checked))
        }
    }

    fn withdraw(&mut self, host: &mut FileOversight, operation: u64, refusal: TopologySourceRefusal,
        checked: Option<Result<CutCheck, Error>>) -> Result<TopologyRefresh, JournalError>
    {
        self.last_refusal = Some(refusal.clone());
        let current = host.mediation_snapshot()?;
        let update = FileMediationUpdate { operation, expected_generation: current.graph.spec().generation,
            expected_authority_epoch: host.inspect().control.ledger.epoch, next: None };
        // Native update installs its loss latch before encoding and storage. If
        // even preflight cannot record loss, retain quarantine explicitly.
        let revision = host.revision();
        match self.observer.update(host, revision, &update) {
            Ok(change) => Ok(self.report(host, checked, Some(refusal), change)),
            Err(error) => { quarantine(host); Err(error) }
        }
    }

    fn report(&self, host: &FileOversight, checked: Option<Result<CutCheck, Error>>,
        refusal: Option<TopologySourceRefusal>, withdrawal: Option<TopologyChange>) -> TopologyRefresh
    {
        TopologyRefresh { journal_revision: host.revision(), work: self.work, checked, refusal, withdrawal }
    }
}

struct PendingSource<'a> { host: &'a mut FileOversight, armed: bool }
impl Drop for PendingSource<'_> {
    fn drop(&mut self) { if self.armed { quarantine(self.host); } }
}

fn quarantine(host: &mut FileOversight) {
    if host.fault.is_none() {
        host.fault = Some(JournalFailure { operation: JournalIo::Stage,
            kind: io::ErrorKind::Other, replacement_may_be_visible: false });
    }
}

fn same_file(a: &Metadata, b: &Metadata) -> bool {
    a.is_file() && b.is_file() && a.dev() == b.dev() && a.ino() == b.ino() && a.len() == b.len()
        && a.mtime() == b.mtime() && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime() && a.ctime_nsec() == b.ctime_nsec()
}
fn read_regular(path: &Path, maximum: usize) -> Result<Vec<u8>, InventoryLoadError> {
    let before = fs::symlink_metadata(path).map_err(|_| InventoryLoadError::Unreadable)?;
    if !before.is_file() || before.file_type().is_symlink() || before.len() > maximum as u64 {
        return Err(InventoryLoadError::Unreadable);
    }
    let mut file = File::open(path).map_err(|_| InventoryLoadError::Unreadable)?;
    if !same_file(&before, &file.metadata().map_err(|_| InventoryLoadError::Unreadable)?) {
        return Err(InventoryLoadError::Unreadable);
    }
    let mut bytes = Vec::new();
    (&mut file).take(maximum as u64 + 1).read_to_end(&mut bytes).map_err(|_| InventoryLoadError::Unreadable)?;
    if bytes.len() > maximum { return Err(InventoryLoadError::Schema(Error::Limit)); }
    let after = fs::symlink_metadata(path).map_err(|_| InventoryLoadError::Unreadable)?;
    if after.file_type().is_symlink() || !same_file(&before, &after)
        || !same_file(&before, &file.metadata().map_err(|_| InventoryLoadError::Unreadable)?) {
        return Err(InventoryLoadError::Unreadable);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
