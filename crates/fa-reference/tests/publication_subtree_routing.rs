//! Real original journal, congress, two independent keys and endpoint settlement.
//! Synthetic views are enforcement fixtures, not authentic producer observations.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::{Directory, Keys, profile, snapshot};
use fa_reference::action::{ElapsedTick, FrozenAction};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::{JournalError, RecoveryReserve};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationEvidence, FilePublicationInputs, FileWitnessInput};
use fa_reference::action::consequence::delivery::publication_gate::PublicationLimits;
use fa_reference::action::consequence::delivery::publication_gate::changes::{ChangeRouting, PublicationChange, PublicationChangePolicy};
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry, WitnessRequest, MAX_WITNESSES};
use fa_reference::witness::refinement::RefinementBudget;
use fa_reference::witness::refinement::index::routing::{RoutingBudget, RoutingStrategy, WitnessChange};
use fa_reference::Error;

fn validation() -> PublicationLimits {
    PublicationLimits { bindings: 8, validation: RefinementBudget { steps: 10_000, value_bytes: 1_048_576 } }
}
fn policy(budget: RoutingBudget) -> PublicationChangePolicy { PublicationChangePolicy { source: 41, after: 0, lookup: budget } }
fn budget() -> RoutingBudget { RoutingBudget { steps: 100, bytes: 8000 } }
fn domain() -> DomainProjection {
    DomainProjection::new(40, 1, ProjectionKey { source: 40, branch: 4, projection: 7, source_epoch: 1 })
}
fn observed(revision: u64, keys: &[u64]) -> FilePublicationInputs {
    let close = TrustedClosingMarker { key: domain().projection(), final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 1).unwrap();
    frontiers.accept(close.key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    FilePublicationInputs::new(Some(FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(domain(), DomainClosure::Closed(close)),
        keys.iter().map(|key| SnapshotEntry::new(*key, 1, b"observed".to_vec()).unwrap()).collect(), &frontiers).unwrap()), None)
}
fn requests(slot: usize) -> Vec<WitnessRequest> {
    (0..MAX_WITNESSES).map(|k| {
        if slot == 0 && k == 0 { WitnessRequest::EmptyRange { start: 0, end: u64::MAX } }
        else { let start = ((slot * MAX_WITNESSES + k) * 4) as u64;
            WitnessRequest::EmptyRange { start, end: start + 1 } }
    }).collect()
}
fn notice(sequence: u64, key: u64) -> PublicationChange {
    PublicationChange { source: 41, sequence, change: WitnessChange::Key { domain: domain(), key } }
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }

