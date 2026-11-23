//! Explicit operator continuation of the original isolated helper schedule.
use super::*;
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorPort, FileActorTicket, FileConveneOutcome, FileConveneReceipt,
    FileConveneRequest, FileSupervisorConveneGrant, FileSupervisorConvenePort,
};
use fa_reference::action::consequence::oversight::evidence_source::FileEvidenceSource;

struct Case {
    root: Directory,
    actor: FileActorPort<Host>,
    ticket: FileActorTicket<Host>,
    reviewer: FileHumanReviewer,
    run: FileProcessSupervisedDriver,
    source: FileEvidenceSource,
}

impl Case {
    fn held(modes: &[&str], operations: usize) -> (Self, FileSupervisorConvenePort, FileSupervisorConveneGrant) {
        let (root, actor, ticket, reviewer, driver, launch) = setup(modes);
        let mut source = policy_source(&root);
        let mut run = driver
            .start_process_learned_sequence(launch, snapshot(), || ElapsedTick(1))
            .unwrap();
        let (port, grant) = run.supervisor().convene_port(32, operations).unwrap();
        finish(&mut run, &mut source, 1);
        assert_hold(&run);
        (Self { root, actor, ticket, reviewer, run, source }, port, grant)
    }

    fn image(&self) -> Vec<u8> {
        std::fs::read(self.root.store().join("delivery.bin")).unwrap()
    }

    fn continue_review(&mut self) {
        let ticket = self.run.take_pending_convene().unwrap().unwrap();
        self.run
            .convene_from_policy_file(&mut self.source, ticket, || ElapsedTick(1))
            .result
            .unwrap();
        finish(&mut self.run, &mut self.source, 1);
    }
}

fn assert_hold(run: &FileProcessSupervisedDriver) {
    let FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. } =
        run.review().history().last().unwrap()
    else {
        panic!("the real process cohort did not apply its original review");
    };
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert_eq!(run.progress().phase, FileDriverPhase::Idle);
}

fn command(port: &FileSupervisorConvenePort, round: u64) -> FileConveneRequest {
    let Knowledge::Known { value, .. } = port.next(41, round).unwrap() else {
        panic!("original held request did not return its bounded plan");
    };
    value.convene_request().expect("original next process slot is eligible")
}

fn receipt(port: &FileSupervisorConvenePort, request: FileConveneRequest) -> FileConveneReceipt {
    let Knowledge::Known { value, .. } = port.convene(request).unwrap() else {
        panic!("independent operator did not receive its original receipt");
    };
    value
}

fn assert_blocked(port: &FileSupervisorConvenePort, round: u64) {
    let Knowledge::Known { value, .. } = port.next(41, round).unwrap() else {
        panic!("expected an observed original request");
    };
    assert!(!value.can_convene());
    assert!(value.block_reason().is_some());
    assert!(value.convene_request().is_none());
}

