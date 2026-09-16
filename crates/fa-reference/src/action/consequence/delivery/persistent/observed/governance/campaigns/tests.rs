//! Actual canonical replacements with deterministic storage-barrier faults.
//! These scenarios do not model hardware power loss or authenticate a governor.
use super::*;
use super::super::super::{BaseEvent, Machine, journal};
use crate::action::{ActionSpec, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::{StopRequest, persistent::{FileDeliveryProfile, JournalIo, JournalLimits}};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use crate::Snapshot;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let tick = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-campaign-{}-{tick}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir(&path).unwrap(); Self(path)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) { eprintln!("campaign cleanup: {error}"); }
    }
}
fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::PayloadAtMost(32)]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("helper".to_owned(),
                MemberPolicy { cohort: "one".to_owned(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
                continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("helper".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"campaign-view".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec()).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn limits() -> ReplayLimits { ReplayLimits { cases: 8, input_bytes: 1_048_576 } }
fn owner(root: &Directory) -> (FileOversight, FilePolicyGovernor) {
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    let governor = host.enable_policy_campaigns(host.revision(), limits(), 8).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.propose(host.revision(), 1, ActionSpec { version: VERSION, scope: profile().delivery.scope,
        target: Some(host.inspect().target), payload: b"case".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: 0, deadline: ElapsedTick(100), units: 8 },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() }).unwrap();
    (host, governor)
}
fn request(host: &mut FileOversight) -> FilePolicyCampaignReview {
    let control = host.inspect().control;
    let update = PolicyUpdate::new(9, control.sequence, control.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(16)]).unwrap()).unwrap();
    host.request_policy_campaign(host.revision(), &update).unwrap()
}
fn canonical(host: &FileOversight) -> Machine {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    Machine::replay(&host.profile, &events).unwrap()
}
fn failure(error: JournalError, stage: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("selected storage barrier did not fail"); };
    assert_eq!(failure.operation, stage);
    assert_eq!(failure.replacement_may_be_visible, matches!(stage, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn failed_approval_returns_no_key_and_reopen_revokes_even_visible_unacknowledged_approval() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, governor) = owner(&root);
        let review = request(&mut host); let before = host.inspect();
        host.store.fail_once(stage);
        let revision = host.revision();
        failure(governor.approve(&mut host, revision, &review, false).unwrap_err(), stage);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.policy_campaign(9).err(), Some(JournalError::Unavailable));
        assert_eq!(host.replay_candidate_policy(Policy::new(2, vec![Predicate::PayloadAtMost(16)]).unwrap(), limits()),
            Err(JournalError::Unavailable));
        let disk = canonical(&host);
        let expected = if stage == JournalIo::DirectorySync { CampaignDisposition::Approved } else { CampaignDisposition::Pending };
        assert_eq!(disk.broker.policy_campaign(9).unwrap().disposition(), expected);
        drop(host);
        let (mut reopened, _, fresh) = FileOversight::open_with_policy_governor(root.store(), profile()).unwrap();
        assert_eq!(reopened.policy_campaign(9).unwrap().observed_disposition(), CampaignDisposition::Revoked);
        let retained = reopened.policy_campaign(9).unwrap(); let revision = reopened.revision();
        assert_eq!(fresh.approve(&mut reopened, revision, &retained, false).err(), Some(JournalError::Contract(Error::WrongState)));
        assert_eq!(reopened.current_policy().unwrap().generation(), 1);
    }
}

