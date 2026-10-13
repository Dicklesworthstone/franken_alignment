//! Unix local-file adapter with exclusive writer locking and sync-before-ack.
//!
//! The directory must be trusted, and the filesystem must honor file locks and
//! file/directory synchronization. This is not a hostile-writer, distributed,
//! remote-receipt authentication, or exactly-once external-execution boundary.

mod recovery;
mod reserve;
mod batch_recovery;
mod batch_commit;
pub use batch_commit::{BatchCommitBudget, BatchCommitReport, BatchCommitStage, FileBatchCommit};
pub use batch_recovery::{BatchAnchor, BatchStatus, BatchInspectionBudget, BatchInspectionReport,
    BatchInspectionStatus, BatchInspectionWork, FileBatchInspection};
pub use recovery::FileRecovery;

use crate::{Event, Head, Identity, Journal, Limits};
use crate::batch::BatchLimits;
use fa_reference::Rights;
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io::{self, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[derive(Debug)]
pub enum Error {
    Journal(crate::Error),
    Io(io::Error),
    Busy,
    Poisoned,
    /// A consumed recovery had not yet verified clean EOF and anchor coverage.
    RecoveryIncomplete,
    /// A consumed new batch had not yet written its complete canonical group.
    CommitIncomplete,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "file rights journal: {self:?}")
    }
}

impl std::error::Error for Error {}

impl From<crate::Error> for Error {
    fn from(value: crate::Error) -> Self {
        Self::Journal(value)
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

/// One locked writer. No raw file or mutable rights state is exposed.
///
/// A failed write/sync has an *unknown* persistence outcome. The object becomes
/// poisoned before I/O starts, including on unwinding. Drop it and reopen using
/// the last externally retained acknowledged head; do not refund or blindly
/// retry the attempted operation. Recovery may include its complete record.
#[derive(Debug)]
pub struct FileJournal {
    file: File,
    journal: Journal,
    poisoned: bool,
}

impl FileJournal {
    /// Create a new journal without replacing an existing path. The header and
    /// parent directory are synced before success. A failure may leave a partial
    /// file for diagnosis; it is never silently deleted or replaced.
    pub fn create(
        path: impl AsRef<Path>,
        identity: Identity,
        limits: Limits,
    ) -> Result<Self, Error> {
        Self::create_journal(path, Journal::new(identity, limits)?)
    }

    // Both profiles publish their complete original header in the first image.
    fn create_journal(path: impl AsRef<Path>, journal: Journal) -> Result<Self, Error> {
        let path = path.as_ref();
        let directory = parent_directory(path)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        lock(&file)?;
        file.write_all(&journal.header())?;
        file.sync_all()?;
        directory.sync_all()?;
        Ok(Self { file, journal, poisoned: false })
    }

    /// Reopen without creating, truncating, repairing, refunding, or dispatching.
    /// `minimum` must be obtained independently of the file being replayed.
    /// Any complete suffix after it is replayed and synced before state is usable.
    pub fn open(
        path: impl AsRef<Path>,
        identity: Identity,
        minimum: Head,
        limits: Limits,
    ) -> Result<Self, Error> {
        let mut recovery = Self::begin_open(path, identity, minimum, limits)?;
        while recovery.progress().status == crate::replay::ReplayStatus::Reading {
            recovery.advance(crate::replay::ReplayBudget::UNLIMITED)?;
        }
        recovery.finish()
    }

    /// Persist one checked transition before returning its new recovery anchor.
    /// A dispatch record is accounting, not authorization or an external send.
    pub fn commit(&mut self, event: &Event) -> Result<Head, Error> {
        self.commit_with(event, |file, bytes| {
            file.write_all(bytes)?;
            file.sync_all()
        })
    }

    /// Persist a nonempty ordered group with ONE final file synchronization.
    /// All events and limits are checked before any file I/O. Only the final
    /// head is acknowledged; no partially committed accounting is returned.
    ///
    /// This is not disk atomicity: failed writes/syncs can leave a complete
    /// prefix or a truncated record. The owner stays poisoned until dropped;
    /// reopen under the independently retained head and original strict rules.
    /// Never infer a refund or replay a dispatch merely because this call failed.
    /// External sends/outcome observations keep their separate causal barriers.
    pub fn commit_batch(&mut self, events: &[Event], limits: BatchLimits) -> Result<Head, Error> {
        self.commit_batch_with(events, limits, |file, bytes| {
            file.write_all(bytes)?;
            file.sync_all()
        })
    }

    /// The last successfully published head, usable as a *minimum* on recovery.
    /// After an I/O error the file may contain a complete later record.
    pub fn acknowledged_head(&self) -> Head {
        self.journal.head()
    }

    /// Never expose stale accounting after an uncertain persistence outcome.
    pub fn rights(&self) -> Result<&Rights, Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        Ok(self.journal.rights())
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    fn commit_with(
        &mut self,
        event: &Event,
        persist: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> Result<Head, Error> {
        self.commit_batch_with(std::slice::from_ref(event), BatchLimits::default(), persist)
    }

    fn commit_batch_with(
        &mut self,
        events: &[Event],
        limits: BatchLimits,
        persist: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> Result<Head, Error> {
        if self.poisoned {
            return Err(Error::Poisoned);
        }
        let (candidate, record) = self.journal.prepare_batch(events, limits)?;
        // Set this before any I/O: even a panic must not leave a usable stale
        // accounting view. Validation errors above do not poison the writer.
        self.poisoned = true;
        if self.file.metadata()?.len() != self.journal.encoded_len()
            || self.file.seek(SeekFrom::End(0))? != self.journal.encoded_len()
        {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "journal extent changed").into());
        }
        persist(&mut self.file, &record)?;
        self.journal = candidate;
        self.poisoned = false;
        Ok(self.journal.head())
    }
}

fn parent_directory(path: &Path) -> io::Result<File> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let directory = File::open(parent)?;
    if !directory.metadata()?.is_dir() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "journal parent is not a directory"));
    }
    Ok(directory)
}

