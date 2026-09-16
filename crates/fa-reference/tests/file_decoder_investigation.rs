//! Actual durable checkpoints, numerical counterfactuals and unchanged live rights.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod ordinary;
#[path = "support/investigation_decoder.rs"] mod numerical;
#[allow(dead_code)]
#[path = "support/decoder_inputs.rs"] mod data;
use fa_reference::action::{ElapsedTick, Purpose};
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::comparison::cursor::DecoderComparisonStatus;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::decoder::{FileDecoderConfig,
    checkpoint::{FileDecoderCheckpoint, investigation::*}};
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
    host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, token,
        numerical::budget()).unwrap().unwrap()
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
    let cp = host.capture_decoder_checkpoint(host.revision(), 7, n.actor_revision,
        host.inspect().control.ledger.epoch).unwrap();
    (host, human, configuration, cp)
}
fn request(value: f32) -> FileInvestigationRequest {
    FileInvestigationRequest { experiment: 71, layers: numerical::layers(value), edit_limit: 1,
        continuation: FileInvestigationContinuation::Greedy { first_token: 0, steps: 3 },
        budget: numerical::comparison_budget() }
}
fn finish(mut run: FileDecoderInvestigation) -> FileInvestigationReport {
    assert_eq!(run.purpose(), Purpose::Experiment);
    while run.status() == DecoderComparisonStatus::Running { run.advance().unwrap(); }
    let work = run.work().unwrap();
    let report = run.finish().unwrap();
    assert_eq!(report.purpose(), Purpose::Experiment);
    assert_eq!(report.comparison().work(), work.completed);
    assert_eq!(report.origin(), run.origin()); assert_eq!(report.request(), run.request());
    report
}

#[test]
fn saved_prefix_branches_without_touching_later_actor_state_or_pending_effect_keys() {
    let root = Directory::new(); let (mut host, human, configuration, cp) = create(&root, 100.0, false);
    force(&mut host, 0); // Current actor is later than the saved branch point.
    let keys = ordinary::ready(&mut host, &human, 1, b"still requires keys");
    let before = host.inspect(); let actor = host.actor_snapshot().unwrap();
    let n = host.decoder_inspection().unwrap(); let recovery = host.decoder_recovery_usage().unwrap();
    let canonical = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let report = finish(host.investigate_decoder_checkpoint(&cp, request(10.0)).unwrap());
    assert_eq!(report.origin().journal_revision(), before.revision);
    assert_eq!(report.origin().checkpoint(), cp.info());
    assert_eq!(report.origin().configuration(), &configuration);
    assert_eq!(report.origin().source_scope(), ordinary::profile().delivery.scope);
    assert_eq!(report.comparison().plan().source().tokens(), &[0]);
    assert_eq!(report.comparison().first_different_consumed_token(), Some(2));
    assert_eq!(actor.state.next_position(), 2);
    assert_eq!(host.inspect(), before); assert_eq!(host.actor_snapshot().unwrap(), actor);
    assert_eq!(host.decoder_inspection().unwrap(), n); assert_eq!(host.decoder_recovery_usage().unwrap(), recovery);
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), canonical);
    // Investigation is not an approval and does not invalidate an existing one.
    ordinary::dispatch(&mut host, &keys);
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), ordinary::snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
}

