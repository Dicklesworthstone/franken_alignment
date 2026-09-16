//! Actor-only message intents reuse the original durable ticket and request book.
#![cfg(unix)]
#[path = "support/file_stream.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer,
    source::FileSourcePolicy, stream::FileStreamProposal};
use fa_reference::action::consequence::delivery::persistent::requests::{FileRequestDisposition,
    actor::FileActorSupervisor};
use fa_reference::action::consequence::delivery::stream::{ReleaseFrame, MAX_MESSAGE_BYTES};
use fa_reference::action::consequence::oversight::actor::{ActorError, ActorOutcome, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};

fn proposal(host: &FileOversight, message: Option<&str>) -> FileStreamProposal {
    let state = host.inspect();
    FileStreamProposal { target: state.target, expected_policy_epoch: state.control.ledger.epoch,
        deadline: ElapsedTick(100), message: message.map(str::to_owned) }
}
fn supply(supervisor: &mut FileActorSupervisor<FileOversight>) {
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
}
fn review_request(host: &mut FileOversight, reviewer: &FileHumanReviewer, request: u64) -> Keys {
    let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(request).unwrap().disposition
        else { panic!("expected original admission"); };
    let action = host.request_action(request).unwrap().clone();
    let inputs = inputs(&action, b"actor stream source");
    review_existing(host, attempt, attempt + 100, &inputs);
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
    let now = host.inspect().control.ledger.elapsed.unwrap().0;
    let request = host.request_human_approval(host.revision(), request + 1000, attempt, &inputs, ElapsedTick(now + 30)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human, request }
}

