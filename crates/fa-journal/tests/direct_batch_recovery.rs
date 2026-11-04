#![cfg(unix)]

use fa_journal::batch::BatchLimits;
use fa_journal::file::{
    BatchCommitBudget, BatchCommitStage, BatchInspectionBudget, BatchInspectionStatus,
    Error as FileError, FileBatchInspection, FileJournal,
};
use fa_journal::replay::{HistoryAnchor, ReplayBudget};
use fa_journal::{Error, Event, Identity, Journal, Limits, RecoveryReserve};
use fa_reference::{Effect, State};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Image {
    root: PathBuf,
    history: HistoryAnchor,
    limits: Limits,
    reserve: Option<RecoveryReserve>,
    events: Vec<Event>,
    prefix: Vec<u8>,
    records: Vec<Vec<u8>>,
    final_state: Journal,
}
impl Image {
    fn new(reserved: bool) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "fa-direct-batch-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).unwrap();
        let limits = if reserved { Limits { max_records: 5, ..Limits::default() } }
            else { Limits::default() };
        let reserve = reserved.then_some(RecoveryReserve { records: 3, bytes: 80 });
        let mut journal = match reserve {
            Some(reserve) => Journal::new_with_recovery_reserve(identity(), limits, reserve),
            None => Journal::new(identity(), limits),
        }.unwrap();
        let mut prefix = journal.header();
        prefix.extend(journal.append(&Event::Reserve { id: 1, effect: effect() }).unwrap());
        let history = match reserve {
            Some(reserve) => HistoryAnchor::from_bytes_with_recovery_reserve(prefix.clone(), identity(), limits, reserve),
            None => HistoryAnchor::from_bytes(prefix.clone(), identity(), limits),
        }.unwrap();
        let events = vec![Event::Dispatch { id: 1, effect: effect() },
            Event::MarkUnknown { id: 1 }, Event::RevokeEpoch,
            Event::Reconcile { id: 1, occurred: true }];
        let records = events.iter().map(|event| journal.append(event).unwrap()).collect();
        Self { root, history, limits, reserve, events, prefix, records, final_state: journal }
    }
    fn path(&self) -> PathBuf { self.root.join("rights.bin") }
    fn bytes(&self, stored: usize) -> Vec<u8> {
        let mut bytes = self.prefix.clone();
        for record in &self.records[..stored] { bytes.extend_from_slice(record); }
        bytes
    }
    fn store(&self, stored: usize) { fs::write(self.path(), self.bytes(stored)).unwrap(); }
    fn begin(&self) -> Result<FileBatchInspection, FileError> {
        match self.reserve {
            Some(reserve) => FileJournal::begin_recover_batch_anchored_with_recovery_reserve(
                self.path(), identity(), self.history.clone(), self.limits, reserve,
                self.events.clone(), BatchLimits::default(),
            ),
            None => FileJournal::begin_recover_batch_anchored(
                self.path(), identity(), self.history.clone(), self.limits,
                self.events.clone(), BatchLimits::default(),
            ),
        }
    }
}
impl Drop for Image {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 10 } }
fn effect() -> Effect {
    Effect { principal: "actor".into(), resolved_target: "publication".into(),
        payload: b"proposal".to_vec(), units: 4 }
}
fn budget(quantum: u64) -> BatchInspectionBudget {
    BatchInspectionBudget { replay: ReplayBudget {
        read_calls: 1, read_bytes: quantum, checked_records: 1,
    }, prepare_records: 1, compare_calls: 1, compare_bytes: quantum }
}
fn drive(run: &mut FileBatchInspection, quantum: u64) {
    let minimum = run.acknowledged_head();
    for _ in 0..100_000 {
        if run.progress().status == BatchInspectionStatus::Ready { return; }
        let before = run.progress().total;
        let after = run.advance(budget(quantum)).unwrap().total;
        assert_eq!(run.acknowledged_head(), minimum);
        assert!(after.replay.read_calls - before.replay.read_calls <= 1);
        assert!(after.replay.read_bytes - before.replay.read_bytes <= quantum);
        assert!(after.replay.checked_records - before.replay.checked_records <= 1);
        assert!(after.prepared_records - before.prepared_records <= 1);
        assert!(after.compare_calls - before.compare_calls <= 1);
        assert!(after.compare_bytes - before.compare_bytes <= quantum);
    }
    panic!("no bounded progress");
}
fn refuse(mut run: FileBatchInspection) {
    for _ in 0..100_000 {
        match run.advance(budget(7)) {
            Ok(report) => assert_ne!(report.status, BatchInspectionStatus::Ready),
            Err(_) => {
                assert_eq!(run.progress().status, BatchInspectionStatus::Failed);
                assert!(matches!(run.resume(), Err(FileError::Poisoned)));
                return;
            }
        }
    }
    panic!("invalid history did not refuse");
}

