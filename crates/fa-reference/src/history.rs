//! Seeded, bounded replay histories for the existing in-memory action oracle.
//!
//! This is a typed event log, not a parser, workflow framework, durable journal,
//! broker, or production differential. It delegates every lifecycle and rights
//! decision to `ReferenceAuthority` and retains semantic refusals as replay
//! counterexamples instead of turning them into success.

use crate::action::{
    ElapsedTick, FrozenAction, Inspection, Permit, ReferenceAuthority, Scope, TrustedOutcome,
};
use crate::{Error, Judgment, ReadWitness, Snapshot};
use std::collections::BTreeMap;

/// Bound the number of independently ordered lanes in one history.
pub const MAX_HISTORY_LANES: usize = 8;
/// Bound all retained events, schedule selections, and replay work.
pub const MAX_HISTORY_EVENTS: usize = 128;
/// Bound key/value entries in one caller-supplied reference snapshot.
pub const MAX_HISTORY_SNAPSHOT_ENTRIES: usize = 64;
/// Bound retained value bytes in one caller-supplied reference snapshot.
pub const MAX_HISTORY_SNAPSHOT_VALUE_BYTES: usize = 4_096;
/// Bound all action payload, declared witness, and snapshot value bytes held by
/// one typed history. This is a reference-profile retention bound, not a
/// measurement of allocator use or process memory.
pub const MAX_HISTORY_RETAINED_BYTES: usize = 12_288;

/// The one reference authority domain replayed by a history.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryConfig {
    pub scope: Scope,
    pub total: u64,
    pub max_attempts: usize,
    pub seed: u64,
}

/// A typed operation over the existing action authority. Authorize derives its
/// judgment from the proposed action's declared witnesses and supplied snapshot;
/// it cannot inject a second decision implementation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryEvent {
    ObserveTime(ElapsedTick),
    Propose {
        attempt: u64,
        action: FrozenAction,
    },
    Prepare {
        attempt: u64,
    },
    BeginReview {
        attempt: u64,
    },
    Authorize {
        attempt: u64,
        snapshot: Snapshot,
    },
    Dispatch {
        attempt: u64,
        action: FrozenAction,
        snapshot: Snapshot,
    },
    Cancel {
        attempt: u64,
    },
    Deny {
        attempt: u64,
    },
    MarkUnknown {
        attempt: u64,
    },
    RecordOutcome {
        attempt: u64,
        outcome: TrustedOutcome,
    },
    RevokeEpoch,
}

impl HistoryEvent {
    fn attempt(&self) -> Option<u64> {
        match self {
            Self::ObserveTime(_) | Self::RevokeEpoch => None,
            Self::Propose { attempt, .. }
            | Self::Prepare { attempt }
            | Self::BeginReview { attempt }
            | Self::Authorize { attempt, .. }
            | Self::Dispatch { attempt, .. }
            | Self::Cancel { attempt }
            | Self::Deny { attempt }
            | Self::MarkUnknown { attempt }
            | Self::RecordOutcome { attempt, .. } => Some(*attempt),
        }
    }
}

/// One deterministic choice made by the bounded scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScheduledStep {
    pub lane: usize,
    pub event_index: usize,
}

/// A checked typed history. Lanes retain their input order; the seed chooses
/// only which ready lane advances next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeededHistory {
    config: HistoryConfig,
    lanes: Vec<Vec<HistoryEvent>>,
}

impl SeededHistory {
    /// Validate configuration and all structural history bounds before keeping
    /// caller-owned events. No parser or input-loading claim is made here.
    pub fn new(config: HistoryConfig, lanes: Vec<Vec<HistoryEvent>>) -> Result<Self, Error> {
        validate_config(config)?;
        validate_lanes(&lanes)?;
        Ok(Self { config, lanes })
    }

    pub fn config(&self) -> HistoryConfig {
        self.config
    }

