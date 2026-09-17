//! Real durable admissions and the original full-input/two-key publication path.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/file_decoder.rs"] mod numerical;
use ordinary::*;
use fa_reference::action::{ActionState, ElapsedTick, Purpose};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::investigation::*;
use fa_reference::action::consequence::experiment::{Intervention, InterventionScope, NextRequirement, EmpiricalStatus};
use fa_reference::action::consequence::experiment::proposal::search::{ProposalSearchBudget, ProposalSearchStatus};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate, Truth};
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::rc::Rc;

fn guards() -> FileGuardSet { FileGuardSet { stream: None, decoder: None, decoder_stop: None,
    source: None, identity: None, campaigns: None, credential: None } }
fn setup(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, roles) = FileOversight::create_guarded(root.store(), profile(), &guards(), None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); (host, roles.human)
}
fn scope() -> InterventionScope { InterventionScope::new(71, true, false, false, &[]).unwrap() }
fn expected(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let state = host.inspect();
    FileRecoveryRequirements { guards, effective_policy: host.current_policy().unwrap().clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: state.revision, control_sequence: state.control.sequence,
            authority_epoch: state.control.ledger.epoch } }
}
fn complete(mut search: FileProposalRepairSearch) -> FileProposalRepairReport {
    while search.status() == ProposalSearchStatus::Running { search.advance().unwrap(); }
    search.finish().unwrap()
}
fn deny(host: &mut FileOversight, id: u64) {
    host.propose(host.revision(), id, spec(host, &vec![b'x'; 129]), snapshot()).unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&id], ActionState::Denied);
}

#[test]
fn hard_denial_repair_requires_a_new_proposal_and_original_congress_and_human_key() {
    let root = Directory::new(); let (mut host, human) = setup(&root); deny(&mut host, 1);
    let before = host.inspect(); let bytes = std::fs::read(root.store().join("delivery.bin")).unwrap();
    assert!(host.review_replay(101).is_err()); // No invented congress for an exact denial.
    let investigation = host.investigate_proposal(1, scope()).unwrap();
    assert_eq!(investigation.purpose(), Purpose::Experiment);
    assert_eq!(investigation.origin().proposal().state, ActionState::Denied);
    assert_eq!(investigation.origin().admission_snapshot(), &before);
    let report = complete(investigation.begin_repair_search(&[Intervention::Payload(b"fixed".to_vec())],
        ProposalSearchBudget { cases: 2, retained_edit_bytes: 5 }).unwrap());
    assert_eq!(report.origin(), investigation.origin());
    assert_eq!(report.search().cases()[0].outcome.as_ref().unwrap().counterfactual(), Truth::Violated);
    assert_eq!(report.search().cases()[1].outcome.as_ref().unwrap().next_requirement(), NextRequirement::FreshIndependentReview);
    assert_eq!(host.inspect(), before); assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
    assert!(host.propose(host.revision(), 1, spec(&host, b"fixed"), snapshot()).is_err());
    let keys = ready(&mut host, &human, 2, b"fixed"); dispatch(&mut host, &keys);
    let outcome = host.publish_checked(host.revision(), 2, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome;
    assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 2).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Denied);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(report.origin().journal_snapshot().executions, 0);
}