#[test]
fn direct_recovery_reads_each_stored_byte_once_and_resumes_all_original_boundaries() {
    for reserved in [false, true] {
        for stored in 0..=4 {
            for quantum in [1, 7, 4096] {
                let image = Image::new(reserved);
                image.store(stored);
                let mut run = image.begin().unwrap();
                assert_eq!(run.progress().total.replay.read_calls, 0);
                assert_eq!(run.acknowledged_head(), image.history.head());
                drive(&mut run, quantum);
                let work = run.progress().total;
                assert_eq!(work.replay.read_bytes, image.prefix.len() as u64);
                assert_eq!(work.replay.checked_records, 1);
                assert_eq!(work.prepared_records, 4);
                assert_eq!(work.replay.read_bytes + work.compare_bytes, image.bytes(stored).len() as u64);
                assert_eq!(fs::read(image.path()).unwrap(), image.bytes(stored));
                assert!(matches!(image.begin(), Err(FileError::Busy)));
                let mut writer = run.resume().unwrap();
                assert_eq!(writer.progress().preparation_attempts, 0);
                assert_eq!(writer.progress().total_records, 4 - stored);
                assert_eq!(writer.acknowledged_head().sequence, 1 + stored as u64);
                while writer.progress().stage != BatchCommitStage::Ready {
                    let before = writer.progress();
                    let after = writer.advance(BatchCommitBudget {
                        prepared_records: 0, write_calls: 1, write_bytes: quantum,
                    }).unwrap();
                    assert!(after.write_calls - before.write_calls <= 1);
                    assert!(after.written_bytes - before.written_bytes <= quantum);
                }
                let owner = writer.finish().unwrap();
                assert_eq!(owner.rights().unwrap(), image.final_state.rights());
                assert_eq!(owner.acknowledged_head(), image.final_state.head());
                assert_eq!(owner.capacity().unwrap(), image.final_state.capacity());
                assert_eq!(fs::read(image.path()).unwrap(), image.bytes(4));
                if reserved { assert_eq!(owner.capacity().unwrap().recovery.records, 0); }
            }
        }
    }
}

#[test]
fn recovery_can_return_current_unknown_accounting_without_completing_the_intent() {
    let image = Image::new(true);
    image.store(2);
    let mut run = image.begin().unwrap();
    drive(&mut run, 1);
    let (mut owner, status) = run.finish().unwrap();
    assert_eq!(status.persisted_records(), 2);
    assert_eq!(status.remaining_records(), 2);
    assert_eq!(status.remaining_bytes(), image.records[2..].iter().map(Vec::len).sum::<usize>());
    assert_eq!(owner.rights().unwrap().state(1).unwrap(), State::Unknown);
    assert_eq!(owner.rights().unwrap().available(), 6);
    assert_eq!(owner.capacity().unwrap().ordinary.records, 0);
    assert_eq!(owner.capacity().unwrap().recovery.records, 2);
    assert_eq!(fs::read(image.path()).unwrap(), image.bytes(2));
    owner.commit_batch(&image.events[2..], BatchLimits::default()).unwrap();
    assert_eq!(owner.rights().unwrap(), image.final_state.rights());
}

