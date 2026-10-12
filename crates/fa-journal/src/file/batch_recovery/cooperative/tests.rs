//! Actual original locked files, replay/encoder parity, and explicit read faults.
use super::*;
use crate::{Identity, Limits, RecoveryReserve};
use fa_reference::{Effect, State};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-coop-batch-{}-{}-{}",
            std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("rights.wal") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.path()).unwrap() }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("cooperative batch cleanup: {error}"); }
    }
}
fn identity() -> Identity { Identity { run: [1; 32], policy: [2; 32], total_units: 100 } }
fn effect() -> Effect {
    Effect { principal: "alice".into(), resolved_target: "ledger://one".into(), payload: vec![0, 255], units: 20 }
}
fn seed() -> Vec<Event> {
    vec![Event::Reserve { id: 1, effect: effect() }, Event::Reserve { id: 2, effect: effect() }]
}
fn group() -> Vec<Event> {
    vec![Event::Dispatch { id: 1, effect: effect() }, Event::Dispatch { id: 2, effect: effect() },
        Event::MarkUnknown { id: 1 }, Event::RevokeEpoch]
}
fn owner(temp: &Temp, count: usize) -> (FileJournal, BatchAnchor) {
    let mut file = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    file.commit_batch(&seed(), BatchLimits::default()).unwrap();
    let anchor = file.batch_anchor().unwrap();
    if count != 0 { file.commit_batch(&group()[..count], BatchLimits::default()).unwrap(); }
    (file, anchor)
}
fn tiny() -> BatchInspectionBudget {
    BatchInspectionBudget { replay: ReplayBudget { read_calls: 2, read_bytes: 3, checked_records: 1 },
        prepare_records: 1, compare_calls: 2, compare_bytes: 3 }
}
fn drive(run: &mut FileBatchInspection, budget: BatchInspectionBudget) {
    for _ in 0..10_000 {
        let before = run.progress().total;
        let report = run.advance(budget).unwrap();
        assert!(report.total.replay.read_calls - before.replay.read_calls <= budget.replay.read_calls);
        assert!(report.total.replay.read_bytes - before.replay.read_bytes <= budget.replay.read_bytes);
        assert!(report.total.replay.checked_records - before.replay.checked_records <= budget.replay.checked_records);
        assert!(report.total.prepared_records - before.prepared_records <= budget.prepare_records);
        assert!(report.total.compare_calls - before.compare_calls <= budget.compare_calls);
        assert!(report.total.compare_bytes - before.compare_bytes <= budget.compare_bytes);
        if report.status == BatchInspectionStatus::Ready { return; }
    }
    panic!("bounded fixture did not finish");
}
fn compare_stage(temp: &Temp) -> FileBatchInspection {
    let (file, anchor) = owner(temp, 2);
    let mut run = file.begin_inspect_batch(anchor, group(), BatchLimits::default()).unwrap();
    assert_eq!(run.advance(BatchInspectionBudget::UNLIMITED).unwrap().status, BatchInspectionStatus::PrepareBatch);
    assert_eq!(run.advance(BatchInspectionBudget::UNLIMITED).unwrap().status, BatchInspectionStatus::CompareSuffix);
    run
}

#[test]
fn all_surviving_prefixes_match_synchronous_inspection_under_three_poll_schedules() {
    for count in 0..=group().len() {
        for budget in [tiny(), BatchInspectionBudget::UNLIMITED,
            BatchInspectionBudget { replay: ReplayBudget { read_calls: 1, read_bytes: 1, checked_records: 1 },
                prepare_records: 1, compare_calls: 1, compare_bytes: 1 }] {
            let temp = Temp::new(); let (mut file, anchor) = owner(&temp, count);
            let expected = file.inspect_batch(anchor, &group(), BatchLimits::default()).unwrap();
            let rights = file.rights().unwrap().clone(); let bytes = temp.bytes();
            let head = file.acknowledged_head();
            let mut run = file.begin_inspect_batch(anchor, group(), BatchLimits::default()).unwrap();
            assert_eq!(run.progress().total, BatchInspectionWork::default());
            assert_eq!(run.acknowledged_head(), head);
            assert!(matches!(FileJournal::open(temp.path(), identity(), head, Limits::default()), Err(Error::Busy)));
            drive(&mut run, budget);
            let finished = run.progress();
            assert_eq!(finished.total.replay.checked_records, 2);
            assert_eq!(finished.total.prepared_records, group().len() as u64);
            assert_eq!(finished.total.compare_bytes, bytes.len() as u64 - anchor.encoded_len);
            assert_eq!(run.advance(BatchInspectionBudget::default()).unwrap(), finished);
            let (file, status) = run.finish().unwrap();
            assert_eq!(status, expected); assert_eq!(file.rights().unwrap(), &rights);
            assert_eq!(file.acknowledged_head(), head); assert_eq!(temp.bytes(), bytes);
            assert!(matches!(FileJournal::open(temp.path(), identity(), head, Limits::default()), Err(Error::Busy)));
        }
    }
}

