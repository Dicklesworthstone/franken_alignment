//! Actual model computation -> original durable identity -> two-key publication.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
#[path = "support/decoder_identity.rs"] mod numerical;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::{FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::identity::{ModelManifest, ModelPassport};
use fa_reference::action::consequence::activation::identity::decoder::DecoderIdentityProbe;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::delivery::{EndpointOutcome, persistent::JournalError};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::containment::FileStateUpdate;
use fa_reference::action::consequence::delivery::persistent::observed::identity::{FileIdentityChallenge, FileIdentityObserver};
use fa_reference::action::consequence::delivery::persistent::observed::identity::decoder::{
    FileDecoderIdentityEvent, FileDecoderIdentityProbe, FileDecoderIdentityProgress,
};
use fa_reference::action::consequence::oversight::identity::{IdentityOutcome, IdentityPolicy, IdentityStatus};
use fa_reference::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn policy() -> IdentityPolicy { IdentityPolicy { observer_id: 91, timeout_ticks: 10, validity_ticks: 50, max_checks: 8 } }
fn configured(root: &Directory) -> (FileOversight, FileHumanReviewer, FileIdentityObserver) {
    let (mut host, human) = create(root);
    let observer = host.enable_identity_checks(host.revision(), numerical::passport(), policy()).unwrap();
    (host, human, observer)
}
fn challenge(host: &mut FileOversight, id: u64) -> FileIdentityChallenge {
    let control = host.inspect().control;
    let actor = host.actor_snapshot().unwrap().actor_revision;
    host.begin_identity_check(host.revision(), id, control.sequence, actor).unwrap().unwrap()
}
fn start(host: &mut FileOversight, observer: &FileIdentityObserver, id: u64,
    sequence: u64, changed: bool, overflow: bool) -> FileDecoderIdentityProbe {
    let check = challenge(host, id);
    let probe = DecoderIdentityProbe::new(numerical::model(changed, overflow), check.evidence().passport(), sequence, budget()).unwrap();
    observer.decoder_probe(host, &check, probe, numerical::manifest()).unwrap()
}
fn finish(host: &mut FileOversight, observer: &FileIdentityObserver, run: &mut FileDecoderIdentityProbe)
    -> FileDecoderIdentityProgress {
    for _ in 0..8 {
        let now = host.inspect().control.ledger.elapsed.unwrap();
        let progress = run.step_with_clock(host, observer, || now).unwrap();
        if run.is_closed() { return progress; }
    }
    panic!("bounded fixture did not terminate")
}