fn lock(file: &File) -> Result<(), Error> {
    match file.try_lock() {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(Error::Busy),
        Err(TryLockError::Error(error)) => Err(Error::Io(error)),
    }
}


/// Durable false-stop accounting, separate from the rights journal wire format.
/// It never grants authority or changes a review policy. The same trusted-local-
/// directory and sync/lock assumptions as `FileJournal` apply. Evaluator facts
/// remain caller-authenticated; chained CRC-32 is NOT an authenticity boundary.
pub mod andon {
    use super::{Error, lock, parent_directory};
    use crate::{Head, crc32};
    use fa_reference::round::review::{CongressReview, ExactStatus};
    use fa_reference::round::review::andon::{
        AndonBudget, AndonLedger, LedgerIdentity, RecordResult, TrustedFalseHold,
        MAX_FALSE_HOLDS, MAX_WINDOWS,
    };
    use fa_reference::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
    use fa_reference::round::MAX_FIELD_LEN;
    use std::collections::BTreeMap;
    use std::fs::{File, OpenOptions};
    use std::io::{self, Read, Seek, SeekFrom, Write};
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::Path;

    const MAGIC: &[u8; 8] = b"FAAL\0\0\0\x01";
    const HEADER_BYTES: u64 = 28;
    const MAX_FRAME_BYTES: usize = 32_768;
    const MAX_RECORDS: u64 = (MAX_WINDOWS + MAX_FALSE_HOLDS) as u64;
    const MAX_FILE_BYTES: u64 = 128 * 1024 * 1024;

    enum Record {
        Window(AndonBudget),
        FalseHold(TrustedFalseHold),
    }

    /// One exclusively locked owner, with sync-before-acknowledgment publication.
    /// Keep each returned head independently of this file. After uncertain I/O,
    /// no ledger is exposed: reopen with the last acknowledged head, replay the
    /// complete suffix, and never interpret a failed call as a budget refund.
    #[derive(Debug)]
    pub struct FileAndonLedger {
        file: File,
        ledger: AndonLedger,
        head: Head,
        encoded_len: u64,
        poisoned: bool,
    }