#[test]
fn later_policy_and_cancellation_do_not_rewrite_the_original_admission() {
    let root = Directory::new(); let (mut host, _) = setup(&root);
    host.propose(host.revision(), 1, spec(&host, b"long payload"), snapshot()).unwrap();
    let initial = host.inspect(); let c = initial.control.clone();
    let next = Policy::new(2, vec![Predicate::PayloadAtMost(3)]).unwrap();
    let change = PolicyUpdate::new(11, c.sequence, c.ledger.epoch, next).unwrap();
    host.replace_policy(host.revision(), &change).unwrap();
    let investigation = host.investigate_proposal(1, scope()).unwrap();
    assert_eq!(investigation.origin().proposal().policy.generation(), 1);
    assert_eq!(investigation.origin().admission_snapshot(), &initial);
    assert_eq!(investigation.origin().journal_snapshot().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(investigation.run(&[]).unwrap().result().counterfactual(), Truth::Satisfied);
    assert_eq!(host.current_policy().unwrap().generation(), 2);
    let pinned = expected(&host, guards());
    let read = FileOversight::read_proposal_investigation(root.store(), &profile(), &pinned, 1, scope()).unwrap();
    assert_eq!(read.origin(), investigation.origin());
    let mut wrong = pinned; wrong.effective_policy = profile().delivery.policy;
    assert!(matches!(FileOversight::read_proposal_investigation(root.store(), &profile(), &wrong, 1, scope()),
        Err(JournalError::Contract(Error::Binding))));
}

#[test]
fn actor_request_ids_and_retries_do_not_replace_the_first_admission_or_invent_refused_actions() {
    let root = Directory::new(); let (mut host, _) = setup(&root);
    let request = spec(&host, &vec![b'x'; 129]);
    let status = host.submit_request(host.revision(), 700, request.clone(), snapshot()).unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else { panic!("expected exact denial"); };
    let first = host.revision();
    let mut missing = snapshot(); missing.complete = false;
    host.submit_request(0, 700, request, missing.clone()).unwrap();
    assert_eq!(host.revision(), first);
    let refused = host.submit_request(host.revision(), 701, spec(&host, b"unobserved"), missing).unwrap();
    assert!(matches!(refused.disposition, FileRequestDisposition::NotAdmitted(_)));
    let investigation = host.investigate_proposal(attempt, scope()).unwrap();
    assert_eq!(investigation.origin().external_request(), Some(700));
    assert_eq!(investigation.origin().admission_snapshot().revision, first);
    assert_eq!(investigation.origin().proposal().state, ActionState::Denied);
    assert!(investigation.origin().journal_snapshot().revision > first);
    assert!(matches!(host.investigate_proposal(999, scope()), Err(JournalError::Contract(Error::Missing))));
}

#[test]
fn read_only_fault_inspection_preserves_staging_unknown_charges_and_executed_outcomes() {
    for executed in [false, true] {
        let root = Directory::new(); let (mut host, human) = setup(&root);
        let keys = ready(&mut host, &human, 1, b"approved"); dispatch(&mut host, &keys);
        if executed { host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap(); }
        let pinned = expected(&host, guards()); let before = host.inspect();
        let path = root.store().join("delivery.bin"); let bytes = std::fs::read(&path).unwrap();
        let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retain evidence").unwrap();
        assert!(host.observe_time(host.revision(), ElapsedTick(2)).is_err());
        assert!(matches!(host.investigate_proposal(1, scope()), Err(JournalError::Unavailable)));
        let read = FileOversight::read_proposal_investigation(root.store(), &profile(), &pinned, 1, scope()).unwrap();
        assert_eq!(read.origin().journal_snapshot(), &before);
        assert_eq!(read.origin().proposal().state, ActionState::Reviewing);
        assert_eq!(read.origin().journal_snapshot().control.ledger.charged, 16);
        assert_eq!(read.origin().journal_snapshot().executions, u64::from(executed));
        assert!(read.run(&[Intervention::Payload(b"different".to_vec())]).is_ok());
        assert_eq!(std::fs::read(&path).unwrap(), bytes); assert_eq!(std::fs::read(&pending).unwrap(), b"retain evidence");
        drop(host);
        let (mut host, _) = FileOversight::open_guarded(root.store(), profile(), &pinned).unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        if executed {
            assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
            assert_eq!(host.inspect().control.ledger.charged, 16);
        } else {
            assert_eq!(host.seal_unexecuted(host.revision(), 1).unwrap(), Reconciliation::Resolved(
                EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
            assert_eq!(host.inspect().control.ledger.charged, 0);
        }
    }
}

#[test]
fn independent_guard_floor_and_entire_canonical_suffix_are_required_without_cleanup() {
    let root = Directory::new(); let (mut host, _) = setup(&root); deny(&mut host, 1);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let pinned = expected(&host, guards()); let path = root.store().join("delivery.bin");
    let bytes = std::fs::read(&path).unwrap(); let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"keep").unwrap();
    let mut wrong = pinned.clone(); wrong.minimum.journal_revision += 1;
    assert!(matches!(FileOversight::read_proposal_investigation(root.store(), &profile(), &wrong, 1, scope()), Err(JournalError::Contract(Error::Stale))));
    let mut p = profile(); p.human.reviewer_id += 1;
    assert!(FileOversight::read_proposal_investigation(root.store(), &p, &pinned, 1, scope()).is_err());
    let mut corrupt = bytes.clone(); corrupt.push(0); std::fs::write(&path, &corrupt).unwrap();
    assert!(FileOversight::read_proposal_investigation(root.store(), &profile(), &pinned, 1, scope()).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt); assert_eq!(std::fs::read(&pending).unwrap(), b"keep");
    std::fs::write(&path, &bytes).unwrap();
    assert!(FileOversight::read_proposal_investigation(root.store(), &profile(), &pinned, 1, scope()).is_ok());
    assert_eq!(std::fs::read(&path).unwrap(), bytes); assert_eq!(std::fs::read(&pending).unwrap(), b"keep");
}

#[test]
fn numerical_configuration_is_pinned_and_post_admission_numerical_corruption_cannot_be_ignored() {
    let root = Directory::new(); let mut declared = guards(); declared.decoder = Some(numerical::configuration(100.0));
    let (mut host, _) = FileOversight::create_guarded(root.store(), profile(), &declared, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); numerical::forced(&mut host, 0); deny(&mut host, 1);
    numerical::forced(&mut host, 0); // The suffix, not just the selected admission, must validate.
    let pinned = expected(&host, declared); let mut wrong = pinned.clone(); wrong.guards.decoder = None;
    let path = root.store().join("delivery.bin"); let bytes = std::fs::read(&path).unwrap();
    assert!(matches!(FileOversight::read_proposal_investigation(root.store(), &profile(), &wrong, 1, scope()),
        Err(JournalError::Contract(Error::Binding))));
    let mut corrupt = bytes.clone(); *corrupt.last_mut().unwrap() ^= 1; std::fs::write(&path, &corrupt).unwrap();
    assert!(matches!(FileOversight::read_proposal_investigation(root.store(), &profile(), &pinned, 1, scope()),
        Err(JournalError::Contract(Error::Binding))));
    std::fs::write(&path, bytes).unwrap();
    let read = FileOversight::read_proposal_investigation(root.store(), &profile(), &pinned, 1, scope()).unwrap();
    assert_eq!(read.origin().proposal().state, ActionState::Denied);
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 2);
}

#[test]
fn full_input_review_diagnosis_requires_the_original_anchor_and_preserves_application_refusal() {
    for stale in [false, true] {
        let root = Directory::new(); let (mut host, _) = setup(&root);
        let action = host.propose(host.revision(), 1, spec(&host, b"original"), snapshot()).unwrap();
        let original = inputs(&action, b"original context");
        host.record_inputs(host.revision(), 1, 0, original.clone()).unwrap();
        host.begin_review(host.revision(), 1, 101, ROOT, window(&host), snapshot()).unwrap();
        let anchor = host.review_anchor(101).unwrap(); votes(&mut host, 101, Verdict::Allow);
        let current = if stale { inputs(&action, b"replacement context") } else { original.clone() };
        if stale { host.record_inputs(host.revision(), 1, host.input_revision(1).unwrap(), current.clone()).unwrap(); }
        let application = host.finish_review(host.revision(), 101, Some(&current), snapshot()).unwrap();
        assert_eq!(application.is_err(), stale);
        let replay = host.review_replay(101).unwrap(); let before = host.inspect();
        assert_eq!(replay.application().is_err(), stale);
        let e = replay.policy_experiment(&anchor, scope()).unwrap();
        let report = e.run(&[Intervention::Payload(b"changed".to_vec())]).unwrap();
        assert_eq!(report.empirical_status(), EmpiricalStatus::InvalidatedByIntervention);
        assert_eq!(report.next_requirement(), NextRequirement::FreshIndependentReview);
        let mut wrong = anchor; wrong.inputs = Rc::new(inputs(&action, b"not the reviewed input"));
        assert!(matches!(replay.policy_experiment(&wrong, scope()), Err(Error::Binding)));
        assert_eq!(host.inspect(), before); assert_eq!(replay.application().is_err(), stale);
    }
}