#[test]
fn missing_files_and_context_or_admission_errors_do_not_create_a_journal() {
    let image = Image::new(false);
    let missing = image.root.join("missing").join("rights.bin");
    let different = Identity { policy: [3; 32], ..identity() };
    assert!(matches!(FileJournal::begin_recover_batch_anchored(
        &missing, different, image.history.clone(), image.limits, image.events.clone(), BatchLimits::default(),
    ), Err(FileError::Journal(Error::Binding))));
    assert!(matches!(FileJournal::begin_recover_batch_anchored(
        &missing, identity(), image.history.clone(), image.limits, Vec::new(), BatchLimits::default(),
    ), Err(FileError::Journal(Error::InvalidInput))));
    assert!(matches!(FileJournal::begin_recover_batch_anchored(
        &missing, identity(), image.history.clone(), image.limits, image.events.clone(),
        BatchLimits { max_records: 3, ..BatchLimits::default() },
    ), Err(FileError::Journal(Error::Limit))));
    assert!(matches!(image.begin(), Err(FileError::Io(_))));
    assert!(!image.path().exists());
    assert!(!missing.exists());
    image.store(0);
    let mut permitted = image.begin().unwrap();
    drive(&mut permitted, 7);
    assert_eq!(permitted.finish().unwrap().1.persisted_records(), 0);
}

#[test]
fn every_torn_suffix_and_unrelated_tail_refuses_without_repair() {
    let image = Image::new(false);
    let suffix: Vec<u8> = image.records.iter().flatten().copied().collect();
    let mut boundaries = vec![0];
    let mut end = 0;
    for record in &image.records { end += record.len(); boundaries.push(end); }
    for cut in 1..suffix.len() {
        if boundaries.contains(&cut) { continue; }
        let mut bytes = image.prefix.clone();
        bytes.extend_from_slice(&suffix[..cut]);
        fs::write(image.path(), &bytes).unwrap();
        refuse(image.begin().unwrap());
        assert_eq!(fs::read(image.path()).unwrap(), bytes);
    }
    let mut extra = image.bytes(4);
    let mut state = image.final_state.clone();
    extra.extend(state.append(&Event::RevokeEpoch).unwrap());
    fs::write(image.path(), &extra).unwrap();
    refuse(image.begin().unwrap());
    assert_eq!(fs::read(image.path()).unwrap(), extra);
    for stored in 0..=4 {
        image.store(stored);
        let mut permitted = image.begin().unwrap();
        drive(&mut permitted, 7);
        assert_eq!(permitted.finish().unwrap().1.persisted_records(), stored);
    }
}

#[test]
fn cancellation_and_premature_completion_keep_the_minimum_and_original_file() {
    for stage in [BatchInspectionStatus::ReplayPrefix, BatchInspectionStatus::PrepareBatch,
        BatchInspectionStatus::CompareSuffix, BatchInspectionStatus::Ready] {
        let image = Image::new(false);
        image.store(2);
        let mut run = image.begin().unwrap();
        while run.progress().status != stage { run.advance(budget(1)).unwrap(); }
        let progress = run.progress();
        assert_eq!(run.advance(BatchInspectionBudget::default()).unwrap(), progress);
        assert_eq!(run.acknowledged_head(), image.history.head());
        assert!(matches!(image.begin(), Err(FileError::Busy)));
        assert_eq!(run.cancel(), progress.total);
        assert_eq!(fs::read(image.path()).unwrap(), image.bytes(2));
        let mut recovered = image.begin().unwrap();
        drive(&mut recovered, 7);
        assert_eq!(recovered.finish().unwrap().0.rights().unwrap().state(1).unwrap(), State::Unknown);
    }
    for read_only in [false, true] {
        let image = Image::new(false);
        image.store(2);
        let run = image.begin().unwrap();
        if read_only { assert!(matches!(run.finish(), Err(FileError::RecoveryIncomplete))); }
        else { assert!(matches!(run.resume(), Err(FileError::RecoveryIncomplete))); }
        assert_eq!(fs::read(image.path()).unwrap(), image.bytes(2));
        assert!(image.begin().is_ok());
    }
}

#[test]
fn changed_suffix_same_extent_and_wrong_reserve_never_gain_an_owner() {
    let image = Image::new(false);
    image.store(2);
    let mut bytes = image.bytes(2);
    *bytes.last_mut().unwrap() ^= 1;
    fs::write(image.path(), &bytes).unwrap();
    refuse(image.begin().unwrap());
    assert_eq!(fs::read(image.path()).unwrap(), bytes);
    let image = Image::new(true);
    image.store(2);
    let wrong = RecoveryReserve { records: 2, bytes: 80 };
    assert!(matches!(FileJournal::begin_recover_batch_anchored_with_recovery_reserve(
        image.path(), identity(), image.history.clone(), image.limits, wrong,
        image.events.clone(), BatchLimits::default(),
    ), Err(FileError::Journal(Error::Binding))));
    let mut permitted = image.begin().unwrap();
    drive(&mut permitted, 7);
    assert_eq!(permitted.finish().unwrap().0.rights().unwrap().available(), 6);
}

