//! Actual learned output and original native models, including both effect keys.
//! Deliberately simple weights test custody and transitions, not model quality.
use super::*;
use crate::action::consequence::Consequence;
use crate::action::consequence::delivery::persistent::observed::{
    decoder::learned::sidecar::FileLearnedSidecarFinish, helpers::learned::native::NativeReviewCost,
};
use crate::action::consequence::delivery::persistent::requests::actor::{
    FileConveneOutcome, FileConvenePlan, FileConveneReceipt, FileConveneRequest, FileConveneTicket,
    FileSupervisorConveneGrant, FileSupervisorConvenePort,
};
use std::collections::BTreeMap;

const MESSAGE: u64 = 71;
const FINISH: u64 = 72;
const HELD: u64 = 101;
const NEXT: u64 = 102;

struct Case {
    reviewer: FileHumanReviewer,
    wire: ActorWire<FileLearnedTextStreamActorPort>,
    source: FileEvidenceSource,
    port: FileSupervisorConvenePort,
    grant: FileSupervisorConveneGrant,
    root: Directory,
}
impl Case {
    fn new() -> (Self, FileSupervisedDriver) {
        let root = Directory::new();
        let (mut host, reviewer) = FileOversight::create_with_learned_text_stream(
            root.store(),
            profile_stream(),
            configured_stream(),
        )
        .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        for _ in 0..3 {
            step(&mut host);
        }
        let (actor, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let (port, grant) = supervisor.convene_port(64, 4).unwrap();
        let source = reader(&root);
        publish(&root.store().with_extension("policy"), &capture(1));
        (
            Self {
                reviewer,
                wire: ActorWire::new(actor),
                source,
                port,
                grant,
                root,
            },
            FileSupervisedDriver::new(supervisor),
        )
    }
    fn admit(
        &mut self,
        driver: &mut FileSupervisedDriver,
        request: u64,
        release: LearnedTextRelease,
    ) {
        let report = driver
            .supervisor_mut()
            .exchange_learned_text_stream_actor_from_policy_file(
                &mut self.wire,
                &document_stream(request, release),
                &mut self.source,
                || ElapsedTick(1),
            )
            .unwrap();
        assert_eq!(report.response.result, Ok(Knowledge::Pending { request }));
        assert!(report.intake.unwrap().result.is_ok());
    }
    fn plan(&self, request: u64, round: u64) -> FileConvenePlan {
        match self.port.next(request, round).unwrap() {
            Knowledge::Known { value, .. } => value,
            other => panic!("live original convening observation: {other:?}"),
        }
    }
    fn receipt(&self, command: FileConveneRequest) -> FileConveneReceipt {
        match self.port.convene(command).unwrap() {
            Knowledge::Known { value, .. } => value,
            other => panic!("original finite grant receipt: {other:?}"),
        }
    }
    fn queue(
        &self,
        run: FileNativeSupervisedDriver,
        request: u64,
        round: u64,
    ) -> (
        FileNativeSupervisedDriver,
        FileConveneTicket,
        FileConveneRequest,
    ) {
        let command = self.plan(request, round).convene_request().unwrap();
        assert_eq!(self.receipt(command).outcome(), FileConveneOutcome::Pending);
        let ticket = run.take_pending_convene().unwrap().unwrap();
        (run, ticket, command)
    }
}

// All actual evaluators, probes and salts are constructed BEFORE the first Begin.
// There is deliberately no member construction in any post-Hold helper below.
fn reserves(
    launch: &FileNativeDriverLaunch,
    spellings: &[&[u8]],
) -> Vec<BTreeMap<String, NativeReviewMember>> {
    let original = launch.rosters.values().next().unwrap();
    spellings
        .iter()
        .enumerate()
        .map(|(round, spelling)| {
            original
                .iter()
                .enumerate()
                .map(|(index, (name, member))| {
                    (
                        name.clone(),
                        NativeReviewMember {
                            evaluator: native_fixture::evaluator(
                                member.evaluator.policy().input_profile.clone(),
                                spelling,
                            ),
                            queries: member.queries.clone(),
                            salt: vec![48 + (round * 2 + index) as u8; 32],
                        },
                    )
                })
                .collect()
        })
        .collect()
}
fn start_reserved(
    mut driver: FileSupervisedDriver,
    request: u64,
    round: u64,
    spelling: &[u8],
    successors: &[&[u8]],
    polls: usize,
) -> FileNativeSupervisedDriver {
    let mut launch = launch(&mut driver, request, round, spelling);
    launch.limits.polls = polls;
    let successors = reserves(&launch, successors);
    driver
        .start_native_learned_sequence_with_convening(
            launch,
            snapshot(),
            ElapsedTick(1),
            successors,
        )
        .unwrap()
}
fn held(successors: &[&[u8]]) -> (Case, FileNativeSupervisedDriver) {
    let (mut case, mut driver) = Case::new();
    case.admit(&mut driver, MESSAGE, LearnedTextRelease::Message);
    let mut run = start_reserved(
        driver,
        MESSAGE,
        HELD,
        b"abstain",
        successors,
        MAX_LEARNED_REVIEW_POLLS,
    );
    complete(&mut run, &mut case.source, 1);
    assert!(matches!(run.review().history().last(),
        Some(FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. })
            if receipt.policy.control.decision.consequence == Consequence::HoldEffect));
    (case, run)
}
fn window() -> ReviewWindow {
    ReviewWindow {
        commit_by: ElapsedTick(20),
        reveal_by: ElapsedTick(30),
    }
}

