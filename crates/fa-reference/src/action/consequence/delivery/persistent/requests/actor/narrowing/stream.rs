//! Real cumulative stream releases retain exact targets and uncertain charges.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversightProfile};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};

fn stream_profile() -> FileOversightProfile {
    let mut selected = profile();
    selected.delivery.initial_payload.clear();
    selected.delivery.total = 10_000;
    selected.delivery.policy = Policy::new(1, vec![
        Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::PayloadAtMost(1024), Predicate::All(vec![0, 1]),
    ]).unwrap();
    selected
}

fn owner(root: &Root) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_stream(root.store(), stream_profile(),
        StreamProfile::new(91, 1, 4, 16, 64).unwrap()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}

fn submit_message(host: &mut FileOversight, request: u64, message: &str) {
    let spec = host.stream_message_spec(message, ElapsedTick(100)).unwrap();
    host.submit_request(host.revision(), request, spec, snapshot()).unwrap();
}

#[test]
fn retained_stream_release_publishes_once_but_its_successor_version_is_not_implicitly_retained() {
    let root = Root::new();
    let (mut host, reviewer) = owner(&root);
    submit_message(&mut host, 700, "kept");
    let (action, inputs, automatic, request) = prepare_existing(&mut host, 700);
    let revision = host.revision();
    let old_human = reviewer.approve(&mut host, revision, &request).unwrap();
    let target = action.spec().target.unwrap();
    let before = host.stream_snapshot().unwrap();
    let input = native(&host, 1, vec![target]);
    let restricted = host.narrow_authority(host.revision(), input).unwrap();
    assert!(restricted.change().cancelled.is_empty());
    assert_eq!(restricted.change().refunded_units, 0);
    assert_eq!(host.stream_snapshot().unwrap().confirmed, before.confirmed);
    assert_eq!(host.stream_snapshot().unwrap().published, before.published);
    assert!(host.dispatch(host.revision(), &automatic, &old_human, &action, &inputs, snapshot()).is_err());
    let request = host.request_human_approval(host.revision(), 2001, 1, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(1))
        .unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    let published = host.stream_snapshot().unwrap();
    assert_eq!(published.published.visible(), b"kept");
    assert_eq!(published.confirmed, published.published);
    assert_eq!(published.confirmed_target.expected_version, target.expected_version + 1);
    assert_eq!(published.publication.control.ceiling.unwrap().targets(), vec![target]);
    // Builders may describe a current unreviewed frame; actual admission must
    // deny both a new message and Finish at the excluded exact successor target.
    for (request, finish) in [(701, false), (702, true)] {
        let spec = if finish { host.stream_finish_spec(ElapsedTick(100)).unwrap() }
            else { host.stream_message_spec("next", ElapsedTick(100)).unwrap() };
        assert_eq!(spec.target.unwrap().expected_version, target.expected_version + 1);
        let status = host.submit_request(host.revision(), request, spec, snapshot()).unwrap();
        assert!(!matches!(status.disposition, FileRequestDisposition::Admitted { .. }));
    }
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.stream_snapshot().unwrap().published, published.published);
    assert!(!host.stream_snapshot().unwrap().published.finished());
}

#[test]
fn stream_narrowing_cannot_reuse_a_consumed_human_key_or_erase_unknown_delivery() {
    for published in [false, true] {
        let root = Root::new();
        let (mut host, reviewer) = owner(&root);
        submit_message(&mut host, 700, "candidate");
        let (action, inputs, automatic, request) = prepare_existing(&mut host, 700);
        let revision = host.revision();
        let human = reviewer.approve(&mut host, revision, &request).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        if published {
            assert!(matches!(host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(1))
                .unwrap().outcome, EndpointOutcome::Executed { .. }));
        } else {
            assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
        }
        let before = host.stream_snapshot().unwrap();
        let charged = before.publication.control.ledger.charged;
        assert!(charged > 0);
        assert_eq!(before.pending, Some(1));
        let retained = if published { Vec::new() } else { vec![action.spec().target.unwrap()] };
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.narrow_port(1, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
        let restricted = receipt(port.narrow(plan(port.next(1, retained)).request().unwrap()));
        assert!(restricted.cancelled().is_empty());
        assert_eq!(restricted.refunded_units(), 0);
        assert_eq!(grant.remaining_numerical_replay_events(), None);
        let mut host = supervisor.host_mut().unwrap();
        let after = host.stream_snapshot().unwrap();
        assert_eq!(after.confirmed, before.confirmed);
        assert_eq!(after.published, before.published);
        assert_eq!(after.pending, before.pending);
        assert_eq!(after.publication.control.ledger.charged, charged);
        assert_eq!(after.publication.control.ledger.epoch, before.publication.control.ledger.epoch);
        assert_eq!(after.publication.executions, before.publication.executions);
        if !published {
            // Even a retained destination cannot reuse the consumed human key
            // whose full control predecessor changed after dispatch.
            let revision = host.revision();
            assert!(matches!(host.publish_checked(revision, 1, Some(&inputs), snapshot(), ElapsedTick(1))
                .unwrap().outcome, EndpointOutcome::NotExecuted { .. }));
        }
        let revision = host.revision();
        host.reconcile(revision, 1).unwrap();
        let after = host.stream_snapshot().unwrap();
        // The restriction never refunded Unknown. Only this later original
        // endpoint nonexecution receipt may return its charged rights.
        assert_eq!(after.publication.control.ledger.charged, if published { charged } else { 0 });
        assert_eq!(after.publication.executions, u64::from(published));
        assert_eq!(after.publication.control.ledger.stages[&1], if published {
            ActionState::Confirmed } else { ActionState::ConfirmedNotExecuted });
        assert_eq!(after.published.visible(), if published { &b"candidate"[..] } else { &b""[..] });
    }
}
