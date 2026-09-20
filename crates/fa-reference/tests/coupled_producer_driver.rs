//! Existing driver and atomic completion use a fresh coherent pair per boundary.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod driver;
#[path = "support/file_driver_source.rs"] mod files;
#[path = "support/file_publication_capture.rs"] mod capture;
use driver::{Rig, profile, snapshot};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{PublicationInputFile, FileCaptureError};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{FilePublicationProducer, PublicationProducerProfile};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateSource, StateLimits, StateFreshness};
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::witness::refinement::index::routing::RoutingBudget;
use fa_reference::Error;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn producer_profile() -> PublicationProducerProfile {
    PublicationProducerProfile { source: capture::SOURCE, scope: profile().delivery.scope,
        feed: 41, clock_domain: profile().delivery.clock_domain, after: 0 }
}
fn configure(host: &mut FileOversight) {
    host.enable_publication_validation(host.revision(), capture::limits()).unwrap();
    host.enable_publication_changes(host.revision(), PublicationChangePolicy { source: 41, after: 0,
        lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 } }).unwrap();
    host.enable_publication_change_freshness(host.revision(), PublicationFreshnessPolicy {
        clock_domain: profile().delivery.clock_domain, max_age_ticks: 3,
    }).unwrap();
}
fn sealed() -> EndpointOutcome { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } }
struct Live { rig: Rig, producer: FilePublicationProducer, witness: PublicationInputFile, feed: PublicationFeedFile, human: FileHumanPermit }
fn ready() -> Live {
    let mut rig = Rig::new(); configure(&mut rig.driver.supervisor_mut().host_mut().unwrap());
    let _ticket = rig.submit(1); rig.reviewed(1);
    let inputs = rig.inputs.as_ref().unwrap();
    let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
    let (producer, _) = FilePublicationProducer::create(rig.root.0.join("producer"), producer_profile(),
        capture::observations(inputs, 1, &[0, 2, 4]), ElapsedTick(1)).unwrap();
    let witness = producer.witness_reader(1, &action).unwrap(); let feed = producer.feed_reader().unwrap();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision(); host.refresh_publication_feed(revision, &feed, || ElapsedTick(1)).unwrap().unwrap();
        let revision = host.revision(); host.bind_publication_file_source(revision, 1, witness.read_capture().unwrap(), capture::requests()).unwrap();
    }
    let human = rig.human(1001, 31);
    Live { rig, producer, witness, feed, human }
}

#[test]
fn ordinary_steps_accept_whole_producer_updates_during_each_committee_capture() {
    let mut live = ready(); let inputs = live.rig.inputs.clone().unwrap(); let mut calls = 0u64;
    let report = live.rig.driver.step_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
        calls += 1;
        live.producer.publish(calls, capture::observations(&inputs, calls + 1, &[0, 2, 4, 100 + calls]), ElapsedTick(2)).unwrap();
        Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) })
    }, Some(&live.human), None);
    assert_eq!(calls, 2);
    assert!(matches!(report.publication.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
    assert_eq!(report.feeds.iter().map(|r| r.as_ref().unwrap().status.through).collect::<Vec<_>>(), vec![1, 3]);
    assert_eq!(report.publication.reads.iter().map(|r| r.as_ref().unwrap().generation).collect::<Vec<_>>(), vec![2, 3]);
    let published = live.rig.driver.step_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
        live.producer.publish(3, capture::observations(&inputs, 4, &[0, 2, 4, 103]), ElapsedTick(2)).unwrap();
        Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) })
    }, None, None);
    assert!(matches!(published.publication.evidence.result, Ok(FileDriverEvent::PublicationChecked { .. })));
    let settled = live.rig.driver.step_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2),
        |_, _| panic!("receipt reconciliation needs no producer"), None, None);
    assert!(settled.feeds.is_empty()); assert!(settled.publication.reads.is_empty());
    assert!(matches!(settled.publication.evidence.result, Ok(FileDriverEvent::Reconciled { .. })));
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn atomic_completion_uses_three_distinct_pairs_and_exposes_no_speculative_effect() {
    let mut live = ready(); let inputs = live.rig.inputs.clone().unwrap(); let mut calls = 0u64;
    let store = live.rig.root.store();
    let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
        calls += 1;
        assert_eq!(FileOversight::read_publication(&store, &profile()).unwrap().executions, 0);
        live.producer.publish(calls, capture::observations(&inputs, calls + 1, &[0, 2, 4, 100 + calls]), ElapsedTick(2)).unwrap();
        Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) })
    }, &live.human, None);
    assert_eq!(calls, 3);
    assert_eq!(report.authorization_feeds[0].as_ref().unwrap().status.through, 1);
    assert_eq!(report.completion.committed.iter().map(|r| r.status.through).collect::<Vec<_>>(), vec![3, 5]);
    assert_eq!(report.completion.completion.reads.iter().map(|r| r.as_ref().unwrap().generation).collect::<Vec<_>>(), vec![3, 4]);
    assert_eq!(report.completion.completion.result.unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
    let host = live.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(FileOversight::read_publication(&store, &profile()).unwrap(), host.inspect());
}