    pub fn lanes(&self) -> &[Vec<HistoryEvent>] {
        &self.lanes
    }
}

/// Terminal result of one bounded replay. `Completed` means every selected
/// event was accepted; it does not mean every proposed action reached a
/// terminal state. An `Err` from `replay` means configuration or typed
/// structure was malformed before replay began.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayOutcome {
    Completed {
        seed: u64,
        chosen_prefix: Vec<ScheduledStep>,
        inspection: Inspection,
    },
    Counterexample {
        seed: u64,
        chosen_prefix: Vec<ScheduledStep>,
        refusal: Error,
        inspection: Inspection,
    },
}

/// Replay a checked history under a deterministic, seeded, bounded scheduler.
///
/// The small PRNG intentionally uses explicit wrapping arithmetic. Lane choice
/// scans at most `MAX_HISTORY_LANES` entries from the generated start index, so
/// a depleted lane cannot cause an unbounded rejection loop.
pub fn replay(history: &SeededHistory) -> Result<ReplayOutcome, Error> {
    validate_config(history.config)?;
    validate_lanes(&history.lanes)?;

    let mut authority = ReferenceAuthority::new(
        history.config.scope,
        history.config.total,
        history.config.max_attempts,
    )?;
    let mut actions = BTreeMap::new();
    let mut permits = BTreeMap::new();
    let mut cursors = vec![0_usize; history.lanes.len()];
    let total_events = history.lanes.iter().map(Vec::len).sum::<usize>();
    let mut chosen_prefix = Vec::with_capacity(total_events);
    let mut state = history.config.seed;

    for _ in 0..total_events {
        let lane = choose_ready_lane(&mut state, &cursors, &history.lanes)?;
        let event_index = cursors[lane];
        let event = &history.lanes[lane][event_index];
        let step = ScheduledStep { lane, event_index };
        chosen_prefix.push(step);
        cursors[lane] = cursors[lane].checked_add(1).ok_or(Error::Overflow)?;

        if let Err(refusal) = apply_event(event, &mut authority, &mut actions, &mut permits) {
            return Ok(ReplayOutcome::Counterexample {
                seed: history.config.seed,
                chosen_prefix,
                refusal,
                inspection: authority.inspect(),
            });
        }
    }

    Ok(ReplayOutcome::Completed {
        seed: history.config.seed,
        chosen_prefix,
        inspection: authority.inspect(),
    })
}

fn validate_config(config: HistoryConfig) -> Result<(), Error> {
    // Delegate exact scope/total/attempt-cap validation to the authority whose
    // transitions will run, before event selection or mutable authority work.
    ReferenceAuthority::new(config.scope, config.total, config.max_attempts).map(|_| ())
}

fn validate_lanes(lanes: &[Vec<HistoryEvent>]) -> Result<(), Error> {
    if lanes.is_empty() || lanes.len() > MAX_HISTORY_LANES {
        return Err(Error::Limit);
    }
    let mut total = 0_usize;
    let mut retained_bytes = 0_usize;
    for lane in lanes {
        if lane.is_empty() {
            return Err(Error::InvalidInput);
        }
        total = total.checked_add(lane.len()).ok_or(Error::Overflow)?;
        if total > MAX_HISTORY_EVENTS {
            return Err(Error::Limit);
        }
        for event in lane {
            if event.attempt().is_some_and(|attempt| attempt == 0) {
                return Err(Error::InvalidInput);
            }
            retained_bytes = add_bounded_bytes(retained_bytes, event_retained_bytes(event)?)?;
        }
    }
    Ok(())
}

