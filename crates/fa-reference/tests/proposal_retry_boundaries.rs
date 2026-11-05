//! Public boundary regressions for the reference proposal recorder.
//! These exercise proposal identity and accounting, not effect dispatch.

use fa_reference::address_resolver::{Address, Noun};
use fa_reference::event_journal::{
    DomainEventJournal, JournalError, JournalEvent, JournalEventKind,
};
use fa_reference::explain_affordances::{Affordance, DecisionCard, ExplainError, ProposalEngine};
use fa_reference::typestate_cx::{PurposeTag, RootCx};

fn address(tenant: &str, id: &str) -> Address {
    Address::new(tenant, Noun::Evidence, id, Some(1)).unwrap()
}

fn affordance(tenant: &str) -> Affordance {
    let target = address(tenant, "observation");
    Affordance {
        affordance_id: "resolve-observation".to_owned(),
        verb: "resolve",
        target: target.clone(),
        authority_required: PurposeTag::Capture,
        cost_estimate: 4,
        expected_information_gain: 20,
        would_change: vec![target],
        unsatisfied_node_id: "required-observation".to_owned(),
        is_hard_floor: true,
        rehearsable: true,
    }
}

fn propose(
    engine: &mut ProposalEngine,
    journal: &mut DomainEventJournal,
    predecessor: u64,
    key: &str,
    chosen: &Affordance,
    alternatives: &[Affordance],
) -> Result<DecisionCard, ExplainError> {
    engine.fa_propose(
        PurposeTag::Capture,
        predecessor,
        key,
        chosen,
        alternatives,
        journal,
    )
}

fn note(seq: u64, tenant: &str) -> JournalEvent {
    JournalEvent::new(
        seq,
        vec![],
        vec![address(tenant, "observation")],
        JournalEventKind::Hold {
            attempt_id: 1,
            reason: "awaiting observation".to_owned(),
        },
        [0x31; 32],
        0,
        10,
    )
    .unwrap()
}

#[test]
fn exact_retry_after_later_events_keeps_original_card_and_single_charge() {
    let mut journal = DomainEventJournal::new_production("org", "control");
    let mut engine = ProposalEngine::new(4, 1);
    let chosen = affordance("org");
    let original = propose(&mut engine, &mut journal, 0, "request", &chosen, &[]).unwrap();
    journal.append(note(2, "org")).unwrap();

    let retry = propose(&mut engine, &mut journal, 0, "request", &chosen, &[]).unwrap();
    assert_eq!(retry, original);
    assert_eq!(retry.new_control_seq, 1);
    assert_eq!(journal.last_control_seq(), 2);
    assert_eq!(journal.len(), 2);
    assert_eq!(engine.remaining_budget(), 0);
}

#[test]
fn reused_key_cannot_change_target_cost_verb_alternatives_or_predecessor() {
    let mut journal = DomainEventJournal::new_production("org", "control");
    let mut engine = ProposalEngine::new(100, 20);
    let chosen = affordance("org");
    let mut alternative = chosen.clone();
    alternative.affordance_id = "another-plan".to_owned();
    let alternatives = vec![alternative.clone()];
    let original = propose(
        &mut engine,
        &mut journal,
        0,
        "request",
        &chosen,
        &alternatives,
    )
    .unwrap();

    let mut changed_target = chosen.clone();
    changed_target.target = address("org", "different-target");
    let mut changed_cost = chosen.clone();
    changed_cost.cost_estimate = 1;
    let mut changed_verb = chosen.clone();
    changed_verb.verb = "get";
    let mut changed_evidence = chosen.clone();
    changed_evidence.would_change = vec![address("org", "different-predicate")];
    let mut changed_alternative = alternative;
    changed_alternative.cost_estimate += 1;
    for (candidate, alternatives, predecessor) in [
        (changed_target, alternatives.clone(), 0),
        (changed_cost, alternatives.clone(), 0),
        (changed_verb, alternatives.clone(), 0),
        (changed_evidence, alternatives.clone(), 0),
        (chosen.clone(), vec![changed_alternative], 0),
        (chosen.clone(), alternatives.clone(), 1),
    ] {
        assert_eq!(
            propose(
                &mut engine,
                &mut journal,
                predecessor,
                "request",
                &candidate,
                &alternatives,
            ),
            Err(ExplainError::IdempotencyConflict {
                idempotency_key: "request".to_owned(),
            }),
        );
        assert_eq!(journal.len(), 1);
        assert_eq!(engine.remaining_budget(), 96);
    }

    assert_eq!(
        propose(
            &mut engine,
            &mut journal,
            0,
            "request",
            &chosen,
            &alternatives,
        )
        .unwrap(),
        original,
    );
}

#[test]
fn cached_reply_rechecks_purpose_and_floor_admission() {
    let mut journal = DomainEventJournal::new_production("org", "control");
    let mut engine = ProposalEngine::new(8, 2);
    let chosen = affordance("org");
    let original = propose(&mut engine, &mut journal, 0, "request", &chosen, &[]).unwrap();

    assert_eq!(
        engine.fa_propose(
            PurposeTag::Experiment,
            0,
            "request",
            &chosen,
            &[],
            &mut journal,
        ),
        Err(ExplainError::AuthorityEscalationRefused {
            required: PurposeTag::Capture,
            actual: PurposeTag::Experiment,
        }),
    );
    let mut forbidden = chosen.clone();
    forbidden.verb = "skip-floor";
    assert!(matches!(
        propose(&mut engine, &mut journal, 0, "request", &forbidden, &[]),
        Err(ExplainError::FloorSkippingForbidden { .. }),
    ));
    let mut changed_purpose = chosen.clone();
    changed_purpose.authority_required = PurposeTag::Judge;
    assert!(matches!(
        engine.fa_propose(
            PurposeTag::Judge,
            0,
            "request",
            &changed_purpose,
            &[],
            &mut journal,
        ),
        Err(ExplainError::IdempotencyConflict { .. }),
    ));
    assert_eq!(journal.len(), 1);
    assert_eq!(engine.remaining_budget(), 4);
    assert_eq!(
        propose(&mut engine, &mut journal, 0, "request", &chosen, &[]).unwrap(),
        original,
    );
}

