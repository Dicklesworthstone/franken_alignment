//! Canonical shutdown evidence is neither a writer recovery nor a guessed receipt.
#![cfg(unix)]
#[path = "support/file_shutdown.rs"] mod support;
#[path = "support/investigation_decoder.rs"] mod numerical;
#[allow(dead_code)]
#[path = "support/decoder_inputs.rs"] mod inputs;
use support::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::StopRequest;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::*;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::Error;

#[test]
fn faulted_owner_can_be_inspected_without_cleanup_refund_or_speculative_acknowledgment() {
    let a = Directory::new(); let b = Directory::new();
    let (mut left, human) = create(&a, 1); let (mut right, _) = create(&b, 2);
    let mut campaign = plan(vec![left.shutdown_domain(1).unwrap(), right.shutdown_domain(2).unwrap()]).start();
    let keys = ready(&mut left, &human, 1, 1, b"public bytes"); dispatch(&mut left, &keys);
    left.publish_checked(left.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let path = a.store().join("delivery.bin"); let bytes = std::fs::read(&path).unwrap();
    let pending = a.store().join("delivery.pending"); std::fs::write(&pending, b"retained unfinished write").unwrap();
    assert!(matches!(campaign.advance(1, &mut left, ElapsedTick(2)), Err(JournalError::Io(_))));
    assert!(left.storage_failure().is_some());
    campaign.advance(2, &mut right, ElapsedTick(2)).unwrap();
    campaign.advance(2, &mut right, ElapsedTick(2)).unwrap();
    let observed = campaign.inspect_canonical(1).unwrap();
    assert_eq!(observed.source, FileShutdownSource::CanonicalImage);
    assert!(observed.stop.is_none()); assert_eq!(observed.executions, 1);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::read(&pending).unwrap(), b"retained unfinished write");
    assert!(left.storage_failure().is_some());
    assert_eq!(left.inspect().control.ledger.charged, 16);
    assert_eq!(campaign.report().unobserved_stops(), vec![1]);
    drop(left);
    // Only explicit ORIGINAL recovery can clean staging and return an owner.
    let (mut recovered, _) = FileOversight::open(a.store(), profile(1)).unwrap();
    campaign.advance(1, &mut recovered, ElapsedTick(2)).unwrap();
    campaign.advance(1, &mut recovered, ElapsedTick(2)).unwrap();
    assert!(campaign.report().all_observed_drained());
    assert_eq!(recovered.inspect().executions, 1);
    assert_eq!(recovered.inspect().control.ledger.charged, 16);
}

#[test]
fn exact_observed_prefix_cannot_roll_back_to_an_older_valid_registration() {
    let root = Directory::new(); let (mut host, _) = create(&root, 1);
    let mut campaign = plan(vec![host.shutdown_domain(1).unwrap()]).start();
    let path = root.store().join("delivery.bin"); let older = std::fs::read(&path).unwrap();
    campaign.advance(1, &mut host, ElapsedTick(2)).unwrap();
    let stopped = std::fs::read(&path).unwrap();
    std::fs::write(&path, &older).unwrap(); // simulate operator-storage rollback
    assert_eq!(campaign.inspect_canonical(1), Err(JournalError::Contract(Error::Stale)));
    let report = campaign.report();
    assert!(report.domains[0].last_observation.as_ref().unwrap().stop.is_some());
    assert!(!report.domains[0].latest_succeeded); assert!(!report.all_observed_stopped());
    assert_eq!(std::fs::read(&path).unwrap(), older); // inspection did not "repair" storage
    std::fs::write(&path, &stopped).unwrap();
    campaign.inspect_canonical(1).unwrap();
    assert!(campaign.report().all_observed_stopped());
    assert!(!campaign.report().all_observed_drained());
}