#[test]
fn process_hold_preserves_the_packet_and_original_spending_until_fresh_two_key_publication() {
    let (mut case, port, grant) = Case::held(&["hold", "allow"], 1);
    let input = case.run.review().input().clone();
    let action = case.run.supervisor().host().unwrap().request_action(41).unwrap().clone();
    let numerical = case.run.supervisor().host().unwrap()
        .learned_generation_inspection().unwrap().numerical;
    let first = case.run.review().records()[&101].clone();
    let archive = case.run.review().history()[0].archive().clone();
    let before = case.run.progress();
    let image = case.image();
    let reads = case.source.status();
    assert!(first.processes["reviewer"].exit.is_some());
    assert_eq!(case.run.review().records()[&102].admission, None);
    assert_eq!(case.run.held_successor_round().unwrap().round, 102);
    assert!(case.run.has_fixed_successor());
    for _ in 0..3 {
        assert!(matches!(
            case.run.step_from_policy_file(
                &mut case.source,
                || panic!("idle held custody must not read the clock"),
                None,
            ).result.unwrap(),
            FileProcessDriverEvent::Driver(FileDriverEvent::Idle)
        ));
    }
    assert_eq!(case.run.progress(), before);
    assert_eq!(case.source.status(), reads);
    assert_eq!(case.image(), image);
    let selected = command(&port, 102);
    let pending = receipt(&port, selected);
    assert_eq!(pending.outcome(), FileConveneOutcome::Pending);
    assert_eq!(receipt(&port.clone(), selected), pending);
    assert_eq!(grant.remaining_operations(), 0);
    assert!(case.run.has_fixed_successor(), "the last accepted operation remains executable");
    let ticket = case.run.take_pending_convene().unwrap().unwrap();
    assert!(case.run.take_pending_convene().unwrap().is_none());
    assert!(case.run.has_fixed_successor(), "taking custody cannot discard the accepted operation");
    case.run
        .convene_from_policy_file(&mut case.source, ticket, || ElapsedTick(1))
        .result
        .unwrap();
    assert_eq!(case.source.status().read_attempts, reads.read_attempts + 1);
    assert_eq!(case.run.progress().polls, before.polls + 1);
    assert_eq!(case.run.progress().admitted_starts, before.admitted_starts);
    assert_eq!(case.run.review().input(), &input);
    assert_eq!(case.run.review().records()[&101], first);
    assert_eq!(case.run.review().records()[&102].admission, Some(HelperRoundAdmission::Committed));
    assert_eq!(receipt(&port, selected).outcome(), FileConveneOutcome::Opened);
    finish(&mut case.run, &mut case.source, 1);
    assert_eq!(case.run.review().history().len(), 2);
    assert_eq!(case.run.review().history()[0].archive(), &archive);
    assert_eq!(case.run.review().records()[&101], first);
    assert!(case.run.progress().polls > before.polls);
    assert_eq!(case.run.progress().admitted_starts, 2);
    assert_eq!(case.run.review().input(), &input);
    assert_eq!(
        std::fs::read(case.root.round(101).join("observed")).unwrap(),
        std::fs::read(case.root.round(102).join("observed")).unwrap()
    );
    assert_eq!(case.run.supervisor().host().unwrap()
        .learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(case.run.supervisor().host().unwrap().request_action(41).unwrap(), &action);
    assert_eq!(case.run.supervisor().host().unwrap().inspect().executions, 0);
    assert!(matches!(
        case.run.step_from_policy_file(&mut case.source, || ElapsedTick(1), None).result.unwrap(),
        FileProcessDriverEvent::Driver(FileDriverEvent::AwaitingHuman { request: 41 })
    ));
    let request = case.run.request_human_approval_from_policy_file(
        &mut case.source, 1001, ElapsedTick(40), || ElapsedTick(1),
    ).result.unwrap();
    let human = {
        let mut host = case.run.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        case.reviewer.approve(&mut host, revision, &request).unwrap()
    };
    assert!(matches!(
        case.run.step_from_policy_file(&mut case.source, || ElapsedTick(1), Some(&human)).result.unwrap(),
        FileProcessDriverEvent::Driver(FileDriverEvent::Dispatched { request: 41, .. })
    ));
    assert!(matches!(
        case.run.step_from_policy_file(&mut case.source, || ElapsedTick(2), None).result.unwrap(),
        FileProcessDriverEvent::Driver(FileDriverEvent::PublicationChecked { .. })
    ));
    assert!(matches!(
        case.run.step_from_policy_file(&mut case.source, || ElapsedTick(2), None).result.unwrap(),
        FileProcessDriverEvent::Driver(FileDriverEvent::Reconciled { .. })
    ));
    assert!(matches!(case.actor.poll(&case.ticket), Knowledge::Known {
        value: ActorOutcome::Executed, ..
    }));
    let state = case.run.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 1);
    assert_eq!(state.control.ledger.charged, 16);
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(case.run.supervisor().host().unwrap()
        .learned_generation_inspection().unwrap().numerical, numerical);
    let image = case.image();
    assert_eq!(receipt(&port, selected).outcome(), FileConveneOutcome::Opened);
    assert_eq!(case.image(), image);
    let handoff = case.run.into_handoff().unwrap();
    assert!(handoff.review.all_reaped());
}

#[test]
fn process_hold_cannot_select_another_roster_or_reuse_an_abandoned_fixed_slot() {
    let (mut case, port, grant) = Case::held(&["hold", "allow"], 2);
    let before = case.run.progress();
    let image = case.image();
    for round in [101, 103, 999] {
        assert_blocked(&port, round);
    }
    assert_eq!(grant.remaining_operations(), 2);
    assert_eq!(case.run.progress(), before);
    assert_eq!(case.image(), image);
    let selected = command(&port, 102);
    receipt(&port, selected);
    drop(case.run.take_pending_convene().unwrap().unwrap());
    assert_eq!(receipt(&port, selected).outcome(), FileConveneOutcome::Abandoned);
    assert_eq!(grant.remaining_operations(), 1);
    assert!(!case.run.has_fixed_successor());
    assert!(case.run.take_pending_convene().unwrap().is_none());
    assert_eq!(case.run.review().records()[&102].admission, None);
    assert!(!case.root.round(102).join("started").exists());
    assert_eq!(case.run.progress(), before);
    assert_eq!(case.image(), image);
    case.run.request_stop_helpers();
    cleanup(&mut case.run);
}

