//! Live immutable native window registration, independent labels and recovery.
//! Synthetic reviews exercise protocol enforcement, not trained-helper quality.
#![cfg(unix)]

#[path = "support/file_andon_window.rs"]
mod fixture;
#[path = "support/file_decoder.rs"]
mod numerical;
#[path = "support/file_consistency.rs"]
mod prediction;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight,
    credibility::andon::FileAndonConfig,
    guarded::{
        FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
        evaluation::andon::FileAndonRecoveryRequirements,
    },
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileAndonWindowBasis, FileAndonWindowBlockReason, FileAndonWindowError, FileAndonWindowPlan,
    FileAndonWindowPlanResult, FileAndonWindowReceipt, FileAndonWindowRequest,
    FileAndonWindowResult, MAX_ANDON_WINDOW_QUERIES, MAX_ANDON_WINDOW_REPLAY_EVENTS,
};
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorProposal, Knowledge, UnknownReason,
};
use fa_reference::action::consequence::oversight::credibility::{Assessment, GroundTruth};
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, MAX_WINDOWS};
use fixture::*;

fn plan(result: FileAndonWindowPlanResult) -> FileAndonWindowPlan {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original window plan: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(value.can_register(), value.block_reason().is_none());
    assert_eq!(value.can_register(), value.request().is_some());
    if let Some(request) = value.request() {
        assert_eq!(request.expected(), basis);
        assert_eq!(request.expected_budget(), value.budget());
        assert_eq!(request.operation(), value.operation());
    }
    value
}

fn receipt(result: FileAndonWindowResult) -> FileAndonWindowReceipt {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original window receipt: {result:?}");
    };
    assert_eq!(value.after(), basis);
    let mut expected = value.before();
    expected.journal_revision += u64::from(value.registered());
    assert_eq!(value.after(), expected);
    value
}

fn proposal(host: &FileOversight) -> ActorProposal {
    ActorProposal {
        target: host.inspect().target,
        payload: b"visible".to_vec(),
        units: 16,
        deadline: ElapsedTick(1000),
        expected_policy_epoch: host.inspect().control.ledger.epoch,
    }
}

fn requirements(host: &FileOversight, windows: Vec<AndonBudget>) -> FileAndonRecoveryRequirements {
    let cut = host.inspect();
    FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: None,
                decoder: None,
                decoder_stop: None,
                source: None,
                identity: None,
                campaigns: None,
                credential: None,
            },
            effective_policy: profile(0).delivery.policy,
            credential_epoch: None,
            minimum: FileRecoveryFloor {
                journal_revision: cut.revision,
                control_sequence: cut.control.sequence,
                authority_epoch: cut.control.ledger.epoch,
            },
        },
        config(),
        windows,
        host.andon_source().unwrap().revision(),
    )
    .unwrap()
}

