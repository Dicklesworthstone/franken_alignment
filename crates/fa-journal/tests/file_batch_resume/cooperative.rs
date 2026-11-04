//! Transfer verified historical custody into the ORIGINAL bounded file writer.
use super::*;
use fa_journal::file::{FileBatchCommit, FileBatchInspection};

fn ready(mut run: FileBatchInspection, budget: BatchInspectionBudget) -> FileBatchInspection {
    for _ in 0..100_000 {
        if run.progress().status == BatchInspectionStatus::Ready { return run; }
        run.advance(budget).unwrap();
    }
    panic!("small exact inspection did not terminate");
}
fn complete(mut run: FileBatchCommit, quantum: u64) -> FileJournal {
    let old = run.acknowledged_head();
    for _ in 0..100_000 {
        if run.progress().stage == BatchCommitStage::Ready { return run.finish().unwrap(); }
        let before = run.progress();
        let after = run.advance(BatchCommitBudget { prepared_records: 0,
            write_calls: 1, write_bytes: quantum }).unwrap();
        assert!(after.written_bytes > before.written_bytes);
        assert!(after.written_bytes - before.written_bytes <= quantum);
        assert_eq!(after.preparation_attempts, 0);
        assert_eq!(run.acknowledged_head(), old);
    }
    panic!("small exact suffix did not terminate");
}
fn interrupted(temp: &Temp, count: usize) -> (FileJournal, BatchAnchor) {
    let (_, _, ends) = original();
    let file = FileJournal::create(temp.path(), identity(), Limits::default()).unwrap();
    let anchor = cancel_at(file, group(), ends[count], 7);
    (FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()).unwrap(), anchor)
}

#[test]
fn every_surviving_prefix_hands_off_once_without_replay_or_new_preparation() {
    let (expected, wire, ends) = original();
    for quantum in [1, 7, u64::MAX] {
        for count in 0..ends.len() {
            let temp = Temp::new();
            let (owner, anchor) = interrupted(&temp, count);
            let old = owner.acknowledged_head(); let before = temp.bytes();
            let mut inspection = owner.begin_resume_batch(anchor, group(), BatchLimits::default()).unwrap();
            let initial = inspection.progress();
            assert_eq!(inspection.advance(BatchInspectionBudget::default()).unwrap(), initial);
            let inspection = ready(inspection, BatchInspectionBudget { replay: ReplayBudget::UNLIMITED,
                prepare_records: 1, compare_calls: 1, compare_bytes: quantum });
            let work = inspection.progress();
            assert_eq!(work.total.prepared_records, group().len() as u64);
            assert_eq!(work.total.compare_bytes, ends[count] as u64);
            assert_eq!(temp.bytes(), before);
            let mut writer = inspection.resume().unwrap();
            assert_eq!(temp.bytes(), before); // Handoff neither reads again nor writes.
            assert_eq!(writer.progress().total_records, group().len() - count);
            assert_eq!(writer.progress().prepared_records, group().len() - count);
            assert_eq!(writer.progress().preparation_attempts, 0);
            assert_eq!(writer.progress().write_calls, 0);
            assert_eq!(writer.acknowledged_head(), old);
            let initial = writer.progress();
            assert_eq!(writer.advance(BatchCommitBudget::default()).unwrap(), initial);
            assert!(matches!(FileJournal::open(temp.path(), identity(), old, Limits::default()), Err(Error::Busy)));
            let owner = complete(writer, quantum);
            assert_eq!(owner.acknowledged_head(), expected.head());
            assert_eq!(owner.rights().unwrap(), expected.rights()); assert_eq!(temp.bytes(), wire);
            // Repeated exact completion passes through both sealed stages with
            // no missing records and no replenished rights or sequence numbers.
            let retry = ready(owner.begin_resume_batch(anchor, group(), BatchLimits::default()).unwrap(),
                BatchInspectionBudget::UNLIMITED).resume().unwrap();
            assert_eq!(retry.progress().stage, BatchCommitStage::Ready);
            assert_eq!(retry.progress().total_records, 0);
            let owner = retry.finish().unwrap();
            assert_eq!(owner.rights().unwrap().available(), 80);
            assert_eq!(temp.bytes(), wire);
        }
    }
}

