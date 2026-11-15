//! Exact screening of actual native inference through the original durable owner.
//! Fixture monitors and synthetic helper verdicts are not detector-quality evidence.
#![cfg(unix)]

#[path = "tower_generated_operator/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::Error;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        JournalError, RecoveryReserve,
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile, source::FileSourcePolicy,
            stream::generated::FileTextMessageRequest,
        },
        requests::actor::{
            FileActorPrecheckPort, FileGeneratedTextActorPort, FilePrecheckDisclosure,
            FilePrecheckObservation, FilePrecheckOutcome,
            precheck_wire::{
                FilePrecheckWire, PrecheckRequest, decode_precheck_response,
                encode_precheck_request,
            },
        },
    },
    stream::{ReleaseFrame, StreamProfile},
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{
    actor::{ActorError, ActorOutcome, ActorProposal, Knowledge, UnknownReason},
    actor_wire::{ActorWire, Command, encode_command},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use fa_reference::action::{ActionState, ElapsedTick, FrozenAction};
use support::{Directory, Supervisor, snapshot};

const REQUEST: u64 = 701;

struct Native {
    actor: FileGeneratedTextActorPort,
    supervisor: Supervisor,
    reviewer: FileHumanReviewer,
    source: FileEvidenceSource,
    capture: EvidenceSnapshot,
}

fn capture(generation: u64, value: &[u8]) -> EvidenceSnapshot {
    let mut state = snapshot();
    state.semantic_epoch = generation;
    state.values.insert(7, value.to_vec());
    EvidenceSnapshot::new(
        EvidenceIdentity {
            scope: fixture::profile().delivery.scope,
            source: 7,
            generation,
        },
        state,
        support::oversight::MEMBERS
            .into_iter()
            .map(|member| (member.to_owned(), b"PRIVATE-GENERATED-CONTEXT".to_vec()))
            .collect(),
    )
    .unwrap()
}

fn setup(root: &Directory, payload_limit: usize, units_limit: u64) -> Native {
    let mut profile: FileOversightProfile = fixture::profile();
    profile.delivery.policy = Policy::new(
        1,
        vec![
            Predicate::ExactValue {
                key: 7,
                value: b"ok".to_vec(),
            },
            Predicate::PayloadAtMost(payload_limit),
            Predicate::UnitsAtMost(units_limit),
            Predicate::All(vec![0, 1, 2]),
        ],
    )
    .unwrap();
    let scope = profile.delivery.scope;
    let (mut host, reviewer) = FileOversight::create_generated_text_stream_with_reserve(
        root.store(),
        profile,
        StreamProfile::new(9, 1, 4, 64, 256).unwrap(),
        fixture::decoder(),
        fixture::tokenizer(),
        RecoveryReserve::terminal(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.enable_file_source(
        host.revision(),
        FileSourcePolicy {
            source: StateSource {
                scope,
                source: 7,
                generation: 1,
            },
            limits: StateLimits::default(),
            freshness: StateFreshness::new(100).unwrap(),
        },
    )
    .unwrap();
    let capture = capture(1, b"ok");
    let path = root.0.join("evidence.json");
    std::fs::write(&path, capture.encode()).unwrap();
    let mut source = FileEvidenceSource::new(path, 7, scope, 4096).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    let (actor, supervisor) = host.into_generated_text_actor_gateway().unwrap();
    Native {
        actor,
        supervisor,
        reviewer,
        source,
        capture,
    }
}

fn disclose(
    native: &Native,
    request: u64,
    intent: &ActorProposal,
    queries: usize,
) -> (FileActorPrecheckPort, FilePrecheckDisclosure) {
    let revision = native.supervisor.host().unwrap().revision();
    native
        .supervisor
        .disclose_generated_text_precheck(revision, 81, queries, request, intent)
        .unwrap()
}

fn known(observation: FilePrecheckObservation, expected: FilePrecheckOutcome, query: u64) {
    let Knowledge::Known { value, basis } = observation else {
        panic!("expected a typed exact-policy observation: {observation:?}");
    };
    assert_eq!(value, expected);
    assert_eq!(
        (basis.disclosure, basis.query, basis.policy_epoch),
        (81, query, 0)
    );
}

fn unknown(port: &FileActorPrecheckPort, intent: &ActorProposal) {
    assert_eq!(
        port.check(99, intent),
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
    );
}

fn journal(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn publish(native: &mut Native, request: u64) -> FrozenAction {
    let mut borrowed = native.supervisor.host_mut().unwrap();
    let host = &mut *borrowed;
    let attempt = support::attempt(host, request);
    let action = host.request_action(request).unwrap().clone();
    let inputs = native
        .capture
        .inputs_for(&action, &fixture::profile().committee)
        .unwrap();
    support::oversight::review_existing(host, attempt, request + 100, &inputs);
    let automatic = host
        .authorize(host.revision(), attempt, &inputs, snapshot())
        .unwrap();
    // A successful precheck and actual helper approval still lack the human key.
    assert!(
        host.publish_checked(
            host.revision(),
            attempt,
            Some(&inputs),
            snapshot(),
            ElapsedTick(2)
        )
        .is_err()
    );
    let human_request = host
        .request_human_approval(
            host.revision(),
            request + 1000,
            attempt,
            &inputs,
            ElapsedTick(31),
        )
        .unwrap();
    let revision = host.revision();
    let human = native
        .reviewer
        .approve(host, revision, &human_request)
        .unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        &inputs,
        snapshot(),
    )
    .unwrap();
    assert!(matches!(
        host.publish_checked(
            host.revision(),
            attempt,
            Some(&inputs),
            snapshot(),
            ElapsedTick(2),
        )
        .unwrap()
        .outcome,
        EndpointOutcome::Executed { .. }
    ));
    host.reconcile(host.revision(), attempt).unwrap();
    action
}

#[test]
fn exact_generated_precheck_uses_actual_frame_bytes_and_charge_without_consuming_intake() {
    for (bytes, units, expected, stage) in [
        (
            512,
            512,
            FilePrecheckOutcome::PrecheckPassed,
            ActionState::Reviewing,
        ),
        (
            32,
            512,
            FilePrecheckOutcome::DeniedByPolicy,
            ActionState::Denied,
        ),
        (
            512,
            32,
            FilePrecheckOutcome::DeniedByPolicy,
            ActionState::Denied,
        ),
    ] {
        let root = Directory::new();
        let mut native = setup(&root, bytes, units);
        let source = fixture::generate(&mut native.supervisor);
        let intent = FileGeneratedTextActorPort::encode_message(&source).unwrap();
        assert_eq!(
            native
                .supervisor
                .prepare_file_intake(&mut native.source, || ElapsedTick(1))
                .result,
            Ok(native.capture.identity())
        );
        let before = native.supervisor.host().unwrap().inspect();
        let numerical = native
            .supervisor
            .host()
            .unwrap()
            .decoder_inspection()
            .unwrap();
        let original = journal(&root);
        let reads = native.source.status().read_attempts;
        assert!(
            native
                .supervisor
                .disclose_exact_precheck(before.revision, 1, 1)
                .is_err()
        );
        let (port, grant) = disclose(&native, REQUEST, &intent, 3);
        let mut wire = FilePrecheckWire::new(port.clone(), 2).unwrap();
        let request = encode_precheck_request(&PrecheckRequest {
            query: REQUEST,
            proposal: intent.clone(),
        })
        .unwrap();
        let encoded = wire.exchange(&request).encode().unwrap();
        let response = decode_precheck_response(&encoded, REQUEST, 0).unwrap();
        known(response.result.unwrap(), expected, REQUEST);
        known(port.check(REQUEST, &intent).unwrap(), expected, REQUEST);
        for secret in [
            "PRIVATE-GENERATED-CONTEXT",
            "\"payload\"",
            "\"token_ids\"",
            "\"logits\"",
        ] {
            assert!(!std::str::from_utf8(&encoded).unwrap().contains(secret));
        }
        assert_eq!(grant.remaining_queries(), 1);
        assert_eq!(native.supervisor.host().unwrap().inspect(), before);
        assert_eq!(
            native
                .supervisor
                .host()
                .unwrap()
                .decoder_inspection()
                .unwrap(),
            numerical
        );
        assert_eq!(native.supervisor.host().unwrap().retained_requests(), 0);
        assert_eq!(journal(&root), original);
        assert_eq!(native.source.status().read_attempts, reads);
        // The same request label and one-use intake slot remain available.
        let ticket = native.actor.submit(&source).unwrap();
        {
            let host = native.supervisor.host().unwrap();
            let attempt = support::attempt(&host, REQUEST);
            assert_eq!(host.inspect().control.ledger.stages[&attempt], stage);
            assert_eq!(host.inspect().executions, 0);
            let action = host.request_action(REQUEST).unwrap();
            assert_eq!(
                ReleaseFrame::decode(&action.spec().payload)
                    .unwrap()
                    .message(),
                Some("A")
            );
            assert_eq!(action.spec().units, action.spec().payload.len() as u64);
            assert!(action.spec().units > intent.units);
        }
        unknown(&port, &intent);
        assert_eq!(grant.remaining_queries(), 1);
        if stage == ActionState::Reviewing {
            assert_eq!(
                native.actor.poll(&ticket),
                Knowledge::Pending { request: REQUEST }
            );
            let action = publish(&mut native, REQUEST);
            assert_eq!(
                native
                    .supervisor
                    .host()
                    .unwrap()
                    .inspect()
                    .control
                    .ledger
                    .charged,
                action.spec().units
            );
            assert!(matches!(
                native.actor.poll(&ticket),
                Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                }
            ));
        } else {
            assert!(matches!(
                native.actor.poll(&ticket),
                Knowledge::Known {
                    value: ActorOutcome::Denied,
                    ..
                }
            ));
        }
    }
}

#[test]
fn generated_disclosure_binds_complete_intent_and_shares_finite_revocable_queries() {
    let root = Directory::new();
    let mut native = setup(&root, 512, 512);
    let source = fixture::generate(&mut native.supervisor);
    let intent = FileGeneratedTextActorPort::encode_message(&source).unwrap();
    let (port, grant) = disclose(&native, REQUEST, &intent, 24);
    let original = journal(&root);
    for which in 0..15 {
        let mut changed = intent.clone();
        match which {
            0 => changed.payload[0] ^= 1,
            1 => changed.payload[8] ^= 1,
            2 => changed.payload[16] ^= 1,
            3 => changed.payload[31] ^= 1,
            4 => changed.payload.truncate(31),
            5 => changed.payload.push(0),
            6 => changed.units += 1,
            7 => changed.target.adapter += 1,
            8 => changed.target.object += 1,
            9 => changed.target.contract_version += 1,
            10 => changed.target.expected_version += 1,
            11 => changed.target.generation += 1,
            12 => changed.deadline = ElapsedTick(0),
            13 => changed.deadline = ElapsedTick(101),
            _ => {
                changed = FileGeneratedTextActorPort::encode_finish(
                    REQUEST,
                    source.target,
                    source.policy_epoch,
                    source.deadline,
                )
                .unwrap()
            }
        }
        let remaining = grant.remaining_queries();
        assert!(port.clone().check(which + 1, &changed).is_err());
        assert_eq!(grant.remaining_queries(), remaining - 1);
        if which < 13 {
            let revision = native.supervisor.host().unwrap().revision();
            assert!(
                native
                    .supervisor
                    .disclose_generated_text_precheck(revision, 82, 1, REQUEST, &changed,)
                    .is_err()
            );
        }
    }
    let mut stale = intent.clone();
    stale.expected_policy_epoch += 1;
    assert_eq!(
        port.check(20, &stale),
        Ok(Knowledge::Stale {
            generation: 1,
            current: 0
        })
    );
    let mut shorter = intent.clone();
    shorter.deadline = ElapsedTick(50);
    known(
        port.check(21, &shorter).unwrap(),
        FilePrecheckOutcome::PrecheckPassed,
        21,
    );
    shorter.deadline = ElapsedTick(1);
    known(
        port.check(22, &shorter).unwrap(),
        FilePrecheckOutcome::DeniedByPolicy,
        22,
    );
    known(
        port.clone().check(23, &intent).unwrap(),
        FilePrecheckOutcome::PrecheckPassed,
        23,
    );
    assert_eq!(grant.remaining_queries(), 5);
    assert_eq!(journal(&root), original);
    grant.revoke();
    assert!(matches!(
        port.check(24, &intent),
        Ok(Knowledge::Withheld { .. })
    ));
    let (port, grant) = disclose(&native, REQUEST, &intent, 1);
    known(
        port.clone().check(1, &intent).unwrap(),
        FilePrecheckOutcome::PrecheckPassed,
        1,
    );
    assert_eq!(port.check(2, &intent), Err(ActorError::Capacity));
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn disclosure_requires_completed_original_generation_and_current_source_without_advancing_it() {
    let root = Directory::new();
    let mut native = setup(&root, 512, 512);
    fixture::begin(&mut native.supervisor);
    for revision in 0..4 {
        let host = native.supervisor.host().unwrap();
        let source = FileTextMessageRequest {
            request: REQUEST,
            generation: 7,
            generation_revision: revision,
            target: host.inspect().target,
            policy_epoch: host.inspect().control.ledger.epoch,
            deadline: ElapsedTick(100),
        };
        let intent = FileGeneratedTextActorPort::encode_message(&source).unwrap();
        let numerical = host.decoder_inspection().unwrap();
        let journal_revision = host.revision();
        drop(host);
        let original = journal(&root);
        assert!(
            native
                .supervisor
                .disclose_generated_text_precheck(journal_revision, 81, 1, REQUEST, &intent)
                .is_err()
        );
        assert_eq!(
            native
                .supervisor
                .host()
                .unwrap()
                .decoder_inspection()
                .unwrap(),
            numerical
        );
        assert_eq!(journal(&root), original);
        native
            .supervisor
            .host_mut()
            .unwrap()
            .advance_decoder_text(journal_revision, 7, revision)
            .unwrap();
    }
    let source = FileTextMessageRequest {
        request: REQUEST,
        generation: 7,
        generation_revision: 4,
        target: native.supervisor.host().unwrap().inspect().target,
        policy_epoch: 0,
        deadline: ElapsedTick(200),
    };
    let intent = FileGeneratedTextActorPort::encode_message(&source).unwrap();
    let revision = native.supervisor.host().unwrap().revision();
    for (disclosure, queries, request) in [
        (0, 1, REQUEST),
        (81, 0, REQUEST),
        (81, 129, REQUEST),
        (81, 1, REQUEST + 1),
    ] {
        assert!(
            native
                .supervisor
                .disclose_generated_text_precheck(revision, disclosure, queries, request, &intent)
                .is_err()
        );
    }
    assert_eq!(
        native
            .supervisor
            .disclose_generated_text_precheck(revision - 1, 81, 1, REQUEST, &intent)
            .err(),
        Some(JournalError::Contract(Error::Stale))
    );
    let (port, grant) = disclose(&native, REQUEST, &intent, 2);
    known(
        port.check(1, &intent).unwrap(),
        FilePrecheckOutcome::PrecheckPassed,
        1,
    );
    {
        let mut host = native.supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(102)).unwrap();
    }
    unknown(&port, &intent);
    assert_eq!(grant.remaining_queries(), 1);
    let revision = native.supervisor.host().unwrap().revision();
    assert!(
        native
            .supervisor
            .disclose_generated_text_precheck(revision, 82, 1, REQUEST, &intent)
            .is_err()
    );
}

#[test]
fn finish_precheck_uses_the_confirmed_prefix_and_keeps_original_two_key_publication() {
    let root = Directory::new();
    let mut native = setup(&root, 512, 512);
    let source = fixture::generate(&mut native.supervisor);
    native
        .supervisor
        .prepare_file_intake(&mut native.source, || ElapsedTick(1))
        .result
        .unwrap();
    native.actor.submit(&source).unwrap();
    let message = publish(&mut native, REQUEST);
    let current = native.supervisor.host().unwrap().inspect();
    let request = REQUEST + 1;
    let intent =
        FileGeneratedTextActorPort::encode_finish(request, current.target, 0, ElapsedTick(100))
            .unwrap();
    native
        .supervisor
        .prepare_file_intake(&mut native.source, || ElapsedTick(2))
        .result
        .unwrap();
    let numerical = native
        .supervisor
        .host()
        .unwrap()
        .decoder_inspection()
        .unwrap();
    let original = journal(&root);
    let (port, grant) = disclose(&native, request, &intent, 2);
    known(
        port.check(1, &intent).unwrap(),
        FilePrecheckOutcome::PrecheckPassed,
        1,
    );
    assert_eq!(journal(&root), original);
    assert_eq!(
        native
            .supervisor
            .host()
            .unwrap()
            .decoder_inspection()
            .unwrap(),
        numerical
    );
    let ticket = native
        .actor
        .finish(request, intent.target, 0, intent.deadline)
        .unwrap();
    let finish = publish(&mut native, request);
    let frame = ReleaseFrame::decode(&finish.spec().payload).unwrap();
    assert!(frame.is_finish());
    assert_eq!(frame.prior_messages(), &["A"]);
    assert_eq!(finish.spec().units, finish.spec().payload.len() as u64);
    assert!(finish.spec().units > intent.units);
    let host = native.supervisor.host().unwrap();
    assert_eq!(host.stream_snapshot().unwrap().confirmed.visible(), b"A");
    assert!(host.stream_snapshot().unwrap().confirmed.finished());
    assert_eq!(
        host.inspect().control.ledger.charged,
        message.spec().units + finish.spec().units
    );
    assert!(matches!(
        native.actor.poll(&ticket),
        Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
    unknown(&port, &intent);
    assert_eq!(grant.remaining_queries(), 1);
    assert!(
        native
            .supervisor
            .disclose_generated_text_precheck(host.revision(), 82, 1, request, &intent)
            .is_err()
    );
}

#[test]
fn passing_native_precheck_cannot_replace_fresh_source_admission() {
    let root = Directory::new();
    let mut native = setup(&root, 512, 512);
    let source = fixture::generate(&mut native.supervisor);
    let intent = FileGeneratedTextActorPort::encode_message(&source).unwrap();
    let (port, grant) = disclose(&native, REQUEST, &intent, 2);
    known(
        port.check(1, &intent).unwrap(),
        FilePrecheckOutcome::PrecheckPassed,
        1,
    );
    let changed = capture(2, b"blocked");
    std::fs::write(root.0.join("evidence.json"), changed.encode()).unwrap();
    let mut wire = ActorWire::new(native.actor.clone());
    let document = encode_command(&Command::Submit {
        request: REQUEST,
        proposal: intent.clone(),
    })
    .unwrap();
    let reads = native.source.status().read_attempts;
    let numerical = native
        .supervisor
        .host()
        .unwrap()
        .decoder_inspection()
        .unwrap()
        .numerical;
    let report = native
        .supervisor
        .exchange_generated_actor_from_file(&mut wire, &document, &mut native.source, || {
            ElapsedTick(2)
        })
        .unwrap();
    assert_eq!(report.intake.unwrap().result, Ok(changed.identity()));
    assert!(matches!(
        report.response.result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Denied,
            ..
        })
    ));
    assert_eq!(native.source.status().read_attempts, reads + 1);
    let host = native.supervisor.host().unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert!(host.inspect().control.decisions.is_empty());
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    unknown(&port, &intent);
    assert_eq!(grant.remaining_queries(), 1);
}

#[test]
fn generated_disclosure_never_survives_owner_loss_or_an_equal_revision_replacement() {
    let root = Directory::new();
    let replacement_root = Directory::new();
    let mut native = setup(&root, 512, 512);
    let mut replacement = setup(&replacement_root, 512, 512);
    let source = fixture::generate(&mut native.supervisor);
    let replacement_source = fixture::generate(&mut replacement.supervisor);
    assert_eq!(source, replacement_source);
    let intent = FileGeneratedTextActorPort::encode_message(&source).unwrap();
    assert_eq!(
        native.supervisor.host().unwrap().inspect(),
        replacement.supervisor.host().unwrap().inspect()
    );
    let (port, grant) = disclose(&native, REQUEST, &intent, 3);
    known(
        port.check(1, &intent).unwrap(),
        FilePrecheckOutcome::PrecheckPassed,
        1,
    );
    {
        let mut owner = native.supervisor.host_mut().unwrap();
        let mut other = replacement.supervisor.host_mut().unwrap();
        std::mem::swap(&mut *owner, &mut *other);
    }
    let original = journal(&replacement_root);
    unknown(&port, &intent);
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(journal(&replacement_root), original);
    let (fresh, fresh_grant) = disclose(&native, REQUEST, &intent, 2);
    known(
        fresh.check(1, &intent).unwrap(),
        FilePrecheckOutcome::PrecheckPassed,
        1,
    );
    drop(native);
    unknown(&fresh, &intent);
    assert_eq!(fresh_grant.remaining_queries(), 1);
}
