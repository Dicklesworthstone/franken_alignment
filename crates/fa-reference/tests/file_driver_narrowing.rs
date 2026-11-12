//! Original socket review, retained driver custody and two-key publication.
//! A local continuation is not a fresh review, source observation or human key.
#![cfg(unix)]
#[path = "support/file_driver.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, Reconciliation, governance::PolicyUpdate,
    observed::{
        FileHumanPermit,
        driver::{FileDriverEvent, FileDriverPhase},
        narrowing::{FileAuthorityNarrowReceipt, FileAuthorityNarrowRequest},
    },
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::action::{ActionState, ElapsedTick, ResolvedTarget};
use fa_reference::round::Verdict;
use fixture::*;

const REQUEST: u64 = 90;

fn image(rig: &Rig) -> Vec<u8> {
    std::fs::read(rig.root.store().join("delivery.bin")).unwrap()
}
fn target(rig: &Rig) -> ResolvedTarget {
    rig.driver.supervisor().host().unwrap().request_action(REQUEST)
        .unwrap().spec().target.unwrap()
}
fn restrict(rig: &mut Rig, operation: u64, targets: Vec<ResolvedTarget>)
    -> FileAuthorityNarrowReceipt
{
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let state = host.inspect();
    let request = FileAuthorityNarrowRequest::new(
        operation, state.control.sequence, state.control.ledger.epoch, targets,
    ).unwrap();
    host.narrow_authority(state.revision, request).unwrap()
}
fn retain(rig: &mut Rig, operation: u64) -> FileAuthorityNarrowReceipt {
    let retained = target(rig);
    restrict(rig, operation, vec![retained])
}
fn step(rig: &mut Rig, human: Option<&FileHumanPermit>)
    -> Result<FileDriverEvent, JournalError>
{
    let inputs = rig.inputs.clone();
    rig.driver.step_with_evidence(
        || ElapsedTick(1),
        |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }),
        human,
    )
}
fn confirmed(rig: &mut Rig, human: &FileHumanPermit) -> u64 {
    let attempt = match step(rig, Some(human)).unwrap() {
        FileDriverEvent::Dispatched { request: REQUEST, attempt } => attempt,
        other => panic!("original two-key dispatch missing: {other:?}"),
    };
    assert!(matches!(step(rig, None).unwrap(),
        FileDriverEvent::Published {
            request: REQUEST,
            outcome: EndpointOutcome::Executed { resulting_version: 2 },
        }));
    assert!(matches!(step(rig, None).unwrap(),
        FileDriverEvent::Reconciled {
            request: REQUEST,
            outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }),
        }));
    attempt
}

#[test]
fn ready_reviewing_job_continues_only_explicitly_and_needs_a_fresh_human_key() {
    let mut rig = Rig::new();
    let ticket = rig.submit(REQUEST);
    let reviewed = rig.reviewed(REQUEST);
    assert_eq!(reviewed.policy.control.decision.consequence, Consequence::Continue);
    let old_human = rig.human(1001, 30);
    let narrowed = retain(&mut rig, 501);
    let before = rig.driver.supervisor().host().unwrap().inspect();
    let bytes = image(&rig);
    assert_eq!(before.control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(before.control.ledger.reserved, 0);
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: REQUEST });
    assert_eq!(step(&mut rig, Some(&old_human)).unwrap_err(),
        JournalError::Contract(Error::Stale));
    assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), Some(501));
    assert!(rig.driver.continue_after_narrowing(REQUEST + 1, 501).is_err());
    assert!(rig.driver.continue_after_narrowing(REQUEST, 502).is_err());

    let continued = rig.driver.continue_after_narrowing(REQUEST, 501).unwrap();
    assert_eq!(continued.request(), REQUEST);
    assert_eq!(continued.attempt(), 1);
    assert_eq!(continued.operation(), 501);
    assert_eq!(continued.previous_control_sequence(), reviewed.policy.control.sequence);
    assert_eq!(continued.control_sequence(), narrowed.change().sequence);
    assert_eq!(continued.authority_epoch(), before.control.ledger.epoch);
    assert_eq!(continued.narrowing_operations(), &[501]);
    assert!(!continued.retained_automatic_permit());
    assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), None);
    assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 501).unwrap(), continued);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(image(&rig), bytes);
    assert_eq!(rig.driver.supervisor().host().unwrap().human_status(1001)
        .unwrap().disposition, HumanDisposition::Revoked);
    assert!(matches!(step(&mut rig, None).unwrap(),
        FileDriverEvent::AwaitingHuman { request: REQUEST }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 0);

    // The original native key was withdrawn. It cannot be relabeled as approval
    // at the new cut; a refused attempt may retain its automatic reservation.
    assert!(step(&mut rig, Some(&old_human)).is_err());
    let reserved = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(reserved.executions, 0);
    assert_eq!(reserved.control.ledger.reserved, 16);
    assert_eq!(reserved.control.ledger.charged, 0);
    let fresh_human = rig.human(1002, 30);
    let attempt = confirmed(&mut rig, &fresh_human);
    let after = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.control.sequence, narrowed.change().sequence);
    assert_eq!(after.control.ledger.stages[&attempt], ActionState::Confirmed);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.charged, 16);
    assert_eq!(after.executions, 1);
    assert_eq!(after.payload, b"publication");
    assert!(matches!(rig.port.poll(&ticket),
        Knowledge::Known { value: ActorOutcome::Executed, .. }));
}

