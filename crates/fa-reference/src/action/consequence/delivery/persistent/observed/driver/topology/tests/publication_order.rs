//! The same production decorator and original two-key effect path, with actual
//! topology files. Controlled policy callbacks isolate publication ordering;
//! they are not a replacement checker, helper vote importer or effect endpoint.
use super::*;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::delivery::persistent::observed::driver::{
    evidence::topology::test_provider, provider::{Callback, EvidenceProvider},
};
use crate::action::consequence::delivery::persistent::observed::mediation::source::TopologyRefresh;
use crate::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
};

fn explicit_case(checks: u64) -> (Case, FileTopologySource) {
    let (mut case, mut topology) = Case::unselected(checks);
    {
        let mut host = case.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        let report = topology.source.refresh(&mut host, revision, 1).unwrap();
        assert!(report.refusal.is_none());
    }
    (case, topology.source)
}

fn step<P, F>(case: &mut Case, topology: &mut FileTopologySource, policy: P, clock: F)
    -> (Vec<Result<TopologyRefresh, JournalError>>, Result<FileDriverEvent, JournalError>)
where P: EvidenceProvider, F: FnMut() -> ElapsedTick {
    let mut reports = Vec::new();
    let result = case.driver.step_with_provider(clock,
        &mut test_provider(topology, &mut reports, policy), None, None);
    (reports, result)
}

fn assert_charged_without_execution(case: &Case, reviewed: &Reviewed) {
    let host = case.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(matches!(host.inspect().control.ledger.stages[&reviewed.attempt],
        ActionState::Dispatching | ActionState::Unknown));
    assert_eq!(case.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: reviewed.request });
}

#[test]
fn explicit_publication_observes_topology_loss_during_policy_capture() {
    for inventory in [false, true] {
        let (mut case, mut source) = explicit_case(8);
        let reviewed = case.review(901); case.dispatch(&reviewed);
        let path = if inventory { case.root.inventory() } else { case.root.topology() };
        let mut captures = 0;
        let (reports, result) = step(&mut case, &mut source, Callback(|_: &crate::action::FrozenAction, _: &CommitteeContract| {
            captures += 1;
            fs::remove_file(&path).unwrap();
            Ok(evidence(&reviewed.inputs))
        }), || ElapsedTick(1));
        assert_eq!(captures, 1);
        assert_eq!(reports.len(), 1, "one final observation, not checks on both sides of policy");
        let report = reports[0].as_ref().unwrap();
        assert!(matches!(&report.refusal, Some(TopologySourceRefusal::Input(_))));
        assert!(report.withdrawal.is_some());
        assert_eq!(source.work().checks, 2);
        assert_eq!(source.last_operation(), 2);
        assert!(matches!(result.unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
            if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
        assert_charged_without_execution(&case, &reviewed);
        let work = source.work();
        assert!(matches!(case.driver.step_with_evidence(|| ElapsedTick(1),
            |_, _| panic!("reconciliation cannot request policy"), None).unwrap(),
            FileDriverEvent::Reconciled { .. }));
        assert_eq!(source.work(), work);
        assert_eq!(case.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    }
}

#[test]
fn explicit_publication_still_observes_topology_after_inner_policy_refusal() {
    let (mut case, mut source) = explicit_case(8);
    let reviewed = case.review(901); case.dispatch(&reviewed);
    let (reports, result) = step(&mut case, &mut source,
        Callback(|_: &crate::action::FrozenAction, _: &CommitteeContract| Err(Error::Binding)), || ElapsedTick(1));
    assert_eq!(reports.len(), 1);
    assert!(reports[0].as_ref().unwrap().refusal.is_none());
    assert_eq!(source.work().checks, 2);
    assert!(matches!(result.unwrap(), FileDriverEvent::PublicationChecked {
        publication, source_failure: Some(Error::Binding), ..
    } if matches!(publication.outcome, EndpointOutcome::NotExecuted { .. })));
    assert_charged_without_execution(&case, &reviewed);
}

struct FailedPolicy;
impl EvidenceProvider for FailedPolicy {
    fn capture<F>(&mut self, _: &mut FileOversight, _: &crate::action::FrozenAction, _: &mut F)
        -> Result<Result<DriverEvidence, Error>, JournalError>
    where F: FnMut() -> ElapsedTick {
        Err(JournalError::Unavailable)
    }
}

#[test]
fn explicit_outer_policy_failure_cannot_spend_topology_or_reach_the_final_clock() {
    let (mut case, mut source) = explicit_case(8);
    let reviewed = case.review(901); case.dispatch(&reviewed);
    let before = case.root.bytes(); let work = source.work(); let mut clocks = 0;
    let (reports, result) = step(&mut case, &mut source, FailedPolicy,
        || { clocks += 1; ElapsedTick(1) });
    assert!(reports.is_empty()); assert_eq!(source.work(), work);
    assert_eq!(clocks, 1); assert_eq!(case.root.bytes(), before);
    assert!(matches!(result.unwrap(), FileDriverEvent::PublicationUnknown {
        error: JournalError::Unavailable, ..
    }));
    assert_charged_without_execution(&case, &reviewed);
}

#[test]
fn explicit_post_policy_topology_storage_failures_preserve_charge_at_every_barrier() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
        JournalIo::Rename, JournalIo::DirectorySync]
    {
        let (mut case, mut source) = explicit_case(8);
        let reviewed = case.review(901); case.dispatch(&reviewed);
        let before = case.root.bytes();
        case.driver.supervisor_mut().host_mut().unwrap().store.fail_once(barrier);
        let mut captures = 0; let mut clocks = 0;
        let (reports, result) = step(&mut case, &mut source, Callback(|_: &crate::action::FrozenAction, _: &CommitteeContract| {
            captures += 1; Ok(evidence(&reviewed.inputs))
        }), || { clocks += 1; ElapsedTick(1) });
        assert_eq!(captures, 1, "policy precedes the source persistence attempt");
        assert_eq!(clocks, 1, "unacknowledged topology cannot acquire final time");
        assert_eq!(reports.len(), 1);
        assert!(matches!(&reports[0], Err(JournalError::Io(failure)) if failure.operation == barrier));
        assert!(matches!(result.unwrap(), FileDriverEvent::PublicationUnknown {
            error: JournalError::Io(ref failure), ..
        } if failure.operation == barrier));
        assert_eq!(source.work().checks, 2);
        assert_eq!(source.last_refusal(), Some(&TopologySourceRefusal::CheckNotAcknowledged));
        assert!(case.driver.supervisor().host().unwrap().storage_failure().is_some());
        assert_charged_without_execution(&case, &reviewed);
        if barrier != JournalIo::DirectorySync { assert_eq!(case.root.bytes(), before); }
    }
}

#[test]
fn explicit_prepublication_capture_keeps_topology_first_and_skips_policy_on_loss() {
    let (mut case, mut source) = explicit_case(8);
    let reviewed = case.review(901);
    fs::remove_file(case.root.topology()).unwrap();
    let mut captures = 0;
    let (reports, result) = step(&mut case, &mut source, Callback(|_: &crate::action::FrozenAction, _: &CommitteeContract| {
        captures += 1; Ok(evidence(&reviewed.inputs))
    }), || ElapsedTick(1));
    assert_eq!(captures, 0, "the Ready capture keeps its original topology-first contract");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].as_ref().unwrap().withdrawal.is_some());
    assert!(result.is_err());
    let host = case.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
}