#[test]
fn missing_occurrence_window_unblocks_only_the_original_independent_label_and_recovers() {
    let (root, mut host, _, evaluator) = ready();
    review(&mut host, 1, Verdict::Hold, Verdict::Allow);
    assess(&mut host, &evaluator, 1, 1, GroundTruth::Benign);
    let original_incident = host.andon_ledger().unwrap().windows()[&1].clone();
    let original_receipt = host.andon_receipt(101).unwrap().clone();

    review(&mut host, 3, Verdict::Hold, Verdict::Allow);
    let ticket = host.evaluation_ticket(103).unwrap();
    let assessment = Assessment {
        origin: 3,
        evidence_id: [2; 32],
        truth: GroundTruth::Benign,
    };
    let bytes = root.bytes();
    let revision = host.revision();
    assert_eq!(
        evaluator.assess(&mut host, revision, &ticket, assessment.clone()),
        Err(JournalError::Contract(Error::Missing))
    );
    assert_eq!(root.bytes(), bytes);
    assert!(host.evaluation_history(103).unwrap().is_empty());
    let evaluation = host.credibility_report().unwrap();
    let actor_state = host.actor_snapshot().unwrap();
    let policy = host.current_policy().unwrap().clone();
    let before = host.inspect();
    assert_eq!(before.control.ledger.elapsed, Some(ElapsedTick(12)));
    let (actor, mut supervisor) = host.into_actor_gateway();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let (port, grant) = supervisor
        .andon_window_port(later_window(), 3, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let offered = plan(port.next(900));
    assert_eq!(offered.budget(), later_window());
    assert!(!offered.already_registered());
    assert!(offered.can_register());
    assert_eq!(root.bytes(), bytes);
    let input = offered.request().unwrap();
    let acknowledged = receipt(port.register(input));
    assert!(acknowledged.registered());
    assert_eq!(grant.remaining_operations(), 0);
    {
        let host = supervisor.host().unwrap();
        let mut expected = before.clone();
        expected.revision += 1;
        assert_eq!(host.inspect(), expected);
        assert_eq!(host.actor_snapshot().unwrap(), actor_state);
        assert_eq!(host.current_policy().unwrap(), &policy);
        assert_eq!(host.credibility_report().unwrap(), evaluation);
        assert!(host.evaluation_history(103).unwrap().is_empty());
        assert_eq!(
            host.andon_ledger().unwrap().windows()[&1],
            original_incident
        );
        assert_eq!(host.andon_receipt(101).unwrap(), &original_receipt);
        assert_eq!(host.andon_ledger().unwrap().windows()[&2].false_stops, 0);
        assert!(
            host.andon_ledger().unwrap().windows()[&2]
                .incident
                .is_none()
        );
    }
    let proposed = proposal(&supervisor.host().unwrap());
    assert!(matches!(
        actor.submit(701, &proposed),
        Err(ActorError::Unavailable)
    ));
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(
            evaluator
                .assess(&mut host, revision, &ticket, assessment.clone())
                .unwrap()
        );
        let second = &host.andon_ledger().unwrap().windows()[&2];
        assert_eq!(second.false_stops, 1);
        assert_eq!(second.total_cost, u128::from(u64::MAX));
        assert_eq!(second.incident.as_ref().unwrap().opened_by_round, 103);
        assert_eq!(
            host.andon_receipt(103).unwrap().confirmation().occurred_at,
            12
        );
        assert_eq!(
            host.andon_receipt(103).unwrap().confirmation().confirmed_at,
            12
        );
        assert_eq!(
            host.andon_ledger().unwrap().windows()[&1],
            original_incident
        );
    }
    let bytes = root.bytes();
    assert_eq!(receipt(port.clone().register(input)), acknowledged);
    assert_eq!(root.bytes(), bytes);

    // The new live inventory is independently retained by the original recovery
    // consumer. Bootstrap remains exactly the original one-window configuration.
    let host = supervisor.host().unwrap();
    let expected = requirements(&host, vec![initial_window(), acknowledged.budget()]);
    let missing = requirements(&host, vec![initial_window()]);
    let mut changed_window = acknowledged.budget();
    changed_window.false_stops_allowed += 1;
    let changed = requirements(&host, vec![initial_window(), changed_window]);
    let anchor = host.history_anchor().unwrap();
    let accounting = host.andon_ledger().unwrap().clone();
    let native = host.andon_receipt(103).unwrap().clone();
    let history =
        FileOversight::read_andon_guarded(&root.0, &profile(0), &expected, &protocol(), &anchor)
            .unwrap();
    assert_eq!(history.accounting, accounting);
    assert_eq!(root.bytes(), bytes);
    drop(host);
    drop(supervisor);
    for invalid in [&missing, &changed] {
        assert!(matches!(
            FileOversight::open_andon_guarded(&root.0, profile(0), invalid, &protocol(), &anchor),
            Err(JournalError::Contract(Error::Binding))
        ));
        assert_eq!(root.bytes(), bytes);
    }
    let (mut restored, roles) =
        FileOversight::open_andon_guarded(&root.0, profile(0), &expected, &protocol(), &anchor)
            .unwrap();
    assert_eq!(restored.andon_ledger().unwrap(), &accounting);
    assert_eq!(restored.andon_receipt(103).unwrap(), &native);
    assert!(!restored.clock_ready());
    let fresh_ticket = restored.evaluation_ticket(103).unwrap();
    let revision = restored.revision();
    assert_eq!(
        evaluator.assess(&mut restored, revision, &fresh_ticket, assessment.clone()),
        Err(JournalError::Contract(Error::Binding))
    );
    let bytes = root.bytes();
    let revision = restored.revision();
    assert!(
        !roles
            .evaluator
            .assess(&mut restored, revision, &fresh_ticket, assessment,)
            .unwrap()
    );
    assert_eq!(root.bytes(), bytes);
    assert!(!restored.clock_ready());
    assert_eq!(restored.andon_ledger().unwrap(), &accounting);
    assert_eq!(
        port.register(input),
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
    );
}

