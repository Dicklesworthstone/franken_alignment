//! An explicit durable liability disposition releases only the local driver job.
//! Real socket review and two independent publication keys remain required.
#![cfg(unix)]
#[path = "support/file_driver.rs"]
mod fixture;
use fixture::*;

use fa_reference::Error;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, Reconciliation,
    observed::driver::{FileDriverEvent, FileDriverPhase},
    requests::actor::{
        FileAbandonmentRequest, FileSupervisorAbandonmentGrant,
        FileSupervisorAbandonmentPort, MAX_ABANDONMENT_REPLAY_EVENTS,
    },
};
use fa_reference::action::consequence::oversight::actor::{
    ActorOutcome, Knowledge, UnknownReason,
};
use fa_reference::action::{ActionState, ElapsedTick};

fn original_dispatch(rig: &mut Rig, request: u64, human_key: u64) -> u64 {
    rig.reviewed(request);
    let human = rig.human(human_key, 30);
    match rig.step(Some(&human)) {
        FileDriverEvent::Dispatched { request: actual, attempt } => {
            assert_eq!(actual, request);
            attempt
        }
        event => panic!("missing original two-key dispatch: {event:?}"),
    }
}

fn make_unknown(rig: &mut Rig, attempt: u64) {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    assert_eq!(
        host.reconcile(revision, attempt).unwrap(),
        Reconciliation::AwaitingResolution
    );
}

fn planned(
    rig: &Rig,
    request: u64,
    operation: u64,
) -> (
    FileSupervisorAbandonmentPort,
    FileSupervisorAbandonmentGrant,
    FileAbandonmentRequest,
) {
    let (port, grant) = rig.driver.supervisor()
        .abandonment_port(2, 1, MAX_ABANDONMENT_REPLAY_EVENTS).unwrap();
    let Knowledge::Known { value, basis } = port.next(request, operation).unwrap() else {
        panic!("original Unknown plan missing");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.stage(), ActionState::Unknown);
    (port, grant, value.request().unwrap())
}

#[test]
fn explicit_abandonment_retires_only_its_job_and_same_driver_publishes_another_request() {
    let mut rig = Rig::new();
    let first = rig.submit(90);
    let first_attempt = original_dispatch(&mut rig, 90, 1001);
    assert!(matches!(
        rig.driver.retire_completed_request(90),
        Err(JournalError::Contract(Error::WrongState))
    ));
    make_unknown(&mut rig, first_attempt);
    assert!(matches!(
        rig.driver.retire_completed_request(90),
        Err(JournalError::Contract(Error::WrongState))
    ));
    let (port, grant, input) = planned(&rig, 90, 5001);
    let Knowledge::Known { value: acknowledged, .. } = port.abandon(input).unwrap() else {
        panic!("original disposition missing");
    };
    assert_eq!(acknowledged.stage(), ActionState::IrrecoverablyUnknown);
    let disposed = rig.driver.supervisor().host().unwrap().inspect();
    rig.driver.supervisor_mut()
        .set_snapshot(disposed.revision, Some(snapshot())).unwrap();
    let image = std::fs::read(rig.root.store().join("delivery.bin")).unwrap();
    assert!(rig.driver.retire_completed_request(90).unwrap());
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert!(rig.driver.helpers_reaped());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), disposed);
    assert_eq!(std::fs::read(rig.root.store().join("delivery.bin")).unwrap(), image);
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(
        rig.port.poll(&first),
        Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }
    );

    // The one-use intake snapshot survives local retirement. The existing actor
    // port, owner, limits and scoped grant all remain the originals.
    let proposal = rig.proposal();
    let second = rig.port.submit(91, &proposal).unwrap();
    rig.start(91, 191);
    assert!(matches!(
        rig.driver.retire_completed_request(90),
        Err(JournalError::Contract(Error::Binding))
    ));
    assert_eq!(rig.driver.phase(), FileDriverPhase::Reviewing { request: 91 });
    assert!(matches!(
        rig.review_event(fa_reference::round::Verdict::Allow, false),
        FileDriverEvent::ReviewApplied { request: 91, .. }
    ));
    let human = rig.human(1002, 30);
    let second_attempt = match rig.step(Some(&human)) {
        FileDriverEvent::Dispatched { request: 91, attempt } => attempt,
        event => panic!("second original dispatch missing: {event:?}"),
    };
    assert!(matches!(
        rig.step(None),
        FileDriverEvent::Published {
            request: 91, outcome: EndpointOutcome::Executed { .. }
        }
    ));
    assert!(matches!(
        rig.step(None),
        FileDriverEvent::Reconciled {
            request: 91, outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. })
        }
    ));
    let after = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, disposed.control.ledger.charged + 16);
    assert_eq!(after.control.ledger.stages[&first_attempt], ActionState::IrrecoverablyUnknown);
    assert_eq!(after.control.ledger.stages[&second_attempt], ActionState::Confirmed);
    assert!(matches!(rig.port.poll(&second), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(rig.port.poll(&first), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    let Knowledge::Known { value: retried, .. } = port.abandon(input).unwrap() else {
        panic!("retained grant lost its original result");
    };
    assert_eq!(retried, acknowledged);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), after);
    assert_eq!(grant.remaining_operations(), 0);
}

#[test]
fn idle_uncertainty_is_not_retirable_and_stopped_event_does_not_read_time_or_evidence() {
    for local_idle in [false, true] {
        let mut rig = Rig::new();
        let ticket = rig.submit(90);
        let attempt = original_dispatch(&mut rig, 90, 1001);
        make_unknown(&mut rig, attempt);
        if local_idle {
            assert!(matches!(
                rig.driver.step_with_evidence(
                    || ElapsedTick(1),
                    |_, _| panic!("query-only reconciliation requested evidence"),
                    None,
                ).unwrap(),
                FileDriverEvent::Reconciled {
                    request: 90, outcome: Reconciliation::AwaitingResolution
                }
            ));
            assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
        }
        let before = rig.driver.supervisor().host().unwrap().inspect();
        assert!(matches!(
            rig.driver.retire_completed_request(90),
            Err(JournalError::Contract(Error::WrongState))
        ));
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
        let (port, _grant, input) = planned(&rig, 90, 6001);
        let _ = port.abandon(input).unwrap();
        let after = rig.driver.supervisor().host().unwrap().inspect();
        if !local_idle {
            assert!(matches!(
                rig.driver.step_with_evidence(
                    || panic!("explicit disposition requested a new clock"),
                    |_, _| panic!("explicit disposition requested new evidence"),
                    None,
                ).unwrap(),
                FileDriverEvent::Stopped { request: 90, stage: ActionState::IrrecoverablyUnknown }
            ));
        }
        assert!(rig.driver.retire_completed_request(90).unwrap());
        assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), after);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.executions, before.executions);
        assert_eq!(after.payload, before.payload);
        assert_eq!(rig.port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
        assert!(matches!(
            rig.driver.resume_reconciliation(90),
            Err(JournalError::Contract(Error::WrongState))
        ));
    }
}
