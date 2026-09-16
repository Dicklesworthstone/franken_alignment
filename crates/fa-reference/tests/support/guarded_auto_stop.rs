//! Automatic numerical stopping in the complete guarded publication profile.
use super::*;
use fa_reference::action::consequence::activation::monitor::MonitorOutcome;
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
use fa_reference::action::consequence::oversight::decoder_host::{HostedStopCause, HostedStopPolicy};
use fa_reference::action::consequence::oversight::policy_governance::CampaignDisposition;

fn stop_policy() -> HostedStopPolicy { HostedStopPolicy::new(91, 1, 9001).unwrap() }
fn complete_guards(threshold: f32) -> FileGuardSet {
    let mut declared = all_guards();
    declared.decoder = Some(numerical::configuration(threshold));
    declared.decoder_stop = Some(stop_policy());
    declared
}

#[test]
fn atomic_stop_policy_preserves_permitted_numerical_publication_across_full_guard_recovery() {
    let root = Directory::new(); let declared = complete_guards(3.0);
    let p = profile_for(&declared); let perimeter = inventory(); let route = binding();
    let (mut host, roles) = FileOversight::create_guarded(root.store(), p.clone(), &declared,
        Some(FileCredentialRegistration { inventory: &perimeter, binding: &route })).unwrap();
    assert_eq!(host.revision(), 7);
    assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
    assert!(host.decoder_stop_incident().unwrap().is_none());
    assert!(!host.clock_ready());
    let mut reader = source(&root, &p);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(matches!(numerical::forced(&mut host, 0), MonitoredStep::Released(_)));
    let observation = host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(1)).unwrap();
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 1, 1);
    let keys = ready_message(&mut host, &roles, &p, &observation, 1, "permitted");
    publish(&mut host, &keys, 1, 2);
    let expected = requirements(&host, declared.clone()); drop(host);
    let (mut host, roles) = FileOversight::open_guarded(root.store(), p.clone(), &expected).unwrap();
    assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
    assert!(host.decoder_inspection().unwrap().paused);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let observation = host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    assert!(matches!(numerical::sampled(&mut host), MonitoredSampledStep::Released(_)));
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 2, 2);
    let keys = ready_message(&mut host, &roles, &p, &observation, 2, "continued");
    publish(&mut host, &keys, 2, 3);
    promote(&mut host, &roles, 41, 2);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), vec!["permitted", "continued"]);
    assert_eq!(host.inspect().executions, 2);
    assert!(host.decoder_stop_incident().unwrap().is_none());
}

#[test]
fn actual_alarm_withdraws_campaign_and_human_approval_without_erasing_disclosed_messages() {
    let root = Directory::new(); let declared = complete_guards(1.5);
    let p = profile_for(&declared); let perimeter = inventory(); let route = binding();
    let (mut host, roles) = FileOversight::create_guarded(root.store(), p.clone(), &declared,
        Some(FileCredentialRegistration { inventory: &perimeter, binding: &route })).unwrap();
    let mut reader = source(&root, &p);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    numerical::forced(&mut host, 0);
    let observation = host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(1)).unwrap();
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 1, 1);
    let first = ready_message(&mut host, &roles, &p, &observation, 1, "already disclosed");
    publish(&mut host, &first, 1, 2);
    let second = ready_message(&mut host, &roles, &p, &observation, 2, "not dispatched");
    let control = host.inspect().control;
    let update = PolicyUpdate::new(41, control.sequence, control.ledger.epoch,
        Policy::new(2, host.current_policy().unwrap().nodes().to_vec()).unwrap()).unwrap();
    let campaign = host.request_policy_campaign(host.revision(), &update).unwrap();
    let revision = host.revision();
    let approval = roles.policy_governor.as_ref().unwrap().approve(&mut host, revision, &campaign, false).unwrap();
    let charged = host.inspect().control.ledger.charged;
    assert!(matches!(numerical::sampled(&mut host), MonitoredSampledStep::Held(_)));
    let incident = host.decoder_stop_incident().unwrap().unwrap();
    assert_eq!(incident.cause(), HostedStopCause::Monitoring(MonitorOutcome::Alarm));
    assert_eq!(host.policy_campaign(41).unwrap().observed_disposition(), CampaignDisposition::Revoked);
    assert!(host.promote_policy_campaign(host.revision(), &approval).is_err());
    assert!(host.dispatch(host.revision(), &second.automatic, &second.human,
        &second.action, &second.inputs, ordinary::snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.charged, charged);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(stream.published.messages().collect::<Vec<_>>(), vec!["already disclosed"]);
    assert!(!stream.published.finished());
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert!(host.progress_stop(host.revision(), ElapsedTick(2)).unwrap().progress.drained());
    let expected = requirements(&host, declared); drop(host);
    let (mut host, _) = FileOversight::open_guarded(root.store(), p, &expected).unwrap();
    assert_eq!(host.decoder_stop_incident().unwrap(), Some(incident));
    host.observe_time(host.revision(), ElapsedTick(3)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    assert!(host.resume_decoder(host.revision(), n.actor_revision, n.position).is_err());
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, charged);
    assert_eq!(host.policy_campaign(41).unwrap().observed_disposition(), CampaignDisposition::Revoked);
}