#[test]
fn exact_existing_window_is_read_only_even_with_no_replay_room_and_keeps_actor_snapshot() {
    let (root, mut host, _, _) = ready();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let before = host.inspect();
    let (actor, mut supervisor) = host.into_actor_gateway();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let (port, grant) = supervisor
        .andon_window_port(initial_window(), 2, 1)
        .unwrap();
    let bytes = root.bytes();
    let offered = plan(port.next(900));
    assert!(offered.can_register());
    assert!(offered.already_registered());
    let input = offered.request().unwrap();
    let acknowledged = receipt(port.register(input));
    assert!(!acknowledged.registered());
    assert_eq!(acknowledged.before(), acknowledged.after());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.bytes(), bytes);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), None);
    assert_eq!(
        plan(port.next(900)).block_reason(),
        Some(FileAndonWindowBlockReason::OperationAlreadyUsed)
    );
    let proposed = proposal(&supervisor.host().unwrap());
    actor.submit(701, &proposed).unwrap();
    let bytes = root.bytes();
    assert_eq!(receipt(port.clone().register(input)), acknowledged);
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn every_independent_predecessor_coordinate_and_operation_identity_is_enforced() {
    let (root, mut host, _, _) = ready();
    host.observe_time(host.revision(), ElapsedTick(12)).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .andon_window_port(later_window(), 3, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let offered = plan(port.next(900));
    let bytes = root.bytes();
    for coordinate in 0..14 {
        let mut expected = offered.basis();
        match coordinate {
            0 => expected.journal_revision += 1,
            1 => expected.control_sequence += 1,
            2 => expected.authority_epoch += 1,
            3 => expected.evaluation_revision += 1,
            4 => expected.congress_generation += 1,
            5 => expected.policy_generation += 1,
            6 => expected.recorded_at = None,
            7 => expected.accounting.deployment += 1,
            8 => expected.accounting.evaluator += 1,
            9 => expected.scope.tenant += 1,
            10 => expected.scope.principal += 1,
            11 => expected.scope.run += 1,
            12 => expected.scope.branch += 1,
            13 => expected.scope.authority += 1,
            _ => unreachable!(),
        }
        let observed =
            port.register(FileAndonWindowRequest::new(900, expected, offered.budget()).unwrap());
        if coordinate >= 9 {
            assert_eq!(
                observed,
                Ok(Knowledge::Withheld {
                    authority_required: "supervisor_andon_window",
                })
            );
        } else {
            assert_eq!(observed, Err(FileAndonWindowError::StalePredecessor));
        }
        assert_eq!(root.bytes(), bytes);
        assert_eq!(grant.remaining_operations(), 1);
    }
    let input = offered.request().unwrap();
    let acknowledged = receipt(port.register(input));
    let bytes = root.bytes();
    assert_eq!(receipt(port.clone().register(input)), acknowledged);
    let mut changed = input.expected();
    changed.journal_revision += 1;
    assert_eq!(
        port.register(FileAndonWindowRequest::new(900, changed, input.expected_budget()).unwrap()),
        Err(FileAndonWindowError::IdempotencyConflict)
    );
    assert_eq!(
        port.register(
            FileAndonWindowRequest::new(901, input.expected(), input.expected_budget()).unwrap()
        ),
        Err(FileAndonWindowError::Capacity)
    );
    assert_eq!(
        plan(port.next(900)).block_reason(),
        Some(FileAndonWindowBlockReason::OperationAlreadyUsed)
    );
    assert_eq!(
        plan(port.next(901)).block_reason(),
        Some(FileAndonWindowBlockReason::OperationLimit)
    );
    assert_eq!(root.bytes(), bytes);
    grant.revoke();
    assert_eq!(
        port.register(input),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_andon_window",
        })
    );
}

