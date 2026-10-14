//! Original socket congress, concrete policy files, actor gateway and two keys.
//! Controlled Allow ballots test integration, not native model detection quality.
use super::*;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase, FileSupervisedDriver};
use crate::action::consequence::delivery::persistent::observed::driver::evidence::{FileEvidenceReport, FileReviewLaunch};
use crate::action::consequence::delivery::persistent::observed::driver::evidence::topology::TopologyReport;
use crate::action::consequence::oversight::actor::ActorProposal;
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use crate::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use crate::action::consequence::oversight::helper_workers::HelperLimits;
use std::os::unix::net::UnixStream;

type Report = TopologyReport<FileEvidenceReport<FileDriverEvent>>;
const REQUEST: u64 = 901;
struct Session {
    driver: FileSupervisedDriver,
    reviewer: FileHumanReviewer,
    topology: FileTopologySource,
    policy: FileEvidenceSource,
}
impl Session {
    fn new(root: &Directory) -> Self {
        let (mut host, reviewer, mut topology) = fixture(root, &text(1));
        certified(&refresh(&mut topology, &mut host, 1));
        let capture = EvidenceSnapshot::new(EvidenceIdentity {
            source: 501, generation: 1, scope: profile().delivery.scope,
        }, snapshot(), BTreeMap::from([("reviewer".into(), b"actual context".to_vec())])).unwrap();
        fs::write(root.0.join("policy.json"), capture.encode()).unwrap();
        let policy = FileEvidenceSource::new(root.0.join("policy.json"), 501,
            profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
        let target = host.inspect().target;
        let (port, mut driver) = host.into_supervised_driver();
        let revision = driver.supervisor().host().unwrap().revision();
        driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
        let _ticket = port.submit(REQUEST, &ActorProposal { target, payload: b"visible".to_vec(),
            expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }).unwrap();
        Self { driver, reviewer, topology, policy }
    }
    fn launch(&mut self, expected_input_revision: u64)
        -> (HelperClient<UnixStream>, TopologyReport<Result<EvidenceIdentity, crate::action::consequence::delivery::persistent::observed::driver::evidence::FileSourceReviewError>>)
    {
        let (worker, peer) = UnixStream::pair().unwrap();
        let client = HelperClient::from_unix(peer, profile().committee.members()["reviewer"].profile_at(0)).unwrap();
        let result = self.driver.start_file_review_with_topology(&mut self.policy, &mut self.topology,
            FileReviewLaunch { request: REQUEST, round: 101, expected_input_revision,
                window: ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) },
                workers: BTreeMap::from([("reviewer".into(), worker)]), limits: HelperLimits::default() },
            || ElapsedTick(1));
        (client, result)
    }
    fn step(&mut self, human: Option<&FileHumanPermit>) -> Report {
        self.driver.step_from_file_with_topology(&mut self.policy, &mut self.topology,
            || ElapsedTick(1), human)
    }
    fn reviewed(&mut self) {
        let (mut client, launch) = self.launch(0);
        assert!(launch.result.is_ok()); assert_eq!(launch.topology.len(), 1);
        certified(launch.topology[0].as_ref().unwrap());
        for _ in 0..64 {
            let report = self.step(None);
            match report.result.result.unwrap() {
                FileDriverEvent::ReviewApplied { .. } => {
                    assert_eq!(report.topology.len(), 1);
                    certified(report.topology[0].as_ref().unwrap());
                    assert_eq!(report.result.observations.len(), 1);
                    return;
                }
                FileDriverEvent::Workers { .. } => {
                    assert!(report.topology.is_empty());
                    client.step().unwrap();
                    if client.phase() == ClientPhase::NeedsInference {
                        client.respond(Verdict::Allow, b"salt").unwrap();
                    }
                }
                other => panic!("original socket review: {other:?}"),
            }
        }
        panic!("socket review did not complete");
    }
    fn approve(&mut self) -> FileHumanPermit {
        let report = self.driver.request_human_approval_from_file_with_topology(&mut self.policy,
            &mut self.topology, 1001, ElapsedTick(40), || ElapsedTick(1));
        assert_eq!(report.topology.len(), 1); certified(report.topology[0].as_ref().unwrap());
        let request = report.result.result.unwrap();
        let mut host = self.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision(); self.reviewer.approve(&mut host, revision, &request).unwrap()
    }
    fn dispatched(&mut self) {
        self.reviewed(); let human = self.approve(); let report = self.step(Some(&human));
        assert_eq!(report.topology.len(), 2);
        for result in &report.topology { certified(result.as_ref().unwrap()); }
        assert_eq!(report.result.observations.len(), 2);
        assert!(matches!(report.result.result.unwrap(), FileDriverEvent::Dispatched { .. }));
    }
}

