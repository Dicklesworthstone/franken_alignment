//! Actual Unix helper clients and original driver phases for actor stream input.
#![cfg(unix)]
#[path = "support/file_stream.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{
    driver::{FileDriverEvent, FileDriverLaunch, FileDriverPhase}, stream::FileStreamProposal,
};
use fa_reference::action::consequence::oversight::{ReviewWindow,
    actor::{ActorOutcome, Knowledge, UnknownReason},
    helper_client::{ClientPhase, HelperClient}, helper_workers::HelperLimits,
    supervised::DriverEvidence,
};
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;

#[test]
fn two_actor_messages_pass_real_worker_io_then_separate_dispatch_publication_and_ack() {
    let root = Directory::new(); let (host, reviewer) = create(&root);
    let (port, mut driver) = host.into_supervised_driver();
    for (request, message) in [(77, "first"), (78, "second")] {
        let intent = {
            let host = driver.supervisor().host().unwrap();
            let state = host.inspect();
            FileStreamProposal { target: state.target, expected_policy_epoch: state.control.ledger.epoch,
                deadline: ElapsedTick(100), message: Some(message.to_owned()) }
        };
        let revision = driver.supervisor().host().unwrap().revision();
        driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
        let ticket = port.submit_stream(request, &intent).unwrap();
        let (action, attempt, revision) = {
            let host = driver.supervisor().host().unwrap();
            let fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition::Admitted { attempt, .. }
                = host.request_status(request).unwrap().disposition else { panic!("original admission"); };
            (host.request_action(request).unwrap().clone(), attempt, host.input_revision(attempt).unwrap())
        };
        let input = inputs(&action, b"exact provider context");
        let contracts = profile().committee;
        let mut streams = BTreeMap::new(); let mut clients = Vec::new();
        for (member, helper) in contracts.members() {
            let (local, peer) = UnixStream::pair().unwrap();
            peer.set_nonblocking(true).unwrap();
            streams.insert(member.clone(), local);
            clients.push(HelperClient::new(peer, helper.profile_at(action.spec().policy_epoch)).unwrap());
        }
        driver.start_review(FileDriverLaunch { request, round: request + 1000, evidence_root: [9; 32],
            window: ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) },
            expected_input_revision: revision, inputs: input.clone(), workers: streams,
            limits: HelperLimits::default(),
        }, snapshot(), || ElapsedTick(1)).unwrap();
        let mut applied = false;
        for _ in 0..128 {
            let event = driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
                snapshot: snapshot(), inputs: Some(input.clone()),
            }), None).unwrap();
            for client in &mut clients {
                client.drive(32).unwrap().progress.unwrap();
                if client.phase() == ClientPhase::NeedsInference {
                    let actual = client.input().unwrap().actual_input().submitted_bytes();
                    let frame = &action.spec().payload;
                    assert!(actual.windows(frame.len()).any(|part| part == frame.as_slice()));
                    client.respond(Verdict::Allow, b"independent-test-salt").unwrap();
                }
            }
            match event {
                FileDriverEvent::ReviewApplied { receipt, .. } => {
                    assert_eq!(receipt.policy.control.decision.consequence,
                        fa_reference::action::consequence::Consequence::Continue);
                    applied = true; break;
                }
                FileDriverEvent::Workers { .. } => {}
                other => panic!("unexpected review event: {other:?}"),
            }
        }
        assert!(applied, "bounded native socket dialogue must finish");
        assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request });
        let human_request = driver.request_human_approval(request + 2000, &input, ElapsedTick(31), ElapsedTick(1)).unwrap();
        let human = {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            reviewer.approve(&mut host, revision, &human_request).unwrap()
        };
        assert!(matches!(driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
            snapshot: snapshot(), inputs: Some(input.clone()),
        }), Some(&human)).unwrap(), FileDriverEvent::Dispatched { attempt: actual, .. } if actual == attempt));
        assert!(matches!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        assert!(matches!(driver.step_with_evidence(|| ElapsedTick(1), |_, _| Ok(DriverEvidence {
            snapshot: snapshot(), inputs: Some(input.clone()),
        }), None).unwrap(), FileDriverEvent::PublicationChecked { .. }));
        assert!(matches!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        assert!(matches!(driver.step_with_evidence(|| ElapsedTick(1), |_, _| {
            panic!("receipt-only reconciliation must not call evidence providers")
        }, None).unwrap(), FileDriverEvent::Reconciled { .. }));
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        assert_eq!(driver.phase(), FileDriverPhase::Idle);
    }
    let state = driver.supervisor().host().unwrap().stream_snapshot().unwrap();
    assert_eq!(state.published.messages().collect::<Vec<_>>(), vec!["first", "second"]);
    assert_eq!(state.confirmed, state.published);
    assert_eq!(state.pending, None);
    assert_eq!(state.publication.executions, 2);
}
