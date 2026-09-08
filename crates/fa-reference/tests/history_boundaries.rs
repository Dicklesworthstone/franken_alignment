//! Public seeded-history boundaries for the FA-004 reference oracle.
//!
//! This exercises typed, in-memory replay only. It does not establish a
//! production differential, durable journal, parser, broker, or real effect.

use std::collections::BTreeMap;

use fa_reference::action::{
    ActionSpec, ActionState, ElapsedTick, FrozenAction, Inspection, Purpose, ResolvedTarget, Scope,
    TrustedOutcome, VERSION,
};
use fa_reference::history::{
    HistoryConfig, HistoryEvent, ReplayOutcome, ScheduledStep, SeededHistory, replay,
};
use fa_reference::{Error, Snapshot};

fn scope() -> Scope {
    Scope {
        tenant: 11,
        principal: 12,
        run: 13,
        branch: 14,
        authority: 15,
        purpose: Purpose::Effect,
    }
}

fn config(seed: u64, total: u64, max_attempts: usize) -> HistoryConfig {
    HistoryConfig {
        scope: scope(),
        total,
        max_attempts,
        seed,
    }
}

fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 7,
        complete: true,
        values: BTreeMap::new(),
    }
}

fn action(object: u64, units: u64, deadline: u64) -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION,
        scope: scope(),
        target: Some(ResolvedTarget {
            adapter: 21,
            object,
            contract_version: 23,
            expected_version: 24,
            generation: 25,
        }),
        payload: vec![object as u8],
        required_witnesses: Vec::new(),
        policy_epoch: 0,
        deadline: ElapsedTick(deadline),
        units,
    })
    .expect("typed history action is within the public FA-001 frozen-action profile")
}

fn successful_lane(attempt: u64, action: FrozenAction) -> Vec<HistoryEvent> {
    let snapshot = snapshot();
    vec![
        HistoryEvent::ObserveTime(ElapsedTick(1)),
        HistoryEvent::Propose {
            attempt,
            action: action.clone(),
        },
        HistoryEvent::Prepare { attempt },
        HistoryEvent::BeginReview { attempt },
        HistoryEvent::Authorize {
            attempt,
            snapshot: snapshot.clone(),
        },
        HistoryEvent::Dispatch {
            attempt,
            action,
            snapshot,
        },
        HistoryEvent::RecordOutcome {
            attempt,
            outcome: TrustedOutcome::Executed,
        },
    ]
}

fn assert_inspection(
    inspection: &Inspection,
    available: u64,
    reserved: u64,
    charged: u64,
    epoch: u64,
    elapsed: Option<ElapsedTick>,
    stages: &[(u64, ActionState)],
) {
    assert_eq!(
        inspection,
        &Inspection {
            available,
            reserved,
            charged,
            epoch,
            elapsed,
            stages: stages.iter().copied().collect(),
        }
    );
    assert_eq!(available + reserved + charged, 10);
}

fn assert_schedule_covers_each_lane_once_in_input_order(
    chosen_prefix: &[ScheduledStep],
    lanes: &[Vec<HistoryEvent>],
) {
    assert_eq!(
        chosen_prefix.len(),
        lanes.iter().map(Vec::len).sum::<usize>(),
        "a successful replay must schedule every retained typed event"
    );
    let mut next_indices = vec![0_usize; lanes.len()];
    for step in chosen_prefix {
        assert!(
            step.lane < lanes.len(),
            "scheduler selected an unknown lane"
        );
        assert_eq!(
            step.event_index, next_indices[step.lane],
            "scheduler must preserve input order within each lane"
        );
        next_indices[step.lane] += 1;
    }
    assert_eq!(
        next_indices,
        lanes.iter().map(Vec::len).collect::<Vec<_>>(),
        "each lane must contribute every event exactly once"
    );
}

#[test]
fn same_seed_multilane_lifecycle_is_deterministic_ordered_and_fully_accounted() {
    let lanes = vec![
        successful_lane(1, action(101, 3, 10)),
        successful_lane(2, action(102, 2, 10)),
    ];
    let history = SeededHistory::new(config(0x5eed, 10, 2), lanes).unwrap();

    let first = replay(&history).unwrap();
    let second = replay(&history).unwrap();
    assert_eq!(
        first, second,
        "the same checked history and seed must choose the same schedule"
    );
    let ReplayOutcome::Completed {
        seed,
        chosen_prefix,
        inspection,
    } = first
    else {
        panic!("two well-formed per-lane lifecycles must complete");
    };
    assert_eq!(seed, 0x5eed);
    assert_schedule_covers_each_lane_once_in_input_order(&chosen_prefix, history.lanes());
    assert_inspection(
        &inspection,
        5,
        0,
        5,
        0,
        Some(ElapsedTick(1)),
        &[(1, ActionState::Confirmed), (2, ActionState::Confirmed)],
    );
}

