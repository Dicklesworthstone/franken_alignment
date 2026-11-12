//! Explicit finite continuation of an original HoldEffect through real evidence,
//! helper sockets and independent human approval. No actor poll opens a round.
#![cfg(unix)]
#[path = "support/file_convene.rs"]
mod fixture;

use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{
    FileDriverEvent, FileDriverPhase,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileConveneBlockReason, FileConveneOutcome, FileConveneRequest,
};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fixture::*;

#[test]
fn explicit_same_source_convene_opens_once_then_original_two_keys_publish() {
    let mut file = held();
    let action = file
        .rig
        .driver
        .supervisor()
        .host()
        .unwrap()
        .request_action(REQUEST)
        .unwrap()
        .clone();
    let proposal = file.rig.proposal();
    let actor_ticket = file.rig.port.submit(REQUEST, &proposal).unwrap();
    let (port, grant) = file.rig.driver.supervisor().convene_port(4, 1).unwrap();
    let command = command(&port, NEXT_ROUND);
    assert_eq!(command.expected().held_round, HELD_ROUND);
    assert_eq!(command.expected().deadline, action.spec().deadline);
    let before = file.rig.driver.supervisor().host().unwrap().inspect();
    let bytes = image(&file);
    let reads = file.source.status();
    for _ in 0..3 {
        assert!(matches!(
            file.rig
                .driver
                .step_from_file(
                    &mut file.source,
                    || panic!("held poll must not sample time"),
                    None,
                )
                .result,
            Ok(FileDriverEvent::Idle)
        ));
        assert_eq!(
            file.rig.port.poll(&actor_ticket),
            Knowledge::Pending { request: REQUEST }
        );
    }
    assert_eq!(file.source.status(), reads);
    assert_eq!(image(&file), bytes);
    assert_eq!(grant.remaining_operations(), 1);

    let queued = receipt(&port, &command);
    assert_eq!(queued.outcome(), FileConveneOutcome::Pending);
    assert!(queued.opened().is_none());
    assert_eq!(receipt(&port.clone(), &command), queued);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(file.source.status(), reads);
    assert_eq!(
        file.rig.driver.supervisor().host().unwrap().inspect(),
        before
    );
    assert_eq!(image(&file), bytes);
    assert_ne!(
        file.rig
            .driver
            .supervisor()
            .next_review_round(NEXT_ROUND)
            .unwrap(),
        NEXT_ROUND
    );

    assert_eq!(launch(&mut file, &command), document(1).identity());
    assert!(file.source.status().read_attempts > reads.read_attempts);
    let opened = receipt(&port, &command);
    assert_eq!(opened.outcome(), FileConveneOutcome::Opened);
    assert_eq!(
        opened.opened().unwrap().input_revision(),
        file.rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .input_revision(1)
            .unwrap()
    );
    assert_eq!(opened.opened().unwrap().started_at(), ElapsedTick(1));
    assert_eq!(
        file.rig.driver.phase(),
        FileDriverPhase::Reviewing { request: REQUEST }
    );
    assert_eq!(
        file.rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .request_action(REQUEST)
            .unwrap(),
        &action
    );
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
    finish_continue(&mut file);
    publish(&mut file);
    assert!(matches!(
        file.rig.port.poll(&actor_ticket),
        Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
    let completed = image(&file);
    assert_eq!(receipt(&port, &command), opened);
    assert_eq!(image(&file), completed);
}

#[test]
fn source_failure_spends_the_selected_round_and_cannot_be_retried_after_restoration() {
    let mut file = held();
    let (port, grant) = file.rig.driver.supervisor().convene_port(4, 1).unwrap();
    let command = command(&port, NEXT_ROUND);
    assert_eq!(
        receipt(&port, &command).outcome(),
        FileConveneOutcome::Pending
    );
    let ticket = file
        .rig
        .driver
        .take_pending_convene(REQUEST)
        .unwrap()
        .unwrap();
    let launch = new_launch(&mut file, NEXT_ROUND);
    std::fs::remove_file(&file.path).unwrap();
    let reads = file.source.status().read_attempts;
    assert!(
        file.rig
            .driver
            .start_file_convened_review(&mut file.source, ticket, launch, || ElapsedTick(1),)
            .is_err()
    );
    assert_eq!(file.source.status().read_attempts, reads + 1);
    assert_eq!(grant.remaining_operations(), 0);
    let failed = receipt(&port, &command);
    assert_eq!(failed.outcome(), FileConveneOutcome::Failed);
    assert!(failed.opened().is_none());
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    assert!(
        file.rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .review_replay(NEXT_ROUND)
            .is_err()
    );
    let state = file.rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(state.control.decisions[&1], Consequence::HoldEffect);
    assert_eq!(state.control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.executions, 0);
    replace(&file.path, &document(1));
    let before = image(&file);
    let reads = file.source.status();
    assert_eq!(receipt(&port, &command), failed);
    assert!(
        file.rig
            .driver
            .take_pending_convene(REQUEST)
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        file.rig
            .driver
            .step_from_file(
                &mut file.source,
                || panic!("failed operation must not retry"),
                None,
            )
            .result,
        Ok(FileDriverEvent::Idle)
    ));
    assert_eq!(file.source.status(), reads);
    assert_eq!(image(&file), before);
}

#[test]
fn abandoned_ticket_and_revoked_grant_cannot_refill_the_original_allowance() {
    let mut file = held();
    let (port, grant) = file.rig.driver.supervisor().convene_port(3, 1).unwrap();
    let command = command(&port, NEXT_ROUND);
    let before = image(&file);
    receipt(&port, &command);
    let ticket = file
        .rig
        .driver
        .take_pending_convene(REQUEST)
        .unwrap()
        .unwrap();
    drop(ticket);
    let abandoned = receipt(&port, &command);
    assert_eq!(abandoned.outcome(), FileConveneOutcome::Abandoned);
    assert!(abandoned.opened().is_none());
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(image(&file), before);
    assert!(
        file.rig
            .driver
            .take_pending_convene(REQUEST)
            .unwrap()
            .is_none()
    );
    assert_ne!(
        file.rig
            .driver
            .supervisor()
            .next_review_round(NEXT_ROUND)
            .unwrap(),
        NEXT_ROUND
    );
    grant.revoke();
    assert!(matches!(
        port.convene(command).unwrap(),
        Knowledge::Withheld { .. }
    ));
    assert!(file.rig.driver.supervisor().convene_port(3, 1).is_err());
    assert_ne!(
        file.rig
            .driver
            .supervisor()
            .next_review_round(NEXT_ROUND)
            .unwrap(),
        NEXT_ROUND
    );
    assert_eq!(image(&file), before);
}

#[test]
fn a_taken_ticket_cannot_cross_later_input_or_control_changes_before_launch() {
    for changed_control in [false, true] {
        let mut file = held();
        let (port, grant) = file.rig.driver.supervisor().convene_port(2, 1).unwrap();
        let command = command(&port, NEXT_ROUND);
        receipt(&port, &command);
        let ticket = file
            .rig
            .driver
            .take_pending_convene(REQUEST)
            .unwrap()
            .unwrap();
        let launch = new_launch(&mut file, NEXT_ROUND);
        {
            let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
            if changed_control {
                // A real independently completed review changes the shared
                // control predecessor without changing this held action.
                driver::helper::oversight::reviewed(&mut host, 8, b"other action");
            } else {
                let revision = host.revision();
                let input = host.input_revision(1).unwrap();
                host.inputs_unavailable(revision, 1, input).unwrap();
            }
        }
        let before = image(&file);
        let reads = file.source.status();
        assert!(
            file.rig
                .driver
                .start_file_convened_review(&mut file.source, ticket, launch, || panic!(
                    "stale ticket reached clock"
                ),)
                .is_err()
        );
        assert_eq!(file.source.status(), reads);
        assert_eq!(image(&file), before);
        assert_eq!(grant.remaining_operations(), 0);
        assert_eq!(
            receipt(&port, &command).outcome(),
            FileConveneOutcome::Failed
        );
        assert!(
            file.rig
                .driver
                .take_pending_convene(REQUEST)
                .unwrap()
                .is_none()
        );
        assert!(
            file.rig
                .driver
                .supervisor()
                .host()
                .unwrap()
                .review_replay(NEXT_ROUND)
                .is_err()
        );
    }
}

#[test]
fn forged_held_provenance_and_stale_native_cuts_never_read_or_change_the_owner() {
    for mutation in 0..7 {
        let file = held();
        let (port, grant) = file.rig.driver.supervisor().convene_port(2, 1).unwrap();
        let original = command(&port, NEXT_ROUND);
        let mut expected = original.expected();
        match mutation {
            0 => expected.request_generation += 1,
            1 => expected.attempt += 1,
            2 => expected.held_round += 500,
            3 => expected.journal_revision += 1,
            4 => expected.control_sequence += 1,
            5 => expected.input_revision += 1,
            _ => expected.scope.branch += 1,
        }
        let forged = FileConveneRequest::new(REQUEST, NEXT_ROUND, expected).unwrap();
        let before = image(&file);
        let reads = file.source.status();
        assert!(port.convene(forged).is_err(), "mutation {mutation}");
        assert_eq!(grant.remaining_operations(), 1);
        assert_eq!(file.source.status(), reads);
        assert_eq!(image(&file), before);
        assert_eq!(
            receipt(&port, &original).outcome(),
            FileConveneOutcome::Pending
        );
    }
    for mutation in 0..3 {
        let mut file = held();
        let (port, grant) = file.rig.driver.supervisor().convene_port(2, 1).unwrap();
        let original = command(&port, NEXT_ROUND);
        {
            let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            match mutation {
                0 => {
                    host.observe_time(revision, ElapsedTick(2)).unwrap();
                }
                1 => {
                    let input = host.input_revision(1).unwrap();
                    host.inputs_unavailable(revision, 1, input).unwrap();
                }
                _ => {
                    host.cancel_request(revision, REQUEST).unwrap();
                }
            }
        }
        let before = image(&file);
        let reads = file.source.status();
        assert!(port.convene(original).is_err(), "mutation {mutation}");
        assert_eq!(grant.remaining_operations(), 1);
        assert_eq!(file.source.status(), reads);
        assert_eq!(image(&file), before);
    }
}

#[test]
fn observation_budget_is_finite_and_round_idempotency_cannot_select_another_cut() {
    let file = held();
    let (port, grant) = file.rig.driver.supervisor().convene_port(1, 1).unwrap();
    let command = command(&port, NEXT_ROUND);
    let bytes = image(&file);
    assert_eq!(grant.remaining_queries(), 0);
    assert!(port.next(REQUEST, NEXT_ROUND + 1).is_err());
    assert_eq!(image(&file), bytes);
    let queued = receipt(&port, &command);
    assert_eq!(queued.outcome(), FileConveneOutcome::Pending);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(receipt(&port.clone(), &command), queued);
    let mut other = command.expected();
    other.journal_revision += 1;
    assert!(
        port.convene(FileConveneRequest::new(REQUEST, NEXT_ROUND, other).unwrap())
            .is_err()
    );
    assert!(
        port.convene(FileConveneRequest::new(REQUEST, NEXT_ROUND + 1, command.expected()).unwrap())
            .is_err()
    );
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(image(&file), bytes);
}

#[test]
fn each_repeated_hold_needs_another_explicit_operation_from_the_same_finite_grant() {
    let mut file = held();
    let (port, grant) = file.rig.driver.supervisor().convene_port(3, 2).unwrap();
    let first = command(&port, NEXT_ROUND);
    receipt(&port, &first);
    launch(&mut file, &first);
    let FileDriverEvent::ReviewApplied { receipt: held, .. } =
        file.finish(Verdict::Abstain).result.unwrap()
    else {
        panic!("second independently frozen hold missing");
    };
    assert_eq!(
        held.policy.control.decision.consequence,
        Consequence::HoldEffect
    );
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(grant.remaining_operations(), 1);
    let before = image(&file);
    let reads = file.source.status();
    assert_eq!(receipt(&port, &first).outcome(), FileConveneOutcome::Opened);
    assert!(
        file.rig
            .driver
            .take_pending_convene(REQUEST)
            .unwrap()
            .is_none()
    );
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status(), reads);

    let second = command(&port, NEXT_ROUND + 1);
    assert_eq!(second.expected().held_round, NEXT_ROUND);
    assert_eq!(second.expected().deadline, first.expected().deadline);
    receipt(&port, &second);
    launch(&mut file, &second);
    assert_eq!(grant.remaining_operations(), 0);
    finish_continue(&mut file);
    publish(&mut file);
}

