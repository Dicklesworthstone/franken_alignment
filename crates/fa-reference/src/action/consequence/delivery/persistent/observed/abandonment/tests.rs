//! Real full-input/two-key dispatches and deterministic original Store barriers.
//! These controls establish a retained liability, never external nonexecution.
use super::*;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::persistent::{FileAbandonRequest, RecoveryReserve};
use crate::action::consequence::oversight::actor::{ActorProposal, Knowledge, UnknownReason};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};

#[path = "numerical_tests.rs"]
mod numerical_tests;

fn request(host: &FileOversight, operation: u64, attempt: u64) -> FileAbandonRequest {
    let control = host.inspect().control;
    FileAbandonRequest {
        operation,
        attempt,
        expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch,
    }
}

fn bytes(host: &FileOversight) -> Vec<u8> {
    host.store.read(host.profile.delivery.limits.bytes).unwrap()
}

fn proposed(host: &mut FileOversight, attempt: u64) -> FrozenAction {
    host.propose(
        host.revision(),
        attempt,
        ActionSpec {
            version: VERSION,
            scope: host.profile.delivery.scope,
            target: Some(host.inspect().target),
            payload: b"visible".to_vec(),
            required_witnesses: Vec::new(),
            policy_epoch: host.inspect().control.ledger.epoch,
            deadline: ElapsedTick(100),
            units: 16,
        },
        snapshot(),
    )
    .unwrap()
}

// Use the exact independently configured committee in each ordinary/numerical
// host, then dispatch with both original keys. No ledger stage is synthesized.
fn reviewed_dispatch(
    host: &mut FileOversight,
    reviewer: &FileHumanReviewer,
    attempt: u64,
    action: &FrozenAction,
) {
    let source = EvidenceSnapshot::new(
        EvidenceIdentity {
            scope: host.profile.delivery.scope,
            source: 7,
            generation: 1,
        },
        snapshot(),
        BTreeMap::from([("reviewer".to_owned(), b"complete context".to_vec())]),
    )
    .unwrap();
    let inputs = source.inputs_for(action, &host.profile.committee).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone())
        .unwrap();
    let round = attempt + 100;
    host.begin_review(
        host.revision(),
        attempt,
        round,
        source.reference_root(),
        ReviewWindow {
            commit_by: ElapsedTick(5),
            reveal_by: ElapsedTick(8),
        },
        snapshot(),
    )
    .unwrap();
    let digest = commitment(
        round,
        "reviewer",
        &source.reference_root(),
        Verdict::Allow,
        b"salt",
    )
    .unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest)
        .unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(
        host.revision(),
        round,
        "reviewer",
        Verdict::Allow,
        b"salt".to_vec(),
    )
    .unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot())
        .unwrap()
        .unwrap();
    let automatic = host
        .authorize(host.revision(), attempt, &inputs, snapshot())
        .unwrap();
    let human_request = host
        .request_human_approval(
            host.revision(),
            attempt + 1000,
            attempt,
            &inputs,
            ElapsedTick(10),
        )
        .unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &human_request).unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        action,
        &inputs,
        snapshot(),
    )
    .unwrap();
}

fn dispatched(root: &Directory) -> (FileOversight, FrozenAction) {
    let (mut host, reviewer) = create(root);
    let action = proposed(&mut host, 1);
    reviewed_dispatch(&mut host, &reviewer, 1, &action);
    (host, action)
}

fn unknown(root: &Directory) -> (FileOversight, FrozenAction) {
    let (mut host, action) = dispatched(root);
    assert_eq!(
        host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::AwaitingResolution
    );
    assert_eq!(
        host.inspect().control.ledger.stages[&1],
        ActionState::Unknown
    );
    (host, action)
}

