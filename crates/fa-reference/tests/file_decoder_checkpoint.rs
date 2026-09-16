//! Original numerical replay/reset through canonical storage; no imported rights.
#![cfg(unix)]
#[path = "support/file_decoder.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    containment::FileResetRequest, decoder::checkpoint::FileDecoderCheckpoint};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::gate::ReviewBinding;
use fa_reference::Error;

fn capture(host: &mut FileOversight, id: u64) -> FileDecoderCheckpoint {
    let n = host.decoder_inspection().unwrap().numerical;
    host.capture_decoder_checkpoint(host.revision(), id, n.actor_revision, host.inspect().control.ledger.epoch).unwrap()
}
fn reset_request(host: &FileOversight, operation: u64) -> FileResetRequest {
    let c = host.inspect().control;
    FileResetRequest { operation, expected_control_sequence: c.sequence,
        expected_actor_revision: host.decoder_inspection().unwrap().numerical.actor_revision,
        expected_authority_epoch: c.ledger.epoch,
        binding: ReviewBinding { round: 5000 + operation, evidence_root: [17; 32], reducer_generation: 1 },
        retained_targets: vec![host.inspect().target] }
}

#[test]
fn reset_recomputes_the_checkpoint_and_preserves_the_original_next_sample() {
    let root = Directory::new(); let (mut host, _, _) = create_decoder(&root, 3.0);
    assert!(matches!(forced(&mut host, 0), MonitoredStep::Released(_)));
    let cp = capture(&mut host, 7);
    let MonitoredSampledStep::Released(expected) = sampled(&mut host) else { panic!("quiet model held"); };
    let before = host.decoder_inspection().unwrap().numerical;
    let request = reset_request(&host, 1);
    let receipt = host.reset_decoder_checkpoint(host.revision(), &cp, request.clone(), budget()).unwrap().unwrap();
    assert!(receipt.control.restored); assert_eq!(receipt.control.consequence, Consequence::ResetToCheckpoint);
    assert_eq!(receipt.position, 1); assert_eq!(receipt.sampled_draws, 0); assert_eq!(receipt.resumed_stream, Some(6));
    assert!(receipt.actor_revision > before.actor_revision);
    assert_eq!(host.inspect().control.ledger.epoch, request.expected_authority_epoch + 1);
    assert_eq!(host.decoder_inspection().unwrap().numerical.numerical.tokens,
        before.numerical.tokens + receipt.replay_numerical.tokens);
    let revision = host.revision(); let usage = host.decoder_recovery_usage().unwrap();
    assert_eq!(host.reset_decoder_checkpoint(0, &cp, request.clone(), budget()).unwrap().unwrap(), receipt);
    assert_eq!(host.revision(), revision); assert_eq!(host.decoder_recovery_usage().unwrap(), usage);
    let MonitoredSampledStep::Released(actual) = sampled(&mut host) else { panic!("replayed quiet sample held"); };
    assert_eq!(actual.choice(), expected.choice());
    assert_eq!(host.decoder_reset_result(1).unwrap().unwrap(), receipt);
    let mut conflicting = request; conflicting.binding.round += 1;
    assert_eq!(host.reset_decoder_checkpoint(host.revision(), &cp, conflicting, budget()), Err(JournalError::Contract(Error::Binding)));
}

#[test]
fn held_continuation_can_only_rewind_via_the_original_incident_and_narrowing_transition() {
    let root = Directory::new(); let (mut host, _, _) = create_decoder(&root, 1.5);
    forced(&mut host, 0); let cp = capture(&mut host, 7);
    assert!(matches!(sampled(&mut host), MonitoredSampledStep::Held(_)));
    assert!(capture_attempt(&mut host, 8).is_err());
    let request = reset_request(&host, 1);
    let receipt = host.reset_decoder_checkpoint(host.revision(), &cp, request, budget()).unwrap().unwrap();
    assert_eq!(receipt.control.incident_count, 1);
    assert_eq!(host.decoder_inspection().unwrap().numerical.status, MonitoringStatus::Ready);
    // The old monitor and sampler remain compulsory: identical continuation is
    // held again, not reclassified or rerolled by the reset operation.
    assert!(matches!(sampled(&mut host), MonitoredSampledStep::Held(_)));
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    assert_eq!(host.inspect().executions, 0);
}
fn capture_attempt(host: &mut FileOversight, id: u64) -> Result<FileDecoderCheckpoint, JournalError> {
    let n = host.decoder_inspection().unwrap().numerical;
    host.capture_decoder_checkpoint(host.revision(), id, n.actor_revision, host.inspect().control.ledger.epoch)
}