#[test]
fn learned_held_message_uses_reserved_native_successor_and_independent_human_approve_or_reject() {
    for reject in [false, true] {
        let (mut case, mut run) = held(&[b"allow"]);
        let (numerical, action) = {
            let host = run.supervisor().host().unwrap();
            (
                host.learned_generation_inspection().unwrap().numerical,
                host.request_action(MESSAGE).unwrap().clone(),
            )
        };
        let input = run.review().input().clone();
        let reservation = run.review().reservation();
        let polls = run.review().polls();
        let first = run.review().records()[&HELD].clone();
        let reads = case.source.status().read_attempts;
        assert!(matches!(
            run.step_from_policy_file(
                &mut case.source,
                || panic!("held polling cannot resample or refresh"),
                None
            )
            .result,
            Ok(FileNativeDriverEvent::Driver(FileDriverEvent::Idle))
        ));
        assert!(
            run.request_human_approval_from_policy_file(
                &mut case.source,
                9000,
                ElapsedTick(80),
                || panic!("Hold cannot request the independent key")
            )
            .result
            .is_err()
        );
        assert_eq!(case.source.status().read_attempts, reads);
        let (mut run, ticket, command) = case.queue(run, MESSAGE, NEXT);
        let remaining = case.grant.remaining_operations();
        assert_eq!(case.receipt(command).outcome(), FileConveneOutcome::Pending);
        assert_eq!(case.grant.remaining_operations(), remaining);
        // The old source lease is expired. The original packet and generation
        // remain fixed while the explicit successor acquires a fresh lease.
        run.convene_reserved_from_policy_file(&mut case.source, ticket, window(), || {
            ElapsedTick(6)
        })
        .result
        .unwrap();
        assert_eq!(case.receipt(command).outcome(), FileConveneOutcome::Opened);
        assert_eq!(run.review().polls(), polls);
        assert_eq!(run.review().reservation(), reservation);
        assert_eq!(run.review().pending_convene_rosters(), 0);
        complete(&mut run, &mut case.source, 6);
        assert_eq!(run.review().history().len(), 2);
        assert_eq!(&run.review().records()[&HELD], &first);
        assert_eq!(run.review().input(), &input);
        assert_eq!(run.review().reservation(), reservation);
        assert!(run.review().polls() > polls);
        assert!(matches!(run.review().history().last(),
            Some(FileLearnedSidecarFinish::Applied { receipt: Ok(receipt), .. })
                if receipt.policy.control.decision.consequence == Consequence::Continue));
        assert!(matches!(
            tick(&mut run, &mut case.source, None, 6),
            FileDriverEvent::AwaitingHuman { request: MESSAGE }
        ));
        if reject {
            let request = run
                .request_human_approval_from_policy_file(
                    &mut case.source,
                    9001,
                    ElapsedTick(80),
                    || ElapsedTick(6),
                )
                .result
                .unwrap();
            {
                let mut host = run.supervisor_mut().host_mut().unwrap();
                let revision = host.revision();
                case.reviewer.reject(&mut host, revision, &request).unwrap();
            }
            assert!(matches!(
                tick(&mut run, &mut case.source, None, 6),
                FileDriverEvent::AwaitingHuman { request: MESSAGE }
            ));
        } else {
            let human = approve(&mut run, &case.reviewer, &mut case.source, 9001, 6);
            assert!(matches!(
                tick(&mut run, &mut case.source, Some(&human), 6),
                FileDriverEvent::Dispatched { .. }
            ));
            assert!(matches!(
                tick(&mut run, &mut case.source, None, 6),
                FileDriverEvent::PublicationChecked { .. }
            ));
            assert!(matches!(
                tick(&mut run, &mut case.source, None, 6),
                FileDriverEvent::Reconciled { .. }
            ));
        }
        let host = run.supervisor().host().unwrap();
        assert_eq!(host.request_action(MESSAGE).unwrap(), &action);
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical
        );
        assert_eq!(host.inspect().executions, u64::from(!reject));
        let stream = host.stream_snapshot().unwrap();
        assert_eq!(
            stream.confirmed.visible(),
            if reject { b"".as_slice() } else { b"aa" }
        );
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(
            host.inspect().control.ledger.charged,
            if reject { 0 } else { action.spec().units }
        );
    }
}

