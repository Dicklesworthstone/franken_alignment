//! Required raw/owned prediction survives genuine held native re-review.
//! Real tiny models test authority, evidence and finite work, not calibration.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "predictive_learned_file_convene/fixture.rs"]
mod fixture;
#[path = "support/file_oversight.rs"]
mod legacy;
#[path = "mediated_predictive_learned_recovery/native_fixture.rs"]
mod native_fixture;
#[path = "support/learned_text_model.rs"]
mod numerical;
#[allow(dead_code)]
#[path = "learned_file_convene/fixture.rs"]
mod ordinary;

use fa_reference::Snapshot;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::activation::probe::LinearProbe;
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::{
    EndpointOutcome, StopRequest,
    persistent::{
        Reconciliation, RecoveryReserve,
        observed::{
            FileHumanPermit, FileHumanReviewer, FileOversight, FileOversightProfile,
            decoder::learned::{FileLearnedConfig, sidecar::FileLearnedSidecarFinish},
            driver::{
                FileDriverEvent, FileSupervisedDriver,
                native_learned::{
                    FileNativeDriverEvent, FileNativeDriverLaunch, FileNativeSupervisedDriver,
                },
            },
            helpers::learned::native::{
                NativeReviewLimits, NativeReviewMember, NativeReviewStatus,
            },
        },
        requests::{
            FileRequestDisposition,
            actor::{
                FileActorTicket, FileConveneOutcome, FileConveneReceipt, FileConveneRequest,
                FileConveneTicket, FileLearnedTextStreamActorPort, FileSupervisorConveneGrant,
                FileSupervisorConvenePort, LearnedTextRelease,
            },
        },
    },
    stream::StreamProfile,
};
use fa_reference::action::consequence::oversight::{
    ReviewWindow,
    actor::{ActorOutcome, Knowledge},
    decoder_monitoring::LearnedDecoderBindingLimits,
    evidence_source::{
        EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
    },
    learned_host::sidecar::{
        LearnedSidecarRequest,
        workers::{LearnedWorkerRound, MAX_LEARNED_REVIEW_POLLS},
    },
    learned_source::LearnedEvidenceLimits,
    sidecar::{
        SidecarCongressBudget, SidecarIdentity,
        receiver::native::{SidecarDecisionBasis, SidecarProbeQuery},
    },
};
use fa_reference::action::{ActionState, ElapsedTick};
use fixture::{Case, Mode, PredictorProof};
use ordinary::{blocked, command, receipt};
use std::collections::{BTreeMap, BTreeSet};

const REQUEST: u64 = 71;
const HELD_ROUND: u64 = 101;
const NEXT_ROUND: u64 = 102;

