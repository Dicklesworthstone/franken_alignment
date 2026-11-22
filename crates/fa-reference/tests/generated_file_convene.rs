//! Held native text is reviewed again without resampling or replacing its
//! original output. Small deterministic weights and controlled socket ballots
//! exercise custody and authority, not empirical detector reliability.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "generated_file_convene/fixture.rs"]
mod fixture;
#[path = "support/file_helper.rs"]
mod helper;
#[path = "tower_generated_operator/fixture.rs"]
#[allow(dead_code)]
mod native;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        Reconciliation, RecoveryReserve,
        observed::{
            FileHumanPermit, FileHumanReviewer, FileOversight, FileOversightProfile,
            driver::{
                FileDriverEvent, FileDriverPhase, FileSupervisedDriver, evidence::FileReviewLaunch,
            },
            stream::generated::FileTextMessageRequest,
        },
        requests::actor::{
            FileConveneBlockReason, FileConveneOutcome, FileConveneReceipt, FileConveneRequest,
            FileGeneratedTextActorPort, FileSupervisorConvenePort,
        },
    },
    stream::{ReleaseFrame, StreamProfile},
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorOutcome, Knowledge},
    evidence_source::{
        EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
    },
    helper_workers::HelperLimits,
};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fixture::{Native, command, document, receipt, replace};
use std::os::unix::net::UnixStream;
use support::{Directory, Supervisor, snapshot};

const REQUEST: u64 = 701;
const FINISH: u64 = 702;
const HELD_ROUND: u64 = 101;
const NEXT_ROUND: u64 = 102;
const ORIGINAL_CONTEXT: &[u8] = b"original independent native context";
const FRESH_CONTEXT: &[u8] = b"new independent native context after the hold";

#[test]
fn generated_message_convenes_once_and_publishes_original_bytes_without_resampling() {
    for changed_source in [false, true] {
        let mut case = Native::held();
        let actor_ticket = case.actor.submit(&case.original).unwrap();
        let (action, numerical, stream, input) = {
            let host = case.driver.supervisor().host().unwrap();
            (
                host.request_action(REQUEST).unwrap().clone(),
                host.decoder_inspection().unwrap().numerical,
                host.stream_snapshot().unwrap(),
                host.review_replay(HELD_ROUND)
                    .unwrap()
                    .application_inputs()
                    .unwrap()
                    .clone(),
            )
        };
        let (port, grant) = case.driver.supervisor().convene_port(4, 1).unwrap();
        let selected = command(&port, REQUEST, NEXT_ROUND);
        assert_eq!(selected.expected().held_round, HELD_ROUND);
        assert_eq!(selected.expected().deadline, action.spec().deadline);
        let image = case.image();
        let reads = case.source.status();
        for _ in 0..3 {
            assert!(matches!(
                case.driver
                    .step_from_file(
                        &mut case.source,
                        || panic!("held polling cannot read a clock or regenerate"),
                        None
                    )
                    .result,
                Ok(FileDriverEvent::Idle)
            ));
            assert!(matches!(
                case.actor.poll(&actor_ticket),
                Knowledge::Pending { request: REQUEST }
            ));
        }
        assert_eq!(case.source.status(), reads);
        assert_eq!(case.image(), image);
        let queued = receipt(&port, &selected);
        assert_eq!(queued.outcome(), FileConveneOutcome::Pending);
        assert_eq!(receipt(&port.clone(), &selected), queued);
        assert_eq!(grant.remaining_operations(), 0);
        assert_eq!(case.image(), image);
        if changed_source {
            replace(&case.path, &document(2, FRESH_CONTEXT));
        }
        let expected_generation = if changed_source { 2 } else { 1 };
        assert_eq!(case.convene(&selected).generation, expected_generation);
        assert!(case.source.status().read_attempts > reads.read_attempts);
        let opened = receipt(&port, &selected);
        assert_eq!(opened.outcome(), FileConveneOutcome::Opened);
        assert_eq!(opened.opened().unwrap().started_at(), ElapsedTick(1));
        case.complete(Verdict::Allow, Consequence::Continue);
        {
            let host = case.driver.supervisor().host().unwrap();
            assert_eq!(host.request_action(REQUEST).unwrap(), &action);
            assert_eq!(
                host.decoder_text_message_request(REQUEST).unwrap(),
                &case.original
            );
            assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
            assert_eq!(host.stream_snapshot().unwrap().confirmed, stream.confirmed);
            assert_eq!(host.inspect().executions, 0);
            let fresh = host.review_replay(NEXT_ROUND).unwrap();
            let fresh_input = fresh.application_inputs().unwrap();
            if changed_source {
                assert_ne!(fresh_input, &input);
            } else {
                assert_eq!(fresh_input, &input);
            }
            for view in fresh_input.views().values() {
                let context = if changed_source {
                    FRESH_CONTEXT
                } else {
                    ORIGINAL_CONTEXT
                };
                assert!(
                    view.actual_input()
                        .submitted_bytes()
                        .windows(context.len())
                        .any(|window| window == context)
                );
            }
        }
        let human = case.approve(REQUEST, 9001);
        case.publish(REQUEST, &human);
        let host = case.driver.supervisor().host().unwrap();
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(host.inspect().payload, b"A");
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 1);
        assert!(matches!(
            case.actor.poll(&actor_ticket),
            Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            }
        ));
        drop(host);
        let image = case.image();
        assert_eq!(receipt(&port, &selected), opened);
        assert_eq!(case.image(), image);
    }
}

