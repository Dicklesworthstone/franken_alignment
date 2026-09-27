//! Numerical evidence must drive refinement; quiet never replaces model judgment.
#[path = "support/sidecar_receiving.rs"]
pub mod shared;
#[path = "support/sidecar_native_model.rs"]
pub mod native_model;
use shared::*;
use native_model::helper;
use fa_reference::Error;
use fa_reference::action::consequence::activation::probe::learned::LearnedProbeWork;
use fa_reference::action::consequence::oversight::{helper_workers::{HelperPort, wire::{decode_request, encode_request}},
    sidecar::receiver::{SidecarReceiver, SidecarReceiveBudget, native::{
        SidecarNativeEvaluator, SidecarProbeQuery, SidecarEvaluationError, SidecarEvaluationStatus,
        SidecarDecisionBasis,
    }}};
use fa_reference::round::Verdict;

pub fn evaluator(setup: &Setup, port: &HelperPort, queries: Vec<SidecarProbeQuery>, budget: LearnedProbeWork,
    spelling: &[u8], alarm: bool) -> SidecarNativeEvaluator
{
    let receiver = SidecarReceiver::new(port, &setup.packet, setup.source.clone(), SidecarReceiveBudget::default()).unwrap();
    let native = helper(receiver.input_profile().clone(), spelling, alarm);
    SidecarNativeEvaluator::new(native, receiver, queries, budget).unwrap()
}
pub fn query(axis: usize, threshold: f32) -> SidecarProbeQuery { SidecarProbeQuery { row: row(), probe: probe(axis, threshold) } }
pub fn budget() -> LearnedProbeWork { LearnedProbeWork { coordinates: 32, reconstruction_products: 32 } }

