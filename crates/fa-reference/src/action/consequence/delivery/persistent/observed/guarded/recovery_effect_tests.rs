//! Public two-key publication paths through the new guarded recovery consumer.
//! Direct congress ballots are deliberate synthetic controls, not detector evidence.
use super::*;
use crate::action::{ActionSpec, ActionState, FrozenAction, VERSION};
use crate::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use crate::action::consequence::delivery::persistent::{FilePermit, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{
    FileHumanRequest, publication::PublicationBasis,
};
use crate::action::consequence::oversight::{CommitteeInput, ReviewWindow, action_frame};
use crate::action::consequence::oversight::human::HumanDisposition;
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
use crate::round::{Verdict, commitment};
use crate::Snapshot;

fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }
}

fn reviewed(host: &mut FileOversight, id: u64)
    -> (FrozenAction, CommitteeInput, FilePermit, FileHumanRequest)
{
    let action = host.propose(host.revision(), id, ActionSpec {
        version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16,
    }, snapshot()).unwrap();
    let contracts = profile().committee;
    let helper = &contracts.members()["reviewer"];
    let mut bytes = action_frame(&action);
    let boundary = bytes.len(); bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let view = EvidenceViewManifest::new(actual, AuthorizationProjection { projection_id: 7,
        policy_epoch: action.spec().policy_epoch, projected_originals: Vec::new() }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &contracts,
        BTreeMap::from([("reviewer".to_owned(), view)])).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    let round = 100 + id;
    host.begin_review(host.revision(), id, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30),
    }, snapshot()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    let offer = host.request_human_approval(host.revision(), 1000 + id, id,
        &inputs, ElapsedTick(40)).unwrap();
    (action, inputs, automatic, offer)
}

#[test]
fn original_two_key_publication_cuts_recover_without_old_keys_or_duplicate_effects() {
    for cut in 0..4 {
        for quantum in [1, 3] {
            let root = Directory::new();
            let (mut host, old_roles) = FileOversight::create_guarded(
                root.store(), profile(), &no_extra_guards(), None).unwrap();
            host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            let (action, inputs, automatic, offer) = reviewed(&mut host, 1);
            let revision = host.revision();
            let human = old_roles.human.approve(&mut host, revision, &offer).unwrap();
            // Retain an independent prefix before dispatch and its possible outcome.
            let anchor = host.history_anchor().unwrap();
            if cut >= 1 {
                host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
            }
            if cut >= 2 {
                let published = host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap();
                assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
            }
            if cut >= 3 { host.reconcile(host.revision(), 1).unwrap(); }
            let expected = requirements(&host, no_extra_guards());
            let mut expected_events = host.events.clone();
            expected_events.push(Event::Core(BaseEvent::Fence));
            let oracle = Machine::replay(&host.profile, &expected_events).unwrap().snapshot(expected_events.len());
            drop(host); let before = files(&root);
            let mut run = FileOversight::begin_open_guarded_anchored(
                root.store(), profile(), &expected, &anchor).unwrap();
            ready(&mut run, quantum); assert_eq!(files(&root), before);
            let (mut recovered, fresh_roles) = run.finish().unwrap();
            assert_eq!(recovered.inspect(), oracle); assert!(!recovered.clock_ready());
            let executions = u64::from(cut >= 2);
            assert_eq!(recovered.inspect().executions, executions);
            assert_eq!(recovered.inspect().control.ledger.charged, if cut == 0 { 0 } else { 16 });
            assert_eq!(recovered.human_status(1001).unwrap().disposition,
                if cut == 0 { HumanDisposition::Revoked } else { HumanDisposition::Consumed });
            let files_before_retry = files(&root); let revision = recovered.revision();
            assert_eq!(recovered.dispatch(revision, &automatic, &human, &action, &inputs, snapshot()).err(),
                Some(Error::Binding.into()));
            assert_eq!(files(&root), files_before_retry); assert_eq!(recovered.revision(), revision);
            recovered.observe_time(recovered.revision(), ElapsedTick(3)).unwrap();
            match cut {
                0 => {
                    assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Cancelled);
                    assert_eq!(recovered.inspect().control.ledger.available, 100);
                    // Recovery is useful, not refusal-only: a NEW original review
                    // and both new keys can publish under the current epoch.
                    let (new_action, new_inputs, new_automatic, new_offer) = reviewed(&mut recovered, 2);
                    let revision = recovered.revision(); let before = files(&root);
                    assert_eq!(old_roles.human.approve(&mut recovered, revision, &new_offer).err(),
                        Some(Error::Binding.into()));
                    assert_eq!(files(&root), before);
                    let new_human = fresh_roles.human.approve(&mut recovered, revision, &new_offer).unwrap();
                    recovered.dispatch(recovered.revision(), &new_automatic, &new_human,
                        &new_action, &new_inputs, snapshot()).unwrap();
                    assert_eq!(recovered.publish_checked(recovered.revision(), 2, Some(&new_inputs),
                        snapshot(), ElapsedTick(4)).unwrap().outcome,
                        EndpointOutcome::Executed { resulting_version: 2 });
                    assert_eq!(recovered.reconcile(recovered.revision(), 2).unwrap(),
                        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
                    assert_eq!(recovered.inspect().executions, 1);
                    assert_eq!(recovered.inspect().control.ledger.charged, 16);
                }
                1 => {
                    assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Unknown);
                    assert!(recovered.cancel(recovered.revision(), 1).is_err());
                    assert_eq!(recovered.reconcile(recovered.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
                    assert_eq!(recovered.inspect().control.ledger.charged, 16);
                    assert_eq!(recovered.inspect().control.ledger.available, 84);
                    assert_eq!(recovered.seal_unexecuted(recovered.revision(), 1).unwrap(),
                        Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
                    assert_eq!(recovered.inspect().control.ledger.available, 100);
                    assert_eq!(recovered.inspect().executions, 0);
                }
                2 => {
                    assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Unknown);
                    assert_eq!(recovered.reconcile(recovered.revision(), 1).unwrap(),
                        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
                    assert_eq!(recovered.inspect().control.ledger.charged, 16);
                    assert_eq!(recovered.inspect().executions, 1);
                }
                _ => {
                    assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Confirmed);
                    assert_eq!(recovered.inspect().control.ledger.available, 84);
                }
            }
            let historical = FileOversight::read_publication(root.store(), &profile()).unwrap();
            assert_eq!(historical, recovered.inspect());
        }
    }
}

#[test]
fn lost_publication_acknowledgements_keep_actual_visibility_and_liabilities_through_recovery() {
    for barrier in BARRIERS {
        let root = Directory::new();
        let (mut host, roles) = FileOversight::create_guarded(
            root.store(), profile(), &no_extra_guards(), None).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let (action, inputs, automatic, offer) = reviewed(&mut host, 1);
        let revision = host.revision(); let human = roles.human.approve(&mut host, revision, &offer).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        let anchor = host.history_anchor().unwrap(); let expected = requirements(&host, no_extra_guards());
        let acknowledged = host.inspect();
        host.store.fail_once(barrier);
        let error = host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(2)).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("original publication barrier must fail"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.inspect(), acknowledged); assert!(!host.clock_ready());
        let visible = barrier == JournalIo::DirectorySync;
        let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
        assert_eq!(disk.executions, u64::from(visible));
        assert_eq!(disk.control.ledger.charged, 16);
        drop(host); let files_before = files(&root);
        let mut run = FileOversight::begin_open_guarded_anchored(
            root.store(), profile(), &expected, &anchor).unwrap();
        ready(&mut run, 1); assert_eq!(files(&root), files_before);
        let (mut recovered, _) = run.finish().unwrap();
        assert_eq!(recovered.inspect().executions, u64::from(visible));
        assert_eq!(recovered.inspect().control.ledger.charged, 16);
        assert_eq!(recovered.inspect().control.ledger.stages[&1], ActionState::Unknown);
        let result = recovered.reconcile_attempt_at(recovered.revision(), 1, ElapsedTick(3)).unwrap();
        assert_eq!(result, if visible {
            Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })
        } else { Reconciliation::AwaitingResolution });
        assert_eq!(recovered.inspect().payload.as_slice(), if visible { b"visible".as_slice() } else { b"initial".as_slice() });
        assert_eq!(recovered.inspect().executions, u64::from(visible));
        assert_eq!(recovered.inspect().control.ledger.charged, 16);
        let before = files(&root); let revision = recovered.revision();
        assert_eq!(recovered.dispatch(revision, &automatic, &human, &action, &inputs, snapshot()).err(),
            Some(Error::Binding.into()));
        assert_eq!(recovered.revision(), revision); assert_eq!(files(&root), before);
        if !visible {
            recovered.seal_unexecuted(recovered.revision(), 1).unwrap();
            assert_eq!(recovered.inspect().control.ledger.available, 100);
        }
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), recovered.inspect());
    }
}