#[test]
fn explicit_finish_convenes_against_the_confirmed_prefix_without_new_inference() {
    let mut case = Native::new();
    case.start(REQUEST, HELD_ROUND);
    case.complete(Verdict::Allow, Consequence::Continue);
    let human = case.approve(REQUEST, 9001);
    case.publish(REQUEST, &human);
    let (numerical, prefix, proposal) = {
        let host = case.driver.supervisor().host().unwrap();
        (
            host.decoder_inspection().unwrap().numerical,
            host.stream_snapshot().unwrap().confirmed,
            host.prepare_decoder_text_finish(REQUEST, FINISH, ElapsedTick(100))
                .unwrap(),
        )
    };
    case.driver
        .prepare_file_intake(&mut case.source, || ElapsedTick(1))
        .result
        .unwrap();
    let ticket = case
        .actor
        .finish(
            FINISH,
            proposal.target,
            proposal.expected_policy_epoch,
            proposal.deadline,
        )
        .unwrap();
    case.start(FINISH, 201);
    case.complete(Verdict::Abstain, Consequence::HoldEffect);
    let action = case
        .driver
        .supervisor()
        .host()
        .unwrap()
        .request_action(FINISH)
        .unwrap()
        .clone();
    assert!(
        ReleaseFrame::decode(&action.spec().payload)
            .unwrap()
            .is_finish()
    );
    let (port, grant) = case.driver.supervisor().convene_port(3, 1).unwrap();
    let selected = command(&port, FINISH, 202);
    receipt(&port, &selected);
    case.convene(&selected);
    case.complete(Verdict::Allow, Consequence::Continue);
    {
        let host = case.driver.supervisor().host().unwrap();
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(host.stream_snapshot().unwrap().confirmed, prefix);
        assert_eq!(
            host.decoder_text_finish_request(REQUEST, FINISH).unwrap(),
            proposal
        );
        assert_eq!(host.request_action(FINISH).unwrap(), &action);
    }
    let fresh_human = case.approve(FINISH, 9002);
    assert_eq!(
        case.driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .executions,
        1
    );
    case.publish(FINISH, &fresh_human);
    let host = case.driver.supervisor().host().unwrap();
    let finished = host.stream_snapshot().unwrap();
    assert_eq!(finished.confirmed.visible(), prefix.visible());
    assert_eq!(finished.confirmed.message_count(), prefix.message_count());
    assert!(finished.confirmed.finished());
    assert_eq!(finished.confirmed, finished.published);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(grant.remaining_operations(), 0);
    assert!(matches!(
        case.actor.poll(&ticket),
        Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
}

#[test]
fn fresh_native_congress_cannot_replace_a_rejected_independent_human_key() {
    let mut case = Native::held();
    let numerical = case
        .driver
        .supervisor()
        .host()
        .unwrap()
        .decoder_inspection()
        .unwrap()
        .numerical;
    let (port, _grant) = case.driver.supervisor().convene_port(2, 1).unwrap();
    let selected = command(&port, REQUEST, NEXT_ROUND);
    receipt(&port, &selected);
    case.convene(&selected);
    case.complete(Verdict::Allow, Consequence::Continue);
    let review = case
        .driver
        .request_human_approval_from_file(&mut case.source, 9001, ElapsedTick(31), || {
            ElapsedTick(1)
        })
        .result
        .unwrap();
    {
        let mut host = case.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        case.reviewer.reject(&mut host, revision, &review).unwrap();
    }
    assert!(matches!(
        case.driver
            .step_from_file(&mut case.source, || ElapsedTick(1), None)
            .result,
        Ok(FileDriverEvent::AwaitingHuman { request: REQUEST })
    ));
    let host = case.driver.supervisor().host().unwrap();
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
}

#[test]
fn missing_native_source_spends_the_operation_without_regenerating_or_retrying() {
    let mut case = Native::held();
    let numerical = case
        .driver
        .supervisor()
        .host()
        .unwrap()
        .decoder_inspection()
        .unwrap()
        .numerical;
    let original = case.original.clone();
    let (port, grant) = case.driver.supervisor().convene_port(3, 1).unwrap();
    let selected = command(&port, REQUEST, NEXT_ROUND);
    receipt(&port, &selected);
    let ticket = case.driver.take_pending_convene(REQUEST).unwrap().unwrap();
    let launch = case.launch(REQUEST, NEXT_ROUND);
    std::fs::remove_file(&case.path).unwrap();
    let reads = case.source.status().read_attempts;
    assert!(
        case.driver
            .start_file_convened_review(&mut case.source, ticket, launch, || ElapsedTick(1))
            .is_err()
    );
    assert_eq!(case.source.status().read_attempts, reads + 1);
    assert_eq!(grant.remaining_operations(), 0);
    let failed = receipt(&port, &selected);
    assert_eq!(failed.outcome(), FileConveneOutcome::Failed);
    assert!(failed.opened().is_none());
    replace(&case.path, &document(1, ORIGINAL_CONTEXT));
    let image = case.image();
    let reads = case.source.status();
    assert_eq!(receipt(&port, &selected), failed);
    assert!(case.driver.take_pending_convene(REQUEST).unwrap().is_none());
    assert!(matches!(
        case.driver
            .step_from_file(
                &mut case.source,
                || panic!("failed successor cannot automatically capture or sample"),
                None
            )
            .result,
        Ok(FileDriverEvent::Idle)
    ));
    assert_eq!(case.image(), image);
    assert_eq!(case.source.status(), reads);
    let host = case.driver.supervisor().host().unwrap();
    assert_eq!(
        host.decoder_text_message_request(REQUEST).unwrap(),
        &original
    );
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert!(host.review_replay(NEXT_ROUND).is_err());
}

#[test]
fn changed_native_predecessors_fail_before_source_or_clock_and_do_not_refill_admission() {
    for after_ticket in [false, true] {
        let mut case = Native::held();
        let (port, grant) = case.driver.supervisor().convene_port(3, 1).unwrap();
        let selected = command(&port, REQUEST, NEXT_ROUND);
        let mut ticket = None;
        if after_ticket {
            receipt(&port, &selected);
            ticket = case.driver.take_pending_convene(REQUEST).unwrap();
        }
        // The native clock transition changes the exact acknowledged predecessor,
        // while the original generation bytes and work remain identical.
        {
            let mut host = case.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            host.observe_time(revision, ElapsedTick(2)).unwrap();
        }
        let image = case.image();
        let reads = case.source.status();
        if let Some(ticket) = ticket {
            let launch = case.launch(REQUEST, NEXT_ROUND);
            assert!(
                case.driver
                    .start_file_convened_review(&mut case.source, ticket, launch, || panic!(
                        "stale native ticket must be checked before the clock"
                    ))
                    .is_err()
            );
            assert_eq!(
                receipt(&port, &selected).outcome(),
                FileConveneOutcome::Failed
            );
            assert_eq!(grant.remaining_operations(), 0);
        } else {
            assert!(port.convene(selected).is_err());
            assert_eq!(grant.remaining_operations(), 1);
        }
        assert_eq!(case.source.status(), reads);
        assert_eq!(case.image(), image);
        assert!(
            case.driver
                .supervisor()
                .host()
                .unwrap()
                .review_replay(NEXT_ROUND)
                .is_err()
        );
    }
}

#[test]
fn each_generated_hold_consumes_an_explicit_finite_operation_and_keeps_the_original_deadline() {
    let mut case = Native::held();
    let numerical = case
        .driver
        .supervisor()
        .host()
        .unwrap()
        .decoder_inspection()
        .unwrap()
        .numerical;
    let (port, grant) = case.driver.supervisor().convene_port(4, 1).unwrap();
    let selected = command(&port, REQUEST, NEXT_ROUND);
    receipt(&port, &selected);
    case.convene(&selected);
    case.complete(Verdict::Abstain, Consequence::HoldEffect);
    let image = case.image();
    let reads = case.source.status();
    let Knowledge::Known { value, .. } = port.next(REQUEST, NEXT_ROUND + 1).unwrap() else {
        panic!("the original second hold remains observable");
    };
    assert_eq!(value.basis().held_round, NEXT_ROUND);
    assert_eq!(value.basis().deadline, selected.expected().deadline);
    assert_eq!(
        value.block_reason(),
        Some(FileConveneBlockReason::OperationLimit)
    );
    assert!(value.convene_request().is_none());
    assert_eq!(grant.remaining_operations(), 0);
    assert!(case.driver.take_pending_convene(REQUEST).unwrap().is_none());
    assert_eq!(case.source.status(), reads);
    assert_eq!(case.image(), image);
    assert_eq!(
        case.driver
            .supervisor()
            .host()
            .unwrap()
            .decoder_inspection()
            .unwrap()
            .numerical,
        numerical
    );
    assert_eq!(
        case.driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .control
            .ledger
            .stages[&1],
        ActionState::Reviewing
    );
}

#[test]
fn later_native_generation_cannot_borrow_the_original_held_source_or_rebase_a_ticket() {
    use fa_reference::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;

    for (completed, after_ticket) in [(false, false), (true, false), (true, true)] {
        let mut case = Native::held();
        let original_numerical = case
            .driver
            .supervisor()
            .host()
            .unwrap()
            .decoder_inspection()
            .unwrap()
            .numerical;
        let original_action = case
            .driver
            .supervisor()
            .host()
            .unwrap()
            .request_action(REQUEST)
            .unwrap()
            .clone();
        let (port, grant) = case.driver.supervisor().convene_port(3, 1).unwrap();
        let mut taken = None;
        if after_ticket {
            let selected = command(&port, REQUEST, NEXT_ROUND);
            receipt(&port, &selected);
            taken = Some((
                selected,
                case.driver.take_pending_convene(REQUEST).unwrap().unwrap(),
            ));
        }
        {
            let mut host = case.driver.supervisor_mut().host_mut().unwrap();
            let request = host
                .decoder_text_generation(7)
                .unwrap()
                .command()
                .request()
                .clone();
            let command = FileTextGenerationCommand::new(
                8,
                original_numerical.actor_revision,
                original_numerical.position,
                request,
            )
            .unwrap();
            let revision = host.revision();
            host.begin_decoder_text(revision, command).unwrap();
            if completed {
                for generation_revision in 0..4 {
                    let revision = host.revision();
                    host.advance_decoder_text(revision, 8, generation_revision)
                        .unwrap();
                }
                assert!(host.decoder_text_progress(8).unwrap().is_complete());
                let current = host.decoder_inspection().unwrap().numerical;
                assert!(current.position > original_numerical.position);
                assert!(current.sampled_draws > original_numerical.sampled_draws);
            } else {
                assert!(!host.decoder_text_progress(8).unwrap().is_complete());
            }
        }
        let numerical = case
            .driver
            .supervisor()
            .host()
            .unwrap()
            .decoder_inspection()
            .unwrap()
            .numerical;
        let image = case.image();
        let reads = case.source.status();
        if let Some((selected, ticket)) = taken {
            let launch = case.launch(REQUEST, NEXT_ROUND);
            assert!(
                case.driver
                    .start_file_convened_review(&mut case.source, ticket, launch, || panic!(
                        "later generation must refuse before source or clock"
                    ))
                    .is_err()
            );
            assert_eq!(
                receipt(&port, &selected).outcome(),
                FileConveneOutcome::Failed
            );
            assert_eq!(grant.remaining_operations(), 0);
        } else {
            let Knowledge::Known { value, .. } = port.next(REQUEST, NEXT_ROUND).unwrap() else {
                panic!("the old held request is still a known historical request");
            };
            assert!(!value.can_convene());
            assert_eq!(
                value.block_reason(),
                Some(FileConveneBlockReason::StaleReview)
            );
            assert!(value.convene_request().is_none());
            assert_eq!(grant.remaining_operations(), 1);
        }
        assert_eq!(case.image(), image);
        assert_eq!(case.source.status(), reads);
        let host = case.driver.supervisor().host().unwrap();
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(host.request_action(REQUEST).unwrap(), &original_action);
        assert_eq!(
            host.decoder_text_message_request(REQUEST).unwrap(),
            &case.original
        );
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
        assert!(host.review_replay(NEXT_ROUND).is_err());
    }
}