#[test]
fn process_successor_rechecks_current_source_and_never_retries_a_failed_launch() {
    for changed in [false, true] {
        let (mut case, port, grant) = Case::held(&["hold", "allow"], 1);
        let selected = command(&port, 102);
        receipt(&port, selected);
        let ticket = case.run.take_pending_convene().unwrap().unwrap();
        let numerical = case.run.supervisor().host().unwrap()
            .learned_generation_inspection().unwrap().numerical;
        let input = case.run.review().input().clone();
        let first = case.run.review().records()[&101].clone();
        let before = case.run.progress();
        if changed {
            let mut replacement = snapshot();
            replacement.semantic_epoch += 1;
            use fa_reference::action::consequence::oversight::evidence_source::{
                EvidenceIdentity, EvidenceSnapshot,
            };
            let document = EvidenceSnapshot::new(
                EvidenceIdentity { source: 91, generation: 2, scope: profile().delivery.scope },
                replacement,
                BTreeMap::from([("reviewer".into(), Vec::new())]),
            ).unwrap();
            std::fs::write(case.root.0.join("policy.json"), document.encode()).unwrap();
        } else {
            std::fs::remove_file(case.root.0.join("policy.json")).unwrap();
        }
        let reads = case.source.status().read_attempts;
        assert!(case.run.convene_from_policy_file(
            &mut case.source, ticket, || ElapsedTick(1),
        ).result.is_err());
        assert_eq!(case.source.status().read_attempts, reads + 1);
        assert_eq!(receipt(&port, selected).outcome(), FileConveneOutcome::Failed);
        assert_eq!(grant.remaining_operations(), 0);
        assert_eq!(case.run.review().records()[&101], first);
        assert_eq!(case.run.review().records()[&102].admission,
            if changed { Some(HelperRoundAdmission::NotStarted) } else { None });
        assert!(!case.root.round(102).join("started").exists());
        // Missing source refuses before launch; a complete changed semantic
        // snapshot spends the original launch poll before Begin refuses it.
        assert_eq!(case.run.progress().polls, before.polls + usize::from(changed));
        assert_eq!(case.run.progress().admitted_starts, before.admitted_starts);
        assert_eq!(case.run.review().input(), &input);
        assert_eq!(case.run.supervisor().host().unwrap()
            .learned_generation_inspection().unwrap().numerical, numerical);
        case.source = policy_source(&case.root);
        assert!(!case.run.has_fixed_successor());
        assert!(case.run.take_pending_convene().unwrap().is_none());
        assert!(case.run.request_human_approval(1001, ElapsedTick(40), ElapsedTick(1)).is_err());
        assert_eq!(case.run.supervisor().host().unwrap().inspect().executions, 0);
        cleanup(&mut case.run);
    }
}

#[test]
fn stale_predecessor_stop_cancel_or_numerical_change_refuses_before_source_or_clock() {
    for change in 0..4 {
        let (mut case, port, grant) = Case::held(&["hold", "allow"], 1);
        let selected = command(&port, 102);
        receipt(&port, selected);
        let ticket = case.run.take_pending_convene().unwrap().unwrap();
        match change {
            0 => {
                let mut host = case.run.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                host.observe_time(revision, ElapsedTick(2)).unwrap();
            }
            1 => {
                let mut host = case.run.supervisor_mut().host_mut().unwrap();
                let state = host.inspect();
                host.request_stop(state.revision, StopRequest {
                    operation: 7001,
                    expected_control_sequence: state.control.sequence,
                    expected_authority_epoch: state.control.ledger.epoch,
                }).unwrap();
            }
            2 => { case.actor.cancel(&case.ticket).unwrap(); }
            _ => step_source(&mut case.run.supervisor_mut().host_mut().unwrap()),
        }
        let image = case.image();
        let reads = case.source.status();
        let before = case.run.progress();
        assert!(case.run.convene_from_policy_file(
            &mut case.source, ticket,
            || panic!("ineligible original custody cannot sample a clock"),
        ).result.is_err());
        assert_eq!(case.source.status(), reads);
        assert_eq!(case.image(), image);
        assert_eq!(case.run.progress(), before);
        assert_eq!(grant.remaining_operations(), 0);
        assert_ne!(receipt(&port, selected).outcome(), FileConveneOutcome::Opened);
        assert_eq!(case.run.review().records()[&102].admission, None);
        assert_eq!(case.run.supervisor().host().unwrap().inspect().executions, 0);
        case.run.request_stop_helpers();
        cleanup(&mut case.run);
    }
}

