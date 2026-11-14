//! Real computed review and retained numerical custody across target restriction.
//! Synthetic weights establish control flow, not detector qualification.
use super::*;
use crate::action::ResolvedTarget;
use crate::action::consequence::delivery::persistent::observed::{
    FileHumanPermit,
    narrowing::{FileAuthorityNarrowReceipt, FileAuthorityNarrowRequest},
};
use crate::action::consequence::oversight::human::HumanDisposition;

fn target(driver: &FileSupervisedDriver) -> ResolvedTarget {
    driver.supervisor().host().unwrap().request_action(901)
        .unwrap().spec().target.unwrap()
}

fn restrict(driver: &mut FileSupervisedDriver, operation: u64,
    retained: Vec<ResolvedTarget>) -> FileAuthorityNarrowReceipt
{
    let mut host = driver.supervisor_mut().host_mut().unwrap();
    let state = host.inspect();
    let request = FileAuthorityNarrowRequest::new(operation, state.control.sequence,
        state.control.ledger.epoch, retained).unwrap();
    host.narrow_authority(state.revision, request).unwrap()
}

fn retain(driver: &mut FileSupervisedDriver, operation: u64) -> FileAuthorityNarrowReceipt {
    let selected = target(driver);
    restrict(driver, operation, vec![selected])
}

fn approve_current(driver: &mut FileSupervisedDriver, reviewer: &FileHumanReviewer,
    key: u64) -> FileHumanPermit
{
    let request = driver.request_learned_human_approval(key, ElapsedTick(80), ElapsedTick(1))
        .unwrap();
    let mut host = driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    reviewer.approve(&mut host, revision, &request).unwrap()
}