#[test]
fn actor_intents_add_private_cumulative_context_and_keep_original_unknown_until_ack() {
    let root = Directory::new(); let (host, reviewer) = create(&root);
    let (port, mut supervisor) = host.into_actor_gateway();
    for (request, message, expected) in [(10, Some("ab"), vec![]), (11, Some("c"), vec!["ab"]),
        (12, None, vec!["ab", "c"])]
    {
        let intent = proposal(&supervisor.host().unwrap(), message);
        supply(&mut supervisor);
        let ticket = port.submit_stream(request, &intent).unwrap();
        assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
        let keys = {
            let mut host = supervisor.host_mut().unwrap();
            let frame = ReleaseFrame::decode(&host.request_action(request).unwrap().spec().payload).unwrap();
            assert_eq!(frame.prior_messages(), expected.as_slice());
            assert_eq!(frame.message(), message);
            review_request(&mut host, &reviewer, request)
        };
        {
            let mut host = supervisor.host_mut().unwrap();
            dispatch(&mut host, &keys); publish(&mut host, &keys);
        }
        assert!(matches!(port.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
        {
            let mut host = supervisor.host_mut().unwrap();
            let revision = host.revision();
            host.reconcile(revision, keys.automatic.attempt()).unwrap();
        }
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    }
    let state = supervisor.host().unwrap().stream_snapshot().unwrap();
    assert_eq!(state.published.messages().collect::<Vec<_>>(), vec!["ab", "c"]);
    assert!(state.published.finished());
}

#[test]
fn exact_retry_keeps_original_prefix_does_not_consume_snapshot_and_survives_reopen() {
    let root = Directory::new(); let (host, reviewer) = create(&root);
    let original = proposal(&host, Some("first"));
    let (port, mut supervisor) = host.into_actor_gateway();
    supply(&mut supervisor);
    let old_ticket = port.submit_stream(10, &original).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let keys = review_request(&mut host, &reviewer, 10);
        dispatch(&mut host, &keys); publish(&mut host, &keys);
        let revision = host.revision();
        host.reconcile(revision, keys.automatic.attempt()).unwrap();
    }
    supply(&mut supervisor);
    let revision = supervisor.host().unwrap().revision();
    let retry = port.submit_stream(10, &original).unwrap();
    assert!(matches!(port.poll(&retry), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    for change in 0..5 {
        let mut altered = original.clone();
        match change {
            0 => altered.message = Some("changed".into()),
            1 => altered.message = None,
            2 => altered.deadline = ElapsedTick(101),
            3 => altered.target.expected_version += 1,
            _ => altered.expected_policy_epoch += 1,
        }
        assert_eq!(port.submit_stream(10, &altered).unwrap_err(), ActorError::IdempotencyConflict);
        assert_eq!(supervisor.host().unwrap().revision(), revision);
    }
    let next = proposal(&supervisor.host().unwrap(), Some("second"));
    // The only supplied snapshot survived every retry/conflict above.
    port.submit_stream(11, &next).unwrap();
    drop(supervisor);
    assert_eq!(port.submit_stream(10, &original).unwrap_err(), ActorError::Unavailable);
    let (host, _) = FileOversight::open_stream(root.store(), profile(), stream()).unwrap();
    let (new_port, new_supervisor) = host.into_actor_gateway();
    let revision = new_supervisor.host().unwrap().revision();
    let retry = new_port.submit_stream(10, &original).unwrap();
    assert_eq!(new_supervisor.host().unwrap().revision(), revision);
    assert!(!new_supervisor.host().unwrap().clock_ready());
    assert!(matches!(new_port.poll(&retry), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert!(matches!(new_port.poll(&old_ticket), Knowledge::Withheld { .. }));
}

#[test]
fn source_refused_request_retries_from_original_spec_without_a_frozen_action() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    host.enable_file_source(host.revision(), FileSourcePolicy {
        source: StateSource { scope: profile().delivery.scope, source: 7, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(20).unwrap(),
    }).unwrap();
    let intent = proposal(&host, Some("unobserved"));
    let (port, mut supervisor) = host.into_actor_gateway();
    supply(&mut supervisor);
    let ticket = port.submit_stream(50, &intent).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::NotAdmitted, .. }));
    assert!(supervisor.host().unwrap().request_action(50).is_err());
    let revision = supervisor.host().unwrap().revision();
    let retry = port.submit_stream(50, &intent).unwrap();
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert!(matches!(port.poll(&retry), Knowledge::Known { value: ActorOutcome::NotAdmitted, .. }));
    assert_eq!(supervisor.host().unwrap().stream_snapshot().unwrap().published.visible(), b"");
    drop(supervisor);
    let (host, _) = FileOversight::open_stream(root.store(), profile(), stream()).unwrap();
    let (port, _supervisor) = host.into_actor_gateway();
    assert!(port.submit_stream(50, &intent).is_ok());
}

#[test]
fn malformed_or_stale_new_intents_do_not_consume_the_one_shot_admission_observation() {
    let root = Directory::new(); let (host, _) = create(&root);
    let original = proposal(&host, Some("valid"));
    let (port, mut supervisor) = host.into_actor_gateway();
    supply(&mut supervisor);
    let revision = supervisor.host().unwrap().revision();
    assert_eq!(port.submit_stream(0, &original).unwrap_err(), ActorError::MalformedProposal);
    let mut invalid = original.clone(); invalid.message = Some(String::new());
    assert_eq!(port.submit_stream(1, &invalid).unwrap_err(), ActorError::MalformedProposal);
    invalid.message = Some("x".repeat(MAX_MESSAGE_BYTES + 1));
    assert_eq!(port.submit_stream(1, &invalid).unwrap_err(), ActorError::Capacity);
    let mut stale = original.clone(); stale.target.expected_version += 1;
    assert_eq!(port.submit_stream(1, &stale).unwrap_err(), ActorError::Unavailable);
    stale = original.clone(); stale.expected_policy_epoch += 1;
    assert_eq!(port.submit_stream(1, &stale).unwrap_err(), ActorError::Unavailable);
    assert_eq!(supervisor.host().unwrap().revision(), revision);
    assert!(port.submit_stream(1, &original).is_ok());
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
}