#[test]
fn zero_budgets_and_exact_comparison_bytes_cannot_skip_the_final_eof_check() {
    let temp = Temp::new(); let (file, anchor) = owner(&temp, 2);
    let len = temp.bytes().len() as u64 - anchor.encoded_len;
    let mut run = file.begin_inspect_batch(anchor, group(), BatchLimits::default()).unwrap();
    let before = run.progress();
    assert_eq!(run.advance(BatchInspectionBudget::default()).unwrap(), before);
    run.advance(BatchInspectionBudget { replay: ReplayBudget::UNLIMITED, ..BatchInspectionBudget::default() }).unwrap();
    let before = run.progress(); assert_eq!(before.status, BatchInspectionStatus::PrepareBatch);
    assert_eq!(run.advance(BatchInspectionBudget::default()).unwrap(), before);
    run.advance(BatchInspectionBudget { prepare_records: 4, ..BatchInspectionBudget::default() }).unwrap();
    let before = run.progress(); assert_eq!(before.status, BatchInspectionStatus::CompareSuffix);
    assert_eq!(run.advance(BatchInspectionBudget::default()).unwrap(), before);
    let exact = run.advance(BatchInspectionBudget { compare_calls: 1, compare_bytes: len,
        ..BatchInspectionBudget::default() }).unwrap();
    assert_eq!(exact.status, BatchInspectionStatus::CompareSuffix);
    assert_eq!(exact.total.compare_bytes, len);
    let final_read = run.advance(BatchInspectionBudget { compare_calls: 1, compare_bytes: 1,
        ..BatchInspectionBudget::default() }).unwrap();
    assert_eq!(final_read.status, BatchInspectionStatus::Ready);
    assert_eq!(final_read.total.compare_bytes, len); assert_eq!(final_read.total.compare_calls, 2);
    assert_eq!(run.finish().unwrap().1.persisted_records(), 2);
}

#[test]
fn cancellation_and_incomplete_finish_release_the_same_lock_without_a_usable_prefix() {
    for polls in 0..9 {
        for finish in [false, true] {
            let temp = Temp::new(); let (file, anchor) = owner(&temp, 2);
            let bytes = temp.bytes(); let minimum = file.acknowledged_head();
            let mut run = file.begin_inspect_batch(anchor, group(), BatchLimits::default()).unwrap();
            for _ in 0..polls {
                run.advance(BatchInspectionBudget { replay: ReplayBudget::UNLIMITED,
                    prepare_records: 1, compare_calls: 1, compare_bytes: 1 }).unwrap();
            }
            assert_ne!(run.progress().status, BatchInspectionStatus::Ready);
            assert!(matches!(FileJournal::open(temp.path(), identity(), minimum, Limits::default()), Err(Error::Busy)));
            if finish { assert!(matches!(run.finish(), Err(Error::RecoveryIncomplete))); }
            else { let costs = run.progress().total; assert_eq!(run.cancel(), costs); }
            let reopened = FileJournal::open(temp.path(), identity(), minimum, Limits::default()).unwrap();
            assert_eq!(reopened.rights().unwrap().state(1), Ok(State::Dispatched));
            assert_eq!(reopened.rights().unwrap().available(), 60); assert_eq!(temp.bytes(), bytes);
        }
    }
}

#[test]
fn reordered_records_and_late_invalid_events_never_export_private_accounting() {
    for reordered in [false, true] {
        let temp = Temp::new(); let (file, anchor) = owner(&temp, 2);
        let minimum = file.acknowledged_head(); let bytes = temp.bytes(); let mut events = group();
        if reordered { events.swap(0, 1); } else { events.push(Event::Reserve { id: 1, effect: effect() }); }
        let mut run = file.begin_inspect_batch(anchor, events, BatchLimits::default()).unwrap();
        run.advance(BatchInspectionBudget::UNLIMITED).unwrap();
        assert!(run.advance(BatchInspectionBudget::UNLIMITED).is_err());
        assert_eq!(run.progress().status, BatchInspectionStatus::Failed);
        assert_eq!(run.acknowledged_head(), minimum);
        assert!(matches!(run.advance(BatchInspectionBudget::UNLIMITED), Err(Error::Poisoned)));
        assert!(matches!(run.finish(), Err(Error::Poisoned)));
        let mut reopened = FileJournal::open(temp.path(), identity(), minimum, Limits::default()).unwrap();
        assert_eq!(reopened.inspect_batch(anchor, &group(), BatchLimits::default()).unwrap().persisted_records(), 2);
        assert_eq!(temp.bytes(), bytes);
    }
}

#[test]
fn complete_group_budgets_and_misaligned_anchors_remain_mandatory() {
    for mode in 0..4 {
        let temp = Temp::new(); let (file, anchor) = owner(&temp, 1);
        let bytes = temp.bytes(); let minimum = file.acknowledged_head(); let mut expected = anchor;
        let mut limits = BatchLimits::default();
        match mode { 0 => expected.encoded_len += 1, 1 => expected.head.checksum ^= 1,
            2 => limits.max_encoded_bytes = 21, _ => limits.max_records = 3 }
        match file.begin_inspect_batch(expected, group(), limits) {
            Err(_) => assert_eq!(mode, 3),
            Ok(mut run) => {
                let mut refused = false;
                for _ in 0..8 {
                    if run.advance(BatchInspectionBudget::UNLIMITED).is_err() { refused = true; break; }
                }
                assert!(refused); assert_eq!(run.progress().status, BatchInspectionStatus::Failed);
                assert!(run.finish().is_err());
            }
        }
        assert_eq!(temp.bytes(), bytes);
        let file = FileJournal::open(temp.path(), identity(), minimum, Limits::default()).unwrap();
        let mut valid = file.begin_inspect_batch(anchor, group(), BatchLimits::default()).unwrap();
        drive(&mut valid, tiny()); assert_eq!(valid.finish().unwrap().1.persisted_records(), 1);
    }
}

