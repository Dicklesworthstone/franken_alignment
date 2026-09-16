//! Original durable checkpoints, fixed stochastic trials and unchanged effect authority.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/investigation_decoder.rs"] mod numerical;
#[allow(dead_code)]
#[path = "support/decoder_inputs.rs"] mod data;
use fa_reference::action::{ElapsedTick, Purpose};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::{
    DecoderComparisonBudget, cursor::DecoderComparisonStatus, sampled::DecoderSampledComparisonBudget,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::{SamplingPolicy, SamplingStart};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::{JournalError, Reconciliation}};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::{FileDecoderConfig,
    checkpoint::{FileDecoderCheckpoint, investigation::sampled::*}};
use fa_reference::action::consequence::oversight::decoder_host::HostedStopPolicy;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::Error;
use ordinary::Directory;

fn config(threshold: f32) -> FileDecoderConfig {
    FileDecoderConfig::new(numerical::profile(), numerical::weights(), data::monitor(threshold),
        data::sampling(), 5, DecoderBindingLimits::default()).unwrap()
}
fn force(host: &mut FileOversight, token: u32) -> MonitoredStep {
    let n = host.decoder_inspection().unwrap().numerical;
    host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, token, numerical::budget()).unwrap().unwrap()
}
fn create(root: &Directory, threshold: f32, stop: bool)
    -> (FileOversight, FileHumanReviewer, FileDecoderConfig, FileDecoderCheckpoint)
{
    let configuration = config(threshold);
    let (mut host, human) = FileOversight::create(root.store(), ordinary::profile()).unwrap();
    host.enable_decoder(host.revision(), configuration.clone()).unwrap();
    if stop { host.enable_decoder_stop(host.revision(), HostedStopPolicy::new(1, 1, 8000).unwrap()).unwrap(); }
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(matches!(force(&mut host, 0), MonitoredStep::Released(_)));
    let n = host.decoder_inspection().unwrap().numerical;
    let cp = host.capture_decoder_checkpoint(host.revision(), 7, n.actor_revision, host.inspect().control.ledger.epoch).unwrap();
    (host, human, configuration, cp)
}
fn request(value: f32) -> FileSampledInvestigationRequest {
    FileSampledInvestigationRequest { experiment: 71, layers: numerical::layers(value), edit_limit: 1,
        first_token: 0, steps: 3, sampling_policy: SamplingPolicy::new(1, 1, 2, 2.0, 0, 1.0).unwrap(),
        sampling_stream: 99, seeds: vec![0, 1, 7, 9], budget: DecoderSampledComparisonBudget {
            comparison: numerical::comparison_budget(), sampling_logits: 32,
        } }
}
fn finish(mut run: FileSampledInvestigation) -> FileSampledInvestigationReport {
    assert_eq!(run.purpose(), Purpose::Experiment);
    while !run.is_finished() { run.advance().unwrap(); }
    run.finish().unwrap()
}
fn words(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }

#[test]
fn fixed_seed_campaign_matches_native_trials_and_leaves_later_actor_and_keys_intact() {
    let root = Directory::new(); let (mut host, human, configuration, cp) = create(&root, 100.0, false);
    force(&mut host, 0); // Deliberately later than the saved original checkpoint.
    let keys = ordinary::ready(&mut host, &human, 1, b"not an experiment approval");
    let before = host.inspect(); let n = host.decoder_inspection().unwrap(); let actor = host.actor_snapshot().unwrap();
    let canonical = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let original = request(10.0);
    let report = finish(host.investigate_sampled_decoder_checkpoint(&cp, original.clone()).unwrap());
    assert_eq!(report.purpose(), Purpose::Experiment); assert_eq!(report.request(), &original);
    assert_eq!(report.origin().checkpoint(), cp.info()); assert_eq!(report.origin().configuration(), &configuration);
    assert_eq!(report.origin().journal_revision(), before.revision);
    assert_eq!(report.completed_trials(), 4); assert_eq!(report.trials().len(), 4);
    assert!(!report.cancelled() && !report.interrupted());
    assert_eq!(report.planned().comparison.retained_logit_values, 48);
    assert_eq!(report.planned().sampling_logits, 32);
    for (trial, seed) in report.trials().iter().zip(&original.seeds) {
        assert_eq!(trial.seed(), *seed); assert_eq!(trial.status(), FileSampledTrialStatus::Completed);
        let plan = numerical::plan(10.0);
        let mut native = plan.begin_sampled_comparison(0, 3, SamplingStart {
            policy: original.sampling_policy.clone(), stream: original.sampling_stream, seed: *seed,
        }, original.budget).unwrap();
        while native.status() == DecoderComparisonStatus::Running { native.advance().unwrap(); }
        let expected = native.finish().unwrap(); assert_eq!(trial.work(), expected.work());
        for (a, b) in trial.pairs().iter().zip(expected.steps()) {
            assert_eq!(a.control_choice, b.control_choice); assert_eq!(a.intervention_choice, b.intervention_choice);
            assert_eq!(words(&a.control.logits), words(&b.control.logits));
            assert_eq!(words(&a.intervention.logits), words(&b.intervention.logits));
        }
    }
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap(), n);
    assert_eq!(host.actor_snapshot().unwrap(), actor);
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), canonical);
    ordinary::dispatch(&mut host, &keys);
    let outcome = host.publish_checked(host.revision(), 1, Some(&keys.inputs), ordinary::snapshot(), ElapsedTick(1)).unwrap().outcome;
    assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
}