#[test]
fn stop_policy_presence_and_every_field_are_pinned_before_cleanup_or_recovery() {
    let root = Directory::new(); let mut declared = guards();
    declared.decoder = Some(numerical::configuration(3.0)); declared.decoder_stop = Some(stop_policy());
    let (mut host, _) = FileOversight::create_guarded(root.store(), profile_for(&declared), &declared, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); numerical::forced(&mut host, 0);
    let expected = requirements(&host, declared.clone()); drop(host);
    let path = root.store().join("delivery.bin"); let pending = root.store().join("delivery.pending");
    let before = std::fs::read(&path).unwrap(); std::fs::write(&pending, b"unacknowledged evidence").unwrap();
    for policy in [None, Some(HostedStopPolicy::new(92, 1, 9001).unwrap()),
        Some(HostedStopPolicy::new(91, 2, 9001).unwrap()), Some(HostedStopPolicy::new(91, 1, 9002).unwrap())]
    {
        let mut wrong = expected.clone(); wrong.guards.decoder_stop = policy;
        assert!(matches!(FileOversight::open_guarded(root.store(), profile_for(&declared), &wrong),
            Err(JournalError::Contract(Error::Binding))));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged evidence");
    }
    let (host, _) = FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
    assert!(!pending.exists()); assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
    assert!(host.decoder_inspection().unwrap().paused);
    let plain = Directory::new(); declared.decoder_stop = None;
    let (host, _) = FileOversight::create_guarded(plain.store(), profile_for(&declared), &declared, None).unwrap();
    let mut expected = requirements(&host, declared.clone()); drop(host);
    expected.guards.decoder_stop = Some(stop_policy());
    assert!(matches!(FileOversight::open_guarded(plain.store(), profile_for(&declared), &expected),
        Err(JournalError::Contract(Error::Binding))));
}

#[test]
fn terminal_stop_without_an_owned_decoder_is_not_silently_dropped_at_bootstrap() {
    let root = Directory::new(); let mut declared = guards(); declared.decoder_stop = Some(stop_policy());
    assert!(matches!(FileOversight::create_guarded(root.store(), profile_for(&declared), &declared, None),
        Err(JournalError::Contract(Error::InvalidInput))));
    assert!(!root.store().exists());
    declared.decoder = Some(numerical::configuration(3.0));
    let (host, _) = FileOversight::create_guarded(root.store(), profile_for(&declared), &declared, None).unwrap();
    assert_eq!(host.decoder_stop_policy().unwrap(), Some(stop_policy()));
}