#[test]
fn abandonment_withdraws_only_the_selected_sendable_envelope_and_retains_the_charge() {
    let root = Directory::new();
    let (mut host, reviewer) = create(&root);
    let first = proposed(&mut host, 1);
    reviewed_dispatch(&mut host, &reviewer, 1, &first);
    let second = proposed(&mut host, 2);
    reviewed_dispatch(&mut host, &reviewer, 2, &second);
    host.reconcile(host.revision(), 1).unwrap();
    let before = host.inspect();
    let instruction = request(&host, 71, 1);
    let receipt = host.abandon_unknown(host.revision(), instruction).unwrap();
    let after = host.inspect();
    assert_eq!(receipt.request(), instruction);
    assert_eq!(receipt.action(), &first);
    assert_eq!(receipt.charged_units(), 16);
    assert_eq!(
        after.control.ledger.stages[&1],
        ActionState::IrrecoverablyUnknown
    );
    assert_eq!(after.control.ledger.stages[&2], ActionState::Dispatching);
    assert_eq!(
        after.control.ledger.available,
        before.control.ledger.available
    );
    assert_eq!(
        after.control.ledger.reserved,
        before.control.ledger.reserved
    );
    assert_eq!(after.control.ledger.charged, 32);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.executions, before.executions);
    assert_eq!(
        host.human_status(1001).unwrap().disposition,
        HumanDisposition::Consumed
    );
    let frozen = bytes(&host);
    assert_eq!(
        host.publish(host.revision(), 1),
        Err(JournalError::Contract(Error::Missing))
    );
    assert_eq!(bytes(&host), frozen);
    assert!(matches!(
        host.publish(host.revision(), 2).unwrap(),
        EndpointOutcome::Executed { .. }
    ));
    host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 32);
    assert_eq!(
        host.inspect().control.ledger.stages[&1],
        ActionState::IrrecoverablyUnknown
    );
}

#[test]
fn exact_retry_survives_epoch_change_but_requires_healthy_owner_and_current_revision() {
    let root = Directory::new();
    let (mut host, _) = unknown(&root);
    let stale = request(&host, 71, 1);
    host.fence(host.revision()).unwrap();
    assert!(!host.clock_ready());
    assert_eq!(
        host.abandon_unknown(host.revision(), stale),
        Err(JournalError::Contract(Error::Stale))
    );
    // An interrupted source cannot obstruct the restrictive disposition or be
    // silently acknowledged by it. No fresh clock is supplied after the fence.
    host.source_interrupted = true;
    let instruction = request(&host, 71, 1);
    let receipt = host.abandon_unknown(host.revision(), instruction).unwrap();
    assert!(host.source_interrupted);
    assert!(!host.clock_ready());
    let frozen = bytes(&host);
    assert_eq!(
        host.abandon_unknown(host.revision() - 1, instruction),
        Err(JournalError::Contract(Error::Stale))
    );
    assert_eq!(
        host.abandon_unknown(host.revision(), instruction).unwrap(),
        receipt
    );
    assert_eq!(bytes(&host), frozen);
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert!(!host.clock_ready());
    assert_ne!(
        host.inspect().control.ledger.epoch,
        instruction.expected_authority_epoch
    );
    let frozen = bytes(&host);
    assert_eq!(
        host.abandon_unknown(host.revision(), instruction).unwrap(),
        receipt
    );
    assert_eq!(host.abandonment_receipt(71).unwrap(), &receipt);
    assert_eq!(bytes(&host), frozen);
    for changed in [
        FileAbandonRequest {
            attempt: 2,
            ..instruction
        },
        FileAbandonRequest {
            expected_authority_epoch: host.inspect().control.ledger.epoch,
            ..instruction
        },
        FileAbandonRequest {
            expected_control_sequence: instruction.expected_control_sequence + 1,
            ..instruction
        },
    ] {
        assert_eq!(
            host.abandon_unknown(host.revision(), changed),
            Err(JournalError::Contract(Error::Binding))
        );
    }
    let another = request(&host, 72, 1);
    assert_eq!(
        host.abandon_unknown(host.revision(), another),
        Err(JournalError::Contract(Error::WrongState))
    );
    assert_eq!(bytes(&host), frozen);
}