#[test]
fn process_successor_keeps_its_original_window_and_refuses_unprovisioned_capacity() {
    let (mut one, port, grant) = Case::held(&["hold"], 1);
    assert!(one.run.held_successor_round().is_none());
    assert!(!one.run.has_fixed_successor());
    assert_blocked(&port, 102);
    assert_eq!(grant.remaining_operations(), 1);
    assert!(one.run.take_pending_convene().unwrap().is_none());
    assert_eq!(one.run.progress().admitted_starts, 1);

    let (mut expired, port, grant) = Case::held(&["hold", "allow"], 1);
    let next = expired.run.held_successor_round().unwrap();
    let selected = command(&port, next.round);
    receipt(&port, selected);
    let ticket = expired.run.take_pending_convene().unwrap().unwrap();
    let before = expired.run.progress();
    assert!(expired.run.convene_from_policy_file(
        &mut expired.source, ticket, || next.window.commit_by,
    ).result.is_err());
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(receipt(&port, selected).outcome(), FileConveneOutcome::Failed);
    assert_eq!(expired.run.review().records()[&102].admission, None);
    assert!(!expired.root.round(102).join("started").exists());
    assert_eq!(expired.run.progress().admitted_starts, before.admitted_starts);
    assert_eq!(expired.run.progress().polls, before.polls);
    assert!(!expired.run.has_fixed_successor());
    cleanup(&mut expired.run);
}

#[test]
fn later_process_hold_requires_another_operation_without_resetting_work() {
    let (mut case, port, grant) = Case::held(&["hold", "hold", "allow"], 2);
    let before = case.run.progress();
    receipt(&port, command(&port, 102));
    case.continue_review();
    assert_hold(&case.run);
    assert_eq!(case.run.review().history().len(), 2);
    assert_eq!(case.run.held_successor_round().unwrap().round, 103);
    assert_eq!(grant.remaining_operations(), 1);
    let polls = case.run.progress().polls;
    assert!(polls > before.polls);
    receipt(&port, command(&port, 103));
    case.continue_review();
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(case.run.review().history().len(), 3);
    assert_eq!(case.run.progress().admitted_starts, before.admitted_starts);
    assert!(case.run.progress().polls > polls);
    assert!(case.run.held_successor_round().is_none());
    assert!(!case.run.has_fixed_successor());
    assert_eq!(case.run.progress().phase, FileDriverPhase::AwaitingDispatch { request: 41 });
    assert_eq!(case.run.supervisor().host().unwrap().inspect().executions, 0);
    assert!(case.run.all_helpers_reaped());
}

#[test]
fn process_handoff_withdraws_taken_custody_from_the_generic_launcher() {
    use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::FileReviewLaunch;

    let (case, port, grant) = Case::held(&["hold", "allow"], 1);
    let selected = command(&port, 102);
    receipt(&port, selected);
    let ticket = case.run.take_pending_convene().unwrap().unwrap();
    let next = case.run.held_successor_round().unwrap();
    let revision = case.run.progress().input_revision;
    let image = case.image();
    let mut source = case.source;
    let reads = source.status();
    let mut handoff = case.run.into_handoff().unwrap();
    assert!(handoff.review.all_reaped());
    let launch = FileReviewLaunch::<UnixStream> {
        request: 41, round: next.round, window: next.window,
        expected_input_revision: revision,
        workers: BTreeMap::new(), limits: HelperLimits::default(),
    };
    assert!(handoff.driver.start_file_convened_review(
        &mut source, ticket, launch,
        || panic!("a process ticket cannot borrow a generic review owner"),
    ).is_err());
    assert_eq!(source.status(), reads);
    assert_eq!(std::fs::read(case.root.store().join("delivery.bin")).unwrap(), image);
    assert_eq!(grant.remaining_operations(), 0);
    assert_ne!(receipt(&port, selected).outcome(), FileConveneOutcome::Opened);
    assert_eq!(handoff.review.records()[&102].admission, None);
    assert_eq!(handoff.driver.supervisor().host().unwrap().inspect().executions, 0);
}