#[test]
fn invalid_lifecycle_and_expiry_are_counterexamples_at_the_exact_scheduled_event() {
    let invalid = SeededHistory::new(
        config(7, 10, 1),
        vec![vec![
            HistoryEvent::ObserveTime(ElapsedTick(1)),
            HistoryEvent::Prepare { attempt: 1 },
        ]],
    )
    .unwrap();
    let ReplayOutcome::Counterexample {
        chosen_prefix,
        refusal,
        inspection,
        ..
    } = replay(&invalid).unwrap()
    else {
        panic!("preparing an absent attempt must remain a counterexample");
    };
    assert_eq!(refusal, Error::Missing);
    assert_eq!(
        chosen_prefix.last(),
        Some(&ScheduledStep {
            lane: 0,
            event_index: 1
        })
    );
    assert_inspection(&inspection, 10, 0, 0, 0, Some(ElapsedTick(1)), &[]);

    let expired = SeededHistory::new(
        config(9, 10, 1),
        vec![vec![
            HistoryEvent::ObserveTime(ElapsedTick(10)),
            HistoryEvent::Propose {
                attempt: 1,
                action: action(103, 3, 10),
            },
            HistoryEvent::Prepare { attempt: 1 },
        ]],
    )
    .unwrap();
    let ReplayOutcome::Counterexample {
        chosen_prefix,
        refusal,
        inspection,
        ..
    } = replay(&expired).unwrap()
    else {
        panic!("deadline equality must remain an expiry counterexample");
    };
    assert_eq!(refusal, Error::Stale);
    assert_eq!(
        chosen_prefix.last(),
        Some(&ScheduledStep {
            lane: 0,
            event_index: 2
        })
    );
    assert_inspection(
        &inspection,
        10,
        0,
        0,
        0,
        Some(ElapsedTick(10)),
        &[(1, ActionState::Proposed)],
    );
}

#[test]
fn unknown_cannot_cancel_but_explicit_trusted_nonexecution_reconciles_it() {
    let action = action(104, 3, 10);
    let mut cancelled = successful_lane(1, action.clone());
    cancelled.pop();
    cancelled.push(HistoryEvent::MarkUnknown { attempt: 1 });
    cancelled.push(HistoryEvent::Cancel { attempt: 1 });
    let cancellation_history = SeededHistory::new(config(11, 10, 1), vec![cancelled]).unwrap();

    let ReplayOutcome::Counterexample {
        chosen_prefix,
        refusal,
        inspection,
        ..
    } = replay(&cancellation_history).unwrap()
    else {
        panic!("unknown effects must not be silently cancelled");
    };
    assert_eq!(refusal, Error::WrongState);
    assert_eq!(
        chosen_prefix.last(),
        Some(&ScheduledStep {
            lane: 0,
            event_index: 7
        })
    );
    assert_inspection(
        &inspection,
        7,
        0,
        3,
        0,
        Some(ElapsedTick(1)),
        &[(1, ActionState::Unknown)],
    );

    let mut reconciled = successful_lane(1, action);
    reconciled.pop();
    reconciled.push(HistoryEvent::MarkUnknown { attempt: 1 });
    reconciled.push(HistoryEvent::RecordOutcome {
        attempt: 1,
        outcome: TrustedOutcome::NotExecuted,
    });
    let reconciliation_history = SeededHistory::new(config(11, 10, 1), vec![reconciled]).unwrap();
    let ReplayOutcome::Completed { inspection, .. } = replay(&reconciliation_history).unwrap()
    else {
        panic!("explicit trusted nonexecution is the supported unknown reconciliation");
    };
    assert_inspection(
        &inspection,
        10,
        0,
        0,
        0,
        Some(ElapsedTick(1)),
        &[(1, ActionState::ConfirmedNotExecuted)],
    );
}

#[test]
fn revocation_orders_before_future_admission_without_restoring_an_old_floor() {
    let action = action(105, 3, 10);
    let history = SeededHistory::new(
        config(13, 10, 2),
        vec![vec![
            HistoryEvent::ObserveTime(ElapsedTick(1)),
            HistoryEvent::Propose {
                attempt: 1,
                action: action.clone(),
            },
            HistoryEvent::Prepare { attempt: 1 },
            HistoryEvent::BeginReview { attempt: 1 },
            HistoryEvent::Authorize {
                attempt: 1,
                snapshot: snapshot(),
            },
            HistoryEvent::RevokeEpoch,
            HistoryEvent::Cancel { attempt: 1 },
            HistoryEvent::Propose { attempt: 2, action },
            HistoryEvent::Prepare { attempt: 2 },
        ]],
    )
    .unwrap();

    let ReplayOutcome::Counterexample {
        chosen_prefix,
        refusal,
        inspection,
        ..
    } = replay(&history).unwrap()
    else {
        panic!("a pre-revocation action must not restore the revoked epoch floor");
    };
    assert_eq!(refusal, Error::Stale);
    assert_eq!(
        chosen_prefix.last(),
        Some(&ScheduledStep {
            lane: 0,
            event_index: 8
        })
    );
    assert_inspection(
        &inspection,
        10,
        0,
        0,
        1,
        Some(ElapsedTick(1)),
        &[(1, ActionState::Cancelled), (2, ActionState::Proposed)],
    );
}