#[test]
fn changed_bytes_and_extent_fail_before_returning_the_acknowledged_owner() {
    for after_ready in [false, true] {
        let temp = Temp::new(); let mut run = compare_stage(&temp);
        if after_ready { drive(&mut run, tiny()); }
        let mut bytes = temp.bytes();
        if after_ready { bytes.push(0); } else { *bytes.last_mut().unwrap() ^= 1; }
        // Deliberate noncooperating corruption, not a supported second writer.
        fs::write(temp.path(), &bytes).unwrap();
        if !after_ready {
            assert!(run.advance(BatchInspectionBudget::UNLIMITED).is_err());
            assert_eq!(run.progress().status, BatchInspectionStatus::Failed);
        }
        assert!(run.finish().is_err()); assert_eq!(temp.bytes(), bytes);
    }
}

#[test]
fn reserved_tail_inspection_conserves_capacity_and_allows_original_settlement_afterward() {
    let seed = vec![Event::Reserve { id: 1, effect: effect() }, Event::Dispatch { id: 1, effect: effect() },
        Event::MarkUnknown { id: 1 }];
    let tail = vec![Event::RevokeEpoch, Event::Reconcile { id: 1, occurred: false }];
    let reserve = RecoveryReserve { records: 2, bytes: 51 };
    let mut limits = Limits { max_records: 5, max_total_bytes: 10_000, ..Limits::default() };
    let mut probe = Journal::new_with_recovery_reserve(identity(), limits, reserve).unwrap();
    probe.append_batch(&seed, BatchLimits::default()).unwrap(); probe.append_batch(&tail, BatchLimits::default()).unwrap();
    limits.max_total_bytes = probe.encoded_len();
    for count in 0..=tail.len() {
        let temp = Temp::new();
        let mut file = FileJournal::create_with_recovery_reserve(temp.path(), identity(), limits, reserve).unwrap();
        file.commit_batch(&seed, BatchLimits::default()).unwrap(); let anchor = file.batch_anchor().unwrap();
        if count > 0 { file.commit_batch(&tail[..count], BatchLimits::default()).unwrap(); }
        let capacity = file.capacity().unwrap(); let bytes = temp.bytes();
        assert_eq!(capacity.ordinary.records, 0); assert_eq!(capacity.ordinary.bytes, 0);
        let expected = file.inspect_batch(anchor, &tail, BatchLimits::default()).unwrap();
        let mut run = file.begin_inspect_batch(anchor, tail.clone(), BatchLimits::default()).unwrap();
        drive(&mut run, tiny()); let (mut file, status) = run.finish().unwrap();
        assert_eq!(status, expected); assert_eq!(file.capacity().unwrap(), capacity);
        assert_eq!(temp.bytes(), bytes);
        if count < tail.len() { file.commit_batch(&tail[count..], BatchLimits::default()).unwrap(); }
        assert_eq!(file.rights().unwrap().available(), 100);
        assert_eq!(file.capacity().unwrap().recovery.bytes, 0); assert_eq!(file.capacity().unwrap().recovery.records, 0);
    }
}

#[test]
fn read_interruptions_yield_and_errors_or_unwinds_latch_failure_with_attempted_work() {
    let temp = Temp::new(); let mut run = compare_stage(&temp);
    let budget = BatchInspectionBudget { compare_calls: 3, compare_bytes: 100, ..BatchInspectionBudget::default() };
    let report = run.advance_with(budget, |_, _| Err(io::ErrorKind::Interrupted.into())).unwrap();
    assert_eq!(report.status, BatchInspectionStatus::CompareSuffix);
    assert_eq!(report.total.compare_calls, 3); assert_eq!(report.total.compare_bytes, 0);
    drive(&mut run, tiny()); run.finish().unwrap();
    for unwind in [false, true] {
        let temp = Temp::new(); let mut run = compare_stage(&temp); let bytes = temp.bytes();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run.advance_with(budget, |file, buffer| {
                if unwind { file.read_exact(buffer)?; panic!("injected read unwind"); }
                Err(io::ErrorKind::PermissionDenied.into())
            })
        }));
        if unwind { assert!(result.is_err()); } else { assert!(result.unwrap().is_err()); }
        assert_eq!(run.progress().status, BatchInspectionStatus::Failed);
        assert_eq!(run.progress().total.compare_calls, 1);
        assert!(matches!(run.finish(), Err(Error::Poisoned))); assert_eq!(temp.bytes(), bytes);
    }
}
