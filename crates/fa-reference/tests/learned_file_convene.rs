//! A held learned sidecar can spend only its next original native-model slot.
//! Actual actor/helper models and a durable policy-only file exercise the full
//! two-key publication path; synthetic weights are not detector qualification.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "learned_file_convene/fixture.rs"]
mod fixture;
#[path = "support/file_oversight.rs"]
mod legacy;
#[path = "mediated_predictive_learned_recovery/native_fixture.rs"]
mod native_fixture;
#[path = "support/learned_text_model.rs"]
mod numerical;

use fa_reference::Snapshot;
use fa_reference::action::{ActionState, ElapsedTick};
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
                FileDriverEvent, FileDriverPhase, FileSupervisedDriver,
                evidence::FileReviewLaunch,
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
                FileLearnedTextStreamActorPort, FileSupervisorConveneGrant, FileSupervisorConvenePort,
                LearnedTextRelease,
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
    helper_workers::HelperLimits,
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
use fixture::{Case, blocked, command, document, receipt, replace};
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::net::UnixStream;

const REQUEST: u64 = 71;
const HELD_ROUND: u64 = 101;
const NEXT_ROUND: u64 = 102;

#[test]
fn native_hold_reuses_the_original_learned_packet_and_publishes_only_after_both_keys() {
    let (mut case, port, grant) = Case::held_with_grant(4, 1);
    let (action, numerical, packet, stream) = {
        let host = case.run.supervisor().host().unwrap();
        (
            host.request_action(REQUEST).unwrap().clone(),
            host.learned_generation_inspection().unwrap().numerical,
            host.retained_learned_sidecar(case.attempt).unwrap(),
            host.stream_snapshot().unwrap(),
        )
    };
    let first = case.run.review().records()[&HELD_ROUND].clone();
    let archive = case.run.review().history()[0].archive().clone();
    let before = case.run.progress();
    assert_eq!(before.phase, FileDriverPhase::Idle);
    assert_eq!(before.reservation.evaluations, 2);
    assert!(!case.run.review().records()[&NEXT_ROUND].started);
    let next = case.run.held_successor_round().unwrap();
    assert_eq!(next.round, NEXT_ROUND);
    assert_eq!(next.window.commit_by, ElapsedTick(30));
    assert_eq!(next.window.reveal_by, ElapsedTick(35));
    let selected = command(&port, NEXT_ROUND);
    assert_eq!(selected.expected().held_round, HELD_ROUND);
    assert_eq!(selected.expected().input_revision, packet.input_revision);
    assert_eq!(selected.expected().deadline, action.spec().deadline);
    let image = case.image();
    let reads = case.source.status();
    for _ in 0..3 {
        assert!(matches!(
            case.run.step_from_policy_file(
                &mut case.source, || panic!("a held sequence cannot automatically poll time"), None,
            ).result,
            Ok(FileNativeDriverEvent::Driver(FileDriverEvent::Idle))
        ));
        assert!(matches!(case.actor.poll(&case.ticket), Knowledge::Pending { request: REQUEST }));
    }
    assert_eq!(case.run.progress(), before);
    assert_eq!(case.source.status(), reads);
    assert_eq!(case.image(), image);
    let pending = receipt(&port, selected);
    assert_eq!(pending.outcome(), FileConveneOutcome::Pending);
    assert_eq!(receipt(&port.clone(), selected), pending);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(case.image(), image);
    case.convene();
    let opened = receipt(&port, selected);
    assert_eq!(opened.outcome(), FileConveneOutcome::Opened);
    assert_eq!(opened.opened().unwrap().input_revision(), packet.input_revision);
    assert_eq!(opened.opened().unwrap().started_at(), ElapsedTick(1));
    assert_eq!(case.source.status().read_attempts, reads.read_attempts + 1);
    assert_eq!(case.run.progress().reservation, before.reservation);
    assert_eq!(case.run.progress().polls, before.polls);
    assert_eq!(case.run.review().records()[&HELD_ROUND], first);
    assert!(case.run.review().records()[&NEXT_ROUND].started);
    assert_eq!(case.run.review().records()[&NEXT_ROUND].polls, 0);
    case.complete(Consequence::Continue);
    assert_eq!(case.run.review().history().len(), 2);
    assert_eq!(case.run.review().history()[0].archive(), &archive);
    assert_eq!(case.run.review().records()[&HELD_ROUND], first);
    assert!(case.run.progress().polls > before.polls);
    assert_eq!(case.run.progress().reservation, before.reservation);
    {
        let host = case.run.supervisor().host().unwrap();
        let retained = host.retained_learned_sidecar(case.attempt).unwrap();
        assert_eq!(retained.packet, packet.packet);
        assert_eq!(retained.actor_revision, packet.actor_revision);
        assert_eq!(retained.input_revision, packet.input_revision);
        assert_eq!(host.request_action(REQUEST).unwrap(), &action);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.stream_snapshot().unwrap().confirmed, stream.confirmed);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
    let human = case.approve();
    case.publish(&human);
    let host = case.run.supervisor().host().unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.request_action(REQUEST).unwrap(), &action);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"OK");
    assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 1);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    drop(host);
    let image = case.image();
    assert_eq!(receipt(&port, selected), opened);
    assert_eq!(case.image(), image);
}

