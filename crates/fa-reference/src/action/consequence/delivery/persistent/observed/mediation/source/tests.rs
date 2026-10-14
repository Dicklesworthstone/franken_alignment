//! Original canonical files, full-input review, both keys and endpoint outcomes.
//! Manual ballots are controls; they do not establish helper accuracy or OS containment.
use super::*;
use crate::action::{ActionSpec, ActionState, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, FilePermit, JournalLimits, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit,
    FileHumanReviewer, FileOversightProfile, Machine, journal};
use crate::action::consequence::delivery::persistent::observed::mediation::FileMediationSnapshot;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract,
    ReviewWindow, action_frame, human::HumanReviewPolicy};
use crate::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::reducer::Caps;
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::{collections::BTreeMap, sync::atomic::{AtomicU64, Ordering}};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-topology-source-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
    fn store(&self) -> PathBuf { self.0.join("publication") }
    fn bytes(&self) -> Vec<u8> { fs::read(self.store().join("delivery.bin")).unwrap() }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("topology source cleanup: {error}"); }
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
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: b"initial".to_vec(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 8 },
    }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) } }
fn canonical(host: &FileOversight) -> FileMediationSnapshot {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    Machine::replay(&host.profile, &events).unwrap().mediation_snapshot(events.len() as u64).unwrap()
}
fn spec(host: &FileOversight) -> ActionSpec {
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"visible".to_vec(), required_witnesses: Vec::new(),
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 16 }
}
struct Ready { action: FrozenAction, inputs: CommitteeInput, automatic: FilePermit, human: FileHumanPermit }
fn prepare(host: &mut FileOversight, reviewer: &FileHumanReviewer, attempt: u64) -> Ready {
    let action = host.propose(host.revision(), attempt, spec(host), snapshot()).unwrap();
    let contracts = profile().committee; let helper = &contracts.members()["reviewer"];
    let epoch = action.spec().policy_epoch;
    let mut bytes = action_frame(&action); let boundary = bytes.len();
    bytes.extend_from_slice(helper.question()); let end = bytes.len();
    let actual = ActualHelperInput::new(bytes, helper.profile_at(epoch), vec![
        SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: boundary } },
        SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: boundary, end } },
    ], Vec::new()).unwrap();
    let manifest = EvidenceViewManifest::new(actual, AuthorizationProjection {
        projection_id: 7, policy_epoch: epoch, projected_originals: Vec::new(),
    }, Vec::new()).unwrap();
    let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("reviewer".to_owned(), manifest)])).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
    let round = attempt + 100;
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot()).unwrap();
    host.commit_review(host.revision(), round, "reviewer", commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), attempt + 1000, attempt, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &request).unwrap();
    Ready { action, inputs, automatic, human }
}
fn dispatch(host: &mut FileOversight, ready: &Ready) {
    host.dispatch(host.revision(), &ready.automatic, &ready.human, &ready.action, &ready.inputs, snapshot()).unwrap();
}


