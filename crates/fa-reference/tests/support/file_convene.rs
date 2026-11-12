#![allow(dead_code, unused_imports)]
//! Actual file evidence and socket congress rounds on the original owner.
//! Controlled ballots exercise authority plumbing, not helper reliability.
#[path = "file_evidence_driver.rs"]
pub mod evidence;

pub use evidence::{FileRig, document, driver, replace};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{
    FileDriverEvent, FileDriverPhase, evidence::FileReviewLaunch,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileConveneReceipt, FileConveneRequest, FileSupervisorConvenePort,
};
use fa_reference::action::consequence::oversight::actor::Knowledge;
use fa_reference::action::consequence::oversight::evidence_source::EvidenceIdentity;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use std::os::unix::net::UnixStream;

pub const REQUEST: u64 = 1;
pub const HELD_ROUND: u64 = 101;
pub const NEXT_ROUND: u64 = 102;

pub fn held() -> FileRig {
    let mut file = FileRig::new();
    file.start(HELD_ROUND);
    // Both explicit abstentions produce HoldEffect under the original policy;
    // two Hold votes in this fixture would instead suspend the run.
    let report = file.finish(Verdict::Abstain);
    let FileDriverEvent::ReviewApplied { receipt, .. } = report.result.unwrap() else {
        panic!("original congress did not finish its held round");
    };
    assert_eq!(
        receipt.policy.control.decision.consequence,
        Consequence::HoldEffect
    );
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    let state = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(state.executions, 0);
    file
}

pub fn image(file: &FileRig) -> Vec<u8> {
    std::fs::read(file.rig.root.store().join("delivery.bin")).unwrap()
}

pub fn command(port: &FileSupervisorConvenePort, round: u64) -> FileConveneRequest {
    let Knowledge::Known { value, basis } = port.next(REQUEST, round).unwrap() else {
        panic!("expected original held-review plan");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.request(), REQUEST);
    assert_eq!(value.round(), round);
    assert!(value.can_convene(), "{:?}", value.block_reason());
    value
        .convene_request()
        .expect("admitted plan carries its exact predecessor")
}

pub fn receipt(
    port: &FileSupervisorConvenePort,
    request: &FileConveneRequest,
) -> FileConveneReceipt {
    let Knowledge::Known { value, .. } = port.convene(*request).unwrap() else {
        panic!("expected original convening operation");
    };
    assert_eq!(value.request(), request);
    value
}

pub fn new_launch(file: &mut FileRig, round: u64) -> FileReviewLaunch<UnixStream> {
    let epoch = file
        .rig
        .driver
        .supervisor()
        .host()
        .unwrap()
        .request_action(REQUEST)
        .unwrap()
        .spec()
        .policy_epoch;
    let (workers, clients) = driver::sockets(epoch);
    file.rig.clients = clients;
    evidence::launch(&file.rig, round, workers)
}

pub fn launch(file: &mut FileRig, request: &FileConveneRequest) -> EvidenceIdentity {
    let ticket = file
        .rig
        .driver
        .take_pending_convene(REQUEST)
        .unwrap()
        .expect("explicit pending operation");
    assert_eq!(ticket.request(), REQUEST);
    assert_eq!(ticket.round(), request.round());
    assert_eq!(ticket.expected(), request.expected());
    let launch = new_launch(file, request.round());
    file.rig
        .driver
        .start_file_convened_review(&mut file.source, ticket, launch, || ElapsedTick(1))
        .unwrap()
}

pub fn finish_continue(file: &mut FileRig) {
    let FileDriverEvent::ReviewApplied { receipt, .. } =
        file.finish(Verdict::Allow).result.unwrap()
    else {
        panic!("fresh independent congress did not finish");
    };
    assert_eq!(
        receipt.policy.control.decision.consequence,
        Consequence::Continue
    );
    assert_eq!(
        file.rig.driver.phase(),
        FileDriverPhase::AwaitingDispatch { request: REQUEST }
    );
}

pub fn publish(file: &mut FileRig) {
    let waiting = file
        .rig
        .driver
        .step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(
        waiting.result,
        Ok(FileDriverEvent::AwaitingHuman { request: REQUEST })
    ));
    assert_eq!(
        file.rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .executions,
        0
    );
    let request = file
        .rig
        .driver
        .request_human_approval_from_file(&mut file.source, 1001, ElapsedTick(31), || {
            ElapsedTick(1)
        })
        .result
        .unwrap();
    let human = {
        let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        file.rig
            .reviewer
            .approve(&mut host, revision, &request)
            .unwrap()
    };
    assert!(matches!(
        file.rig
            .driver
            .step_from_file(&mut file.source, || ElapsedTick(1), Some(&human))
            .result,
        Ok(FileDriverEvent::Dispatched {
            request: REQUEST,
            attempt: 1
        })
    ));
    let event = file
        .rig
        .driver
        .step_from_file(&mut file.source, || ElapsedTick(1), None)
        .result
        .unwrap();
    let FileDriverEvent::PublicationChecked {
        publication,
        source_failure,
        ..
    } = event
    else {
        panic!("original checked publication missing: {event:?}");
    };
    assert_eq!(source_failure, None);
    assert!(matches!(
        publication.outcome,
        EndpointOutcome::Executed { .. }
    ));
    assert!(matches!(
        file.rig
            .driver
            .step_from_file(&mut file.source, || ElapsedTick(1), None)
            .result,
        Ok(FileDriverEvent::Reconciled {
            request: REQUEST,
            outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }),
        })
    ));
    let state = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.executions, 1);
    assert_eq!(state.payload, b"publication");
    assert_eq!(state.control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.control.ledger.charged, 16);
}