#[test]
fn final_producer_change_is_checked_exactly_not_rebased_under_original_approvals() {
    for key in [99, 1] {
        let mut live = ready(); let inputs = live.rig.inputs.clone().unwrap(); let mut calls = 0;
        let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
            calls += 1;
            if calls == 3 { live.producer.publish(1, capture::observations(&inputs, 2, &[0, 2, 4, key]), ElapsedTick(2)).unwrap(); }
            Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) })
        }, &live.human, None);
        assert_eq!(report.completion.committed.len(), 2);
        let publication = report.completion.completion.result.unwrap();
        assert_eq!(publication.basis, if key == 99 { PublicationBasis::Revalidated } else { PublicationBasis::Rejected(Error::Stale) });
        assert_eq!(publication.outcome, if key == 99 { EndpointOutcome::Executed { resulting_version: 2 } } else { sealed() });
        assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
    }
}

#[test]
fn bundle_loss_before_dispatch_retains_the_original_permit_for_actual_retry() {
    let mut live = ready(); let inputs = live.rig.inputs.clone(); let mut calls = 0;
    let path = live.rig.root.0.join("producer/delivery.bin"); let saved = path.with_extension("saved");
    let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
        calls += 1; if calls == 2 { std::fs::rename(&path, &saved).unwrap(); }
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &live.human, None);
    assert_eq!(report.authorization_reads.len(), 1);
    assert_eq!(report.completion.reads, vec![Err(FileCaptureError::Io(std::io::ErrorKind::NotFound))]);
    assert_eq!(report.completion.completion.result, Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.reserved, 16);
    std::fs::rename(saved, path).unwrap();
    let retry = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2),
        |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }), &live.human, None);
    assert!(retry.authorization_feeds.is_empty()); assert!(retry.authorization_reads.is_empty());
    assert_eq!(retry.completion.completion.result.unwrap().basis, PublicationBasis::Revalidated);
}

#[test]
fn final_bundle_loss_seals_and_settles_without_a_fabricated_feed_acknowledgment() {
    let mut live = ready(); let inputs = live.rig.inputs.clone(); let mut calls = 0;
    let path = live.rig.root.0.join("producer/delivery.bin");
    let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || ElapsedTick(2), |_, _| {
        calls += 1; if calls == 3 { std::fs::remove_file(&path).unwrap(); }
        Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &live.human, None);
    assert_eq!(report.completion.committed.len(), 1);
    assert_eq!(report.completion.reads.len(), 2);
    assert!(report.completion.reads[1].is_err());
    assert_eq!(report.completion.completion.result.unwrap().outcome, sealed());
    assert_eq!(live.rig.driver.supervisor().host().unwrap().inspect().control.ledger.available, 100);
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
}

