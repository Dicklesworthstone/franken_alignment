//! Actual learned output and original two-key publication with delayed producer
//! snapshots. Warm/cold twins exercise the retained replay prefix, not a second
//! machine or a cached positive witness. Scripted review is not detector quality.
use super::*;
use fa_reference::action::ActionState;
use fa_reference::action::consequence::delivery::persistent::{FilePermit, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileHumanPermit;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{
    deferred::FileCaptureObservation,
    heartbeat::feed::{PublicationFeedBatch, PublicationFeedFile, PublicationFeedReport},
};
use fa_reference::action::consequence::delivery::publication_gate::changes::{
    PublicationCaptureOutcome, PublicationChange, PublicationInputCut,
    freshness::PublicationHeartbeat,
};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::witness::refinement::index::routing::WitnessChange;

fn cut(through: u64) -> PublicationInputCut { PublicationInputCut { source: 41, through } }
fn replace(path: &std::path::Path, bytes: &[u8]) {
    let next = path.with_extension("next");
    std::fs::write(&next, bytes).unwrap(); std::fs::rename(next, path).unwrap();
}

struct Case {
    root: Directory,
    config: FileLearnedConfig,
    host: FileOversight,
    reviewer: FileHumanReviewer,
    action: FrozenAction,
    input: CommitteeInput,
    original: FilePublicationCapture,
    source: PublicationInputFile,
    feed: PublicationFeedFile,
    cold: bool,
}
impl Case {
    fn new(cold: bool) -> Self {
        let root = Directory::new(); let mut selection = selected(true);
        selection.feed.as_mut().unwrap().snapshot_fallback = false;
        let config = checked(selection);
        let (mut host, reviewer) = FileOversight::create_with_learned_text_stream(
            root.store(), profile(), config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); generate(&mut host);
        cold_control(&mut host, cold);
        let (action, input) = unbound_proposal(&mut host, 1);
        let original = FilePublicationCapture::new_at_cut(1,
            FileCaptureIdentity { source: 91, generation: 1 }, &action,
            observations(&input, 1, &[0, 2, 4], true), cut(0)).unwrap();
        let source = PublicationInputFile::new(root.0.join("witness-input.bin"), 91).unwrap();
        let feed = PublicationFeedFile::new(root.0.join("feed.bin"), 41).unwrap();
        let mut case = Self { root, config, host, reviewer, action, input, original, source, feed, cold };
        replace(&case.root.0.join("witness-input.bin"), &case.original.to_bytes().unwrap());
        case.feed_window(0, 0);
        case.cold();
        case.host.bind_publication_file_source(case.host.revision(), 1,
            case.original.clone(), requests()).unwrap();
        case.cold(); review(&mut case.host, 1, &case.input);
        case
    }
    fn cold(&mut self) { cold_control(&mut self.host, self.cold); }
    fn feed_window(&mut self, after: u64, through: u64) -> PublicationFeedReport {
        let batch = PublicationFeedBatch::new(PublicationHeartbeat { source: 41, clock_domain: 99,
            generation: through + 1, through, produced_at: ElapsedTick(1) }, after,
            (after + 1..=through).map(|sequence| PublicationChange {
                source: 41, sequence, change: WitnessChange::All,
            }).collect()).unwrap();
        replace(&self.root.0.join("feed.bin"), &batch.to_bytes().unwrap()); self.cold();
        self.host.refresh_publication_feed(self.host.revision(), &self.feed,
            || ElapsedTick(1)).unwrap().unwrap()
    }
    fn packet(&self, generation: u64, through: u64, keys: &[u64]) -> FilePublicationCapture {
        FilePublicationCapture::new_at_cut(1, FileCaptureIdentity { source: 91, generation },
            &self.action, observations(&self.input, generation, keys, true), cut(through)).unwrap()
    }
    fn write(&self, generation: u64, through: u64, keys: &[u64]) {
        replace(&self.root.0.join("witness-input.bin"), &self.packet(generation, through, keys).to_bytes().unwrap());
    }
    fn refresh(&mut self) -> Result<Result<FileCaptureObservation, FileCaptureError>, JournalError> {
        self.cold();
        self.host.refresh_publication_from_file_or_defer(self.host.revision(), 1, &self.source)
    }
    fn keys(&mut self) -> (FilePermit, FileHumanPermit) {
        assert!(!self.refresh().unwrap().unwrap().outcome.deferred()); self.cold();
        let automatic = self.host.authorize(self.host.revision(), 1, &self.input, snapshot()).unwrap();
        self.cold();
        let request = self.host.request_human_approval(self.host.revision(), 1001, 1,
            &self.input, ElapsedTick(40)).unwrap();
        self.cold(); let revision = self.host.revision();
        let human = self.reviewer.approve(&mut self.host, revision, &request).unwrap();
        (automatic, human)
    }
    fn dispatch(&mut self, keys: &(FilePermit, FileHumanPermit)) -> Result<(), JournalError> {
        self.cold();
        self.host.dispatch(self.host.revision(), &keys.0, &keys.1, &self.action, &self.input, snapshot())
    }
    fn assert_history(&self) {
        let bytes = self.root.files();
        assert_eq!(FileOversight::read_stream_publication_with_learned_generation(self.root.store(),
            &profile(), &self.config).unwrap(), self.host.stream_snapshot().unwrap());
        assert_eq!(self.root.files(), bytes, "historical replay must not mutate the original store");
    }
}

#[test]
fn learned_deferral_warm_and_cold_twins_keep_both_keys_and_publish_once_after_catch_up() {
    let mut reference = None;
    for cold in [false, true] {
        let mut case = Case::new(cold); let keys = case.keys();
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        assert!(case.feed_window(0, 1).status.complete()); case.write(2, 0, &[0, 2, 4]);
        let control = case.host.inspect().control;
        let human = case.host.human_status(1001).unwrap();
        for _ in 0..2 {
            let observed = case.refresh().unwrap().unwrap();
            assert_eq!(observed.identity, FileCaptureIdentity { source: 91, generation: 2 });
            assert_eq!(observed.outcome, PublicationCaptureOutcome::Deferred {
                revision: case.host.publication_input_revision(1).unwrap(), observed: cut(0), required_through: 1,
            });
            assert_eq!(case.host.inspect().control, control);
            assert_eq!(case.host.human_status(1001).unwrap(), human);
            assert!(!case.host.publication_source(1).unwrap().unwrap().fresh);
            assert!(case.host.storage_failure().is_none());
        }
        assert_eq!(control.ledger.stages[&1], ActionState::Authorized);
        assert_eq!(control.ledger.reserved, case.action.spec().units);
        assert_eq!(human.disposition, HumanDisposition::Approved);
        // A following ordinary event must replay the actual deferred suffix.
        case.cold(); case.host.observe_time(case.host.revision(), ElapsedTick(2)).unwrap();
        case.write(3, 1, &[0, 2, 4, 99]);
        assert!(!case.refresh().unwrap().unwrap().outcome.deferred());
        case.dispatch(&keys).unwrap();
        assert_eq!(case.host.human_status(1001).unwrap().disposition, HumanDisposition::Consumed);
        assert!(!case.host.publication_source(1).unwrap().unwrap().fresh);
        // Dispatch consumed the catch-up read. First publication stays strict.
        case.cold(); case.host.refresh_publication_from_file(case.host.revision(), 1, &case.source).unwrap().unwrap();
        case.cold();
        let receipt = case.host.publish_checked(case.host.revision(), 1,
            Some(&case.input), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(receipt.basis, PublicationBasis::Revalidated);
        assert_eq!(receipt.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        case.cold();
        assert_eq!(case.host.reconcile(case.host.revision(), 1).unwrap(), Reconciliation::Resolved(receipt.outcome));
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(case.host.inspect().executions, 1);
        assert_eq!(case.host.inspect().control.ledger.reserved, 0);
        assert_eq!(case.host.inspect().control.ledger.charged, case.action.spec().units);
        assert_eq!(case.host.retained_publication_evidence(1).unwrap().original(), case.original.inputs());
        let published = case.host.stream_snapshot().unwrap();
        assert_eq!(published.confirmed.visible(), b"OK"); case.assert_history();
        let actual = (receipt, published, numerical, case.host.publication_input_cut(1).unwrap());
        if let Some(expected) = &reference { assert_eq!(&actual, expected); } else { reference = Some(actual); }
    }
}

#[test]
fn learned_catch_up_cannot_rebase_negative_witnesses_or_changed_opaque_input() {
    for defect in 0..5 {
        let mut case = Case::new(false); let keys = case.keys();
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        case.feed_window(0, 1); case.write(2, 0, &[0, 2, 4]);
        assert!(case.refresh().unwrap().unwrap().outcome.deferred());
        let members: &[u64] = match defect {
            1 => &[0, 1, 2, 4], 2 => &[0, 2, 3, 4], 3 => &[0, 2, 4, 7], _ => &[0, 2, 4, 99],
        };
        let mut packet = case.packet(3, 1, members);
        if defect == 4 {
            let old = packet.inputs().opaque().unwrap(); let mut profile = old.input_profile().clone();
            profile.tokenizer_epoch += 1;
            let changed = ActualHelperInput::new(old.submitted_bytes().to_vec(), profile,
                old.ordered_parts().to_vec(), old.omissions().to_vec()).unwrap();
            packet = FilePublicationCapture::new_at_cut(1, packet.identity(), &case.action,
                FilePublicationInputs::new(packet.inputs().structured().cloned(), Some(changed)), cut(1)).unwrap();
        }
        replace(&case.root.0.join("witness-input.bin"), &packet.to_bytes().unwrap());
        assert!(!case.refresh().unwrap().unwrap().outcome.deferred());
        if defect == 0 { case.dispatch(&keys).unwrap(); }
        else {
            let before = case.host.inspect();
            assert_eq!(case.dispatch(&keys), Err(JournalError::Contract(Error::Stale)));
            assert_eq!(case.host.inspect(), before);
            assert_eq!(case.host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
            assert_eq!(case.host.inspect().control.ledger.charged, 0);
            assert_eq!(case.host.inspect().control.ledger.reserved, case.action.spec().units);
            // A new real image matching the ORIGINAL basis is a permitted neighbor.
            case.write(4, 1, &[0, 2, 4, 99]);
            assert!(!case.refresh().unwrap().unwrap().outcome.deferred()); case.dispatch(&keys).unwrap();
        }
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(case.host.inspect().control.ledger.charged, case.action.spec().units);
        assert_eq!(case.host.inspect().executions, 0, "dispatch does not fabricate first publication");
        assert_eq!(case.host.retained_publication_evidence(1).unwrap().original(), case.original.inputs());
        case.assert_history();
    }
}

#[test]
fn learned_deferral_missing_history_equivocation_and_regression_are_not_waits() {
    for defect in 0..4 { for cold in [false, true] {
        let mut case = Case::new(cold); let _keys = case.keys();
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        if defect == 0 {
            assert!(!case.feed_window(1, 2).status.complete()); case.write(2, 0, &[0, 2, 4]);
        } else {
            case.feed_window(0, 1); case.write(2, 0, &[0, 2, 4]);
            assert!(case.refresh().unwrap().unwrap().outcome.deferred());
            match defect {
                1 => case.write(2, 0, &[0, 2, 4, 99]),
                2 => case.write(1, 0, &[0, 2, 4]),
                _ => case.write(2, 1, &[0, 2, 4]),
            }
        }
        assert_eq!(case.refresh(), Err(JournalError::Contract(match defect {
            0 => Error::Incomplete, 2 => Error::Stale, _ => Error::Binding,
        })));
        assert!(case.host.storage_failure().is_some());
        assert_eq!(case.host.inspect().executions, 0);
        assert_eq!(case.host.inspect().control.ledger.reserved, case.action.spec().units);
        assert_eq!(case.host.inspect().control.ledger.charged, 0);
        assert_eq!(case.host.learned_generation_inspection(), Err(JournalError::Unavailable));
        let bytes = case.root.files();
        let history = FileOversight::read_stream_publication_with_learned_generation(case.root.store(),
            &profile(), &case.config).unwrap();
        assert!(history.confirmed.visible().is_empty()); assert_eq!(case.root.files(), bytes);
        // Recovery uses the original acknowledged prefix, not the refused image.
        drop(case.host);
        case.host = FileOversight::open_with_learned_text_stream(case.root.store(), profile(), &case.config).unwrap().0;
        assert_eq!(case.host.publication_source(1).unwrap().unwrap().generation, if defect == 0 { 1 } else { 2 });
        assert!(!case.host.publication_source(1).unwrap().unwrap().fresh);
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
    } }
}

#[test]
fn learned_deferral_recovery_keeps_high_water_marks_and_fences_original_keys() {
    for cold in [false, true] {
        let mut case = Case::new(cold); let keys = case.keys();
        case.feed_window(0, 1); case.write(7, 0, &[0, 2, 4]);
        assert!(case.refresh().unwrap().unwrap().outcome.deferred());
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        let before = case.host.inspect(); let files = case.root.files();
        assert_eq!(case.host.refresh_publication_from_file_or_defer(before.revision - 1, 1, &case.source),
            Err(JournalError::Contract(Error::Stale)));
        assert_eq!(case.host.inspect(), before); assert_eq!(case.root.files(), files);
        let cut_before = case.host.publication_input_cut(1).unwrap(); case.assert_history();
        drop(case.host);
        case.host = FileOversight::open_with_learned_text_stream(case.root.store(), profile(), &case.config).unwrap().0;
        assert!(case.host.learned_generation_inspection().unwrap().paused);
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(case.host.publication_source(1).unwrap().unwrap().generation, 7);
        assert_eq!(case.host.publication_input_cut(1).unwrap(), cut_before);
        assert!(!case.host.publication_source(1).unwrap().unwrap().fresh);
        assert_eq!(case.host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
        assert_eq!(case.host.inspect().control.ledger.reserved, 0);
        assert_eq!(case.dispatch(&keys), Err(JournalError::Contract(Error::Binding)));
        let before = case.host.inspect(); let files = case.root.files();
        assert_eq!(case.refresh(), Err(JournalError::Contract(Error::WrongState)));
        assert_eq!(case.host.inspect(), before); assert_eq!(case.root.files(), files);
    }
}

#[test]
fn learned_deferral_does_not_acknowledge_failed_withdrawal_or_extend_dispatched_work() {
    for failed_write in [false, true] {
        let mut case = Case::new(false); let keys = case.keys();
        let numerical = case.host.learned_generation_inspection().unwrap().numerical;
        if failed_write {
            case.feed_window(0, 1); case.write(2, 0, &[0, 2, 4]);
            let before = case.host.inspect();
            std::fs::write(case.root.store().join("delivery.pending"), b"occupied journal staging path").unwrap();
            assert!(matches!(case.refresh(), Err(JournalError::Io(_))));
            assert_eq!(case.host.inspect(), before); assert!(case.host.storage_failure().is_some());
            assert_eq!(case.refresh(), Err(JournalError::Unavailable));
            assert_eq!(case.host.inspect().control.ledger.reserved, case.action.spec().units);
        } else {
            assert!(!case.refresh().unwrap().unwrap().outcome.deferred()); case.dispatch(&keys).unwrap();
            case.feed_window(0, 1); case.write(2, 0, &[0, 2, 4]);
            let before = case.host.inspect(); let files = case.root.files();
            assert_eq!(case.refresh(), Err(JournalError::Contract(Error::WrongState)));
            assert_eq!(case.host.inspect(), before); assert_eq!(case.root.files(), files);
            // The strict final acquisition retains its original quarantine.
            assert_eq!(case.host.refresh_publication_from_file(case.host.revision(), 1, &case.source),
                Err(JournalError::Contract(Error::Stale)));
            assert!(case.host.storage_failure().is_some());
            assert_eq!(case.host.inspect().control.ledger.charged, case.action.spec().units);
        }
        assert_eq!(case.host.learned_generation_inspection(), Err(JournalError::Unavailable));
        assert_eq!(case.host.inspect().executions, 0);
        assert_eq!(case.host.publish_checked(case.host.revision(), 1,
            Some(&case.input), snapshot(), ElapsedTick(2)).err(), Some(JournalError::Unavailable));
        drop(case.host);
        case.host = FileOversight::open_with_learned_text_stream(case.root.store(), profile(), &case.config).unwrap().0;
        assert_eq!(case.host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(case.host.inspect().executions, 0);
    }
}
