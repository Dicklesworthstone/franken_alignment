use super::*;
use std::cell::Cell;

fn approve_current(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer)
    -> super::super::super::super::FileHumanPermit
{
    let request = driver.request_learned_human_approval(1001, ElapsedTick(80), ElapsedTick(1)).unwrap();
    let mut host = driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

#[test]
fn original_current_inputs_drive_both_keys_and_publication_without_caller_packets() {
    let root = Directory::new(); let (port, ticket, mut driver, reviewer) = setup(&root, 1);
    complete(&mut driver);
    // Transferring the terminal report cannot transfer or disable the live gate.
    let report = driver.take_learned_probe_review().unwrap().unwrap();
    let calls = Cell::new(0);
    let mut policy = || { calls.set(calls.get() + 1); Ok(snapshot()) };
    assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1), &mut policy, None).unwrap(),
        FileDriverEvent::AwaitingHuman { request: 901 }));
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    let human = approve_current(&mut driver, &reviewer);
    assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1), &mut policy, Some(&human)).unwrap(),
        FileDriverEvent::Dispatched { request: 901, .. }));
    assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1), &mut policy, None).unwrap(),
        FileDriverEvent::PublicationChecked { publication, source_failure: None, .. }
            if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
    assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1),
        || panic!("reconciliation must not capture evidence"), None).unwrap(), FileDriverEvent::Reconciled { .. }));
    assert_eq!(calls.get(), 4); // waiting + authorization + dispatch + publication
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(report.review().status(), FileLearnedProbeStatus::Finished);
    assert!(report.review().history().len() > 1);
    let state = driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 1); assert_eq!(state.payload, b"visible");
}

#[test]
fn changed_source_before_dispatch_refuses_before_policy_callback_or_reservation() {
    let root = Directory::new(); let (_, _, mut driver, reviewer) = setup(&root, 0);
    complete(&mut driver); let human = approve_current(&mut driver, &reviewer);
    { let mut host = driver.supervisor_mut().host_mut().unwrap(); step(&mut host); }
    assert!(driver.request_learned_human_approval(1002, ElapsedTick(80), ElapsedTick(1)).is_err());
    assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1),
        || panic!("stale source cannot invoke the policy callback"), Some(&human)), Err(JournalError::Contract(_))));
    let state = driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 0); assert_eq!(state.control.ledger.available, 100);
}

#[test]
fn losing_policy_evidence_after_reservation_retains_charge_until_original_cancellation() {
    let root = Directory::new(); let (port, ticket, mut driver, reviewer) = setup(&root, 0);
    complete(&mut driver); let human = approve_current(&mut driver, &reviewer);
    let records = driver.learned_probe_review().unwrap().review().records().clone();
    let mut calls = 0;
    let result = driver.step_with_learned_evidence(|| ElapsedTick(1), || {
        calls += 1;
        if calls == 1 { Ok(snapshot()) } else { Err(Error::Incomplete) }
    }, Some(&human));
    assert!(matches!(result, Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(calls, 2);
    let state = driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 0); assert_eq!(state.control.ledger.available, 84);
    assert_eq!(driver.learned_probe_review().unwrap().review().records(), &records);
    driver.cancel_active().unwrap();
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
}

#[test]
fn final_publication_rechecks_source_and_routes_loss_through_original_nonexecution_gate() {
    let root = Directory::new(); let (port, ticket, mut driver, reviewer) = setup(&root, 0);
    complete(&mut driver); let human = approve_current(&mut driver, &reviewer);
    assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1), || Ok(snapshot()), Some(&human)).unwrap(),
        FileDriverEvent::Dispatched { .. }));
    { let mut host = driver.supervisor_mut().host_mut().unwrap(); step(&mut host); }
    let result = driver.step_with_learned_evidence(|| ElapsedTick(1),
        || panic!("lost source cannot be repaired by a supplied snapshot"), None).unwrap();
    assert!(matches!(result, FileDriverEvent::PublicationChecked { publication, source_failure: Some(_), .. }
        if !matches!(publication.outcome, EndpointOutcome::Executed { .. })));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1),
        || panic!("no evidence during settlement"), None).unwrap(), FileDriverEvent::Reconciled { .. }));
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::ConfirmedNotExecuted, .. }));
    assert_eq!(driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
}

#[test]
fn recovered_settlement_needs_no_current_plan_no_policy_capture_and_no_human_role() {
    for executed in [false, true] {
        let root = Directory::new(); let (_, _, mut driver, reviewer) = setup(&root, 0);
        complete(&mut driver); let human = approve_current(&mut driver, &reviewer);
        assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1), || Ok(snapshot()), Some(&human)).unwrap(),
            FileDriverEvent::Dispatched { .. }));
        if executed {
            assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1), || Ok(snapshot()), None).unwrap(),
                FileDriverEvent::PublicationChecked { publication, .. }
                    if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
        }
        drop(human); drop(reviewer); drop(driver);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config()).unwrap();
        assert!(host.learned_generation_inspection().unwrap().paused);
        let (_, mut driver) = host.into_supervised_driver();
        driver.resume_reconciliation(901).unwrap();
        let event = driver.step_with_learned_evidence(|| ElapsedTick(2),
            || panic!("recovered query cannot need a live learned source"), None).unwrap();
        if executed {
            assert!(matches!(event, FileDriverEvent::Reconciled {
                outcome: Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }), .. }));
        } else {
            assert!(matches!(event, FileDriverEvent::Reconciled { outcome: Reconciliation::AwaitingResolution, .. }));
        }
        let state = driver.supervisor().host().unwrap().inspect();
        assert_eq!(state.executions, u64::from(executed));
        assert_eq!(state.control.ledger.available, 84);
        assert!(driver.learned_probe_review().is_none());
    }
}

#[test]
fn launch_binds_the_exact_request_and_owner_before_any_durable_clock_or_begin() {
    for foreign in [false, true] {
        let root = Directory::new(); let other_root = Directory::new();
        let (mut host, _) = owner(&root, &config()); step(&mut host); step(&mut host);
        let (port, mut driver) = host.into_supervised_driver();
        let ticket = submit(&mut driver, &port, 901);
        let mut request = launch(&mut driver, 901, 0);
        let (mut other, _) = owner(&other_root, &config()); step(&mut other); step(&mut other);
        if foreign {
            let (_, sidecar) = propose(&mut other);
            request.sidecar = sidecar;
        } else { request.request = 902; }
        let (revision, bytes) = {
            let host = driver.supervisor().host().unwrap();
            (host.revision(), host.store.read(host.profile.delivery.limits.bytes).unwrap())
        };
        let result = driver.start_learned_probe_review(request, snapshot(), ElapsedTick(2));
        assert_eq!(result, Err(JournalError::Contract(if foreign { Error::Binding } else { Error::Missing })));
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.revision(), revision);
        assert_eq!(host.store.read(host.profile.delivery.limits.bytes).unwrap(), bytes);
        assert!(driver.learned_probe_review().is_none());
        assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 901 }));
    }
    let root = Directory::new(); let (_, _, mut driver, _) = setup(&root, 0);
    advance(&mut driver, 1).unwrap(); advance(&mut driver, 1).unwrap();
    assert!(driver.learned_probe_review().unwrap().review().records().values()
        .any(|record| record.work.evaluated_probes > 0));
}
