//! Existing helper-socket driver and atomic completion, with actual lost history.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod driver;
#[path = "support/file_publication_capture.rs"] mod capture;
use driver::{Rig, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileHumanPermit;
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileDriverPhase;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::PublicationInputFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::heartbeat::feed::PublicationFeedFile;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{PublicationProducerImage, PublicationProducerProfile};
use fa_reference::action::consequence::delivery::publication_gate::PublicationLimits;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::action::consequence::oversight::{CommitteeInput, human::HumanDisposition, supervised::DriverEvidence};
use fa_reference::witness::refinement::{RefinementBudget, index::routing::RoutingBudget};
use fa_reference::Error;
use std::path::Path;

fn write(path: &Path, image: &PublicationProducerImage) {
    let pending = path.with_extension("next");
    std::fs::write(&pending, image.to_bytes().unwrap()).unwrap(); std::fs::rename(pending, path).unwrap();
}
struct Live {
    rig: Rig,
    inputs: CommitteeInput,
    image: PublicationProducerImage,
    witness: PublicationInputFile,
    feed: PublicationFeedFile,
    human: FileHumanPermit,
}
impl Live {
    fn new(budget: PublicationLimits) -> Self {
        let mut rig = Rig::new();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let r = host.revision(); host.enable_publication_validation(r, budget).unwrap();
            let r = host.revision(); host.enable_publication_changes(r, PublicationChangePolicy {
                source: 41, after: 0, lookup: RoutingBudget { steps: 10000, bytes: 1000000 },
            }).unwrap();
            let r = host.revision(); host.enable_publication_change_freshness(r, PublicationFreshnessPolicy {
                clock_domain: profile().delivery.clock_domain, max_age_ticks: 3,
            }).unwrap();
            let r = host.revision(); host.enable_publication_snapshot_fallback(r).unwrap();
        }
        let _ticket = rig.submit(1); rig.reviewed(1);
        let inputs = rig.inputs.clone().unwrap();
        let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let producer = PublicationProducerProfile { source: capture::SOURCE, scope: profile().delivery.scope,
            feed: 41, clock_domain: profile().delivery.clock_domain, after: 0 };
        let mut image = PublicationProducerImage::new(producer,
            capture::observations(&inputs, 1, &[0, 2, 4]), ElapsedTick(1)).unwrap();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let r = host.revision();
            host.bind_publication_file_source(r, 1, image.capture(1, &action).unwrap(), capture::requests()).unwrap();
        }
        for revision in 2..=131 {
            image = image.advance(image.generation(), capture::observations(&inputs, revision,
                &[0, 2, 4, if revision % 2 == 0 { 99 } else { 100 }]), ElapsedTick(1)).unwrap();
        }
        assert!(image.batch().after() > 0);
        let path = rig.root.0.join("producer.bin"); write(&path, &image);
        let witness = PublicationInputFile::from_producer(&path, producer, 1, &action).unwrap();
        let feed = PublicationFeedFile::from_producer(&path, producer).unwrap();
        let human = rig.human(1001, 31);
        Self { rig, inputs, image, witness, feed, human }
    }
}

#[test]
fn original_atomic_driver_revalidates_all_dependencies_without_claiming_tail_repair() {
    for inserted in [99, 1] {
        let mut live = Live::new(capture::limits());
        let changed = live.image.advance(live.image.generation(),
            capture::observations(&live.inputs, 132, &[0, 2, 4, inserted]), ElapsedTick(2)).unwrap();
        let path = live.rig.root.0.join("producer.bin"); let mut captures = 0;
        let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed,
            || ElapsedTick(2), |_, _| {
                captures += 1;
                if captures == 3 { write(&path, &changed); }
                Ok(DriverEvidence { inputs: Some(live.inputs.clone()), snapshot: snapshot() })
            }, &live.human, None);
        assert_eq!(captures, 3);
        assert_eq!(report.authorization_feeds.len(), 1);
        assert_eq!(report.completion.committed.len(), 2);
        for feed in &report.completion.committed {
            assert_eq!(feed.status.through, 0); assert!(!feed.status.complete());
            assert_eq!(feed.freshness.eligibility, Err(Error::Incomplete));
            assert!(feed.changes.is_empty());
        }
        let publication = report.completion.completion.result.unwrap();
        assert_eq!(publication.basis, if inserted == 99 { PublicationBasis::Revalidated }
            else { PublicationBasis::Rejected(Error::Stale) });
        assert_eq!(publication.outcome, if inserted == 99 { EndpointOutcome::Executed { resulting_version: 2 } }
            else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } });
        assert_eq!(live.rig.driver.phase(), FileDriverPhase::Idle);
        let host = live.rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(inserted == 99));
        assert_eq!(host.inspect().control.ledger.charged, if inserted == 99 { 16 } else { 0 });
        assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
}

#[test]
fn missing_second_bundle_is_sealed_not_replaced_by_the_first_successful_capture() {
    let mut live = Live::new(capture::limits());
    let path = live.rig.root.0.join("producer.bin"); let mut captures = 0;
    let report = live.rig.driver.complete_with_publication_feed(&live.witness, &live.feed,
        || ElapsedTick(2), |_, _| {
            captures += 1;
            if captures == 3 { std::fs::remove_file(&path).unwrap(); }
            Ok(DriverEvidence { inputs: Some(live.inputs.clone()), snapshot: snapshot() })
        }, &live.human, None);
    assert_eq!(captures, 3);
    assert!(report.completion.reads.last().unwrap().is_err());
    let publication = report.completion.completion.result.unwrap();
    assert_eq!(publication.basis, PublicationBasis::Rejected(Error::Incomplete));
    assert_eq!(publication.outcome, EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    let host = live.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().control.ledger.available, 100);
}

#[test]
fn exact_comparison_exhaustion_cannot_spend_either_key_even_with_fresh_head_evidence() {
    let mut live = Live::new(PublicationLimits { validation: RefinementBudget::default(), ..capture::limits() });
    let report = live.rig.driver.step_with_publication_feed(&live.witness, &live.feed,
        || ElapsedTick(2), |_, _| Ok(DriverEvidence { inputs: Some(live.inputs.clone()), snapshot: snapshot() }),
        Some(&live.human), None);
    assert_eq!(report.feeds.len(), 1);
    assert!(matches!(report.publication.evidence.result, Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(live.rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    let host = live.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().executions, 0);
}
