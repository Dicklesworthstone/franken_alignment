//! All configured guards precede the first externally visible canonical image.
#![cfg(unix)]
#[path = "support/file_guarded.rs"] mod fixture;
use fixture::{identity, ordinary, *};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::identity::ModelPassport;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::gate::containment::session::policy::Policy;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::identity::IdentityStatus;
use fa_reference::Error;

fn source(root: &Directory, profile: &FileOversightProfile) -> FileEvidenceSource {
    let observation = EvidenceSnapshot::new(EvidenceIdentity { source: 17, generation: 1, scope: profile.delivery.scope },
        ordinary::snapshot(), ordinary::MEMBERS.into_iter().map(|member|
            (member.to_owned(), format!("registered context for {member}").into_bytes())).collect()).unwrap();
    let path = root.0.join("evidence.json"); std::fs::write(&path, observation.encode()).unwrap();
    FileEvidenceSource::new(path, 17, profile.delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap()
}
fn ready_message(host: &mut FileOversight, roles: &FileOversightRoles, profile: &FileOversightProfile,
    observation: &EvidenceSnapshot, id: u64, message: &str) -> ordinary::Keys
{
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let spec = host.stream_message_spec(message, ElapsedTick(now.0 + 100)).unwrap();
    let action = host.propose(host.revision(), id, spec, observation.snapshot().clone()).unwrap();
    let inputs = observation.inputs_for(&action, &profile.committee).unwrap();
    ordinary::review_existing(host, id, id + 100, &inputs);
    let automatic = host.authorize(host.revision(), id, &inputs, observation.snapshot().clone()).unwrap();
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(now.0 + 10)).unwrap();
    let revision = host.revision();
    let human = roles.human.approve(host, revision, &request).unwrap();
    ordinary::Keys { action, inputs, automatic, human, request }
}
fn publish(host: &mut FileOversight, keys: &ordinary::Keys, id: u64, resulting_version: u64) {
    ordinary::dispatch(host, keys);
    let pair = credential(host); let now = host.inspect().control.ledger.elapsed.unwrap();
    let result = host.publish_checked_with_credential(host.revision(), id, Some(&keys.inputs), ordinary::snapshot(), now, &pair).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version });
    assert_eq!(host.stream_snapshot().unwrap().pending, Some(id));
    assert_eq!(host.reconcile(host.revision(), id).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version }));
}
fn promote(host: &mut FileOversight, roles: &FileOversightRoles, operation: u64, generation: u64) {
    let control = host.inspect().control;
    let next = Policy::new(generation, host.current_policy().unwrap().nodes().to_vec()).unwrap();
    let update = PolicyUpdate::new(operation, control.sequence, control.ledger.epoch, next).unwrap();
    let campaign = host.request_policy_campaign(host.revision(), &update).unwrap();
    let revision = host.revision();
    let key = roles.policy_governor.as_ref().unwrap().approve(host, revision, &campaign, false).unwrap();
    host.promote_policy_campaign(host.revision(), &key).unwrap();
}

#[test]
fn all_guard_bootstrap_and_recovery_preserve_real_source_identity_governance_and_message_publication() {
    let root = Directory::new(); let declared = all_guards(); let p = profile_for(&declared);
    let perimeter = inventory(); let route = binding();
    let (mut host, roles) = FileOversight::create_guarded(root.store(), p.clone(), &declared,
        Some(FileCredentialRegistration { inventory: &perimeter, binding: &route })).unwrap();
    assert_eq!(host.revision(), 5); // Stream/publication, source, identity, campaigns, credential.
    assert!(!host.clock_ready()); assert!(host.file_source_required());
    assert!(host.identity_checks_required()); assert!(host.policy_campaigns_required());
    assert!(host.file_source_status().unwrap().capture.closed.is_none());
    assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
    let mut reader = source(&root, &p);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let spec = host.stream_message_spec("first", ElapsedTick(100)).unwrap();
    assert!(host.propose(host.revision(), 1, spec, ordinary::snapshot()).is_err());
    let observation = host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(1)).unwrap();
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 1, 1);
    let keys = ready_message(&mut host, &roles, &p, &observation, 1, "first");
    publish(&mut host, &keys, 1, 2);
    promote(&mut host, &roles, 41, 2);
    let expected = requirements(&host, declared.clone()); drop(host);

    let (mut host, roles) = FileOversight::open_guarded(root.store(), p.clone(), &expected).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().published.messages().collect::<Vec<_>>(), vec!["first"]);
    assert!(host.file_source_status().unwrap().capture.closed.is_none());
    assert!(!host.clock_ready());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    let observation = host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(2)).unwrap();
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 2, 2);
    let keys = ready_message(&mut host, &roles, &p, &observation, 2, "second");
    publish(&mut host, &keys, 2, 3);
    // Both independently re-provisioned roles are useful on this SAME owner.
    promote(&mut host, &roles, 42, 3);
    assert_eq!(host.current_policy().unwrap().generation(), 3);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), vec!["first", "second"]);
    assert_eq!(host.inspect().executions, 2);
}

