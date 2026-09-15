//! Supervised durable execution consumes the credential only at first publication.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
#[path = "support/file_source_intake.rs"] mod source_fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome,
    credential_broker::{BrokerCredential, BrokerRouteBinding, ProviderCredential}};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::credential::FILE_OVERSIGHT_CREDENTIAL_PROFILE;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::policy_state::StateLimits;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
use fa_reference::Error;

fn binding() -> BrokerRouteBinding {
    BrokerRouteBinding { family: "publication".into(), route: "adapter:file-oversight".into() }
}
fn inventory() -> LoadedPerimeterInventory {
    LoadedPerimeterInventory::from_json_bytes(format!(r#"{{"version":1,"families":[{{
      "scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"publication",
      "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
      "credentials":[{{"credential":"oversight-token","holder":"broker"}}],
      "routes":[{{"route":"adapter:file-oversight","effect":"file_write",
      "profile":{{"id":"{}","generation":1}},"trust_path":["actor","enforcement"],
      "threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},
      "mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["reference"]}}],
      "residual_nonclaims":["reference"]}}]}}"#, FILE_OVERSIGHT_CREDENTIAL_PROFILE).as_bytes()).unwrap()
}
fn enable(rig: &mut Rig) {
    let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    host.enable_credential_guard(revision, &inventory(), &binding()).unwrap();
}
fn bind(rig: &Rig, bytes: &[u8])
    -> fa_reference::action::consequence::delivery::persistent::observed::credential::FileCredentialPermit
{
    let host = rig.driver.supervisor().host().unwrap();
    host.bind_credential_pair(host.revision(), &inventory(), &binding(),
        BrokerCredential::new(bytes.to_vec()).unwrap(), ProviderCredential::new(bytes.to_vec()).unwrap()).unwrap()
}
fn publish_with(rig: &mut Rig,
    credential: &fa_reference::action::consequence::delivery::persistent::observed::credential::FileCredentialPermit)
    -> FileDriverEvent
{
    let input = rig.inputs.clone();
    let now = rig.driver.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
    rig.driver.step_with_evidence_and_credential(|| now,
        |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: input.clone() }), None, credential).unwrap()
}

#[test]
fn original_driver_publishes_only_when_the_live_owner_credential_is_present_at_first_effect() {
    let mut rig = Rig::new(); enable(&mut rig); let ticket = rig.submit(1);
    rig.reviewed(1); let human = rig.human(1001, 30);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { request: 1, attempt: 1 }));
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingPublication { request: 1 });
    let credential = bind(&rig, b"secret");
    let event = publish_with(&mut rig, &credential);
    assert!(matches!(event, FileDriverEvent::PublicationChecked { publication, .. }
        if publication.outcome == EndpointOutcome::Executed { resulting_version: 2 }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { request: 1, .. }));
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
}

#[test]
fn legacy_driver_publication_cannot_bypass_the_guard_and_is_not_auto_retried() {
    let mut rig = Rig::new(); enable(&mut rig); let ticket = rig.submit(1);
    rig.reviewed(1); let human = rig.human(1001, 30);
    assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
    let event = rig.step(None);
    assert!(matches!(event, FileDriverEvent::PublicationUnknown {
        error: JournalError::Contract(Error::Incomplete), ..
    }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
    assert_eq!(rig.driver.phase(), FileDriverPhase::AwaitingReconciliation { request: 1 });
    assert!(matches!(rig.port.poll(&ticket), Knowledge::Unknown { .. }));
}

#[test]
fn credential_from_another_durable_owner_cannot_publish_the_identical_phase() {
    let mut left = Rig::new(); enable(&mut left); left.submit(1); left.reviewed(1);
    let human = left.human(1001, 30); assert!(matches!(left.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
    let mut right = Rig::new(); enable(&mut right);
    let foreign = bind(&right, b"secret");
    let event = publish_with(&mut left, &foreign);
    assert!(matches!(event, FileDriverEvent::PublicationUnknown {
        error: JournalError::Contract(Error::Binding), ..
    }));
    assert_eq!(left.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(left.driver.supervisor().host().unwrap().inspect().control.ledger.charged, 16);
}

#[test]
fn credential_is_not_required_for_query_only_reconciliation_after_a_terminal_receipt() {
    let mut rig = Rig::new(); enable(&mut rig); rig.submit(1); rig.reviewed(1);
    let human = rig.human(1001, 30); assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
    let credential = bind(&rig, b"secret");
    assert!(matches!(publish_with(&mut rig, &credential), FileDriverEvent::PublicationChecked { .. }));
    // No credential is presented after publication; the retained endpoint receipt
    // remains the authority for reconciliation.
    assert!(matches!(rig.step(None), FileDriverEvent::Reconciled { request: 1, .. }));
    assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}

#[test]
fn registered_file_driver_refreshes_source_and_uses_credential_only_at_publication() {
    let mut file = source_fixture::cold(10, StateLimits::default());
    {
        let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_credential_guard(revision, &inventory(), &binding()).unwrap();
    }
    let prepared = file.rig.driver.prepare_file_intake(&mut file.source, || ElapsedTick(1));
    assert!(prepared.result.is_ok());
    let proposal = file.rig.proposal();
    let _ticket = file.rig.port.submit(1, &proposal).unwrap();
    file.reviewed();
    let human = file.human();
    file.dispatch(&human);
    let credential = {
        let host = file.rig.driver.supervisor().host().unwrap();
        host.bind_credential_pair(host.revision(), &inventory(), &binding(),
            BrokerCredential::new(b"secret".to_vec()).unwrap(), ProviderCredential::new(b"secret".to_vec()).unwrap()).unwrap()
    };
    let captures = file.rig.driver.supervisor().host().unwrap().file_source_status().unwrap().capture.retained_events;
    let report = file.rig.driver.step_from_file_with_credential(&mut file.source, || ElapsedTick(1), None, &credential);
    assert_eq!(report.source_updates.len(), 1);
    assert!(report.source_updates[0].is_ok());
    assert!(matches!(report.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
        if publication.outcome == EndpointOutcome::Executed { resulting_version: 2 }));
    assert!(file.rig.driver.supervisor().host().unwrap().file_source_status().unwrap().capture.retained_events > captures);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
}
