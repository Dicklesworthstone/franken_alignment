//! Real journal byte exhaustion after admitted numerical computation.
#![cfg(unix)]
#[path = "support/file_decoder.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::Error;

#[test]
fn unrecordable_held_or_quiet_sample_cannot_leave_the_old_owner_eligible() {
    for threshold in [1.5, 3.0] {
        let root = Directory::new();
        // Paired positive control and exact size calibration at the SAME path.
        let (mut control, _, config) = create_decoder(&root, threshold);
        assert!(matches!(forced(&mut control, 0), MonitoredStep::Released(_)));
        let canonical = root.store().join("delivery.bin");
        let prefix_bytes = std::fs::read(&canonical).unwrap().len();
        let result = sampled(&mut control);
        assert_eq!(matches!(result, MonitoredSampledStep::Held(_)), threshold == 1.5);
        assert_eq!(control.decoder_inspection().unwrap().numerical.sampled_draws, 1);
        drop(control);
        // Only this test's disposable store is removed. The new independent
        // bootstrap has room for recovery records, but not another token image.
        std::fs::remove_dir_all(root.store()).unwrap();
        let mut limited = profile(); limited.delivery.limits.bytes = prefix_bytes + 256;
        let (mut host, _) = FileOversight::create(root.store(), limited.clone()).unwrap();
        host.enable_decoder(host.revision(), config.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        assert!(matches!(forced(&mut host, 0), MonitoredStep::Released(_)));
        let original = std::fs::read(&canonical).unwrap();
        assert_eq!(original.len(), prefix_bytes);
        let before = host.inspect(); let n = host.decoder_inspection().unwrap().numerical;
        let spec = spec(&host, b"old-prefix-must-not-authorize");
        assert!(matches!(host.advance_decoder_sampled(host.revision(), n.actor_revision,
            n.position, sample_budget()), Err(JournalError::Contract(Error::Limit))));
        assert_eq!(host.inspect(), before); // historical, not current eligibility
        assert!(!host.clock_ready());
        assert!(!host.storage_failure().unwrap().replacement_may_be_visible);
        assert_eq!(host.decoder_inspection(), Err(JournalError::Unavailable));
        assert_eq!(host.propose(host.revision(), 1, spec, snapshot()), Err(JournalError::Unavailable));
        assert!(matches!(host.advance_decoder_sampled(host.revision(), n.actor_revision,
            n.position, sample_budget()), Err(JournalError::Unavailable)));
        assert_eq!(std::fs::read(&canonical).unwrap(), original);
        assert!(!root.store().join("delivery.pending").exists());
        drop(host);
        let (recovered, _) = FileOversight::open_with_decoder(root.store(), limited, &config).unwrap();
        assert!(!recovered.clock_ready());
        assert!(recovered.decoder_inspection().unwrap().paused);
        assert_eq!(recovered.decoder_inspection().unwrap().numerical, n);
        assert!(recovered.storage_failure().is_none());
    }
}

#[test]
fn unconfigured_unclocked_and_stale_journal_refusals_do_not_retire_a_usable_owner() {
    let root = Directory::new();
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    assert!(matches!(host.advance_decoder_forced(host.revision(), 0, 0, 0, budget()),
        Err(JournalError::Contract(Error::Incomplete))));
    assert!(host.storage_failure().is_none());
    host.enable_decoder(host.revision(), configuration(3.0)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), n.actor_revision, 0, 0, budget()),
        Err(JournalError::Contract(Error::Incomplete))));
    assert!(host.storage_failure().is_none());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(matches!(host.advance_decoder_forced(0, n.actor_revision, 0, 0, budget()),
        Err(JournalError::Contract(Error::Stale))));
    assert!(host.storage_failure().is_none());
    assert!(matches!(forced(&mut host, 0), MonitoredStep::Released(_)));
    assert!(matches!(sampled(&mut host), MonitoredSampledStep::Released(_)));
    assert!(host.storage_failure().is_none());
}