#[test]
fn price_allowance_and_overlap_refusals_preserve_the_original_accounting_and_operation() {
    let (root, mut host, _, evaluator) = ready();
    review(&mut host, 1, Verdict::Hold, Verdict::Allow);
    assess(&mut host, &evaluator, 1, 1, GroundTruth::Benign);
    let accounting = host.andon_ledger().unwrap().clone();
    let (_, supervisor) = host.into_actor_gateway();
    for dimension in 0..5 {
        let mut budget = initial_window();
        let reason = if dimension == 4 {
            budget.window = 3;
            budget.start = 9;
            budget.end = 11;
            FileAndonWindowBlockReason::WindowOverlap
        } else {
            match dimension {
                0 => budget.false_stops_allowed += 1,
                1 => budget.cost_per_stop += 1,
                2 => budget.start += 1,
                3 => budget.end += 1,
                _ => unreachable!(),
            }
            FileAndonWindowBlockReason::WindowConflict
        };
        let (port, grant) = supervisor
            .andon_window_port(budget, 1, MAX_ANDON_WINDOW_REPLAY_EVENTS)
            .unwrap();
        let bytes = root.bytes();
        let offered = plan(port.next(900));
        assert_eq!(offered.block_reason(), Some(reason));
        let input = FileAndonWindowRequest::new(900, offered.basis(), offered.budget()).unwrap();
        assert_eq!(
            port.register(input),
            Err(if dimension == 4 {
                FileAndonWindowError::Unsupported
            } else {
                FileAndonWindowError::IdempotencyConflict
            })
        );
        assert_eq!(grant.remaining_operations(), 1);
        assert_eq!(
            supervisor.host().unwrap().andon_ledger().unwrap(),
            &accounting
        );
        assert_eq!(root.bytes(), bytes);
    }
    let zero = AndonBudget {
        false_stops_allowed: 0,
        cost_per_stop: 0,
        ..later_window()
    };
    let (port, grant) = supervisor
        .andon_window_port(zero, 1, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let input = plan(port.next(901)).request().unwrap();
    assert_eq!(receipt(port.register(input)).budget(), zero);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(
        supervisor.host().unwrap().andon_ledger().unwrap().windows()[&1],
        accounting.windows()[&1]
    );
}

#[test]
fn complete_window_inventory_capacity_keeps_exact_existing_noop_available() {
    let root = Directory::new();
    let (mut host, _) = FileOversight::create(&root.0, profile(0)).unwrap();
    let _evaluator = host
        .enable_credibility(host.revision(), protocol())
        .unwrap();
    let windows: Vec<_> = (0..MAX_WINDOWS)
        .map(|i| AndonBudget {
            window: i as u64 + 1,
            start: i as u64 * 10,
            end: (i as u64 + 1) * 10,
            false_stops_allowed: 0,
            cost_per_stop: 0,
        })
        .collect();
    let full = FileAndonConfig::new(identity(), windows.clone()).unwrap();
    host.enable_andon(host.revision(), &full).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let budget = AndonBudget {
        window: MAX_WINDOWS as u64 + 1,
        start: MAX_WINDOWS as u64 * 10,
        end: (MAX_WINDOWS as u64 + 1) * 10,
        false_stops_allowed: 0,
        cost_per_stop: 0,
    };
    let (port, grant) = supervisor
        .andon_window_port(budget, 1, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let bytes = root.bytes();
    let offered = plan(port.next(900));
    assert_eq!(
        offered.block_reason(),
        Some(FileAndonWindowBlockReason::WindowLimit)
    );
    assert_eq!(
        port.register(FileAndonWindowRequest::new(900, offered.basis(), offered.budget()).unwrap()),
        Err(FileAndonWindowError::Capacity)
    );
    assert_eq!(grant.remaining_operations(), 1);
    let (same, _grant) = supervisor.andon_window_port(windows[0], 1, 1).unwrap();
    assert!(!receipt(same.register(plan(same.next(901)).request().unwrap())).registered());
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn exact_history_work_bound_and_shared_queries_cannot_be_replenished() {
    for enough in [false, true] {
        let (root, host, _, _) = ready();
        let events = usize::try_from(host.revision()).unwrap() + usize::from(enough);
        let (_, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .andon_window_port(later_window(), 1, events)
            .unwrap();
        assert_eq!(port.next(0), Err(FileAndonWindowError::MalformedRequest));
        assert_eq!(grant.remaining_queries(), 1);
        let clone = port.clone();
        let offered = plan(port.next(900));
        assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(grant.max_replay_events(), events);
        assert_eq!(clone.next(901), Err(FileAndonWindowError::Capacity));
        let bytes = root.bytes();
        let input = FileAndonWindowRequest::new(900, offered.basis(), offered.budget()).unwrap();
        if enough {
            assert!(offered.can_register());
            let acknowledged = receipt(clone.register(input));
            assert_eq!(receipt(port.register(input)), acknowledged);
            assert_eq!(grant.remaining_operations(), 0);
        } else {
            assert_eq!(
                offered.block_reason(),
                Some(FileAndonWindowBlockReason::ReplayLimit)
            );
            assert_eq!(clone.register(input), Err(FileAndonWindowError::Capacity));
            assert_eq!(port.register(input), Err(FileAndonWindowError::Capacity));
            assert_eq!(grant.remaining_operations(), 1);
            assert_eq!(root.bytes(), bytes);
        }
    }
}

#[test]
fn revoked_borrowed_replaced_and_dropped_owner_never_supplies_a_new_window_owner() {
    for mode in 0..4 {
        let (root, host, _, _) = ready();
        let (actor, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor
            .andon_window_port(later_window(), 2, MAX_ANDON_WINDOW_REPLAY_EVENTS)
            .unwrap();
        let input = plan(port.next(900)).request().unwrap();
        let bytes = root.bytes();
        match mode {
            0 => {
                grant.revoke();
                assert_eq!(
                    port.next(900),
                    Ok(Knowledge::Withheld {
                        authority_required: "supervisor_andon_window",
                    })
                );
                assert_eq!(
                    port.register(input),
                    Ok(Knowledge::Withheld {
                        authority_required: "supervisor_andon_window",
                    })
                );
            }
            1 => {
                drop(supervisor);
                let _actor = actor;
                assert_eq!(
                    port.next(900),
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable,
                    })
                );
                assert_eq!(
                    port.register(input),
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable,
                    })
                );
                assert_eq!(grant.remaining_operations(), 1);
                assert_eq!(grant.remaining_queries(), 1);
            }
            2 => {
                let host = supervisor.host_mut().unwrap();
                assert_eq!(
                    port.next(900),
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable,
                    })
                );
                assert_eq!(
                    port.register(input),
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable,
                    })
                );
                assert_eq!(grant.remaining_operations(), 1);
                assert_eq!(grant.remaining_queries(), 1);
                drop(host);
                assert_eq!(root.bytes(), bytes);
                receipt(port.register(input));
                continue;
            }
            3 => {
                let (foreign_root, foreign, _, _) = ready();
                assert_eq!(foreign.inspect(), supervisor.host().unwrap().inspect());
                let foreign_bytes = foreign_root.bytes();
                *supervisor.host_mut().unwrap() = foreign;
                assert_eq!(
                    port.next(900),
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable,
                    })
                );
                assert_eq!(
                    port.register(input),
                    Ok(Knowledge::Unknown {
                        reason: UnknownReason::ControllerUnavailable,
                    })
                );
                assert_eq!(foreign_root.bytes(), foreign_bytes);
                assert_eq!(grant.remaining_operations(), 1);
            }
            _ => unreachable!(),
        }
        assert_eq!(root.bytes(), bytes);
    }
}