#[test]
fn ambiguous_promotion_recovers_the_canonical_generation_without_repromoting() {
    for stage in BARRIERS {
        let root = Directory::new(); let (mut host, governor) = owner(&root);
        let review = request(&mut host); let revision = host.revision();
        let key = governor.approve(&mut host, revision, &review, false).unwrap();
        let before = host.inspect(); host.store.fail_once(stage);
        failure(host.promote_policy_campaign(host.revision(), &key).unwrap_err(), stage);
        assert_eq!(host.inspect(), before);
        assert_eq!(host.current_policy().err(), Some(JournalError::Unavailable));
        let disk = canonical(&host);
        let promoted = stage == JournalIo::DirectorySync;
        assert_eq!(disk.policy_updates.receipt(9).is_some(), promoted);
        drop(host);
        let (mut reopened, _, _) = FileOversight::open_with_policy_governor(root.store(), profile()).unwrap();
        assert_eq!(reopened.current_policy().unwrap().generation(), if promoted { 2 } else { 1 });
        assert_eq!(reopened.policy_campaign(9).unwrap().observed_disposition(),
            if promoted { CampaignDisposition::Promoted } else { CampaignDisposition::Revoked });
        assert_eq!(reopened.policy_update_receipt(9).is_ok(), promoted);
        let revision = reopened.revision();
        assert_eq!(reopened.promote_policy_campaign(revision, &key), Err(JournalError::Contract(Error::Binding)));
        assert_eq!(reopened.revision(), revision);
        assert_eq!(reopened.inspect().control.ledger.available, 100);
        assert_eq!(reopened.inspect().executions, 0);
    }
}

#[test]
fn journal_replay_cannot_import_a_promotion_without_approval_or_after_recovery() {
    let root = Directory::new(); let (mut host, governor) = owner(&root);
    let review = request(&mut host);
    let mut unapproved = host.events.clone();
    unapproved.push(Event::Campaign(CampaignEvent::Promote(9)));
    assert_eq!(Machine::replay(&host.profile, &unapproved).err(), Some(Error::Missing));
    let revision = host.revision(); let key = governor.approve(&mut host, revision, &review, false).unwrap();
    let mut recovered = host.events.clone(); recovered.push(Event::Core(BaseEvent::Fence));
    recovered.push(Event::Campaign(CampaignEvent::Promote(9)));
    assert_eq!(Machine::replay(&host.profile, &recovered).err(), Some(Error::Missing));
    // The unchanged legitimate path is the positive control for both refusals.
    host.promote_policy_campaign(host.revision(), &key).unwrap();
    assert_eq!(canonical(&host).broker.policy_campaign(9).unwrap().disposition(), CampaignDisposition::Promoted);
}

#[test]
fn stop_withdraws_policy_approvals_and_cannot_be_used_to_resume_through_promotion() {
    let root = Directory::new(); let (mut host, governor) = owner(&root);
    let review = request(&mut host); let revision = host.revision();
    let key = governor.approve(&mut host, revision, &review, false).unwrap();
    let state = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 33,
        expected_control_sequence: state.sequence, expected_authority_epoch: state.ledger.epoch }).unwrap();
    assert_eq!(host.policy_campaign(9).unwrap().observed_disposition(), CampaignDisposition::Revoked);
    assert_eq!(host.promote_policy_campaign(host.revision(), &key), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(host.current_policy().unwrap().generation(), 1);
    assert!(host.inspect().stop.is_some());
}

#[test]
fn campaign_codec_rejects_unknown_decisions_and_preserves_exact_update_identity() {
    use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
    let update = PolicyUpdate::new(9, 2, 3, Policy::new(4, vec![Predicate::PayloadAtMost(16)]).unwrap()).unwrap();
    for event in [CampaignEvent::Enable(limits(), 8), CampaignEvent::Request(update),
        CampaignEvent::Approve(9, false), CampaignEvent::Approve(9, true), CampaignEvent::Reject(9),
        CampaignEvent::Revoke(9), CampaignEvent::Promote(9)] {
        let mut writer = Writer::new(131072); write(&mut writer, &event).unwrap(); let bytes = writer.finish();
        let mut reader = Reader::new(&bytes); let decoded = read(&mut reader).unwrap(); reader.end().unwrap();
        let mut writer = Writer::new(131072); write(&mut writer, &decoded).unwrap(); assert_eq!(writer.finish(), bytes);
        for end in 0..bytes.len() { assert!(read(&mut Reader::new(&bytes[..end])).is_err()); }
    }
    assert!(read(&mut Reader::new(&[255])).is_err());
    let mut writer = Writer::new(16); write(&mut writer, &CampaignEvent::Approve(9, false)).unwrap();
    let mut bytes = writer.finish(); bytes[9] = 2;
    assert_eq!(read(&mut Reader::new(&bytes)).err(), Some(Error::InvalidInput));
}
