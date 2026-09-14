//! Automatic learned-KV refinement against the original exact-source oracle.
#[path = "support/learned_kv.rs"] mod fixture;
use fa_reference::action::consequence::activation::{SourceFrame, ProgressiveFrame};
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome,
    learned::{LearnedMonitorBudget, LearnedMonitorWork, LearnedRefinementMonitor}};
use fa_reference::action::consequence::activation::probe::{LinearProbe, ProbeOutcome};
use fa_reference::action::consequence::activation::probe::learned::{CheckedKvBudget, CheckedLearnedKv,
    KvGroup, KvRow, ResidualRetention};
use fa_reference::action::consequence::activation::tensor::kv::experiment::KvSide;
use fa_reference::action::consequence::activation::tensor::kv::model::{ModelKvImage,
    learned::{LearnedKvCodec, LearnedKvPolicy, FitBudget, CompressionBudget}};
use fa_reference::Error;
use std::collections::{BTreeMap, BTreeSet};

fn checked(training: ModelKvImage, source: &ModelKvImage, retention: ResidualRetention) -> CheckedLearnedKv {
    let codec = LearnedKvCodec::fit(LearnedKvPolicy::new(1, 1, 1, 8).unwrap(),
        &BTreeMap::from([(1, training)]), FitBudget::default()).unwrap();
    let (image, _) = codec.evaluate_held_out(2, source, CompressionBudget::default()).unwrap();
    CheckedLearnedKv::new(image, source, retention, CheckedKvBudget::default()).unwrap()
}
fn row() -> KvRow { KvRow { layer: 1, side: KvSide::Value, position: 0 } }
fn group(head: usize) -> KvGroup { KvGroup { row: row(), head } }
fn probe(source: &CheckedLearnedKv, id: u64, weights: &[f32], threshold: f32) -> LinearProbe {
    LinearProbe::new(id, 1, source.row_shape(row()).unwrap().0.profile, weights, 0.0, threshold).unwrap()
}
fn rare(retention: ResidualRetention) -> CheckedLearnedKv {
    let source = fixture::image(2, 1, 3, &[vec![1.0, 2.0, 0.25]]);
    checked(fixture::line(1, 3, 5), &source, retention)
}
fn alarm_monitor(source: &CheckedLearnedKv, budget: LearnedMonitorBudget) -> LearnedRefinementMonitor {
    LearnedRefinementMonitor::new(vec![probe(source, 1, &[0.0, 0.0, 1.0], 0.125)], budget).unwrap()
}

#[test]
fn rare_alarm_automatically_buys_one_exact_group_and_reuses_the_already_quiet_probe() {
    let source = rare(ResidualRetention::All);
    let original = source.encode().unwrap();
    let monitor = LearnedRefinementMonitor::new(vec![probe(&source, 1, &[0.0, 0.0, 1.0], 0.125),
        probe(&source, 2, &[0.0; 3], 1.0)], LearnedMonitorBudget::default()).unwrap();
    let report = monitor.analyze(&source, row()).unwrap();
    assert_eq!(report.outcome(), MonitorOutcome::Alarm);
    assert_eq!(report.steps().len(), 2);
    assert_eq!(report.steps()[0].observations.len(), 2);
    assert_eq!(report.steps()[0].observations[0].outcome(), ProbeOutcome::NeedsRefinement);
    assert_eq!(report.steps()[0].observations[1].outcome(), ProbeOutcome::CertifiedQuiet);
    assert_eq!(report.steps()[1].observations.len(), 1);
    assert_eq!(report.steps()[1].observations[0].probe().id, 1);
    assert_eq!(report.steps()[1].refinement.unwrap().group, group(0));
    assert_eq!(report.steps()[0].observations[0].view().revision(), 0);
    assert_eq!(report.view().revision(), 1);
    assert_eq!(report.work(), LearnedMonitorWork { encoded_bytes: source.report().base_encoded_bytes + source.residual_bytes(group(0)).unwrap().len(),
        probe_coordinates: 9, reconstruction_products: 4, materialized_values: 3, refinements: 1 });
    assert!(!report.view().is_refined(KvGroup { row: KvRow { side: KvSide::Key, ..row() }, head: 0 }).unwrap());
    assert_eq!(source.encode().unwrap(), original);
}