#[test]
fn only_the_next_original_roster_and_its_exact_owner_can_receive_a_handoff() {
    let (mut case, port, grant) = Case::held_with_grant(8, 1);
    let image = case.image();
    let progress = case.run.progress();
    for round in [HELD_ROUND, NEXT_ROUND + 1, 999] {
        blocked(&port, round);
    }
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(case.run.progress(), progress);
    assert_eq!(case.image(), image);
    let selected = command(&port, NEXT_ROUND);
    receipt(&port, selected);
    let ticket = case.run.take_pending_convene().unwrap().unwrap();
    let mut foreign = Case::held();
    let foreign_image = foreign.image();
    let foreign_progress = foreign.run.progress();
    let foreign_reads = foreign.source.status();
    assert!(foreign.run.convene_from_policy_file(
        &mut foreign.source, ticket,
        || panic!("a foreign original owner cannot reach a clock"),
    ).result.is_err());
    assert_eq!(foreign.source.status(), foreign_reads);
    assert_eq!(foreign.run.progress(), foreign_progress);
    assert_eq!(foreign.image(), foreign_image);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(receipt(&port, selected).outcome(), FileConveneOutcome::Failed);
    assert!(!case.run.review().records()[&NEXT_ROUND].started);
    assert!(case.run.take_pending_convene().unwrap().is_none());
}

#[test]
fn policy_source_loss_or_changed_input_never_rebases_the_held_packet_or_retries_the_slot() {
    for failure in 0..3 {
        let (mut case, port, grant) = Case::held_with_grant(4, 1);
        let numerical = case.run.supervisor().host().unwrap()
            .learned_generation_inspection().unwrap().numerical;
        let packet = case.run.supervisor().host().unwrap()
            .retained_learned_sidecar(case.attempt).unwrap().packet;
        let reservation = case.run.progress().reservation;
        let original = case.run.review().records()[&HELD_ROUND].clone();
        let archive = case.run.review().history()[0].archive().clone();
        let selected = command(&port, NEXT_ROUND);
        receipt(&port, selected);
        let ticket = case.run.take_pending_convene().unwrap().unwrap();
        match failure {
            0 => std::fs::remove_file(&case.path).unwrap(),
            1 => replace(&case.path, &document(2, legacy::snapshot(), b"caller replacement input")),
            _ => {
                let mut changed = legacy::snapshot();
                changed.semantic_epoch += 1;
                changed.values.insert(8, b"new independent source context".to_vec());
                replace(&case.path, &document(2, changed, &[]));
            }
        }
        let reads = case.source.status().read_attempts;
        let report = case.run.convene_from_policy_file(&mut case.source, ticket, || ElapsedTick(1));
        assert!(report.result.is_err(), "source failure case {failure} reopened the packet");
        assert_eq!(case.source.status().read_attempts, reads + 1);
        assert_eq!(grant.remaining_operations(), 0);
        assert_eq!(receipt(&port, selected).outcome(), FileConveneOutcome::Failed);
        assert!(!case.run.review().records()[&NEXT_ROUND].started);
        assert_eq!(case.run.review().records()[&HELD_ROUND], original);
        assert_eq!(case.run.review().history()[0].archive(), &archive);
        assert_eq!(case.run.progress().reservation, reservation);
        let host = case.run.supervisor().host().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(host.retained_learned_sidecar(case.attempt).unwrap().packet, packet);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        drop(host);
        replace(&case.path, &document(1, legacy::snapshot(), &[]));
        let reads = case.source.status();
        let image = case.image();
        assert_eq!(receipt(&port, selected).outcome(), FileConveneOutcome::Failed);
        assert!(case.run.take_pending_convene().unwrap().is_none());
        assert_eq!(case.source.status(), reads);
        assert_eq!(case.image(), image);
        assert!(case.run.request_human_approval(9001, ElapsedTick(31), ElapsedTick(1)).is_err());
    }
}