#[test]
fn retained_plan_rebuilds_observations_without_importing_old_results_or_issuing_stops() {
    let a = Directory::new(); let b = Directory::new();
    let (mut left, _) = create(&a, 1); let (mut right, _) = create(&b, 2);
    let saved = plan(vec![left.shutdown_domain(1).unwrap(), right.shutdown_domain(2).unwrap()]);
    let mut prior = saved.start(); prior.advance(1, &mut left, ElapsedTick(2)).unwrap(); drop(prior);
    let before_left = left.inspect(); let before_right = right.inspect();
    let mut rebuilt = saved.start();
    assert_eq!(rebuilt.report().unobserved_stops(), vec![1, 2]);
    rebuilt.inspect_canonical(1).unwrap(); rebuilt.inspect_canonical(2).unwrap();
    assert_eq!(left.inspect(), before_left); assert_eq!(right.inspect(), before_right);
    assert_eq!(rebuilt.report().unobserved_stops(), vec![2]);
    rebuilt.advance(2, &mut right, ElapsedTick(2)).unwrap();
    rebuilt.advance(2, &mut right, ElapsedTick(2)).unwrap();
    rebuilt.advance(1, &mut left, ElapsedTick(2)).unwrap();
    assert!(rebuilt.report().all_observed_drained());
}

#[test]
fn another_operations_valid_stop_is_visible_but_cannot_complete_this_campaign() {
    let root = Directory::new(); let (mut host, _) = create(&root, 1);
    let mut campaign = plan(vec![host.shutdown_domain(1).unwrap()]).start();
    let cut = host.inspect();
    host.request_stop(host.revision(), StopRequest { operation: 999,
        expected_control_sequence: cut.control.sequence,
        expected_authority_epoch: cut.control.ledger.epoch }).unwrap();
    host.progress_stop(host.revision(), ElapsedTick(2)).unwrap();
    let revision = host.revision();
    let observed = campaign.inspect_canonical(1).unwrap();
    assert_eq!(observed.stop.unwrap().receipt.request().operation, 999);
    assert_eq!(host.revision(), revision);
    assert_eq!(campaign.report().unobserved_stops(), vec![1]);
    assert!(!campaign.report().all_observed_drained());
}

#[test]
fn newly_supplied_numerical_configuration_is_refused_before_historical_execution() {
    let root = Directory::new(); let (mut host, _) = create(&root, 1);
    let mut early = plan(vec![host.shutdown_domain(1).unwrap()]).start();
    let config = FileDecoderConfig::new(numerical::profile(), numerical::weights(),
        inputs::monitor(100.0), inputs::sampling(), 5, DecoderBindingLimits::default()).unwrap();
    host.enable_decoder(host.revision(), config).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 0, numerical::budget()).unwrap().unwrap();
    let before = host.inspect(); let numbers = host.decoder_inspection().unwrap();
    let pending = root.store().join("delivery.pending"); std::fs::write(&pending, b"keep").unwrap();
    assert_eq!(early.inspect_canonical(1), Err(JournalError::Contract(Error::Binding)));
    let mut pinned = plan(vec![host.shutdown_domain(1).unwrap()]).start();
    assert!(pinned.inspect_canonical(1).unwrap().stop.is_none());
    assert_eq!(host.inspect(), before); assert_eq!(host.decoder_inspection().unwrap(), numbers);
    assert_eq!(std::fs::read(&pending).unwrap(), b"keep");
}

#[test]
fn original_actor_ticket_stays_unknown_after_stop_until_the_native_drain_reconciles() {
    use fa_reference::action::consequence::oversight::actor::{ActorProposal, ActorOutcome, Knowledge, UnknownReason};
    let root = Directory::new(); let (mut host, human) = create(&root, 1);
    let saved = plan(vec![host.shutdown_domain(1).unwrap()]);
    let raw = spec(&host, 1, b"actor publish");
    let proposal = ActorProposal { target: raw.target.unwrap(), payload: raw.payload.clone(),
        expected_policy_epoch: raw.policy_epoch, deadline: raw.deadline, units: raw.units };
    host.submit_request(host.revision(), 9000, raw, snapshot()).unwrap();
    let action = host.request_action(9000).unwrap().clone();
    let keys = ready_existing(&mut host, &human, 1, action);
    dispatch(&mut host, &keys);
    host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let ticket = actor.submit(9000, &proposal).unwrap(); // exact retry, no fresh snapshot
    assert_eq!(actor.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    let mut campaign = saved.start();
    campaign.advance_supervised(1, &mut supervisor, ElapsedTick(2)).unwrap();
    assert_eq!(actor.poll(&ticket), Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown });
    assert!(!campaign.report().all_observed_drained());
    campaign.advance_supervised(1, &mut supervisor, ElapsedTick(2)).unwrap();
    assert!(matches!(actor.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert!(campaign.report().all_observed_drained());
    assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 16);
}