#[test]
fn consecutive_restrictions_preserve_the_same_original_automatic_reservation() {
    let mut rig = Rig::new();
    rig.submit(REQUEST);
    rig.reviewed(REQUEST);
    let old_human = rig.human(1001, 30);
    let mut foreign = Rig::new();
    foreign.submit(REQUEST);
    foreign.reviewed(REQUEST);
    let wrong = foreign.human(1001, 30);
    assert_eq!(step(&mut rig, Some(&wrong)).unwrap_err(),
        JournalError::Contract(Error::Binding));
    let reserved = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(reserved.control.ledger.stages[&1], ActionState::Authorized);
    assert_eq!(reserved.control.ledger.reserved, 16);
    let first = retain(&mut rig, 902);
    let second = retain(&mut rig, 401); // Operation IDs are not control order.
    assert_eq!(first.change().refunded_units + second.change().refunded_units, 0);
    let before = rig.driver.supervisor().host().unwrap().inspect();
    let bytes = image(&rig);
    assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), Some(401));
    assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 902),
        Err(JournalError::Contract(Error::Stale)));
    let continued = rig.driver.continue_after_narrowing(REQUEST, 401).unwrap();
    assert_eq!(continued.narrowing_operations(), &[902, 401]);
    assert!(continued.retained_automatic_permit());
    assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 401).unwrap(), continued);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(image(&rig), bytes);
    assert!(step(&mut rig, Some(&old_human)).is_err());
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    let fresh_human = rig.human(1002, 30);
    confirmed(&mut rig, &fresh_human);
    let after = rig.driver.supervisor().host().unwrap().inspect();
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.available, reserved.control.ledger.available);
    assert_eq!(after.control.ledger.charged, 16);
}

#[test]
fn a_control_change_between_restrictions_cannot_borrow_a_later_receipt() {
    let mut rig = Rig::new();
    rig.submit(REQUEST);
    rig.reviewed(REQUEST);
    retain(&mut rig, 501);
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        // A real independent completed control decision advances the sequence
        // without changing the retained job's action, input, epoch or decision.
        helper::oversight::reviewed(&mut host, 8, b"other");
    }
    retain(&mut rig, 502);
    let before = rig.driver.supervisor().host().unwrap().inspect();
    let bytes = image(&rig);
    assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 502),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(image(&rig), bytes);
    assert_eq!(step(&mut rig, None).unwrap_err(), JournalError::Contract(Error::Stale));
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: REQUEST });
}

#[test]
fn missing_changed_or_restored_input_bytes_cannot_revive_the_old_review_cut() {
    for mode in 0..3 {
        let mut rig = Rig::new();
        rig.submit(REQUEST);
        rig.reviewed(REQUEST);
        retain(&mut rig, 501);
        {
            let inputs = rig.inputs.clone().unwrap();
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            let input_revision = host.input_revision(1).unwrap();
            if mode == 1 {
                let changed = helper::inputs(inputs.action(), b"new complete source evidence");
                host.record_inputs(revision, 1, input_revision, changed).unwrap();
            } else {
                host.inputs_unavailable(revision, 1, input_revision).unwrap();
                if mode == 2 {
                    let revision = host.revision();
                    let input_revision = host.input_revision(1).unwrap();
                    host.record_inputs(revision, 1, input_revision, inputs).unwrap();
                }
            }
        }
        let before = rig.driver.supervisor().host().unwrap().inspect();
        let bytes = image(&rig);
        assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST),
            Err(JournalError::Contract(Error::Stale)), "mode {mode}");
        assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 501),
            Err(JournalError::Contract(Error::Stale)), "mode {mode}");
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
        assert_eq!(image(&rig), bytes);
        assert_eq!(before.executions, 0);
        assert_eq!(before.control.ledger.charged, 0);
    }
}