    impl FileAndonLedger {
        /// Never replace an existing path, even when the existing file is bad.
        pub fn create(path: impl AsRef<Path>, identity: LedgerIdentity) -> Result<Self, Error> {
            let ledger = AndonLedger::new(identity).map_err(crate::Error::Model)?;
            let path = path.as_ref();
            let directory = parent_directory(path)?;
            let mut file = OpenOptions::new().read(true).write(true).create_new(true).mode(0o600).open(path)?;
            lock(&file)?;
            let (header, head) = header(identity);
            file.write_all(&header)?;
            file.sync_all()?;
            directory.sync_all()?;
            Ok(Self { file, ledger, head, encoded_len: HEADER_BYTES, poisoned: false })
        }

        /// Strict bounded replay: no repair/truncation, no skipped records, and
        /// no exported accounting before complete history and anchor validation.
        /// The supplied identity and minimum head must be independently retained.
        pub fn open(path: impl AsRef<Path>, identity: LedgerIdentity, minimum: Head) -> Result<Self, Error> {
            let mut ledger = AndonLedger::new(identity).map_err(crate::Error::Model)?;
            if minimum.sequence > MAX_RECORDS {
                return Err(crate::Error::BehindAnchor.into());
            }
            let mut file = OpenOptions::new().read(true).write(true).open(path)?;
            lock(&file)?;
            let metadata = file.metadata()?;
            let extent = metadata.len();
            if !metadata.is_file() || !(HEADER_BYTES..=MAX_FILE_BYTES).contains(&extent) {
                return Err(crate::Error::Limit.into());
            }
            let mut supplied_header = [0_u8; HEADER_BYTES as usize];
            file.read_exact(&mut supplied_header)?;
            let (expected_header, mut head) = header(identity);
            if supplied_header.as_slice() != expected_header.as_slice() {
                return Err(crate::Error::Binding.into());
            }
            if minimum.sequence == 0 && minimum != head {
                return Err(crate::Error::Binding.into());
            }
            let mut encoded_len = HEADER_BYTES;
            while encoded_len < extent {
                if head.sequence >= MAX_RECORDS || extent - encoded_len < 8 {
                    return Err(corrupt("Andon record count or truncated frame"));
                }
                let mut length = [0; 4];
                file.read_exact(&mut length)?;
                let length = u32::from_le_bytes(length) as usize;
                if !(9..=MAX_FRAME_BYTES).contains(&length) {
                    return Err(corrupt("Andon frame length"));
                }
                let next_len = encoded_len.checked_add(length as u64 + 8)
                    .ok_or(crate::Error::Limit)?;
                if next_len > extent { return Err(corrupt("truncated Andon frame")); }
                let mut body = vec![0; length];
                file.read_exact(&mut body)?;
                let mut supplied_checksum = [0; 4];
                file.read_exact(&mut supplied_checksum)?;
                let sequence = u64::from_le_bytes(body[..8].try_into().map_err(|_| corrupt("Andon sequence"))?);
                let checksum = checksum(head.checksum, &body);
                if sequence != head.sequence + 1 || checksum != u32::from_le_bytes(supplied_checksum) {
                    return Err(corrupt("Andon sequence/checksum mismatch"));
                }
                let record = decode(&body[8..])?;
                if !apply(&mut ledger, &record)? {
                    return Err(corrupt("duplicate Andon record"));
                }
                head = Head { sequence, checksum };
                if head.sequence == minimum.sequence && head != minimum {
                    return Err(crate::Error::Binding.into());
                }
                encoded_len = next_len;
            }
            if head.sequence < minimum.sequence { return Err(crate::Error::BehindAnchor.into()); }
            if file.metadata()?.len() != extent || file.seek(SeekFrom::End(0))? != extent {
                return Err(corrupt("Andon extent changed during recovery"));
            }
            // Includes a complete but previously unacknowledged suffix.
            file.sync_all()?;
            Ok(Self { file, ledger, head, encoded_len, poisoned: false })
        }