#[test]
fn required_predictors_reuse_the_exact_settled_message_or_finish_observation_and_both_keys() {
    for owned in [false, true] {
        for mode in [Mode::Fixed, Mode::Reserved] {
            for finish in [false, true] {
                let (mut case, port, grant) = Case::held(owned, mode, finish, false, &[b"allow"]);
                let proof = PredictorProof::capture(&case);
                assert_eq!(
                    proof.evidence.evidence.samples(),
                    if finish { 2 } else { 1 }
                );
                assert_eq!(proof.evidence.pending_attempt, None);
                assert!(!proof.evidence.coverage_lost);
                assert!(!proof.evidence.evidence.crossed());
                let (action, numerical, retained, stream, executions, charged) = {
                    let host = case.run.supervisor().host().unwrap();
                    (
                        host.request_action(REQUEST).unwrap().clone(),
                        host.learned_generation_inspection().unwrap().numerical,
                        host.retained_learned_sidecar(case.attempt).unwrap(),
                        host.stream_snapshot().unwrap(),
                        host.inspect().executions,
                        host.inspect().control.ledger.charged,
                    )
                };
                let first = case.run.review().records()[&HELD_ROUND].clone();
                let history = case.run.review().history()[0].archive().clone();
                let before = case.run.progress();
                let reads = case.source.status().read_attempts;
                assert_eq!(before.reservation.evaluations, 2);
                assert!(
                    case.run
                        .request_human_approval(9001, ElapsedTick(40), ElapsedTick(1))
                        .is_err()
                );
                let selected = command(&port, NEXT_ROUND);
                assert_eq!(selected.expected().input_revision, retained.input_revision);
                assert_eq!(selected.expected().deadline, action.spec().deadline);
                let image = case.image();
                let pending = receipt(&port, selected);
                assert_eq!(pending.outcome(), FileConveneOutcome::Pending);
                assert_eq!(receipt(&port.clone(), selected), pending);
                assert_eq!(grant.remaining_operations(), 3);
                assert_eq!(case.image(), image);
                let ticket = case.run.take_pending_convene().unwrap().unwrap();
                case.convene(ticket, ElapsedTick(1)).unwrap();
                let opened = receipt(&port, selected);
                assert_eq!(opened.outcome(), FileConveneOutcome::Opened);
                assert_eq!(case.source.status().read_attempts, reads + 1);
                assert_eq!(case.run.progress().reservation, before.reservation);
                assert_eq!(case.run.progress().polls, before.polls);
                assert_eq!(PredictorProof::capture(&case), proof);
                case.complete(Consequence::Continue, ElapsedTick(1));
                assert_eq!(case.run.review().history().len(), 2);
                assert_eq!(case.run.review().history()[0].archive(), &history);
                assert_eq!(case.run.review().records()[&HELD_ROUND], first);
                assert_eq!(case.run.progress().reservation, before.reservation);
                assert!(case.run.progress().polls > before.polls);
                assert_eq!(PredictorProof::capture(&case), proof);
                {
                    let host = case.run.supervisor().host().unwrap();
                    assert_eq!(host.request_action(REQUEST).unwrap(), &action);
                    let next = host.retained_learned_sidecar(case.attempt).unwrap();
                    assert_eq!(next.packet, retained.packet);
                    assert_eq!(next.actor_revision, retained.actor_revision);
                    assert_eq!(next.input_revision, retained.input_revision);
                    assert_eq!(
                        host.learned_generation_inspection().unwrap().numerical,
                        numerical
                    );
                    assert_eq!(host.stream_snapshot().unwrap().confirmed, stream.confirmed);
                    assert_eq!(host.inspect().executions, executions);
                    assert_eq!(host.inspect().control.ledger.charged, charged);
                    assert_eq!(host.inspect().control.ledger.reserved, 0);
                }
                let human = case.approve(ElapsedTick(1));
                case.publish(&human, ElapsedTick(1));
                assert_eq!(PredictorProof::capture(&case), proof);
                let host = case.run.supervisor().host().unwrap();
                assert_eq!(
                    host.learned_generation_inspection().unwrap().numerical,
                    numerical
                );
                assert_eq!(host.inspect().executions, executions + 1);
                assert_eq!(
                    host.inspect().control.ledger.charged,
                    charged + action.spec().units
                );
                assert_eq!(host.inspect().control.ledger.reserved, 0);
                let final_stream = host.stream_snapshot().unwrap();
                assert_eq!(final_stream.confirmed, final_stream.published);
                assert_eq!(final_stream.confirmed.visible(), b"OK");
                assert_eq!(final_stream.confirmed.message_count(), 1);
                assert_eq!(final_stream.confirmed.finished(), finish);
                drop(host);
                let image = case.image();
                assert_eq!(receipt(&port, selected), opened);
                assert_eq!(case.image(), image);
            }
        }
    }
}

