//! Paired cursor against the unchanged eager numerical evaluator.
#[path = "support/investigation_decoder.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::{
    DecoderComparisonBudget, DecoderContinuationComparison,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::cursor::{
    DecoderComparisonCursor, DecoderComparisonStatus,
};
use fa_reference::Error;

fn complete(mut run: DecoderComparisonCursor) -> DecoderContinuationComparison {
    let horizon = run.horizon();
    for i in 0..2 * horizon {
        assert_eq!(run.status(), DecoderComparisonStatus::Running);
        assert!(run.finish().is_err());
        run.advance().unwrap();
        let work = run.work().unwrap();
        assert_eq!(work.entered_tokens, (i + 1) as u64);
        assert_eq!(work.completed.tokens, (i + 1) as u64);
        assert_eq!(work.completed_pairs, (i + 1) / 2);
        assert_eq!(work.retained_logit_values, 2 * (i + 1));
    }
    assert_eq!(run.status(), DecoderComparisonStatus::Complete);
    let before = run.work().unwrap();
    assert_eq!(run.advance().unwrap(), DecoderComparisonStatus::Complete);
    assert_eq!(run.work().unwrap(), before);
    let report = run.finish().unwrap();
    assert_eq!(report.steps().len(), horizon);
    assert_eq!(report.work(), before.planned);
    assert_eq!(before.entered_products, report.work().scalar_products().unwrap());
    report
}
fn same(a: &DecoderContinuationComparison, b: &DecoderContinuationComparison) {
    assert_eq!(a.policy(), b.policy()); assert_eq!(a.work(), b.work());
    assert_eq!(a.retained_logit_values(), b.retained_logit_values());
    assert_eq!(a.first_different_logits(), b.first_different_logits());
    assert_eq!(a.first_different_consumed_token(), b.first_different_consumed_token());
    assert_eq!(a.first_different_next_choice(), b.first_different_next_choice());
    assert_eq!(a.plan().specification(), b.plan().specification());
    assert_eq!(a.steps().len(), b.steps().len());
    for (a, b) in a.steps().iter().zip(b.steps()) {
        assert_eq!((a.position, a.control_token, a.intervention_token, a.control_next_token, a.intervention_next_token),
            (b.position, b.control_token, b.intervention_token, b.control_next_token, b.intervention_next_token));
        for (left, right) in [(&a.control_logits, &b.control_logits), (&a.intervention_logits, &b.intervention_logits)] {
            assert_eq!(left.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), right.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        }
        assert_eq!(a.changed_logit_words, b.changed_logit_words);
        assert_eq!(a.max_abs_logit_delta.to_bits(), b.max_abs_logit_delta.to_bits());
        assert_eq!(a.l2_logit_delta.to_bits(), b.l2_logit_delta.to_bits());
    }
}

#[test]
fn cursor_and_eager_oracle_match_every_word_for_forcing_and_feedback() {
    for value in [-10.0, -0.0, 0.0, 0.25, 10.0] {
        let p = plan(value);
        for horizon in 1..=3 {
            for first in [0, 1] {
                let tokens = vec![first; horizon];
                let a = p.compare_forced(&tokens, comparison_budget()).unwrap();
                let b = complete(p.begin_forced_comparison(&tokens, comparison_budget()).unwrap());
                same(&a, &b);
                let a = p.compare_greedy(first, horizon, comparison_budget()).unwrap();
                let b = complete(p.begin_greedy_comparison(first, horizon, comparison_budget()).unwrap());
                same(&a, &b);
            }
        }
    }
}

#[test]
fn actual_value_edit_changes_feedback_while_the_noop_control_is_exact() {
    let changed = complete(plan(10.0).begin_greedy_comparison(0, 3, comparison_budget()).unwrap());
    assert_eq!(changed.first_different_logits(), Some(1));
    assert_eq!(changed.first_different_next_choice(), Some(2));
    assert_eq!(changed.first_different_consumed_token(), Some(2));
    let unchanged = complete(plan(0.0).begin_greedy_comparison(0, 3, comparison_budget()).unwrap());
    assert_eq!(unchanged.first_different_logits(), None);
    assert_eq!(unchanged.first_different_consumed_token(), None);
    assert_eq!(unchanged.plan().effective_edits(), 0);
}

#[test]
fn cancellation_preserves_unpaired_and_completed_work_but_cannot_finish_or_resume() {
    for advances in 0..6 {
        let mut run = plan(10.0).begin_greedy_comparison(0, 3, comparison_budget()).unwrap();
        for _ in 0..advances { run.advance().unwrap(); }
        let before = run.work().unwrap();
        run.cancel().unwrap(); run.cancel().unwrap();
        assert_eq!(run.status(), DecoderComparisonStatus::Cancelled);
        assert_eq!(run.advance(), Err(Error::WrongState));
        assert!(run.finish().is_err());
        assert_eq!(run.work().unwrap(), before);
        assert_eq!(run.completed_pairs().len(), advances / 2);
    }
}

#[test]
fn intervention_overflow_keeps_the_completed_control_and_refuses_all_reentry() {
    let p = plan(f32::MAX);
    assert!(p.compare_forced(&[0], comparison_budget()).is_err());
    let mut run = p.begin_forced_comparison(&[0], comparison_budget()).unwrap();
    run.advance().unwrap();
    assert_eq!(run.work().unwrap().completed.tokens, 1);
    assert_eq!(run.advance(), Err(Error::Overflow));
    assert_eq!(run.status(), DecoderComparisonStatus::Failed(Error::Overflow));
    let before = run.work().unwrap();
    assert_eq!(before.entered_tokens, 2); assert_eq!(before.completed.tokens, 1);
    assert_eq!(before.completed_pairs, 0);
    assert!(before.entered_products > before.completed.scalar_products().unwrap());
    assert!(run.finish().is_err()); assert_eq!(run.advance(), Err(Error::WrongState));
    assert_eq!(run.cancel(), Err(Error::WrongState));
    assert_eq!(run.work().unwrap(), before);
}

#[test]
fn entire_horizon_and_all_original_tokens_are_checked_before_either_arm_exists() {
    let p = plan(10.0);
    let valid = comparison_budget();
    for tokens in [vec![], vec![0, 2], vec![0; 4]] {
        assert!(p.begin_forced_comparison(&tokens, valid).is_err());
        assert!(p.compare_forced(&tokens, valid).is_err());
    }
    for budget in [DecoderComparisonBudget { scalar_products: 0, ..valid },
        DecoderComparisonBudget { retained_logit_values: 3, ..valid }] {
        assert!(p.begin_forced_comparison(&[0], budget).is_err());
        assert!(p.compare_forced(&[0], budget).is_err());
    }
    let exact = p.compare_forced(&[0, 1], valid).unwrap();
    let budget = DecoderComparisonBudget { scalar_products: exact.work().scalar_products().unwrap(),
        retained_logit_values: exact.retained_logit_values() };
    same(&exact, &complete(p.begin_forced_comparison(&[0, 1], budget).unwrap()));
}