#[test]
fn equal_domain_names_and_keys_do_not_reuse_another_journals_result() {
    let mut engine = ProposalEngine::new(20, 5);
    let chosen = affordance("org");
    let mut first = DomainEventJournal::new_production("org", "control");
    let mut independent = DomainEventJournal::new_production("org", "control");
    propose(&mut engine, &mut first, 0, "request", &chosen, &[]).unwrap();
    propose(&mut engine, &mut independent, 0, "request", &chosen, &[]).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(independent.len(), 1);
    assert_eq!(engine.remaining_budget(), 12);

    let mut other_tenant = DomainEventJournal::new_production("other", "control");
    let other = affordance("other");
    let card = propose(&mut engine, &mut other_tenant, 0, "request", &other, &[]).unwrap();
    assert_eq!(card.chosen.target.tenant, "other");
    assert_eq!(other_tenant.len(), 1);
    assert_eq!(engine.remaining_budget(), 8);
}

#[test]
fn cloned_journals_record_their_own_proposals_and_never_borrow_cached_results() {
    let mut journal = DomainEventJournal::new_production("org", "control");
    let mut before = journal.clone();
    let mut engine = ProposalEngine::new(12, 3);
    let chosen = affordance("org");
    let original = propose(&mut engine, &mut journal, 0, "request", &chosen, &[]).unwrap();
    let mut after = journal.clone();

    // Equal display scope and an equal request are independent work on a clone.
    propose(&mut engine, &mut before, 0, "request", &chosen, &[]).unwrap();
    assert_eq!(before.len(), 1);
    assert_eq!(engine.remaining_budget(), 4);
    assert_eq!(
        propose(&mut engine, &mut after, 0, "request", &chosen, &[]),
        Err(ExplainError::PredecessorSeqMismatch {
            expected: 0,
            actual: 1,
        }),
    );
    assert_eq!(after.len(), 1);
    assert_eq!(engine.remaining_budget(), 4);

    // A new operation at that clone's own current cut is still usable.
    let clone_card = propose(&mut engine, &mut after, 1, "request", &chosen, &[]).unwrap();
    assert_eq!(clone_card.new_control_seq, 2);
    assert_eq!(after.len(), 2);
    assert_eq!(engine.remaining_budget(), 0);
    assert_eq!(
        propose(&mut engine, &mut journal, 0, "request", &chosen, &[]).unwrap(),
        original,
    );
    assert_eq!(journal.len(), 1);
}

#[test]
fn rejected_journal_append_does_not_charge_or_consume_the_only_admission_slot() {
    let mut journal = DomainEventJournal::new_production("org", "control");
    let mut engine = ProposalEngine::new(4, 1);
    let foreign_target = affordance("other");
    assert!(matches!(
        propose(
            &mut engine,
            &mut journal,
            0,
            "request",
            &foreign_target,
            &[],
        ),
        Err(ExplainError::JournalFailure(_)),
    ));
    assert_eq!(engine.remaining_budget(), 4);
    assert!(journal.is_empty());

    let chosen = affordance("org");
    propose(&mut engine, &mut journal, 0, "request", &chosen, &[]).unwrap();
    assert_eq!(engine.remaining_budget(), 0);
    assert_eq!(journal.len(), 1);
}

#[test]
fn exhausted_sequence_and_empty_key_are_nonmutating() {
    let mut journal = DomainEventJournal::new_production("org", "control");
    let mut engine = ProposalEngine::new(8, 2);
    let chosen = affordance("org");
    assert_eq!(
        propose(&mut engine, &mut journal, 0, "", &chosen, &[]),
        Err(ExplainError::InvalidIdempotencyKey),
    );
    assert!(journal.is_empty());

    journal.append(note(u64::MAX, "org")).unwrap();
    assert_eq!(
        propose(&mut engine, &mut journal, u64::MAX, "request", &chosen, &[],),
        Err(ExplainError::JournalFailure(
            JournalError::ControlSequenceExhausted,
        )),
    );
    assert_eq!(journal.last_control_seq(), u64::MAX);
    assert_eq!(journal.len(), 1);
    assert_eq!(engine.remaining_budget(), 8);
}

#[test]
fn experiment_proposal_uses_branch_journal_without_touching_production() {
    let root = RootCx::new_nucleus(1, 1, 1);
    let experiment = root.mint_experiment(7).unwrap();
    let mut production = DomainEventJournal::new_production("org", "control");
    let mut branch = DomainEventJournal::new_branch(&experiment, "org", "control", 7);
    let mut engine = ProposalEngine::new(8, 2);
    let mut chosen = affordance("org");
    chosen.authority_required = PurposeTag::Experiment;
    assert_eq!(
        engine.fa_propose(
            PurposeTag::Experiment,
            0,
            "request",
            &chosen,
            &[],
            &mut production,
        ),
        Err(ExplainError::ProductionExperimentForbidden),
    );
    assert!(production.is_empty());
    assert_eq!(engine.remaining_budget(), 8);
    engine
        .fa_propose(
            PurposeTag::Experiment,
            0,
            "request",
            &chosen,
            &[],
            &mut branch,
        )
        .unwrap();
    assert_eq!(branch.len(), 1);
    assert!(production.is_empty());
    assert_eq!(engine.remaining_budget(), 4);
}
