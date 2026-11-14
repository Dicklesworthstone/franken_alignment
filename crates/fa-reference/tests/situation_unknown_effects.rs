//! Direct expectations for unresolved effects in the imported reference journal.
//!
//! These cases exercise the public fold and Tower facade, not the fold's shared
//! batch-oracle helper. Imported outcome labels are declarations; these tests do
//! not authenticate receipts, qualify a live adapter, or grant effect authority.
#![forbid(unsafe_code)]

use fa_reference::address_resolver::{Address, CallerScope, Noun, ResolverPlane};
use fa_reference::event_journal::{DomainEventJournal, JournalEventKind};
use fa_reference::observation_algebra::{
    Basis, BasisIdentity, ClaimClass, Knowledge, ProofStates, ScopedProof, UnknownReason,
};
use fa_reference::situation_rehearsal::{
    SituationReport, SituationSectionKind, fold_situation_report, project_situation_for_scope,
};
use fa_reference::tower::{
    ResponseStatus, TowerAccess, TowerBuilder, TowerLimits, TowerRequest, TowerValue, Verb,
};

const TENANT: &str = "liability-tenant";

fn attempt(id: &str, generation: u64) -> Address {
    Address::new(TENANT, Noun::Attempt, id, Some(generation)).unwrap()
}

fn journal() -> DomainEventJournal {
    DomainEventJournal::new_production(TENANT, "liability-journal")
}

fn emit(journal: &mut DomainEventJournal, addresses: Vec<Address>, kind: JournalEventKind) {
    journal
        .emit_transition(Vec::new(), addresses, kind, [7; 32], 1, 0)
        .unwrap();
}

fn dispatch(journal: &mut DomainEventJournal, id: u64, subject: &Address) {
    emit(
        journal,
        vec![subject.clone()],
        JournalEventKind::Dispatch {
            attempt_id: id,
            adapter_id: 19,
        },
    );
}

fn outcome(journal: &mut DomainEventJournal, id: u64, subject: &Address, class: &str) {
    emit(
        journal,
        vec![subject.clone()],
        JournalEventKind::Outcome {
            attempt_id: id,
            outcome_class: class.to_owned(),
        },
    );
}

fn unresolved(report: &SituationReport, expected: &[(&Address, u64)]) {
    let actual: Vec<_> = report
        .items
        .iter()
        .filter_map(|entry| match entry {
            Knowledge::Known { value, basis }
                if value.section == SituationSectionKind::UnknownEffects =>
            {
                Some((value, basis))
            }
            _ => None,
        })
        .collect();
    assert_eq!(actual.len(), expected.len(), "{report:?}");
    for (address, produced_at) in expected {
        let (item, basis) = actual
            .iter()
            .find(|(item, _)| &item.address == *address)
            .unwrap_or_else(|| panic!("missing unresolved {address:?}: {report:?}"));
        assert_eq!(basis.control_seq(), *produced_at);
        assert!(item.is_hard_floor);
    }
}

fn incomplete(report: &SituationReport) {
    assert!(
        report
            .items
            .contains(&Knowledge::unknown(UnknownReason::Incomplete)),
        "{report:?}"
    );
}

#[test]
fn dispatch_and_unknown_outcomes_remain_visible_until_confirmed_settlement() {
    for class in ["unknown", "outcome_unknown"] {
        let subject = attempt("message-7", 3);
        let mut history = journal();
        dispatch(&mut history, 7, &subject);
        let pending = fold_situation_report(&history, 1);
        unresolved(&pending, &[(&subject, 1)]);
        assert_eq!(pending.items.len(), 1);
        assert!(matches!(
            &pending.items[0],
            Knowledge::Known { value, .. }
                if value.summary.contains("outcome has not been observed")
        ));

        outcome(&mut history, 7, &subject, class);
        let unknown = fold_situation_report(&history, 2);
        unresolved(&unknown, &[(&subject, 2)]);
        assert_eq!(unknown.items.len(), 1);
        assert!(matches!(
            &unknown.items[0],
            Knowledge::Known { value, .. }
                if value.summary.contains("reconciliation is required")
        ));

        let finding = Address::new(TENANT, Noun::Finding, "unrelated", Some(1)).unwrap();
        emit(
            &mut history,
            vec![finding],
            JournalEventKind::Finding {
                finding_id: 11,
                detector_id: "unrelated-detector".to_owned(),
                severity: 1,
            },
        );
        let later = fold_situation_report(&history, 3);
        unresolved(&later, &[(&subject, 2)]);
        assert_eq!(later.exact_at_control_seq, 3);
        assert_eq!(later.items[0], unknown.items[0]);

        outcome(&mut history, 7, &subject, "confirmed");
        let settled = fold_situation_report(&history, 4);
        unresolved(&settled, &[]);
        assert_eq!(settled.items.len(), 1);
        assert_eq!(fold_situation_report(&history, 2), unknown);
        assert_eq!(history.last_control_seq(), 4);
    }
}

