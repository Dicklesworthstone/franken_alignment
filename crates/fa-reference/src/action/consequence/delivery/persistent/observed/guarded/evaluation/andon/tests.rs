use super::*;
use super::super::super::super::credibility::andon::tests as f;
use super::super::super::FileRecoveryFloor;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::oversight::credibility::{Assessment, GroundTruth};
use crate::round::Verdict;

fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn owner(root: &f::Directory) -> (FileOversight, FileEvaluatedOversightRoles) {
    let (mut host, roles) = FileOversight::create_andon_guarded(root.store(), f::profile(), &guards(),
        None, f::protocol(), &f::config()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles)
}
fn requirements(host: &FileOversight) -> FileAndonRecoveryRequirements {
    let control = host.inspect().control;
    let base = FileRecoveryRequirements { guards: guards(), effective_policy: f::profile().delivery.policy,
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: control.sequence, authority_epoch: control.ledger.epoch } };
    FileAndonRecoveryRequirements::new(base, f::config(),
        host.andon_ledger().unwrap().windows().values().map(|state| state.budget).collect(),
        host.andon_source().unwrap().revision()).unwrap()
}

#[test]
fn first_image_contains_every_guard_and_both_native_ledgers_or_returns_no_roles() {
    for stage in f::BARRIERS {
        let root = f::Directory::new();
        let prepared = PreparedGuardedBootstrap::prepare(f::profile(), &guards(), None).unwrap()
            .evaluated(f::protocol()).unwrap().with_andon(&f::config()).unwrap();
        let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(stage);
        let JournalError::Io(failure) = prepared.publish(store).unwrap_err() else { panic!("expected original barrier"); };
        assert_eq!(failure.operation, stage);
        if stage == JournalIo::DirectorySync {
            let (host, _) = FileOversight::open(root.store(), f::profile()).unwrap();
            assert_eq!(host.andon_config().unwrap(), Some(&f::config()));
            assert_eq!(host.credibility_report().unwrap().retained_cases, 0);
            assert!(!host.clock_ready());
        } else { assert!(!root.store().join(storage::CANONICAL).exists()); }
    }
    let root = f::Directory::new(); let (host, _) = owner(&root);
    assert_eq!(host.revision(), 4); // three bootstrap inputs plus explicit Time
    assert_eq!(host.andon_ledger().unwrap().windows().len(), 1);
}

