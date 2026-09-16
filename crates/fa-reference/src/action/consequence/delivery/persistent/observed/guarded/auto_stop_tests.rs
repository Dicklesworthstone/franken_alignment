//! Original Store barriers on composed stopping, not an alternate effect sink.
use super::*;
use super::super::bootstrap::PreparedGuardedBootstrap;
use crate::action::consequence::activation::{HEADER_BYTES, monitor::decoder::MonitoredStep};
use crate::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, DecoderIdentity,
    DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS};
use crate::action::consequence::activation::tensor::kv::decoder::sampling::{SampleBudget, SamplingBudget};
use crate::action::consequence::delivery::persistent::observed::containment::FileResetRequest;
use crate::action::consequence::gate::ReviewBinding;
use crate::action::consequence::oversight::decoder_host::HostedStopPolicy;
use crate::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
mod data { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/decoder_inputs.rs")); }

fn stop_policy() -> HostedStopPolicy { HostedStopPolicy::new(91, 1, 9001).unwrap() }
fn budget() -> DecoderBudget { DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS } }
fn sampling() -> SampleBudget { SampleBudget { decoder: budget(), sampling: SamplingBudget { vocabulary: 2 } } }
fn declared(limited: bool) -> FileGuardSet {
    let p = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9, model_generation: 1,
        tokenizer_generation: 1, profile_generation: 1 }, DecoderShape { vocabulary: 2,
        hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 4 },
        1e-5, 10000.0).unwrap();
    let monitor = if limited {
        String::from_utf8(data::monitor(3.0)).unwrap().replace("\"encoded_bytes\":10000",
            &format!("\"encoded_bytes\":{}", HEADER_BYTES + 8)).into_bytes()
    } else { data::monitor(1.5) };
    let mut g = guards(); g.identity = None; g.campaigns = None;
    g.decoder = Some(FileDecoderConfig::new(p, data::weights(), monitor, data::sampling(),
        5, DecoderBindingLimits::default()).unwrap());
    g.decoder_stop = Some(stop_policy()); g
}
fn owner(root: &Directory, g: &FileGuardSet) -> FileOversight {
    let (mut host, _) = FileOversight::create_guarded(root.store(), profile(), g, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, 0, 0, budget()).unwrap().unwrap(),
        MonitoredStep::Released(_)));
    host
}
fn requirements(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let c = host.inspect().control;
    FileRecoveryRequirements { guards, effective_policy: host.current_policy().unwrap().clone(),
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: c.sequence, authority_epoch: c.ledger.epoch } }
}
fn fault(error: JournalError, stage: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("expected actual Store fault"); };
    assert_eq!(failure.operation, stage);
    assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn bootstrap_and_recovery_barriers_never_expose_an_owner_missing_its_stop_policy() {
    for stage in BARRIERS {
        let root = Directory::new(); let g = declared(false);
        let prepared = PreparedGuardedBootstrap::prepare(profile(), &g, None).unwrap();
        let store = storage::Store::create(&root.store()).unwrap(); store.fail_once(stage);
        fault(prepared.publish(store).unwrap_err(), stage);
        if stage == JournalIo::DirectorySync {
            let expected = FileRecoveryRequirements { guards: g, effective_policy: profile().delivery.policy,
                credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: 3,
                    control_sequence: 0, authority_epoch: 0 } };
            let (host, _) = FileOversight::open_guarded(root.store(), profile(), &expected).unwrap();
            assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
            assert!(host.decoder_inspection().unwrap().paused); assert!(!host.clock_ready());
        } else {
            assert!(!root.store().join(storage::CANONICAL).exists());
            assert!(FileOversight::open(root.store(), profile()).is_err());
        }
        let root = Directory::new(); let g = declared(false); let host = owner(&root, &g);
        let expected = requirements(&host, g); let n = host.decoder_inspection().unwrap().numerical; drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(stage);
        fault(FileOversight::open_guarded_store(store, profile(), &expected).unwrap_err(), stage);
        let (mut host, _) = FileOversight::open_guarded(root.store(), profile(), &expected).unwrap();
        assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
        assert!(host.decoder_inspection().unwrap().paused);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
        assert!(matches!(host.advance_decoder_sampled(host.revision(), n.actor_revision,
            n.position, sampling()).unwrap().unwrap(), MonitoredSampledStep::Held(_)));
        assert!(host.decoder_stop_incident().unwrap().unwrap().stop_receipt().is_some());
        assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
    }
}