#[test]
fn confirmed_and_no_effect_settlements_cannot_retire_another_attempt() {
    for class in ["confirmed", "verified_success", "confirmed_not_executed"] {
        let first = attempt("first", 1);
        let second = attempt("second", 1);
        let mut history = journal();
        dispatch(&mut history, 7, &first);
        dispatch(&mut history, 8, &second);
        outcome(&mut history, 8, &second, class);
        let report = fold_situation_report(&history, 3);
        unresolved(&report, &[(&first, 1)]);
        assert_eq!(report.items.len(), 1);

        outcome(&mut history, 7, &first, class);
        assert!(fold_situation_report(&history, 4).items.is_empty());
    }
}

#[test]
fn complete_attempt_identity_is_required_even_with_matching_numeric_id() {
    let subject = attempt("subject", 3);
    for wrong in [
        attempt("unrelated", 3),
        attempt("subject", 4),
        subject.without_generation(),
        Address::new(TENANT, Noun::Evidence, "not-an-attempt", Some(3)).unwrap(),
    ] {
        for class in ["confirmed", "confirmed_not_executed", "refused", "unknown"] {
            let mut history = journal();
            dispatch(&mut history, 7, &subject);
            outcome(&mut history, 7, &subject, "unknown");
            let original = fold_situation_report(&history, 2).items[0].clone();
            outcome(&mut history, 7, &wrong, class);
            let report = fold_situation_report(&history, 3);
            unresolved(&report, &[(&subject, 2)]);
            assert!(report.items.contains(&original));
            incomplete(&report);
        }
    }

    // An existing exact subject may follow unrelated typed addresses.
    let mut history = journal();
    dispatch(&mut history, 7, &subject);
    emit(
        &mut history,
        vec![attempt("related", 4), subject.clone()],
        JournalEventKind::Outcome {
            attempt_id: 7,
            outcome_class: "confirmed".to_owned(),
        },
    );
    assert!(fold_situation_report(&history, 2).items.is_empty());

    // The same address with a different numeric attempt cannot settle attempt 7.
    let mut history = journal();
    dispatch(&mut history, 7, &subject);
    outcome(&mut history, 8, &subject, "confirmed");
    unresolved(&fold_situation_report(&history, 2), &[(&subject, 1)]);
}

#[test]
fn predispatch_retirement_is_distinct_from_external_effect_settlement() {
    for class in ["refused", "cancelled", "resolved"] {
        let subject = attempt("subject", 1);
        let mut held = journal();
        emit(
            &mut held,
            vec![subject.clone()],
            JournalEventKind::Hold {
                attempt_id: 7,
                reason: "awaiting-review".to_owned(),
            },
        );
        outcome(&mut held, 7, &subject, class);
        assert!(fold_situation_report(&held, 2).items.is_empty());

        let mut sent = journal();
        dispatch(&mut sent, 7, &subject);
        let original = fold_situation_report(&sent, 1).items[0].clone();
        outcome(&mut sent, 7, &subject, class);
        let report = fold_situation_report(&sent, 2);
        unresolved(&report, &[(&subject, 1)]);
        assert!(report.items.contains(&original));
        incomplete(&report);

        // A refusal for another held attempt does not change the known liability.
        let other = attempt("other", 1);
        emit(
            &mut sent,
            vec![other.clone()],
            JournalEventKind::Hold {
                attempt_id: 8,
                reason: "another-review".to_owned(),
            },
        );
        outcome(&mut sent, 8, &other, class);
        unresolved(&fold_situation_report(&sent, 4), &[(&subject, 1)]);
    }
}

#[test]
fn irrecoverably_unknown_is_a_permanent_visible_liability() {
    for class in [
        "confirmed",
        "confirmed_not_executed",
        "verified_success",
        "resolved",
        "refused",
        "unknown",
        "irrecoverably_unknown",
    ] {
        let subject = attempt("abandoned", 1);
        let mut history = journal();
        dispatch(&mut history, 7, &subject);
        outcome(&mut history, 7, &subject, "irrecoverably_unknown");
        let original = fold_situation_report(&history, 2).items[0].clone();
        assert!(matches!(
            &original,
            Knowledge::Known { value, .. }
                if value.summary.contains("irrecoverably unknown")
        ));
        outcome(&mut history, 7, &subject, class);
        let report = fold_situation_report(&history, 3);
        unresolved(&report, &[(&subject, 2)]);
        assert!(report.items.contains(&original));
        if class == "irrecoverably_unknown" {
            assert_eq!(report.items.len(), 1);
        } else {
            incomplete(&report);
        }
        emit(
            &mut history,
            vec![subject.clone()],
            JournalEventKind::Permit {
                attempt_id: 7,
                cost_units: 1,
            },
        );
        dispatch(&mut history, 7, &subject);
        let still_unknown = fold_situation_report(&history, 5);
        unresolved(&still_unknown, &[(&subject, 2)]);
        assert!(still_unknown.items.contains(&original));
    }
}