#[test]
fn quiet_counterfactuals_cannot_resume_held_or_terminally_stopped_live_inference() {
    for stop in [false, true] {
        let root = Directory::new(); let (mut host, _, _, cp) = create(&root, 1.5, stop);
        assert!(matches!(force(&mut host, 1), MonitoredStep::Held(_)));
        let before = host.inspect(); let n = host.decoder_inspection().unwrap();
        assert_eq!(n.numerical.status, MonitoringStatus::Held);
        assert_eq!(before.stop.is_some(), stop);
        let report = finish(host.investigate_decoder_checkpoint(&cp, request(0.0)).unwrap());
        assert_eq!(report.comparison().first_different_logits(), None);
        assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap(), n);
        assert!(host.resume_decoder(host.revision(), n.numerical.actor_revision, n.numerical.position).is_err());
        assert!(host.propose(host.revision(), 1, ordinary::spec(&host, b"not authorized"), ordinary::snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn failed_or_cancelled_experiments_do_not_poison_or_refund_the_live_owner() {
    let root = Directory::new(); let (mut host, _, _, cp) = create(&root, 100.0, false);
    let before = host.inspect(); let n = host.decoder_inspection().unwrap();
    let mut failed = host.investigate_decoder_checkpoint(&cp, request(f32::MAX)).unwrap();
    failed.advance().unwrap(); assert_eq!(failed.advance(), Err(Error::Overflow));
    let work = failed.work().unwrap();
    assert_eq!(work.entered_tokens, 2); assert_eq!(work.completed.tokens, 1);
    assert!(failed.finish().is_err()); assert_eq!(failed.advance(), Err(Error::WrongState));
    assert_eq!(failed.work().unwrap(), work);
    let mut cancelled = host.investigate_decoder_checkpoint(&cp, request(10.0)).unwrap();
    cancelled.advance().unwrap(); cancelled.cancel().unwrap();
    assert_eq!(cancelled.work().unwrap().completed.tokens, 1);
    assert!(cancelled.finish().is_err()); assert_eq!(cancelled.advance(), Err(Error::WrongState));
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap(), n);
    assert!(matches!(force(&mut host, 0), MonitoredStep::Released(_)));
}

#[test]
fn foreign_and_pre_recovery_handles_refuse_but_new_handles_allow_passive_paused_investigation() {
    let left = Directory::new(); let right = Directory::new();
    let (host, _, configuration, cp) = create(&left, 100.0, false);
    let (other, _, _, _) = create(&right, 100.0, false);
    assert!(matches!(other.investigate_decoder_checkpoint(&cp, request(10.0)), Err(JournalError::Contract(Error::Binding))));
    let prepared = host.investigate_decoder_checkpoint(&cp, request(10.0)).unwrap();
    drop(host);
    assert_eq!(finish(prepared).comparison().first_different_logits(), Some(1));
    let (host, _) = FileOversight::open_with_decoder(left.store(), ordinary::profile(), &configuration).unwrap();
    assert!(matches!(host.investigate_decoder_checkpoint(&cp, request(10.0)), Err(JournalError::Contract(Error::Binding))));
    assert!(host.decoder_inspection().unwrap().paused); assert!(!host.clock_ready());
    let before = host.inspect(); let fresh = host.decoder_checkpoint(7).unwrap();
    finish(host.investigate_decoder_checkpoint(&fresh, request(10.0)).unwrap());
    assert_eq!(host.inspect(), before);
    assert!(host.decoder_inspection().unwrap().paused); assert!(!host.clock_ready());
}

#[test]
fn read_only_investigation_works_beside_locked_and_faulted_owners_without_cleanup_or_recovery() {
    let root = Directory::new(); let (mut host, _, configuration, cp) = create(&root, 100.0, false);
    let canonical_path = root.store().join("delivery.bin");
    let canonical = std::fs::read(&canonical_path).unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"retain staged evidence").unwrap();
    let report = finish(FileOversight::read_decoder_investigation(root.store(), &ordinary::profile(),
        &configuration, 7, request(10.0)).unwrap());
    assert_eq!(report.origin().journal_revision(), host.revision());
    assert_eq!(report.origin().checkpoint(), cp.info());
    assert_eq!(std::fs::read(&pending).unwrap(), b"retain staged evidence");
    assert_eq!(std::fs::read(&canonical_path).unwrap(), canonical);
    // Existing staged bytes cause the ORIGINAL Store to reject the next write.
    assert!(host.observe_time(host.revision(), ElapsedTick(2)).is_err());
    assert!(matches!(host.investigate_decoder_checkpoint(&cp, request(10.0)), Err(JournalError::Unavailable)));
    let run = FileOversight::read_decoder_investigation(root.store(), &ordinary::profile(),
        &configuration, 7, request(10.0)).unwrap();
    assert_eq!(finish(run).comparison().first_different_logits(), Some(1));
    assert!(host.storage_failure().is_some());
    assert_eq!(std::fs::read(&pending).unwrap(), b"retain staged evidence");
    assert_eq!(std::fs::read(&canonical_path).unwrap(), canonical);
}

#[test]
fn independently_pinned_configuration_and_entire_suffix_are_required_before_branching() {
    let root = Directory::new(); let (mut host, _, configuration, _) = create(&root, 100.0, false);
    force(&mut host, 0); // Valid historical suffix after the selected checkpoint.
    let path = root.store().join("delivery.bin"); let canonical = std::fs::read(&path).unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"keep").unwrap();
    assert!(matches!(FileOversight::read_decoder_investigation(root.store(), &ordinary::profile(),
        &config(99.0), 7, request(10.0)), Err(JournalError::Contract(Error::Binding))));
    assert!(matches!(FileOversight::read_decoder_investigation(root.store(), &ordinary::profile(),
        &configuration, 999, request(10.0)), Err(JournalError::Contract(Error::Missing))));
    let mut corrupt = canonical.clone(); *corrupt.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &corrupt).unwrap();
    assert!(matches!(FileOversight::read_decoder_investigation(root.store(), &ordinary::profile(),
        &configuration, 7, request(10.0)), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(&path).unwrap(), corrupt); assert_eq!(std::fs::read(&pending).unwrap(), b"keep");
    std::fs::write(&path, &canonical).unwrap();
    finish(FileOversight::read_decoder_investigation(root.store(), &ordinary::profile(), &configuration, 7, request(10.0)).unwrap());
    assert_eq!(std::fs::read(path).unwrap(), canonical); assert_eq!(std::fs::read(pending).unwrap(), b"keep");
}

#[test]
fn exact_edit_preimages_scope_and_whole_horizon_refuse_without_altering_checkpoint_or_actor() {
    let root = Directory::new(); let (host, _, _, cp) = create(&root, 100.0, false);
    let before = host.inspect();
    for variant in 0..6 {
        let mut invalid = request(10.0);
        match variant {
            0 => invalid.layers.get_mut(&1).unwrap().edits[0].expected_bits = 1.0_f32.to_bits(),
            1 => invalid.layers.get_mut(&1).unwrap().scope.values = false,
            2 => invalid.layers.get_mut(&1).unwrap().edits[0].cell.position = 1,
            3 => invalid.edit_limit = 0,
            4 => invalid.continuation = FileInvestigationContinuation::TeacherForced(vec![0, 2]),
            _ => invalid.budget.scalar_products = 0,
        }
        assert!(host.investigate_decoder_checkpoint(&cp, invalid).is_err(), "variant {variant}");
        assert_eq!(host.inspect(), before);
    }
    let mut valid = request(10.0);
    valid.continuation = FileInvestigationContinuation::TeacherForced(vec![0, 1]);
    let report = finish(host.investigate_decoder_checkpoint(&cp, valid).unwrap());
    assert_eq!(report.comparison().first_different_consumed_token(), None);
    assert_eq!(report.comparison().first_different_logits(), Some(1));
    assert_eq!(host.inspect(), before);
}
