//! Same Store barriers, now between external-key forecast and actual intake.
use super::*;
use crate::action::{ActionSpec, ActionState, VERSION};
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::Snapshot;

fn start(root: &Directory) -> (FileOversight, FilePredictiveRoles, FilePredictiveRequirements) {
    let expected = expected();
    let (mut host, roles) = FileOversight::create_predictive_guarded(root.store(), profile(),
        &expected.oversight.guards, None, expected.prediction.clone(), None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles, expected)
}
fn frame() -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: capture(), stream: 17, sequence: 1, position: 0 }, &[-1.0]).unwrap()
}
fn spec(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"ordinary".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }

#[test]
fn forecast_write_faults_never_expose_speculative_binding_and_recovery_uses_only_canonical_state() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, roles, expected) = start(&root);
        let before = host.inspect(); host.store.fail_once(barrier); let revision = host.revision();
        failure(roles.consistency_observer.forecast_request(&mut host, revision, 700, 0, &frame()).unwrap_err(), barrier);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.pending_forecast_request(), Err(JournalError::Unavailable));
        let visible = barrier == JournalIo::DirectorySync;
        let disk = FileOversight::read_predictive_consistency(root.store(), &profile(), &expected).unwrap();
        assert_eq!(disk.consistency.pending_attempt, visible.then_some(1));
        assert_eq!(disk.consistency.evidence.samples(), 0); drop(host);
        let (mut host, _) = FileOversight::open_predictive_guarded(root.store(), profile(), &expected).unwrap();
        assert_eq!(host.pending_forecast_request().unwrap(), visible.then_some(700));
        assert_eq!(host.action_consistency_snapshot().unwrap().coverage_lost, visible);
        let revision = host.revision();
        assert!(matches!(roles.consistency_observer.unavailable(&mut host, revision), Err(JournalError::Contract(Error::Binding))));
    }
}

#[test]
fn admission_write_faults_preserve_either_pending_gap_or_consumed_request_never_both_or_neither() {
    for barrier in BARRIERS {
        let root = Directory::new(); let (mut host, roles, expected) = start(&root);
        let revision = host.revision();
        roles.consistency_observer.forecast_request(&mut host, revision, 700, 0, &frame()).unwrap().unwrap();
        let original = spec(&host); let before = host.inspect();
        host.store.fail_once(barrier);
        failure(host.submit_request(host.revision(), 700, original.clone(), snapshot()).unwrap_err(), barrier);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.request_status(700), Err(JournalError::Unavailable));
        let visible = barrier == JournalIo::DirectorySync;
        let disk = FileOversight::read_predictive_consistency(root.store(), &profile(), &expected).unwrap();
        assert_eq!(disk.consistency.evidence.samples(), usize::from(visible));
        assert_eq!(disk.consistency.pending_attempt, (!visible).then_some(1)); drop(host);
        let (mut host, _) = FileOversight::open_predictive_guarded(root.store(), profile(), &expected).unwrap();
        assert_eq!(host.pending_forecast_request().unwrap(), (!visible).then_some(700));
        assert_eq!(host.action_consistency_snapshot().unwrap().coverage_lost, !visible);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 0);
        if visible {
            let before = host.action_consistency_snapshot().unwrap();
            assert_eq!(host.submit_request(0, 700, original, Snapshot::default()).unwrap().disposition,
                FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Cancelled });
            assert_eq!(host.action_consistency_snapshot().unwrap(), before);
        } else {
            assert_eq!(host.request_status(700), Err(JournalError::Contract(Error::Missing)));
        }
    }
}

#[test]
fn full_semantic_replay_refuses_forecast_theft_even_when_the_input_frame_is_well_formed() {
    let root = Directory::new(); let (mut host, roles, _) = start(&root); let revision = host.revision();
    roles.consistency_observer.forecast_request(&mut host, revision, 700, 0, &frame()).unwrap().unwrap();
    let original = spec(&host);
    for forged in [Event::Core(BaseEvent::Propose(1, original.clone(), snapshot())),
        Event::Core(BaseEvent::SubmitRequest(701, original.clone(), snapshot()))] {
        let mut events = host.events.clone(); events.push(forged);
        let bytes = journal::encode(&profile(), host.store.identity(), &events).unwrap();
        let decoded = journal::decode(&profile(), host.store.identity(), &bytes).unwrap();
        assert!(matches!(Machine::replay(&profile(), &decoded), Err(Error::Binding)));
    }
    assert_eq!(host.submit_request(host.revision(), 700, original, snapshot()).unwrap().disposition,
        FileRequestDisposition::Admitted { attempt: 1, stage: ActionState::Reviewing });
    assert_eq!(host.pending_forecast_request().unwrap(), None);
}