fn progress(driver: &mut FileSupervisedDriver, human: Option<&FileHumanPermit>)
    -> Result<FileDriverEvent, JournalError>
{
    driver.step_with_learned_evidence(|| ElapsedTick(1), || Ok(snapshot()), human)
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

#[test]
fn refined_original_review_continues_repeated_restrictions_and_publishes_with_fresh_human() {
    for reserve in [false, true] {
        let root = Directory::new();
        let (port, ticket, mut driver, reviewer) = setup(&root, 1);
        let FileDriverEvent::ReviewApplied { receipt: reviewed, .. } = complete(&mut driver)
            else { panic!("original computed review was not applied"); };
        assert_eq!(reviewed.policy.control.decision.consequence, Consequence::Continue);
        let records = driver.learned_probe_review().unwrap().review().records().clone();
        let input = driver.learned_probe_review().unwrap().review().input().clone();
        let evaluations = driver.learned_probe_review().unwrap().review().evaluations();
        assert!(driver.learned_probe_review().unwrap().review().history().len() > 1);
        let numerical = driver.supervisor().host().unwrap()
            .learned_generation_inspection().unwrap().numerical;
        let actor = driver.supervisor().host().unwrap().actor_snapshot().unwrap();
        let old_human = approve_current(&mut driver, &reviewer, 1001);
        if reserve {
            let mut captures = 0;
            assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1), || {
                captures += 1;
                if captures == 1 { Ok(snapshot()) } else { Err(Error::Incomplete) }
            }, Some(&old_human)), Err(JournalError::Contract(Error::Incomplete))));
            assert_eq!(captures, 2);
        }
        let first = retain(&mut driver, 501);
        assert_eq!(first.change().refunded_units, 0);
        assert!(first.change().cancelled.is_empty());
        let state = driver.supervisor().host().unwrap().inspect();
        let bytes = image(&root);
        assert_eq!(state.control.ledger.reserved, if reserve { 16 } else { 0 });
        assert_eq!(driver.pending_narrowing_continuation(901).unwrap(), Some(501));
        let continued = driver.continue_after_narrowing(901, 501).unwrap();
        assert_eq!(continued.previous_control_sequence(), reviewed.policy.control.sequence);
        assert_eq!(continued.control_sequence(), first.change().sequence);
        assert_eq!(continued.retained_automatic_permit(), reserve);
        assert_eq!(driver.continue_after_narrowing(901, 501).unwrap(), continued);
        assert_eq!(driver.pending_narrowing_continuation(901).unwrap(), None);
        assert_eq!(driver.supervisor().host().unwrap().inspect(), state);
        assert_eq!(image(&root), bytes);

        // The historical computed result is still the original cut after more
        // than one local continuation. Each new bridge proves only restrictions.
        let second = retain(&mut driver, 502);
        let third = retain(&mut driver, 503);
        assert_eq!(third.change().previous_sequence, second.change().sequence);
        assert_eq!(driver.pending_narrowing_continuation(901).unwrap(), Some(503));
        assert!(driver.continue_after_narrowing(901, 502).is_err());
        let continued = driver.continue_after_narrowing(901, 503).unwrap();
        assert_eq!(continued.previous_control_sequence(), first.change().sequence);
        assert_eq!(continued.narrowing_operations(), &[502, 503]);
        assert_eq!(driver.continue_after_narrowing(901, 503).unwrap(), continued);
        assert_eq!(driver.learned_probe_review().unwrap().review().input(), &input);
        assert_eq!(driver.learned_probe_review().unwrap().review().records(), &records);
        assert_eq!(driver.learned_probe_review().unwrap().review().evaluations(), evaluations);
        assert_eq!(driver.supervisor().host().unwrap().learned_generation_inspection()
            .unwrap().numerical, numerical);
        assert_eq!(driver.supervisor().host().unwrap().actor_snapshot().unwrap(), actor);
        assert_eq!(driver.supervisor().host().unwrap().human_status(1001).unwrap().disposition,
            HumanDisposition::Revoked);
        assert!(progress(&mut driver, Some(&old_human)).is_err());
        assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
        let fresh_human = approve_current(&mut driver, &reviewer, 1002);
        assert!(matches!(progress(&mut driver, Some(&fresh_human)).unwrap(),
            FileDriverEvent::Dispatched { request: 901, .. }));
        assert!(matches!(progress(&mut driver, None).unwrap(),
            FileDriverEvent::PublicationChecked { publication, source_failure: None, .. }
            if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
        assert!(matches!(driver.step_with_learned_evidence(|| ElapsedTick(1),
            || panic!("reconciliation cannot recapture or recompute evidence"), None).unwrap(),
            FileDriverEvent::Reconciled { .. }));
        assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
        let state = driver.supervisor().host().unwrap().inspect();
        assert_eq!(state.payload, b"visible");
        assert_eq!(state.executions, 1);
        assert_eq!(state.control.ledger.charged, 16);
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(driver.learned_probe_review().unwrap().review().records(), &records);
    }
}