#[test]
fn checkpoints_survive_recovery_but_old_handles_and_automatic_resume_do_not() {
    let root = Directory::new(); let (mut host, _, config) = create_decoder(&root, 1.5);
    assert!(capture_attempt(&mut host, 7).is_err());
    forced(&mut host, 0); let old = capture(&mut host, 7);
    assert!(matches!(sampled(&mut host), MonitoredSampledStep::Held(_)));
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), profile(), &config).unwrap();
    let cp = host.decoder_checkpoint(7).unwrap(); assert_eq!(old.info(), cp.info());
    let request = reset_request(&host, 1);
    assert_eq!(host.reset_decoder_checkpoint(host.revision(), &old, request.clone(), budget()), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.reset_decoder_checkpoint(host.revision(), &cp, request.clone(), budget()), Err(JournalError::Contract(Error::Incomplete)));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let receipt = host.reset_decoder_checkpoint(host.revision(), &cp, request, budget()).unwrap().unwrap();
    assert!(receipt.control.restored); assert!(host.decoder_inspection().unwrap().paused);
    assert!(host.advance_decoder_sampled(host.revision(), receipt.actor_revision, receipt.position, sample_budget()).is_err());
    host.resume_decoder(host.revision(), receipt.actor_revision, receipt.position).unwrap();
    assert!(matches!(sampled(&mut host), MonitoredSampledStep::Held(_)));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn reset_cancels_only_undispatched_work_and_requires_new_two_key_review() {
    let root = Directory::new(); let (mut host, human, _) = create_decoder(&root, 3.0);
    forced(&mut host, 0); let cp = capture(&mut host, 7);
    let sent = ready(&mut host, &human, 1, b"old sent"); dispatch(&mut host, &sent);
    let reserved = ready(&mut host, &human, 2, b"old reserved");
    let before = host.inspect().control.ledger;
    forced(&mut host, 1);
    let request = reset_request(&host, 1);
    let receipt = host.reset_decoder_checkpoint(host.revision(), &cp, request, budget()).unwrap().unwrap();
    assert_eq!(receipt.control.cancelled, vec![2]);
    assert_eq!(host.inspect().control.ledger.charged, before.charged);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.stages[&2], ActionState::Cancelled);
    assert!(host.dispatch(host.revision(), &reserved.automatic, &reserved.human,
        &reserved.action, &reserved.inputs, snapshot()).is_err());
    let sealed = host.publish_checked(host.revision(), 1, Some(&sent.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert!(matches!(sealed.outcome, EndpointOutcome::NotExecuted { .. }));
    assert_eq!(host.inspect().control.ledger.charged, before.charged);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(sealed.outcome));
    assert_eq!(host.inspect().control.ledger.charged, 0);
    let fresh = ready(&mut host, &human, 3, b"new review"); dispatch(&mut host, &fresh);
    let outcome = host.publish_checked(host.revision(), 3, Some(&fresh.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome;
    assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 3).unwrap(), Reconciliation::Resolved(outcome));
}

#[test]
fn insufficient_budget_is_a_deduplicated_refusal_not_free_or_repeated_replay() {
    let root = Directory::new(); let (mut host, _, _) = create_decoder(&root, 3.0);
    forced(&mut host, 0); let cp = capture(&mut host, 7); sampled(&mut host);
    let before = host.decoder_inspection().unwrap().numerical;
    let usage = host.decoder_recovery_usage().unwrap(); let request = reset_request(&host, 1);
    let small = DecoderBudget { scalar_products: 0 };
    assert_eq!(host.reset_decoder_checkpoint(host.revision(), &cp, request.clone(), small).unwrap(), Err(Error::Limit));
    let revision = host.revision();
    assert_eq!(host.reset_decoder_checkpoint(0, &cp, request.clone(), small).unwrap(), Err(Error::Limit));
    assert_eq!(host.revision(), revision); assert_eq!(host.decoder_recovery_usage().unwrap(), usage);
    assert_eq!(host.decoder_inspection().unwrap().numerical, before);
    assert_eq!(host.reset_decoder_checkpoint(host.revision(), &cp, request, budget()), Err(JournalError::Contract(Error::Binding)));
    let request = reset_request(&host, 2);
    assert!(host.reset_decoder_checkpoint(host.revision(), &cp, request, budget()).unwrap().unwrap().control.restored);
}

#[test]
fn capture_retries_preserve_the_original_prefix_and_reset_witness_tampering_refuses() {
    let root = Directory::new(); let (mut host, _, _) = create_decoder(&root, 3.0);
    forced(&mut host, 0); let cp = capture(&mut host, 7); sampled(&mut host);
    let revision = host.revision();
    assert_eq!(host.capture_decoder_checkpoint(0, 7, cp.info().actor_revision, cp.info().authority_epoch).unwrap().info(), cp.info());
    assert_eq!(host.revision(), revision);
    assert!(capture_attempt(&mut host, 7).is_err());
    let request = reset_request(&host, 1);
    host.reset_decoder_checkpoint(host.revision(), &cp, request, budget()).unwrap().unwrap();
    let path = root.store().join("delivery.bin"); let original = std::fs::read(&path).unwrap();
    let mut changed = original.clone(); *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, changed).unwrap();
    assert_eq!(FileOversight::read_publication(root.store(), &profile()), Err(JournalError::Contract(Error::Binding)));
    std::fs::write(path, original).unwrap();
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
}