#[test]
fn continuation_does_not_replace_fresh_evidence_or_reset_human_request_capacity() {
    {
        let mut rig = Rig::new();
        rig.submit(REQUEST);
        rig.reviewed(REQUEST);
        retain(&mut rig, 501);
        rig.driver.continue_after_narrowing(REQUEST, 501).unwrap();
        let result = rig.driver.step_with_evidence(
            || ElapsedTick(1), |_, _| Err(Error::Incomplete), None,
        );
        assert_eq!(result.unwrap_err(), JournalError::Contract(Error::Incomplete));
        let after = rig.driver.supervisor().host().unwrap().inspect();
        assert_eq!(after.executions, 0);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.control.ledger.charged, 0);
        assert!(rig.driver.continue_after_narrowing(REQUEST, 501).is_err());
    }
    {
        let mut rig = Rig::new();
        rig.submit(REQUEST);
        rig.reviewed(REQUEST);
        let limit = profile().human.max_requests;
        for index in 0..limit {
            let operation = 100 + index as u64;
            retain(&mut rig, operation);
            rig.driver.continue_after_narrowing(REQUEST, operation).unwrap();
            let _human = rig.human(1000 + index as u64, 30);
        }
        retain(&mut rig, 999);
        rig.driver.continue_after_narrowing(REQUEST, 999).unwrap();
        let inputs = rig.inputs.clone().unwrap();
        let result = rig.driver.request_human_approval(
            9000, &inputs, ElapsedTick(30), ElapsedTick(1),
        );
        assert_eq!(result.unwrap_err(), JournalError::Contract(Error::Limit));
        let after = rig.driver.supervisor().host().unwrap().inspect();
        assert_eq!(after.executions, 0);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.control.ledger.charged, 0);
    }
}

#[test]
fn incomplete_socket_review_is_not_rebased_or_silently_convened_again() {
    let mut rig = Rig::new();
    rig.submit(REQUEST);
    rig.start(REQUEST, 190);
    retain(&mut rig, 501);
    let before = rig.driver.supervisor().host().unwrap().inspect();
    let bytes = image(&rig);
    assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), None);
    assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 501),
        Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(image(&rig), bytes);
    assert!(matches!(rig.review_event(Verdict::Allow, false),
        FileDriverEvent::ReviewRejected { error: Error::Stale, .. }));
    assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), None);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn cancelled_fenced_or_policy_replaced_jobs_cannot_reuse_a_narrowing_result() {
    for mode in 0..3 {
        let mut rig = Rig::new();
        rig.submit(REQUEST);
        rig.reviewed(REQUEST);
        retain(&mut rig, 501);
        rig.driver.continue_after_narrowing(REQUEST, 501).unwrap();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let state = host.inspect();
            match mode {
                0 => { host.cancel(state.revision, 1).unwrap(); }
                1 => { host.fence(state.revision).unwrap(); }
                _ => {
                    let update = PolicyUpdate::new(
                        701, state.control.sequence, state.control.ledger.epoch,
                        Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap(),
                    ).unwrap();
                    host.replace_policy(state.revision, &update).unwrap();
                }
            }
        }
        let before = rig.driver.supervisor().host().unwrap().inspect();
        let bytes = image(&rig);
        assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), None);
        assert!(rig.driver.continue_after_narrowing(REQUEST, 501).is_err());
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
        assert_eq!(image(&rig), bytes);
        assert!(matches!(rig.driver.step_with_evidence(
            || panic!("cancelled request acquired time"),
            |_, _| panic!("cancelled request acquired evidence"), None,
        ).unwrap(), FileDriverEvent::Stopped { stage: ActionState::Cancelled, .. }));
    }
    let mut rig = Rig::new();
    rig.submit(REQUEST);
    rig.reviewed(REQUEST);
    let receipt = restrict(&mut rig, 501, Vec::new());
    assert_eq!(receipt.change().cancelled, vec![1]);
    assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), None);
    assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 501),
        Err(JournalError::Contract(Error::WrongState)));
}