#[test]
fn anchored_restart_resumes_censored_labels_with_new_roles_and_fresh_time_only() {
    let root = f::Directory::new(); let (mut host, old_roles) = owner(&root);
    f::case(&mut host, 1, Verdict::Hold); f::case(&mut host, 2, Verdict::Hold);
    f::assess(&mut host, &old_roles.evaluator, 1, GroundTruth::Censored).unwrap();
    f::assess(&mut host, &old_roles.evaluator, 2, GroundTruth::Benign).unwrap();
    let old_ticket = host.evaluation_ticket(101).unwrap();
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap();
    let old_receipt = host.andon_receipt(102).unwrap().clone(); drop(host);
    let (mut restored, roles) = FileOversight::open_andon_guarded(root.store(), f::profile(),
        &expected, &f::protocol(), &anchor).unwrap();
    assert!(!restored.clock_ready());
    assert_eq!(restored.andon_receipt(102).unwrap(), &old_receipt);
    assert_eq!(restored.credibility_report().unwrap().censored_cases, 1);
    assert_eq!(f::assess(&mut restored, &old_roles.evaluator, 1, GroundTruth::Benign), Err(JournalError::Contract(Error::Binding)));
    let revision = restored.revision();
    assert_eq!(roles.evaluator.assess(&mut restored, revision, &old_ticket,
        Assessment { origin: 1, evidence_id: [17; 32], truth: GroundTruth::Benign }), Err(JournalError::Contract(Error::Binding)));
    let before = root.bytes();
    assert_eq!(f::assess(&mut restored, &roles.evaluator, 1, GroundTruth::Benign), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(root.bytes(), before); assert_eq!(restored.revision(), revision);
    assert!(!f::assess(&mut restored, &roles.evaluator, 2, GroundTruth::Benign).unwrap());
    restored.observe_time(restored.revision(), ElapsedTick(40)).unwrap();
    f::assess(&mut restored, &roles.evaluator, 1, GroundTruth::Benign).unwrap();
    assert_eq!(restored.andon_receipt(101).unwrap().confirmation().confirmed_at, 40);
    assert_eq!(restored.andon_receipt(101).unwrap().confirmation().occurred_at, 1);
    assert_eq!(restored.andon_ledger().unwrap().windows()[&1].false_stops, 2);
    assert_eq!(restored.evaluation_history(101).unwrap().len(), 2);
}

#[test]
fn complete_window_contract_and_each_floor_are_checked_before_any_recovery_write() {
    let root = f::Directory::new(); let (mut host, _) = owner(&root);
    host.register_andon_window(host.revision(), AndonBudget { window: 2, start: 5, end: 20, ..f::budget() }).unwrap();
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap(); let before = root.bytes(); drop(host);
    for changed in 0..6 {
        let mut wrong = expected.clone(); let mut protocol = f::protocol();
        match changed {
            0 => wrong.windows.pop().map(|_| ()).unwrap(),
            1 => wrong.windows[1].cost_per_stop = 1,
            2 => wrong.minimum_evaluation_revision += 1,
            3 => wrong.base.minimum.authority_epoch += 1,
            4 => protocol.period += 1,
            _ => wrong.config = FileAndonConfig::new(
                crate::round::review::andon::LedgerIdentity { deployment: 404, evaluator: 202 }, vec![f::budget()]).unwrap(),
        }
        assert!(FileOversight::open_andon_guarded(root.store(), f::profile(), &wrong, &protocol, &anchor).is_err());
        assert_eq!(root.bytes(), before);
    }
    let (restored, _) = FileOversight::open_andon_guarded(root.store(), f::profile(), &expected, &f::protocol(), &anchor).unwrap();
    assert_eq!(restored.andon_ledger().unwrap().windows().len(), 2);
}

#[test]
fn exact_anchor_rejects_a_valid_equal_counter_history_with_different_incident_provenance() {
    let root = f::Directory::new(); let (mut host, roles) = owner(&root);
    f::case(&mut host, 1, Verdict::Hold); f::case(&mut host, 2, Verdict::Hold);
    f::assess(&mut host, &roles.evaluator, 2, GroundTruth::Benign).unwrap();
    f::assess(&mut host, &roles.evaluator, 1, GroundTruth::Benign).unwrap();
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap();
    let original = root.bytes(); let mut fork = host.events.clone(); let last = fork.len() - 1;
    fork.swap(last, last - 1);
    let checked = Machine::replay(&f::profile(), &fork).unwrap();
    assert_eq!(checked.broker.andon_ledger().unwrap().windows()[&1].false_stops, 2);
    assert_eq!(checked.broker.andon_ledger().unwrap().windows()[&1].incident.as_ref().unwrap().opened_by_round, 102);
    let alternate = journal::encode(&f::profile(), host.store.identity(), &fork).unwrap(); drop(host);
    std::fs::write(root.store().join(storage::CANONICAL), &alternate).unwrap();
    assert!(FileOversight::open_andon_guarded(root.store(), f::profile(), &expected, &f::protocol(), &anchor).is_err());
    assert_eq!(root.bytes(), alternate);
    std::fs::write(root.store().join(storage::CANONICAL), original).unwrap();
    assert!(FileOversight::open_andon_guarded(root.store(), f::profile(), &expected, &f::protocol(), &anchor).is_ok());
}

#[test]
fn every_recovery_fence_barrier_preserves_source_labels_and_issues_no_partial_roles() {
    for stage in f::BARRIERS {
        let root = f::Directory::new(); let (mut host, roles) = owner(&root);
        f::case(&mut host, 1, Verdict::Hold); f::assess(&mut host, &roles.evaluator, 1, GroundTruth::Benign).unwrap();
        let expected = requirements(&host); let anchor = host.history_anchor().unwrap();
        let accounting = host.andon_ledger().unwrap().clone(); let receipt = host.andon_receipt(101).unwrap().clone(); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(stage);
        let result = FileOversight::open_andon_guarded_store(store, f::profile(), &expected, &f::protocol(), &anchor);
        let JournalError::Io(failure) = result.unwrap_err() else { panic!("expected original barrier"); };
        assert_eq!(failure.operation, stage);
        let read = FileOversight::read_andon_guarded(root.store(), &f::profile(), &expected, &f::protocol(), &anchor).unwrap();
        assert_eq!(read.accounting, accounting); assert_eq!(read.receipts, vec![receipt]);
        let (mut restored, fresh) = FileOversight::open_andon_guarded(root.store(), f::profile(), &expected, &f::protocol(), &anchor).unwrap();
        assert_eq!(f::assess(&mut restored, &roles.evaluator, 1, GroundTruth::Benign), Err(JournalError::Contract(Error::Binding)));
        let bytes = root.bytes(); assert!(!f::assess(&mut restored, &fresh.evaluator, 1, GroundTruth::Benign).unwrap());
        assert_eq!(root.bytes(), bytes); assert!(!restored.clock_ready());
    }
}

#[test]
fn read_only_snapshot_works_beside_locked_writer_and_keeps_pending_cases_explicit() {
    let root = f::Directory::new(); let (mut host, _) = owner(&root); f::case(&mut host, 1, Verdict::Hold);
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap(); let bytes = root.bytes();
    let read = FileOversight::read_andon_guarded(root.store(), &f::profile(), &expected, &f::protocol(), &anchor).unwrap();
    assert_eq!(read.credibility.report.pending_cases, 1); assert!(read.receipts.is_empty());
    assert_eq!(read.accounting.windows()[&1].false_stops, 0); assert_eq!(root.bytes(), bytes);
    assert!(FileOversight::open_andon_guarded(root.store(), f::profile(), &expected, &f::protocol(), &anchor).is_err());
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn protected_recovery_refuses_missing_activation_and_truncated_canonical_source() {
    let root = f::Directory::new(); let (mut host, _) = owner(&root);
    let expected = requirements(&host); let anchor = host.history_anchor().unwrap(); let original = root.bytes();
    let mut missing = host.events.clone(); missing.retain(|e| !matches!(e, Event::Credibility(CredibilityEvent::EnableAndon(_))));
    let missing_bytes = journal::encode(&f::profile(), host.store.identity(), &missing).unwrap();
    for bytes in [&original[..original.len() / 2], missing_bytes.as_slice()] {
        std::fs::write(root.store().join(storage::CANONICAL), bytes).unwrap();
        assert!(FileOversight::read_andon_guarded(root.store(), &f::profile(), &expected, &f::protocol(), &anchor).is_err());
        assert_eq!(root.bytes(), bytes);
    }
    std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    // Independent expectations may allow a genuine append-only successor.
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(FileOversight::read_andon_guarded(root.store(), &f::profile(), &expected, &f::protocol(), &anchor).is_ok());
}