#[test]
fn early_cancellation_cannot_hide_failed_or_unstarted_seeds_from_the_denominator() {
    let root = Directory::new(); let (host, _, _, cp) = create(&root, 100.0, false);
    let mut run = host.investigate_sampled_decoder_checkpoint(&cp, request(10.0)).unwrap();
    for _ in 0..7 { run.advance().unwrap(); } // One full seed plus the next control token.
    assert!(matches!(run.finish(), Err(Error::Incomplete)));
    let partial = run.trial_work(1).unwrap();
    assert_eq!(partial.numerical.completed.tokens, 1);
    run.cancel().unwrap(); run.cancel().unwrap();
    assert_eq!(run.advance(), Err(Error::WrongState));
    let report = run.finish().unwrap();
    assert!(report.cancelled()); assert_eq!(report.trials().len(), 4); assert_eq!(report.completed_trials(), 1);
    assert_eq!(report.trials().iter().map(|r| r.status()).collect::<Vec<_>>(), vec![
        FileSampledTrialStatus::Completed, FileSampledTrialStatus::Cancelled,
        FileSampledTrialStatus::NotRun, FileSampledTrialStatus::NotRun,
    ]);
    assert_eq!(report.trials()[1].work(), partial);
    assert_eq!(report.trials().iter().map(|r| r.seed()).collect::<Vec<_>>(), request(10.0).seeds);
    assert_eq!(run.finish().unwrap().trials()[1].work(), partial);
}

#[test]
fn real_numerical_failures_remain_visible_and_do_not_skip_the_remaining_fixed_seeds() {
    let root = Directory::new(); let (mut host, _, _, cp) = create(&root, 100.0, false);
    let before = host.inspect(); let mut run = host.investigate_sampled_decoder_checkpoint(&cp, request(f32::MAX)).unwrap();
    for index in 0..4 {
        run.advance().unwrap();
        assert!(matches!(run.advance().unwrap(), FileSampledInvestigationProgress::Trial {
            index: actual, status: DecoderComparisonStatus::Failed(Error::Overflow), ..
        } if actual == index));
    }
    assert!(run.is_finished()); assert_eq!(run.advance().unwrap(), FileSampledInvestigationProgress::Finished);
    let report = run.finish().unwrap(); assert_eq!(report.completed_trials(), 0); assert_eq!(report.trials().len(), 4);
    for trial in report.trials() {
        assert_eq!(trial.status(), FileSampledTrialStatus::Failed(Error::Overflow));
        assert_eq!(trial.work().numerical.entered_tokens, 2);
        assert_eq!(trial.work().numerical.completed.tokens, 1); assert!(trial.pairs().is_empty());
    }
    assert_eq!(host.inspect(), before); assert!(matches!(force(&mut host, 0), MonitoredStep::Released(_)));
    let mut cancelled = host.investigate_sampled_decoder_checkpoint(&cp, request(f32::MAX)).unwrap();
    cancelled.advance().unwrap(); cancelled.advance().unwrap(); cancelled.cancel().unwrap();
    let rows = cancelled.finish().unwrap();
    assert_eq!(rows.trials()[0].status(), FileSampledTrialStatus::Failed(Error::Overflow));
    assert!(rows.trials()[1..].iter().all(|r| r.status() == FileSampledTrialStatus::NotRun));
}

