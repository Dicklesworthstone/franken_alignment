//! Recover the composed original owner, not isolated replacement authorities.
#![cfg(unix)]
#[path = "support/file_guarded.rs"] mod fixture;
use fixture::{identity, ordinary, *};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::credential_broker::{CredentialRevocationRequest, CredentialRotationRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::identity::IdentityStatus;
use fa_reference::action::consequence::oversight::policy_governance::CampaignDisposition;
use fa_reference::Error;

fn candidate() -> Policy {
    Policy::new(2, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::PayloadAtMost(64), Predicate::All(vec![0, 1])]).unwrap()
}
fn update(host: &FileOversight, operation: u64) -> PolicyUpdate {
    let state = host.inspect().control;
    PolicyUpdate::new(operation, state.sequence, state.ledger.epoch, candidate()).unwrap()
}

#[test]
fn one_reopen_recovers_both_roles_without_recovering_old_approvals() {
    let root = Directory::new(); let declared = guards();
    let (mut host, old_roles) = create(&root, &declared);
    let old_observer = old_roles.identity_observer.unwrap();
    let old_governor = old_roles.policy_governor.unwrap();
    identity::matched(&mut host, &old_observer, 1, 1);
    let old_keys = ordinary::ready(&mut host, &old_roles.human, 1, b"before recovery");
    let request = update(&host, 41);
    let campaign = host.request_policy_campaign(host.revision(), &request).unwrap();
    let revision = host.revision();
    let old_promotion = old_governor.approve(&mut host, revision, &campaign, false).unwrap();
    let expected = requirements(&host, declared.clone()); let before = host.revision(); drop(host);
    let (mut host, roles) = FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
    assert_eq!(host.revision(), before + 1);
    assert!(!host.clock_ready());
    assert_eq!(host.identity_status(), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.policy_campaign(41).unwrap().observed_disposition(), CampaignDisposition::Revoked);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert!(matches!(host.promote_policy_campaign(host.revision(), &old_promotion), Err(JournalError::Contract(Error::Binding))));
    assert!(host.dispatch(host.revision(), &old_keys.automatic, &old_keys.human,
        &old_keys.action, &old_keys.inputs, ordinary::snapshot()).is_err());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert!(host.propose(host.revision(), 2, ordinary::spec(&host, b"new"), ordinary::snapshot()).is_err());
    let observer = roles.identity_observer.unwrap(); let governor = roles.policy_governor.unwrap();
    let check = identity::begin(&mut host, 2); let revision = host.revision();
    assert!(matches!(old_observer.observe_manifest(&mut host, revision, &check, identity::manifest(), ElapsedTick(2)),
        Err(JournalError::Contract(Error::Binding))));
    identity::measure(&mut host, &observer, &check, 2); identity::install(&mut host, &check);
    ordinary::reviewed(&mut host, 2, b"current corpus");
    let request = update(&host, 42);
    assert!(host.replace_policy(host.revision(), &request).is_err());
    let campaign = host.request_policy_campaign(host.revision(), &request).unwrap(); let revision = host.revision();
    assert!(matches!(old_governor.approve(&mut host, revision, &campaign, false), Err(JournalError::Contract(Error::Binding))));
    let promotion = governor.approve(&mut host, revision, &campaign, false).unwrap();
    host.promote_policy_campaign(host.revision(), &promotion).unwrap();
    // Policy revocation changed the identity basis. A fresh governor did not
    // grant a new identity or effect key as a side effect of promotion.
    identity::matched(&mut host, &observer, 3, 3);
    let keys = ordinary::ready(&mut host, &roles.human, 3, b"after recovery"); let revision = host.revision();
    assert!(matches!(old_roles.human.approve(&mut host, revision, &keys.request), Err(JournalError::Contract(Error::Binding))));
    ordinary::dispatch(&mut host, &keys);
    assert_eq!(host.publish_checked(host.revision(), 3, Some(&keys.inputs), ordinary::snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 3).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
}

#[test]
fn every_guard_is_exact_and_mismatches_preserve_canonical_and_pending_bytes() {
    let root = Directory::new(); let declared = all_guards(); let (host, _) = create(&root, &declared);
    let expected = requirements(&host, declared.clone()); drop(host);
    let canonical = root.store().join("delivery.bin"); let pending = root.store().join("delivery.pending");
    let bytes = std::fs::read(&canonical).unwrap(); std::fs::write(&pending, b"unacknowledged").unwrap();
    for variant in 0..12 {
        let mut wrong = expected.clone();
        match variant {
            0 => wrong.guards.identity = None,
            1 => wrong.guards.identity.as_mut().unwrap().policy.validity_ticks += 1,
            2 => wrong.guards.campaigns = None,
            3 => wrong.guards.campaigns.as_mut().unwrap().max_campaigns += 1,
            4 => wrong.guards.campaigns.as_mut().unwrap().limits.input_bytes -= 1,
            5 => wrong.guards.source = None,
            6 => wrong.guards.source.as_mut().unwrap().source.generation += 1,
            7 => wrong.guards.source.as_mut().unwrap().limits.events -= 1,
            8 => { wrong.guards.credential = None; wrong.credential_epoch = None; }
            9 => wrong.guards.credential.as_mut().unwrap().credential.push_str("-other"),
            10 => wrong.guards.stream = None,
            _ => wrong.guards.stream = Some(StreamProfile::new(7, 1, 3, 1024, 4096).unwrap()),
        }
        assert!(FileOversight::open_guarded(root.store(), profile_for(&declared), &wrong).is_err(), "variant {variant}");
        assert_eq!(std::fs::read(&canonical).unwrap(), bytes);
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged");
    }
    let (host, roles) = FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
    assert!(!pending.exists()); assert!(roles.identity_observer.is_some()); assert!(roles.policy_governor.is_some());
    assert!(host.file_source_required()); assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
}

#[test]
fn every_external_history_floor_is_checked_before_adding_a_fence() {
    let root = Directory::new(); let declared = guards(); let (mut host, roles) = create(&root, &declared);
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 1, 1);
    ordinary::ready(&mut host, &roles.human, 1, b"known cut");
    let expected = requirements(&host, declared.clone()); drop(host);
    let canonical = root.store().join("delivery.bin"); let bytes = std::fs::read(&canonical).unwrap();
    for field in 0..3 {
        let mut wrong = expected.clone();
        match field {
            0 => wrong.minimum.journal_revision += 1,
            1 => wrong.minimum.control_sequence += 1,
            _ => wrong.minimum.authority_epoch += 1,
        }
        assert!(matches!(FileOversight::open_guarded(root.store(), profile_for(&declared), &wrong),
            Err(JournalError::Contract(Error::Stale))));
        assert_eq!(std::fs::read(&canonical).unwrap(), bytes);
    }
    let (host, _) = FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
    assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
}