#[test]
fn coarse_quiet_and_threshold_equality_do_not_buy_unneeded_escape_blocks() {
    let source = fixture::image(2, 1, 3, &[vec![0.0; 3]]);
    let evidence = checked(fixture::line(1, 3, 5), &source, ResidualRetention::None);
    for (threshold, expected) in [(1.0, MonitorOutcome::NoAlarm), (0.0, MonitorOutcome::AtThreshold)] {
        let monitor = LearnedRefinementMonitor::new(vec![probe(&evidence, 1, &[1.0, 0.0, 0.0], threshold)],
            LearnedMonitorBudget { materialized_values: 0, refinements: 0, ..LearnedMonitorBudget::default() }).unwrap();
        let report = monitor.analyze(&evidence, row()).unwrap();
        assert_eq!(report.outcome(), expected); assert_eq!(report.steps().len(), 1);
        assert_eq!(report.work().refinements, 0); assert_eq!(report.work().materialized_values, 0);
        assert_eq!(report.work().encoded_bytes, evidence.report().base_encoded_bytes);
    }
}

#[test]
fn absent_relevant_residual_is_unresolved_not_quiet_or_an_implicitly_regenerated_block() {
    let evidence = rare(ResidualRetention::None);
    let report = alarm_monitor(&evidence, LearnedMonitorBudget::default()).analyze(&evidence, row()).unwrap();
    assert_eq!(report.outcome(), MonitorOutcome::Unresolved);
    assert_eq!(report.unavailable_groups(), &[group(0)]);
    assert_eq!(report.steps().len(), 1); assert_eq!(report.view().revision(), 0);
    assert_eq!(evidence.residual_bytes(group(0)), Err(Error::Missing));
    let complete = rare(ResidualRetention::All);
    assert_eq!(alarm_monitor(&complete, LearnedMonitorBudget::default()).analyze(&complete, row()).unwrap().outcome(), MonitorOutcome::Alarm);
}

#[test]
fn missing_earlier_head_does_not_block_an_available_head_that_can_resolve_the_probe() {
    let training = fixture::image(1, 2, 2, &[vec![0.0; 4], vec![0.0; 4]]);
    let source = fixture::image(2, 2, 2, &[vec![0.0, 0.25, 0.0, 0.25]]);
    let evidence = checked(training, &source, ResidualRetention::Groups(BTreeSet::from([group(1)])));
    let detector = probe(&evidence, 1, &[0.0, 1e-10, 0.0, 1.0], 0.125);
    assert_eq!(detector.learned_dependencies(&evidence.view(), row()).unwrap().len(), 2);
    assert_eq!(detector.evaluate_learned(&evidence.view(), row()).unwrap().outcome(), ProbeOutcome::NeedsRefinement);
    let report = LearnedRefinementMonitor::new(vec![detector], LearnedMonitorBudget::default()).unwrap()
        .analyze(&evidence, row()).unwrap();
    assert_eq!(report.outcome(), MonitorOutcome::Alarm);
    assert_eq!(report.steps()[1].refinement.unwrap().group, group(1));
    assert_eq!(report.work().materialized_values, 2); assert_eq!(report.work().refinements, 1);
    assert!(!report.view().is_refined(group(0)).unwrap());
    assert_eq!(evidence.residual_bytes(group(0)), Err(Error::Missing));
}

#[test]
fn every_exact_budget_boundary_is_preflighted_before_materializing_or_scoring_the_next_step() {
    let evidence = rare(ResidualRetention::All);
    let complete = alarm_monitor(&evidence, LearnedMonitorBudget::default()).analyze(&evidence, row()).unwrap();
    let work = complete.work();
    let exact = LearnedMonitorBudget { encoded_bytes: work.encoded_bytes, probe_coordinates: work.probe_coordinates,
        reconstruction_products: work.reconstruction_products, materialized_values: work.materialized_values,
        refinements: work.refinements };
    assert_eq!(alarm_monitor(&evidence, exact).analyze(&evidence, row()).unwrap().outcome(), MonitorOutcome::Alarm);
    for budget in [LearnedMonitorBudget { encoded_bytes: exact.encoded_bytes - 1, ..exact },
        LearnedMonitorBudget { probe_coordinates: exact.probe_coordinates - 1, ..exact },
        LearnedMonitorBudget { reconstruction_products: exact.reconstruction_products - 1, ..exact },
        LearnedMonitorBudget { materialized_values: exact.materialized_values - 1, ..exact },
        LearnedMonitorBudget { refinements: 0, ..exact }] {
        let monitor = alarm_monitor(&evidence, budget);
        let report = monitor.analyze_with_budget(&evidence, row(), LearnedMonitorBudget::default()).unwrap();
        assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted);
        assert_eq!(report.steps().len(), 1); assert_eq!(report.work().refinements, 0);
        assert_eq!(report.view().revision(), 0);
        assert_eq!(report.steps()[0].observations[0].outcome(), ProbeOutcome::NeedsRefinement);
    }
    let budget = LearnedMonitorBudget { encoded_bytes: evidence.report().base_encoded_bytes - 1, ..exact };
    let report = alarm_monitor(&evidence, budget).analyze(&evidence, row()).unwrap();
    assert_eq!(report.outcome(), MonitorOutcome::BudgetExhausted); assert!(report.steps().is_empty());
    assert_eq!(report.work(), LearnedMonitorWork::default());
}