#[test]
fn learned_held_finish_retains_confirmed_prefix_and_requires_a_new_human_key() {
    let (mut case, mut driver) = Case::new();
    case.admit(&mut driver, MESSAGE, LearnedTextRelease::Message);
    let mut run = start_reserved(
        driver,
        MESSAGE,
        HELD,
        b"allow",
        &[],
        MAX_LEARNED_REVIEW_POLLS,
    );
    complete(&mut run, &mut case.source, 1);
    let message_key = approve(&mut run, &case.reviewer, &mut case.source, 9001, 1);
    assert!(matches!(
        tick(&mut run, &mut case.source, Some(&message_key), 1),
        FileDriverEvent::Dispatched { .. }
    ));
    tick(&mut run, &mut case.source, None, 1);
    tick(&mut run, &mut case.source, None, 1);
    let mut driver = run.into_handoff().unwrap().driver;
    case.admit(&mut driver, FINISH, LearnedTextRelease::Finish);
    let mut run = start_reserved(
        driver,
        FINISH,
        201,
        b"abstain",
        &[b"allow"],
        MAX_LEARNED_REVIEW_POLLS,
    );
    complete(&mut run, &mut case.source, 1);
    let (numerical, prefix) = {
        let host = run.supervisor().host().unwrap();
        assert!(
            ReleaseFrame::decode(&host.request_action(FINISH).unwrap().spec().payload)
                .unwrap()
                .is_finish()
        );
        (
            host.learned_generation_inspection().unwrap().numerical,
            host.stream_snapshot().unwrap().confirmed,
        )
    };
    let (mut run, ticket, _) = case.queue(run, FINISH, 202);
    run.convene_reserved_from_policy_file(&mut case.source, ticket, window(), || ElapsedTick(1))
        .result
        .unwrap();
    complete(&mut run, &mut case.source, 1);
    assert_eq!(
        run.supervisor()
            .host()
            .unwrap()
            .stream_snapshot()
            .unwrap()
            .confirmed,
        prefix
    );
    assert!(
        run.step_from_policy_file(&mut case.source, || ElapsedTick(1), Some(&message_key))
            .result
            .is_err()
    );
    let finish_key = approve(&mut run, &case.reviewer, &mut case.source, 9002, 1);
    assert!(matches!(
        tick(&mut run, &mut case.source, Some(&finish_key), 1),
        FileDriverEvent::Dispatched { .. }
    ));
    tick(&mut run, &mut case.source, None, 1);
    tick(&mut run, &mut case.source, None, 1);
    let host = run.supervisor().host().unwrap();
    let stream = host.stream_snapshot().unwrap();
    assert!(stream.confirmed.finished() && stream.published.finished());
    assert_eq!(stream.confirmed.visible(), b"aa");
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        numerical
    );
}

