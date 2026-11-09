//! Original source and durable owners, complete native policy corpora, and
//! causal refusal controls. No constructed success or promotion key is used.
use super::*;
use crate::action::consequence::delivery::persistent::governance::PolicyUpdate;
use crate::action::consequence::gate::containment::session::policy::{
    MAX_POLICY_LITERAL_BYTES, Predicate, Truth,
};
use crate::action::consequence::policy_campaign::{
    MAX_REPLAY_CASES, MAX_REPLAY_INPUT_BYTES, PolicyDelta, ReplayCaseId,
};
use crate::action::{ActionState, ResolvedTarget};

mod fixture;
use fixture::*;

fn limits() -> ReplayLimits {
    ReplayLimits {
        cases: 16,
        input_bytes: 1_048_576,
    }
}

fn candidate() -> Policy {
    Policy::new(2, vec![Predicate::PayloadAtMost(8)]).unwrap()
}

fn known(result: FilePolicyReplayResult) -> FilePolicyReplay {
    let Ok(Knowledge::Known { value, basis }) = result else {
        panic!("expected original complete policy replay: {result:?}");
    };
    assert_eq!(value.basis(), basis);
    value
}

fn assert_unknown(result: FilePolicyReplayResult) {
    assert_eq!(result, unknown());
}

#[test]
fn original_denials_and_admissions_are_compared_without_source_or_snapshot_spending() {
    let root = Root::new();
    let (mut host, source) = source_owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    host.submit_request(host.revision(), 902, spec(b"blocked"), snapshot())
        .unwrap();
    let expected = host.replay_candidate_policy(candidate(), limits()).unwrap();
    assert_eq!(expected.cases().len(), 2);
    assert_eq!(expected.newly_reviewable(), [ReplayCaseId::Proposal(2)]);
    let (actor, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    assert_eq!(grant.replay_limits(), limits());
    assert_eq!(grant.retained_bytes(), 0);
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let reads = source.status().read_attempts;
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    std::fs::remove_file(root.source()).unwrap();
    let report = known(port.replay_candidate_policy(u64::MAX, 1, candidate()));
    assert_eq!(report.report(), &expected);
    assert_eq!(report.limits(), limits());
    assert_eq!(report.remaining_queries(), 1);
    assert_eq!(
        report.basis(),
        FilePolicyReplayBasis {
            scope: profile().delivery.scope,
            query: u64::MAX,
            journal_revision: before.revision,
            control_sequence: before.control.sequence,
            policy_epoch: before.control.ledger.epoch,
            policy_generation: 1,
            recorded_at: Some(ElapsedTick(1)),
        }
    );
    assert_eq!(
        report.report().cases()[1].original().result(),
        Truth::Violated
    );
    assert_eq!(
        report.report().cases()[1].candidate().unwrap().result(),
        Truth::Satisfied
    );
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
    assert!(grant.retained_bytes() > expected.input_bytes());
    assert!(grant.retained_bytes() <= MAX_POLICY_REPLAY_RETAINED_BYTES);
    // The experiment did not consume the unused one-use admission snapshot.
    actor.submit(903, &proposal(b"ok")).unwrap();
    assert_eq!(
        supervisor.host().unwrap().inspect().control.ledger.stages[&3],
        ActionState::Reviewing
    );
}

#[test]
fn missing_witnesses_in_a_satisfied_boolean_arm_still_require_shadow() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    host.submit_request(host.revision(), 902, spec(b"blocked"), snapshot())
        .unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.policy_replay_port(3, limits()).unwrap();
    let baseline = known(port.replay_candidate_policy(1, 1, candidate()));
    assert_eq!(
        baseline.report().newly_reviewable(),
        [ReplayCaseId::Proposal(2)]
    );
    // 99 was present in the original Snapshot but was not selected by policy;
    // 98 was absent but has no retained negative observation. Neither is known.
    for (query, key) in [(2, 99), (3, 98)] {
        let policy = Policy::new(
            2,
            vec![
                Predicate::PayloadAtMost(8),
                Predicate::Absent { key },
                Predicate::Any(vec![0, 1]),
            ],
        )
        .unwrap();
        let replay = known(port.replay_candidate_policy(query, 1, policy));
        assert_eq!(replay.report().cases().len(), 2);
        assert!(replay.report().requires_shadow());
        assert!(replay.report().newly_reviewable().is_empty());
        assert!(replay.report().cases().iter().all(|case| {
            case.delta() == PolicyDelta::RequiresShadow
                && case.candidate().is_none()
                && case.missing_nodes() == [1]
        }));
    }
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn exact_campaign_retries_keep_original_cut_after_a_denial_advances_only_history() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    let first = known(port.replay_candidate_policy(10, 1, candidate()));
    let retained = grant.retained_bytes();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.submit_request(revision, 902, spec(b"blocked"), snapshot())
            .unwrap();
    }
    let after = supervisor.host().unwrap().inspect();
    assert!(after.revision > first.basis().journal_revision);
    assert_eq!(after.control.sequence, first.basis().control_sequence);
    assert_eq!(after.control.ledger.epoch, first.basis().policy_epoch);
    assert_eq!(
        known(port.clone().replay_candidate_policy(10, 1, candidate())),
        first
    );
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.retained_bytes(), retained);
    let newer = known(port.replay_candidate_policy(11, 1, candidate()));
    assert_eq!(newer.basis().journal_revision, after.revision);
    assert_eq!(newer.report().cases().len(), 2);
    assert_eq!(first.report().cases().len(), 1);
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(
        known(port.replay_candidate_policy(10, 1, candidate())),
        first
    );
    assert_eq!(
        port.replay_candidate_policy(12, 1, candidate()),
        Err(ActorError::Capacity)
    );
}

