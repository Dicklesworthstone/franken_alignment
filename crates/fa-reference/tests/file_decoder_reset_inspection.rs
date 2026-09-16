//! Historical reset evidence never becomes new runtime authority.
#![cfg(unix)]
#[path = "support/file_decoder.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::activation::{HEADER_BYTES, monitor::decoder::MonitoringStatus};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    containment::FileResetRequest, decoder::FileDecoderConfig};
use fa_reference::action::consequence::gate::ReviewBinding;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::Error;

fn request(host: &FileOversight, operation: u64) -> FileResetRequest {
    let c = host.inspect().control;
    FileResetRequest { operation, expected_control_sequence: c.sequence,
        expected_actor_revision: host.decoder_inspection().unwrap().numerical.actor_revision,
        expected_authority_epoch: c.ledger.epoch,
        binding: ReviewBinding { round: 5000 + operation, evidence_root: [17; 32], reducer_generation: 1 },
        retained_targets: vec![host.inspect().target] }
}

#[test]
fn read_only_inspection_preserves_live_owner_and_distinguishes_reset_from_later_continuation() {
    let root = Directory::new(); let (mut host, _, config) = create_decoder(&root, 3.0);
    forced(&mut host, 0);
    let n = host.decoder_inspection().unwrap().numerical;
    let cp = host.capture_decoder_checkpoint(host.revision(), 7, n.actor_revision, 0).unwrap();
    sampled(&mut host);
    let receipt = host.reset_decoder_checkpoint(host.revision(), &cp, request(&host, 1), budget()).unwrap().unwrap();
    sampled(&mut host);
    let before = host.inspect(); let bytes = std::fs::read(root.store().join("delivery.bin")).unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"preserve staged evidence").unwrap();
    let image = FileOversight::read_decoder_reset(root.store(), &profile(), &config, 1).unwrap();
    assert_eq!(image.result.unwrap(), receipt); assert_eq!(receipt.position, 1);
    assert_eq!(image.numerical.numerical.position, 2);
    assert_eq!(image.publication, before); assert_eq!(image.recovery, host.decoder_recovery_usage().unwrap());
    assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), bytes);
    assert_eq!(std::fs::read(root.store().join("delivery.pending")).unwrap(), b"preserve staged evidence");
    assert_eq!(host.inspect(), before);
    std::fs::remove_file(root.store().join("delivery.pending")).unwrap();
    // Reading beside a locked owner did not append a fence or withdraw its run.
    assert!(matches!(sampled(&mut host), fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep::Released(_)));
}

#[test]
fn independent_configuration_and_the_complete_suffix_are_required_for_a_reset_read() {
    let root = Directory::new(); let (mut host, _, config) = create_decoder(&root, 3.0);
    forced(&mut host, 0); let n = host.decoder_inspection().unwrap().numerical;
    let cp = host.capture_decoder_checkpoint(host.revision(), 7, n.actor_revision, 0).unwrap();
    host.reset_decoder_checkpoint(host.revision(), &cp, request(&host, 1), budget()).unwrap().unwrap();
    assert_eq!(FileOversight::read_decoder_reset(root.store(), &profile(), &configuration(4.0), 1), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(FileOversight::read_decoder_reset(root.store(), &profile(), &config, 99), Err(JournalError::Contract(Error::Missing)));
    let path = root.store().join("delivery.bin"); let bytes = std::fs::read(&path).unwrap();
    let mut corrupt = bytes.clone(); corrupt.push(0); std::fs::write(&path, corrupt).unwrap();
    assert!(FileOversight::read_decoder_reset(root.store(), &profile(), &config, 1).is_err());
    std::fs::write(path, bytes).unwrap();
    assert!(FileOversight::read_decoder_reset(root.store(), &profile(), &config, 1).unwrap().result.is_ok());
}

#[test]
fn failed_re_review_is_durable_and_does_not_refund_admitted_replay_work() {
    let root = Directory::new();
    // Exactly one full-precision two-coordinate frame fits. A new checkpoint
    // replay must use the remaining lifetime allowance, not the saved allowance.
    let one_frame = HEADER_BYTES + 2 * 4;
    let monitor = String::from_utf8(data::monitor(3.0)).unwrap()
        .replace("\"encoded_bytes\":10000", &format!("\"encoded_bytes\":{one_frame}"));
    let config = FileDecoderConfig::new(numerical_profile(), data::weights(), monitor.into_bytes(),
        data::sampling(), 5, DecoderBindingLimits::default()).unwrap();
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_decoder(host.revision(), config.clone()).unwrap(); host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    forced(&mut host, 0); let n = host.decoder_inspection().unwrap().numerical;
    let cp = host.capture_decoder_checkpoint(host.revision(), 7, n.actor_revision, 0).unwrap();
    let original = request(&host, 1);
    let result = host.reset_decoder_checkpoint(host.revision(), &cp, original.clone(), budget()).unwrap();
    assert!(result.is_err());
    let usage = host.decoder_recovery_usage().unwrap(); assert_eq!(usage.replay_attempts, 1); assert!(usage.admitted_products > 0);
    let after = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(after.status, MonitoringStatus::Failed(_)));
    assert!(after.numerical.tokens > n.numerical.tokens);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 0); // No successful control reset.
    let revision = host.revision();
    assert_eq!(host.reset_decoder_checkpoint(0, &cp, original, budget()).unwrap(), result);
    assert_eq!(host.revision(), revision); assert_eq!(host.decoder_recovery_usage().unwrap(), usage);
    assert_eq!(FileOversight::read_decoder_reset(root.store(), &profile(), &config, 1).unwrap().result, result);
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), profile(), &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    assert_eq!(host.decoder_reset_result(1).unwrap(), result);
    assert!(host.resume_decoder(host.revision(), n.actor_revision, n.position).is_err());
}

#[test]
fn incident_threshold_suspension_survives_recovery_and_cannot_be_reset_away() {
    let root = Directory::new(); let (mut host, _, config) = create_decoder(&root, 3.0);
    forced(&mut host, 0); let n = host.decoder_inspection().unwrap().numerical;
    let cp = host.capture_decoder_checkpoint(host.revision(), 7, n.actor_revision, 0).unwrap();
    let threshold = profile().delivery.suspend_at_incident;
    for operation in 1..=threshold {
        let result = host.reset_decoder_checkpoint(host.revision(), &cp, request(&host, operation), budget()).unwrap().unwrap();
        assert_eq!(result.control.incident_count, operation);
        assert_eq!(result.control.restored, operation < threshold);
        if operation == threshold {
            assert_eq!(result.control.consequence, Consequence::SuspendRun);
            assert_eq!(result.resumed_stream, None);
        }
    }
    assert!(host.inspect().control.suspended);
    assert_eq!(host.reset_decoder_checkpoint(host.revision(), &cp, request(&host, threshold + 1), budget()).unwrap(), Err(Error::WrongState));
    drop(host);
    let (mut host, _) = FileOversight::open_with_decoder(root.store(), profile(), &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(host.inspect().control.suspended);
    assert_eq!(host.resume_decoder(host.revision(), n.actor_revision, n.position), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(FileOversight::read_decoder_reset(root.store(), &profile(), &config, threshold).unwrap().result.unwrap().control.consequence, Consequence::SuspendRun);
}
