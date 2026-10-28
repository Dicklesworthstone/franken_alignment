//! Independent source-work custody paired with the exact acknowledged journal.
//! This is evidence transport, never source authentication or recovered authority.
mod credential;
mod credential_recovery;
mod recovery;
mod reconciled;
mod abandonment;
mod terminal;
pub use recovery::TopologyCheckpointResume;

use super::{FileDriverTopology, FileOversight, JournalError, LoadedAuthorityTopology,
    TOPOLOGY_REFRESH_READ_BYTES};
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor, anchored::{FileHistoryAnchor, FileHistoryAnchorError, MAX_HISTORY_ANCHOR_BYTES}};
use crate::action::consequence::delivery::persistent::observed::mediation::source::TopologySourceWork;
use crate::perimeter_inventory::{MAX_INVENTORY_BYTES, topology::MAX_TOPOLOGY_BYTES};
use crate::Error;
use std::{fmt, io::{self, Read, Write}};

const DOMAIN: &[u8; 8] = b"FATOPC\0\x01";
const HEADER_BYTES: usize = 73;
pub const MAX_TOPOLOGY_CHECKPOINT_BYTES: usize = HEADER_BYTES + MAX_TOPOLOGY_BYTES
    + MAX_INVENTORY_BYTES + MAX_HISTORY_ANCHOR_BYTES;

/// One independently retained cut: original input documents, actual spent work,
/// last source operation, native availability/floors, and original history anchor.
/// No live source, observer, certificate, owner or effect key is serialized.
///
/// The input documents are checked against the custodian's COMPLETE expected
/// input at capture, including unselected inventory metadata. They are not read
/// from possibly replaced files during capture. Import requires trusted custody;
/// framing is not authentication. Joint rollback and work since the last retained
/// checkpoint remain outside its guarantee. Sensitive Debug output is redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct TopologyCheckpoint {
    history: FileHistoryAnchor,
    topology: Vec<u8>,
    inventory: Vec<u8>,
    expected: LoadedAuthorityTopology,
    work: TopologySourceWork,
    last_operation: u64,
    available: bool,
    control_sequence: u64,
    authority_epoch: u64,
}
impl fmt::Debug for TopologyCheckpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TopologyCheckpoint").field("revision", &self.history.revision())
            .field("work", &self.work).finish_non_exhaustive()
    }
}

impl FileDriverTopology {
    /// Capture while the SAME healthy owner and source are held, including an
    /// acknowledged source refusal. Raw documents are the independently retained
    /// originals; changed live files cannot redefine the expectation. This checks
    /// the original source/owner binding and captures only acknowledged history.
    /// No read, clock call, source check, counter increment or journal write occurs.
    /// Parsing/copying/encoding are bounded synchronous work, not poll-time work.
    pub fn checkpoint(&self, host: &FileOversight, topology: &[u8], inventory: &[u8])
        -> Result<TopologyCheckpoint, JournalError>
    {
        self.source.check_driver_binding(host, host.revision())?;
        let expected = load(topology, inventory)?;
        if &expected != self.source.expected() { return Err(Error::Binding.into()); }
        let work = self.source.work();
        let last_operation = self.source.last_operation();
        check_work(&expected, work, last_operation)?;
        let state = host.inspect();
        let available = host.mediation_snapshot()?.available;
        Ok(TopologyCheckpoint { history: host.history_anchor()?,
            topology: copy(topology)?, inventory: copy(inventory)?, expected, work,
            last_operation, available, control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch })
    }
}

impl TopologyCheckpoint {
    pub fn history_anchor(&self) -> &FileHistoryAnchor { &self.history }
    pub fn expected(&self) -> &LoadedAuthorityTopology { &self.expected }
    pub fn topology_bytes(&self) -> &[u8] { &self.topology }
    pub fn inventory_bytes(&self) -> &[u8] { &self.inventory }
    pub fn work(&self) -> TopologySourceWork { self.work }
    pub fn last_operation(&self) -> u64 { self.last_operation }
    /// Historical availability, never a fresh cut certificate.
    pub fn available(&self) -> bool { self.available }
    pub fn minimum(&self) -> FileRecoveryFloor {
        FileRecoveryFloor { journal_revision: self.history.revision(),
            control_sequence: self.control_sequence, authority_epoch: self.authority_epoch }
    }

    /// Preflight the entire bounded archive before touching the supplied writer.
    /// Original anchor framing remains nested unchanged. No flush or sync occurs;
    /// only successful independent retention establishes custody. Partial output
    /// is not success and is never silently repaired or retried here.
    pub fn write_to(&self, writer: &mut impl Write, max_bytes: usize)
        -> Result<usize, FileHistoryAnchorError>
    {
        check_budget(max_bytes)?;
        check_work(&self.expected, self.work, self.last_operation)?;
        let anchor_len = self.history.write_to(&mut io::sink(), MAX_HISTORY_ANCHOR_BYTES)?;
        let sizes = [self.topology.len(), self.inventory.len(), anchor_len];
        let total = checked_sizes(sizes, max_bytes)?;
        let fields = [self.control_sequence, self.authority_epoch, self.work.checks,
            self.work.reserved_read_bytes, self.last_operation,
            sizes[0] as u64, sizes[1] as u64, sizes[2] as u64];
        let mut header = [0; HEADER_BYTES];
        header[..8].copy_from_slice(DOMAIN);
        for (index, value) in fields.iter().enumerate() {
            header[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_le_bytes());
        }
        header[72] = u8::from(self.available);
        writer.write_all(&header)?;
        writer.write_all(&self.topology)?;
        writer.write_all(&self.inventory)?;
        self.history.write_to(writer, anchor_len)?;
        Ok(total)
    }