#[test]
fn unrecognized_outcomes_cannot_be_mistaken_for_success() {
    for class in ["", "CONFIRMED", "confirmed ", "success", "failed", "new-class"] {
        let subject = attempt("subject", 1);
        let mut history = journal();
        dispatch(&mut history, 7, &subject);
        outcome(&mut history, 7, &subject, class);
        let report = fold_situation_report(&history, 2);
        unresolved(&report, &[(&subject, 2)]);
        incomplete(&report);
        assert!(report.items.iter().any(|entry| matches!(
            entry,
            Knowledge::Known { value, .. }
                if value.summary.contains("unrecognized outcome")
        )));

        // An exact declared settlement can resolve the liability; the earlier
        // unrecognized event remains explicit incomplete interpretation.
        outcome(&mut history, 7, &subject, "confirmed_not_executed");
        let settled = fold_situation_report(&history, 3);
        unresolved(&settled, &[]);
        incomplete(&settled);
    }
}

#[test]
fn missing_or_ambiguous_effect_subjects_never_create_known_history() {
    let evidence = Address::new(TENANT, Noun::Evidence, "related", Some(1)).unwrap();
    let first = attempt("first", 1);
    let second = attempt("second", 1);
    for addresses in [
        vec![evidence.clone()],
        vec![first.clone(), second],
    ] {
        for kind in [
            JournalEventKind::Dispatch {
                attempt_id: 7,
                adapter_id: 19,
            },
            JournalEventKind::Outcome {
                attempt_id: 7,
                outcome_class: "unknown".to_owned(),
            },
        ] {
            let mut history = journal();
            emit(&mut history, addresses.clone(), kind);
            let report = fold_situation_report(&history, 1);
            assert_eq!(
                report.items,
                vec![Knowledge::unknown(UnknownReason::Incomplete)]
            );
        }
    }

    let mut permitted = journal();
    emit(
        &mut permitted,
        vec![evidence, first.clone()],
        JournalEventKind::Outcome {
            attempt_id: 7,
            outcome_class: "unknown".to_owned(),
        },
    );
    let report = fold_situation_report(&permitted, 1);
    unresolved(&report, &[(&first, 1)]);
    assert_eq!(report.items.len(), 1);
}

fn snapshot_basis(control_seq: u64) -> Basis {
    let proof = ScopedProof::Declared {
        scope_id: 1,
        generation: 42,
    };
    Basis::new(
        BasisIdentity {
            subject_id: 1,
            question_id: 2,
            profile_id: 3,
            semantic_generation: 42,
            policy_epoch: 4,
            model_epoch: 5,
            tokenizer_epoch: 6,
            codec_epoch: 7,
            control_seq,
        },
        ClaimClass::BoundedModel,
        ProofStates {
            authentic_origin: proof,
            complete_for_contract: proof,
            semantically_valid: proof,
            available_for_replay: proof,
        },
    )
}

#[test]
fn tower_situation_discloses_the_original_unknown_without_mutating_history() {
    let subject = attempt("subject", 3);
    let mut history = journal();
    dispatch(&mut history, 7, &subject);
    outcome(&mut history, 7, &subject, "unknown");
    emit(
        &mut history,
        vec![subject.clone()],
        JournalEventKind::Permit {
            attempt_id: 7,
            cost_units: 9,
        },
    );
    let direct = fold_situation_report(&history, 3);
    unresolved(&direct, &[(&subject, 2)]);
    let mut partial = CallerScope::full_tenant_access(TENANT);
    partial.allowed_planes.remove(&ResolverPlane::ControlLedger);
    let private = project_situation_for_scope(&direct, &partial);
    assert!(!private.items.iter().any(|item| matches!(item, Knowledge::Known { .. })));
    assert_eq!(private.exact_at_control_seq, 0);

    let mut tower = TowerBuilder::new(history, snapshot_basis(3), TowerLimits::default(), 8)
        .unwrap()
        .build();
    let access = TowerAccess::new(
        CallerScope::full_tenant_access(TENANT),
        true,
        Vec::new(),
        0,
    );
    let request = TowerRequest::new(
        "inspect-original-liability",
        tower.domain_address().clone(),
        Verb::Situation,
    );
    let response = tower.dispatch(&access, request);
    assert_eq!(response.envelope.status, ResponseStatus::Ok);
    let Knowledge::Known {
        value: TowerValue::Situation(report),
        basis,
    } = &response.result else {
        panic!("expected original situation: {response:?}");
    };
    assert_eq!(*basis, snapshot_basis(3));
    unresolved(report, &[(&subject, 2)]);
    let Knowledge::Known { basis, .. } = &report.items[0] else {
        panic!("expected addressed liability");
    };
    assert_eq!(basis.semantic_generation(), 42);
    assert_eq!(basis.policy_epoch(), 4);
    assert_eq!(basis.control_seq(), 2);
    let encoded = response.to_json().unwrap();
    assert!(encoded.contains("\"unknown_effects\""));
    assert!(encoded.contains("reconciliation is required"));
    assert_eq!(tower.journal_control_seq(), 3);
    assert_eq!(tower.remaining_read_units(), 7);
}