#[test]
fn invalid_identity_predecessor_and_nonunknown_stages_cannot_write_a_disposition() {
    let root = Directory::new();
    let (mut host, _) = dispatched(&root);
    let instruction = request(&host, 71, 1);
    let frozen = bytes(&host);
    assert_eq!(
        host.abandon_unknown(host.revision(), instruction),
        Err(JournalError::Contract(Error::WrongState))
    );
    assert_eq!(bytes(&host), frozen);
    host.reconcile(host.revision(), 1).unwrap();
    let instruction = request(&host, 71, 1);
    let frozen = bytes(&host);
    for (changed, error) in [
        (
            FileAbandonRequest {
                operation: 0,
                ..instruction
            },
            Error::InvalidInput,
        ),
        (
            FileAbandonRequest {
                attempt: 0,
                ..instruction
            },
            Error::InvalidInput,
        ),
        (
            FileAbandonRequest {
                attempt: 9,
                ..instruction
            },
            Error::Missing,
        ),
        (
            FileAbandonRequest {
                expected_control_sequence: instruction.expected_control_sequence + 1,
                ..instruction
            },
            Error::Stale,
        ),
        (
            FileAbandonRequest {
                expected_authority_epoch: instruction.expected_authority_epoch + 1,
                ..instruction
            },
            Error::Stale,
        ),
    ] {
        assert_eq!(
            host.abandon_unknown(host.revision(), changed),
            Err(JournalError::Contract(error))
        );
        assert_eq!(bytes(&host), frozen);
    }
    host.seal_unexecuted(host.revision(), 1).unwrap();
    assert_eq!(
        host.inspect().control.ledger.stages[&1],
        ActionState::ConfirmedNotExecuted
    );
    let instruction = request(&host, 71, 1);
    let frozen = bytes(&host);
    assert_eq!(
        host.abandon_unknown(host.revision(), instruction),
        Err(JournalError::Contract(Error::WrongState))
    );
    assert_eq!(bytes(&host), frozen);
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn canonical_duplicate_abandonment_is_invalid_even_when_public_retry_is_exact() {
    let root = Directory::new();
    let (mut host, _) = unknown(&root);
    let instruction = request(&host, 71, 1);
    host.abandon_unknown(host.revision(), instruction).unwrap();
    let mut events = host.events.clone();
    events.push(Event::Core(BaseEvent::AbandonUnknown(instruction)));
    let encoded = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    let decoded = journal::decode(&host.profile, host.store.identity(), &encoded).unwrap();
    assert!(matches!(
        Machine::replay(&host.profile, &decoded),
        Err(Error::Duplicate)
    ));
    let actual = FileOversight::read_publication(root.store(), &host.profile).unwrap();
    assert_eq!(actual, host.inspect());
}

#[test]
fn failed_replacement_exposes_no_receipt_and_recovery_preserves_actual_old_or_new_liability() {
    for stage in BARRIERS {
        let root = Directory::new();
        let (mut host, _) = unknown(&root);
        let before = host.inspect();
        let instruction = request(&host, 71, 1);
        host.store.fail_once(stage);
        check_failure(
            host.abandon_unknown(host.revision(), instruction)
                .unwrap_err(),
            stage,
        );
        assert_eq!(host.inspect(), before);
        assert_eq!(host.abandonment_receipt(71), Err(JournalError::Unavailable));
        assert_eq!(
            host.abandon_unknown(host.revision(), instruction),
            Err(JournalError::Unavailable)
        );
        let disk = canonical(&host);
        let replaced = stage == JournalIo::DirectorySync;
        assert_eq!(
            disk.broker.inspect().ledger.stages[&1],
            if replaced {
                ActionState::IrrecoverablyUnknown
            } else {
                ActionState::Unknown
            }
        );
        assert_eq!(disk.abandonments.receipt(71).is_some(), replaced);
        assert_eq!(disk.broker.inspect().ledger.charged, 16);
        assert_eq!(disk.snapshot(host.events.len()).executions, 0);
        drop(host);
        let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
        if replaced {
            assert_eq!(
                host.abandon_unknown(host.revision(), instruction)
                    .unwrap()
                    .request(),
                instruction
            );
        } else {
            assert_eq!(
                host.abandonment_receipt(71),
                Err(JournalError::Contract(Error::Missing))
            );
            let instruction = request(&host, 71, 1);
            host.abandon_unknown(host.revision(), instruction).unwrap();
        }
        assert_eq!(host.inspect().control.ledger.available, 84);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(
            host.inspect().control.ledger.stages[&1],
            ActionState::IrrecoverablyUnknown
        );
        assert_eq!(host.inspect().executions, 0);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        assert_eq!(
            host.publish(host.revision(), 1),
            Err(JournalError::Contract(Error::Missing))
        );
    }
}

#[test]
fn actor_stays_unknown_and_empty_reconciliation_cannot_make_the_stopped_domain_drained() {
    let root = Directory::new();
    let (host, reviewer) = create(&root);
    let proposal = ActorProposal {
        target: host.inspect().target,
        payload: b"visible".to_vec(),
        expected_policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100),
        units: 16,
    };
    let (actor, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let ticket = actor.submit(41, &proposal).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let action = host.request_action(41).unwrap().clone();
        reviewed_dispatch(&mut host, &reviewer, 1, &action);
        let revision = host.revision();
        host.reconcile(revision, 1).unwrap();
    }
    assert_eq!(
        actor.poll(&ticket),
        Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown
        }
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        let instruction = request(&host, 71, 1);
        let revision = host.revision();
        host.abandon_unknown(revision, instruction).unwrap();
        assert_eq!(
            host.request_status(41).unwrap().disposition,
            FileRequestDisposition::Admitted {
                attempt: 1,
                stage: ActionState::IrrecoverablyUnknown
            }
        );
        let revision = host.revision();
        assert!(host.reconcile_pending(revision).unwrap().is_empty());
        let control = host.inspect().control;
        let revision = host.revision();
        host.request_stop(
            revision,
            StopRequest {
                operation: 72,
                expected_control_sequence: control.sequence,
                expected_authority_epoch: control.ledger.epoch,
            },
        )
        .unwrap();
        let revision = host.revision();
        let sweep = host.progress_stop(revision, ElapsedTick(2)).unwrap();
        assert!(sweep.outcomes.is_empty());
        assert!(sweep.progress.endpoint_fenced);
        assert_eq!(sweep.progress.unresolved, vec![1]);
        assert_eq!(sweep.progress.irrecoverable, vec![1]);
        assert_eq!(sweep.progress.charged_units, 16);
        assert!(!sweep.progress.drained());
    }
    assert_eq!(
        actor.poll(&ticket),
        Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown
        }
    );
    actor.cancel(&ticket).unwrap();
    assert_eq!(
        actor.poll(&ticket),
        Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown
        }
    );
}