#[test]
fn a_failed_later_congress_cannot_relabel_an_older_successful_hold_as_current() {
    let mut file = held();
    let launch = new_launch(&mut file, 303);
    file.rig
        .driver
        .start_file_review(&mut file.source, launch, || ElapsedTick(1))
        .unwrap();
    // The original workers committed to generation 1. Actual generation 2 at
    // completion must reject their Continue instead of silently rebasing votes.
    replace(&file.path, &document(2));
    assert!(matches!(
        file.finish(Verdict::Allow).result,
        Ok(FileDriverEvent::ReviewRejected { .. })
    ));
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(
        file.rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .control
            .decisions[&1],
        Consequence::HoldEffect
    );
    let (port, grant) = file.rig.driver.supervisor().convene_port(1, 1).unwrap();
    let before = image(&file);
    let reads = file.source.status();
    let Knowledge::Known { value, .. } = port.next(REQUEST, 304).unwrap() else {
        panic!("original native history should explain the stale held round");
    };
    assert_eq!(value.basis().held_round, HELD_ROUND);
    assert_eq!(
        value.block_reason(),
        Some(FileConveneBlockReason::StaleReview)
    );
    assert!(!value.can_convene());
    assert!(value.convene_request().is_none());
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(file.source.status(), reads);
    assert_eq!(image(&file), before);
}

#[test]
fn a_weak_port_cannot_retain_or_replace_the_original_controller() {
    let mut original = held();
    let mut foreign = held();
    let (port, grant) = original.rig.driver.supervisor().convene_port(4, 1).unwrap();
    let request = command(&port, NEXT_ROUND);
    receipt(&port, &request);
    let ticket = original
        .rig
        .driver
        .take_pending_convene(REQUEST)
        .unwrap()
        .unwrap();
    let launch = new_launch(&mut foreign, NEXT_ROUND);
    let before = image(&foreign);
    let reads = foreign.source.status();
    assert!(
        foreign
            .rig
            .driver
            .start_file_convened_review(&mut foreign.source, ticket, launch, || panic!(
                "foreign owner reached clock"
            ),)
            .is_err()
    );
    assert_eq!(foreign.source.status(), reads);
    assert_eq!(image(&foreign), before);
    assert_eq!(grant.remaining_operations(), 0);
    drop(original.rig.driver);
    assert!(matches!(
        port.next(REQUEST, NEXT_ROUND + 1).unwrap(),
        Knowledge::Unknown { .. }
    ));
    assert!(matches!(
        port.convene(request).unwrap(),
        Knowledge::Unknown { .. }
    ));
}
