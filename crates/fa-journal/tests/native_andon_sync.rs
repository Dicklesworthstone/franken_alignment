#![cfg(unix)]
#[path = "../../fa-reference/tests/support/native_andon.rs"]
mod native;
use native::*;
use fa_journal::evaluated_andon::{EvaluatedAndonImport, ImportStatus};
use fa_journal::evaluated_andon::native_sync::NativeAndonSync;
use fa_journal::file::andon::FileAndonLedger;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::oversight::OversightBroker;
use fa_reference::action::consequence::oversight::credibility::{GroundTruth,
    native_andon::NativeAndonBinding};
use fa_reference::round::review::andon::AndonBudget;
use fa_reference::round::Verdict;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("fa-native-sync-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("ledger") }
    fn journal(&self) -> FileAndonLedger {
        FileAndonLedger::create(self.path(), identity()).unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("native sync cleanup: {error}"); }
    }
}
fn source() -> (OversightBroker, NativeAndonBinding) {
    let (mut host, evaluator, endpoint) = setup(0);
    let binding = host.enable_andon(accounting()).unwrap();
    apply(&mut host, 1, 10, [Verdict::Hold, Verdict::Allow]);
    apply(&mut host, 2, 12, [Verdict::Allow, Verdict::Hold]);
    host.observe_time(ElapsedTick(13)).unwrap();
    // Reverse ID order, SAME clock tick. Only actual label acceptance orders these.
    for id in [2, 1] {
        host.record_evaluation(label(&host, &evaluator, id, GroundTruth::Benign)).unwrap();
    }
    assert_eq!(endpoint.execution_count(), 0);
    (host, binding)
}

#[test]
fn whole_source_preserves_incident_order_even_when_ids_and_ticks_cannot() {
    let (host, binding) = source();
    let view = host.andon_source().unwrap();
    assert_eq!(view.receipts().iter().map(|r| r.case().round).collect::<Vec<_>>(), vec![102, 101]);
    assert!(view.receipts()[0].accepted_revision() < view.receipts()[1].accepted_revision());
    assert_eq!(view.receipts()[0].confirmation().confirmed_at, view.receipts()[1].confirmation().confirmed_at);
    let before = host.inspect();
    let temp = Temp::new(); let mut file = temp.journal();
    let mut sync = NativeAndonSync::begin(&mut file, &binding, &host).unwrap();
    assert_eq!((sync.window_count(), sync.receipt_count()), (1, 2));
    assert_eq!(sync.source_revision(), host.credibility_report().unwrap().revision);
    let complete = sync.advance(usize::MAX).unwrap();
    assert_eq!((complete.completed, complete.newly_recorded, complete.status), (3, 3, ImportStatus::Complete));
    sync.finish().unwrap();
    assert_eq!(file.ledger().unwrap(), host.andon_ledger().unwrap());
    assert_eq!(file.ledger().unwrap().windows()[&1].incident.as_ref().unwrap().opened_by_round, 102);
    assert_eq!(host.inspect(), before);
}

#[test]
fn equal_counts_with_different_incident_provenance_refuse_before_any_write() {
    let (host, binding) = source();
    let temp = Temp::new(); let mut file = temp.journal(); file.register_window(budget()).unwrap();
    // Original subset importer is not a full-native-equality claim. Reversed
    // acceptance order yields the same totals but different incident provenance.
    let receipts = [host.andon_receipt(101).unwrap().clone(), host.andon_receipt(102).unwrap().clone()];
    let mut subset = EvaluatedAndonImport::begin_native(&mut file, &binding, &receipts).unwrap();
    subset.advance(2).unwrap(); subset.finish().unwrap();
    assert_eq!(file.ledger().unwrap().windows()[&1].false_stops,
        host.andon_ledger().unwrap().windows()[&1].false_stops);
    assert_ne!(file.ledger().unwrap(), host.andon_ledger().unwrap());
    let bytes = fs::read(temp.path()).unwrap(); let head = file.acknowledged_head();
    assert!(NativeAndonSync::begin(&mut file, &binding, &host).is_err());
    assert_eq!(fs::read(temp.path()).unwrap(), bytes); assert_eq!(file.acknowledged_head(), head);
    assert!(!file.is_poisoned());
}

#[test]
fn bounded_sync_reopens_and_completes_all_windows_and_receipts_without_duplicates() {
    let (host, binding) = source();
    let temp = Temp::new(); let mut file = temp.journal();
    let bytes = fs::read(temp.path()).unwrap();
    let mut sync = NativeAndonSync::begin(&mut file, &binding, &host).unwrap();
    assert_eq!(sync.advance(0).unwrap().completed, 0);
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    let prefix = sync.advance(2).unwrap();
    assert_eq!((prefix.completed, prefix.status), (2, ImportStatus::Pending));
    assert!(sync.finish().is_err()); drop(file);
    let mut file = FileAndonLedger::open(temp.path(), identity(), prefix.head).unwrap();
    let mut sync = NativeAndonSync::begin(&mut file, &binding, &host).unwrap();
    let done = sync.advance(usize::MAX).unwrap();
    assert_eq!((done.newly_recorded, done.already_recorded), (1, 2)); sync.finish().unwrap();
    assert_eq!(file.ledger().unwrap(), host.andon_ledger().unwrap());
    let bytes = fs::read(temp.path()).unwrap();
    let mut retry = NativeAndonSync::begin(&mut file, &binding, &host).unwrap();
    let repeated = retry.advance(usize::MAX).unwrap();
    assert_eq!(repeated, retry.advance(usize::MAX).unwrap()); retry.finish().unwrap();
    assert_eq!((repeated.newly_recorded, repeated.already_recorded, repeated.head), (0, 3, done.head));
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
}

