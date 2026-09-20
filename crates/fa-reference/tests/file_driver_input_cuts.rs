//! The existing supervised driver consumes stamped packets without a new wrapper.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod driver;
#[path = "support/publication_input_cut.rs"] mod f;
use driver::{Rig, snapshot};
use f::base;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::PublicationInputFile;
use fa_reference::action::consequence::delivery::publication_gate::changes::PublicationChangePolicy;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::witness::refinement::index::routing::RoutingBudget;
use fa_reference::Error;

#[test]
fn real_helper_review_needs_caught_up_source_at_authorization_and_preserves_the_normal_driver_path() {
    for caught_up in [false, true] {
        let mut rig = Rig::new();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); host.enable_publication_validation(revision, base::limits()).unwrap();
            let revision = host.revision(); host.enable_publication_changes(revision, PublicationChangePolicy {
                source: f::FEED, after: 0, lookup: RoutingBudget { steps: 10_000, bytes: 1_000_000 },
            }).unwrap();
        }
        let _ticket = rig.submit(1); rig.reviewed(1);
        let inputs = rig.inputs.clone();
        let action = rig.driver.supervisor().host().unwrap().request_action(1).unwrap().clone();
        let path = rig.root.0.join("cut-witness.bin");
        let original = f::packet(&action, inputs.as_ref().unwrap(), 1, 0, &[0, 2, 4], true);
        std::fs::write(&path, original.to_bytes().unwrap()).unwrap();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); host.bind_publication_file_source(revision, 1, original, base::requests()).unwrap();
        }
        let human = rig.human(1001, 31);
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision(); host.record_publication_change(revision, f::notice(1, 99)).unwrap();
        }
        let current = f::packet(&action, inputs.as_ref().unwrap(), 2, u64::from(caught_up), &[0, 2, 4], true);
        std::fs::write(&path, current.to_bytes().unwrap()).unwrap();
        let source = PublicationInputFile::new(&path, base::SOURCE).unwrap();
        let report = rig.driver.step_with_publication_source(&source, || ElapsedTick(1),
            |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }), Some(&human), None);
        if !caught_up {
            assert!(matches!(report.evidence.result, Err(JournalError::Contract(Error::Stale))));
            let host = rig.driver.supervisor().host().unwrap();
            assert!(host.storage_failure().is_some());
            assert_eq!(host.inspect().control.ledger.reserved, 0); assert_eq!(host.inspect().executions, 0);
        } else {
            assert!(matches!(report.evidence.result, Ok(FileDriverEvent::Dispatched { .. })));
            assert_eq!(report.reads.len(), 2);
            let publication = rig.driver.step_with_publication_source(&source, || ElapsedTick(2),
                |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: inputs.clone() }), None, None);
            assert!(matches!(publication.evidence.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
                if publication.outcome == (EndpointOutcome::Executed { resulting_version: 2 })));
            let settled = rig.driver.step_with_publication_source(&source, || ElapsedTick(2),
                |_, _| panic!("receipt reconciliation must not read sources"), None, None);
            assert!(settled.reads.is_empty()); assert!(settled.evidence.result.is_ok());
            assert_eq!(rig.driver.phase(), FileDriverPhase::Idle);
            assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
        }
    }
}