#[test]
fn final_install_failure_returns_read_diagnostics_but_no_committed_candidate_reports() {
    let mut live = ready(); let inputs = live.rig.inputs.clone(); let calls = Cell::new(0);
    let store = live.rig.root.store();
    let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed, || {
        if calls.get() == 3 { std::fs::write(store.join("delivery.pending"), b"obstruction").unwrap(); }
        ElapsedTick(2)
    }, |_, _| {
        calls.set(calls.get() + 1); Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
    }, &live.human, None);
    assert_eq!(report.completion.reads.len(), 2); assert!(report.completion.reads.iter().all(Result::is_ok));
    assert_eq!(report.completion.completion.reads.len(), 2);
    assert!(report.completion.committed.is_empty());
    assert!(matches!(report.completion.completion.result, Err(JournalError::Io(_))));
    assert_eq!(FileOversight::read_publication(store, &profile()).unwrap().executions, 0);
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
}

#[test]
fn caught_final_provider_unwind_does_not_reenable_driver_dispatch() {
    let mut live = ready(); let inputs = live.rig.inputs.clone(); let mut calls = 0;
    assert!(catch_unwind(AssertUnwindSafe(|| live.rig.driver.complete_with_publication_feed(
        &live.witness, &live.feed, || ElapsedTick(2), |_, _| {
            calls += 1; assert!(calls != 3, "interrupted final committee capture");
            Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() })
        }, &live.human, None))).is_err());
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    let host = live.rig.driver.supervisor().host().unwrap();
    assert!(host.storage_failure().is_some());
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Authorized);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn concrete_policy_lease_is_still_independently_refreshed_in_three_reader_completion() {
    let mut rig = files::driver::Rig::new(); configure(&mut rig.driver.supervisor_mut().host_mut().unwrap());
    let path = rig.root.0.join("policy.json"); files::replace(&path, &files::document(1));
    let mut source = FileEvidenceSource::new(&path, files::SOURCE, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision(); host.enable_file_source(revision, FileSourcePolicy {
            source: StateSource { scope: profile().delivery.scope, source: files::SOURCE, generation: 1 },
            limits: StateLimits::default(), freshness: StateFreshness::new(3).unwrap(),
        }).unwrap();
        let revision = host.revision(); host.refresh_file_source(revision, &mut source, ElapsedTick(1)).unwrap();
    }
    let proposal = rig.proposal(); let revision = rig.driver.supervisor().host().unwrap().revision();
    rig.driver.supervisor_mut().set_snapshot(revision, Some(files::document(1).snapshot().clone())).unwrap();
    let _ticket = rig.port.submit(1, &proposal).unwrap();
    let mut file = files::FileRig { rig, source, path }; files::reviewed(&mut file);
    let action = file.rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
    let inputs = files::document(1).inputs_for(&action, &profile().committee).unwrap();
    let (mut producer, _) = FilePublicationProducer::create(file.rig.root.0.join("producer"), producer_profile(),
        capture::observations(&inputs, 1, &[0, 2, 4]), ElapsedTick(1)).unwrap();
    let witness = producer.witness_reader(1, &action).unwrap(); let feed = producer.feed_reader().unwrap();
    {
        let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision(); host.refresh_publication_feed(revision, &feed, || ElapsedTick(1)).unwrap().unwrap();
        let revision = host.revision(); host.bind_publication_file_source(revision, 1, witness.read_capture().unwrap(), capture::requests()).unwrap();
    }
    let human = files::human(&mut file, 1); let before = files::capture_count(&file);
    producer.publish(1, producer.image().inputs().clone(), ElapsedTick(5)).unwrap();
    let report = file.rig.driver.complete_from_files_with_publication(&mut file.source, &witness,
        Some(&feed), || ElapsedTick(5), &human, None);
    assert_eq!(files::capture_count(&file), before + 3);
    assert_eq!(report.completion.committed_source_updates.len(), 2);
    assert_eq!(report.completion.completion.committed.len(), 2);
    assert_eq!(report.completion.completion.completion.result.unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
}