#[test]
fn dispatched_and_unknown_obligations_never_become_ready_through_continuation() {
    for unknown in [false, true] {
        let mut rig = Rig::new();
        rig.submit(REQUEST);
        rig.reviewed(REQUEST);
        let human = rig.human(1001, 30);
        let attempt = match step(&mut rig, Some(&human)).unwrap() {
            FileDriverEvent::Dispatched { attempt, .. } => attempt,
            event => panic!("original dispatch missing: {event:?}"),
        };
        if unknown {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            assert_eq!(host.reconcile(revision, attempt).unwrap(),
                Reconciliation::AwaitingResolution);
        }
        retain(&mut rig, 501);
        let before = rig.driver.supervisor().host().unwrap().inspect();
        let bytes = image(&rig);
        assert_eq!(before.control.ledger.charged, 16);
        assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST).unwrap(), None);
        assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 501),
            Err(JournalError::Contract(Error::WrongState)));
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
        assert_eq!(image(&rig), bytes);
        if unknown {
            assert!(matches!(rig.driver.step_with_evidence(
                || ElapsedTick(1), |_, _| panic!("Unknown acquired evidence"), None,
            ).unwrap(), FileDriverEvent::Reconciled {
                outcome: Reconciliation::AwaitingResolution, ..
            }));
            assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
        } else {
            assert!(matches!(step(&mut rig, None).unwrap(), FileDriverEvent::Published { .. }));
            assert!(matches!(step(&mut rig, None).unwrap(), FileDriverEvent::Reconciled { .. }));
            assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
        }
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    }
}

#[test]
fn numerically_matching_foreign_owner_cannot_supply_a_continuation() {
    let mut original = Rig::new();
    let mut foreign = Rig::new();
    for rig in [&mut original, &mut foreign] {
        rig.submit(REQUEST);
        rig.reviewed(REQUEST);
        retain(rig, 501);
    }
    let original_image = image(&original);
    let foreign_image = image(&foreign);
    {
        let mut left = original.driver.supervisor_mut().host_mut().unwrap();
        let mut right = foreign.driver.supervisor_mut().host_mut().unwrap();
        std::mem::swap(&mut *left, &mut *right);
    }
    assert_eq!(original.driver.pending_narrowing_continuation(REQUEST),
        Err(JournalError::Contract(Error::Binding)));
    assert_eq!(original.driver.continue_after_narrowing(REQUEST, 501),
        Err(JournalError::Contract(Error::Binding)));
    assert_eq!(image(&original), original_image);
    assert_eq!(image(&foreign), foreign_image);
}

#[test]
fn failed_durable_successor_cannot_return_a_cached_continuation_as_live() {
    let mut rig = Rig::new();
    rig.submit(REQUEST);
    rig.reviewed(REQUEST);
    retain(&mut rig, 501);
    let continued = rig.driver.continue_after_narrowing(REQUEST, 501).unwrap();
    let before = rig.driver.supervisor().host().unwrap().inspect();
    let bytes = image(&rig);
    let pending = rig.root.store().join("delivery.pending");
    std::fs::write(&pending, b"occupied staging witness").unwrap();
    {
        let retained = target(&rig);
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let request = FileAuthorityNarrowRequest::new(
            502, before.control.sequence, before.control.ledger.epoch, vec![retained],
        ).unwrap();
        assert!(matches!(host.narrow_authority(before.revision, request),
            Err(JournalError::Io(_))));
        assert!(host.storage_failure().is_some());
    }
    assert_eq!(continued.operation(), 501);
    assert_eq!(rig.driver.pending_narrowing_continuation(REQUEST),
        Err(JournalError::Unavailable));
    assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 501),
        Err(JournalError::Unavailable));
    assert_eq!(rig.driver.continue_after_narrowing(REQUEST, 502),
        Err(JournalError::Unavailable));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect(), before);
    assert_eq!(image(&rig), bytes);
    assert_eq!(std::fs::read(pending).unwrap(), b"occupied staging witness");
}
