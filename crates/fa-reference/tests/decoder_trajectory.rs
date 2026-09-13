//! Continuous monitoring is evaluated per task, not by its final frame alone.
#[path = "support/decoder_probe_campaign.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::{ProgressiveFrame, HEADER_BYTES};
use fa_reference::action::consequence::activation::monitor::{MonitorOutcome, RefinementBudget};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredDecoder, MonitoredStep};
use fa_reference::action::consequence::activation::probe::{ProbeOutcome, training::CaseOrigin};
use fa_reference::action::consequence::activation::probe::training::decoder::*;
use fa_reference::action::consequence::activation::probe::training::decoder::trajectory::*;
use fa_reference::action::consequence::activation::probe::training::interchange::{LayerMonitorSettings, MonitorExportSettings};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderModel, DecoderProfile, MAX_DECODER_PRODUCTS};
use fa_reference::Error;

fn settings() -> MonitorExportSettings {
    let budget = RefinementBudget { encoded_bytes: 10_000, probe_coordinates: 10_000 };
    MonitorExportSettings { generation: 77, budget,
        layers: (1..=2).map(|id| (id, LayerMonitorSettings { levels: vec![23], budget })).collect() }
}
fn rules() -> TrajectoryCriteria { TrajectoryCriteria::new(1, 1, 1, 0, 0).unwrap() }
fn origin(task: u64) -> CaseOrigin { CaseOrigin { task, lineage: task + 1000 } }
fn trajectory(task: u64, tokens: &[u32], effect: Option<usize>) -> LabelledTrajectory {
    LabelledTrajectory { origin: origin(task), tokens: tokens.to_vec(),
        expectation: effect.map_or(TrajectoryExpectation::Benign, |effect_position| TrajectoryExpectation::Violation { effect_position }) }
}
fn population() -> Vec<LabelledTrajectory> {
    vec![trajectory(10, &[0, 0, 0], None), trajectory(11, &[0, 1, 1], Some(1))]
}
fn trained(model: &DecoderModel) -> DecoderCampaign { campaign(&capture(model, &cases())) }
fn evaluate(campaign: &DecoderCampaign, options: MonitorExportSettings, cases: Vec<LabelledTrajectory>) -> TrajectoryReport {
    let mut suite = campaign.trajectory_suite(options, cases, rules()).unwrap();
    let mut budget = TrajectoryBudget::new(suite.planned_work()).unwrap();
    let report = suite.run(&mut budget).unwrap();
    assert_eq!(budget.remaining(), TrajectoryWork::default());
    report
}
fn held(case: &TrajectoryCaseResult) -> (usize, MonitorOutcome) {
    match case.termination() {
        TrajectoryTermination::Held { position, review } => (*position, review.outcome()),
        other => panic!("expected actual monitor hold: {other:?}"),
    }
}