#[test]
fn exact_cancellation_oracle_keeps_a_subnormal_squared_signal_that_float_scoring_would_lose() {
    let values = [f32::MAX, f32::from_bits(1), -f32::MAX];
    let source = fixture::image(2, 1, 3, &[values.to_vec()]);
    let evidence = checked(fixture::image(1, 1, 3, &[vec![0.0; 3], vec![0.0; 3]]), &source, ResidualRetention::All);
    let detector = probe(&evidence, 1, &[f32::MAX, f32::from_bits(1), f32::MAX], 0.0);
    let exact_source = SourceFrame::capture(evidence.row_shape(row()).unwrap().0, &values).unwrap();
    let block = exact_source.verify_block(&exact_source.encode_initial(23).unwrap()).unwrap();
    let oracle = detector.evaluate(&ProgressiveFrame::from_initial(&block).unwrap()).unwrap();
    let report = LearnedRefinementMonitor::new(vec![detector], LearnedMonitorBudget::default()).unwrap()
        .analyze(&evidence, row()).unwrap();
    assert_eq!(report.outcome(), MonitorOutcome::Alarm);
    let final_score = report.steps().last().unwrap().observations[0].exact_score().unwrap();
    assert_eq!(final_score, &oracle.interval().lower);
    assert_eq!(final_score.magnitude_words()[0], 1);
    assert!(final_score.magnitude_words()[1..].iter().all(|word| *word == 0));
}

#[test]
fn probe_roster_and_row_bindings_cannot_be_relaxed_by_an_empty_allowance() {
    let evidence = rare(ResidualRetention::All); let detector = probe(&evidence, 1, &[0.0, 0.0, 1.0], 0.125);
    assert!(matches!(LearnedRefinementMonitor::new(vec![], LearnedMonitorBudget::default()), Err(Error::InvalidInput)));
    assert!(matches!(LearnedRefinementMonitor::new(vec![detector.clone(), detector.clone()], LearnedMonitorBudget::default()), Err(Error::Duplicate)));
    let monitor = LearnedRefinementMonitor::new(vec![detector], LearnedMonitorBudget { encoded_bytes: 0, ..LearnedMonitorBudget::default() }).unwrap();
    assert_eq!(monitor.analyze(&evidence, KvRow { side: KvSide::Key, ..row() }).unwrap_err(), Error::Binding);
    assert_eq!(monitor.analyze(&evidence, KvRow { position: 1, ..row() }).unwrap_err(), Error::Missing);
    assert_eq!(monitor.analyze(&evidence, row()).unwrap().outcome(), MonitorOutcome::BudgetExhausted);
}

#[test]
fn independent_analyses_do_not_mutate_source_or_old_observations_and_reports_own_their_evidence() {
    let evidence = rare(ResidualRetention::All); let encoded = evidence.encode().unwrap();
    let monitor = alarm_monitor(&evidence, LearnedMonitorBudget::default());
    let first = monitor.analyze(&evidence, row()).unwrap();
    let second = monitor.analyze(&evidence, row()).unwrap();
    assert_eq!(first.work(), second.work());
    assert_eq!(first.steps()[0].observations[0].outcome(), ProbeOutcome::NeedsRefinement);
    assert_eq!(second.steps()[0].observations[0].view().revision(), 0);
    assert_eq!(evidence.view().revision(), 0);
    assert_eq!(evidence.encode().unwrap(), encoded);
    drop(evidence); drop(monitor);
    assert_eq!(first.view().interval(group(0), 2).unwrap(), [0.25, 0.25]);
    assert_eq!(first.steps().last().unwrap().observations[0].outcome(), ProbeOutcome::CertifiedAlarm);
}