#[test]
fn actual_anchor_computation_installs_identity_then_original_congress_and_two_keys_publish() {
    let root = Directory::new(); let (mut host, human, observer) = configured(&root);
    let actor = host.actor_snapshot().unwrap();
    let mut run = start(&mut host, &observer, 1, 1, false, false);
    for _ in 0..6 {
        run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap();
        assert!(host.identity_installation(1).unwrap().is_none());
    }
    assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Matched);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    let progress = run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap();
    assert!(matches!(progress.event, FileDecoderIdentityEvent::Installed(_)));
    assert_eq!(progress.work.completed_tokens, 5);
    assert_eq!(progress.work.completed_scalar_products, 382);
    assert_eq!(host.actor_snapshot().unwrap(), actor);
    assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Matching { check: 1, .. }));
    let keys = ready(&mut host, &human, 1, b"identity ok");
    dispatch(&mut host, &keys);
    let result = host.publish_checked(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn same_metadata_actual_parameter_mutation_causes_original_durable_containment() {
    let root = Directory::new(); let (mut host, _, observer) = configured(&root);
    let mut run = start(&mut host, &observer, 1, 1, true, false);
    let result = finish(&mut host, &observer, &mut run);
    let FileDecoderIdentityEvent::Measured { anchor, observation } = result.event else { panic!("expected numerical mismatch"); };
    assert_eq!(anchor, 20);
    assert!(matches!(observation.measurement.unwrap().outcome, IdentityOutcome::Mismatch(_)));
    assert!(observation.containment.unwrap().is_ok());
    assert_eq!(result.work.completed_tokens, 5);
    assert!(host.inspect().control.suspended);
    assert_eq!(host.inspect().executions, 0);
    assert!(matches!(host.identity_status().unwrap(), IdentityStatus::Mismatch { check: 1 }));
    assert!(run.step_with_clock(&mut host, &observer, || panic!("closed runner read clock")).is_err());
}

#[test]
fn manifest_mismatch_is_committed_before_any_decoder_work() {
    let root = Directory::new(); let (mut host, _, observer) = configured(&root);
    let check = challenge(&mut host, 1);
    let probe = DecoderIdentityProbe::new(numerical::model(false, false), &numerical::passport(), 1, budget()).unwrap();
    let mut manifest: ModelManifest = numerical::manifest(); manifest.weights[0] ^= 1;
    let mut run = observer.decoder_probe(&host, &check, probe, manifest).unwrap();
    let result = run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap();
    let FileDecoderIdentityEvent::Manifest(observation) = result.event else { panic!("manifest was not recorded"); };
    assert!(matches!(observation.measurement.unwrap().outcome, IdentityOutcome::Mismatch(_)));
    assert!(host.inspect().control.suspended);
    assert!(run.is_closed()); assert_eq!(run.work().entered_tokens, 0);
}

#[test]
fn clock_after_computation_withdraws_late_work_without_extending_the_challenge() {
    for (expired, prior_tokens) in [(false, 0), (true, 0), (true, 1)] {
        let root = Directory::new(); let (mut host, _, observer) = configured(&root);
        let mut run = start(&mut host, &observer, 1, 1, false, false);
        run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap();
        for _ in 0..prior_tokens { run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap(); }
        let deadline = run.challenge().evidence().deadline();
        let mut reads = 0;
        let progress = run.step_with_clock(&mut host, &observer, || {
            reads += 1; if reads == 2 && expired { deadline } else { ElapsedTick(1) }
        }).unwrap();
        assert_eq!(reads, 2); assert_eq!(progress.work.completed_tokens, prior_tokens + 1);
        if expired {
            assert!(matches!(progress.event, FileDecoderIdentityEvent::Withdrawn { reason: Error::Stale, withdrawal: Ok(_) }));
            assert!(run.is_closed());
            let report = host.identity_report(1).unwrap();
            assert_eq!(report.outcome, IdentityOutcome::Unavailable);
            assert!(report.observations.is_empty()); // even a completed late frame is not admitted
            assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
        } else {
            assert!(matches!(progress.event, FileDecoderIdentityEvent::Advanced));
            assert!(matches!(finish(&mut host, &observer, &mut run).event, FileDecoderIdentityEvent::Installed(_)));
        }
    }
}

#[test]
fn completed_measurements_use_the_installation_validity_window_not_a_renewed_collection_window() {
    for expired in [false, true] {
        let root = Directory::new(); let (mut host, _, observer) = configured(&root);
        let mut run = start(&mut host, &observer, 1, 1, false, false);
        for _ in 0..6 { run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap(); }
        let now = if expired { run.challenge().evidence().valid_until() } else { run.challenge().evidence().deadline() };
        let result = run.step_with_clock(&mut host, &observer, || now).unwrap();
        assert!(run.is_closed()); assert_eq!(result.work.completed_tokens, 5);
        if expired {
            assert!(matches!(result.event, FileDecoderIdentityEvent::Withdrawn { reason: Error::Stale, withdrawal: Ok(_) }));
        } else {
            assert!(matches!(result.event, FileDecoderIdentityEvent::Installed(_)));
        }
    }
}

#[test]
fn numerical_failure_cannot_resurrect_the_preceding_matching_identity() {
    let root = Directory::new(); let (mut host, _, observer) = configured(&root);
    let mut old = start(&mut host, &observer, 1, 1, false, false);
    assert!(matches!(finish(&mut host, &observer, &mut old).event, FileDecoderIdentityEvent::Installed(_)));
    let mut run = start(&mut host, &observer, 2, 2, false, true);
    let result = finish(&mut host, &observer, &mut run);
    assert!(matches!(result.event, FileDecoderIdentityEvent::Withdrawn { reason: Error::Overflow, withdrawal: Ok(_) }));
    assert_eq!(result.work.entered_tokens, 1); assert_eq!(result.work.completed_tokens, 0);
    assert_eq!(result.work.entered_scalar_product_bound, 70);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert!(host.identity_installation(1).unwrap().is_some()); // historical, not restored
    assert!(host.identity_installation(2).unwrap().is_none());
}

#[test]
fn caught_post_computation_unwind_retires_runner_without_repeating_work() {
    let root = Directory::new(); let (mut host, _, observer) = configured(&root);
    let mut run = start(&mut host, &observer, 1, 1, false, false);
    run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap();
    let mut reads = 0;
    assert!(catch_unwind(AssertUnwindSafe(|| run.step_with_clock(&mut host, &observer, || {
        reads += 1; assert!(reads != 2, "injected post-computation unwind"); ElapsedTick(1)
    }))).is_err());
    assert!(run.is_closed()); assert_eq!(run.work().completed_tokens, 1);
    let work = run.work();
    assert!(run.step_with_clock(&mut host, &observer, || panic!("must not reenter")).is_err());
    assert_eq!(run.work(), work); assert!(host.identity_installation(1).unwrap().is_none());
}

#[test]
fn recovery_refuses_old_runner_and_retains_measurement_sequence_floors() {
    let root = Directory::new(); let (mut host, _, observer) = configured(&root);
    let mut old = start(&mut host, &observer, 1, 1, false, false);
    for _ in 0..3 { old.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap(); }
    assert_eq!(host.identity_report(1).unwrap().observations.len(), 1);
    drop(host);
    let (mut host, _, fresh) = FileOversight::open_with_identity_observer(root.store(), profile(), &numerical::passport(), policy()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(matches!(old.step_with_clock(&mut host, &fresh, || panic!("foreign owner clock")), Err(JournalError::Contract(Error::Binding))));
    let mut stale = start(&mut host, &fresh, 2, 1, false, false);
    let result = finish(&mut host, &fresh, &mut stale);
    assert!(matches!(result.event, FileDecoderIdentityEvent::Measured { observation, .. }
        if observation.measurement == Err(Error::Stale)));
    assert!(host.identity_installation(2).unwrap().is_none());
    host.identity_unavailable(host.revision(), host.identity_basis().unwrap()).unwrap();
    let mut current = start(&mut host, &fresh, 3, 2, false, false);
    assert!(matches!(finish(&mut host, &fresh, &mut current).event, FileDecoderIdentityEvent::Installed(_)));
}

#[test]
fn precomputed_or_foreign_passports_and_manual_measurement_mixing_cannot_skip_computation() {
    let root = Directory::new(); let (mut host, _, observer) = configured(&root);
    let check = challenge(&mut host, 1);
    let mut computed = DecoderIdentityProbe::new(numerical::model(false, false), &numerical::passport(), 1, budget()).unwrap();
    computed.advance().unwrap();
    let before = std::fs::read(root.store().join("delivery.bin")).unwrap();
    assert!(matches!(observer.decoder_probe(&host, &check, computed, numerical::manifest()), Err(JournalError::Contract(Error::WrongState))));
    let foreign = ModelPassport::new(2, 1, numerical::manifest(), numerical::passport().anchors().values().cloned().collect()).unwrap();
    let probe = DecoderIdentityProbe::new(numerical::model(false, false), &foreign, 1, budget()).unwrap();
    assert!(matches!(observer.decoder_probe(&host, &check, probe, numerical::manifest()), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), before);
    let probe = DecoderIdentityProbe::new(numerical::model(false, false), &numerical::passport(), 1, budget()).unwrap();
    let mut run = observer.decoder_probe(&host, &check, probe, numerical::manifest()).unwrap();
    run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap();
    let frame = SourceFrame::capture(FrameIdentity { profile: numerical::passport().anchors()[&10].profile(),
        stream: 21, sequence: 1, position: 1 }, &[0.0, 1.0]).unwrap();
    let revision = host.revision();
    observer.observe_anchor(&mut host, revision, &check, 10, &frame, ElapsedTick(1)).unwrap().measurement.unwrap();
    assert!(matches!(run.step_with_clock(&mut host, &observer, || panic!("mixed observations")), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(run.work().entered_tokens, 0); assert!(run.is_closed());
}

#[test]
fn changed_actor_revision_prevents_finishing_a_probe_for_the_old_actor_state() {
    let root = Directory::new(); let (mut host, _, observer) = configured(&root);
    let mut run = start(&mut host, &observer, 1, 1, false, false);
    run.step_with_clock(&mut host, &observer, || ElapsedTick(1)).unwrap();
    let actor = host.actor_snapshot().unwrap();
    host.record_actor_state(host.revision(), FileStateUpdate { operation: 1,
        expected_actor_revision: actor.actor_revision, expected_authority_epoch: host.inspect().control.ledger.epoch,
        state: actor.state }).unwrap();
    assert!(matches!(run.step_with_clock(&mut host, &observer, || panic!("stale actor")), Err(JournalError::Contract(Error::Stale))));
    assert!(run.is_closed()); assert_eq!(run.work().entered_tokens, 0);
}