#[test]
fn full_histories_use_the_original_monitor_and_export_exactly_the_exercised_configuration() {
    let model = model(); let campaign = trained(&model); let cases = population();
    let bytes = campaign.monitor_json(&settings(), 1_048_576).unwrap();
    let report = evaluate(&campaign, settings(), cases.clone());
    assert!(report.accepted()); assert_eq!(report.counts().total(), 2);
    assert_eq!(report.counts().benign_complete, 1); assert_eq!(report.counts().violation_timely_alarm, 1);
    assert_eq!(report.monitor_json().unwrap(), bytes);
    assert_eq!(report.cases()[&origin(11)].alarm_lead_tokens(), Some(0));
    for case in cases {
        let mut direct = MonitoredDecoder::from_json(model.clone(), case.origin.task, &bytes).unwrap();
        let mut quiet = 0;
        for (position, token) in case.tokens.iter().copied().enumerate() {
            match direct.advance(position as u64, token, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap() {
                MonitoredStep::Released(_) => quiet += 1,
                MonitoredStep::Held(actual) => {
                    let TrajectoryTermination::Held { review, .. } = report.cases()[&case.origin].termination() else { panic!("missing hold"); };
                    assert_eq!(actual, *review); break;
                }
            }
        }
        let actual = &report.cases()[&case.origin];
        assert_eq!(actual.quiet_tokens(), quiet);
        assert_eq!(actual.numerical(), direct.decoder_work());
        assert_eq!(actual.monitoring(), direct.monitoring_work());
    }
}

#[test]
fn an_early_benign_false_stop_is_not_erased_by_a_quiet_final_residual() {
    let model = model(); let campaign = trained(&model); let mut cases = population();
    cases[0].tokens = vec![0, 1, 0];
    // Independent unrestricted reference execution demonstrates why selecting
    // only the final residual would miss this task-level false stop.
    let mut raw = model.session(500).unwrap(); let mut last = None;
    for (position, token) in cases[0].tokens.iter().copied().enumerate() {
        last = Some(raw.advance(position as u64, token, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap());
    }
    for layer in last.unwrap().layers {
        let source = layer.residual.source(); let bytes = source.encode_initial(23).unwrap();
        let frame = ProgressiveFrame::from_initial(&source.verify_block(&bytes).unwrap()).unwrap();
        assert_eq!(campaign.probes().unwrap()[&layer.layer].evaluate(&frame).unwrap().outcome(), ProbeOutcome::CertifiedQuiet);
    }
    let report = evaluate(&campaign, settings(), cases);
    assert_eq!(held(&report.cases()[&origin(10)]), (1, MonitorOutcome::Alarm));
    assert_eq!(report.counts().benign_alarm, 1); assert!(!report.accepted());
    assert_eq!(report.monitor_json(), Err(Error::WrongState));
    assert!(campaign.accepted()); // Original final-frame result is not rewritten.
}

#[test]
fn an_alarm_after_the_declared_effect_is_late_not_a_timely_success() {
    let campaign = trained(&model()); let mut cases = population();
    cases[1].tokens = vec![0, 0, 1];
    let late = evaluate(&campaign, settings(), cases.clone());
    assert_eq!(held(&late.cases()[&origin(11)]), (2, MonitorOutcome::Alarm));
    assert_eq!(late.counts().violation_late_alarm, 1);
    assert_eq!(late.counts().violation_timely_alarm, 0);
    assert_eq!(late.cases()[&origin(11)].alarm_lead_tokens(), None); assert!(!late.accepted());
    cases[1].expectation = TrajectoryExpectation::Violation { effect_position: 2 };
    let timely = evaluate(&campaign, settings(), cases);
    assert_eq!(timely.counts().violation_timely_alarm, 1); assert!(timely.accepted());
    assert_eq!(timely.cases()[&origin(11)].alarm_lead_tokens(), Some(0));
}

#[test]
fn complete_quiet_violations_and_true_early_alarms_have_distinct_denominators() {
    let campaign = trained(&model()); let mut cases = population();
    cases[1] = trajectory(11, &[0, 0], Some(1));
    cases.push(trajectory(12, &[0, 1, 0, 0], Some(3)));
    let report = evaluate(&campaign, settings(), cases);
    assert_eq!(report.counts().violations(), 2); assert_eq!(report.counts().violation_complete, 1);
    assert_eq!(report.counts().violation_timely_alarm, 1); assert!(!report.accepted());
    assert_eq!(report.cases()[&origin(12)].alarm_lead_tokens(), Some(2));
}

#[test]
fn monitoring_allowance_is_not_renewed_per_token_and_exhaustion_is_not_detection() {
    let campaign = trained(&model()); let mut options = settings();
    options.budget.encoded_bytes = 2 * (HEADER_BYTES + 8); // Exactly one all-layer token.
    let report = evaluate(&campaign, options, population());
    for result in report.cases().values() {
        assert_eq!(held(result), (1, MonitorOutcome::BudgetExhausted));
        assert_eq!(result.quiet_tokens(), 1); assert_eq!(result.numerical().tokens, 2);
    }
    assert_eq!(report.counts().benign_other_hold, 1);
    assert_eq!(report.counts().violation_other_hold, 1);
    assert_eq!(report.counts().violation_timely_alarm, 0); assert!(!report.accepted());
}

#[test]
fn every_admission_dimension_is_checked_atomically_before_a_suite_is_used() {
    let campaign = trained(&model());
    for dimension in 0..6 {
        let mut suite = campaign.trajectory_suite(settings(), population(), rules()).unwrap();
        let full = suite.planned_work(); let mut short = full;
        match dimension { 0 => short.cases -= 1, 1 => short.original_tokens -= 1,
            2 => short.scalar_products -= 1, 3 => short.monitor_encoded_bytes -= 1,
            4 => short.monitor_probe_coordinates -= 1, _ => short.retained_score_words -= 1 }
        let mut budget = TrajectoryBudget::new(short).unwrap();
        assert!(matches!(suite.run(&mut budget), Err(Error::Limit)));
        assert!(!suite.started()); assert_eq!(budget.remaining(), short);
        assert!(suite.run(&mut TrajectoryBudget::new(full).unwrap()).unwrap().accepted());
    }
}

#[test]
fn short_circuits_keep_full_admission_and_neither_suite_nor_budget_can_be_rerolled() {
    let campaign = trained(&model()); let mut suite = campaign.trajectory_suite(settings(), population(), rules()).unwrap();
    let planned = suite.planned_work(); let mut budget = TrajectoryBudget::new(planned).unwrap();
    let report = suite.run(&mut budget).unwrap();
    assert!(report.cases().values().map(|case| case.numerical().tokens as usize).sum::<usize>() < planned.original_tokens);
    assert_eq!(budget.remaining(), TrajectoryWork::default());
    assert!(matches!(suite.run(&mut TrajectoryBudget::new(planned).unwrap()), Err(Error::WrongState)));
    let mut second = campaign.trajectory_suite(settings(), population(), rules()).unwrap();
    assert!(matches!(second.run(&mut budget), Err(Error::Limit))); assert!(!second.started());
}

#[test]
fn origins_from_any_prior_split_and_exact_duplicate_histories_cannot_enter_holdout() {
    let campaign = trained(&model());
    for prior in 1..=6 {
        let mut cases = population(); cases[1].origin.task = prior;
        assert!(matches!(campaign.trajectory_suite(settings(), cases, rules()), Err(Error::Duplicate)));
        let mut cases = population(); cases[1].origin.lineage = prior + 100;
        assert!(matches!(campaign.trajectory_suite(settings(), cases, rules()), Err(Error::Duplicate)));
    }
    let mut cases = population(); cases[0].tokens = vec![0];
    assert!(matches!(campaign.trajectory_suite(settings(), cases, rules()), Err(Error::Duplicate)));
    let mut cases = population(); cases.push(trajectory(12, &[0, 0, 0], None));
    assert!(matches!(campaign.trajectory_suite(settings(), cases, rules()), Err(Error::Duplicate)));
    let mut cases = population(); cases[1].origin = cases[0].origin;
    assert!(matches!(campaign.trajectory_suite(settings(), cases, rules()), Err(Error::Duplicate)));
}

#[test]
fn a_malformed_late_case_or_missing_class_refuses_the_entire_plan() {
    let campaign = trained(&model());
    for malformed in [trajectory(11, &[0, 99], Some(1)), trajectory(11, &[0, 1], Some(2)), trajectory(11, &[], Some(0))] {
        assert!(matches!(campaign.trajectory_suite(settings(), vec![population()[0].clone(), malformed], rules()), Err(Error::InvalidInput)));
    }
    assert!(matches!(campaign.trajectory_suite(settings(), vec![population()[0].clone()], rules()), Err(Error::Incomplete)));
    assert!(matches!(campaign.trajectory_suite(settings(), vec![population()[0].clone(), trajectory(11, &[1; 9], Some(1))], rules()), Err(Error::Limit)));
}

fn extra_token_model(overflow: bool) -> DecoderModel {
    use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderLayerWeights;
    let source = model(); let p = source.profile(); let mut shape = p.shape(); shape.vocabulary = 3;
    let p = DecoderProfile::new(p.identity(), shape, p.epsilon(), p.theta()).unwrap();
    let layers = (0..2).map(|_| DecoderLayerWeights {
        attention_norm: vec![1.0; 2], queries: vec![0.0; 4], keys: vec![0.0; 4], values: vec![0.0; 4],
        attention_output: vec![0.0; 4], feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4],
    }).collect();
    DecoderModel::new(p, if overflow { vec![-1.0, 1.0, 1.0, -1.0, 1.0, 1.0] }
        else { vec![-1.0, 0.0, 1.0, 0.0, 0.0, 0.0] }, layers, vec![1.0; 2],
        if overflow { vec![f32::MAX; 6] } else { vec![0.0; 6] }).unwrap()
}

#[test]
fn exact_threshold_equality_holds_without_earning_violation_alarm_credit() {
    let campaign = trained(&extra_token_model(false));
    let report = evaluate(&campaign, settings(), vec![trajectory(10, &[0, 0], None), trajectory(11, &[0, 2, 1], Some(2))]);
    assert_eq!(held(&report.cases()[&origin(11)]), (1, MonitorOutcome::AtThreshold));
    assert_eq!(report.counts().violation_other_hold, 1); assert_eq!(report.counts().violation_timely_alarm, 0);
    assert!(!report.accepted()); assert_eq!(report.cases()[&origin(11)].alarm_lead_tokens(), None);
}

#[test]
fn numerical_failure_remains_a_case_and_does_not_censor_later_tasks() {
    let campaign = trained(&extra_token_model(true));
    let cases = vec![trajectory(10, &[0, 0], None), trajectory(11, &[0, 2, 1], Some(2)),
        trajectory(12, &[0, 0, 0], None), trajectory(13, &[0, 1, 0], Some(2))];
    let report = evaluate(&campaign, settings(), cases);
    assert_eq!(report.counts().total(), 4); assert_eq!(report.counts().benign_complete, 2);
    assert_eq!(report.counts().violation_failed, 1); assert_eq!(report.counts().violation_timely_alarm, 1);
    assert!(matches!(report.cases()[&origin(11)].termination(), TrajectoryTermination::Failed { position: 1, .. }));
    assert_eq!(report.cases()[&origin(11)].quiet_tokens(), 1); assert!(!report.accepted());
    assert_eq!(report.monitor_json(), Err(Error::WrongState));
}

#[test]
fn ordering_does_not_change_task_denominators_work_or_original_report_identity() {
    let campaign = trained(&model()); let a = population(); let mut b = a.clone(); b.reverse();
    let left = evaluate(&campaign, settings(), a); let right = evaluate(&campaign, settings(), b);
    assert_eq!(left.cases(), right.cases()); assert_eq!(left.counts(), right.counts());
    assert_eq!(left.admitted_work(), right.admitted_work()); assert_eq!(left.monitor_json(), right.monitor_json());
}

#[test]
fn suite_retains_original_parameters_without_a_replacement_model_or_source_lifetime() {
    let mut suite = {
        let model = model(); let corpus = capture(&model, &cases()); let campaign = campaign(&corpus);
        campaign.trajectory_suite(settings(), population(), rules()).unwrap()
    };
    let work = suite.planned_work();
    assert!(suite.run(&mut TrajectoryBudget::new(work).unwrap()).unwrap().accepted());
}