        pub fn acknowledged_head(&self) -> Head { self.head }
        pub fn is_poisoned(&self) -> bool { self.poisoned }

        pub fn ledger(&self) -> Result<&AndonLedger, Error> {
            if self.poisoned { return Err(Error::Poisoned); }
            Ok(&self.ledger)
        }

        /// Exact registration retries return the original head without any I/O.
        pub fn register_window(&mut self, budget: AndonBudget) -> Result<Head, Error> {
            let mut candidate = self.ledger()?.clone();
            if !candidate.register_window(budget).map_err(crate::Error::Model)? { return Ok(self.head); }
            self.publish(candidate, &Record::Window(budget))
        }

        /// Persist a false-hold observation bound to a completed empirical review.
        pub fn record_review(&mut self, review: &CongressReview, exact: ExactStatus, fact: &TrustedFalseHold)
            -> Result<(RecordResult, Head), Error>
        {
            let mut candidate = self.ledger()?.clone();
            let result = candidate.record_review(review, exact, fact).map_err(crate::Error::Model)?;
            self.publish_confirmation(candidate, fact, result)
        }

        /// Explicit trusted evaluator entry point; does not authenticate a label.
        pub fn record_trusted(&mut self, fact: &TrustedFalseHold) -> Result<(RecordResult, Head), Error> {
            let mut candidate = self.ledger()?.clone();
            let result = candidate.record_trusted(fact).map_err(crate::Error::Model)?;
            self.publish_confirmation(candidate, fact, result)
        }

        fn publish_confirmation(&mut self, candidate: AndonLedger, fact: &TrustedFalseHold, result: RecordResult)
            -> Result<(RecordResult, Head), Error>
        {
            let head = if matches!(result, RecordResult::AlreadyRecorded { .. }) {
                self.head
            } else {
                self.publish(candidate, &Record::FalseHold(fact.clone()))?
            };
            Ok((result, head))
        }

        fn publish(&mut self, candidate: AndonLedger, record: &Record) -> Result<Head, Error> {
            self.publish_with(candidate, record, |file, bytes| {
                file.write_all(bytes)?;
                file.sync_all()
            })
        }

        fn publish_with(&mut self, candidate: AndonLedger, record: &Record,
            persist: impl FnOnce(&mut File, &[u8]) -> io::Result<()>) -> Result<Head, Error>
        {
            if self.poisoned { return Err(Error::Poisoned); }
            let (bytes, head) = frame(self.head, record)?;
            let next_len = self.encoded_len.checked_add(bytes.len() as u64).ok_or(crate::Error::Limit)?;
            if next_len > MAX_FILE_BYTES { return Err(crate::Error::Limit.into()); }
            // Validation/allocation precede poisoning. All I/O, including extent
            // checks and unwinding in the persistence hook, follows poisoning.
            self.poisoned = true;
            if self.file.metadata()?.len() != self.encoded_len
                || self.file.seek(SeekFrom::End(0))? != self.encoded_len
            {
                return Err(corrupt("Andon extent changed before append"));
            }
            persist(&mut self.file, &bytes)?;
            self.ledger = candidate;
            self.head = head;
            self.encoded_len = next_len;
            self.poisoned = false;
            Ok(head)
        }
    }

