mod support {
    pub mod native_andon;
}
use support::native_andon::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, AndonLedger};
use fa_reference::Error;
use std::collections::BTreeMap;

#[test]
fn accepted_native_label_emits_original_case_and_charges_without_authority_change() {
    let (mut host, evaluator, endpoint) = setup(0);
    let binding = host.enable_andon(accounting()).unwrap();
    apply(&mut host, 1, 10, [Verdict::Hold, Verdict::Allow]);
    assert_eq!(host.andon_receipt(101), Err(Error::Missing));
    let truth = label(&host, &evaluator, 1, GroundTruth::Benign);
    host.observe_time(ElapsedTick(30)).unwrap();
    let before = host.inspect();
    assert_eq!(host.record_evaluation(truth), Ok(true));
    assert_eq!(host.inspect(), before);
    let receipt = host.andon_receipt(101).unwrap();
    assert_eq!(receipt.binding(), &binding);
    assert_eq!(receipt.budget(), budget());
    assert_eq!(receipt.case(), host.evaluation_ticket(101).unwrap().case());
    assert_eq!(receipt.assessment().truth, GroundTruth::Benign);
    assert_eq!(receipt.confirmation().helper_generations, BTreeMap::from([("a".into(), 11)]));
    assert_eq!((receipt.confirmation().occurred_at, receipt.confirmation().confirmed_at), (10, 30));
    let window = &host.andon_ledger().unwrap().windows()[&1];
    assert_eq!((window.false_stops, window.total_cost), (1, 7));
    assert_eq!(window.incident.as_ref().unwrap().opened_by_round, 101);
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn foreign_evaluator_cannot_charge_or_record_but_corrected_label_succeeds() {
    let (mut host, evaluator, _) = setup(0);
    let (mut foreign, foreign_evaluator, _) = setup(0);
    host.enable_andon(accounting()).unwrap();
    apply(&mut host, 1, 10, [Verdict::Hold, Verdict::Allow]);
    apply(&mut foreign, 1, 10, [Verdict::Hold, Verdict::Allow]);
    let before = host.credibility_report().unwrap();
    let accounting = host.andon_ledger().unwrap().clone();
    assert_eq!(host.record_evaluation(label(&foreign, &foreign_evaluator, 1, GroundTruth::Benign)), Err(Error::Binding));
    assert_eq!(host.credibility_report().unwrap(), before);
    assert_eq!(host.andon_ledger().unwrap(), &accounting);
    assert!(host.evaluation_history(101).unwrap().is_empty());
    host.record_evaluation(label(&host, &evaluator, 1, GroundTruth::Benign)).unwrap();
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
}

#[test]
fn native_censor_resolution_and_exact_retry_retain_first_confirmation_clock() {
    let (mut host, evaluator, _) = setup(0);
    host.enable_andon(accounting()).unwrap();
    apply(&mut host, 1, 10, [Verdict::Hold, Verdict::Allow]);
    host.record_evaluation(label(&host, &evaluator, 1, GroundTruth::Censored)).unwrap();
    assert_eq!(host.andon_receipt(101), Err(Error::Missing));
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    let truth = label(&host, &evaluator, 1, GroundTruth::Benign);
    host.observe_time(ElapsedTick(30)).unwrap();
    host.record_evaluation(truth.clone()).unwrap();
    let receipt = host.andon_receipt(101).unwrap().clone();
    let report = host.credibility_report().unwrap();
    host.observe_time(ElapsedTick(90)).unwrap();
    assert_eq!(host.record_evaluation(truth), Ok(false));
    assert_eq!(host.andon_receipt(101).unwrap(), &receipt);
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(host.evaluation_history(101).unwrap().len(), 2);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
}

#[test]
fn uncovered_occurrence_refuses_both_updates_and_adding_window_allows_same_label() {
    let (mut host, evaluator, _) = setup(0);
    let mut seed = AndonLedger::new(identity()).unwrap();
    seed.register_window(AndonBudget { window: 2, start: 20, end: 40, ..budget() }).unwrap();
    host.enable_andon(seed).unwrap();
    apply(&mut host, 1, 10, [Verdict::Hold, Verdict::Allow]);
    host.observe_time(ElapsedTick(30)).unwrap();
    let truth = label(&host, &evaluator, 1, GroundTruth::Benign);
    let before = host.credibility_report().unwrap();
    let control = host.inspect();
    assert_eq!(host.record_evaluation(truth.clone()), Err(Error::Missing));
    assert_eq!(host.credibility_report().unwrap(), before);
    assert_eq!(host.inspect(), control);
    assert!(host.evaluation_history(101).unwrap().is_empty());
    assert_eq!(host.andon_ledger().unwrap().windows()[&2].false_stops, 0);
    host.register_andon_window(budget()).unwrap();
    assert_eq!(host.record_evaluation(truth), Ok(true));
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&2].false_stops, 0);
}