#[test]
fn actual_uncertainty_refines_before_original_native_judgment_and_publication() {
    let mut setup = Setup::new(b"approve?"); let (mut round, ports) = setup.workers(10);
    let mut coarse = evaluator(&setup, &ports["reviewer"], vec![query(0, 0.5)], budget(), b"allow", false);
    let coarse_verdict = coarse.evaluate(&input(&ports["reviewer"])).unwrap();
    assert_eq!(coarse_verdict, Verdict::Abstain);
    assert_eq!(coarse.progress().basis, Some(SidecarDecisionBasis::NumericalUncertainty));
    assert!(!coarse.progress().native_started); assert_eq!(coarse.progress().native.work.position, 0);
    let review = finish(&mut round, &ports["reviewer"], coarse_verdict); setup.refine(&review);
    let (mut round, ports) = setup.workers(11); let input = input(&ports["reviewer"]);
    let mut exact = evaluator(&setup, &ports["reviewer"], vec![query(0, 0.5)], budget(), b"allow", false);
    let exact_verdict = exact.evaluate(&input).unwrap();
    assert_eq!(exact_verdict, Verdict::Allow);
    assert_eq!(exact.progress().basis, Some(SidecarDecisionBasis::NativeModel));
    let mut independently_run = helper(input.actual_input().input_profile().clone(), b"allow", false);
    assert_eq!(independently_run.evaluate(&input), Ok(Verdict::Allow));
    assert_eq!(exact.progress().native, independently_run.progress());
    assert_eq!(exact.progress().native.work.sampled_draws, 2);
    let review = finish(&mut round, &ports["reviewer"], exact_verdict);
    setup.owner.apply_review(review, Some(setup.packet.input()), &snapshot()).unwrap();
    let key = setup.owner.authorize(1, Some(setup.packet.input()), &snapshot()).unwrap();
    let envelope = setup.owner.dispatch(&key, &setup.action, Some(setup.packet.input()), &snapshot()).unwrap();
    setup.owner.accept_receipt(setup.endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(setup.endpoint.payload(), b"visible"); assert_eq!(setup.endpoint.execution_count(), 1);
    assert!(setup.owner.dispatch(&key, &setup.action, Some(setup.packet.input()), &snapshot()).is_err());
}

#[test]
fn quiet_probes_do_not_override_a_genuine_native_deny() {
    let mut setup = Setup::new(b"approve!"); let (mut round, ports) = setup.workers(10);
    let review = finish(&mut round, &ports["reviewer"], Verdict::Abstain); setup.refine(&review);
    let (_round, ports) = setup.workers(11); let input = input(&ports["reviewer"]);
    let mut worker = evaluator(&setup, &ports["reviewer"], vec![query(0, 0.5)], budget(), b"allow", false);
    assert_eq!(worker.evaluate(&input), Ok(Verdict::Deny));
    assert_eq!(worker.progress().basis, Some(SidecarDecisionBasis::NativeModel));
    assert_eq!(worker.progress().native.work.sampled_draws, 2);
    assert_eq!(setup.endpoint.execution_count(), 0);
}

#[test]
fn alarm_dominates_undecided_and_exact_equality_abstains_without_native_work() {
    let mut setup = refined(); let (_round, ports) = setup.workers(11);
    for (queries, verdict, basis) in [
        (vec![query(0, 0.0)], Verdict::Abstain, SidecarDecisionBasis::NumericalUncertainty),
        (vec![query(0, 0.0), query(1, 0.5)], Verdict::Hold, SidecarDecisionBasis::NumericalAlarm),
    ] {
        let count = queries.len();
        let mut worker = evaluator(&setup, &ports["reviewer"], queries, budget(), b"allow", false);
        assert_eq!(worker.evaluate(&input(&ports["reviewer"])), Ok(verdict));
        assert_eq!(worker.progress().basis, Some(basis));
        assert_eq!(worker.progress().completed_probes, count);
        assert!(!worker.progress().native_started); assert_eq!(worker.progress().native.work.position, 0);
    }
}

#[test]
fn cooperative_probe_and_native_steps_match_whole_evaluation_with_free_stale_calls() {
    let mut setup = Setup::new(b"approve?"); let (_round, ports) = setup.workers(10); let port = &ports["reviewer"];
    let queries = vec![query(0, 10.0), query(1, 10.0)];
    let mut worker = evaluator(&setup, port, queries.clone(), budget(), b"allow", false);
    let mut whole = evaluator(&setup, port, queries, budget(), b"allow", false);
    let input = input(port); worker.begin(&input).unwrap();
    for completed in 1..=2 {
        let before = worker.progress();
        assert_eq!(worker.advance(before.revision + 1), Err(SidecarEvaluationError::Contract(Error::Stale)));
        assert_eq!(worker.progress(), before);
        let next = worker.advance(before.revision).unwrap();
        assert_eq!(next.completed_probes, completed); assert_eq!(next.native.work.position, 0);
    }
    for _ in 0..4098 {
        let before = worker.progress();
        if matches!(before.status, SidecarEvaluationStatus::Judged(_)) { break; }
        let next = worker.advance(before.revision).unwrap();
        assert_eq!(next.native.work.position, before.native.work.position + 1);
    }
    assert_eq!(worker.progress().status, SidecarEvaluationStatus::Judged(Verdict::Allow));
    assert_eq!(whole.evaluate(&input), Ok(Verdict::Allow));
    assert_eq!(worker.progress(), whole.progress());
    let final_state = worker.progress();
    assert_eq!(worker.advance(final_state.revision).unwrap(), final_state);
    assert!(worker.begin(&input).is_err()); assert_eq!(worker.progress(), final_state);
}

#[test]
fn whole_probe_inventory_is_admitted_before_any_score_or_native_inference() {
    let mut setup = Setup::new(b"approve?"); let (_round, ports) = setup.workers(10); let port = &ports["reviewer"];
    let queries = vec![query(0, 10.0), query(1, 10.0)];
    let exact = LearnedProbeWork { coordinates: 4, reconstruction_products: 2 };
    let mut valid = evaluator(&setup, port, queries.clone(), exact, b"allow", false);
    assert_eq!(valid.evaluate(&input(port)), Ok(Verdict::Allow));
    assert_eq!(valid.progress().completed_probe_work, exact);
    let mut short = evaluator(&setup, port, queries.clone(), LearnedProbeWork { reconstruction_products: 1, ..exact }, b"allow", false);
    assert_eq!(short.begin(&input(port)), Err(SidecarEvaluationError::Contract(Error::Limit)));
    assert_eq!(short.progress().completed_probes, 0); assert_eq!(short.progress().native.work.position, 0);
    assert!(short.begin(&input(port)).is_err());
    for (queries, cap, error) in [(queries, LearnedProbeWork { coordinates: 3, ..exact }, Error::Limit),
        (vec![], exact, Error::InvalidInput), (vec![query(0, 10.0), query(0, 10.0)], exact, Error::Duplicate)] {
        let receiver = SidecarReceiver::new(port, &setup.packet, setup.source.clone(), SidecarReceiveBudget::default()).unwrap();
        let native = helper(receiver.input_profile().clone(), b"allow", false);
        assert_eq!(SidecarNativeEvaluator::new(native, receiver, queries, cap).err(), Some(error));
    }
}

#[test]
fn wrong_whole_input_cannot_spend_native_work_or_be_replaced_after_failure() {
    let mut setup = refined(); let (_round, ports) = setup.workers(11); let port = &ports["reviewer"];
    let mut bytes = encode_request(port).unwrap(); bytes[17] ^= 1;
    let mut worker = evaluator(&setup, port, vec![query(0, 0.5)], budget(), b"allow", false);
    assert_eq!(worker.begin(&decode_request(&bytes).unwrap()), Err(SidecarEvaluationError::Contract(Error::Binding)));
    assert_eq!(worker.progress().native.work.position, 0); assert!(worker.progress().receive.is_none());
    assert!(worker.begin(&input(port)).is_err());
}

#[test]
fn native_monitor_failure_and_invalid_generated_answer_never_become_votes() {
    let mut setup = refined(); let (_round, ports) = setup.workers(11); let port = &ports["reviewer"];
    for (spelling, alarm) in [(b"allow".as_slice(), true), (b"allow\n".as_slice(), false)] {
        let mut worker = evaluator(&setup, port, vec![query(0, 0.5)], budget(), spelling, alarm);
        assert!(matches!(worker.evaluate(&input(port)), Err(SidecarEvaluationError::Native(_))));
        assert!(matches!(worker.progress().status, SidecarEvaluationStatus::Failed(_)));
        assert!(worker.progress().basis.is_none());
        let before = worker.progress(); assert!(before.native.work.sampled_draws > 0);
        assert!(worker.advance(before.revision).is_err()); assert_eq!(worker.progress(), before);
    }
}

#[test]
fn cancellation_preserves_received_and_partial_work_without_a_native_or_probe_fallback() {
    let mut setup = Setup::new(b"approve?"); let (_round, ports) = setup.workers(10); let port = &ports["reviewer"];
    for operations in 0..4 {
        let mut worker = evaluator(&setup, port, vec![query(0, 10.0), query(1, 10.0)], budget(), b"allow", false);
        worker.begin(&input(port)).unwrap();
        for _ in 0..operations { worker.advance(worker.progress().revision).unwrap(); }
        let before = worker.progress(); assert!(worker.cancel()); assert!(!worker.cancel());
        let after = worker.progress();
        assert_eq!(after.status, SidecarEvaluationStatus::Cancelled);
        assert_eq!(after.receive, before.receive); assert_eq!(after.reserved_probes, before.reserved_probes);
        assert_eq!(after.completed_probe_work, before.completed_probe_work);
        assert_eq!(after.native.work, before.native.work);
        assert!(worker.advance(after.revision).is_err()); assert!(worker.begin(&input(port)).is_err());
    }
}

#[cfg(unix)]
#[path = "sidecar_native_helper/transport.rs"]
mod transport;
