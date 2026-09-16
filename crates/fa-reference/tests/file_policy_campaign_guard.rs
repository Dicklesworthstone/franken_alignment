//! Campaign governance through the original durable policy and effect owners.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation, governance::PolicyUpdate};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight,
    governance::campaigns::FilePolicyGovernor};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use fa_reference::action::consequence::oversight::policy_governance::CampaignDisposition;
use fa_reference::action::consequence::policy_campaign::{ReplayCaseId, ReplayLimits};
use fa_reference::Error;

fn limits() -> ReplayLimits { ReplayLimits { cases: 16, input_bytes: 1_048_576 } }
fn enable(host: &mut FileOversight) -> FilePolicyGovernor {
    host.enable_policy_campaigns(host.revision(), limits(), 8).unwrap()
}
fn same_policy(generation: u64) -> Policy {
    Policy::new(generation, profile().delivery.policy.nodes().to_vec()).unwrap()
}
fn update(host: &FileOversight, id: u64, candidate: Policy) -> PolicyUpdate {
    let control = host.inspect().control;
    PolicyUpdate::new(id, control.sequence, control.ledger.epoch, candidate).unwrap()
}

#[test]
fn guarded_promotion_uses_original_cancellations_and_new_work_still_needs_both_effect_keys() {
    let root = Directory::new(); let (mut host, human) = create(&root);
    let governor = enable(&mut host);
    let old = ready(&mut host, &human, 1, b"long payload");
    let change = update(&host, 10, Policy::new(2, vec![Predicate::PayloadAtMost(3)]).unwrap());
    let before = host.inspect();
    assert_eq!(host.replace_policy(host.revision(), &change), Err(JournalError::Contract(Error::Incomplete)));
    assert!(host.replay_candidate_policy(change.policy().clone(), limits()).is_ok());
    assert_eq!(host.replace_policy(host.revision(), &change), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.inspect(), before);
    let review = host.request_policy_campaign(host.revision(), &change).unwrap();
    assert_eq!(review.report().newly_blocked().len(), 2);
    let revision = host.revision();
    let key = governor.approve(&mut host, revision, &review, false).unwrap();
    let receipt = host.promote_policy_campaign(host.revision(), &key).unwrap();
    assert_eq!(receipt.update(), &change);
    assert_eq!(host.current_policy().unwrap(), change.policy());
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.human_status(old.human.request()).unwrap().disposition, HumanDisposition::Revoked);
    assert!(host.dispatch(host.revision(), &old.automatic, &old.human, &old.action, &old.inputs, snapshot()).is_err());
    let next = ready(&mut host, &human, 2, b"ok");
    dispatch(&mut host, &next);
    assert_eq!(host.publish(host.revision(), 2).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    assert!(host.policy_campaigns_required());
    assert_eq!(host.policy_campaign(10).unwrap().observed_disposition(), CampaignDisposition::Promoted);
}

#[test]
fn exact_denials_are_in_the_relaxation_set_and_require_explicit_governor_acceptance() {
    let root = Directory::new(); let (mut host, _) = create(&root); let governor = enable(&mut host);
    let mut denied = snapshot(); denied.values.insert(7, b"bad".to_vec());
    host.propose(host.revision(), 1, spec(&host, b"denied"), denied).unwrap();
    let change = update(&host, 10, Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap());
    let review = host.request_policy_campaign(host.revision(), &change).unwrap();
    assert_eq!(review.report().newly_reviewable(), vec![ReplayCaseId::Proposal(1)]);
    let revision = host.revision();
    assert_eq!(governor.approve(&mut host, revision, &review, false).err(), Some(JournalError::Contract(Error::Binding)));
    assert_eq!(host.revision(), revision);
    assert_eq!(host.policy_campaign(10).unwrap().observed_disposition(), CampaignDisposition::Pending);
    let key = governor.approve(&mut host, revision, &review, true).unwrap();
    host.promote_policy_campaign(host.revision(), &key).unwrap();
    // Policy promotion does not resurrect the formerly denied attempt.
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Denied);
}

