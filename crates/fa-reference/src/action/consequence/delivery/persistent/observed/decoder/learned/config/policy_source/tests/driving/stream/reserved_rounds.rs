//! The original owner protects later fixed releases before admitting a dynamic
//! successor. The positive path still runs the original models and both keys.
use super::*;
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileConveneBlockReason, FileConveneError,
};

const FINISH_ROUND: u64 = 201;

#[test]
fn fixed_schedule_selection_is_atomic_bounded_and_cannot_reissue_a_grant() {
    let root = Directory::new();
    let (host, _reviewer) = FileOversight::create_with_learned_text_stream(
        root.store(),
        profile_stream(),
        configured_stream(),
    )
    .unwrap();
    let (_actor, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let revision = supervisor.host().unwrap().revision();
    for schedule in [vec![], vec![(0, HELD)], vec![(MESSAGE, 0)]] {
        assert!(matches!(
            supervisor.reserve_review_rounds(&schedule),
            Err(JournalError::Contract(Error::InvalidInput))
        ));
    }
    assert!(matches!(
        supervisor.reserve_review_rounds(&[(MESSAGE, HELD), (FINISH, HELD)]),
        Err(JournalError::Contract(Error::Duplicate))
    ));
    let too_many: Vec<_> = (1..=crate::action::consequence::oversight::MAX_OBSERVED_ROUNDS + 1)
        .map(|round| (MESSAGE, round as u64))
        .collect();
    assert!(matches!(
        supervisor.reserve_review_rounds(&too_many),
        Err(JournalError::Contract(Error::Limit))
    ));
    assert_eq!(supervisor.next_review_round(HELD).unwrap(), HELD);
    supervisor
        .reserve_review_rounds(&[(MESSAGE, HELD), (MESSAGE, NEXT), (FINISH, FINISH_ROUND)])
        .unwrap();
    assert!(!supervisor.has_live_convening().unwrap());
    assert_eq!(supervisor.next_review_round(HELD).unwrap(), NEXT + 1);
    assert!(matches!(
        supervisor.check_initial_review_rounds(MESSAGE, [HELD]),
        Err(JournalError::Contract(Error::Binding))
    ));
    supervisor
        .check_initial_review_rounds(MESSAGE, [HELD, NEXT])
        .unwrap();
    supervisor
        .check_initial_review_rounds(FINISH, [FINISH_ROUND])
        .unwrap();
    let (_port, grant) = supervisor.convene_port(64, 4).unwrap();
    assert_eq!(grant.remaining_operations(), 4);
    assert!(supervisor.has_live_convening().unwrap());
    grant.revoke();
    assert!(!supervisor.has_live_convening().unwrap());
    assert!(matches!(
        supervisor.convene_port(64, 4),
        Err(JournalError::Contract(Error::Duplicate))
    ));
    assert!(
        supervisor
            .reserve_review_rounds(&[(MESSAGE, NEXT + 1)])
            .is_err()
    );
    supervisor
        .check_initial_review_rounds(FINISH, [FINISH_ROUND])
        .unwrap();
    assert_eq!(supervisor.host().unwrap().revision(), revision);
}

#[test]
fn future_fixed_round_is_unspendable_while_fresh_successor_and_exact_finish_succeed() {
    let (mut case, mut driver) =
        Case::new_with_schedule(&[(MESSAGE, HELD), (FINISH, FINISH_ROUND)]);
    case.admit(&mut driver, MESSAGE, LearnedTextRelease::Message);
    let mut run = start_reserved(
        driver,
        MESSAGE,
        HELD,
        b"abstain",
        &[b"allow"],
        MAX_LEARNED_REVIEW_POLLS,
    );
    complete(&mut run, &mut case.source, 1);
    let polls = run.review().polls();
    let reservation = run.review().reservation();
    let input = run.review().input().clone();
    let revision = run.supervisor().host().unwrap().revision();
    let numerical = run
        .supervisor()
        .host()
        .unwrap()
        .learned_generation_inspection()
        .unwrap()
        .numerical;
    let reads = case.source.status().read_attempts;
    let operations = case.grant.remaining_operations();

    let blocked = case.plan(MESSAGE, FINISH_ROUND);
    assert_eq!(
        blocked.block_reason(),
        Some(FileConveneBlockReason::RoundAlreadyUsed)
    );
    assert!(blocked.convene_request().is_none());
    let forged = FileConveneRequest::new(MESSAGE, FINISH_ROUND, blocked.basis()).unwrap();
    assert_eq!(
        case.port.convene(forged),
        Err(FileConveneError::IdempotencyConflict)
    );
    assert_eq!(case.grant.remaining_operations(), operations);
    assert_eq!(case.source.status().read_attempts, reads);
    assert_eq!(run.review().polls(), polls);
    assert_eq!(run.review().reservation(), reservation);
    assert_eq!(run.supervisor().host().unwrap().revision(), revision);
    assert_eq!(
        run.supervisor().next_review_round(FINISH_ROUND).unwrap(),
        FINISH_ROUND + 1
    );

    let (mut run, ticket, command) = case.queue(run, MESSAGE, NEXT);
    run.convene_reserved_from_policy_file(&mut case.source, ticket, window(), || ElapsedTick(6))
        .result
        .unwrap();
    assert_eq!(case.receipt(command).outcome(), FileConveneOutcome::Opened);
    assert_eq!(case.grant.remaining_operations(), operations - 1);
    complete(&mut run, &mut case.source, 6);
    assert_eq!(
        run.review().records().keys().copied().collect::<Vec<_>>(),
        vec![HELD, NEXT]
    );
    assert_eq!(run.review().input(), &input);
    assert_eq!(run.review().reservation(), reservation);
    assert_eq!(
        run.supervisor()
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap()
            .numerical,
        numerical
    );
    assert!(matches!(
        tick(&mut run, &mut case.source, None, 6),
        FileDriverEvent::AwaitingHuman { request: MESSAGE }
    ));
    let message_key = approve(&mut run, &case.reviewer, &mut case.source, 9001, 6);
    assert!(matches!(
        tick(&mut run, &mut case.source, Some(&message_key), 6),
        FileDriverEvent::Dispatched { .. }
    ));
    tick(&mut run, &mut case.source, None, 6);
    tick(&mut run, &mut case.source, None, 6);

    let mut driver = run.into_handoff().unwrap().driver;
    // Keep the monotone clock at the original cut for this second release.
    // Intake observes its own source and keeps the selected fixed identity.
    let report = driver
        .supervisor_mut()
        .exchange_learned_text_stream_actor_from_policy_file(
            &mut case.wire,
            &document_stream(FINISH, LearnedTextRelease::Finish),
            &mut case.source,
            || ElapsedTick(6),
        )
        .unwrap();
    assert_eq!(
        report.response.result,
        Ok(Knowledge::Pending { request: FINISH })
    );
    assert!(report.intake.unwrap().result.is_ok());
    let launch = launch(&mut driver, FINISH, FINISH_ROUND, b"allow");
    let mut run = driver
        .start_native_learned_sequence(launch, snapshot(), ElapsedTick(6))
        .unwrap();
    complete(&mut run, &mut case.source, 6);
    assert_eq!(
        run.review().records().keys().copied().collect::<Vec<_>>(),
        vec![FINISH_ROUND]
    );
    assert!(
        run.step_from_policy_file(&mut case.source, || ElapsedTick(6), Some(&message_key))
            .result
            .is_err()
    );
    let finish_key = approve(&mut run, &case.reviewer, &mut case.source, 9002, 6);
    assert!(matches!(
        tick(&mut run, &mut case.source, Some(&finish_key), 6),
        FileDriverEvent::Dispatched { .. }
    ));
    tick(&mut run, &mut case.source, None, 6);
    tick(&mut run, &mut case.source, None, 6);
    let host = run.supervisor().host().unwrap();
    let stream = host.stream_snapshot().unwrap();
    assert!(stream.confirmed.finished() && stream.published.finished());
    assert_eq!(stream.confirmed.visible(), b"aa");
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(case.grant.remaining_operations(), operations - 1);
}

#[test]
fn own_fixed_successor_remains_available_with_the_complete_schedule_reserved() {
    let (mut case, mut driver) =
        Case::new_with_schedule(&[(MESSAGE, HELD), (MESSAGE, NEXT), (FINISH, FINISH_ROUND)]);
    case.admit(&mut driver, MESSAGE, LearnedTextRelease::Message);
    let mut launch = launch(&mut driver, MESSAGE, HELD, b"abstain");
    let second = reserves(&launch, &[b"allow"]).pop().unwrap();
    launch.rounds.push(LearnedWorkerRound {
        round: NEXT,
        evidence_root: [9; 32],
        window: window(),
    });
    launch.rosters.insert(NEXT, second);
    let mut run = driver
        .start_native_learned_sequence(launch, snapshot(), ElapsedTick(1))
        .unwrap();
    complete(&mut run, &mut case.source, 1);
    assert_eq!(run.held_successor_round().unwrap().round, NEXT);
    assert!(!case.plan(MESSAGE, FINISH_ROUND).can_convene());
    let (mut run, ticket, command) = case.queue(run, MESSAGE, NEXT);
    run.convene_from_policy_file(&mut case.source, ticket, || ElapsedTick(1))
        .result
        .unwrap();
    complete(&mut run, &mut case.source, 1);
    assert_eq!(case.receipt(command).outcome(), FileConveneOutcome::Opened);
    assert_eq!(
        run.review().records().keys().copied().collect::<Vec<_>>(),
        vec![HELD, NEXT]
    );
    assert!(matches!(run.review().history().last(),
        Some(FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. })
            if receipt.policy.control.decision.consequence == Consequence::Continue));
    assert!(matches!(
        tick(&mut run, &mut case.source, None, 1),
        FileDriverEvent::AwaitingHuman { request: MESSAGE }
    ));
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn initial_native_review_cannot_substitute_another_requests_reserved_round() {
    for wrong_round in [NEXT, FINISH_ROUND] {
        let (mut case, mut driver) =
            Case::new_with_schedule(&[(MESSAGE, HELD), (FINISH, FINISH_ROUND)]);
        case.admit(&mut driver, MESSAGE, LearnedTextRelease::Message);
        let launch = launch(&mut driver, MESSAGE, wrong_round, b"allow");
        let revision = driver.supervisor().host().unwrap().revision();
        let numerical = driver
            .supervisor()
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap()
            .numerical;
        let reads = case.source.status().read_attempts;
        let rejected = driver
            .start_native_learned_sequence(launch, snapshot(), ElapsedTick(99))
            .unwrap_err();
        assert!(matches!(
            rejected.error,
            JournalError::Contract(Error::Binding)
        ));
        let host = rejected.driver.supervisor().host().unwrap();
        assert_eq!(host.revision(), revision);
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical
        );
        assert!(host.review_replay(wrong_round).is_err());
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(case.source.status().read_attempts, reads);
        assert_eq!(case.grant.remaining_operations(), 4);
    }
}