#[test]
fn recovered_owner_still_needs_current_full_evidence_at_the_actual_publication_gate() {
    for missing in [false, true] {
        let root = Directory::new();
        let (host, _) = FileOversight::create_guarded(
            root.store(), profile(), &no_extra_guards(), None).unwrap();
        let expected = requirements(&host, no_extra_guards()); let anchor = host.history_anchor().unwrap(); drop(host);
        let mut run = FileOversight::begin_open_guarded_anchored(
            root.store(), profile(), &expected, &anchor).unwrap();
        ready(&mut run, 1); let (mut recovered, roles) = run.finish().unwrap();
        recovered.observe_time(recovered.revision(), ElapsedTick(1)).unwrap();
        let (action, inputs, automatic, offer) = reviewed(&mut recovered, 1);
        let revision = recovered.revision(); let human = roles.human.approve(&mut recovered, revision, &offer).unwrap();
        recovered.dispatch(recovered.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
        let outcome = recovered.publish_checked(recovered.revision(), 1,
            if missing { None } else { Some(&inputs) }, snapshot(), ElapsedTick(2)).unwrap();
        let expected_outcome = if missing {
            assert_eq!(outcome.basis, PublicationBasis::Rejected(Error::Incomplete));
            EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }
        } else {
            assert_eq!(outcome.basis, PublicationBasis::Revalidated);
            EndpointOutcome::Executed { resulting_version: 2 }
        };
        assert_eq!(outcome.outcome, expected_outcome);
        // Final-check nonexecution is not an immediate refund. Only the
        // original receipt reconciliation can settle the charged liability.
        assert_eq!(recovered.inspect().control.ledger.charged, 16);
        assert_eq!(recovered.inspect().executions, u64::from(!missing));
        assert_eq!(recovered.reconcile(recovered.revision(), 1).unwrap(), Reconciliation::Resolved(expected_outcome));
        assert_eq!(recovered.inspect().control.ledger.available, if missing { 100 } else { 84 });
        // Better evidence cannot reroll a recorded denial into execution, nor
        // duplicate an already executed publication. The endpoint outcome wins.
        let again = recovered.publish_checked(recovered.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(3)).unwrap();
        assert_eq!(again.basis, PublicationBasis::PreviouslyResolved);
        assert_eq!(again.outcome, expected_outcome);
        assert_eq!(recovered.inspect().executions, u64::from(!missing));
        assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), recovered.inspect());
    }
}