#[test]
fn credential_revocation_and_effective_policy_cannot_be_replaced_with_older_expectations() {
    let root = Directory::new(); let mut declared = guards(); declared.credential = Some(credential_policy());
    let (mut host, _) = create(&root, &declared); let old_key = credential(&host);
    host.rotate_credential_guard(host.revision(), CredentialRotationRequest { operation: 1, expected_generation: 1, next_generation: 2 }).unwrap();
    host.revoke_credential_guard(host.revision(), CredentialRevocationRequest { operation: 2, expected_generation: 2 }).unwrap();
    let expected = requirements(&host, declared.clone()); drop(host);
    let path = root.store().join("delivery.bin"); let bytes = std::fs::read(&path).unwrap();
    for variant in 0..4 {
        let mut wrong = expected.clone();
        match variant {
            0 => wrong.credential_epoch.as_mut().unwrap().generation = 1,
            1 => wrong.credential_epoch.as_mut().unwrap().revoked = false,
            2 => wrong.credential_epoch = None,
            _ => wrong.effective_policy = Policy::new(1, vec![Predicate::PayloadAtMost(127)]).unwrap(),
        }
        assert!(FileOversight::open_guarded(root.store(), profile_for(&declared), &wrong).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    let (mut host, _) = FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
    let status = host.credential_status().unwrap().unwrap(); assert_eq!(status.generation, 2); assert!(status.revoked);
    assert!(host.publish_checked_with_credential(host.revision(), 999, None, ordinary::snapshot(), ElapsedTick(2), &old_key).is_err());
}

#[test]
fn recovered_roles_cannot_erase_an_unresolved_dispatch_charge() {
    let root = Directory::new(); let declared = guards(); let (mut host, roles) = create(&root, &declared);
    identity::matched(&mut host, roles.identity_observer.as_ref().unwrap(), 1, 1);
    let keys = ordinary::ready(&mut host, &roles.human, 1, b"not published"); ordinary::dispatch(&mut host, &keys);
    let expected = requirements(&host, declared.clone()); drop(host);
    let (mut host, roles) = FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
    assert!(roles.identity_observer.is_some() && roles.policy_governor.is_some());
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 0);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(host.seal_unexecuted(host.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
    assert_eq!(host.inspect().control.ledger.charged, 0); assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn an_unguarded_legacy_journal_is_not_silently_upgraded_or_modified() {
    let root = Directory::new(); let (host, _) = ordinary::create(&root);
    let declared = FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None, identity: None, campaigns: None, credential: None };
    let expected = requirements(&host, declared); drop(host);
    let path = root.store().join("delivery.bin"); let bytes = std::fs::read(&path).unwrap();
    assert!(matches!(FileOversight::open_guarded(root.store(), ordinary::profile(), &expected),
        Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    // The explicit legacy entry point retains its original behavior.
    FileOversight::open(root.store(), ordinary::profile()).unwrap();
}

#[test]
fn a_valid_guard_prefix_cannot_hide_a_corrupt_suffix_or_wrong_bootstrap() {
    let root = Directory::new(); let declared = guards(); let (host, _) = create(&root, &declared);
    let expected = requirements(&host, declared.clone()); drop(host);
    let path = root.store().join("delivery.bin"); let bytes = std::fs::read(&path).unwrap();
    let mut wrong_profile = profile_for(&declared); wrong_profile.human.reviewer_id += 1;
    assert!(FileOversight::open_guarded(root.store(), wrong_profile, &expected).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    let mut corrupt = bytes.clone(); corrupt.push(0); std::fs::write(&path, &corrupt).unwrap();
    assert!(FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), corrupt);
    std::fs::write(&path, bytes).unwrap();
    FileOversight::open_guarded(root.store(), profile_for(&declared), &expected).unwrap();
}