#[test]
fn extracted_completed_report_keeps_original_sidecar_and_ready_custody() {
    let root = Directory::new();
    let (port, ticket, mut driver, reviewer) = setup(&root, 0);
    complete(&mut driver);
    let report = driver.take_learned_probe_review().unwrap().unwrap();
    retain(&mut driver, 501);
    driver.continue_after_narrowing(901, 501).unwrap();
    let human = approve_current(&mut driver, &reviewer, 1001);
    assert!(matches!(progress(&mut driver, Some(&human)).unwrap(), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(progress(&mut driver, None).unwrap(),
        FileDriverEvent::PublicationChecked { publication, .. }
        if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
    progress(&mut driver, None).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
    assert_eq!(report.review().status(), FileLearnedProbeStatus::Finished);
}

#[test]
fn changed_original_source_cannot_rebase_a_finished_learned_review() {
    let root = Directory::new();
    let (_, _, mut driver, _) = setup(&root, 0);
    complete(&mut driver);
    retain(&mut driver, 501);
    driver.continue_after_narrowing(901, 501).unwrap();
    let ready = driver.job.as_ref().unwrap().control_sequence;
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        step(&mut host);
    }
    retain(&mut driver, 502);
    let state = driver.supervisor().host().unwrap().inspect();
    let bytes = image(&root);
    let records = driver.learned_probe_review().unwrap().review().records().clone();
    assert!(driver.pending_narrowing_continuation(901).is_err());
    assert!(driver.continue_after_narrowing(901, 502).is_err());
    assert_eq!(driver.job.as_ref().unwrap().control_sequence, ready);
    assert_eq!(driver.supervisor().host().unwrap().inspect(), state);
    assert_eq!(image(&root), bytes);
    assert_eq!(driver.learned_probe_review().unwrap().review().records(), &records);
    assert_eq!(state.executions, 0);
}

#[test]
fn unresolved_original_generation_or_reset_is_not_completed_by_narrowing_continuation() {
    use crate::action::consequence::activation::tensor::kv::{
        decoder::monitoring::restart::KvRestartBudget, model::MAX_MODEL_KV_VALUES,
    };
    use crate::action::consequence::delivery::persistent::observed::containment::FileResetRequest;
    use crate::action::consequence::gate::ReviewBinding;
    for reset in [false, true] {
        let root = Directory::new();
        let (_, _, mut driver, _) = setup(&root, 0);
        complete(&mut driver);
        let ready = driver.job.as_ref().unwrap().control_sequence;
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let state = host.inspect();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            if reset {
                let checkpoint = host.capture_learned_checkpoint(state.revision, 601,
                    numerical.actor_revision, state.control.ledger.epoch).unwrap();
                let budget = KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
                    audit: host.machine.broker.hosted_learned_original().unwrap().policy().allowance() };
                let request = FileResetRequest { operation: 602,
                    expected_control_sequence: state.control.sequence,
                    expected_actor_revision: numerical.actor_revision,
                    expected_authority_epoch: state.control.ledger.epoch,
                    binding: ReviewBinding { round: 603, reducer_generation: 1, evidence_root: [8; 32] },
                    retained_targets: vec![state.target] };
                let revision = host.revision();
                host.begin_learned_reset(revision, &checkpoint, request, budget).unwrap();
                assert!(host.pending_learned_reset().unwrap().is_some());
            } else {
                host.begin_learned_step(state.revision, numerical.actor_revision, numerical.position)
                    .unwrap();
                assert!(host.learned_generation_inspection().unwrap().pending.is_some());
            }
        }
        retain(&mut driver, 501);
        let state = driver.supervisor().host().unwrap().inspect();
        let numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
        let pending = driver.supervisor().host().unwrap().pending_learned_reset().unwrap();
        let bytes = image(&root);
        assert_eq!(driver.pending_narrowing_continuation(901),
            Err(JournalError::Contract(Error::Incomplete)));
        assert_eq!(driver.continue_after_narrowing(901, 501),
            Err(JournalError::Contract(Error::Incomplete)));
        assert_eq!(driver.job.as_ref().unwrap().control_sequence, ready);
        assert_eq!(driver.supervisor().host().unwrap().inspect(), state);
        assert_eq!(driver.supervisor().host().unwrap().learned_generation_inspection().unwrap(), numerical);
        assert_eq!(driver.supervisor().host().unwrap().pending_learned_reset().unwrap(), pending);
        assert_eq!(image(&root), bytes);
        assert_eq!(state.executions, 0);
    }
}

#[test]
fn running_computation_and_excluded_completed_target_cannot_borrow_a_narrowing_receipt() {
    for completed in [false, true] {
        let root = Directory::new();
        let (port, ticket, mut driver, _) = setup(&root, 0);
        if completed { complete(&mut driver); } else { advance(&mut driver, 1).unwrap(); }
        let records = driver.learned_probe_review().unwrap().review().records().clone();
        if completed { restrict(&mut driver, 501, Vec::new()); } else { retain(&mut driver, 501); }
        let state = driver.supervisor().host().unwrap().inspect();
        let bytes = image(&root);
        assert_eq!(driver.pending_narrowing_continuation(901).unwrap(), None);
        assert!(driver.continue_after_narrowing(901, 501).is_err());
        assert_eq!(driver.supervisor().host().unwrap().inspect(), state);
        assert_eq!(image(&root), bytes);
        assert_eq!(driver.learned_probe_review().unwrap().review().records(), &records);
        if completed {
            assert!(matches!(port.poll(&ticket),
                Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
        } else {
            assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 901 }));
        }
        assert_eq!(state.executions, 0);
    }
}