fn event_retained_bytes(event: &HistoryEvent) -> Result<usize, Error> {
    match event {
        HistoryEvent::Propose { action, .. } => action_retained_bytes(action),
        HistoryEvent::Authorize { snapshot, .. } => snapshot_retained_bytes(snapshot),
        HistoryEvent::Dispatch {
            action, snapshot, ..
        } => add_bounded_bytes(
            action_retained_bytes(action)?,
            snapshot_retained_bytes(snapshot)?,
        ),
        HistoryEvent::ObserveTime(_)
        | HistoryEvent::Prepare { .. }
        | HistoryEvent::BeginReview { .. }
        | HistoryEvent::Cancel { .. }
        | HistoryEvent::Deny { .. }
        | HistoryEvent::MarkUnknown { .. }
        | HistoryEvent::RecordOutcome { .. }
        | HistoryEvent::RevokeEpoch => Ok(0),
    }
}

fn action_retained_bytes(action: &FrozenAction) -> Result<usize, Error> {
    let mut retained = action.spec().payload.len();
    for witness in &action.spec().required_witnesses {
        if let ReadWitness::Exact {
            value: Some(value), ..
        } = witness
        {
            retained = add_bounded_bytes(retained, value.len())?;
        }
    }
    Ok(retained)
}

fn snapshot_retained_bytes(snapshot: &Snapshot) -> Result<usize, Error> {
    if snapshot.values.len() > MAX_HISTORY_SNAPSHOT_ENTRIES {
        return Err(Error::Limit);
    }
    let mut retained = 0_usize;
    for value in snapshot.values.values() {
        retained = retained.checked_add(value.len()).ok_or(Error::Overflow)?;
        if retained > MAX_HISTORY_SNAPSHOT_VALUE_BYTES {
            return Err(Error::Limit);
        }
    }
    Ok(retained)
}

fn add_bounded_bytes(total: usize, added: usize) -> Result<usize, Error> {
    let total = total.checked_add(added).ok_or(Error::Overflow)?;
    if total > MAX_HISTORY_RETAINED_BYTES {
        Err(Error::Limit)
    } else {
        Ok(total)
    }
}

fn choose_ready_lane(
    state: &mut u64,
    cursors: &[usize],
    lanes: &[Vec<HistoryEvent>],
) -> Result<usize, Error> {
    if cursors.len() != lanes.len() || lanes.is_empty() || lanes.len() > MAX_HISTORY_LANES {
        return Err(Error::InvalidInput);
    }
    let lane_count = u64::try_from(lanes.len()).map_err(|_| Error::Overflow)?;
    let start = usize::try_from(next_random(state) % lane_count).map_err(|_| Error::Overflow)?;
    for offset in 0..lanes.len() {
        let lane = (start + offset) % lanes.len();
        if cursors[lane] < lanes[lane].len() {
            return Ok(lane);
        }
    }
    Err(Error::Incomplete)
}

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    mix_random(*state)
}

fn mix_random(value: u64) -> u64 {
    let mut mixed = value ^ (value >> 30);
    mixed = mixed.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    mixed ^= mixed >> 27;
    mixed = mixed.wrapping_mul(0x94d0_49bb_1331_11eb);
    mixed ^ (mixed >> 31)
}