#[test]
fn ordinary_inspection_and_incomplete_or_failed_resumption_never_issue_a_writer() {
    for boundary in 0..4 {
        let temp = Temp::new(); let (owner, anchor) = interrupted(&temp, 2);
        let before = temp.bytes();
        let mut inspection = if boundary == 0 {
            owner.begin_inspect_batch(anchor, group(), BatchLimits::default()).unwrap()
        } else { owner.begin_resume_batch(anchor, group(), BatchLimits::default()).unwrap() };
        match boundary {
            0 => { inspection = ready(inspection, BatchInspectionBudget::UNLIMITED); }
            1 => {} // Before reading any prefix.
            2 => { inspection.advance(BatchInspectionBudget::UNLIMITED).unwrap(); }
            _ => {
                inspection.advance(BatchInspectionBudget::UNLIMITED).unwrap();
                inspection.advance(BatchInspectionBudget::UNLIMITED).unwrap();
                assert_eq!(inspection.progress().status, BatchInspectionStatus::CompareSuffix);
            }
        }
        assert!(inspection.resume().is_err()); assert_eq!(temp.bytes(), before);
        let owner = FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()).unwrap();
        let writer = ready(owner.begin_resume_batch(anchor, group(), BatchLimits::default()).unwrap(),
            BatchInspectionBudget::UNLIMITED).resume().unwrap();
        assert_eq!(complete(writer, 7).rights().unwrap(), original().0.rights());
    }
    let temp = Temp::new(); let (owner, anchor) = interrupted(&temp, 2);
    let mut invalid = group(); invalid.push(Event::Reserve { id: 1, effect: effect() });
    let mut inspection = owner.begin_resume_batch(anchor, invalid, BatchLimits::default()).unwrap();
    let before = temp.bytes();
    inspection.advance(BatchInspectionBudget::UNLIMITED).unwrap();
    assert!(inspection.advance(BatchInspectionBudget::UNLIMITED).is_err());
    assert_eq!(inspection.progress().status, BatchInspectionStatus::Failed);
    assert!(matches!(inspection.resume(), Err(Error::Poisoned)));
    assert_eq!(temp.bytes(), before);
}

#[test]
fn cancellation_of_each_stage_and_ready_writer_releases_custody_without_rollback() {
    let (_, wire, ends) = original();
    for polls in 0..=3 {
        let temp = Temp::new(); let (owner, anchor) = interrupted(&temp, 2);
        let before = temp.bytes();
        let mut run = owner.begin_resume_batch(anchor, group(), BatchLimits::default()).unwrap();
        for _ in 0..polls { run.advance(BatchInspectionBudget::UNLIMITED).unwrap(); }
        let work = run.progress().total; assert_eq!(run.cancel(), work);
        assert_eq!(temp.bytes(), before);
        assert!(FileJournal::open(temp.path(), identity(), anchor.head, Limits::default()).is_ok());
    }
    // Each surviving suffix record boundary, plus one torn byte before it.
    let offsets: Vec<_> = ends[3..].iter().flat_map(|end| [end - ends[2] - 1, end - ends[2]])
        .chain([0]).collect();
    for cut in offsets {
        let temp = Temp::new(); let (owner, anchor) = interrupted(&temp, 2);
        let mut writer = ready(owner.begin_resume_batch(anchor, group(), BatchLimits::default()).unwrap(),
            BatchInspectionBudget::UNLIMITED).resume().unwrap();
        while writer.progress().written_bytes < cut as u64 {
            let amount = (cut as u64 - writer.progress().written_bytes).min(7);
            writer.advance(BatchCommitBudget { prepared_records: 0, write_calls: 1, write_bytes: amount }).unwrap();
        }
        let report = writer.cancel(); assert_eq!(report.written_bytes, cut as u64);
        let bytes = temp.bytes();
        assert_eq!(bytes, wire[..anchor.encoded_len as usize + ends[2] + cut]);
        let result = FileJournal::open(temp.path(), identity(), anchor.head, Limits::default());
        if ends.contains(&(ends[2] + cut)) {
            let writer = ready(result.unwrap().begin_resume_batch(anchor, group(), BatchLimits::default()).unwrap(),
                BatchInspectionBudget::UNLIMITED).resume().unwrap();
            assert_eq!(complete(writer, 1).rights().unwrap(), original().0.rights());
        } else { assert!(result.is_err()); assert_eq!(temp.bytes(), bytes); }
    }
}

