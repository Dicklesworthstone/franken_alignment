//! Actual decoder computation drives observation lifetime and prefix identity.
#[path = "support/monitored_source.rs"]
mod support;
use support::*;
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::observation::DecoderAvailability;
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, RefinementBudget};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderModel};
use fa_reference::Error;

#[test]
fn original_tokens_and_all_layer_reports_are_captured_only_after_full_quiet_review() {
    let mut run = quiet(); let source = run.observation(); let clone = source.clone();
    assert_eq!(source.availability(), DecoderAvailability::Empty);
    assert!(matches!(source.capture(), Err(Error::Incomplete)));
    let mut prefix = Vec::new();
    for token in [0, 3, 1, 5] {
        let step = run.advance(prefix.len() as u64, token, compute()).unwrap();
        assert!(matches!(step, MonitoredStep::Released(_)));
        prefix.push(token);
        let captured = clone.capture().unwrap();
        assert_eq!(captured.tokens(), prefix);
        assert_eq!(captured.next_position(), prefix.len() as u64);
        assert_eq!(captured.review(), step.review());
        assert_eq!(captured.review().unreviewed_layers(), 0);
        assert_eq!(captured.review().layers().len(), 2);
        assert_eq!(captured.generation(), 11); assert_eq!(captured.stream(), 7);
        assert_eq!(captured.profile(), run.profile());
        source.validate(&captured).unwrap();
    }
}

#[test]
fn identical_independent_owners_cannot_exchange_evidence_and_new_work_stales_old_prefixes() {
    let mut first = quiet(); let mut other = quiet();
    for token in [1, 2] {
        first.advance(first.position(), token, compute()).unwrap();
        other.advance(other.position(), token, compute()).unwrap();
    }
    let source = first.observation(); let old = source.capture().unwrap();
    assert_eq!(other.observation().capture().unwrap().tokens(), old.tokens());
    assert_eq!(other.observation().validate(&old), Err(Error::Binding));
    first.advance_greedy(2, compute()).unwrap();
    assert_eq!(source.validate(&old), Err(Error::Stale));
    assert_eq!(old.tokens(), &[1, 2]);
    assert_eq!(old.review().position(), 1);
    let current = source.capture().unwrap(); source.validate(&current).unwrap();
    assert_eq!(&current.tokens()[..2], old.tokens());
    assert_eq!(current.next_position(), 3);
}

#[test]
fn preflight_refusal_preserves_a_live_basis_but_drop_revokes_all_observer_clones() {
    let mut run = quiet(); run.advance(0, 1, compute()).unwrap();
    let source = run.observation(); let observer = source.clone(); let captured = source.capture().unwrap();
    let before = (run.decoder_work(), run.monitoring_work());
    assert!(matches!(run.advance(0, 0, compute()), Err(Error::Stale)));
    assert!(matches!(run.advance(1, 99, compute()), Err(Error::InvalidInput)));
    assert!(matches!(run.advance(1, 0, DecoderBudget { scalar_products: 0 }), Err(Error::Limit)));
    assert_eq!((run.decoder_work(), run.monitoring_work()), before);
    source.validate(&captured).unwrap();
    drop(run);
    for source in [source, observer] {
        assert_eq!(source.availability(), DecoderAvailability::Closed);
        assert_eq!(source.validate(&captured), Err(Error::Incomplete));
        assert!(matches!(source.capture(), Err(Error::Incomplete)));
    }
    assert_eq!(captured.tokens(), &[1]);
    assert_eq!(captured.review().outcome(), MonitorOutcome::NoAlarm);
}

#[test]
fn global_budget_hold_invalidates_previously_quiet_evidence_without_refunding_computation() {
    let model = fixture::model(fixture::profile(8));
    let per_frame = fa_reference::action::consequence::activation::HEADER_BYTES + 4 * model.profile().shape().hidden;
    let budget = RefinementBudget { encoded_bytes: 2 * per_frame, probe_coordinates: 8 };
    let mut run = monitored(model, 1_000_000.0, budget);
    run.advance(0, 1, compute()).unwrap();
    let source = run.observation(); let first = source.capture().unwrap();
    let second = run.advance(1, 2, compute()).unwrap();
    assert!(matches!(second, MonitoredStep::Held(_)));
    assert_eq!(second.review().outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(source.availability(), DecoderAvailability::Held);
    assert_eq!(source.validate(&first), Err(Error::Incomplete));
    assert!(matches!(source.capture(), Err(Error::Incomplete)));
    assert_eq!(run.decoder_work().tokens, 2);
    assert_eq!(run.monitoring_work().encoded_bytes, budget.encoded_bytes);
    assert!(matches!(run.advance(2, 0, compute()), Err(Error::WrongState)));
}

#[test]
fn an_alarm_never_exports_a_quiet_prefix_even_when_later_layers_were_not_reviewed() {
    let mut run = monitored(fixture::model(fixture::profile(8)), -1_000_000.0, allowance());
    let source = run.observation();
    let held = run.advance(0, 0, compute()).unwrap();
    assert_eq!(held.review().outcome(), MonitorOutcome::Alarm);
    assert_eq!(held.review().unreviewed_layers(), 1);
    assert_eq!(source.availability(), DecoderAvailability::Held);
    assert!(matches!(source.capture(), Err(Error::Incomplete)));
    assert_eq!(run.position(), 1);
}

#[test]
fn numerical_failure_withdraws_the_last_good_prefix_even_when_the_cache_did_not_advance() {
    let profile = fixture::profile(8); let shape = profile.shape();
    let mut layers = fixture::zero_layers(&profile); layers[0].queries.fill(f32::MAX);
    let mut embeddings = vec![1.0; shape.vocabulary * shape.hidden];
    embeddings[..shape.hidden].fill(0.0);
    let model = DecoderModel::new(profile, embeddings, layers, vec![1.0; shape.hidden],
        vec![0.0; shape.vocabulary * shape.hidden]).unwrap();
    let mut run = monitored(model, 1_000_000.0, allowance());
    run.advance(0, 0, compute()).unwrap();
    let source = run.observation(); let before = source.capture().unwrap();
    assert!(matches!(run.advance(1, 1, compute()), Err(Error::Overflow)));
    assert_eq!(run.position(), 1);
    assert_eq!(run.status(), MonitoringStatus::Failed(Error::Overflow));
    assert_eq!(source.availability(), DecoderAvailability::Failed);
    assert_eq!(source.validate(&before), Err(Error::Incomplete));
    assert!(matches!(source.capture(), Err(Error::Incomplete)));
    assert_eq!(before.tokens(), &[0]);
}
