//! Cooperative import through the ORIGINAL locked, sync-before-ack Andon file.
//! Semantic preflight is all-or-nothing; disk publication is an explicit prefix
//! of individually acknowledged records, not a new atomic batch/file format.

use crate::file::{Error, andon::FileAndonLedger};
use crate::Head;
use fa_reference::Error as ModelError;
use fa_reference::action::consequence::congress::credibility::MAX_CASES;
use fa_reference::action::consequence::oversight::andon::EvaluatedFalseHold;
use fa_reference::round::review::andon::RecordResult;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportStatus { Pending, Complete, Failed }

/// Counts describe only successfully acknowledged ORIGINAL record calls. An
/// error may have persisted an additional record; recovery, not this counter,
/// decides that. Retain `head` independently and retain the input receipts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportProgress {
    pub total: usize,
    pub completed: usize,
    pub newly_recorded: usize,
    pub already_recorded: usize,
    pub head: Head,
    pub status: ImportStatus,
}

#[derive(Debug)]
pub struct ImportFailure { pub error: Error, pub progress: ImportProgress }
impl fmt::Display for ImportFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "evaluated Andon import after {}/{} acknowledgments: {}",
            self.progress.completed, self.progress.total, self.error)
    }
}
impl std::error::Error for ImportFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(&self.error) }
}

/// Holds the original exclusive writer borrow and immutable input slice. Drop
/// is cancellation of local iteration only, never a refund or a claim that an
/// uncertain write did not persist. It does not close the underlying writer.
///
/// ```compile_fail,E0599
/// use fa_journal::evaluated_andon::EvaluatedAndonImport;
/// fn duplicate(cursor: EvaluatedAndonImport<'_>) { cursor.clone(); }
/// ```
#[derive(Debug)]
pub struct EvaluatedAndonImport<'a> {
    journal: &'a mut FileAndonLedger,
    receipts: &'a [EvaluatedFalseHold],
    progress: ImportProgress,
}

impl<'a> EvaluatedAndonImport<'a> {
    /// No I/O or canonical changes during preflight. Reject a bad last entry
    /// before a good first entry is appended. Existing original storage limits
    /// still apply at each append; preflight is not a disk-capacity reservation.
    pub fn begin(journal: &'a mut FileAndonLedger, receipts: &'a [EvaluatedFalseHold]) -> Result<Self, Error> {
        if receipts.len() > MAX_CASES { return Err(crate::Error::Limit.into()); }
        let mut candidate = journal.ledger()?.clone();
        for receipt in receipts { receipt.record(&mut candidate).map_err(crate::Error::Model)?; }
        let progress = ImportProgress {
            total: receipts.len(), completed: 0, newly_recorded: 0, already_recorded: 0,
            head: journal.acknowledged_head(),
            status: if receipts.is_empty() { ImportStatus::Complete } else { ImportStatus::Pending },
        };
        Ok(Self { journal, receipts, progress })
    }

    pub fn progress(&self) -> ImportProgress { self.progress }

    /// At most max_records original calls, including exact retries. Zero makes
    /// no calls. Each new record keeps the original write+sync causal barrier;
    /// this bounds record count, not wall time or interruptible filesystem I/O.
    pub fn advance(&mut self, max_records: usize) -> Result<ImportProgress, ImportFailure> {
        if self.progress.status == ImportStatus::Failed {
            return Err(self.failure(crate::Error::Model(ModelError::WrongState).into()));
        }
        for _ in 0..max_records.min(self.receipts.len() - self.progress.completed) {
            let fact = self.receipts[self.progress.completed].confirmation();
            // Latch BEFORE entry, so caught unwinding cannot resume this cursor.
            self.progress.status = ImportStatus::Failed;
            let (result, head) = self.journal.record_trusted(fact).map_err(|error| self.failure(error))?;
            self.progress.completed += 1;
            self.progress.head = head;
            match result {
                RecordResult::Recorded { .. } => self.progress.newly_recorded += 1,
                RecordResult::AlreadyRecorded { .. } => self.progress.already_recorded += 1,
            }
            self.progress.status = if self.progress.completed == self.receipts.len() {
                ImportStatus::Complete
            } else { ImportStatus::Pending };
        }
        Ok(self.progress)
    }