#[test]
fn complete_candidate_and_expected_predecessor_bind_each_campaign_identity() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    let high = Policy::new(u64::MAX, vec![Predicate::PayloadAtMost(8)]).unwrap();
    let first = known(port.replay_candidate_policy(9, 1, high.clone()));
    let retained = grant.retained_bytes();
    for (expected, policy) in [
        (2, high.clone()),
        (
            1,
            Policy::new(u64::MAX, vec![Predicate::PayloadAtMost(7)]).unwrap(),
        ),
        (1, candidate()),
    ] {
        assert_eq!(
            port.replay_candidate_policy(9, expected, policy),
            Err(ActorError::IdempotencyConflict)
        );
    }
    assert_eq!(known(port.replay_candidate_policy(9, 1, high)), first);
    assert_eq!(first.report().candidate_policy().generation(), u64::MAX);
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(grant.retained_bytes(), retained);
    for (query, expected, policy) in [
        (0, 1, candidate()),
        (10, 0, candidate()),
        (10, 2, candidate()),
    ] {
        assert_eq!(
            port.replay_candidate_policy(query, expected, policy),
            Err(ActorError::MalformedProposal)
        );
    }
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn empty_and_stale_campaigns_are_one_shot_even_after_the_corpus_and_policy_change() {
    let root = Root::new();
    let (actor, mut supervisor) = owner(&root).into_actor_gateway();
    let (port, grant) = supervisor.policy_replay_port(4, limits()).unwrap();
    assert_unknown(port.replay_candidate_policy(1, 1, candidate()));
    let next = Policy::new(3, vec![Predicate::PayloadAtMost(8)]).unwrap();
    let stale = port.replay_candidate_policy(2, 2, next.clone());
    assert_eq!(
        stale,
        Ok(Knowledge::Stale {
            generation: 2,
            current: 1
        })
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        let cut = host.inspect();
        let update = PolicyUpdate::new(
            77,
            cut.control.sequence,
            cut.control.ledger.epoch,
            candidate(),
        )
        .unwrap();
        let revision = host.revision();
        host.replace_policy(revision, &update).unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let state = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(state.revision, Some(snapshot()))
        .unwrap();
    let mut proposal = proposal(b"ok");
    proposal.expected_policy_epoch = state.control.ledger.epoch;
    actor.submit(901, &proposal).unwrap();
    assert_unknown(port.replay_candidate_policy(1, 1, candidate()));
    assert_eq!(port.replay_candidate_policy(2, 2, next.clone()), stale);
    assert_eq!(grant.remaining_queries(), 2);
    let current = known(port.replay_candidate_policy(3, 2, next));
    assert_eq!(current.report().previous_policy(), &candidate());
    assert_eq!(current.report().cases().len(), 1);
    assert_eq!(current.basis().policy_generation, 2);
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn full_corpus_and_input_bounds_refuse_without_a_prefix_and_retries_do_not_spend_again() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    host.submit_request(host.revision(), 902, spec(b"blocked"), snapshot())
        .unwrap();
    let expected = host.replay_candidate_policy(candidate(), limits()).unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    for bound in [
        ReplayLimits {
            cases: 1,
            ..limits()
        },
        ReplayLimits {
            input_bytes: expected.input_bytes() - 1,
            ..limits()
        },
    ] {
        let (port, grant) = supervisor.policy_replay_port(2, bound).unwrap();
        for _ in 0..3 {
            assert_eq!(
                port.replay_candidate_policy(1, 1, candidate()),
                Err(ActorError::Capacity)
            );
        }
        assert_eq!(grant.remaining_queries(), 1);
        assert!(grant.retained_bytes() > 0);
    }
    let (port, _) = supervisor.policy_replay_port(1, limits()).unwrap();
    // Losing the custodian never bypasses the earlier capacity failures.
    assert!(matches!(
        port.replay_candidate_policy(1, 1, candidate()),
        Ok(Knowledge::Withheld { .. })
    ));
    let exact = ReplayLimits {
        cases: expected.cases().len(),
        input_bytes: expected.input_bytes(),
    };
    let (port, _grant) = supervisor.policy_replay_port(1, exact).unwrap();
    assert_eq!(
        known(port.replay_candidate_policy(1, 1, candidate())).report(),
        &expected
    );
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn cached_results_reauthorize_original_owner_custody_and_borrow_health_first() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.policy_replay_port(3, limits()).unwrap();
    let clone = port.clone();
    let first = known(port.replay_candidate_policy(1, 1, candidate()));
    assert!(port.belongs_to(&actor));
    let other_root = Root::new();
    let (other_actor, _other_supervisor) = owner(&other_root).into_actor_gateway();
    assert!(!port.belongs_to(&other_actor));
    let mut wrong_scope = port.clone();
    wrong_scope.scope.branch += 1;
    assert!(!wrong_scope.belongs_to(&actor));
    assert_unknown(wrong_scope.replay_candidate_policy(1, 1, candidate()));
    {
        let _busy = supervisor.owner.borrow_mut();
        assert_unknown(port.replay_candidate_policy(1, 1, candidate()));
        assert!(matches!(
            supervisor.policy_replay_port(1, limits()),
            Err(JournalError::Unavailable)
        ));
    }
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(
        known(clone.replay_candidate_policy(1, 1, candidate())),
        first
    );
    grant.revoke();
    for port in [&port, &clone] {
        assert_eq!(
            port.replay_candidate_policy(1, 1, candidate()),
            Ok(Knowledge::Withheld {
                authority_required: "supervisor_policy_replay"
            })
        );
    }
}

#[test]
fn dropped_owner_releases_its_lock_and_old_cached_results_cannot_follow_reopen() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (old_actor, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    let first = known(port.replay_candidate_policy(1, 1, candidate()));
    drop(supervisor);
    assert_unknown(port.replay_candidate_policy(1, 1, candidate()));
    assert_eq!(grant.remaining_queries(), 1);
    let (reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    let (actor, supervisor) = reopened.into_actor_gateway();
    assert!(port.belongs_to(&old_actor));
    assert!(!port.belongs_to(&actor));
    let (fresh, _fresh_grant) = supervisor.policy_replay_port(1, limits()).unwrap();
    let replay = known(fresh.replay_candidate_policy(1, 1, candidate()));
    assert_eq!(replay.report(), first.report());
    assert!(replay.basis().journal_revision > first.basis().journal_revision);
    assert!(replay.basis().policy_epoch > first.basis().policy_epoch);
    assert_unknown(port.replay_candidate_policy(1, 1, candidate()));
}

#[test]
fn acknowledged_storage_failure_suppresses_cached_and_new_experiments() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.policy_replay_port(2, limits()).unwrap();
    known(port.replay_candidate_policy(1, 1, candidate()));
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"existing staging must remain").unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
        assert!(host.storage_failure().is_some());
    }
    let files = root.files();
    assert_unknown(port.replay_candidate_policy(1, 1, candidate()));
    assert_unknown(port.replay_candidate_policy(2, 1, candidate()));
    assert_eq!(grant.remaining_queries(), 1);
    assert!(matches!(
        supervisor.policy_replay_port(1, limits()),
        Err(JournalError::Unavailable)
    ));
    assert_eq!(root.files(), files);
    assert_eq!(
        std::fs::read(pending).unwrap(),
        b"existing staging must remain"
    );
}

