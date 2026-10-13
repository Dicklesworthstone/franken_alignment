#![cfg(unix)]
#[path = "../../fa-reference/tests/support/native_andon.rs"]
mod native;
use native::*;
use fa_journal::evaluated_andon::{EvaluatedAndonImport, ImportStatus};
use fa_journal::file::andon::FileAndonLedger;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::congress::credibility::MAX_CASES;
use fa_reference::action::consequence::oversight::credibility::{GroundTruth,
    native_andon::{NativeAndonBinding, NativeEvaluatedFalseHold}};
use fa_reference::round::review::andon::{AndonBudget, AndonLedger, LedgerIdentity};
use fa_reference::round::Verdict;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("fa-native-andon-{}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn path(&self) -> PathBuf { self.0.join("ledger") }
    fn journal(&self) -> FileAndonLedger {
        let mut journal = FileAndonLedger::create(self.path(), identity()).unwrap();
        journal.register_window(budget()).unwrap(); journal
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("native Andon test cleanup: {error}");
        }
    }
}
fn receipts() -> (NativeAndonBinding, Vec<NativeEvaluatedFalseHold>, AndonLedger) {
    let (mut host, evaluator, endpoint) = setup(0);
    let binding = host.enable_andon(accounting()).unwrap();
    for (id, tick, verdicts) in [(1, 10, [Verdict::Hold, Verdict::Allow]),
        (2, 12, [Verdict::Allow, Verdict::Hold])]
    {
        apply(&mut host, id, tick, verdicts);
        host.observe_time(ElapsedTick(tick + 1)).unwrap();
        host.record_evaluation(label(&host, &evaluator, id, GroundTruth::Benign)).unwrap();
    }
    assert_eq!(endpoint.execution_count(), 0);
    (binding, vec![host.andon_receipt(101).unwrap().clone(), host.andon_receipt(102).unwrap().clone()],
        host.andon_ledger().unwrap().clone())
}

#[test]
fn native_labels_flow_through_original_file_and_anchored_interrupted_resume() {
    let (binding, receipts, expected) = receipts();
    let temp = Temp::new(); let mut journal = temp.journal();
    let bytes = fs::read(temp.path()).unwrap();
    let mut cursor = EvaluatedAndonImport::begin_native(&mut journal, &binding, &receipts).unwrap();
    assert_eq!(cursor.advance(0).unwrap().completed, 0);
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    let first = cursor.advance(1).unwrap();
    assert_eq!((first.newly_recorded, first.status), (1, ImportStatus::Pending));
    assert!(cursor.finish().is_err()); drop(journal);
    let mut journal = FileAndonLedger::open(temp.path(), identity(), first.head).unwrap();
    let mut cursor = EvaluatedAndonImport::begin_native(&mut journal, &binding, &receipts).unwrap();
    let done = cursor.advance(usize::MAX).unwrap(); cursor.finish().unwrap();
    assert_eq!((done.newly_recorded, done.already_recorded, done.status), (1, 1, ImportStatus::Complete));
    assert_eq!(journal.ledger().unwrap(), &expected);
    let bytes = fs::read(temp.path()).unwrap();
    let mut retry = EvaluatedAndonImport::begin_native(&mut journal, &binding, &receipts).unwrap();
    let repeated = retry.advance(usize::MAX).unwrap(); retry.finish().unwrap();
    assert_eq!((repeated.newly_recorded, repeated.already_recorded, repeated.head), (0, 2, done.head));
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
}

#[test]
fn foreign_final_native_binding_refuses_before_first_append_even_with_same_identity() {
    let (binding, mut receipts, _) = receipts();
    let (mut foreign, evaluator, _) = setup_with_period(0, 4);
    let foreign_binding = foreign.enable_andon(accounting()).unwrap();
    assert_eq!(foreign_binding.identity(), binding.identity());
    assert_ne!(foreign_binding.protocol(), binding.protocol());
    apply(&mut foreign, 2, 12, [Verdict::Allow, Verdict::Hold]);
    foreign.record_evaluation(label(&foreign, &evaluator, 2, GroundTruth::Benign)).unwrap();
    receipts[1] = foreign.andon_receipt(102).unwrap().clone();
    let temp = Temp::new(); let mut journal = temp.journal();
    let bytes = fs::read(temp.path()).unwrap(); let head = journal.acknowledged_head();
    assert!(EvaluatedAndonImport::begin_native(&mut journal, &binding, &receipts).is_err());
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    assert_eq!(journal.acknowledged_head(), head);
    assert!(!journal.is_poisoned());
    assert!(journal.ledger().unwrap().confirmation(101).is_none());
    // The same genuine receipt succeeds under its own independently retained binding.
    let mut matching = EvaluatedAndonImport::begin_native(&mut journal, &foreign_binding, &receipts[1..]).unwrap();
    matching.advance(1).unwrap(); matching.finish().unwrap();
}