#[test]
fn invalid_native_guard_combinations_refuse_before_creating_the_store() {
    let perimeter = inventory(); let route = binding();
    for variant in 0..7 {
        let root = Directory::new(); let mut declared = all_guards(); let mut p = profile_for(&declared);
        match variant {
            0 => declared.source.as_mut().unwrap().source.scope.tenant += 1,
            1 => declared.campaigns.as_mut().unwrap().limits.cases = 0,
            2 => declared.identity.as_mut().unwrap().policy.max_checks = 0,
            3 => {
                let required = declared.identity.as_mut().unwrap(); let passport = &required.passport;
                let mut manifest = passport.manifest().clone(); manifest.tokenizer_generation += 1;
                required.passport = ModelPassport::new(passport.id(), passport.generation(), manifest,
                    passport.anchors().values().cloned().collect()).unwrap();
            }
            4 => declared.credential.as_mut().unwrap().credential.push_str("-not-registered"),
            5 => p.delivery.initial_payload = b"unreviewed prefix".to_vec(),
            _ => declared.campaigns.as_mut().unwrap().max_campaigns = 65,
        }
        assert!(FileOversight::create_guarded(root.store(), p, &declared,
            Some(FileCredentialRegistration { inventory: &perimeter, binding: &route })).is_err(), "variant {variant}");
        assert!(!root.store().exists(), "variant {variant} created storage");
    }
    let root = Directory::new(); let declared = all_guards();
    FileOversight::create_guarded(root.store(), profile_for(&declared), &declared,
        Some(FileCredentialRegistration { inventory: &perimeter, binding: &route })).unwrap();
}

#[test]
fn credential_policy_assertions_cannot_replace_original_perimeter_resolution() {
    let root = Directory::new(); let mut declared = guards(); declared.credential = Some(credential_policy());
    let perimeter = inventory(); let mut route = binding();
    assert!(matches!(FileOversight::create_guarded(root.store(), profile_for(&declared), &declared, None),
        Err(JournalError::Contract(Error::InvalidInput))));
    assert!(!root.store().exists());
    route.route.push_str("-unregistered");
    assert!(FileOversight::create_guarded(root.store(), profile_for(&declared), &declared,
        Some(FileCredentialRegistration { inventory: &perimeter, binding: &route })).is_err());
    assert!(!root.store().exists());
    let route = binding();
    let (host, _) = FileOversight::create_guarded(root.store(), profile_for(&declared), &declared,
        Some(FileCredentialRegistration { inventory: &perimeter, binding: &route })).unwrap();
    assert_eq!(host.credential_status().unwrap().unwrap().generation, 1);
    let extra = Directory::new(); let plain = guards();
    assert!(FileOversight::create_guarded(extra.store(), profile_for(&plain), &plain,
        Some(FileCredentialRegistration { inventory: &perimeter, binding: &route })).is_err());
    assert!(!extra.store().exists());
}

