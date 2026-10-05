//! Current original input plus real, explicitly policy-only file observations.
use super::*;
use crate::action::consequence::oversight::evidence_source::{EvidenceError, EvidenceIdentity,
    EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use std::cell::Cell;
use std::path::{Path, PathBuf};

fn native_tick(driver: &mut FileSupervisedDriver, human: Option<&FileHumanPermit>)
    -> Result<FileDriverEvent, JournalError>
{
    driver.step_computed(|| ElapsedTick(1), || Ok(snapshot()), human)
}
fn complete_native(h: &mut Harness) -> FileDriverEvent {
    for _ in 0..64 {
        let event = native_tick(&mut h.driver, None).unwrap();
        if !matches!(event, FileDriverEvent::Workers { .. }) { return event; }
    }
    panic!("current-source loop exceeded the original finite schedule");
}
fn approve_current(h: &mut Harness) -> FileHumanPermit {
    let request = h.driver.request_learned_human_approval(1001, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = h.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision(); h.reviewer.approve(&mut host, revision, &request).unwrap()
}
fn policy_observation(generation: u64) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 99, generation, scope: profile().delivery.scope },
        snapshot(), BTreeMap::from([("alpha".to_owned(), Vec::new()), ("beta".to_owned(), Vec::new())])).unwrap()
}
fn replace(path: &Path, observation: &EvidenceSnapshot) {
    let pending = path.with_extension("next");
    std::fs::write(&pending, observation.encode()).unwrap();
    std::fs::rename(pending, path).unwrap();
}
fn policy_file(root: &Directory) -> (PathBuf, FileEvidenceSource) {
    let path = root.store().parent().unwrap().join("policy.json");
    replace(&path, &policy_observation(1));
    let source = FileEvidenceSource::new(&path, 99, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    (path, source)
}

#[test]
fn computed_current_loop_needs_no_caller_packets_even_after_terminal_report_extraction() {
    let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 1);
    assert!(matches!(complete_native(&mut h), FileDriverEvent::ReviewApplied { .. }));
    let saved = h.driver.take_learned_probe_review().unwrap().unwrap();
    assert!(saved.review().history().len() > 1);
    assert!(matches!(native_tick(&mut h.driver, None).unwrap(), FileDriverEvent::AwaitingHuman { .. }));
    let human = approve_current(&mut h);
    assert!(matches!(native_tick(&mut h.driver, Some(&human)).unwrap(), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(native_tick(&mut h.driver, None).unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
        if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
    assert!(matches!(h.driver.step_computed(|| ElapsedTick(1), || panic!("no policy during settlement"), None).unwrap(),
        FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }), .. }));
    assert!(matches!(h.port.poll(&h.ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(saved.review().status(), FileLearnedProbeStatus::Finished);
    assert_eq!(h.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn computed_current_loop_rereads_real_policy_file_through_refinement_and_both_keys() {
    let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 1);
    let (path, mut source) = policy_file(&root);
    let mut finished = false;
    for _ in 0..64 {
        let before = work(&h.driver);
        let report = h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None);
        assert_eq!(report.observations, vec![Ok(policy_observation(1).identity())]);
        assert!(report.source_updates.is_empty(), "policy-only reads are not durable producer-floor updates");
        assert!(work(&h.driver) <= before + 2);
        match report.result.unwrap() {
            FileDriverEvent::Workers { .. } => {}
            FileDriverEvent::ReviewApplied { receipt, .. } => {
                assert_eq!(receipt.policy.control.decision.consequence, Consequence::Continue);
                finished = true; break;
            }
            other => panic!("unexpected file-driven review result: {other:?}"),
        }
    }
    assert!(finished);
    assert!(run(&h.driver).history().len() > 1);
    let report = h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None);
    assert_eq!(report.observations.len(), 1);
    assert!(matches!(report.result.unwrap(), FileDriverEvent::AwaitingHuman { .. }));
    let human = approve_current(&mut h);
    let calls = source.status().read_attempts;
    let report = h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), Some(&human));
    assert_eq!(report.observations.len(), 2);
    assert_eq!(source.status().read_attempts, calls + 2);
    assert!(matches!(report.result.unwrap(), FileDriverEvent::Dispatched { .. }));
    let report = h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None);
    assert_eq!(report.observations.len(), 1);
    assert!(matches!(report.result.unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
        if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
    let calls = source.status().read_attempts;
    std::fs::remove_file(path).unwrap();
    let report = h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None);
    assert!(report.observations.is_empty() && report.source_updates.is_empty());
    assert_eq!(source.status().read_attempts, calls);
    assert!(matches!(report.result.unwrap(), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(h.port.poll(&h.ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    drop(h.driver);
    let stored = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(stored.payload, b"visible"); assert_eq!(stored.executions, 1);
    assert_eq!(stored.control.ledger.charged, 16);
}

#[test]
fn computed_current_file_loss_context_scope_and_version_refuse_before_more_scoring() {
    for defect in 0..6 {
        let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 1);
        native_tick(&mut h.driver, None).unwrap(); native_tick(&mut h.driver, None).unwrap();
        let before = work(&h.driver); assert!(before > 0);
        let (path, mut source) = policy_file(&root);
        let original = policy_observation(1);
        let mut identity = original.identity(); let mut state = original.snapshot().clone();
        let mut contexts = original.contexts().clone();
        match defect {
            0 => { std::fs::remove_file(&path).unwrap(); }
            1 => { contexts.insert("alpha".to_owned(), b"helper must see these bytes".to_vec()); }
            2 => { state.complete = false; }
            3 => {
                identity.scope.tenant += 1;
                source = FileEvidenceSource::new(&path, 99, identity.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
            }
            4 => { contexts.remove("beta"); }
            _ => {
                source.read().unwrap();
                state.values.insert(7, b"changed without a version".to_vec());
            }
        }
        if defect != 0 { replace(&path, &EvidenceSnapshot::new(identity, state, contexts).unwrap()); }
        let report = h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None);
        assert!(report.result.is_err()); assert_eq!(report.observations.len(), 1);
        assert!(report.source_updates.is_empty());
        match defect {
            0 => assert_eq!(report.observations[0], Err(EvidenceError::Io(std::io::ErrorKind::NotFound))),
            2 => assert_eq!(report.observations[0], Err(EvidenceError::Data(Error::Incomplete))),
            _ => assert_eq!(report.observations[0], Err(EvidenceError::Data(Error::Binding))),
        }
        assert_eq!(work(&h.driver), before);
        assert_eq!(run(&h.driver).status(), FileLearnedProbeStatus::Cancelled);
        assert_eq!(h.driver.phase(), FileDriverPhase::Idle);
        let input = run(&h.driver).input().clone();
        let mut host = h.driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        assert!(host.authorize(revision, 1, &input, snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn computed_current_loop_changed_policy_at_final_publication_reaches_original_nonexecution() {
    let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 0);
    complete_native(&mut h); let human = approve_current(&mut h);
    let (path, mut source) = policy_file(&root);
    assert!(matches!(h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), Some(&human)).result.unwrap(),
        FileDriverEvent::Dispatched { .. }));
    let records = run(&h.driver).records().clone();
    let original = policy_observation(2); let mut state = original.snapshot().clone();
    state.values.insert(7, b"not approved".to_vec());
    replace(&path, &EvidenceSnapshot::new(original.identity(), state, original.contexts().clone()).unwrap());
    let report = h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None);
    assert_eq!(report.observations, vec![Ok(original.identity())]);
    assert!(matches!(report.result.unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
        if !matches!(publication.outcome, EndpointOutcome::Executed { .. })));
    std::fs::remove_file(path).unwrap();
    let reads = source.status().read_attempts;
    let report = h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None);
    assert!(report.observations.is_empty()); assert_eq!(source.status().read_attempts, reads);
    assert!(matches!(report.result.unwrap(), FileDriverEvent::Reconciled { .. }));
    assert_eq!(run(&h.driver).records(), &records);
    assert!(matches!(h.port.poll(&h.ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
    assert_eq!(h.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn computed_current_source_loss_refuses_before_policy_callback_or_file_read() {
    for file in [false, true] {
        let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 0);
        native_tick(&mut h.driver, None).unwrap(); native_tick(&mut h.driver, None).unwrap();
        let before = work(&h.driver);
        step(&mut h.driver.supervisor_mut().host_mut().unwrap());
        if file {
            let (_, mut source) = policy_file(&root);
            let report = h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), None);
            assert!(report.result.is_err() && report.observations.is_empty());
            assert_eq!(source.status().read_attempts, 0);
        } else {
            assert!(h.driver.step_computed(|| ElapsedTick(1), || panic!("stale source before observation"), None).is_err());
        }
        assert_eq!(work(&h.driver), before);
        assert_ne!(run(&h.driver).status(), FileLearnedProbeStatus::Running);
        assert_eq!(h.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn computed_current_observation_unwinds_close_custody_without_erasing_refinement_or_charging_effects() {
    for refined in [false, true] {
        for cut in 0..3 {
            let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 1);
            native_tick(&mut h.driver, None).unwrap(); native_tick(&mut h.driver, None).unwrap();
            if refined {
                for _ in 0..64 {
                    if run(&h.driver).input_revision() > 1 { break; }
                    native_tick(&mut h.driver, None).unwrap();
                }
                assert!(run(&h.driver).input_revision() > 1);
                assert_eq!(run(&h.driver).status(), FileLearnedProbeStatus::Running);
            }
            let work = run(&h.driver).records().clone(); let rounds = run(&h.driver).history().len();
            let bytes = {
                let host = h.driver.supervisor().host().unwrap();
                host.store.read(host.profile.delivery.limits.bytes).unwrap()
            };
            let clocks = Cell::new(0);
            let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                h.driver.step_computed(|| {
                    let call = clocks.get(); clocks.set(call + 1);
                    if (cut == 0 && call == 0) || (cut == 2 && call == 1) { panic!("selected clock interruption"); }
                    ElapsedTick(1)
                }, || {
                    if cut == 1 { panic!("selected policy interruption"); }
                    Ok(snapshot())
                }, None)
            }));
            assert!(interrupted.is_err());
            assert_eq!(h.driver.phase(), FileDriverPhase::Idle);
            assert_eq!(run(&h.driver).status(), FileLearnedProbeStatus::Cancelled);
            assert_eq!(run(&h.driver).history().len(), rounds);
            for (key, record) in run(&h.driver).records() { assert_eq!(record.work, work[key].work); }
            let host = h.driver.supervisor().host().unwrap();
            assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
            assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
            drop(host);
            assert!(matches!(h.port.poll(&h.ticket), Knowledge::Pending { .. }));
            assert!(matches!(h.driver.step_computed(|| panic!("no live job"), || panic!("no live observer"), None).unwrap(),
                FileDriverEvent::Idle));
            let saved = h.driver.take_learned_probe_review().unwrap().unwrap();
            assert_eq!(saved.review().history().len(), rounds);
        }
    }
}

#[test]
fn computed_current_recovered_unknown_settlement_does_not_reopen_the_policy_file() {
    let root = Directory::new(); let config = config(); let mut h = setup(&root, &config, 0);
    complete_native(&mut h); let human = approve_current(&mut h);
    let (path, mut source) = policy_file(&root);
    assert!(matches!(h.driver.step_computed_from_policy_file(&mut source, || ElapsedTick(1), Some(&human)).result.unwrap(),
        FileDriverEvent::Dispatched { .. }));
    drop(h.driver); std::fs::remove_file(path).unwrap();
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    let (_, mut driver) = host.into_supervised_driver(); driver.resume_reconciliation(REQUEST).unwrap();
    let before = source.status().read_attempts;
    let report = driver.step_computed_from_policy_file(&mut source, || ElapsedTick(2), None);
    assert!(report.observations.is_empty() && report.source_updates.is_empty());
    assert_eq!(source.status().read_attempts, before);
    assert!(matches!(report.result.unwrap(), FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 84);
    assert!(driver.learned_probe_review().is_none());
}
