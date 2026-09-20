//! Native helper-socket review consumes actual coupled-source replacements.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod driver;
#[path = "support/file_publication_capture.rs"] mod capture;
use driver::{Rig, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileDriverPhase;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::{
    FilePublicationProducer, PublicationProducerProfile,
};
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::delivery::publication_gate::changes::freshness::PublicationFreshnessPolicy;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::witness::refinement::index::routing::RoutingBudget;
use fa_reference::Error;
use std::cell::Cell;

#[test]
fn supervised_atomic_completion_consumes_derived_notices_and_actual_caught_up_images() {
    for inserted in [99, 1] {
        let mut rig = Rig::new();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); host.enable_publication_validation(revision, capture::limits()).unwrap();
            let revision = host.revision(); host.enable_publication_changes(revision, PublicationChangePolicy {
                source: 41, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 },
            }).unwrap();
            let revision = host.revision(); host.enable_publication_change_freshness(revision, PublicationFreshnessPolicy {
                clock_domain: profile().delivery.clock_domain, max_age_ticks: 3,
            }).unwrap();
        }
        let _ticket = rig.submit(1); rig.reviewed(1);
        let inputs = rig.inputs.clone().unwrap();
        let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let (mut producer, _) = FilePublicationProducer::create(rig.root.0.join("producer"), PublicationProducerProfile {
            source: capture::SOURCE, scope: profile().delivery.scope, feed: 41,
            clock_domain: profile().delivery.clock_domain, after: 0,
        }, capture::observations(&inputs, 1, &[0, 2, 4]), ElapsedTick(1)).unwrap();
        let witness = producer.witness_reader(1, &action).unwrap();
        let feed = producer.feed_reader().unwrap();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); host.refresh_publication_feed(revision, &feed, || ElapsedTick(1)).unwrap().unwrap();
            let revision = host.revision();
            host.bind_publication_file_source(revision, 1, witness.read_capture().unwrap(), capture::requests()).unwrap();
        }
        let human = rig.human(1001, 31);
        let calls = Cell::new(0);
        let changed = Cell::new(false);
        let next = capture::observations(&inputs, 2, &[0, 2, 4, inserted]);
        let report = rig.driver.complete_with_publication_feed(&witness, &feed, || {
            // After the dispatch-bound witness read, but before the independent
            // publication-bound feed read. Neither reader accepts a cached image.
            if calls.get() == 2 && !changed.replace(true) {
                producer.publish(1, next.clone(), ElapsedTick(2)).unwrap();
            }
            ElapsedTick(2)
        }, |_, _| {
            calls.set(calls.get() + 1);
            Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(inputs.clone()) })
        }, &human, None);
        assert!(changed.get()); assert_eq!(calls.get(), 3);
        assert_eq!(report.authorization_feeds.len(), 1);
        assert_eq!(report.completion.committed.len(), 2);
        let publication = report.completion.completion.result.unwrap();
        let allowed = inserted == 99;
        assert_eq!(publication.basis, if allowed { PublicationBasis::Revalidated } else { PublicationBasis::Rejected(Error::Stale) });
        assert_eq!(publication.outcome, if allowed { EndpointOutcome::Executed { resulting_version: 2 } }
            else { EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed } });
        assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
        let host = rig.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, u64::from(allowed));
        assert_eq!(host.inspect().control.ledger.charged, if allowed { 16 } else { 0 });
        assert_eq!(host.inspect().control.ledger.reserved, 0);
    }
}
