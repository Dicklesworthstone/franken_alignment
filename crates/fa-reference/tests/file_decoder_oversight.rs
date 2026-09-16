//! Original numerical computation, canonical persistence and original effect gate.
#![cfg(unix)]
#[path = "support/file_decoder.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::monitor::decoder::{MonitoredStep, MonitoringStatus};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderBudget;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, decoder::FileDecoderConfig};
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::Error;

#[test]
fn actual_monitored_continuation_and_two_key_publication_share_one_durable_owner() {
    let root = Directory::new(); let (mut host, human, _) = create_decoder(&root, 3.0);
    assert!(host.decoder_required());
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
    assert!(host.propose(host.revision(), 1, spec(&host, b"not-yet"), snapshot()).is_err());
    let MonitoredStep::Released(first) = forced(&mut host, 0) else { panic!("quiet prefix held"); };
    assert_eq!(first.step().token, 0);
    let MonitoredSampledStep::Released(next) = sampled(&mut host) else { panic!("quiet continuation held"); };
    assert_eq!(next.choice().token, 1); assert_eq!(next.choice().draw, 1);
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 2); assert_eq!(numerical.sampled_draws, 1);
    assert_eq!(numerical.status, MonitoringStatus::Ready);
    let keys = ready(&mut host, &human, 1, b"computed"); dispatch(&mut host, &keys);
    assert_eq!(host.publish(host.revision(), 1), Err(JournalError::Contract(Error::Incomplete)));
    let outcome = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap().outcome;
    assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn held_sample_commits_its_draw_and_cannot_be_rerolled_or_resumed() {
    let root = Directory::new(); let (mut host, _, config) = create_decoder(&root, 1.5);
    assert!(matches!(forced(&mut host, 0), MonitoredStep::Released(_)));
    assert!(matches!(sampled(&mut host), MonitoredSampledStep::Held(_)));
    let held = host.decoder_inspection().unwrap().numerical;
    assert_eq!(held.position, 2); assert_eq!(held.sampled_draws, 1); assert_eq!(held.status, MonitoringStatus::Held);
    assert!(host.propose(host.revision(), 1, spec(&host, b"not releasable"), snapshot()).is_err());
    assert!(matches!(host.advance_decoder_sampled(host.revision(), held.actor_revision, held.position, sample_budget()).unwrap(), Err(Error::WrongState)));
    assert_eq!(host.decoder_inspection().unwrap().numerical, held);
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), profile(), &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.resume_decoder(host.revision(), held.actor_revision, held.position), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(host.decoder_inspection().unwrap().numerical, held);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn recovered_quiet_prefix_needs_explicit_resume_and_matches_uninterrupted_next_draw() {
    let left_root = Directory::new(); let right_root = Directory::new();
    let (mut left, _, _) = create_decoder(&left_root, 3.0);
    let (mut right, _, config) = create_decoder(&right_root, 3.0);
    forced(&mut left, 0); forced(&mut right, 0);
    let before = right.decoder_inspection().unwrap().numerical;
    drop(right);
    let (mut right, _) = FileOversight::open_with_decoder(right_root.store(), profile(), &config).unwrap();
    assert!(right.decoder_inspection().unwrap().paused);
    assert_eq!(right.resume_decoder(right.revision(), before.actor_revision, before.position), Err(JournalError::Contract(Error::Incomplete)));
    right.observe_time(right.revision(), ElapsedTick(2)).unwrap();
    assert!(right.propose(right.revision(), 1, spec(&right, b"still paused"), snapshot()).is_err());
    assert!(right.advance_decoder_sampled(right.revision(), before.actor_revision, before.position, sample_budget()).is_err());
    assert_eq!(right.resume_decoder(right.revision(), before.actor_revision + 1, before.position), Err(JournalError::Contract(Error::Stale)));
    right.resume_decoder(right.revision(), before.actor_revision, before.position).unwrap();
    let MonitoredSampledStep::Released(a) = sampled(&mut left) else { panic!("left held"); };
    let MonitoredSampledStep::Released(b) = sampled(&mut right) else { panic!("right held"); };
    assert_eq!(a.choice(), b.choice());
    assert_eq!(left.actor_snapshot().unwrap(), right.actor_snapshot().unwrap());
    assert_eq!(left.decoder_inspection().unwrap().numerical, right.decoder_inspection().unwrap().numerical);
}

#[test]
fn even_structurally_valid_comparison_byte_tampering_fails_semantic_replay() {
    let root = Directory::new(); let (mut host, _, _) = create_decoder(&root, 3.0);
    forced(&mut host, 0);
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
    let path = root.store().join("delivery.bin");
    let original = std::fs::read(&path).unwrap(); let mut changed = original.clone();
    // The final length-delimited field is comparison material, not imported state.
    *changed.last_mut().unwrap() ^= 1; std::fs::write(&path, &changed).unwrap();
    assert!(matches!(FileOversight::read_publication(root.store(), &profile()), Err(JournalError::Contract(Error::Binding))));
    std::fs::write(&path, original).unwrap();
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn budget_and_predecessor_refusals_do_not_consume_a_token_or_draw() {
    let root = Directory::new(); let (mut host, _, _) = create_decoder(&root, 3.0);
    let before = host.decoder_inspection().unwrap().numerical;
    let revision = host.revision();
    assert!(matches!(host.advance_decoder_forced(revision, before.actor_revision, 0, 0,
        DecoderBudget { scalar_products: 0 }).unwrap(), Err(Error::Limit)));
    assert!(host.revision() > revision);
    assert_eq!(host.decoder_inspection().unwrap().numerical, before);
    forced(&mut host, 0);
    let current = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), before.actor_revision, 0, 0, budget()).unwrap(), Err(Error::Stale)));
    assert_eq!(host.decoder_inspection().unwrap().numerical, current);
    assert!(matches!(sampled(&mut host), MonitoredSampledStep::Released(_)));
}

#[test]
fn independently_pinned_input_bytes_are_checked_before_recovery_writes() {
    let root = Directory::new(); let (mut host, _, config) = create_decoder(&root, 3.0);
    forced(&mut host, 0); drop(host);
    let path = root.store().join("delivery.bin"); let before = std::fs::read(&path).unwrap();
    let mut monitor = data::monitor(3.0); monitor.push(b' ');
    let different = FileDecoderConfig::new(numerical_profile(), data::weights(), monitor,
        data::sampling(), 5, DecoderBindingLimits::default()).unwrap();
    assert_ne!(config, different);
    assert!(matches!(FileOversight::open_with_decoder(root.store(), profile(), &different), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let (host, _) = FileOversight::open_with_decoder(root.store(), profile(), &config).unwrap();
    assert!(host.decoder_inspection().unwrap().paused);
}