#[test]
fn aggregate_admission_cannot_refresh_a_per_trial_budget_and_rejects_duplicate_seeds() {
    let root = Directory::new(); let (host, _, _, cp) = create(&root, 100.0, false);
    let before = host.inspect(); let per = numerical::model().estimate(1, 3).unwrap().scalar_products().unwrap() * 2;
    for variant in 0..5 {
        let mut invalid = request(10.0);
        match variant {
            0 => invalid.budget.comparison.scalar_products = per,
            1 => invalid.budget.comparison.retained_logit_values = 12,
            2 => invalid.budget.sampling_logits = 8,
            3 => invalid.seeds[1] = invalid.seeds[0],
            _ => invalid.seeds = (0..=MAX_SAMPLED_INVESTIGATION_SEEDS as u64).collect(),
        }
        let result = host.investigate_sampled_decoder_checkpoint(&cp, invalid);
        let expected = if variant == 3 { Error::Duplicate } else { Error::Limit };
        assert!(matches!(&result, Err(JournalError::Contract(error)) if *error == expected));
        assert_eq!(host.inspect(), before);
    }
    let mut exact = request(10.0);
    exact.budget.comparison = DecoderComparisonBudget { scalar_products: per * 4, retained_logit_values: 48 };
    assert_eq!(finish(host.investigate_sampled_decoder_checkpoint(&cp, exact).unwrap()).completed_trials(), 4);
}

#[test]
fn canonical_campaign_works_beside_a_faulted_owner_without_cleaning_or_recovering_it() {
    let root = Directory::new(); let (mut host, _, configuration, cp) = create(&root, 100.0, false);
    let path = root.store().join("delivery.bin"); let canonical = std::fs::read(&path).unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retain staged evidence").unwrap();
    assert!(host.observe_time(host.revision(), ElapsedTick(2)).is_err());
    assert!(matches!(host.investigate_sampled_decoder_checkpoint(&cp, request(10.0)), Err(JournalError::Unavailable)));
    let report = finish(FileOversight::read_sampled_decoder_investigation(root.store(), &ordinary::profile(),
        &configuration, 7, request(10.0)).unwrap());
    assert_eq!(report.completed_trials(), 4); assert_eq!(report.origin().journal_revision(), host.revision());
    assert!(host.storage_failure().is_some()); assert_eq!(std::fs::read(path).unwrap(), canonical);
    assert_eq!(std::fs::read(pending).unwrap(), b"retain staged evidence");
}

#[test]
fn exact_configuration_and_a_valid_post_checkpoint_suffix_are_required_for_sampling() {
    let root = Directory::new(); let (mut host, _, configuration, _) = create(&root, 100.0, false);
    force(&mut host, 0);
    let path = root.store().join("delivery.bin"); let canonical = std::fs::read(&path).unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"keep").unwrap();
    assert!(matches!(FileOversight::read_sampled_decoder_investigation(root.store(), &ordinary::profile(),
        &config(99.0), 7, request(10.0)), Err(JournalError::Contract(Error::Binding))));
    let mut corrupt = canonical.clone(); *corrupt.last_mut().unwrap() ^= 1; std::fs::write(&path, &corrupt).unwrap();
    assert!(matches!(FileOversight::read_sampled_decoder_investigation(root.store(), &ordinary::profile(),
        &configuration, 7, request(10.0)), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(&path).unwrap(), corrupt); assert_eq!(std::fs::read(&pending).unwrap(), b"keep");
    std::fs::write(&path, &canonical).unwrap();
    assert_eq!(finish(FileOversight::read_sampled_decoder_investigation(root.store(), &ordinary::profile(),
        &configuration, 7, request(10.0)).unwrap()).completed_trials(), 4);
    assert_eq!(std::fs::read(path).unwrap(), canonical);
}

#[test]
fn sampling_cannot_resume_a_stopped_actor_or_reuse_a_pre_recovery_checkpoint_handle() {
    let root = Directory::new(); let (mut host, _, configuration, cp) = create(&root, 1.5, true);
    assert!(matches!(force(&mut host, 1), MonitoredStep::Held(_)));
    let before = host.inspect(); let n = host.decoder_inspection().unwrap();
    assert!(before.stop.is_some()); assert_eq!(n.numerical.status, MonitoringStatus::Held);
    let report = finish(host.investigate_sampled_decoder_checkpoint(&cp, request(0.0)).unwrap());
    assert_eq!(report.completed_trials(), 4); assert_eq!(host.inspect(), before);
    assert!(host.resume_decoder(host.revision(), n.numerical.actor_revision, n.numerical.position).is_err());
    drop(host);
    let (host, _) = FileOversight::open_with_decoder(root.store(), ordinary::profile(), &configuration).unwrap();
    assert!(matches!(host.investigate_sampled_decoder_checkpoint(&cp, request(0.0)), Err(JournalError::Contract(Error::Binding))));
    let fresh = host.decoder_checkpoint(7).unwrap(); let before = host.inspect();
    finish(host.investigate_sampled_decoder_checkpoint(&fresh, request(0.0)).unwrap());
    assert_eq!(host.inspect(), before); assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
    assert!(host.inspect().stop.is_some());
}