#[test]
fn socket_review_human_request_dispatch_and_publication_use_both_original_sources() {
    let root = Directory::new(); let mut session = Session::new(&root);
    session.dispatched(); let report = session.step(None);
    assert_eq!(report.topology.len(), 1); certified(report.topology[0].as_ref().unwrap());
    assert_eq!(report.result.observations.len(), 1);
    assert!(matches!(report.result.result.unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
        if matches!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 })));
    let spent = session.topology.work(); let reads = session.policy.status().read_attempts;
    fs::remove_file(root.0.join("topology.json")).unwrap();
    fs::remove_file(root.0.join("policy.json")).unwrap();
    let reconciled = session.step(None);
    assert!(matches!(reconciled.result.result.unwrap(), FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. }));
    assert!(reconciled.topology.is_empty()); assert!(reconciled.result.observations.is_empty());
    assert_eq!(session.topology.work(), spent); assert_eq!(session.policy.status().read_attempts, reads);
    assert!(matches!(session.step(None).result.result.unwrap(), FileDriverEvent::Idle));
    let host = session.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn stale_launch_spends_nothing_but_real_topology_loss_prevents_policy_read_and_worker_admission() {
    let root = Directory::new(); let mut session = Session::new(&root);
    let work = session.topology.work(); let bytes = root.bytes();
    let (_, refused) = session.launch(1);
    assert!(refused.result.is_err()); assert!(refused.topology.is_empty());
    assert_eq!(session.topology.work(), work); assert_eq!(root.bytes(), bytes);
    fs::remove_file(root.0.join("topology.json")).unwrap();
    let (_, refused) = session.launch(0);
    assert!(refused.result.is_err()); assert_eq!(refused.topology.len(), 1);
    assert!(refused.topology[0].as_ref().unwrap().refusal.is_some());
    assert_eq!(session.policy.status().read_attempts, 0);
    assert_eq!(session.driver.phase(), FileDriverPhase::Idle);
    let host = session.driver.supervisor().host().unwrap();
    assert!(!host.mediation_snapshot().unwrap().available);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn topology_loss_at_human_request_never_issues_a_key_or_spends_policy_reads() {
    let root = Directory::new(); let mut session = Session::new(&root); session.reviewed();
    let reads = session.policy.status().read_attempts;
    fs::remove_file(root.0.join("inventory.json")).unwrap();
    let report = session.driver.request_human_approval_from_file_with_topology(&mut session.policy,
        &mut session.topology, 1001, ElapsedTick(40), || ElapsedTick(1));
    assert!(report.result.result.is_err()); assert_eq!(report.topology.len(), 1);
    assert!(report.topology[0].as_ref().unwrap().refusal.is_some());
    assert!(report.result.observations.is_empty()); assert_eq!(session.policy.status().read_attempts, reads);
    let host = session.driver.supervisor().host().unwrap();
    assert!(host.human_status(1001).is_err()); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn topology_loss_between_ready_captures_revokes_the_actual_reservation_before_dispatch() {
    let root = Directory::new(); let mut session = Session::new(&root); session.reviewed();
    let human = session.approve(); let mut clocks = 0;
    let report = session.driver.step_from_file_with_topology(&mut session.policy, &mut session.topology,
        || { clocks += 1; if clocks == 2 { fs::remove_file(root.0.join("topology.json")).unwrap(); } ElapsedTick(1) }, Some(&human));
    assert!(report.result.result.is_err()); assert_eq!(report.topology.len(), 2);
    certified(report.topology[0].as_ref().unwrap());
    let refused = report.topology[1].as_ref().unwrap();
    assert!(refused.refusal.is_some()); assert_eq!(refused.withdrawal.as_ref().unwrap().refunded_units, 16);
    assert_eq!(report.result.observations.len(), 1);
    let host = session.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn loss_inside_publication_seals_nonexecution_but_only_reconciliation_refunds_the_charge() {
    let root = Directory::new(); let mut session = Session::new(&root); session.dispatched();
    let reads = session.policy.status().read_attempts;
    fs::remove_file(root.0.join("topology.json")).unwrap();
    let report = session.step(None);
    assert_eq!(report.topology.len(), 1); assert!(report.topology[0].as_ref().unwrap().refusal.is_some());
    assert!(report.result.observations.is_empty()); assert_eq!(session.policy.status().read_attempts, reads);
    assert!(matches!(report.result.result.unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
        if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
    assert_eq!(session.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    let report = session.step(None); assert!(report.topology.is_empty());
    assert!(matches!(report.result.result.unwrap(), FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::NotExecuted { .. }), .. }));
    let host = session.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.available, 100); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn valid_topology_does_not_replace_missing_policy_and_post_capture_expiry_still_wins() {
    for expire in [false, true] {
        let root = Directory::new(); let mut session = Session::new(&root); session.dispatched();
        if !expire { fs::remove_file(root.0.join("policy.json")).unwrap(); }
        let mut clocks = 0;
        let report = session.driver.step_from_file_with_topology(&mut session.policy, &mut session.topology,
            || { clocks += 1; ElapsedTick(if expire && clocks > 1 { 100 } else { 1 }) }, None);
        assert_eq!(report.topology.len(), 1); certified(report.topology[0].as_ref().unwrap());
        assert!(matches!(report.result.result.unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
            if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
        assert_eq!(session.driver.supervisor().host().unwrap().inspect().executions, 0);
    }
}

#[test]
fn source_check_storage_failure_never_reaches_policy_or_publication_at_any_original_barrier() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut session = Session::new(&root); session.dispatched();
        let reads = session.policy.status().read_attempts;
        session.driver.supervisor_mut().host_mut().unwrap().store.fail_once(barrier);
        // Driver entry observes time before capture. An equal trusted tick is
        // idempotent in the original observe helper and causes no replacement.
        let report = session.step(None);
        assert_eq!(report.topology.len(), 1); assert!(report.topology[0].is_err());
        assert!(report.result.observations.is_empty()); assert_eq!(session.policy.status().read_attempts, reads);
        assert!(matches!(report.result.result.unwrap(), FileDriverEvent::PublicationUnknown { .. }));
        let host = session.driver.supervisor().host().unwrap();
        assert!(host.storage_failure().is_some()); assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert!(host.mediation_snapshot().is_err());
    }
}

#[test]
fn operation_ids_continue_the_same_custodian_and_never_wrap_into_a_stale_success() {
    let root = Directory::new(); let mut session = Session::new(&root); session.dispatched();
    {
        let mut host = session.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        certified(&session.topology.refresh(&mut host, revision, u64::MAX).unwrap());
    }
    let work = session.topology.work(); let reads = session.policy.status().read_attempts;
    let report = session.step(None);
    assert!(matches!(report.topology.as_slice(), [Err(JournalError::Contract(Error::Overflow))]));
    assert_eq!(session.topology.work(), work); assert_eq!(session.policy.status().read_attempts, reads);
    assert!(matches!(report.result.result.unwrap(), FileDriverEvent::PublicationUnknown { .. }));
    assert_eq!(session.topology.last_refusal(), Some(&TopologySourceRefusal::OperationExhausted));
    let host = session.driver.supervisor().host().unwrap();
    assert!(host.storage_failure().is_some()); assert_eq!(host.inspect().executions, 0);
}