#[test]
fn a_fresh_native_continue_cannot_replace_human_rejection_of_predictive_finish() {
    for owned in [false, true] {
        for mode in [Mode::Fixed, Mode::Reserved] {
            let (mut case, port, _grant) = Case::held(owned, mode, true, false, &[b"allow"]);
            let proof = PredictorProof::capture(&case);
            let (numerical, stream, ledger) = {
                let host = case.run.supervisor().host().unwrap();
                (
                    host.learned_generation_inspection().unwrap().numerical,
                    host.stream_snapshot().unwrap(),
                    host.inspect().control.ledger,
                )
            };
            receipt(&port, command(&port, NEXT_ROUND));
            let ticket = case.run.take_pending_convene().unwrap().unwrap();
            case.convene(ticket, ElapsedTick(1)).unwrap();
            case.complete(Consequence::Continue, ElapsedTick(1));
            let request = case
                .run
                .request_human_approval_from_policy_file(
                    &mut case.source,
                    9901,
                    ElapsedTick(40),
                    || ElapsedTick(1),
                )
                .result
                .unwrap();
            {
                let mut host = case.run.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                case.reviewer.reject(&mut host, revision, &request).unwrap();
            }
            assert!(matches!(
                case.tick(None, ElapsedTick(1)),
                FileDriverEvent::AwaitingHuman { request: REQUEST }
            ));
            assert_eq!(PredictorProof::capture(&case), proof);
            let host = case.run.supervisor().host().unwrap();
            assert_eq!(
                host.learned_generation_inspection().unwrap().numerical,
                numerical
            );
            assert_eq!(host.inspect().executions, 1);
            assert_eq!(host.inspect().control.ledger.charged, ledger.charged);
            assert_eq!(host.inspect().control.ledger.reserved, 0);
            assert_eq!(host.stream_snapshot().unwrap().confirmed, stream.confirmed);
            assert!(!host.stream_snapshot().unwrap().confirmed.finished());
        }
    }
}

#[test]
fn settled_observation_outlives_pending_forecast_ttl_without_reforecast_or_new_work() {
    for owned in [false, true] {
        for mode in [Mode::Fixed, Mode::Reserved] {
            let (mut case, port, _grant) = Case::held(owned, mode, false, false, &[b"allow"]);
            let proof = PredictorProof::capture(&case);
            receipt(&port, command(&port, NEXT_ROUND));
            let ticket = case.run.take_pending_convene().unwrap().unwrap();
            // The original ticket is checked at the held cut. Acquisition then
            // observes 12, past a forecast at 1 with pending age 10. Its proposal
            // settled at 1, and the action's original deadline remains 100.
            case.convene(ticket, ElapsedTick(12)).unwrap();
            case.complete(Consequence::Continue, ElapsedTick(12));
            let human = case.approve(ElapsedTick(12));
            case.publish(&human, ElapsedTick(12));
            assert_eq!(PredictorProof::capture(&case), proof);
        }
    }
}