const INVENTORY: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"));
fn document(generation: u64, checks: u64, read_bytes: u64, mode: u8, budget: usize) -> String {
    let extra = if mode == 1 {
        r#",{"id":3,"from":1,"to":4,"channel":"dispatch","route":"publication","provenance":103}"#
    } else { "" };
    let second = if mode == 2 { "" } else {
        r#",{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}"#
    };
    format!(r#"{{"schema":"fa.authority-topology/1","generation":{generation},"inventory_generation":{generation},
        "complete_inventory_generation":{generation},
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}}{second}{extra}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{budget},"source_limits":{{"checks":{checks},"read_bytes":{read_bytes}}}}}"#)
}
fn text(generation: u64) -> String {
    document(generation, 32, 32 * TOPOLOGY_REFRESH_READ_BYTES, 0,
        crate::action::consequence::mediation::MAX_CHECK_EDGE_VISITS)
}
fn loaded(document: &str) -> LoadedAuthorityTopology {
    LoadedAuthorityTopology::from_json_bytes(document.as_bytes(), INVENTORY.as_bytes()).unwrap()
}
fn fixture(root: &Directory, document: &str) -> (FileOversight, FileHumanReviewer, FileTopologySource) {
    let expected = loaded(document);
    fs::write(root.0.join("topology.json"), document).unwrap();
    fs::write(root.0.join("inventory.json"), INVENTORY).unwrap();
    let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let role = host.enable_mediation(host.revision(), expected.graph().clone()).unwrap();
    let source = role.bind_topology_source(&host, host.revision(), root.0.join("topology.json"),
        root.0.join("inventory.json"), expected).unwrap();
    (host, human, source)
}
fn refresh(source: &mut FileTopologySource, host: &mut FileOversight, operation: u64) -> TopologyRefresh {
    let revision = host.revision(); source.refresh(host, revision, operation).unwrap()
}
fn certified(report: &TopologyRefresh) {
    assert!(report.refusal.is_none()); assert!(report.withdrawal.is_none());
    assert!(matches!(&report.checked, Some(Ok(CutCheck::Verified(_)))));
}