#[test]
fn changed_predecessor_stop_and_cancellation_revoke_taken_tickets_before_source_or_clock() {
    for change in 0..4 {
        let (mut case, port, grant) = Case::held_with_grant(4, 1);
        let selected = command(&port, NEXT_ROUND);
        receipt(&port, selected);
        let ticket = case.run.take_pending_convene().unwrap().unwrap();
        match change {
            0 => {
                let mut host = case.run.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                host.observe_time(revision, ElapsedTick(2)).unwrap();
            }
            1 => {
                let mut host = case.run.supervisor_mut().host_mut().unwrap();
                let state = host.inspect();
                host.request_stop(state.revision, StopRequest {
                    operation: 7001, expected_control_sequence: state.control.sequence,
                    expected_authority_epoch: state.control.ledger.epoch,
                }).unwrap();
            }
            2 => case.actor.cancel(&case.ticket).unwrap(),
            _ => case.run.cancel(case.run.review().revision()).unwrap(),
        }
        let image = case.image();
        let reads = case.source.status();
        let numerical = case.run.supervisor().host().unwrap()
            .learned_generation_inspection().unwrap().numerical;
        assert!(case.run.convene_from_policy_file(
            &mut case.source, ticket,
            || panic!("revoked or stale held predecessor cannot acquire time"),
        ).result.is_err());
        assert_eq!(case.source.status(), reads);
        assert_eq!(case.image(), image);
        assert_eq!(grant.remaining_operations(), 0);
        assert_ne!(receipt(&port, selected).outcome(), FileConveneOutcome::Opened);
        assert!(!case.run.review().records()[&NEXT_ROUND].started);
        assert_eq!(case.run.supervisor().host().unwrap()
            .learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(case.run.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn terminal_handoff_revokes_a_taken_ticket_and_generic_file_review_cannot_replace_the_native_owner() {
    let (mut case, port, grant) = Case::held_with_grant(3, 1);
    let selected = command(&port, NEXT_ROUND);
    receipt(&port, selected);
    let ticket = case.run.take_pending_convene().unwrap().unwrap();
    let input_revision = case.run.progress().input_revision;
    let reservation = case.run.progress().reservation;
    let next = case.run.held_successor_round().unwrap();
    let mut handoff = case.run.into_handoff().unwrap();
    assert_eq!(handoff.review.reservation(), reservation);
    assert!(!handoff.review.records()[&NEXT_ROUND].started);
    let image = std::fs::read(case.root.store().join("delivery.bin")).unwrap();
    let reads = case.source.status();
    let launch = FileReviewLaunch::<UnixStream> {
        request: REQUEST, round: NEXT_ROUND, window: next.window,
        expected_input_revision: input_revision,
        workers: BTreeMap::new(), limits: HelperLimits::default(),
    };
    assert!(handoff.driver.start_file_convened_review(
        &mut case.source, ticket, launch,
        || panic!("generic review cannot borrow a terminal native handoff"),
    ).is_err());
    assert_eq!(case.source.status(), reads);
    assert_eq!(std::fs::read(case.root.store().join("delivery.bin")).unwrap(), image);
    assert_eq!(grant.remaining_operations(), 0);
    assert_ne!(receipt(&port, selected).outcome(), FileConveneOutcome::Opened);
    assert_eq!(handoff.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn original_native_poll_ceiling_and_fixed_successor_window_do_not_renew_after_hold() {
    let baseline = Case::held();
    let held_polls = baseline.run.progress().polls;
    assert!(held_polls > 0 && held_polls + 1 < MAX_LEARNED_REVIEW_POLLS);
    drop(baseline);
    let mut limited = Case::new(&[b"hold", b"allow"], held_polls + 1);
    let (port, _grant) = limited.run.supervisor().convene_port(2, 1).unwrap();
    limited.complete(Consequence::HoldEffect);
    assert_eq!(limited.run.progress().polls, held_polls);
    let first = limited.run.review().records()[&HELD_ROUND].clone();
    let reservation = limited.run.progress().reservation;
    receipt(&port, command(&port, NEXT_ROUND));
    limited.convene();
    assert_eq!(limited.run.progress().polls, held_polls);
    limited.run.step(|| ElapsedTick(1), || Ok(legacy::snapshot()), None).unwrap();
    assert_eq!(limited.run.progress().polls, held_polls + 1);
    assert!(limited.run.step(|| ElapsedTick(1), || Ok(legacy::snapshot()), None).is_err());
    assert_eq!(limited.run.review().status(), NativeReviewStatus::Failed);
    assert_eq!(limited.run.review().records()[&HELD_ROUND], first);
    assert_eq!(limited.run.progress().reservation, reservation);
    assert_eq!(limited.run.supervisor().host().unwrap().inspect().executions, 0);
    assert!(limited.run.request_human_approval(9001, ElapsedTick(31), ElapsedTick(1)).is_err());

    let (mut expired, port, grant) = Case::held_with_grant(2, 1);
    let reservation = expired.run.progress().reservation;
    let next = expired.run.held_successor_round().unwrap();
    let selected = command(&port, NEXT_ROUND);
    receipt(&port, selected);
    let ticket = expired.run.take_pending_convene().unwrap().unwrap();
    assert!(expired.run.convene_from_policy_file(
        &mut expired.source, ticket, || next.window.commit_by,
    ).result.is_err());
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(receipt(&port, selected).outcome(), FileConveneOutcome::Failed);
    assert!(!expired.run.review().records()[&NEXT_ROUND].started);
    assert_eq!(expired.run.progress().reservation, reservation);
    assert_eq!(expired.run.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn another_native_hold_preserves_the_last_unused_roster_and_the_spent_grant() {
    let mut case = Case::new(&[b"hold", b"hold", b"allow"], MAX_LEARNED_REVIEW_POLLS);
    let (port, grant) = case.run.supervisor().convene_port(4, 1).unwrap();
    case.complete(Consequence::HoldEffect);
    let reservation = case.run.progress().reservation;
    assert_eq!(reservation.evaluations, 3);
    let selected = command(&port, NEXT_ROUND);
    receipt(&port, selected);
    case.convene();
    case.complete(Consequence::HoldEffect);
    assert_eq!(case.run.review().history().len(), 2);
    assert_eq!(case.run.progress().reservation, reservation);
    let next = case.run.held_successor_round().unwrap();
    assert_eq!(next.round, NEXT_ROUND + 1);
    assert_eq!(next.window.commit_by, ElapsedTick(50));
    assert!(!case.run.review().records()[&(NEXT_ROUND + 1)].started);
    assert_eq!(grant.remaining_operations(), 0);
    blocked(&port, NEXT_ROUND + 1);
    let image = case.image();
    let reads = case.source.status();
    let progress = case.run.progress();
    assert!(matches!(
        case.run.step_from_policy_file(
            &mut case.source, || panic!("another Hold cannot automatically renew its work"), None,
        ).result,
        Ok(FileNativeDriverEvent::Driver(FileDriverEvent::Idle))
    ));
    assert_eq!(case.run.progress(), progress);
    assert_eq!(case.source.status(), reads);
    assert_eq!(case.image(), image);
}

#[test]
fn a_new_native_allow_still_cannot_replace_the_independent_humans_rejection() {
    let (mut case, port, _grant) = Case::held_with_grant(2, 1);
    receipt(&port, command(&port, NEXT_ROUND));
    case.convene();
    case.complete(Consequence::Continue);
    let numerical = case.run.supervisor().host().unwrap()
        .learned_generation_inspection().unwrap().numerical;
    let request = case.run.request_human_approval_from_policy_file(
        &mut case.source, 9001, ElapsedTick(31), || ElapsedTick(1),
    ).result.unwrap();
    {
        let mut host = case.run.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        case.reviewer.reject(&mut host, revision, &request).unwrap();
    }
    assert!(matches!(case.tick(None), FileDriverEvent::AwaitingHuman { request: REQUEST }));
    let host = case.run.supervisor().host().unwrap();
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.message_count(), 0);
}