    fn corrupt(message: &'static str) -> Error { crate::Error::Corrupt(message).into() }

    fn header(identity: LedgerIdentity) -> (Vec<u8>, Head) {
        let mut bytes = MAGIC.to_vec();
        bytes.extend_from_slice(&identity.deployment.to_le_bytes());
        bytes.extend_from_slice(&identity.evaluator.to_le_bytes());
        let checksum = crc32(&bytes);
        bytes.extend_from_slice(&checksum.to_le_bytes());
        (bytes, Head { sequence: 0, checksum })
    }

    fn checksum(previous: u32, body: &[u8]) -> u32 {
        let mut bytes = previous.to_le_bytes().to_vec();
        bytes.extend_from_slice(body);
        crc32(&bytes)
    }

    fn apply(ledger: &mut AndonLedger, record: &Record) -> Result<bool, crate::Error> {
        Ok(match record {
            Record::Window(budget) => ledger.register_window(*budget)?,
            Record::FalseHold(fact) => matches!(ledger.record_trusted(fact)?, RecordResult::Recorded { .. }),
        })
    }

    fn blob(out: &mut Vec<u8>, bytes: &[u8]) -> Result<(), crate::Error> {
        let length = u32::try_from(bytes.len()).map_err(|_| crate::Error::Limit)?;
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(bytes);
        Ok(())
    }

    fn frame(head: Head, record: &Record) -> Result<(Vec<u8>, Head), crate::Error> {
        let sequence = head.sequence.checked_add(1).ok_or(crate::Error::Limit)?;
        if sequence > MAX_RECORDS { return Err(crate::Error::Limit); }
        let mut body = sequence.to_le_bytes().to_vec();
        match record {
            Record::Window(budget) => {
                body.push(1);
                for value in [budget.window, budget.start, budget.end, budget.false_stops_allowed, budget.cost_per_stop] {
                    body.extend_from_slice(&value.to_le_bytes());
                }
            }
            Record::FalseHold(fact) => {
                body.push(2);
                for value in [fact.identity.deployment, fact.identity.evaluator, fact.report, fact.round,
                    fact.occurred_at, fact.confirmed_at] {
                    body.extend_from_slice(&value.to_le_bytes());
                }
                blob(&mut body, &fact.evidence_root)?;
                let count = u32::try_from(fact.helper_generations.len()).map_err(|_| crate::Error::Limit)?;
                body.extend_from_slice(&count.to_le_bytes());
                for (member, generation) in &fact.helper_generations {
                    blob(&mut body, member.as_bytes())?;
                    body.extend_from_slice(&generation.to_le_bytes());
                }
            }
        }
        if body.len() > MAX_FRAME_BYTES { return Err(crate::Error::Limit); }
        let checksum = checksum(head.checksum, &body);
        let mut bytes = (body.len() as u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(&body);
        bytes.extend_from_slice(&checksum.to_le_bytes());
        Ok((bytes, Head { sequence, checksum }))
    }

    struct Decoder<'a> { bytes: &'a [u8], offset: usize }
    impl<'a> Decoder<'a> {
        fn take(&mut self, count: usize) -> Result<&'a [u8], crate::Error> {
            let end = self.offset.checked_add(count).ok_or(crate::Error::Limit)?;
            let value = self.bytes.get(self.offset..end).ok_or(crate::Error::Corrupt("truncated Andon value"))?;
            self.offset = end;
            Ok(value)
        }
        fn u64(&mut self) -> Result<u64, crate::Error> {
            Ok(u64::from_le_bytes(self.take(8)?.try_into().map_err(|_| crate::Error::Corrupt("Andon u64"))?))
        }
        fn u32(&mut self) -> Result<u32, crate::Error> {
            Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(|_| crate::Error::Corrupt("Andon u32"))?))
        }
        fn blob(&mut self, maximum: usize) -> Result<&'a [u8], crate::Error> {
            let count = self.u32()? as usize;
            if count > maximum { return Err(crate::Error::Limit); }
            self.take(count)
        }
    }

    fn decode(bytes: &[u8]) -> Result<Record, crate::Error> {
        let mut d = Decoder { bytes, offset: 0 };
        let record = match d.take(1)?[0] {
            1 => Record::Window(AndonBudget { window: d.u64()?, start: d.u64()?, end: d.u64()?,
                false_stops_allowed: d.u64()?, cost_per_stop: d.u64()? }),
            2 => {
                let identity = LedgerIdentity { deployment: d.u64()?, evaluator: d.u64()? };
                let report = d.u64()?;
                let round = d.u64()?;
                let occurred_at = d.u64()?;
                let confirmed_at = d.u64()?;
                let evidence_root = d.blob(MAX_FIELD_LEN)?.to_vec();
                let count = d.u32()? as usize;
                if count > MAX_VOTES { return Err(crate::Error::Limit); }
                let mut helper_generations = BTreeMap::new();
                let mut previous: Option<String> = None;
                for _ in 0..count {
                    let member = std::str::from_utf8(d.blob(MAX_IDENTIFIER_BYTES)?)
                        .map_err(|_| crate::Error::Corrupt("Andon helper UTF-8"))?.to_owned();
                    if previous.as_ref().is_some_and(|p| p >= &member) {
                        return Err(crate::Error::Corrupt("noncanonical Andon helpers"));
                    }
                    previous = Some(member.clone());
                    helper_generations.insert(member, d.u64()?);
                }
                Record::FalseHold(TrustedFalseHold { identity, report, round, occurred_at, confirmed_at,
                    evidence_root, helper_generations })
            }
            _ => return Err(crate::Error::Corrupt("unknown Andon record")),
        };
        if d.offset != bytes.len() { return Err(crate::Error::Corrupt("trailing Andon record bytes")); }
        Ok(record)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU64, Ordering};

        struct Scratch(PathBuf);
        impl Scratch {
            fn new() -> Self {
                static NEXT: AtomicU64 = AtomicU64::new(0);
                loop {
                    let path = std::env::temp_dir().join(format!("fa-andon-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
                    match std::fs::create_dir(&path) {
                        Ok(()) => return Self(path),
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                        Err(error) => panic!("create test directory: {error}"),
                    }
                }
            }
            fn path(&self) -> PathBuf { self.0.join("ledger") }
        }
        impl Drop for Scratch {
            fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
        }
        fn identity() -> LedgerIdentity { LedgerIdentity { deployment: 1, evaluator: 2 } }
        fn budget() -> AndonBudget {
            AndonBudget { window: 7, start: 10, end: 20, false_stops_allowed: 2, cost_per_stop: 9 }
        }
        fn fact(round: u64) -> TrustedFalseHold {
            TrustedFalseHold { identity: identity(), report: 100 + round, round, occurred_at: 12, confirmed_at: 30,
                evidence_root: b"evidence".to_vec(), helper_generations: BTreeMap::from([("helper".into(), round)]) }
        }
        fn opened(path: &Path) -> FileAndonLedger {
            let mut owner = FileAndonLedger::create(path, identity()).unwrap();
            owner.register_window(budget()).unwrap();
            owner
        }

        #[test]
        fn restart_preserves_counts_generations_incident_and_retry_head() {
            let scratch = Scratch::new();
            let path = scratch.path();
            let mut owner = opened(&path);
            let (_, first) = owner.record_trusted(&fact(1)).unwrap();
            drop(owner);
            let mut owner = FileAndonLedger::open(&path, identity(), first).unwrap();
            assert_eq!(owner.ledger().unwrap().windows()[&7].false_stops, 1);
            let before = std::fs::read(&path).unwrap();
            assert_eq!(owner.record_trusted(&fact(1)).unwrap(), (RecordResult::AlreadyRecorded { window: 7 }, first));
            assert_eq!(std::fs::read(&path).unwrap(), before);
            let (result, second) = owner.record_trusted(&fact(2)).unwrap();
            assert_eq!(result, RecordResult::Recorded { window: 7, incident_opened: true });
            drop(owner);
            let mut owner = FileAndonLedger::open(&path, identity(), second).unwrap();
            owner.record_trusted(&fact(3)).unwrap();
            let ledger = owner.ledger().unwrap();
            let incident = ledger.windows()[&7].incident.as_ref().unwrap();
            assert_eq!(incident.opened_by_round, 2);
            assert_eq!(incident.rounds.len(), 3);
            assert_eq!(ledger.confirmation(1).unwrap().helper_generations["helper"], 1);
            assert_eq!(ledger.confirmation(3).unwrap().helper_generations["helper"], 3);
        }

        #[test]
        fn uncertain_complete_write_poisoned_until_full_suffix_recovery() {
            let scratch = Scratch::new();
            let path = scratch.path();
            let mut owner = opened(&path);
            owner.record_trusted(&fact(1)).unwrap();
            let anchor = owner.acknowledged_head();
            let mut candidate = owner.ledger().unwrap().clone();
            candidate.record_trusted(&fact(2)).unwrap();
            assert!(owner.publish_with(candidate, &Record::FalseHold(fact(2)), |file, bytes| {
                file.write_all(bytes)?;
                Err(io::Error::other("injected sync failure"))
            }).is_err());
            assert!(owner.is_poisoned());
            assert!(matches!(owner.ledger(), Err(Error::Poisoned)));
            assert_eq!(owner.acknowledged_head(), anchor);
            assert!(matches!(owner.record_trusted(&fact(2)), Err(Error::Poisoned)));
            drop(owner);
            let owner = FileAndonLedger::open(&path, identity(), anchor).unwrap();
            assert_eq!(owner.ledger().unwrap().windows()[&7].false_stops, 2);
            assert!(owner.ledger().unwrap().windows()[&7].incident.is_some());
        }

        #[test]
        fn partial_write_is_never_silently_repaired_or_refunded() {
            let scratch = Scratch::new();
            let path = scratch.path();
            let mut owner = opened(&path);
            let anchor = owner.acknowledged_head();
            let mut candidate = owner.ledger().unwrap().clone();
            candidate.record_trusted(&fact(1)).unwrap();
            assert!(owner.publish_with(candidate, &Record::FalseHold(fact(1)), |file, bytes| {
                file.write_all(&bytes[..bytes.len() - 1])?;
                Err(io::Error::other("injected partial write"))
            }).is_err());
            drop(owner);
            let before = std::fs::read(&path).unwrap();
            assert!(FileAndonLedger::open(&path, identity(), anchor).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }

        #[test]
        fn panic_during_persistence_poisoned_without_publishing_candidate() {
            let scratch = Scratch::new();
            let path = scratch.path();
            let mut owner = opened(&path);
            let anchor = owner.acknowledged_head();
            let mut candidate = owner.ledger().unwrap().clone();
            candidate.record_trusted(&fact(1)).unwrap();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = owner.publish_with(candidate, &Record::FalseHold(fact(1)), |_, _| panic!("injected unwind"));
            }));
            assert!(panic.is_err());
            assert!(owner.is_poisoned());
            assert_eq!(owner.acknowledged_head(), anchor);
            assert!(matches!(owner.ledger(), Err(Error::Poisoned)));
        }

        #[test]
        fn rollback_wrong_identity_and_wrong_anchor_refuse_without_repair() {
            let scratch = Scratch::new();
            let path = scratch.path();
            let mut owner = opened(&path);
            let prefix = std::fs::read(&path).unwrap();
            let (_, anchor) = owner.record_trusted(&fact(1)).unwrap();
            drop(owner);
            let before = std::fs::read(&path).unwrap();
            assert!(FileAndonLedger::open(&path, LedgerIdentity { deployment: 9, ..identity() }, anchor).is_err());
            assert!(FileAndonLedger::open(&path, identity(), Head { checksum: anchor.checksum ^ 1, ..anchor }).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
            std::fs::write(&path, &prefix).unwrap();
            assert!(matches!(FileAndonLedger::open(&path, identity(), anchor), Err(Error::Journal(crate::Error::BehindAnchor))));
            assert_eq!(std::fs::read(&path).unwrap(), prefix);
        }

        #[test]
        fn lock_conflict_create_collision_and_validation_preserve_bytes() {
            let scratch = Scratch::new();
            let path = scratch.path();
            let mut owner = opened(&path);
            let anchor = owner.acknowledged_head();
            let before = std::fs::read(&path).unwrap();
            assert!(matches!(FileAndonLedger::open(&path, identity(), anchor), Err(Error::Busy)));
            assert!(FileAndonLedger::create(&path, identity()).is_err());
            let mut invalid = fact(1);
            invalid.report = 0;
            assert!(owner.record_trusted(&invalid).is_err());
            assert!(!owner.is_poisoned());
            assert_eq!(owner.acknowledged_head(), anchor);
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }

        #[test]
        fn checksum_damage_oversize_and_duplicate_records_are_rejected() {
            for case in 0..3 {
                let scratch = Scratch::new();
                let path = scratch.path();
                let owner = opened(&path);
                let anchor = owner.acknowledged_head();
                drop(owner);
                let mut bytes = std::fs::read(&path).unwrap();
                match case {
                    0 => { let last = bytes.len() - 1; bytes[last] ^= 1; }
                    1 => { bytes.extend_from_slice(&u32::MAX.to_le_bytes()); bytes.extend_from_slice(&[0; 4]); }
                    _ => bytes.extend_from_slice(&frame(anchor, &Record::Window(budget())).unwrap().0),
                }
                std::fs::write(&path, &bytes).unwrap();
                assert!(FileAndonLedger::open(&path, identity(), anchor).is_err());
                assert_eq!(std::fs::read(&path).unwrap(), bytes);
            }
        }

        #[test]
        fn codec_round_trip_preserves_exact_evidence_and_rejects_trailing_bytes() {
            let mut original = fact(1);
            original.evidence_root = vec![0, 255, 0, 128];
            original.helper_generations.insert("peer".into(), 5);
            let (bytes, _) = frame(header(identity()).1, &Record::FalseHold(original.clone())).unwrap();
            let payload = &bytes[12..bytes.len() - 4];
            let decoded = decode(payload).unwrap();
            let mut ledger = AndonLedger::new(identity()).unwrap();
            ledger.register_window(budget()).unwrap();
            assert!(apply(&mut ledger, &decoded).unwrap());
            assert_eq!(ledger.confirmation(1), Some(&original));
            let mut trailing = payload.to_vec();
            trailing.push(0);
            assert!(decode(&trailing).is_err());
            trailing[0] = 255;
            assert!(decode(&trailing).is_err());
        }

        #[test]
        fn completed_review_is_persisted_but_unknown_evidence_is_not() {
            use fa_reference::reducer::Caps;
            use fa_reference::round::{Verdict, commitment};
            use fa_reference::round::review::{Member, Requirements, ReviewPolicy};
            let scratch = Scratch::new();
            let path = scratch.path();
            let mut owner = opened(&path);
            let policy = ReviewPolicy::new(vec![Member { id: "helper".into(), cohort: "c".into(), weight: 1 }],
                Requirements { caps: Caps { per_member: 1, per_cohort: 1 }, min_permit_weight: 1,
                    max_hold_weight: 0, min_permit_cohorts: 1 }).unwrap();
            let mut review = CongressReview::new(policy, 1, b"evidence").unwrap();
            review.commit("helper", commitment(1, "helper", b"evidence", Verdict::Hold, b"salt").unwrap()).unwrap();
            review.open_reveals().unwrap();
            review.reveal("helper", Verdict::Hold, b"salt").unwrap();
            let before = std::fs::read(&path).unwrap();
            assert!(owner.record_review(&review, ExactStatus::Unknown, &fact(1)).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), before);
            assert!(!owner.is_poisoned());
            let (_, head) = owner.record_review(&review, ExactStatus::Clear, &fact(1)).unwrap();
            drop(owner);
            let owner = FileAndonLedger::open(&path, identity(), head).unwrap();
            assert_eq!(owner.ledger().unwrap().confirmation(1), Some(&fact(1)));
        }

        #[test]
        fn changed_extent_poisoned_before_any_append() {
            let scratch = Scratch::new();
            let path = scratch.path();
            let mut owner = opened(&path);
            let anchor = owner.acknowledged_head();
            OpenOptions::new().append(true).open(&path).unwrap().write_all(b"unexpected").unwrap();
            let before = std::fs::read(&path).unwrap();
            assert!(owner.record_trusted(&fact(1)).is_err());
            assert!(owner.is_poisoned());
            assert_eq!(owner.acknowledged_head(), anchor);
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod batch_tests;