#[test]
fn a_relaxation_flag_cannot_approve_missing_shadow_observations() {
    let root = Directory::new(); let (mut host, _) = create(&root); let governor = enable(&mut host);
    reviewed(&mut host, 1, b"reviewed");
    let change = update(&host, 10, Policy::new(2, vec![Predicate::Absent { key: 99 }]).unwrap());
    let review = host.request_policy_campaign(host.revision(), &change).unwrap();
    assert!(review.report().requires_shadow());
    let revision = host.revision();
    assert_eq!(governor.approve(&mut host, revision, &review, true).err(), Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), revision);
    assert_eq!(host.current_policy().unwrap().generation(), 1);
}

#[test]
fn a_new_denied_proposal_stales_approval_even_without_a_new_control_sequence() {
    let root = Directory::new(); let (mut host, _) = create(&root); let governor = enable(&mut host);
    reviewed(&mut host, 1, b"reviewed");
    let change = update(&host, 10, same_policy(2));
    let review = host.request_policy_campaign(host.revision(), &change).unwrap();
    let revision = host.revision(); let key = governor.approve(&mut host, revision, &review, false).unwrap();
    let sequence = host.inspect().control.sequence;
    let mut denied = snapshot(); denied.values.insert(7, b"bad".to_vec());
    host.propose(host.revision(), 2, spec(&host, b"new denial"), denied).unwrap();
    assert_eq!(host.inspect().control.sequence, sequence);
    let before = host.inspect();
    assert_eq!(host.promote_policy_campaign(host.revision(), &key), Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect(), before);
    let revised = update(&host, 11, same_policy(2));
    let current = host.request_policy_campaign(host.revision(), &revised).unwrap();
    assert_eq!(current.report().cases().len(), review.report().cases().len() + 1);
    let revision = host.revision(); let fresh = governor.approve(&mut host, revision, &current, false).unwrap();
    host.promote_policy_campaign(host.revision(), &fresh).unwrap();
}

#[test]
fn exact_retries_are_historical_and_never_duplicate_promotion_or_recover_an_approval_key() {
    let root = Directory::new(); let (mut host, _) = create(&root); let governor = enable(&mut host);
    reviewed(&mut host, 1, b"reviewed");
    let change = update(&host, 10, same_policy(2));
    let review = host.request_policy_campaign(host.revision(), &change).unwrap();
    let revision = host.revision();
    assert_eq!(host.request_policy_campaign(0, &change).unwrap().report(), review.report());
    assert_eq!(host.revision(), revision);
    let conflict = update(&host, 10, same_policy(3));
    assert_eq!(host.request_policy_campaign(revision, &conflict).err(), Some(JournalError::Contract(Error::Binding)));
    let key = governor.approve(&mut host, revision, &review, false).unwrap();
    let revision = host.revision();
    assert_eq!(governor.approve(&mut host, revision, &review, false).err(), Some(JournalError::Contract(Error::WrongState)));
    let receipt = host.promote_policy_campaign(host.revision(), &key).unwrap();
    let revision = host.revision();
    assert_eq!(host.promote_policy_campaign(0, &key).unwrap(), receipt);
    assert_eq!(host.replace_policy(0, &change).unwrap(), receipt);
    assert_eq!(host.revision(), revision);
}

#[test]
fn recovered_roles_cannot_resurrect_pending_or_approved_campaigns() {
    let root = Directory::new(); let (mut host, _) = create(&root); let governor = enable(&mut host);
    reviewed(&mut host, 1, b"reviewed");
    let change = update(&host, 10, same_policy(2));
    let approved = host.request_policy_campaign(host.revision(), &change).unwrap();
    let revision = host.revision(); let key = governor.approve(&mut host, revision, &approved, false).unwrap();
    let other = update(&host, 11, same_policy(3));
    host.request_policy_campaign(host.revision(), &other).unwrap();
    drop(host);
    let (mut host, _, fresh_governor) = FileOversight::open_with_policy_governor(root.store(), profile()).unwrap();
    assert!(host.policy_campaigns_required());
    for id in [10, 11] { assert_eq!(host.policy_campaign(id).unwrap().observed_disposition(), CampaignDisposition::Revoked); }
    assert_eq!(host.promote_policy_campaign(host.revision(), &key), Err(JournalError::Contract(Error::Binding)));
    let revision = host.revision();
    assert_eq!(governor.approve(&mut host, revision, &approved, false).err(), Some(JournalError::Contract(Error::Binding)));
    let old = host.policy_campaign(10).unwrap();
    assert_eq!(fresh_governor.approve(&mut host, revision, &old, false).err(), Some(JournalError::Contract(Error::WrongState)));
    let next = update(&host, 12, same_policy(2));
    let next_review = host.request_policy_campaign(host.revision(), &next).unwrap();
    let revision = host.revision(); let fresh = fresh_governor.approve(&mut host, revision, &next_review, false).unwrap();
    host.promote_policy_campaign(host.revision(), &fresh).unwrap();
    assert_eq!(host.current_policy().unwrap().generation(), 2);
}

