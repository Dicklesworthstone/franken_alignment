//! Causal refusals on the same original owner and its actually spent work.
use super::*;

#[test]
fn failed_fresh_policy_read_spends_selected_roster_and_retires_unused_owners() {
    for malformed in [false, true] {
        let (mut case, run) = held(&[b"allow", b"allow"]);
        let numerical = run
            .supervisor()
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap()
            .numerical;
        let reservation = run.review().reservation();
        let polls = run.review().polls();
        let revision = run.review().revision();
        let (mut run, ticket, command) = case.queue(run, MESSAGE, NEXT);
        let host_revision = run.supervisor().host().unwrap().revision();
        let reads = case.source.status().read_attempts;
        if malformed {
            std::fs::write(
                case.root.store().with_extension("policy"),
                b"incomplete capture",
            )
            .unwrap();
        } else {
            // A real newer capture still cannot smuggle helper evidence into
            // the original policy-only source contract.
            let mut contexts = capture(2).contexts().clone();
            contexts.insert("alpha".to_owned(), b"replacement learned packet".to_vec());
            let bad = EvidenceSnapshot::new(capture(2).identity(), snapshot(), contexts).unwrap();
            publish(&case.root.store().with_extension("policy"), &bad);
        }
        let report =
            run.convene_reserved_from_policy_file(&mut case.source, ticket, window(), || {
                ElapsedTick(2)
            });
        assert!(report.result.is_err());
        assert_eq!(case.receipt(command).outcome(), FileConveneOutcome::Failed);
        assert_eq!(case.grant.remaining_operations(), 3);
        assert!(case.source.status().read_attempts > reads);
        assert_eq!(run.review().revision(), revision + 1);
        assert_eq!(run.review().pending_convene_rosters(), 0);
        assert_eq!(run.review().status(), NativeReviewStatus::Failed);
        assert_eq!(run.review().reservation(), reservation);
        assert_eq!(run.review().polls(), polls);
        assert_eq!(run.review().history().len(), 1);
        assert!(!run.review().records()[&NEXT].started);
        assert!(!case.plan(MESSAGE, 103).can_convene());
        let host = run.supervisor().host().unwrap();
        assert!(
            host.revision() > host_revision,
            "acknowledged fresh work is retained"
        );
        assert!(host.review_replay(NEXT).is_err());
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical
        );
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
}

#[test]
fn stale_ticket_and_changed_source_cut_are_refused_before_clock_or_file_work() {
    for changed_input in [false, true] {
        let (mut case, run) = held(&[b"allow"]);
        let (mut run, ticket, command) = case.queue(run, MESSAGE, NEXT);
        {
            let mut host = run.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            if changed_input {
                host.inputs_unavailable(
                    revision,
                    command.expected().attempt,
                    command.expected().input_revision,
                )
                .unwrap();
            } else {
                host.observe_time(revision, ElapsedTick(2)).unwrap();
            }
        }
        let reads = case.source.status().read_attempts;
        let polls = run.review().polls();
        let report =
            run.convene_reserved_from_policy_file(&mut case.source, ticket, window(), || {
                panic!("stale original ticket must fail before clock/source")
            });
        assert!(report.result.is_err());
        assert_eq!(case.source.status().read_attempts, reads);
        assert_eq!(case.receipt(command).outcome(), FileConveneOutcome::Failed);
        assert_eq!(run.review().pending_convene_rosters(), 1);
        assert_eq!(run.review().polls(), polls);
        assert!(
            run.supervisor()
                .host()
                .unwrap()
                .review_replay(NEXT)
                .is_err()
        );
        if changed_input {
            assert!(!case.plan(MESSAGE, 103).can_convene());
        }
    }
}

#[test]
fn foreign_original_ticket_cannot_spend_this_sequence_or_consult_its_sources() {
    let (mut case, run) = held(&[b"allow"]);
    let (other, foreign) = held(&[b"allow"]);
    let (_foreign_owner, ticket, command) = other.queue(foreign, MESSAGE, NEXT);
    let mut run = run;
    let reads = case.source.status().read_attempts;
    let revision = run.review().revision();
    let host_revision = run.supervisor().host().unwrap().revision();
    let report = run.convene_reserved_from_policy_file(&mut case.source, ticket, window(), || {
        panic!("foreign owner must fail before clock/source")
    });
    assert!(report.result.is_err());
    assert_eq!(run.review().revision(), revision);
    assert_eq!(run.review().pending_convene_rosters(), 1);
    assert_eq!(run.supervisor().host().unwrap().revision(), host_revision);
    assert_eq!(case.source.status().read_attempts, reads);
    assert_eq!(other.receipt(command).outcome(), FileConveneOutcome::Failed);
    assert_eq!(case.grant.remaining_operations(), 4);
    assert!(case.plan(MESSAGE, NEXT).can_convene());
}

#[test]
fn malformed_successor_window_does_no_source_work_and_cannot_open_a_round() {
    let (mut case, run) = held(&[b"allow"]);
    let (mut run, ticket, command) = case.queue(run, MESSAGE, NEXT);
    let reads = case.source.status().read_attempts;
    let report = run.convene_reserved_from_policy_file(
        &mut case.source,
        ticket,
        ReviewWindow {
            commit_by: ElapsedTick(20),
            reveal_by: ElapsedTick(20),
        },
        || panic!("malformed window must fail before clock/source"),
    );
    assert!(report.result.is_err());
    assert_eq!(case.source.status().read_attempts, reads);
    assert_eq!(case.receipt(command).outcome(), FileConveneOutcome::Failed);
    assert_eq!(run.review().pending_convene_rosters(), 1);
    assert!(
        run.supervisor()
            .host()
            .unwrap()
            .review_replay(NEXT)
            .is_err()
    );
}

#[test]
fn unstarted_refinement_ids_remain_leased_against_explicit_convening() {
    let (mut case, mut driver) = Case::new();
    case.admit(&mut driver, MESSAGE, LearnedTextRelease::Message);
    let mut launch = launch(&mut driver, MESSAGE, HELD, b"abstain");
    let second = reserves(&launch, &[b"allow"]).pop().unwrap();
    launch.rounds.push(LearnedWorkerRound {
        round: NEXT,
        evidence_root: [9; 32],
        window: window(),
    });
    launch.rosters.insert(NEXT, second);
    let run = driver
        .start_native_learned_sequence(launch, snapshot(), ElapsedTick(1))
        .unwrap();
    assert!(!run.review().records()[&NEXT].started);
    assert_eq!(run.supervisor().next_review_round(NEXT).unwrap(), 103);
    assert!(!case.plan(MESSAGE, NEXT).can_convene());
}
