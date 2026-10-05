//! Failure after admission never substitutes a newer policy for an old judgment.
use super::*;
use crate::action::consequence::delivery::persistent::Reconciliation;

#[test]
fn new_policy_or_helper_context_during_review_withdraws_old_input_and_keeps_the_new_floor() {
    for context_injection in [false, true] {
        let mut s = native(b"allow");
        assert!(s.run.step_from_policy_file(&mut s.source, || ElapsedTick(1), None).result.is_ok());
        let polls = s.run.progress().polls;
        let (attempt, revision) = { let host = s.run.supervisor().host().unwrap(); let id = attempt(&host, 71);
            (id, host.input_revision(id).unwrap()) };
        let mut state = snapshot(); let mut contexts = capture(2).contexts().clone();
        if context_injection { contexts.insert("alpha".to_owned(), b"replace native evidence".to_vec()); }
        else { state.values.insert(7, b"no".to_vec()); }
        publish(&s.root.store().with_extension("policy"),
            &EvidenceSnapshot::new(capture(2).identity(), state, contexts).unwrap());
        let report = s.run.step_from_policy_file(&mut s.source, || ElapsedTick(1), None);
        assert!(report.result.is_err()); assert_eq!(report.source_updates.len(), 1);
        if context_injection { assert_eq!(report.source_updates[0], Err(FileSourceError::Refused(Error::Binding))); }
        else { assert_eq!(report.source_updates[0], Ok(capture(2).identity())); }
        assert_ne!(s.run.review().status(), NativeReviewStatus::Running);
        assert_eq!(s.run.progress().polls, polls, "no helper inference on invalidated evidence");
        let host = s.run.supervisor().host().unwrap();
        assert_eq!(host.file_source_status().unwrap().producer, Some(capture(2).identity()));
        assert!(host.input_revision(attempt).unwrap() > revision);
        assert!(host.machine.checked_learned_sidecar(attempt).is_err());
        assert_eq!(host.inspect().executions, 0); drop(host);
        // Repair policy is explicit, not a restoration of the old numerical
        // judgment or a restart of its work budget under the old request.
        publish(&s.root.store().with_extension("policy"), &capture(3));
        s.run.supervisor_mut().prepare_learned_policy_intake(&mut s.source, || ElapsedTick(2)).result.unwrap();
        assert!(s.run.request_human_approval(1001, ElapsedTick(80), ElapsedTick(2)).is_err());
        assert!(s.run.supervisor().host().unwrap().machine.checked_learned_sidecar(attempt).is_err());
        assert_eq!(s.run.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn file_loss_after_automatic_reservation_refuses_dispatch_without_refunding_reserved_rights() {
    let mut s = native(b"allow"); complete(&mut s.run, &mut s.source, 1);
    let human = approve(&mut s.run, &s.reviewer, &mut s.source, 1001, 1);
    let path = s.root.store().with_extension("policy"); let mut clocks = 0;
    let report = s.run.step_from_policy_file(&mut s.source, || {
        clocks += 1;
        // Original Ready: outer time, first read-start, post-read time,
        // automatic reservation, second read-start. The file vanishes HERE.
        if clocks == 4 { std::fs::rename(&path, path.with_extension("unavailable")).unwrap(); }
        ElapsedTick(1)
    }, Some(&human));
    assert!(report.result.is_err()); assert_eq!(report.source_updates.len(), 2);
    assert_eq!(report.source_updates[0], Ok(capture(1).identity()));
    assert!(matches!(report.source_updates[1], Err(FileSourceError::Read { withdrawal: None, .. })));
    let host = s.run.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 98);
    assert!(host.machine.broker.capture_policy_state().is_err()); drop(host);
    publish(&path, &capture(1));
    s.run.supervisor_mut().prepare_learned_policy_intake(&mut s.source, || ElapsedTick(2)).result.unwrap();
    assert!(s.run.step_from_policy_file(&mut s.source, || ElapsedTick(2), Some(&human)).result.is_err());
    assert_eq!(s.run.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(s.run.supervisor().host().unwrap().inspect().control.ledger.available, 98);
}

#[test]
fn policy_loss_after_dispatch_cannot_publish_generated_output_using_the_old_keys() {
    let mut s = native(b"allow"); complete(&mut s.run, &mut s.source, 1);
    let human = approve(&mut s.run, &s.reviewer, &mut s.source, 1001, 1);
    assert!(matches!(tick(&mut s.run, &mut s.source, Some(&human), 1), FileDriverEvent::Dispatched { .. }));
    let path = s.root.store().with_extension("policy");
    std::fs::rename(&path, path.with_extension("unavailable")).unwrap();
    let report = s.run.step_from_policy_file(&mut s.source, || ElapsedTick(1), None);
    assert!(matches!(report.source_updates.as_slice(), [Err(FileSourceError::Read { withdrawal: None, .. })]));
    assert!(matches!(report.result.unwrap(), FileNativeDriverEvent::Driver(FileDriverEvent::PublicationChecked {
        source_failure: Some(_), .. })));
    let host = s.run.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 98); drop(host);
    let stored = FileOversight::read_publication_with_learned_generation(s.root.store(), &profile(), &s.config).unwrap();
    assert_eq!(stored.payload, b"initial"); assert_eq!(stored.executions, 0);
}

#[test]
fn unknown_dispatch_recovery_retains_source_floor_and_charge_without_policy_reads_or_model_resume() {
    let mut s = native(b"allow"); complete(&mut s.run, &mut s.source, 1);
    let human = approve(&mut s.run, &s.reviewer, &mut s.source, 1001, 1);
    assert!(matches!(tick(&mut s.run, &mut s.source, Some(&human), 1), FileDriverEvent::Dispatched { .. }));
    drop(s.run); drop(s.reviewer); drop(human);
    let (host, _) = FileOversight::open_with_learned_generation(s.root.store(), profile(), &s.config).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture(1).identity()));
    assert!(host.machine.broker.capture_policy_state().is_err());
    let (port, supervisor) = host.into_learned_text_actor_gateway().unwrap();
    let ticket = port.submit(71, intent()).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
    let mut driver = FileSupervisedDriver::new(supervisor);
    driver.resume_reconciliation(71).unwrap();
    let mut fresh_reader = reader(&s.root);
    let report = driver.step_computed_from_policy_file(&mut fresh_reader, || ElapsedTick(2), None);
    assert!(report.observations.is_empty() && report.source_updates.is_empty());
    assert!(matches!(report.result.unwrap(), FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
    assert_eq!(fresh_reader.status().read_attempts, 0);
    let host = driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 98);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(host.machine.broker.capture_policy_state().is_err());
    assert!(matches!(port.poll(&ticket), Knowledge::Unknown { .. }));
}

#[test]
fn every_source_storage_fault_stops_native_review_without_fabricating_an_observation() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let mut s = native(b"allow");
        let polls = s.run.progress().polls;
        s.run.supervisor().host().unwrap().store.fail_once(barrier);
        let report = s.run.step_from_policy_file(&mut s.source, || ElapsedTick(1), None);
        assert!(report.result.is_err()); assert!(report.observations.is_empty());
        assert!(matches!(report.source_updates.as_slice(), [Err(FileSourceError::Journal(_))]));
        assert_eq!(s.run.progress().polls, polls);
        assert_ne!(s.run.review().status(), NativeReviewStatus::Running);
        assert!(s.run.supervisor().host().unwrap().storage_failure().is_some());
        drop(s.run); drop(s.reviewer);
        let (host, _) = FileOversight::open_with_learned_generation(s.root.store(), profile(), &s.config).unwrap();
        assert!(host.policy_only_file_source_required());
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(host.machine.broker.capture_policy_state().is_err());
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    }
}