#[test]
fn whole_group_byte_limit_not_only_missing_bytes_is_enforced_and_conflicts_stay_conflicts() {
    let (_, _, ends) = original();
    for mode in 0..3 {
        let temp = Temp::new(); let (mut owner, anchor) = interrupted(&temp, 3);
        if mode == 2 { owner.commit(&Event::Reconcile { id: 1, occurred: true }).unwrap(); }
        let mut limits = BatchLimits::default();
        if mode == 0 { limits.max_encoded_bytes = *ends.last().unwrap() - 1; }
        if mode == 1 { limits.max_records = group().len() - 1; }
        let bytes = temp.bytes();
        let result = owner.begin_resume_batch(anchor, group(), limits);
        if let Ok(mut run) = result {
            let mut failed = false;
            for _ in 0..10 {
                if run.advance(BatchInspectionBudget::UNLIMITED).is_err() { failed = true; break; }
            }
            assert!(failed); assert!(run.resume().is_err());
        } else { assert_eq!(mode, 1); }
        assert_eq!(temp.bytes(), bytes);
    }
}

#[test]
fn exact_reserved_tail_reuses_its_original_profile_and_complete_retry_has_no_missing_work() {
    let reserve = RecoveryReserve { records: 2, bytes: 51 };
    let seed = &group()[..3];
    let tail = [Event::RevokeEpoch, Event::Reconcile { id: 1, occurred: false }];
    let mut limits = Limits { max_records: 5, max_total_bytes: 10_000, ..Limits::default() };
    let mut probe = Journal::new_with_recovery_reserve(identity(), limits, reserve).unwrap();
    for event in seed.iter().chain(&tail) { probe.append(event).unwrap(); }
    limits.max_total_bytes = probe.encoded_len();
    for count in 0..=2 {
        let temp = Temp::new();
        let mut owner = FileJournal::create_with_recovery_reserve(temp.path(), identity(), limits, reserve).unwrap();
        owner.commit_batch(seed, BatchLimits::default()).unwrap();
        let anchor = owner.batch_anchor().unwrap();
        for event in &tail[..count] { owner.commit(event).unwrap(); }
        drop(owner);
        let owner = FileJournal::open_with_recovery_reserve(temp.path(), identity(), anchor.head, limits, reserve).unwrap();
        let writer = ready(owner.begin_resume_batch(anchor, tail.to_vec(), BatchLimits::default()).unwrap(),
            BatchInspectionBudget::UNLIMITED).resume().unwrap();
        assert_eq!(writer.progress().total_records, 2 - count);
        let owner = complete(writer, 1); assert_eq!(owner.rights().unwrap().available(), 100);
        let capacity = owner.capacity().unwrap(); let bytes = temp.bytes();
        assert_eq!(capacity.ordinary.records, 0); assert_eq!(capacity.ordinary.bytes, 0);
        assert_eq!(capacity.recovery.records, 0); assert_eq!(capacity.recovery.bytes, 0);
        let owner = ready(owner.begin_resume_batch(anchor, tail.to_vec(), BatchLimits::default()).unwrap(),
            BatchInspectionBudget::UNLIMITED).resume().unwrap().finish().unwrap();
        assert_eq!(owner.capacity().unwrap(), capacity); assert_eq!(temp.bytes(), bytes);
    }
}

#[test]
fn changed_extent_between_inspection_handoff_writes_and_finish_returns_no_owner() {
    use std::io::Write;
    for cut in 0..3 {
        let temp = Temp::new(); let (owner, anchor) = interrupted(&temp, 2);
        let run = ready(owner.begin_resume_batch(anchor, group(), BatchLimits::default()).unwrap(),
            BatchInspectionBudget::UNLIMITED);
        if cut == 0 {
            fs::OpenOptions::new().append(true).open(temp.path()).unwrap().write_all(b"x").unwrap();
            let bytes = temp.bytes(); assert!(run.resume().is_err()); assert_eq!(temp.bytes(), bytes);
            continue;
        }
        let mut writer = run.resume().unwrap();
        if cut == 2 { writer.advance(BatchCommitBudget::UNLIMITED).unwrap(); }
        fs::OpenOptions::new().append(true).open(temp.path()).unwrap().write_all(b"x").unwrap();
        let bytes = temp.bytes();
        if cut == 1 { assert!(writer.advance(BatchCommitBudget::UNLIMITED).is_err()); }
        assert!(writer.finish().is_err()); assert_eq!(temp.bytes(), bytes);
    }
}