#[test]
fn creation_cannot_reset_an_existing_owner_or_rewrite_its_guard_contract() {
    let root = Directory::new(); let declared = guards();
    let (mut host, roles) = FileOversight::create_guarded(root.store(), profile_for(&declared), &declared, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 1, 1);
    let path = root.store().join("delivery.bin"); let bytes = std::fs::read(&path).unwrap(); let before = host.inspect();
    assert!(FileOversight::create_guarded(root.store(), profile_for(&declared), &declared, None).is_err());
    assert_eq!(host.inspect(), before); assert_eq!(std::fs::read(path).unwrap(), bytes);
    ordinary::ready(&mut host, &roles.human, 1, b"owner alive");
}

#[test]
fn black_box_guarded_bootstrap_still_requires_both_effect_keys_and_publication_revalidation() {
    let root = Directory::new();
    let declared = FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None, identity: None, campaigns: None, credential: None };
    let (mut host, roles) = FileOversight::create_guarded(root.store(), ordinary::profile(), &declared, None).unwrap();
    assert!(roles.identity_observer.is_none() && roles.policy_governor.is_none()); assert_eq!(host.revision(), 1);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let keys = ordinary::ready(&mut host, &roles.human, 1, b"black box"); ordinary::dispatch(&mut host, &keys);
    assert!(matches!(host.publish(host.revision(), 1), Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&keys.inputs), ordinary::snapshot(), ElapsedTick(1)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
}

#[path = "support/file_decoder.rs"] mod numerical;

#[test]
fn atomic_decoder_startup_composes_every_guard_through_restart_and_second_publication() {
    let root = Directory::new(); let mut declared = all_guards();
    declared.decoder = Some(numerical::configuration(100.0));
    let p = profile_for(&declared); let perimeter = inventory(); let route = binding();
    let (mut host, roles) = FileOversight::create_guarded(root.store(), p.clone(), &declared,
        Some(FileCredentialRegistration { inventory: &perimeter, binding: &route })).unwrap();
    assert_eq!(host.revision(), 6);
    assert!(host.decoder_required()); assert!(!host.clock_ready());
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
    let mut reader = source(&root, &p);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    numerical::forced(&mut host, 0);
    let observation = host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(1)).unwrap();
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 1, 1);
    let keys = ready_message(&mut host, &roles, &p, &observation, 1, "first");
    publish(&mut host, &keys, 1, 2);
    promote(&mut host, &roles, 41, 2);
    let numerical_before = host.decoder_inspection().unwrap().numerical;
    let expected = requirements(&host, declared.clone()); drop(host);

    let (mut host, roles) = FileOversight::open_guarded(root.store(), p.clone(), &expected).unwrap();
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical_before);
    assert!(host.decoder_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let observation = host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(2)).unwrap();
    host.resume_decoder(host.revision(), numerical_before.actor_revision, numerical_before.position).unwrap();
    numerical::sampled(&mut host);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, numerical_before.sampled_draws + 1);
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 2, 2);
    let keys = ready_message(&mut host, &roles, &p, &observation, 2, "second");
    publish(&mut host, &keys, 2, 3);
    promote(&mut host, &roles, 42, 3);
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), vec!["first", "second"]);
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(host.current_policy().unwrap().generation(), 3);
}

#[test]
fn recovered_roles_cannot_resume_a_numerically_held_run() {
    use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
    use fa_reference::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledStep;
    let root = Directory::new(); let mut declared = guards();
    declared.decoder = Some(numerical::configuration(1.5));
    let (mut host, _) = FileOversight::create_guarded(root.store(), profile_for(&declared), &declared, None).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(matches!(numerical::forced(&mut host, 0), MonitoredStep::Released(_)));
    assert!(matches!(numerical::sampled(&mut host), MonitoredSampledStep::Held(_)));
    let before = host.decoder_inspection().unwrap().numerical;
    let expected = requirements(&host, declared.clone()); drop(host);
    let (mut host, roles) = FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 1, 1);
    assert!(matches!(host.resume_decoder(host.revision(), before.actor_revision, before.position),
        Err(JournalError::Contract(Error::WrongState))));
    assert_eq!(host.decoder_inspection().unwrap().numerical, before);
    assert!(host.decoder_inspection().unwrap().paused);
    assert_eq!(host.inspect().executions, 0);
}

#[path = "support/guarded_auto_stop.rs"] mod auto_stop;