#[test]
fn empty_receipt_source_still_synchronizes_all_windows_without_label_fabrication() {
    let (mut host, _, _) = setup(0);
    let binding = host.enable_andon(accounting()).unwrap();
    host.register_andon_window(AndonBudget { window: 2, start: 20, end: 40, ..budget() }).unwrap();
    apply(&mut host, 1, 10, [Verdict::Hold, Verdict::Allow]); // no evaluator label
    let before = host.credibility_report().unwrap();
    let temp = Temp::new(); let mut file = temp.journal();
    let mut sync = NativeAndonSync::begin(&mut file, &binding, &host).unwrap();
    assert_eq!((sync.window_count(), sync.receipt_count()), (2, 0));
    assert_eq!(sync.advance(2).unwrap().newly_recorded, 2); sync.finish().unwrap();
    assert_eq!(file.ledger().unwrap(), host.andon_ledger().unwrap());
    assert_eq!(host.credibility_report().unwrap(), before);
    assert!(host.evaluation_history(101).unwrap().is_empty());
}

#[test]
fn missing_recovered_seed_cannot_be_replaced_by_only_this_owners_new_receipts() {
    let (old, _) = source();
    let (mut host, evaluator, _) = setup(0);
    let binding = host.enable_andon(old.andon_ledger().unwrap().clone()).unwrap();
    apply(&mut host, 3, 14, [Verdict::Hold, Verdict::Allow]);
    host.record_evaluation(label(&host, &evaluator, 3, GroundTruth::Benign)).unwrap();
    assert_eq!(host.andon_source().unwrap().receipts().len(), 1);
    let temp = Temp::new(); let mut file = temp.journal();
    let bytes = fs::read(temp.path()).unwrap();
    assert!(NativeAndonSync::begin(&mut file, &binding, &host).is_err());
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    // Recover the original seed through its own original receipts in source order.
    file.register_window(budget()).unwrap();
    for receipt in old.andon_source().unwrap().receipts() {
        file.record_trusted(receipt.confirmation()).unwrap();
    }
    let mut sync = NativeAndonSync::begin(&mut file, &binding, &host).unwrap();
    assert_eq!(sync.advance(usize::MAX).unwrap().newly_recorded, 1); sync.finish().unwrap();
    assert_eq!(file.ledger().unwrap(), host.andon_ledger().unwrap());
}

#[test]
fn complete_preflight_rejects_foreign_binding_extra_records_and_budget_drift() {
    let (host, binding) = source();
    let (mut foreign, _, _) = setup_with_period(0, 99);
    foreign.enable_andon(accounting()).unwrap();
    for mode in 0..3 {
        let temp = Temp::new(); let mut file = temp.journal();
        match mode {
            0 => {},
            1 => { file.register_window(AndonBudget { cost_per_stop: 99, ..budget() }).unwrap(); },
            _ => {
                file.register_window(budget()).unwrap();
                let mut extra = host.andon_receipt(101).unwrap().confirmation().clone(); extra.round = 999;
                file.record_trusted(&extra).unwrap();
            },
        }
        let bytes = fs::read(temp.path()).unwrap(); let head = file.acknowledged_head();
        let supplied = if mode == 0 { &foreign } else { &host };
        assert!(NativeAndonSync::begin(&mut file, &binding, supplied).is_err());
        assert_eq!(fs::read(temp.path()).unwrap(), bytes); assert_eq!(file.acknowledged_head(), head);
        assert!(!file.is_poisoned());
    }
}

#[test]
fn native_sync_keeps_original_uncertain_suffix_and_partial_tail_recovery_rules() {
    let (host, binding) = source();
    let donor = Temp::new(); let mut donor_file = donor.journal(); donor_file.register_window(budget()).unwrap();
    let start = fs::metadata(donor.path()).unwrap().len() as usize;
    donor_file.record_trusted(host.andon_receipt(102).unwrap().confirmation()).unwrap();
    let suffix = fs::read(donor.path()).unwrap()[start..].to_vec();
    for complete in [true, false] {
        let temp = Temp::new(); let mut file = temp.journal(); file.register_window(budget()).unwrap();
        let head = file.acknowledged_head();
        let mut sync = NativeAndonSync::begin(&mut file, &binding, &host).unwrap();
        sync.advance(1).unwrap(); // existing window, next call attempts first source receipt
        let count = if complete { suffix.len() } else { suffix.len() / 2 };
        let mut writer = OpenOptions::new().append(true).open(temp.path()).unwrap();
        writer.write_all(&suffix[..count]).unwrap(); writer.sync_all().unwrap(); drop(writer);
        let failure = sync.advance(1).unwrap_err();
        assert_eq!((failure.progress.completed, failure.progress.head, failure.progress.status), (1, head, ImportStatus::Failed));
        assert!(sync.advance(1).is_err()); assert!(sync.finish().is_err());
        assert!(file.is_poisoned()); drop(file);
        let bytes = fs::read(temp.path()).unwrap();
        if complete {
            let mut restored = FileAndonLedger::open(temp.path(), identity(), head).unwrap();
            let mut sync = NativeAndonSync::begin(&mut restored, &binding, &host).unwrap();
            let done = sync.advance(usize::MAX).unwrap(); sync.finish().unwrap();
            assert_eq!((done.newly_recorded, done.already_recorded), (1, 2));
            assert_eq!(restored.ledger().unwrap(), host.andon_ledger().unwrap());
        } else {
            assert!(FileAndonLedger::open(temp.path(), identity(), head).is_err());
            assert_eq!(fs::read(temp.path()).unwrap(), bytes);
        }
    }
}