#[test]
fn ordinary_stop_suspension_and_clock_fence_preserve_native_accounting_registration() {
    for mode in 0..4 {
        let root = Directory::new();
        let mut p = profile(0);
        p.delivery.congress.narrow_at = 15;
        p.delivery.congress.suspend_at = 20;
        let (mut host, _) = FileOversight::create(&root.0, p).unwrap();
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
            .unwrap();
        let _evaluator = host
            .enable_credibility(host.revision(), protocol())
            .unwrap();
        host.enable_andon(host.revision(), &config()).unwrap();
        match mode {
            0 => assert!(!host.clock_ready()),
            1 => {
                host.observe_time(host.revision(), ElapsedTick(12)).unwrap();
                host.fence(host.revision()).unwrap();
                assert!(!host.clock_ready());
            }
            2 => {
                host.observe_time(host.revision(), ElapsedTick(12)).unwrap();
                let cut = host.inspect();
                host.request_stop(
                    host.revision(),
                    StopRequest {
                        operation: 77,
                        expected_control_sequence: cut.control.sequence,
                        expected_authority_epoch: cut.control.ledger.epoch,
                    },
                )
                .unwrap();
                assert!(host.inspect().stop.is_some());
            }
            3 => {
                review(&mut host, 1, Verdict::Hold, Verdict::Hold);
                assert!(host.inspect().control.suspended);
            }
            _ => unreachable!(),
        }
        let before = host.inspect();
        let ready = host.clock_ready();
        let report = host.credibility_report().unwrap();
        let (_, supervisor) = host.into_actor_gateway();
        let (port, _grant) = supervisor
            .andon_window_port(later_window(), 1, MAX_ANDON_WINDOW_REPLAY_EVENTS)
            .unwrap();
        let offered = plan(port.next(900));
        assert!(offered.can_register());
        receipt(port.register(offered.request().unwrap()));
        let host = supervisor.host().unwrap();
        let mut expected = before;
        expected.revision += 1;
        assert_eq!(host.inspect(), expected);
        assert_eq!(host.clock_ready(), ready);
        assert_eq!(host.credibility_report().unwrap(), report);
    }
}