#[test]
fn ready_minority_abstaining_and_confirmed_violations_do_not_charge_native_andon() {
    for (maximum, verdicts, truth) in [
        (0, [Verdict::Allow, Verdict::Allow], GroundTruth::Benign),
        (10, [Verdict::Hold, Verdict::Allow], GroundTruth::Benign),
        (0, [Verdict::Abstain, Verdict::Allow], GroundTruth::Benign),
        (0, [Verdict::Hold, Verdict::Allow], GroundTruth::Violation),
    ] {
        let (mut host, evaluator, endpoint) = setup(maximum);
        host.enable_andon(accounting()).unwrap();
        apply(&mut host, 1, 10, verdicts);
        let control = host.inspect();
        host.record_evaluation(label(&host, &evaluator, 1, truth)).unwrap();
        assert_eq!(host.inspect(), control);
        assert_eq!(host.andon_receipt(101), Err(Error::Missing));
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn unconsumed_native_review_never_becomes_an_evaluation_or_charge() {
    let (mut host, _, _) = setup(0);
    host.enable_andon(accounting()).unwrap();
    let (review, _, _) = prepare(&mut host, 1, 10, [Verdict::Hold, Verdict::Allow]);
    drop(review);
    assert_eq!(host.evaluation_ticket(101).unwrap_err(), Error::Missing);
    assert_eq!(host.andon_receipt(101), Err(Error::Missing));
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    assert_eq!(host.credibility_report().unwrap().pending_cases, 1);
}

#[test]
fn recovered_seed_retains_old_accounting_without_claiming_to_recreate_receipts() {
    let (mut old, evaluator, _) = setup(0);
    old.enable_andon(accounting()).unwrap();
    apply(&mut old, 1, 10, [Verdict::Hold, Verdict::Allow]);
    old.record_evaluation(label(&old, &evaluator, 1, GroundTruth::Benign)).unwrap();
    let old_receipt = old.andon_receipt(101).unwrap().clone();
    let seed = old.andon_ledger().unwrap().clone();
    let (mut next, evaluator, _) = setup(0);
    next.enable_andon(seed).unwrap();
    next.register_andon_window(AndonBudget { window: 2, start: 20, end: 40, ..budget() }).unwrap();
    apply(&mut next, 2, 30, [Verdict::Allow, Verdict::Hold]);
    next.record_evaluation(label(&next, &evaluator, 2, GroundTruth::Benign)).unwrap();
    assert_eq!(next.andon_ledger().unwrap().confirmation(101), Some(old_receipt.confirmation()));
    assert_eq!(next.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(next.andon_ledger().unwrap().windows()[&2].false_stops, 1);
    assert_eq!(next.andon_receipt(101), Err(Error::Missing));
    assert_eq!(next.andon_receipt(102).unwrap().confirmation().helper_generations, BTreeMap::from([("b".into(), 22)]));
}

#[test]
fn bootstrap_and_window_changes_cannot_reconfigure_prior_accounting() {
    let (mut host, evaluator, _) = setup(0);
    assert_eq!(host.enable_andon(AndonLedger::new(identity()).unwrap()), Err(Error::Incomplete));
    let binding = host.enable_andon(accounting()).unwrap();
    assert_eq!(host.enable_andon(accounting()), Err(Error::Duplicate));
    apply(&mut host, 1, 10, [Verdict::Hold, Verdict::Allow]);
    host.record_evaluation(label(&host, &evaluator, 1, GroundTruth::Benign)).unwrap();
    let before = host.andon_ledger().unwrap().clone();
    assert_eq!(host.register_andon_window(budget()), Ok(false));
    assert_eq!(host.register_andon_window(AndonBudget { false_stops_allowed: 1000, ..budget() }), Err(Error::Binding));
    assert_eq!(host.register_andon_window(AndonBudget { window: 2, ..budget() }), Err(Error::Binding));
    assert_eq!(host.enable_andon(accounting()), Err(Error::WrongState));
    assert_eq!(host.andon_ledger().unwrap(), &before);
    assert_eq!(host.andon_binding().unwrap(), &binding);
}

#[test]
fn native_labels_keep_legacy_behavior_when_andon_is_not_enabled() {
    let (mut host, evaluator, _) = setup(0);
    apply(&mut host, 1, 10, [Verdict::Hold, Verdict::Allow]);
    let truth = label(&host, &evaluator, 1, GroundTruth::Benign);
    assert_eq!(host.record_evaluation(truth.clone()), Ok(true));
    assert_eq!(host.record_evaluation(truth), Ok(false));
    assert_eq!(host.credibility_report().unwrap().lifetime_false_stops, 1);
    assert_eq!(host.andon_ledger(), Err(Error::Incomplete));
    assert_eq!(host.enable_andon(accounting()), Err(Error::WrongState));
}