struct Rig { host: FileOversight, reviewer: FileHumanReviewer, actions: Vec<FrozenAction>, root: Directory }
impl Rig {
    fn new(subtree: bool, lookup: RoutingBudget) -> Self {
        let root = Directory::new();
        let (mut host, reviewer) = if subtree {
            FileOversight::create_with_publication_subtree_routing(root.store(), profile(), validation(), policy(lookup)).unwrap()
        } else {
            let (mut host, reviewer) = FileOversight::create_with_publication_validation(root.store(), profile(), validation()).unwrap();
            host.enable_publication_changes(host.revision(), policy(lookup)).unwrap();
            (host, reviewer)
        };
        // This new selection is bootstrap work, not consumption of terminal reserve.
        host.enable_recovery_reserve(host.revision(), RecoveryReserve::terminal()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let mut actions = Vec::new();
        for slot in 0..8 {
            let id = slot as u64 + 1;
            let action = host.propose(host.revision(), id, fixture::spec(&host, b"visible"), snapshot()).unwrap();
            host.bind_publication_evidence(host.revision(), id,
                FilePublicationEvidence::new(observed(1, &[]), requests(slot)).unwrap()).unwrap();
            host.record_publication_inputs(host.revision(), id, 0, Some(observed(1, &[]))).unwrap();
            actions.push(action);
        }
        Self { host, reviewer, actions, root }
    }
    fn keys(&mut self) -> Keys {
        let action = self.actions[7].clone(); let inputs = fixture::inputs(&action, b"complete reviewed input");
        fixture::review_existing(&mut self.host, 8, 108, &inputs);
        let automatic = self.host.authorize(self.host.revision(), 8, &inputs, snapshot()).unwrap();
        let request = self.host.request_human_approval(self.host.revision(), 1008, 8, &inputs, ElapsedTick(31)).unwrap();
        let revision = self.host.revision();
        let human = self.reviewer.approve(&mut self.host, revision, &request).unwrap();
        Keys { action, inputs, automatic, human, request }
    }
}

#[test]
fn sparse_notification_preserves_unrelated_observations_and_the_original_two_key_path() {
    let mut rig = Rig::new(true, budget());
    let report = rig.host.record_publication_change(rig.host.revision(), notice(1, 1_000_000)).unwrap();
    assert_eq!(report.routing, ChangeRouting::Indexed);
    assert_eq!(report.affected, vec![1]);
    assert!(report.spent.steps <= budget().steps && report.spent.bytes <= budget().bytes);
    assert_eq!(rig.host.publication_input_revision(1).unwrap(), 2);
    for id in 2..=8 { assert_eq!(rig.host.publication_input_revision(id).unwrap(), 1); }
    let keys = rig.keys(); fixture::dispatch(&mut rig.host, &keys);
    let result = rig.host.publish_checked(rig.host.revision(), 8, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(result.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    rig.host.reconcile(rig.host.revision(), 8).unwrap();
    assert_eq!(rig.host.inspect().executions, 1); assert_eq!(rig.host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(rig.root.store(), &profile()).unwrap(), rig.host.inspect());
}

#[test]
fn relevant_late_notification_still_seals_while_the_unrelated_neighbor_executes() {
    for relevant in [false, true] {
        let mut rig = Rig::new(true, budget()); let keys = rig.keys();
        fixture::dispatch(&mut rig.host, &keys);
        let key = if relevant { (7 * MAX_WITNESSES * 4) as u64 } else { 1_000_000 };
        let report = rig.host.record_publication_change(rig.host.revision(), notice(1, key)).unwrap();
        assert_eq!(report.routing, ChangeRouting::Indexed);
        assert_eq!(report.affected, if relevant { vec![1, 8] } else { vec![1] });
        assert_eq!(rig.host.inspect().control.ledger.charged, 16);
        let result = rig.host.publish_checked(rig.host.revision(), 8, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
        assert_eq!(result.basis, if relevant { PublicationBasis::Rejected(Error::Incomplete) } else { PublicationBasis::Revalidated });
        assert_eq!(result.outcome, if relevant { sealed() } else { EndpointOutcome::Executed { resulting_version: 2 } });
        assert_eq!(rig.host.inspect().control.ledger.charged, 16, "routing and sealing never refund on their own");
        rig.host.reconcile(rig.host.revision(), 8).unwrap();
        assert_eq!(rig.host.inspect().control.ledger.charged, if relevant { 0 } else { 16 });
    }
}

#[test]
fn an_index_nonhit_does_not_replace_exact_final_snapshot_validation() {
    let mut rig = Rig::new(true, budget()); let keys = rig.keys(); fixture::dispatch(&mut rig.host, &keys);
    let report = rig.host.record_publication_change(rig.host.revision(), notice(1, 1_000_000)).unwrap();
    assert_eq!(report.affected, vec![1]);
    let current = observed(2, &[(7 * MAX_WITNESSES * 4) as u64]);
    rig.host.record_publication_inputs(rig.host.revision(), 8, 1, Some(current)).unwrap();
    let result = rig.host.publish_checked(rig.host.revision(), 8, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Stale)); assert_eq!(result.outcome, sealed());
    assert_eq!(rig.host.inspect().executions, 0); assert_eq!(rig.host.inspect().control.ledger.charged, 16);
}

#[test]
fn both_routing_versions_replay_their_exact_budget_sensitive_withdrawals() {
    for subtree in [false, true] {
        let mut rig = Rig::new(subtree, budget());
        let report = rig.host.record_publication_change(rig.host.revision(), notice(1, 1_000_000)).unwrap();
        assert_eq!(report.routing, if subtree { ChangeRouting::Indexed } else { ChangeRouting::Conservative(Error::Incomplete) });
        assert_eq!(report.affected, if subtree { vec![1] } else { (1..=8).collect::<Vec<_>>() });
        let revisions: Vec<_> = (1..=8).map(|id| rig.host.publication_input_revision(id).unwrap()).collect();
        // Persist a later expected-input revision: changing V1 replay to V2
        // would now either fail this record or alter the historical transition.
        let next = rig.host.publication_input_revision(8).unwrap();
        rig.host.record_publication_inputs(rig.host.revision(), 8, next, Some(observed(2, &[1_000_000]))).unwrap();
        let before = rig.host.inspect();
        assert_eq!(FileOversight::read_publication(rig.root.store(), &profile()).unwrap(), before);
        let Rig { host, reviewer, actions: _, root } = rig; drop(reviewer); drop(host);
        let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
        assert_eq!(host.publication_routing_strategy().unwrap(), if subtree { RoutingStrategy::SubtreeV2 } else { RoutingStrategy::PrefixV1 });
        assert_eq!(host.publication_change_report().unwrap().unwrap(), report);
        for id in 1..=8 {
            assert_eq!(host.publication_input_revision(id).unwrap(), revisions[id as usize - 1] + u64::from(id == 8));
        }
        assert_eq!(host.inspect().executions, 0); assert!(!host.clock_ready());
        assert!(host.enable_publication_subtree_routing(host.revision()).is_err());
    }
}

#[test]
fn gaps_semantic_changes_and_insufficient_budgets_keep_conservative_withdrawal() {
    for mode in 0..3 {
        let mut rig = Rig::new(true, if mode == 2 { RoutingBudget::default() } else { budget() });
        let notice = match mode {
            0 => notice(2, 1_000_000),
            1 => PublicationChange { source: 41, sequence: 1, change: WitnessChange::Domain { domain: domain() } },
            _ => notice(1, 1_000_000),
        };
        let report = rig.host.record_publication_change(rig.host.revision(), notice).unwrap();
        assert_eq!(report.affected, (1..=8).collect::<Vec<_>>());
        assert_eq!(report.routing, match mode { 0 => ChangeRouting::MissingTail, 1 => ChangeRouting::Indexed,
            _ => ChangeRouting::Conservative(Error::Incomplete) });
        for id in 1..=8 { assert_eq!(rig.host.publication_input_revision(id).unwrap(), 2); }
        assert_eq!(rig.host.inspect().control.ledger.available, 100);
        if mode == 0 {
            let repair = rig.host.record_publication_change(rig.host.revision(), self::notice(1, 0)).unwrap();
            assert_eq!(repair.routing, ChangeRouting::RecoveringTail);
            assert_eq!(repair.affected, (1..=8).collect::<Vec<_>>());
            assert!(!repair.status.complete());
        }
    }
}

#[test]
fn pinned_open_refuses_legacy_or_different_cost_policy_before_cleanup() {
    for subtree in [false, true] {
        let rig = Rig::new(subtree, budget()); let before = rig.host.inspect();
        let root = rig.root.0.clone(); let store = rig.root.store();
        std::fs::write(store.join("delivery.pending"), b"unacknowledged staging data").unwrap();
        let Rig { host, reviewer, actions: _, root: directory } = rig; drop(reviewer); drop(host);
        let expected = if subtree { policy(RoutingBudget { steps: 101, ..budget() }) } else { policy(budget()) };
        assert!(matches!(FileOversight::open_with_publication_subtree_routing(&store, profile(), validation(), expected),
            Err(JournalError::Contract(Error::Binding))));
        assert!(store.join("delivery.pending").exists());
        assert_eq!(FileOversight::read_publication(&store, &profile()).unwrap(), before);
        if subtree {
            let (host, _) = FileOversight::open_with_publication_subtree_routing(&store, profile(), validation(), policy(budget())).unwrap();
            assert_eq!(host.publication_routing_strategy().unwrap(), RoutingStrategy::SubtreeV2);
            assert!(!store.join("delivery.pending").exists());
        }
        assert_eq!(directory.0, root);
    }
}

#[test]
fn bootstrap_selection_is_atomic_and_cannot_be_applied_after_work_begins() {
    let root = Directory::new();
    let invalid = PublicationLimits { bindings: 0, ..validation() };
    assert!(FileOversight::create_with_publication_subtree_routing(root.store(), profile(), invalid, policy(budget())).is_err());
    assert!(!root.store().exists());
    let (mut host, _) = FileOversight::create_with_publication_validation(root.store(), profile(), validation()).unwrap();
    let initial = host.revision();
    assert_eq!(host.enable_publication_subtree_routing(initial), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), initial);
    host.enable_publication_changes(host.revision(), policy(budget())).unwrap();
    host.enable_publication_subtree_routing(host.revision()).unwrap();
    let revision = host.revision();
    assert_eq!(host.enable_publication_subtree_routing(revision), Err(JournalError::Contract(Error::Duplicate)));
    assert_eq!(host.revision(), revision);
    assert_eq!(host.enable_publication_subtree_routing(revision - 1), Err(JournalError::Contract(Error::Stale)));
    let other = Directory::new();
    let (mut legacy, _) = FileOversight::create_with_publication_validation(other.store(), profile(), validation()).unwrap();
    legacy.enable_publication_changes(legacy.revision(), policy(budget())).unwrap();
    legacy.record_publication_change(legacy.revision(), notice(1, 0)).unwrap();
    let before = legacy.inspect();
    assert_eq!(legacy.enable_publication_subtree_routing(legacy.revision()), Err(JournalError::Contract(Error::WrongState)));
    assert_eq!(legacy.inspect(), before);
    assert_eq!(legacy.publication_routing_strategy().unwrap(), RoutingStrategy::PrefixV1);
}

#[test]
fn failed_notification_write_cannot_expose_the_private_selective_candidate() {
    let mut rig = Rig::new(true, budget()); let before = rig.host.inspect();
    let pending = rig.root.store().join("delivery.pending"); std::fs::create_dir(&pending).unwrap();
    assert!(matches!(rig.host.record_publication_change(rig.host.revision(), notice(1, 1_000_000)), Err(JournalError::Io(_))));
    assert_eq!(rig.host.inspect(), before);
    assert_eq!(rig.host.publication_routing_strategy(), Err(JournalError::Unavailable));
    assert!(rig.host.storage_failure().is_some());
    assert_eq!(FileOversight::read_publication(rig.root.store(), &profile()).unwrap(), before);
    let Rig { host, reviewer, actions: _, root } = rig; drop(reviewer); drop(host);
    std::fs::remove_dir(&pending).unwrap();
    let (host, _) = FileOversight::open_with_publication_subtree_routing(root.store(), profile(), validation(), policy(budget())).unwrap();
    assert_eq!(host.publication_routing_strategy().unwrap(), RoutingStrategy::SubtreeV2);
    assert!(host.publication_change_report().unwrap().is_none());
    for id in 1..=8 { assert_eq!(host.publication_input_revision(id).unwrap(), 1); }
    assert_eq!(host.inspect().executions, 0);
}