#[test]
fn malformed_or_unbound_provisioning_never_installs_the_native_role() {
    for credible in [false, true] {
        let root = Directory::new();
        let (mut host, _) = FileOversight::create(&root.0, profile(0)).unwrap();
        if credible {
            let _evaluator = host
                .enable_credibility(host.revision(), protocol())
                .unwrap();
        }
        let bytes = root.bytes();
        let (_, supervisor) = host.into_actor_gateway();
        assert!(
            supervisor
                .andon_window_port(later_window(), 1, MAX_ANDON_WINDOW_REPLAY_EVENTS,)
                .is_err()
        );
        assert_eq!(root.bytes(), bytes);
    }
    let (root, host, _, _) = ready();
    let (_, supervisor) = host.into_actor_gateway();
    let bytes = root.bytes();
    for budget in [
        AndonBudget {
            window: 0,
            ..later_window()
        },
        AndonBudget {
            start: 30,
            ..later_window()
        },
        AndonBudget {
            start: 31,
            ..later_window()
        },
    ] {
        assert!(supervisor.andon_window_port(budget, 1, 64).is_err());
    }
    for (queries, events) in [
        (0, 1),
        (1, 0),
        (MAX_ANDON_WINDOW_QUERIES + 1, 1),
        (1, MAX_ANDON_WINDOW_REPLAY_EVENTS + 1),
    ] {
        assert!(
            supervisor
                .andon_window_port(later_window(), queries, events)
                .is_err()
        );
    }
    for (events, numerical) in [(1, 0), (1, 2)] {
        assert!(
            supervisor
                .andon_window_port_with_numerical_budget(later_window(), 1, events, numerical,)
                .is_err()
        );
    }
    let (port, _grant) = supervisor.andon_window_port(later_window(), 1, 64).unwrap();
    let basis = plan(port.next(900)).basis();
    assert_eq!(
        FileAndonWindowRequest::new(0, basis, later_window()),
        Err(FileAndonWindowError::MalformedRequest)
    );
    for dimension in 0..9 {
        let mut expected: FileAndonWindowBasis = basis;
        match dimension {
            0 => expected.scope.tenant = 0,
            1 => expected.scope.principal = 0,
            2 => expected.scope.run = 0,
            3 => expected.scope.branch = 0,
            4 => expected.scope.authority = 0,
            5 => expected.accounting.deployment = 0,
            6 => expected.accounting.evaluator = 0,
            7 => expected.policy_generation = 0,
            8 => expected.congress_generation = 0,
            _ => unreachable!(),
        }
        assert_eq!(
            FileAndonWindowRequest::new(900, expected, later_window()),
            Err(FileAndonWindowError::MalformedRequest)
        );
    }
    for budget in [
        AndonBudget {
            window: 0,
            ..later_window()
        },
        AndonBudget {
            start: 30,
            ..later_window()
        },
        AndonBudget {
            end: 9,
            ..later_window()
        },
    ] {
        assert_eq!(
            FileAndonWindowRequest::new(900, basis, budget),
            Err(FileAndonWindowError::MalformedRequest)
        );
    }
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn entered_storage_failure_spends_the_operation_without_a_second_native_attempt() {
    let (root, mut host, _, _) = ready();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let (port, grant) = supervisor
        .andon_window_port(later_window(), 2, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let input = plan(port.next(900)).request().unwrap();
    let bytes = root.bytes();
    let pending = root.0.join("delivery.pending");
    std::fs::create_dir(&pending).unwrap();
    assert_eq!(port.register(input), Err(FileAndonWindowError::Unavailable));
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(root.bytes(), bytes);
    std::fs::remove_dir(&pending).unwrap();
    assert!(matches!(
        port.clone().register(input),
        Err(FileAndonWindowError::Unavailable) | Ok(Knowledge::Unknown { .. })
    ));
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(root.bytes(), bytes);
    let proposed = proposal(&supervisor.host().unwrap());
    assert!(matches!(
        actor.submit(701, &proposed),
        Err(ActorError::Unavailable)
    ));
}

#[test]
fn numerical_history_needs_explicit_complete_allowance_and_retains_original_pause_gate() {
    for mode in 0..4 {
        let (root, mut host, _, _) = ready();
        host.enable_decoder(host.revision(), numerical::configuration(3.0))
            .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        numerical::forced(&mut host, 0);
        if mode == 3 {
            host.fence(host.revision()).unwrap();
            assert!(host.decoder_inspection().unwrap().paused);
        }
        let numerical_before = host.decoder_inspection().unwrap();
        let actor_before = host.actor_snapshot().unwrap();
        let needed = usize::try_from(host.revision()).unwrap() + 1;
        let (_, supervisor) = host.into_actor_gateway();
        let (port, grant) = if mode == 0 {
            supervisor
                .andon_window_port(later_window(), 1, needed)
                .unwrap()
        } else {
            supervisor
                .andon_window_port_with_numerical_budget(
                    later_window(),
                    1,
                    needed,
                    if mode == 1 { needed - 1 } else { needed },
                )
                .unwrap()
        };
        let allowance = grant.remaining_numerical_replay_events();
        let offered = plan(port.next(900));
        let bytes = root.bytes();
        let input = FileAndonWindowRequest::new(900, offered.basis(), offered.budget()).unwrap();
        if mode == 2 {
            assert!(offered.can_register());
            let acknowledged = receipt(port.register(input));
            assert_eq!(grant.remaining_operations(), 0);
            assert_eq!(grant.remaining_numerical_replay_events(), Some(0));
            let bytes = root.bytes();
            assert_eq!(receipt(port.clone().register(input)), acknowledged);
            assert_eq!(root.bytes(), bytes);
        } else {
            assert_eq!(
                offered.block_reason(),
                Some(match mode {
                    0 => FileAndonWindowBlockReason::NumericalReplayUnsupported,
                    1 => FileAndonWindowBlockReason::NumericalReplayLimit,
                    3 => FileAndonWindowBlockReason::NativeAdmissionUnavailable,
                    _ => unreachable!(),
                })
            );
            assert_eq!(
                port.register(input),
                Err(if mode == 1 {
                    FileAndonWindowError::Capacity
                } else {
                    FileAndonWindowError::Unsupported
                })
            );
            assert_eq!(grant.remaining_operations(), 1);
            assert_eq!(grant.remaining_numerical_replay_events(), allowance);
            assert_eq!(root.bytes(), bytes);
        }
        let host = supervisor.host().unwrap();
        let after = host.decoder_inspection().unwrap();
        assert_eq!(after.numerical, numerical_before.numerical);
        assert_eq!(after.paused, numerical_before.paused);
        assert_eq!(host.actor_snapshot().unwrap(), actor_before);
        drop(host);
        // Native exact-existing admission performs no replay, even while paused,
        // and cannot be turned into a numerical work grant.
        let (same, same_grant) = supervisor
            .andon_window_port(initial_window(), 1, 1)
            .unwrap();
        let bytes = root.bytes();
        assert!(!receipt(same.register(plan(same.next(901)).request().unwrap())).registered());
        assert_eq!(same_grant.remaining_numerical_replay_events(), None);
        assert_eq!(root.bytes(), bytes);
    }
}

#[test]
fn numerical_allowance_does_not_confer_action_predictor_custody() {
    let (root, mut host, _, _) = ready();
    let _observer = host
        .enable_action_consistency(host.revision(), prediction::configuration())
        .unwrap();
    let before = host.action_consistency_snapshot().unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .andon_window_port_with_numerical_budget(
            later_window(),
            1,
            MAX_ANDON_WINDOW_REPLAY_EVENTS,
            MAX_ANDON_WINDOW_REPLAY_EVENTS,
        )
        .unwrap();
    let bytes = root.bytes();
    let offered = plan(port.next(900));
    assert_eq!(
        offered.block_reason(),
        Some(FileAndonWindowBlockReason::NumericalReplayUnsupported)
    );
    assert_eq!(
        port.register(FileAndonWindowRequest::new(900, offered.basis(), offered.budget()).unwrap()),
        Err(FileAndonWindowError::Unsupported)
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(MAX_ANDON_WINDOW_REPLAY_EVENTS)
    );
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .action_consistency_snapshot()
            .unwrap(),
        before
    );
    let (same, _grant) = supervisor
        .andon_window_port(initial_window(), 1, 1)
        .unwrap();
    assert!(!receipt(same.register(plan(same.next(901)).request().unwrap())).registered());
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn interrupted_original_source_admission_preserves_the_operation_and_numerical_allowance() {
    use fa_reference::action::consequence::delivery::persistent::observed::source::{
        FileSourceError, FileSourcePolicy,
    };
    use fa_reference::action::consequence::oversight::evidence_source::{
        EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
    };
    use fa_reference::action::consequence::oversight::policy_state::{
        StateFreshness, StateLimits, StateSource,
    };

    let root = Directory::new();
    let mut p = profile(0);
    p.delivery.limits.bytes = 32_768;
    let scope = p.delivery.scope;
    let (mut host, _) = FileOversight::create(&root.0, p).unwrap();
    host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal())
        .unwrap();
    let _evaluator = host
        .enable_credibility(host.revision(), protocol())
        .unwrap();
    host.enable_andon(host.revision(), &config()).unwrap();
    host.enable_decoder(host.revision(), numerical::configuration(3.0))
        .unwrap();
    host.enable_file_source(
        host.revision(),
        FileSourcePolicy {
            source: StateSource {
                scope,
                source: 901,
                generation: 1,
            },
            limits: StateLimits::default(),
            freshness: StateFreshness::new(20).unwrap(),
        },
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let evidence = |generation, context: Vec<u8>| {
        EvidenceSnapshot::new(
            EvidenceIdentity {
                source: 901,
                generation,
                scope,
            },
            snapshot(),
            ["a", "b"]
                .into_iter()
                .map(|name| (name.to_owned(), context.clone()))
                .collect(),
        )
        .unwrap()
    };
    let path = root.0.join("evidence.json");
    std::fs::write(&path, evidence(1, b"original source".to_vec()).encode()).unwrap();
    let mut source =
        FileEvidenceSource::new(path.clone(), 901, scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    numerical::forced(&mut host, 0);
    let numerical_before = host.decoder_inspection().unwrap();
    let revision = host.revision();
    let bytes = root.bytes();
    // Actual original source acquisition succeeds, but its oversized canonical
    // event cannot fit. This leaves the native interrupted-source latch set.
    std::fs::write(&path, evidence(2, vec![7; 20_000]).encode()).unwrap();
    assert_eq!(
        host.refresh_file_source(revision, &mut source, ElapsedTick(2))
            .unwrap_err(),
        FileSourceError::Journal(JournalError::Contract(Error::Limit))
    );
    assert_eq!(host.revision(), revision);
    assert!(host.storage_failure().is_none());
    assert!(host.file_source_status().unwrap().interrupted);
    assert_eq!(root.bytes(), bytes);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .andon_window_port_with_numerical_budget(
            later_window(),
            2,
            MAX_ANDON_WINDOW_REPLAY_EVENTS,
            MAX_ANDON_WINDOW_REPLAY_EVENTS,
        )
        .unwrap();
    let offered = plan(port.next(900));
    assert_eq!(
        offered.block_reason(),
        Some(FileAndonWindowBlockReason::NativeAdmissionUnavailable)
    );
    assert_eq!(
        port.register(FileAndonWindowRequest::new(900, offered.basis(), offered.budget()).unwrap()),
        Err(FileAndonWindowError::Unsupported)
    );
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(MAX_ANDON_WINDOW_REPLAY_EVENTS)
    );
    let host = supervisor.host().unwrap();
    assert!(host.file_source_status().unwrap().interrupted);
    assert_eq!(host.decoder_inspection().unwrap(), numerical_before);
    assert_eq!(root.bytes(), bytes);
    drop(host);
    let (same, _grant) = supervisor
        .andon_window_port(initial_window(), 1, 1)
        .unwrap();
    assert!(!receipt(same.register(plan(same.next(901)).request().unwrap())).registered());
    assert!(
        supervisor
            .host()
            .unwrap()
            .file_source_status()
            .unwrap()
            .interrupted
    );
    assert_eq!(root.bytes(), bytes);

    std::fs::write(
        &path,
        evidence(3, b"renewed original source".to_vec()).encode(),
    )
    .unwrap();
    let mut fresh_reader =
        FileEvidenceSource::new(path, 901, scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.refresh_file_source(revision, &mut fresh_reader, ElapsedTick(2))
            .unwrap();
        assert!(!host.file_source_status().unwrap().interrupted);
    }
    let fresh = plan(port.next(900));
    assert!(fresh.can_register());
    receipt(port.register(fresh.request().unwrap()));
    assert_eq!(grant.remaining_operations(), 0);
    assert!(grant.remaining_numerical_replay_events().unwrap() < MAX_ANDON_WINDOW_REPLAY_EVENTS);
}

#[test]
fn original_owner_event_capacity_refuses_before_consuming_the_funded_operation() {
    let root = Directory::new();
    let mut p = profile(0);
    p.delivery.limits.events = 8;
    let (mut host, _) = FileOversight::create(&root.0, p).unwrap();
    let _evaluator = host
        .enable_credibility(host.revision(), protocol())
        .unwrap();
    host.enable_andon(host.revision(), &config()).unwrap();
    while host.revision() < 8 {
        host.observe_time(host.revision(), ElapsedTick(host.revision()))
            .unwrap();
    }
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .andon_window_port(later_window(), 1, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let bytes = root.bytes();
    let offered = plan(port.next(900));
    assert_eq!(
        offered.block_reason(),
        Some(FileAndonWindowBlockReason::ReplayLimit)
    );
    assert_eq!(
        port.register(FileAndonWindowRequest::new(900, offered.basis(), offered.budget()).unwrap()),
        Err(FileAndonWindowError::Capacity)
    );
    assert_eq!(grant.remaining_operations(), 1);
    let (same, _grant) = supervisor
        .andon_window_port(initial_window(), 1, 1)
        .unwrap();
    assert!(!receipt(same.register(plan(same.next(901)).request().unwrap())).registered());
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn same_owner_grants_cannot_execute_another_frozen_budget_plan() {
    let (root, host, _, _) = ready();
    let (_, supervisor) = host.into_actor_gateway();
    let selected_a = later_window();
    let selected_b = AndonBudget {
        window: 3,
        start: 30,
        end: 60,
        false_stops_allowed: 0,
        cost_per_stop: 0,
    };
    let (a, grant_a) = supervisor
        .andon_window_port(selected_a, 2, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let (b, grant_b) = supervisor
        .andon_window_port(selected_b, 2, MAX_ANDON_WINDOW_REPLAY_EVENTS)
        .unwrap();
    let plan_a = plan(a.next(900));
    let plan_b = plan(b.next(900));
    assert_eq!(plan_a.basis(), plan_b.basis());
    let request_a = plan_a.request().unwrap();
    let request_b = plan_b.request().unwrap();
    assert_eq!(request_a.expected_budget(), selected_a);
    assert_eq!(request_b.expected_budget(), selected_b);
    let bytes = root.bytes();
    assert_eq!(
        b.register(request_a),
        Err(FileAndonWindowError::IdempotencyConflict)
    );
    assert_eq!(
        a.register(request_b),
        Err(FileAndonWindowError::IdempotencyConflict)
    );
    for dimension in 0..5 {
        let mut assertion = selected_a;
        match dimension {
            0 => assertion.window += 10,
            1 => assertion.start += 1,
            2 => assertion.end += 1,
            3 => assertion.false_stops_allowed += 1,
            4 => assertion.cost_per_stop -= 1,
            _ => unreachable!(),
        }
        let changed = FileAndonWindowRequest::new(900, plan_a.basis(), assertion).unwrap();
        assert_eq!(
            a.register(changed),
            Err(FileAndonWindowError::IdempotencyConflict)
        );
        assert_eq!(grant_a.remaining_operations(), 1);
        assert_eq!(grant_b.remaining_operations(), 1);
        assert_eq!(root.bytes(), bytes);
    }
    assert_eq!(receipt(a.register(request_a)).budget(), selected_a);
    assert_eq!(grant_a.remaining_operations(), 0);
    // A real intervening registration also changes the original-owner cut.
    assert_eq!(
        b.register(request_b),
        Err(FileAndonWindowError::StalePredecessor)
    );
    assert_eq!(grant_b.remaining_operations(), 1);
    let current_b = plan(b.next(900)).request().unwrap();
    assert_eq!(receipt(b.register(current_b)).budget(), selected_b);
    assert_eq!(grant_b.remaining_operations(), 0);
    let host = supervisor.host().unwrap();
    assert_eq!(
        host.andon_ledger().unwrap().windows()[&2].budget,
        selected_a
    );
    assert_eq!(
        host.andon_ledger().unwrap().windows()[&3].budget,
        selected_b
    );
}