#[test]
fn coverage_loss_stop_cancel_or_stale_input_revoke_taken_tickets_before_acquisition() {
    for owned in [false, true] {
        for mode in [Mode::Fixed, Mode::Reserved] {
            for change in 0..4 {
                let (mut case, port, grant) = Case::held(owned, mode, false, false, &[b"allow"]);
                let selected = command(&port, NEXT_ROUND);
                receipt(&port, selected);
                let ticket = case.run.take_pending_convene().unwrap().unwrap();
                match change {
                    0 => {
                        let mut host = case.run.supervisor_mut().host_mut().unwrap();
                        let revision = host.revision();
                        case.observer.unavailable(&mut host, revision).unwrap();
                        assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
                    }
                    1 => {
                        let mut host = case.run.supervisor_mut().host_mut().unwrap();
                        let state = host.inspect();
                        host.request_stop(
                            state.revision,
                            StopRequest {
                                operation: 7001,
                                expected_control_sequence: state.control.sequence,
                                expected_authority_epoch: state.control.ledger.epoch,
                            },
                        )
                        .unwrap();
                    }
                    2 => case.actor.cancel(&case.ticket).unwrap(),
                    _ => {
                        let mut host = case.run.supervisor_mut().host_mut().unwrap();
                        let revision = host.revision();
                        host.inputs_unavailable(
                            revision,
                            case.attempt,
                            selected.expected().input_revision,
                        )
                        .unwrap();
                    }
                }
                let proof = PredictorProof::capture(&case);
                let image = case.image();
                let reads = case.source.status();
                let progress = case.run.progress();
                let report = match mode {
                    Mode::Fixed => {
                        case.run
                            .convene_from_policy_file(&mut case.source, ticket, || {
                                panic!("revoked original predictive cut cannot acquire a clock")
                            })
                    }
                    Mode::Reserved => case.run.convene_reserved_from_policy_file(
                        &mut case.source,
                        ticket,
                        ReviewWindow {
                            commit_by: ElapsedTick(30),
                            reveal_by: ElapsedTick(35),
                        },
                        || panic!("revoked original predictive cut cannot acquire a clock"),
                    ),
                };
                assert!(report.result.is_err());
                assert!(report.observations.is_empty());
                assert!(report.source_updates.is_empty());
                assert_eq!(case.source.status(), reads);
                assert_eq!(case.image(), image);
                assert_eq!(case.run.progress(), progress);
                assert_eq!(PredictorProof::capture(&case), proof);
                assert_eq!(grant.remaining_operations(), 3);
                assert_ne!(
                    receipt(&port, selected).outcome(),
                    FileConveneOutcome::Opened
                );
                assert!(
                    case.run
                        .supervisor()
                        .host()
                        .unwrap()
                        .review_replay(NEXT_ROUND)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn actually_crossed_predictors_cannot_offer_or_convene_a_retained_native_hold() {
    for owned in [false, true] {
        for mode in [Mode::Fixed, Mode::Reserved] {
            let (case, port, grant) = Case::held(owned, mode, false, true, &[b"allow"]);
            let proof = PredictorProof::capture(&case);
            assert_eq!(proof.evidence.evidence.samples(), 1);
            assert!(proof.evidence.evidence.crossed());
            let image = case.image();
            let reads = case.source.status();
            let progress = case.run.progress();
            blocked(&port, NEXT_ROUND);
            assert!(case.run.held_successor_round().is_none());
            assert!(!case.run.has_reserved_successor());
            assert!(case.run.take_pending_convene().unwrap().is_none());
            assert_eq!(grant.remaining_operations(), 4);
            assert_eq!(case.source.status(), reads);
            assert_eq!(case.run.progress(), progress);
            assert_eq!(case.image(), image);
            assert_eq!(PredictorProof::capture(&case), proof);
            assert_eq!(
                case.run.supervisor().host().unwrap().inspect().executions,
                0
            );
        }
    }
}

#[test]
fn policy_capture_failure_retires_native_reserves_without_refreshing_predictive_evidence() {
    for owned in [false, true] {
        for mode in [Mode::Fixed, Mode::Reserved] {
            for missing in [false, true] {
                let (mut case, port, grant) =
                    Case::held(owned, mode, false, false, &[b"allow", b"allow"]);
                let proof = PredictorProof::capture(&case);
                let progress = case.run.progress();
                let selected = command(&port, NEXT_ROUND);
                receipt(&port, selected);
                let ticket = case.run.take_pending_convene().unwrap().unwrap();
                if missing {
                    std::fs::remove_file(&case.path).unwrap();
                } else {
                    ordinary::replace(
                        &case.path,
                        &ordinary::document(
                            2,
                            legacy::snapshot(),
                            b"replacement learned helper input",
                        ),
                    );
                }
                let reads = case.source.status().read_attempts;
                assert!(case.convene(ticket, ElapsedTick(1)).is_err());
                assert_eq!(case.source.status().read_attempts, reads + 1);
                assert_eq!(
                    receipt(&port, selected).outcome(),
                    FileConveneOutcome::Failed
                );
                assert_eq!(grant.remaining_operations(), 3);
                assert_eq!(
                    case.run.review().status(),
                    match mode {
                        Mode::Fixed => NativeReviewStatus::Cancelled,
                        Mode::Reserved => NativeReviewStatus::Failed,
                    }
                );
                assert_eq!(case.run.progress().reservation, progress.reservation);
                assert_eq!(case.run.progress().polls, progress.polls);
                assert_eq!(case.run.review().history().len(), 1);
                assert_eq!(PredictorProof::capture(&case), proof);
                assert!(
                    case.run
                        .supervisor()
                        .host()
                        .unwrap()
                        .review_replay(NEXT_ROUND)
                        .is_err()
                );
                ordinary::replace(&case.path, &ordinary::document(1, legacy::snapshot(), &[]));
                let reads = case.source.status();
                let image = case.image();
                blocked(&port, NEXT_ROUND + 1);
                assert!(case.run.take_pending_convene().unwrap().is_none());
                assert_eq!(case.source.status(), reads);
                assert_eq!(case.image(), image);
            }
        }
    }
}

#[test]
fn a_clock_crossing_after_real_source_capture_cannot_begin_the_paid_predictive_successor() {
    for owned in [false, true] {
        for mode in [Mode::Fixed, Mode::Reserved] {
            let (mut case, port, grant) =
                Case::held(owned, mode, false, false, &[b"allow", b"allow"]);
            let proof = PredictorProof::capture(&case);
            let progress = case.run.progress();
            let selected = command(&port, NEXT_ROUND);
            receipt(&port, selected);
            let ticket = case.run.take_pending_convene().unwrap().unwrap();
            let reads = case.source.status().read_attempts;
            let report = match mode {
                Mode::Fixed => {
                    // The final clock is after the original policy-file capture.
                    let mut times = [ElapsedTick(1), ElapsedTick(1), ElapsedTick(30)].into_iter();
                    case.run
                        .convene_from_policy_file(&mut case.source, ticket, || {
                            times
                                .next()
                                .expect("only original bounded clock boundaries")
                        })
                }
                Mode::Reserved => {
                    let mut times = [ElapsedTick(1), ElapsedTick(30)].into_iter();
                    case.run.convene_reserved_from_policy_file(
                        &mut case.source,
                        ticket,
                        ReviewWindow {
                            commit_by: ElapsedTick(30),
                            reveal_by: ElapsedTick(35),
                        },
                        || {
                            times
                                .next()
                                .expect("only original bounded clock boundaries")
                        },
                    )
                }
            };
            assert!(report.result.is_err());
            assert_eq!(
                report.observations,
                vec![Ok(ordinary::document(1, legacy::snapshot(), &[]).identity())]
            );
            assert_eq!(case.source.status().read_attempts, reads + 1);
            assert_eq!(
                receipt(&port, selected).outcome(),
                FileConveneOutcome::Failed
            );
            assert_eq!(grant.remaining_operations(), 3);
            assert_eq!(
                case.run.review().status(),
                match mode {
                    Mode::Fixed => NativeReviewStatus::Cancelled,
                    Mode::Reserved => NativeReviewStatus::Failed,
                }
            );
            assert_eq!(case.run.progress().reservation, progress.reservation);
            assert_eq!(case.run.progress().polls, progress.polls);
            assert_eq!(case.run.review().history().len(), 1);
            assert_eq!(PredictorProof::capture(&case), proof);
            assert!(
                case.run
                    .supervisor()
                    .host()
                    .unwrap()
                    .review_replay(NEXT_ROUND)
                    .is_err()
            );
        }
    }
}

#[test]
fn an_overlarge_grant_cannot_create_a_third_predictive_native_roster_after_two_holds() {
    for owned in [false, true] {
        for mode in [Mode::Fixed, Mode::Reserved] {
            let (mut case, port, grant) = Case::held(owned, mode, false, false, &[b"hold"]);
            let proof = PredictorProof::capture(&case);
            let reservation = case.run.progress().reservation;
            receipt(&port, command(&port, NEXT_ROUND));
            let ticket = case.run.take_pending_convene().unwrap().unwrap();
            case.convene(ticket, ElapsedTick(1)).unwrap();
            case.complete(Consequence::HoldEffect, ElapsedTick(1));
            assert_eq!(case.run.review().history().len(), 2);
            assert_eq!(case.run.progress().reservation, reservation);
            assert_eq!(grant.remaining_operations(), 3);
            let image = case.image();
            let reads = case.source.status();
            let progress = case.run.progress();
            blocked(&port, NEXT_ROUND + 1);
            assert!(case.run.held_successor_round().is_none());
            assert!(!case.run.has_reserved_successor());
            assert!(case.run.take_pending_convene().unwrap().is_none());
            assert!(matches!(
                case.run
                    .step_from_policy_file(
                        &mut case.source,
                        || panic!("a held terminal roster must not acquire time"),
                        None
                    )
                    .result,
                Ok(FileNativeDriverEvent::Driver(FileDriverEvent::Idle))
            ));
            assert_eq!(case.image(), image);
            assert_eq!(case.source.status(), reads);
            assert_eq!(case.run.progress(), progress);
            assert_eq!(PredictorProof::capture(&case), proof);
        }
    }
}
