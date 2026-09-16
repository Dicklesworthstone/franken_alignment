//! Combined role recovery also pins and pauses the original numerical owner.
#![cfg(unix)]
#[path = "support/file_guarded.rs"] mod fixture;
#[path = "support/file_decoder.rs"] mod numerical;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::identity::IdentityStatus;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::Error;

#[test]
fn combined_recovery_preserves_numerical_state_but_requires_explicit_resume_and_new_identity() {
    let root = Directory::new(); let mut declared = guards();
    declared.decoder = Some(numerical::configuration(100.0));
    let (mut host, roles) = create(&root, &declared);
    assert!(matches!(numerical::forced(&mut host, 0), MonitoredStep::Released(_)));
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 1, 1);
    let before = host.decoder_inspection().unwrap().numerical;
    let expected = requirements(&host, declared.clone()); drop(host);
    let (mut host, roles) = FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
    assert_eq!(host.decoder_inspection().unwrap().numerical, before);
    assert!(host.decoder_inspection().unwrap().paused);
    assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
    assert!(host.resume_decoder(host.revision(), before.actor_revision, before.position).is_err());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert!(matches!(host.advance_decoder_forced(host.revision(), before.actor_revision,
        before.position, 0, numerical::budget()), Err(JournalError::Contract(Error::Incomplete))));
    host.resume_decoder(host.revision(), before.actor_revision, before.position).unwrap();
    assert!(matches!(numerical::sampled(&mut host), MonitoredSampledStep::Released(_)));
    let continued = host.decoder_inspection().unwrap().numerical;
    assert_eq!(continued.position, before.position + 1);
    assert_eq!(continued.sampled_draws, before.sampled_draws + 1);
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 2, 2);
    let keys = ordinary::ready(&mut host, &roles.human, 1, b"resumed");
    ordinary::dispatch(&mut host, &keys);
    let outcome = host.publish_checked(host.revision(), 1, Some(&keys.inputs), ordinary::snapshot(), ElapsedTick(2)).unwrap().outcome;
    assert_eq!(outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(outcome));
}

#[test]
fn unexpected_decoder_or_changed_monitor_is_refused_without_cleaning_or_writing() {
    let root = Directory::new(); let mut declared = guards();
    declared.decoder = Some(numerical::configuration(100.0));
    let (mut host, _) = create(&root, &declared);
    numerical::forced(&mut host, 0);
    let expected = requirements(&host, declared.clone()); drop(host);
    let canonical = root.store().join("delivery.bin");
    let pending = root.store().join("delivery.pending");
    let original = std::fs::read(&canonical).unwrap();
    std::fs::write(&pending, b"unacknowledged").unwrap();
    for decoder in [None, Some(numerical::configuration(99.0))] {
        let mut wrong = expected.clone(); wrong.guards.decoder = decoder;
        assert!(matches!(FileOversight::open_guarded(root.store(), profile_for(&declared), &wrong),
            Err(JournalError::Contract(Error::Binding))));
        assert_eq!(std::fs::read(&canonical).unwrap(), original);
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged");
    }
    let (host, _) = FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
    assert!(!pending.exists()); assert_eq!(host.decoder_inspection().unwrap().numerical.position, 1);
    assert!(host.decoder_inspection().unwrap().paused);
    // Requiring a decoder on a domain that never owned one is also a mismatch.
    let plain = Directory::new(); let declared = guards(); let (host, _) = create(&plain, &declared);
    let mut expected = requirements(&host, declared.clone()); drop(host);
    expected.guards.decoder = Some(numerical::configuration(100.0));
    assert!(matches!(FileOversight::open_guarded(plain.store(), profile_for(&declared), &expected),
        Err(JournalError::Contract(Error::Binding))));
}