#[test]
fn trigger_and_failed_replay_barriers_preserve_only_the_actual_canonical_stop() {
    for replay in [false, true] {
        for stage in BARRIERS {
            let root = Directory::new(); let g = declared(replay); let mut host = owner(&root, &g);
            let n = host.decoder_inspection().unwrap().numerical;
            let cp = if replay {
                Some(host.capture_decoder_checkpoint(host.revision(), 7, n.actor_revision,
                    host.inspect().control.ledger.epoch).unwrap())
            } else { None };
            let c = host.inspect().control;
            let request = FileResetRequest { operation: 41, expected_control_sequence: c.sequence,
                expected_authority_epoch: c.ledger.epoch, expected_actor_revision: n.actor_revision,
                binding: ReviewBinding { round: 700, evidence_root: [17; 32], reducer_generation: 1 },
                retained_targets: vec![host.inspect().target] };
            let expected = requirements(&host, g);
            let before = host.inspect(); host.store.fail_once(stage);
            let error = match &cp {
                Some(cp) => host.reset_decoder_checkpoint(host.revision(), cp, request.clone(), budget()).unwrap_err(),
                None => host.advance_decoder_sampled(host.revision(), n.actor_revision, n.position, sampling()).unwrap_err(),
            };
            fault(error, stage);
            assert_eq!(host.inspect(), before);
            assert_eq!(host.decoder_stop_incident(), Err(JournalError::Unavailable));
            assert!(matches!(host.advance_decoder_sampled(host.revision(), n.actor_revision,
                n.position, sampling()), Err(JournalError::Unavailable)));
            let disk = FileOversight::read_publication(root.store(), &profile()).unwrap();
            let visible = stage == JournalIo::DirectorySync;
            assert_eq!(disk.stop.is_some(), visible);
            drop(host);
            let (mut host, _) = FileOversight::open_guarded(root.store(), profile(), &expected).unwrap();
            assert!(host.decoder_inspection().unwrap().paused);
            assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
            assert_eq!(host.decoder_stop_incident().unwrap().is_some(), visible);
            if visible {
                let before = host.inspect(); let usage = host.decoder_recovery_usage().unwrap();
                if replay {
                    let cp = host.decoder_checkpoint(7).unwrap();
                    assert!(host.reset_decoder_checkpoint(0, &cp, request, budget()).unwrap().is_err());
                    assert_eq!(host.inspect(), before);
                    assert_eq!(host.decoder_recovery_usage().unwrap(), usage);
                }
                host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
                let n = host.decoder_inspection().unwrap().numerical;
                assert!(host.resume_decoder(host.revision(), n.actor_revision, n.position).is_err());
                // Draining existing obligations requires no permitting source.
                host.source_interrupted = true;
                assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
                assert!(host.source_interrupted);
            } else {
                assert_eq!(host.decoder_inspection().unwrap().numerical, n);
                assert_eq!(host.decoder_recovery_usage().unwrap().replay_attempts, 0);
                host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
                host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
                assert!(matches!(host.advance_decoder_sampled(host.revision(), n.actor_revision,
                    n.position, sampling()).unwrap().unwrap(), MonitoredSampledStep::Held(_)));
                assert!(host.decoder_stop_incident().unwrap().is_some());
            }
        }
    }
}

#[test]
fn numerical_preflight_rejects_duplicate_or_missing_stop_policy_before_token_replay() {
    let root = Directory::new(); let g = declared(false); let host = owner(&root, &g);
    g.check_decoder_config(&host.events).unwrap();
    let mut duplicated = host.events.clone();
    duplicated.push(Event::Decoder(DecoderEvent::StopPolicy(stop_policy())));
    assert_eq!(g.check_decoder_config(&duplicated), Err(Error::Binding));
    let mut missing = host.events.clone();
    missing.retain(|event| !matches!(event, Event::Decoder(DecoderEvent::StopPolicy(_))));
    assert_eq!(g.check_decoder_config(&missing), Err(Error::Binding));
    let mut wrong = g.clone(); wrong.decoder_stop = None;
    assert_eq!(wrong.check_decoder_config(&host.events), Err(Error::Binding));
    assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 1);
}