#[test]
fn native_occurrence_budget_cannot_be_repriced_or_retimed_by_the_file_adapter() {
    let (binding, receipts, _) = receipts();
    for changed in [
        AndonBudget { cost_per_stop: 99, ..budget() },
        AndonBudget { false_stops_allowed: 100, ..budget() },
        AndonBudget { end: 21, ..budget() },
    ] {
        let temp = Temp::new();
        let mut journal = FileAndonLedger::create(temp.path(), identity()).unwrap();
        journal.register_window(changed).unwrap();
        let bytes = fs::read(temp.path()).unwrap();
        assert!(EvaluatedAndonImport::begin_native(&mut journal, &binding, &receipts).is_err());
        assert_eq!(fs::read(temp.path()).unwrap(), bytes);
        assert_eq!(journal.ledger().unwrap().windows()[&1].false_stops, 0);
        assert!(!journal.is_poisoned());
    }
}

#[test]
fn native_empty_bounds_and_file_identity_remain_explicit() {
    let (binding, receipts, _) = receipts();
    let temp = Temp::new(); let mut journal = temp.journal();
    let bytes = fs::read(temp.path()).unwrap();
    let empty = EvaluatedAndonImport::begin_native(&mut journal, &binding, &[]).unwrap().finish().unwrap();
    assert_eq!((empty.completed, empty.status), (0, ImportStatus::Complete));
    let oversized = vec![receipts[0].clone(); MAX_CASES + 1];
    assert!(EvaluatedAndonImport::begin_native(&mut journal, &binding, &oversized).is_err());
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    let other = Temp::new();
    let mut foreign = FileAndonLedger::create(other.path(), LedgerIdentity { deployment: 999, ..identity() }).unwrap();
    assert!(EvaluatedAndonImport::begin_native(&mut foreign, &binding, &[]).is_err());
}

fn canonical_suffix(receipt: &NativeEvaluatedFalseHold) -> Vec<u8> {
    let temp = Temp::new(); let mut journal = temp.journal();
    let prefix = fs::metadata(temp.path()).unwrap().len() as usize;
    journal.record_trusted(receipt.confirmation()).unwrap();
    fs::read(temp.path()).unwrap()[prefix..].to_vec()
}

#[test]
fn native_import_preserves_original_complete_uncertainty_and_partial_tail_rules() {
    let (binding, receipts, expected) = receipts();
    for complete in [true, false] {
        let temp = Temp::new(); let mut journal = temp.journal();
        let head = journal.acknowledged_head();
        let mut cursor = EvaluatedAndonImport::begin_native(&mut journal, &binding, &receipts).unwrap();
        let suffix = canonical_suffix(&receipts[0]);
        let count = if complete { suffix.len() } else { suffix.len() / 2 };
        let mut writer = OpenOptions::new().append(true).open(temp.path()).unwrap();
        writer.write_all(&suffix[..count]).unwrap(); writer.sync_all().unwrap(); drop(writer);
        let failed = cursor.advance(1).unwrap_err();
        assert_eq!((failed.progress.completed, failed.progress.head, failed.progress.status), (0, head, ImportStatus::Failed));
        assert!(cursor.advance(1).is_err()); assert!(cursor.finish().is_err());
        assert!(journal.is_poisoned()); assert!(journal.ledger().is_err()); drop(journal);
        let bytes = fs::read(temp.path()).unwrap();
        if complete {
            let mut restored = FileAndonLedger::open(temp.path(), identity(), head).unwrap();
            let mut cursor = EvaluatedAndonImport::begin_native(&mut restored, &binding, &receipts).unwrap();
            let done = cursor.advance(2).unwrap(); cursor.finish().unwrap();
            assert_eq!((done.newly_recorded, done.already_recorded), (1, 1));
            assert_eq!(restored.ledger().unwrap(), &expected);
        } else {
            assert!(FileAndonLedger::open(temp.path(), identity(), head).is_err());
            assert_eq!(fs::read(temp.path()).unwrap(), bytes);
        }
    }
}

#[test]
fn native_conflicting_existing_last_round_cannot_partially_charge_the_group() {
    let (binding, receipts, _) = receipts();
    let temp = Temp::new(); let mut journal = temp.journal();
    let mut conflict = receipts[1].confirmation().clone(); conflict.report += 1;
    journal.record_trusted(&conflict).unwrap();
    let bytes = fs::read(temp.path()).unwrap(); let head = journal.acknowledged_head();
    assert!(EvaluatedAndonImport::begin_native(&mut journal, &binding, &receipts).is_err());
    assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    assert_eq!(journal.acknowledged_head(), head);
    assert!(journal.ledger().unwrap().confirmation(101).is_none());
}