#[test]
fn published_but_unknown_disposition_keeps_visible_effect_and_ordinary_reconciliation_is_a_control()
{
    for abandon in [false, true] {
        let root = Directory::new();
        let (mut host, _) = dispatched(&root);
        host.publish(host.revision(), 1).unwrap();
        host.fence(host.revision()).unwrap();
        assert_eq!(
            host.inspect().control.ledger.stages[&1],
            ActionState::Unknown
        );
        let before = host.inspect();
        if abandon {
            let instruction = request(&host, 71, 1);
            host.abandon_unknown(host.revision(), instruction).unwrap();
            assert!(!host.clock_ready());
            assert_eq!(
                host.inspect().control.ledger.stages[&1],
                ActionState::IrrecoverablyUnknown
            );
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            let frozen = bytes(&host);
            assert_eq!(
                host.reconcile(host.revision(), 1),
                Err(JournalError::Contract(Error::WrongState))
            );
            assert_eq!(bytes(&host), frozen);
        } else {
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            assert_eq!(
                host.reconcile(host.revision(), 1).unwrap(),
                Reconciliation::Resolved(EndpointOutcome::Executed {
                    resulting_version: 2
                })
            );
            assert_eq!(
                host.inspect().control.ledger.stages[&1],
                ActionState::Confirmed
            );
        }
        assert_eq!(host.inspect().payload, before.payload);
        assert_eq!(host.inspect().payload, b"visible");
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, 16);
    }
}

#[test]
fn abandonment_cannot_consume_the_unchanged_terminal_recovery_reserve() {
    let root = Directory::new();
    let mut p = profile();
    p.delivery.limits.events = 32;
    let (mut host, reviewer) = FileOversight::create(root.store(), p).unwrap();
    host.enable_recovery_reserve(0, RecoveryReserve::terminal())
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let action = proposed(&mut host, 1);
    reviewed_dispatch(&mut host, &reviewer, 1, &action);
    host.reconcile(host.revision(), 1).unwrap();
    while host.revision() < 29 {
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    }
    let instruction = request(&host, 71, 1);
    let frozen = bytes(&host);
    assert_eq!(
        host.abandon_unknown(host.revision(), instruction),
        Err(JournalError::Contract(Error::Limit))
    );
    assert_eq!(bytes(&host), frozen);
    assert!(host.storage_failure().is_none());
    host.fence(host.revision()).unwrap();
    let control = host.inspect().control;
    host.request_stop(
        host.revision(),
        StopRequest {
            operation: 72,
            expected_control_sequence: control.sequence,
            expected_authority_epoch: control.ledger.epoch,
        },
    )
    .unwrap();
    assert!(
        host.progress_stop(host.revision(), ElapsedTick(2))
            .unwrap()
            .progress
            .drained()
    );
    assert_eq!(host.revision(), 32);
    assert_eq!(host.inspect().control.ledger.charged, 0);
}