#[test]
fn campaign_cache_accounts_for_policy_and_trace_metadata_and_retains_capacity_errors() {
    let root = Root::new();
    let mut host = owner(&root);
    host.submit_request(host.revision(), 901, spec(b"ok"), snapshot())
        .unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor
        .policy_replay_port(MAX_POLICY_REPLAY_QUERIES, limits())
        .unwrap();
    let before = supervisor.host().unwrap().inspect();
    let files = root.files();
    let mut failed = None;
    for query in 1..=MAX_POLICY_REPLAY_QUERIES as u64 {
        let available = MAX_POLICY_REPLAY_RETAINED_BYTES - grant.retained_bytes();
        // Fill with actual replay reports. At the final boundary this literal
        // fits the request alone, but its retained report cannot also fit.
        let literal = MAX_POLICY_LITERAL_BYTES.min(available / 2);
        let candidate = Policy::new(2, vec![Predicate::PayloadIs(vec![b'x'; literal])]).unwrap();
        assert!(retention::request_bytes(&candidate).unwrap() < available);
        let remaining = grant.remaining_queries();
        match port.replay_candidate_policy(query, 1, candidate.clone()) {
            Ok(Knowledge::Known { value, .. }) => {
                assert!(value.report().input_bytes() < 100);
                assert_eq!(value.report().cases().len(), 1);
            }
            Err(ActorError::Capacity) => {
                assert_eq!(grant.remaining_queries(), remaining - 1);
                failed = Some((query, candidate));
                break;
            }
            other => panic!("unexpected cache-boundary result: {other:?}"),
        }
        assert_eq!(grant.remaining_queries(), remaining - 1);
    }
    let (query, candidate) = failed.expect("whole report crosses cumulative retained-data bound");
    let remaining = grant.remaining_queries();
    let retained = grant.retained_bytes();
    for _ in 0..3 {
        assert_eq!(
            port.clone()
                .replay_candidate_policy(query, 1, candidate.clone()),
            Err(ActorError::Capacity)
        );
    }
    assert_eq!(grant.remaining_queries(), remaining);
    assert_eq!(grant.retained_bytes(), retained);
    assert!(retained <= MAX_POLICY_REPLAY_RETAINED_BYTES);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn grants_refuse_empty_or_excessive_bounds_without_issuing_custody() {
    let root = Root::new();
    let (_, supervisor) = owner(&root).into_actor_gateway();
    for (queries, bound) in [
        (0, limits()),
        (
            1,
            ReplayLimits {
                cases: 0,
                ..limits()
            },
        ),
        (
            1,
            ReplayLimits {
                input_bytes: 0,
                ..limits()
            },
        ),
    ] {
        assert!(matches!(
            supervisor.policy_replay_port(queries, bound),
            Err(JournalError::Contract(Error::InvalidInput))
        ));
    }
    for (queries, bound) in [
        (MAX_POLICY_REPLAY_QUERIES + 1, limits()),
        (
            1,
            ReplayLimits {
                cases: MAX_REPLAY_CASES + 1,
                ..limits()
            },
        ),
        (
            1,
            ReplayLimits {
                input_bytes: MAX_REPLAY_INPUT_BYTES + 1,
                ..limits()
            },
        ),
    ] {
        assert!(matches!(
            supervisor.policy_replay_port(queries, bound),
            Err(JournalError::Contract(Error::Limit))
        ));
    }
    let (_, grant) = supervisor
        .policy_replay_port(
            MAX_POLICY_REPLAY_QUERIES,
            ReplayLimits {
                cases: MAX_REPLAY_CASES,
                input_bytes: MAX_REPLAY_INPUT_BYTES,
            },
        )
        .unwrap();
    assert_eq!(grant.remaining_queries(), MAX_POLICY_REPLAY_QUERIES);
    assert_eq!(grant.retained_bytes(), 0);
}

mod numerical;