#[test]
fn failed_and_abandoned_admitted_rounds_stay_spent_after_grant_revocation() {
    for failed in [false, true] {
        let (mut case, run) = held(&[b"allow"]);
        let (mut run, ticket, command) = case.queue(run, MESSAGE, NEXT);
        if failed {
            let report = run.convene_reserved_from_policy_file(
                &mut case.source,
                ticket,
                ReviewWindow {
                    commit_by: ElapsedTick(20),
                    reveal_by: ElapsedTick(20),
                },
                || panic!("a malformed window cannot consult the source or clock"),
            );
            assert!(report.result.is_err());
        } else {
            drop(ticket);
        }
        assert_eq!(
            case.receipt(command).outcome(),
            if failed {
                FileConveneOutcome::Failed
            } else {
                FileConveneOutcome::Abandoned
            }
        );
        assert_eq!(case.grant.remaining_operations(), 3);
        assert!(
            run.supervisor()
                .host()
                .unwrap()
                .review_replay(NEXT)
                .is_err()
        );
        let revision = run.supervisor().host().unwrap().revision();
        assert!(matches!(
            run.supervisor().check_initial_review_rounds(FINISH, [NEXT]),
            Err(JournalError::Contract(Error::Duplicate))
        ));
        assert_eq!(run.supervisor().next_review_round(NEXT).unwrap(), NEXT + 1);
        case.grant.revoke();
        assert!(matches!(
            run.supervisor().check_initial_review_rounds(FINISH, [NEXT]),
            Err(JournalError::Contract(Error::Duplicate))
        ));
        assert!(run.supervisor().convene_port(64, 4).is_err());
        assert_eq!(run.supervisor().host().unwrap().revision(), revision);
    }
}