    /// Finishing an incomplete/failed import cannot turn a prefix into success.
    pub fn finish(self) -> Result<ImportProgress, ImportFailure> {
        if self.progress.status != ImportStatus::Complete {
            return Err(self.failure(crate::Error::Model(ModelError::Incomplete).into()));
        }
        Ok(self.progress)
    }

    fn failure(&self, error: Error) -> ImportFailure { ImportFailure { error, progress: self.progress } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fa_reference::action::consequence::congress::credibility::{
        Campaign, CaseSpec, CredibilityLedger, EvaluationLabel, EvaluationScope,
        HelperGeneration, LabelSource, LabelVerdict, Observation,
    };
    use fa_reference::action::consequence::oversight::andon::EvaluationBinding;
    use fa_reference::round::review::{CongressReview, ExactStatus, Member, Requirements, ReviewPolicy};
    use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};
    use fa_reference::reducer::Caps;
    use fa_reference::round::{Verdict, commitment};
    use std::collections::BTreeMap;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!("fa-evaluated-andon-{}-{}",
                std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap(); Self(path)
        }
        fn path(&self) -> PathBuf { self.0.join("ledger") }
    }
    impl Drop for Temp { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }
    fn identity() -> LedgerIdentity { LedgerIdentity { deployment: 1, evaluator: 2 } }
    fn journal(temp: &Temp) -> FileAndonLedger {
        let mut journal = FileAndonLedger::create(temp.path(), identity()).unwrap();
        journal.register_window(AndonBudget { window: 1, start: 0, end: 15,
            false_stops_allowed: 2, cost_per_stop: 7 }).unwrap();
        journal
    }
    fn receipt(round: u64) -> EvaluatedFalseHold {
        let scope = EvaluationScope { campaign: 7, model_generation: 3, evaluator_generation: 4,
            held_out_manifest: [8; 32] };
        let helpers = BTreeMap::from([("helper".into(), HelperGeneration { generation: 10, cohort: "c".into() })]);
        let binding = EvaluationBinding::new(identity(), scope.clone(), "evaluator".into(), helpers.clone()).unwrap();
        let mut evidence = CredibilityLedger::new(Campaign {
            scope, label_owner: "evaluator".into(), helpers, strata: ["attack".into()].into(),
            cases: vec![CaseSpec { id: round, stratum: "attack".into(), evidence_root: [9; 32], dispatch_sequence: 10 }],
        }).unwrap();
        evidence.record_observations(round, BTreeMap::from([
            ("helper".into(), Observation::Hold { first_sequence: 5 })])).unwrap();
        evidence.record_label(round, EvaluationLabel { owner: "evaluator".into(), evaluator_generation: 4,
            source: LabelSource::IndependentEvaluation, evidence_root: [9; 32], recorded_sequence: 20,
            verdict: LabelVerdict::Safe }).unwrap();
        let policy = ReviewPolicy::new(vec![Member { id: "helper".into(), cohort: "c".into(), weight: 1 }],
            Requirements { caps: Caps { per_member: 1, per_cohort: 1 }, min_permit_weight: 1,
                max_hold_weight: 0, min_permit_cohorts: 1 }).unwrap();
        let mut review = CongressReview::new(policy, round, &[9; 32]).unwrap();
        review.commit("helper", commitment(round, "helper", &[9; 32], Verdict::Hold, b"s").unwrap()).unwrap();
        review.open_reveals().unwrap(); review.reveal("helper", Verdict::Hold, b"s").unwrap();
        EvaluatedFalseHold::from_review(&binding, &evidence.seal(100).unwrap(), round, &review, ExactStatus::Clear).unwrap()
    }

    #[test]
    fn budgeted_publication_and_reopen_match_original_accounting() {
        let temp = Temp::new(); let mut journal = journal(&temp);
        let original = fs::read(temp.path()).unwrap(); let head = journal.acknowledged_head();
        let receipts = [receipt(1), receipt(2)];
        let mut cursor = EvaluatedAndonImport::begin(&mut journal, &receipts).unwrap();
        assert_eq!(cursor.advance(0).unwrap().completed, 0);
        assert_eq!(fs::read(temp.path()).unwrap(), original);
        let first = cursor.advance(1).unwrap();
        assert_eq!((first.completed, first.newly_recorded, first.status), (1, 1, ImportStatus::Pending));
        assert_eq!(first.head.sequence, head.sequence + 1);
        let final_progress = cursor.advance(1).unwrap();
        assert_eq!(cursor.finish().unwrap(), final_progress);
        let expected = journal.ledger().unwrap().clone(); drop(journal);
        let restored = FileAndonLedger::open(temp.path(), identity(), final_progress.head).unwrap();
        assert_eq!(restored.ledger().unwrap(), &expected);
        assert_eq!(expected.windows()[&1].false_stops, 2);
        assert!(expected.windows()[&1].incident.is_some());
    }

    #[test]
    fn interruption_resumes_same_receipts_without_double_charging() {
        let temp = Temp::new(); let mut journal = journal(&temp);
        let receipts = [receipt(1), receipt(2)];
        let mut cursor = EvaluatedAndonImport::begin(&mut journal, &receipts).unwrap();
        let first = cursor.advance(1).unwrap(); drop(cursor); drop(journal);
        let mut journal = FileAndonLedger::open(temp.path(), identity(), first.head).unwrap();
        let mut cursor = EvaluatedAndonImport::begin(&mut journal, &receipts).unwrap();
        let done = cursor.advance(usize::MAX).unwrap();
        assert_eq!((done.newly_recorded, done.already_recorded, done.completed), (1, 1, 2));
        cursor.finish().unwrap();
        let bytes = fs::read(temp.path()).unwrap();
        let mut retry = EvaluatedAndonImport::begin(&mut journal, &receipts).unwrap();
        let repeated = retry.advance(usize::MAX).unwrap();
        assert_eq!((repeated.newly_recorded, repeated.already_recorded, repeated.head), (0, 2, done.head));
        retry.finish().unwrap(); assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    }

    #[test]
    fn duplicate_in_same_group_spends_one_step_but_no_write() {
        let temp = Temp::new(); let mut journal = journal(&temp);
        let one = receipt(1); let receipts = [one.clone(), one, receipt(2)];
        let mut cursor = EvaluatedAndonImport::begin(&mut journal, &receipts).unwrap();
        let first = cursor.advance(1).unwrap(); let bytes = fs::read(temp.path()).unwrap();
        let duplicate = cursor.advance(1).unwrap();
        assert_eq!((duplicate.completed, duplicate.newly_recorded, duplicate.already_recorded), (2, 1, 1));
        assert_eq!(duplicate.head, first.head); assert_eq!(fs::read(temp.path()).unwrap(), bytes);
        let done = cursor.advance(1).unwrap(); cursor.finish().unwrap();
        assert_eq!((done.completed, done.newly_recorded, done.already_recorded), (3, 2, 1));
        assert_eq!(journal.ledger().unwrap().windows()[&1].false_stops, 2);
    }

    #[test]
    fn conflicting_last_entry_refuses_before_first_write() {
        let temp = Temp::new(); let mut journal = journal(&temp);
        let receipts = [receipt(1), receipt(2)];
        let mut conflicting = receipts[1].confirmation().clone(); conflicting.report += 1;
        journal.record_trusted(&conflicting).unwrap();
        let bytes = fs::read(temp.path()).unwrap(); let head = journal.acknowledged_head();
        assert!(EvaluatedAndonImport::begin(&mut journal, &receipts).is_err());
        assert_eq!(journal.acknowledged_head(), head); assert_eq!(fs::read(temp.path()).unwrap(), bytes);
        assert!(journal.ledger().unwrap().confirmation(1).is_none()); assert!(!journal.is_poisoned());
    }

    #[test]
    fn incomplete_finish_empty_groups_and_import_bounds_are_explicit() {
        let temp = Temp::new(); let mut journal = journal(&temp);
        let receipts = [receipt(1)]; let bytes = fs::read(temp.path()).unwrap();
        let cursor = EvaluatedAndonImport::begin(&mut journal, &receipts).unwrap();
        assert!(cursor.finish().is_err());
        let empty = EvaluatedAndonImport::begin(&mut journal, &[]).unwrap().finish().unwrap();
        assert_eq!((empty.completed, empty.status), (0, ImportStatus::Complete));
        assert!(EvaluatedAndonImport::begin(&mut journal, &vec![receipt(1); MAX_CASES + 1]).is_err());
        assert_eq!(fs::read(temp.path()).unwrap(), bytes);
    }

    // Copy a REAL canonical record from another original writer with the same
    // prefix. This injects a storage completion/acknowledgment gap, not a second
    // encoder or a fixture that asserts persistence without writing bytes.
    fn canonical_suffix(receipt: &EvaluatedFalseHold) -> Vec<u8> {
        let temp = Temp::new(); let mut journal = journal(&temp);
        let prefix = fs::metadata(temp.path()).unwrap().len() as usize;
        journal.record_trusted(receipt.confirmation()).unwrap();
        fs::read(temp.path()).unwrap()[prefix..].to_vec()
    }

    #[test]
    fn complete_uncertain_suffix_is_replayed_not_refunded_or_duplicated() {
        let temp = Temp::new(); let mut journal = journal(&temp);
        let head = journal.acknowledged_head(); let receipts = [receipt(1), receipt(2)];
        let mut cursor = EvaluatedAndonImport::begin(&mut journal, &receipts).unwrap();
        let mut injection = OpenOptions::new().append(true).open(temp.path()).unwrap();
        injection.write_all(&canonical_suffix(&receipts[0])).unwrap(); injection.sync_all().unwrap(); drop(injection);
        let failed = cursor.advance(1).unwrap_err();
        assert_eq!((failed.progress.completed, failed.progress.head, failed.progress.status), (0, head, ImportStatus::Failed));
        assert!(cursor.advance(1).is_err()); drop(cursor);
        assert!(journal.is_poisoned()); assert!(journal.ledger().is_err());
        assert!(EvaluatedAndonImport::begin(&mut journal, &[]).is_err()); drop(journal);
        let mut journal = FileAndonLedger::open(temp.path(), identity(), head).unwrap();
        assert_eq!(journal.ledger().unwrap().windows()[&1].false_stops, 1);
        let mut cursor = EvaluatedAndonImport::begin(&mut journal, &receipts).unwrap();
        let done = cursor.advance(2).unwrap();
        assert_eq!((done.newly_recorded, done.already_recorded), (1, 1)); cursor.finish().unwrap();
        assert_eq!(journal.ledger().unwrap().windows()[&1].false_stops, 2);
    }

    #[test]
    fn partial_tail_remains_diagnostic_and_never_becomes_completed_import() {
        let temp = Temp::new(); let mut journal = journal(&temp);
        let head = journal.acknowledged_head(); let receipts = [receipt(1)];
        let mut cursor = EvaluatedAndonImport::begin(&mut journal, &receipts).unwrap();
        let suffix = canonical_suffix(&receipts[0]);
        let mut injection = OpenOptions::new().append(true).open(temp.path()).unwrap();
        injection.write_all(&suffix[..suffix.len() / 2]).unwrap(); injection.sync_all().unwrap(); drop(injection);
        assert_eq!(cursor.advance(1).unwrap_err().progress.completed, 0);
        assert!(cursor.finish().is_err()); drop(journal);
        let before = fs::read(temp.path()).unwrap();
        assert!(FileAndonLedger::open(temp.path(), identity(), head).is_err());
        assert_eq!(fs::read(temp.path()).unwrap(), before);
    }
}