#[test]
fn direct_recovery_refuses_a_valid_checksum_collision_and_rollback_before_any_group_work() {
    let image = Image::new(false);
    let mut left = Journal::new(identity(), image.limits).unwrap();
    let mut right = Journal::new(identity(), image.limits).unwrap();
    let mut left_bytes = left.header();
    let mut right_bytes = right.header();
    let mut left_effect = effect();
    left_effect.payload = b"plumless".to_vec();
    let mut right_effect = effect();
    right_effect.payload = b"buckeroo".to_vec();
    left_bytes.extend(left.append(&Event::Reserve { id: 1, effect: left_effect }).unwrap());
    right_bytes.extend(right.append(&Event::Reserve { id: 1, effect: right_effect }).unwrap());
    assert_eq!(left.head(), right.head());
    let history = HistoryAnchor::from_bytes(left_bytes.clone(), identity(), image.limits).unwrap();
    fs::write(image.path(), &right_bytes).unwrap();
    let mut run = FileJournal::begin_recover_batch_anchored(image.path(), identity(), history.clone(),
        image.limits, vec![Event::RevokeEpoch], BatchLimits::default()).unwrap();
    assert!(matches!(run.advance(BatchInspectionBudget::UNLIMITED), Err(FileError::Journal(Error::Binding))));
    assert_eq!(run.progress().total.prepared_records, 0);
    assert_eq!(run.progress().total.compare_calls, 0);
    assert_eq!(run.acknowledged_head(), history.head());
    assert!(matches!(run.resume(), Err(FileError::Poisoned)));
    assert_eq!(fs::read(image.path()).unwrap(), right_bytes);
    fs::write(image.path(), left.header()).unwrap();
    assert!(matches!(FileJournal::begin_recover_batch_anchored(image.path(), identity(), history.clone(),
        image.limits, vec![Event::RevokeEpoch], BatchLimits::default()), Err(FileError::Journal(Error::BehindAnchor))));
    fs::write(image.path(), &left_bytes).unwrap();
    let mut permitted = FileJournal::begin_recover_batch_anchored(image.path(), identity(), history,
        image.limits, vec![Event::RevokeEpoch], BatchLimits::default()).unwrap();
    drive(&mut permitted, 1);
    let mut writer = permitted.resume().unwrap();
    writer.advance(BatchCommitBudget::UNLIMITED).unwrap();
    assert_eq!(writer.finish().unwrap().rights().unwrap().epoch(), 1);
}

#[test]
fn a_cancelled_missing_suffix_can_recover_again_only_at_original_record_boundaries() {
    let image = Image::new(true);
    let complete: usize = image.records.iter().map(Vec::len).sum();
    let first = image.records[0].len();
    for cut in [0, first - 1, first, first + image.records[1].len(), complete] {
        image.store(0);
        let mut run = image.begin().unwrap();
        drive(&mut run, 7);
        let mut writer = run.resume().unwrap();
        while writer.progress().written_bytes < cut as u64 {
            let left = cut as u64 - writer.progress().written_bytes;
            writer.advance(BatchCommitBudget { prepared_records: 0, write_calls: 1, write_bytes: left }).unwrap();
        }
        assert_eq!(writer.cancel().written_bytes, cut as u64);
        let bytes = fs::read(image.path()).unwrap();
        if cut == first - 1 {
            refuse(image.begin().unwrap());
            assert_eq!(fs::read(image.path()).unwrap(), bytes);
        } else {
            let mut retry = image.begin().unwrap();
            drive(&mut retry, 7);
            let mut writer = retry.resume().unwrap();
            while writer.progress().stage != BatchCommitStage::Ready {
                writer.advance(BatchCommitBudget::UNLIMITED).unwrap();
            }
            let owner = writer.finish().unwrap();
            assert_eq!(owner.rights().unwrap(), image.final_state.rights());
            assert_eq!(owner.capacity().unwrap().recovery.records, 0);
            assert_eq!(fs::read(image.path()).unwrap(), image.bytes(4));
        }
    }
}