#[test]
fn promotion_retains_unresolved_effect_charges_until_original_endpoint_reconciliation() {
    let root = Directory::new(); let (mut host, human) = create(&root); let governor = enable(&mut host);
    let keys = ready(&mut host, &human, 1, b"admitted"); dispatch(&mut host, &keys);
    let change = update(&host, 10, same_policy(2));
    let review = host.request_policy_campaign(host.revision(), &change).unwrap();
    let revision = host.revision(); let key = governor.approve(&mut host, revision, &review, false).unwrap();
    host.promote_policy_campaign(host.revision(), &key).unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::AwaitingResolution);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.seal_unexecuted(host.revision(), 1).unwrap(), Reconciliation::Resolved(
        EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn rejection_revocation_and_foreign_governors_cannot_promote() {
    let root = Directory::new(); let (mut host, _) = create(&root); let governor = enable(&mut host);
    reviewed(&mut host, 1, b"reviewed");
    let change = update(&host, 10, same_policy(2));
    let review = host.request_policy_campaign(host.revision(), &change).unwrap();
    let other_root = Directory::new(); let (mut other, _) = create(&other_root); let foreign = enable(&mut other);
    let revision = host.revision();
    assert_eq!(foreign.approve(&mut host, revision, &review, false).err(), Some(JournalError::Contract(Error::Binding)));
    governor.reject(&mut host, revision, &review).unwrap();
    let revision = host.revision();
    assert_eq!(governor.approve(&mut host, revision, &review, false).err(), Some(JournalError::Contract(Error::WrongState)));
    let change = update(&host, 11, same_policy(3));
    let review = host.request_policy_campaign(host.revision(), &change).unwrap();
    let revision = host.revision(); let key = governor.approve(&mut host, revision, &review, false).unwrap();
    let revision = host.revision(); governor.revoke(&mut host, revision, &review).unwrap();
    assert_eq!(host.promote_policy_campaign(host.revision(), &key), Err(JournalError::Contract(Error::Missing)));
    assert_eq!(host.current_policy().unwrap().generation(), 1);
}

#[test]
fn bootstrap_and_campaign_capacity_fail_without_a_late_mode_change() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    assert_eq!(host.enable_policy_campaigns(host.revision(), limits(), 0).err(), Some(JournalError::Contract(Error::InvalidInput)));
    assert!(!host.policy_campaigns_required());
    host.enable_policy_campaigns(host.revision(), limits(), 1).unwrap();
    reviewed(&mut host, 1, b"reviewed");
    let first = update(&host, 10, same_policy(2)); host.request_policy_campaign(host.revision(), &first).unwrap();
    let second = update(&host, 11, same_policy(3));
    let revision = host.revision();
    assert_eq!(host.request_policy_campaign(revision, &second).err(), Some(JournalError::Contract(Error::Limit)));
    assert_eq!(host.revision(), revision);
    let legacy_root = Directory::new(); let (mut legacy, _) = create(&legacy_root);
    reviewed(&mut legacy, 1, b"legacy");
    assert_eq!(legacy.enable_policy_campaigns(legacy.revision(), limits(), 8).err(), Some(JournalError::Contract(Error::WrongState)));
    assert!(!legacy.policy_campaigns_required());
    legacy.observe_time(legacy.revision(), ElapsedTick(2)).unwrap();
}
