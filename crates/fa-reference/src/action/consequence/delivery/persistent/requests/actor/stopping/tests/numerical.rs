//! Real numerical histories retain their original spent model/source budgets.
//! An explicit full-cut reservation admits Stop without generating or publishing.
use super::*;
mod fixture;
use fixture::{learned_owner, raw_owner};

#[test]
fn learned_fencing_reserves_the_full_original_cut_in_both_warm_and_cold_histories() {
    for cold in [false, true] {
        for published in [false, true] {
            let root = Root::new();
            let host = learned_owner(&root, published, cold);
            let (_, mut supervisor) = host.into_actor_gateway();
            let before = supervisor.host().unwrap().inspect();
            let numerical = supervisor.host().unwrap().learned_generation_inspection()
                .unwrap().numerical;
            let input = request(&supervisor, 1501);
            let events = usize::try_from(before.revision).unwrap() + 1;
            supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
            let files = root.files();

            let (plain, plain_grant) = supervisor.fence_port(events).unwrap();
            assert_eq!(plain.fence(input), Err(FileFenceError::Unsupported));
            assert_eq!(plain_grant.remaining_operations(), 1);
            let (short, short_grant) = supervisor
                .fence_port_with_numerical_budget(events, events - 1).unwrap();
            assert_eq!(short.fence(input), Err(FileFenceError::Capacity));
            assert_eq!(short_grant.remaining_operations(), 1);
            assert_eq!(short_grant.remaining_numerical_replay_events(), Some(events - 1));
            assert!(supervisor.owner.borrow().snapshot.is_some());
            assert_eq!(supervisor.host().unwrap().inspect(), before);
            assert_eq!(supervisor.host().unwrap().learned_generation_inspection()
                .unwrap().numerical, numerical);
            assert_eq!(root.files(), files);

            let (port, grant) = supervisor.fence_port_with_numerical_budget(events, events).unwrap();
            let stopped = receipt(port.fence(input));
            assert_eq!(grant.remaining_operations(), 0);
            assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
            assert_eq!(stopped.progress().unresolved, vec![1]);
            assert_eq!(stopped.progress().charged_units, 8);
            assert_eq!(stopped.stop().refunded_units(), 0);
            assert!(supervisor.owner.borrow().snapshot.is_none());
            let after = supervisor.host().unwrap().inspect();
            assert_eq!(after.control.ledger.stages[&1], ActionState::Unknown);
            assert_eq!(after.executions, u64::from(published));
            assert_eq!(after.executions, before.executions);
            assert_eq!(after.payload, before.payload);
            assert_eq!(after.control.ledger.elapsed, before.control.ledger.elapsed);
            assert_eq!(supervisor.host().unwrap().learned_generation_inspection()
                .unwrap().numerical, numerical);
            let files = root.files();
            assert_eq!(receipt(port.clone().fence(input)), stopped);
            assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
            assert_eq!(supervisor.host().unwrap().inspect(), after);
            assert_eq!(root.files(), files);
        }
    }
}

#[test]
fn raw_action_predictor_requires_its_own_allowance_without_a_decoder() {
    let root = Root::new();
    let host = raw_owner(&root, true);
    assert!(!host.decoder_required());
    assert!(host.action_consistency_required());
    let (_, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let forecast = supervisor.host().unwrap().action_consistency_snapshot().unwrap();
    let observation = supervisor.host().unwrap().action_consistency_observation(1)
        .unwrap().clone();
    let events = usize::try_from(before.revision).unwrap() + 1;
    let input = request(&supervisor, 1601);
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let files = root.files();
    let (plain, plain_grant) = supervisor.fence_port(events).unwrap();
    assert_eq!(plain.fence(input), Err(FileFenceError::Unsupported));
    assert_eq!(plain_grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);

    let (port, grant) = supervisor.fence_port_with_numerical_budget(events, events).unwrap();
    let stopped = receipt(port.fence(input));
    assert_eq!(stopped.progress().charged_units, 8);
    assert_eq!(stopped.progress().unresolved, vec![1]);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    let after = supervisor.host().unwrap().inspect();
    let current_forecast = supervisor.host().unwrap().action_consistency_snapshot().unwrap();
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(current_forecast.evidence, forecast.evidence);
    assert_eq!(current_forecast.pending_attempt, forecast.pending_attempt);
    assert_eq!(current_forecast.coverage_lost, forecast.coverage_lost);
    assert_eq!(supervisor.host().unwrap().action_consistency_observation(1).unwrap(), &observation);
    let files = root.files();
    assert_eq!(receipt(port.clone().fence(input)), stopped);
    assert_eq!(root.files(), files);
}

#[test]
fn failed_numerical_stop_keeps_its_complete_reservation_and_never_replays_a_retry() {
    let root = Root::new();
    let host = learned_owner(&root, false, true);
    let (_, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let input = request(&supervisor, 1701);
    let events = usize::try_from(before.revision).unwrap() + 1;
    let (port, grant) = supervisor.fence_port_with_numerical_budget(events, events).unwrap();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"occupied numerical stage").unwrap();
    assert_eq!(port.fence(input), Err(FileFenceError::Unavailable));
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert!(supervisor.owner.borrow().snapshot.is_none());
    assert!(supervisor.host().unwrap().storage_failure().is_some());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    // The last acknowledged observation is retained internally; no successful
    // fence receipt can escape for a candidate image that was not acknowledged.
    assert_eq!(supervisor.owner.borrow().host.inspect().control.ledger.charged, 8);
    let files = root.files();
    unknown(port.clone().fence(input));
    assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(root.files(), files);
}