#[test]
fn fresh_complete_source_certifies_real_two_key_publication_without_changing_control_cut() {
    let root = Directory::new(); let (mut host, human, mut source) = fixture(&root, &text(1));
    assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
    assert_eq!(source.work(), TopologySourceWork::default());
    certified(&refresh(&mut source, &mut host, 1));
    let ready = prepare(&mut host, &human, 1); let control = host.inspect().control.sequence;
    certified(&refresh(&mut source, &mut host, 2));
    assert_eq!(host.inspect().control.sequence, control);
    dispatch(&mut host, &ready);
    certified(&refresh(&mut source, &mut host, 3));
    let result = host.publish_checked(host.revision(), 1, Some(&ready.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(source.work(), TopologySourceWork { checks: 3, reserved_read_bytes: 3 * TOPOLOGY_REFRESH_READ_BYTES });
    assert_eq!(canonical(&host), host.mediation_snapshot().unwrap());
}

#[test]
fn missing_malformed_changed_and_symlinked_sources_withdraw_approved_work() {
    for mode in 0..6 {
        let root = Directory::new(); let (mut host, human, mut source) = fixture(&root, &text(1));
        certified(&refresh(&mut source, &mut host, 1)); let ready = prepare(&mut host, &human, 1);
        match mode {
            0 => fs::remove_file(root.0.join("topology.json")).unwrap(),
            1 => fs::write(root.0.join("inventory.json"), b"{}").unwrap(),
            2 => fs::write(root.0.join("topology.json"), text(2)).unwrap(),
            3 => {
                let inventory = INVENTORY.replace("reference-publication", "different-profile");
                let changed = LoadedAuthorityTopology::from_json_bytes(text(1).as_bytes(), inventory.as_bytes()).unwrap();
                assert_eq!(changed.graph(), source.expected().graph()); // metadata-only change
                assert_ne!(&changed, source.expected());
                fs::write(root.0.join("inventory.json"), inventory).unwrap();
            }
            4 => {
                fs::rename(root.0.join("topology.json"), root.0.join("other.json")).unwrap();
                std::os::unix::fs::symlink(root.0.join("other.json"), root.0.join("topology.json")).unwrap();
            }
            _ => fs::write(root.0.join("topology.json"), vec![b' '; MAX_TOPOLOGY_BYTES + 1]).unwrap(),
        }
        let result = refresh(&mut source, &mut host, 2);
        assert!(result.refusal.is_some()); assert!(result.checked.is_none());
        assert_eq!(result.withdrawal.as_ref().unwrap().cancelled, vec![1]);
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
        assert!(!host.mediation_snapshot().unwrap().available);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert!(host.dispatch(host.revision(), &ready.automatic, &ready.human,
            &ready.action, &ready.inputs, snapshot()).is_err());
        assert_eq!(host.inspect().executions, 0); assert_eq!(source.work().checks, 2);
        assert!(!canonical(&host).available);
    }
}

#[test]
fn source_loss_after_dispatch_keeps_charge_until_original_nonexecution_receipt() {
    let root = Directory::new(); let (mut host, human, mut source) = fixture(&root, &text(1));
    certified(&refresh(&mut source, &mut host, 1)); let ready = prepare(&mut host, &human, 1);
    dispatch(&mut host, &ready); let bound = host.delivery_mediation(1).unwrap().unwrap().clone();
    fs::remove_file(root.0.join("inventory.json")).unwrap();
    let loss = refresh(&mut source, &mut host, 2); assert!(loss.refusal.is_some());
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.delivery_mediation(1).unwrap(), Some(&bound));
    let result = host.publish_checked(host.revision(), 1, Some(&ready.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert!(matches!(result.outcome, EndpointOutcome::NotExecuted { .. }));
    assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(), Reconciliation::Resolved(result.outcome));
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn old_files_cannot_resurrect_coverage_but_explicit_newer_binding_allows_fresh_publication() {
    let root = Directory::new(); let (mut host, human, mut source) = fixture(&root, &text(1));
    certified(&refresh(&mut source, &mut host, 1)); let old = prepare(&mut host, &human, 1);
    fs::remove_file(root.0.join("topology.json")).unwrap(); refresh(&mut source, &mut host, 2);
    fs::write(root.0.join("topology.json"), text(1)).unwrap();
    let work = source.work(); let revision = host.revision();
    let result = refresh(&mut source, &mut host, 3);
    assert_eq!(result.refusal, Some(TopologySourceRefusal::CoverageUnavailable));
    assert_eq!(source.work(), work); assert_eq!(host.revision(), revision);
    assert!(source.rebind(&mut host, revision, 4, loaded(&text(1))).is_err());
    let replacement = loaded(&text(2)); fs::write(root.0.join("topology.json"), text(2)).unwrap();
    let change = source.rebind(&mut host, revision, 4, replacement.clone()).unwrap();
    assert_eq!(source.work(), work); assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    let changed = root.bytes(); let cut = host.revision();
    assert_eq!(source.rebind(&mut host, 0, 4, replacement).unwrap(), change);
    assert_eq!(root.bytes(), changed); assert_eq!(host.revision(), cut);
    certified(&refresh(&mut source, &mut host, 5));
    assert!(host.dispatch(host.revision(), &old.automatic, &old.human, &old.action, &old.inputs, snapshot()).is_err());
    let fresh = prepare(&mut host, &human, 2); dispatch(&mut host, &fresh);
    assert_eq!(host.publish_checked(host.revision(), 2, Some(&fresh.inputs), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
}

#[test]
fn lifetime_check_and_read_exhaustion_withdraw_without_reading_or_refilling() {
    for by_checks in [false, true] {
        let root = Directory::new();
        let checks = if by_checks { 1 } else { 32 };
        let bytes = if by_checks { 32 * TOPOLOGY_REFRESH_READ_BYTES } else { TOPOLOGY_REFRESH_READ_BYTES };
        let doc = document(1, checks, bytes, 0, crate::action::consequence::mediation::MAX_CHECK_EDGE_VISITS);
        let (mut host, _, mut source) = fixture(&root, &doc);
        certified(&refresh(&mut source, &mut host, 1)); let work = source.work();
        let revision = host.revision();
        let result = source.refresh_with(&mut host, revision, 2, |_, _| panic!("exhaustion reached input")).unwrap();
        assert_eq!(result.refusal, Some(TopologySourceRefusal::BudgetExhausted)); assert_eq!(source.work(), work);
        assert!(!host.mediation_snapshot().unwrap().available);
        let inflated = loaded(&text(2)); let revision = host.revision();
        assert!(source.rebind(&mut host, revision, 3, inflated).is_err());
        assert_eq!(source.work(), work); assert!(!canonical(&host).available);
    }
}

#[test]
fn bypass_unreachable_and_insufficient_checker_work_never_become_accepted_source_coverage() {
    for mode in 0..3 {
        let root = Directory::new(); let budget = if mode == 0 { 1 }
            else { crate::action::consequence::mediation::MAX_CHECK_EDGE_VISITS };
        let doc = document(1, 32, 32 * TOPOLOGY_REFRESH_READ_BYTES, mode, budget);
        let (mut host, _, mut source) = fixture(&root, &doc);
        let result = refresh(&mut source, &mut host, 1);
        assert_eq!(result.refusal, Some(TopologySourceRefusal::NotCertified));
        match mode {
            0 => assert_eq!(result.checked, Some(Err(Error::Limit))),
            1 => assert!(matches!(result.checked, Some(Ok(CutCheck::Bypass(_))))),
            _ => assert!(matches!(result.checked, Some(Ok(CutCheck::Unreachable { .. })))),
        }
        assert!(!host.mediation_snapshot().unwrap().available);
        assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
    }
}

#[test]
fn wrong_owner_or_caller_predecessor_spends_no_source_work_and_revokes_nothing() {
    let root = Directory::new(); let (mut host, _, mut source) = fixture(&root, &text(1));
    certified(&refresh(&mut source, &mut host, 1));
    let other = Directory::new(); let (mut foreign, _, _) = fixture(&other, &text(1));
    let before = root.bytes(); let other_bytes = other.bytes(); let work = source.work();
    let revision = foreign.revision();
    assert!(source.refresh_with(&mut foreign, revision, 2, |_, _| panic!("foreign reached input")).is_err());
    let revision = host.revision();
    for (cut, operation) in [(revision - 1, 2), (revision, 0), (revision, 1)] {
        assert!(source.refresh_with(&mut host, cut, operation, |_, _| panic!("stale reached input")).is_err());
    }
    assert_eq!(source.work(), work); assert_eq!(root.bytes(), before); assert_eq!(other.bytes(), other_bytes);
    assert!(host.mediation_snapshot().unwrap().accepted.is_some());
    certified(&refresh(&mut source, &mut host, 2));
}

#[test]
fn read_unwind_quarantines_the_original_owner_and_old_source_cannot_bind_recovery() {
    let root = Directory::new(); let (mut host, _, mut source) = fixture(&root, &text(1));
    certified(&refresh(&mut source, &mut host, 1)); let before = root.bytes(); let revision = host.revision();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        source.refresh_with(&mut host, revision, 2, |path, limit| {
            let _ = read_regular(path, limit)?; panic!("read interrupted before whole input comparison")
        })
    }));
    assert!(result.is_err()); assert_eq!(host.mediation_snapshot(), Err(JournalError::Unavailable));
    assert_eq!(root.bytes(), before); assert_eq!(source.work().checks, 2);
    drop(host);
    let (mut recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
    let cut = recovered.revision();
    assert!(source.refresh(&mut recovered, cut, 3).is_err());
    assert!(!recovered.mediation_snapshot().unwrap().available);
}

#[test]
fn source_loss_at_every_original_store_barrier_returns_no_usable_old_certificate() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let (mut host, _, mut source) = fixture(&root, &text(1));
        certified(&refresh(&mut source, &mut host, 1)); let before = host.inspect();
        fs::remove_file(root.0.join("topology.json")).unwrap(); host.store.fail_once(barrier);
        let revision = host.revision();
        let error = source.refresh(&mut host, revision, 2).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("original Store failure expected"); };
        assert_eq!(failure.operation, barrier); assert_eq!(host.inspect(), before);
        assert_eq!(host.mediation_snapshot(), Err(JournalError::Unavailable));
        assert_eq!(canonical(&host).available, barrier != JournalIo::DirectorySync);
        assert_eq!(source.work().checks, 2);
        assert!(matches!(source.last_refusal(), Some(TopologySourceRefusal::Input(_))));
    }
}