#[test]
fn explicit_resolved_and_expired_effects_need_neither_source_nor_policy() {
    for resolved in [false, true] {
        let (mut case, mut source) = explicit_case(1);
        let reviewed = case.review(901); case.dispatch(&reviewed);
        if resolved {
            let mut host = case.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            let publication = host.publish_checked(revision, reviewed.attempt,
                Some(&reviewed.inputs), snapshot(), ElapsedTick(1)).unwrap();
            assert!(matches!(publication.outcome, EndpointOutcome::Executed { .. }));
        }
        fs::remove_file(case.root.topology()).unwrap(); fs::remove_file(case.root.inventory()).unwrap();
        let work = source.work();
        let (reports, result) = step(&mut case, &mut source,
            Callback(|_: &crate::action::FrozenAction, _: &CommitteeContract| panic!("resolved/expired policy")),
            || if resolved { ElapsedTick(1) } else { ElapsedTick(100) });
        assert!(reports.is_empty()); assert_eq!(source.work(), work);
        assert_eq!(source.last_operation(), 1);
        let expected = if resolved { PublicationBasis::PreviouslyResolved }
            else { PublicationBasis::DeadlineElapsed };
        assert!(matches!(result.unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
            if publication.basis == expected));
        assert_eq!(case.driver.supervisor().host().unwrap().inspect().executions, u64::from(resolved));
    }
}

#[test]
fn concrete_file_adapter_publishes_the_actual_reviewed_file_input_with_one_final_check() {
    let (mut case, mut topology) = explicit_case(8);
    let captured = EvidenceSnapshot::new(EvidenceIdentity { source: 7, generation: 1,
        scope: profile().delivery.scope }, snapshot(),
        BTreeMap::from([("reviewer".to_owned(), b"observed context".to_vec())])).unwrap();
    let path = case.root.0.join("policy.json"); fs::write(&path, captured.encode()).unwrap();
    let mut policy = FileEvidenceSource::new(&path, 7, profile().delivery.scope,
        MAX_EVIDENCE_FILE_BYTES).unwrap();
    let reviewed = case.review_with_file_input(901, Some(&captured)); case.dispatch(&reviewed);
    let report = case.driver.step_from_file_with_topology(&mut policy, &mut topology,
        || ElapsedTick(1), None);
    assert_eq!(report.topology.len(), 1);
    assert!(report.topology[0].as_ref().unwrap().refusal.is_none());
    assert_eq!(report.result.observations, vec![Ok(captured.identity())]);
    assert!(matches!(report.result.result.unwrap(), FileDriverEvent::PublicationChecked { publication, .. }
        if matches!(publication.outcome, EndpointOutcome::Executed { .. })));
    assert_eq!(topology.work().checks, 2);
    assert_eq!(policy.status().read_attempts, 1);
    assert_eq!(case.driver.supervisor().host().unwrap().inspect().executions, 1);
}