fn apply_event(
    event: &HistoryEvent,
    authority: &mut ReferenceAuthority,
    actions: &mut BTreeMap<u64, FrozenAction>,
    permits: &mut BTreeMap<u64, Permit>,
) -> Result<(), Error> {
    match event {
        HistoryEvent::ObserveTime(elapsed) => authority.observe_time(*elapsed),
        HistoryEvent::Propose { attempt, action } => {
            authority.propose(*attempt, action.clone())?;
            if actions.insert(*attempt, action.clone()).is_some() {
                return Err(Error::Duplicate);
            }
            Ok(())
        }
        HistoryEvent::Prepare { attempt } => authority.prepare(*attempt),
        HistoryEvent::BeginReview { attempt } => authority.begin_review(*attempt),
        HistoryEvent::Authorize { attempt, snapshot } => {
            let action = actions.get(attempt).ok_or(Error::Missing)?;
            let judgment = Judgment::capture(snapshot, action.spec().required_witnesses.clone())?;
            let permit = authority.authorize(*attempt, &judgment, snapshot)?;
            if permits.insert(*attempt, permit).is_some() {
                return Err(Error::Duplicate);
            }
            Ok(())
        }
        HistoryEvent::Dispatch {
            attempt,
            action,
            snapshot,
        } => {
            let permit = permits.get(attempt).ok_or(Error::Missing)?;
            authority.dispatch(permit, action, snapshot)
        }
        HistoryEvent::Cancel { attempt } => authority.cancel(*attempt),
        HistoryEvent::Deny { attempt } => authority.deny(*attempt),
        HistoryEvent::MarkUnknown { attempt } => authority.mark_unknown(*attempt),
        HistoryEvent::RecordOutcome { attempt, outcome } => {
            authority.record_trusted_outcome(*attempt, *outcome)
        }
        HistoryEvent::RevokeEpoch => authority.revoke_epoch(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReadWitness;
    use crate::action::{ActionSpec, ActionState, Purpose, ResolvedTarget, VERSION};

    fn scope() -> Scope {
        Scope {
            tenant: 1,
            principal: 2,
            run: 3,
            branch: 4,
            authority: 5,
            purpose: Purpose::Effect,
        }
    }

    fn config(seed: u64, max_attempts: usize) -> HistoryConfig {
        HistoryConfig {
            scope: scope(),
            total: 10,
            max_attempts,
            seed,
        }
    }

    fn snapshot() -> Snapshot {
        Snapshot {
            semantic_epoch: 0,
            complete: true,
            values: BTreeMap::new(),
        }
    }

    fn snapshot_with_value_bytes(bytes: usize) -> Snapshot {
        Snapshot {
            semantic_epoch: 0,
            complete: true,
            values: BTreeMap::from([(1, vec![0; bytes])]),
        }
    }

    fn snapshot_with_entries(entries: usize) -> Snapshot {
        let mut values = BTreeMap::new();
        for entry in 0..entries {
            values.insert(u64::try_from(entry + 1).unwrap(), Vec::new());
        }
        Snapshot {
            semantic_epoch: 0,
            complete: true,
            values,
        }
    }

    fn action(object: u64, deadline: u64) -> FrozenAction {
        action_with_data(object, deadline, vec![object as u8], Vec::new())
    }

    fn action_with_data(
        object: u64,
        deadline: u64,
        payload: Vec<u8>,
        required_witnesses: Vec<ReadWitness>,
    ) -> FrozenAction {
        FrozenAction::freeze(ActionSpec {
            version: VERSION,
            scope: scope(),
            target: Some(ResolvedTarget {
                adapter: 7,
                object,
                contract_version: 1,
                expected_version: 1,
                generation: 1,
            }),
            payload,
            required_witnesses,
            policy_epoch: 0,
            deadline: ElapsedTick(deadline),
            units: 2,
        })
        .unwrap()
    }

    fn successful_lane(attempt: u64, action: FrozenAction) -> Vec<HistoryEvent> {
        let snapshot = snapshot();
        vec![
            HistoryEvent::ObserveTime(ElapsedTick(0)),
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

    #[test]
    fn seeded_multilane_success_reuses_reference_authority_lifecycle() {
        let history = SeededHistory::new(
            config(17, 2),
            vec![
                successful_lane(1, action(11, 8)),
                successful_lane(2, action(12, 8)),
            ],
        )
        .unwrap();
        let first = replay(&history).unwrap();
        let second = replay(&history).unwrap();

        assert_eq!(first, second);
        let ReplayOutcome::Completed {
            seed,
            chosen_prefix,
            inspection,
        } = first
        else {
            panic!("successful histories must complete");
        };
        assert_eq!(seed, 17);
        assert_eq!(chosen_prefix.len(), 14);
        assert_eq!(inspection.stages[&1], ActionState::Confirmed);
        assert_eq!(inspection.stages[&2], ActionState::Confirmed);
        assert_eq!(inspection.available, 6);
        assert_eq!(inspection.charged, 4);
    }

    #[test]
    fn declared_seed_set_has_multiple_lane_order_preserving_schedules() {
        let seeds = [1, 2, 3, 4, 5, 6];
        let schedules: Vec<Vec<ScheduledStep>> = seeds
            .into_iter()
            .map(|seed| {
                let history = SeededHistory::new(
                    config(seed, 2),
                    vec![
                        successful_lane(1, action(11, 8)),
                        successful_lane(2, action(12, 8)),
                    ],
                )
                .unwrap();
                let ReplayOutcome::Completed { chosen_prefix, .. } = replay(&history).unwrap()
                else {
                    panic!("declared successful history must complete");
                };
                chosen_prefix
            })
            .collect();

        let mut distinct = Vec::new();
        for schedule in &schedules {
            if !distinct.contains(&schedule) {
                distinct.push(schedule);
            }
        }
        // The unmixed low-bit LCG produced only two alternating schedules.
        assert!(distinct.len() > 2);
        for schedule in &schedules {
            let mut next_event = [0_usize; 2];
            for step in schedule {
                assert_eq!(step.event_index, next_event[step.lane]);
                next_event[step.lane] += 1;
            }
            assert_eq!(next_event, [7, 7]);
        }
        // This is a fixed-seed regression witness, not a uniformity claim.
    }

    #[test]
    fn cancellation_before_dispatch_completes_and_refunds() {
        let action = action(11, 8);
        let history = SeededHistory::new(
            config(3, 1),
            vec![vec![
                HistoryEvent::ObserveTime(ElapsedTick(0)),
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
                HistoryEvent::Cancel { attempt: 1 },
            ]],
        )
        .unwrap();

        let ReplayOutcome::Completed { inspection, .. } = replay(&history).unwrap() else {
            panic!("pre-dispatch cancellation is a valid lifecycle");
        };
        assert_eq!(inspection.stages[&1], ActionState::Cancelled);
        assert_eq!(inspection.available, 10);
        assert_eq!(inspection.charged, 0);
    }

    #[test]
    fn completed_only_means_all_events_were_accepted() {
        let history = SeededHistory::new(
            config(19, 1),
            vec![vec![
                HistoryEvent::ObserveTime(ElapsedTick(0)),
                HistoryEvent::Propose {
                    attempt: 1,
                    action: action(11, 8),
                },
            ]],
        )
        .unwrap();

        let ReplayOutcome::Completed { inspection, .. } = replay(&history).unwrap() else {
            panic!("accepted prefix must complete its event-log replay");
        };
        assert_eq!(inspection.stages[&1], ActionState::Proposed);
    }

    #[test]
    fn cancellation_after_dispatch_is_a_retained_counterexample() {
        let action = action(11, 8);
        let mut lane = successful_lane(1, action);
        lane.pop();
        lane.push(HistoryEvent::Cancel { attempt: 1 });
        let history = SeededHistory::new(config(5, 1), vec![lane]).unwrap();

        let ReplayOutcome::Counterexample {
            seed,
            chosen_prefix,
            refusal,
            inspection,
        } = replay(&history).unwrap()
        else {
            panic!("post-dispatch cancellation must refuse");
        };
        assert_eq!(seed, 5);
        assert_eq!(refusal, Error::WrongState);
        assert_eq!(
            chosen_prefix.last(),
            Some(&ScheduledStep {
                lane: 0,
                event_index: 6
            })
        );
        assert_eq!(inspection.stages[&1], ActionState::Dispatching);
        assert_eq!(inspection.charged, 2);
    }

    #[test]
    fn expiry_and_invalid_order_are_retained_counterexamples() {
        let expired = SeededHistory::new(
            config(7, 1),
            vec![vec![
                HistoryEvent::ObserveTime(ElapsedTick(8)),
                HistoryEvent::Propose {
                    attempt: 1,
                    action: action(11, 8),
                },
                HistoryEvent::Prepare { attempt: 1 },
            ]],
        )
        .unwrap();
        let ReplayOutcome::Counterexample {
            refusal,
            chosen_prefix,
            ..
        } = replay(&expired).unwrap()
        else {
            panic!("expired preparation must refuse");
        };
        assert_eq!(refusal, Error::Stale);
        assert_eq!(chosen_prefix.len(), 3);

        let out_of_order = SeededHistory::new(
            config(11, 1),
            vec![vec![HistoryEvent::Prepare { attempt: 1 }]],
        )
        .unwrap();
        assert!(matches!(
            replay(&out_of_order),
            Ok(ReplayOutcome::Counterexample {
                refusal: Error::Missing,
                ..
            })
        ));
    }

    #[test]
    fn malformed_typed_histories_and_exact_bounds_refuse_before_replay() {
        let mut experiment_config = config(1, 1);
        experiment_config.scope.purpose = Purpose::Experiment;
        assert_eq!(
            SeededHistory::new(
                experiment_config,
                vec![vec![HistoryEvent::ObserveTime(ElapsedTick(0))]],
            ),
            Err(Error::Binding)
        );
        assert_eq!(
            SeededHistory::new(
                config(1, 0),
                vec![vec![HistoryEvent::ObserveTime(ElapsedTick(0))]],
            ),
            Err(Error::InvalidInput)
        );
        assert_eq!(
            SeededHistory::new(config(1, 1), vec![Vec::new()]),
            Err(Error::InvalidInput)
        );
        assert_eq!(
            SeededHistory::new(
                config(1, 1),
                vec![vec![HistoryEvent::Prepare { attempt: 0 }]],
            ),
            Err(Error::InvalidInput)
        );

        let exact_events = vec![HistoryEvent::ObserveTime(ElapsedTick(0)); MAX_HISTORY_EVENTS];
        let exact = SeededHistory::new(config(1, 1), vec![exact_events]).unwrap();
        assert!(matches!(
            replay(&exact),
            Ok(ReplayOutcome::Completed { .. })
        ));

        let one_over = vec![HistoryEvent::ObserveTime(ElapsedTick(0)); MAX_HISTORY_EVENTS + 1];
        assert_eq!(
            SeededHistory::new(config(1, 1), vec![one_over]),
            Err(Error::Limit)
        );

        let exact_lanes = vec![vec![HistoryEvent::ObserveTime(ElapsedTick(0))]; MAX_HISTORY_LANES];
        assert!(SeededHistory::new(config(1, 1), exact_lanes).is_ok());
        let one_over_lanes =
            vec![vec![HistoryEvent::ObserveTime(ElapsedTick(0))]; MAX_HISTORY_LANES + 1];
        assert_eq!(
            SeededHistory::new(config(1, 1), one_over_lanes),
            Err(Error::Limit)
        );
    }

    #[test]
    fn snapshot_and_retained_byte_bounds_refuse_before_replay() {
        let exact_entries = SeededHistory::new(
            config(1, 1),
            vec![vec![HistoryEvent::Authorize {
                attempt: 1,
                snapshot: snapshot_with_entries(MAX_HISTORY_SNAPSHOT_ENTRIES),
            }]],
        );
        assert!(exact_entries.is_ok());
        assert_eq!(
            SeededHistory::new(
                config(1, 1),
                vec![vec![HistoryEvent::Authorize {
                    attempt: 1,
                    snapshot: snapshot_with_entries(MAX_HISTORY_SNAPSHOT_ENTRIES + 1),
                }]],
            ),
            Err(Error::Limit)
        );

        let exact_snapshot = SeededHistory::new(
            config(1, 1),
            vec![vec![HistoryEvent::Authorize {
                attempt: 1,
                snapshot: snapshot_with_value_bytes(MAX_HISTORY_SNAPSHOT_VALUE_BYTES),
            }]],
        );
        assert!(exact_snapshot.is_ok());
        assert_eq!(
            SeededHistory::new(
                config(1, 1),
                vec![vec![HistoryEvent::Authorize {
                    attempt: 1,
                    snapshot: snapshot_with_value_bytes(MAX_HISTORY_SNAPSHOT_VALUE_BYTES + 1),
                }]],
            ),
            Err(Error::Limit)
        );

        let exact_payload =
            action_with_data(11, 8, vec![0; MAX_HISTORY_RETAINED_BYTES], Vec::new());
        assert!(
            SeededHistory::new(
                config(1, 1),
                vec![vec![HistoryEvent::Propose {
                    attempt: 1,
                    action: exact_payload,
                }]],
            )
            .is_ok()
        );
        let one_over_payload =
            action_with_data(11, 8, vec![0; MAX_HISTORY_RETAINED_BYTES + 1], Vec::new());
        assert_eq!(
            SeededHistory::new(
                config(1, 1),
                vec![vec![HistoryEvent::Propose {
                    attempt: 1,
                    action: one_over_payload,
                }]],
            ),
            Err(Error::Limit)
        );

        let exact_witness = action_with_data(
            11,
            8,
            Vec::new(),
            vec![ReadWitness::Exact {
                key: 1,
                value: Some(vec![0; MAX_HISTORY_RETAINED_BYTES]),
            }],
        );
        assert!(
            SeededHistory::new(
                config(1, 1),
                vec![vec![HistoryEvent::Propose {
                    attempt: 1,
                    action: exact_witness,
                }]],
            )
            .is_ok()
        );
        let one_over_witness = action_with_data(
            11,
            8,
            Vec::new(),
            vec![ReadWitness::Exact {
                key: 1,
                value: Some(vec![0; MAX_HISTORY_RETAINED_BYTES + 1]),
            }],
        );
        assert_eq!(
            SeededHistory::new(
                config(1, 1),
                vec![vec![HistoryEvent::Propose {
                    attempt: 1,
                    action: one_over_witness,
                }]],
            ),
            Err(Error::Limit)
        );

        let exact_aggregate = SeededHistory::new(
            config(1, 1),
            vec![vec![
                HistoryEvent::Authorize {
                    attempt: 1,
                    snapshot: snapshot_with_value_bytes(MAX_HISTORY_SNAPSHOT_VALUE_BYTES),
                },
                HistoryEvent::Authorize {
                    attempt: 2,
                    snapshot: snapshot_with_value_bytes(MAX_HISTORY_SNAPSHOT_VALUE_BYTES),
                },
                HistoryEvent::Authorize {
                    attempt: 3,
                    snapshot: snapshot_with_value_bytes(MAX_HISTORY_SNAPSHOT_VALUE_BYTES),
                },
            ]],
        );
        assert!(exact_aggregate.is_ok());
        assert_eq!(
            SeededHistory::new(
                config(1, 1),
                vec![vec![
                    HistoryEvent::Authorize {
                        attempt: 1,
                        snapshot: snapshot_with_value_bytes(MAX_HISTORY_SNAPSHOT_VALUE_BYTES),
                    },
                    HistoryEvent::Authorize {
                        attempt: 2,
                        snapshot: snapshot_with_value_bytes(MAX_HISTORY_SNAPSHOT_VALUE_BYTES),
                    },
                    HistoryEvent::Authorize {
                        attempt: 3,
                        snapshot: snapshot_with_value_bytes(MAX_HISTORY_SNAPSHOT_VALUE_BYTES),
                    },
                    HistoryEvent::Authorize {
                        attempt: 4,
                        snapshot: snapshot_with_value_bytes(1),
                    },
                ]],
            ),
            Err(Error::Limit)
        );
    }
}