#[test]
fn every_reserved_native_model_is_funded_before_initial_begin() {
    for dimension in 0..3 {
        let (mut case, mut driver) = Case::new();
        case.admit(&mut driver, MESSAGE, LearnedTextRelease::Message);
        let mut launch = launch(&mut driver, MESSAGE, HELD, b"abstain");
        let successors = reserves(&launch, &[b"allow"]);
        let initial = NativeReviewCost::required_sequence(&launch.rosters).unwrap();
        let extra = NativeReviewCost::required(&successors[0]).unwrap();
        launch.limits.native = NativeReviewCost {
            evaluations: initial.evaluations + extra.evaluations,
            scalar_products: initial.scalar_products + extra.scalar_products,
            sampling_entries: initial.sampling_entries + extra.sampling_entries,
        };
        match dimension {
            0 => launch.limits.native.evaluations -= 1,
            1 => launch.limits.native.scalar_products -= 1,
            _ => launch.limits.native.sampling_entries -= 1,
        }
        let numerical = driver
            .supervisor()
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap()
            .numerical;
        let refused = driver
            .start_native_learned_sequence_with_convening(
                launch,
                snapshot(),
                ElapsedTick(1),
                successors,
            )
            .unwrap_err();
        assert!(matches!(
            refused.error,
            JournalError::Contract(Error::Limit)
        ));
        let host = refused.driver.supervisor().host().unwrap();
        assert!(host.review_replay(HELD).is_err());
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical
        );
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn unreserved_dropped_or_fully_spent_native_custody_cannot_advertise_a_successor() {
    let (case, run) = held(&[]);
    assert!(!case.plan(MESSAGE, NEXT).can_convene());
    let polls = run.review().polls();
    drop(run);
    let (case, run) = held(&[b"allow"]);
    assert!(case.plan(MESSAGE, NEXT).can_convene());
    let handoff = run.into_handoff().unwrap();
    drop(handoff.review);
    assert!(!case.plan(MESSAGE, NEXT).can_convene());
    assert!(handoff.driver.supervisor().host().is_ok());
    let (mut case, mut driver) = Case::new();
    case.admit(&mut driver, MESSAGE, LearnedTextRelease::Message);
    let mut run = start_reserved(driver, MESSAGE, HELD, b"abstain", &[b"allow"], polls);
    complete(&mut run, &mut case.source, 1);
    assert_eq!(run.review().polls(), polls);
    assert_eq!(run.review().pending_convene_rosters(), 0);
    assert!(!case.plan(MESSAGE, NEXT).can_convene());
}

#[test]
fn explicit_successor_spends_the_original_remaining_poll_allowance() {
    let (_baseline, original) = held(&[]);
    let original_polls = original.review().polls();
    let (mut case, mut driver) = Case::new();
    case.admit(&mut driver, MESSAGE, LearnedTextRelease::Message);
    let mut run = start_reserved(
        driver,
        MESSAGE,
        HELD,
        b"abstain",
        &[b"allow"],
        original_polls + 1,
    );
    complete(&mut run, &mut case.source, 1);
    let reservation = run.review().reservation();
    let (mut run, ticket, _) = case.queue(run, MESSAGE, NEXT);
    run.convene_reserved_from_policy_file(&mut case.source, ticket, window(), || ElapsedTick(1))
        .result
        .unwrap();
    assert_eq!(run.review().polls(), original_polls);
    assert!(
        run.step_from_policy_file(&mut case.source, || ElapsedTick(1), None)
            .result
            .is_ok()
    );
    assert_eq!(run.review().polls(), original_polls + 1);
    assert!(
        run.step_from_policy_file(&mut case.source, || ElapsedTick(1), None)
            .result
            .is_err()
    );
    assert_eq!(run.review().reservation(), reservation);
    assert_eq!(run.review().history().len(), 1);
    assert!(!case.plan(MESSAGE, 103).can_convene());
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 0);
}

mod boundaries;