    /// Import ONLY an independently retained finite archive. Check every length
    /// before allocation, use the original input parsers and anchor importer,
    /// reject trailing data and noncanonical flags, and preserve I/O errors.
    /// This does not replay the journal or certify the topology. Actual recovery
    /// must still select the embedded anchor, original profile, full mediated
    /// requirements and floors before issuing any owner. Never infer a new quota.
    pub fn read_trusted(reader: &mut impl Read, max_bytes: usize)
        -> Result<Self, FileHistoryAnchorError>
    {
        check_budget(max_bytes)?;
        let mut header = [0; HEADER_BYTES];
        reader.read_exact(&mut header)?;
        if &header[..8] != DOMAIN { return Err(Error::Binding.into()); }
        let mut fields = [0_u64; 8];
        for (index, value) in fields.iter_mut().enumerate() {
            *value = u64::from_le_bytes(header[8 + index * 8..16 + index * 8]
                .try_into().map_err(|_| Error::InvalidInput)?);
        }
        let available = match header[72] { 0 => false, 1 => true, _ => return Err(Error::InvalidInput.into()) };
        let mut sizes = [0_usize; 3];
        for (size, value) in sizes.iter_mut().zip(&fields[5..]) {
            *size = usize::try_from(*value).map_err(|_| Error::Limit)?;
        }
        checked_sizes(sizes, max_bytes)?;
        let topology = read_bytes(reader, sizes[0])?;
        let inventory = read_bytes(reader, sizes[1])?;
        let expected = load(&topology, &inventory)?;
        let work = TopologySourceWork { checks: fields[2], reserved_read_bytes: fields[3] };
        check_work(&expected, work, fields[4])?;
        let history = {
            let mut bounded = reader.by_ref().take(sizes[2] as u64);
            let history = FileHistoryAnchor::read_trusted(&mut bounded, sizes[2])?;
            if bounded.limit() != 0 { return Err(Error::Incomplete.into()); }
            history
        };
        let mut trailing = [0];
        loop {
            match reader.read(&mut trailing) {
                Ok(0) => break,
                Ok(_) => return Err(Error::InvalidInput.into()),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Ok(Self { history, topology, inventory, expected, work, last_operation: fields[4],
            available, control_sequence: fields[0], authority_epoch: fields[1] })
    }
}

fn check_budget(max_bytes: usize) -> Result<(), Error> {
    if !(HEADER_BYTES..=MAX_TOPOLOGY_CHECKPOINT_BYTES).contains(&max_bytes) { return Err(Error::Limit); }
    Ok(())
}
fn checked_sizes(sizes: [usize; 3], max_bytes: usize) -> Result<usize, Error> {
    if sizes[0] == 0 || sizes[0] > MAX_TOPOLOGY_BYTES || sizes[1] == 0
        || sizes[1] > MAX_INVENTORY_BYTES || sizes[2] == 0 || sizes[2] > MAX_HISTORY_ANCHOR_BYTES {
        return Err(Error::Limit);
    }
    let total = sizes.iter().try_fold(HEADER_BYTES, |total, size| total.checked_add(*size))
        .ok_or(Error::Limit)?;
    if total > max_bytes { return Err(Error::Limit); }
    Ok(total)
}
fn check_work(expected: &LoadedAuthorityTopology, work: TopologySourceWork, operation: u64)
    -> Result<(), Error>
{
    let limits = expected.source_limits();
    if work.checks.checked_mul(TOPOLOGY_REFRESH_READ_BYTES) != Some(work.reserved_read_bytes)
        || operation < work.checks { return Err(Error::Binding); }
    if work.checks > limits.checks || work.reserved_read_bytes > limits.read_bytes { return Err(Error::Limit); }
    Ok(())
}
fn load(topology: &[u8], inventory: &[u8]) -> Result<LoadedAuthorityTopology, Error> {
    if topology.len() > MAX_TOPOLOGY_BYTES || inventory.len() > MAX_INVENTORY_BYTES { return Err(Error::Limit); }
    LoadedAuthorityTopology::from_json_bytes(topology, inventory).map_err(|_| Error::Binding)
}
fn copy(bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let mut out = Vec::new();
    out.try_reserve_exact(bytes.len()).map_err(|_| Error::Limit)?;
    out.extend_from_slice(bytes);
    Ok(out)
}
fn read_bytes(reader: &mut impl Read, len: usize) -> Result<Vec<u8>, FileHistoryAnchorError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(len).map_err(|_| Error::Limit)?;
    bytes.resize(len, 0);
    reader.read_exact(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests;

