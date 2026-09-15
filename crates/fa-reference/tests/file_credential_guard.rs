//! Durable credential mediation without persisting secret bytes.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason,
    credential_broker::{BrokerCredential, BrokerRouteBinding, ProviderCredential}};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, publication::PublicationBasis};
use fa_reference::action::consequence::delivery::persistent::observed::credential::FILE_OVERSIGHT_CREDENTIAL_PROFILE;
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
use fa_reference::{Error, Snapshot};

fn binding() -> BrokerRouteBinding {
    BrokerRouteBinding { family: "publication".to_owned(), route: "adapter:file-oversight".to_owned() }
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
      "residual_nonclaims":["operator storage remains trusted"]}}]}}"#,
      FILE_OVERSIGHT_CREDENTIAL_PROFILE).as_bytes()).unwrap()
}
fn broker(bytes: &[u8]) -> BrokerCredential { BrokerCredential::new(bytes.to_vec()).unwrap() }
fn provider(bytes: &[u8]) -> ProviderCredential { ProviderCredential::new(bytes.to_vec()).unwrap() }
fn enable(host: &mut FileOversight) {
    let policy = host.enable_credential_guard(host.revision(), &inventory(), &binding()).unwrap();
    assert_eq!(policy.credential, "oversight-token");
    assert_eq!(host.credential_policy(), Some(&policy));
    assert!(host.publication_guard_required());
}

#[test]
fn valid_effect_requires_live_credential_pair_and_legacy_publication_cannot_bypass_it() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root); enable(&mut host);
    let keys = ready(&mut host, &reviewer, 1, b"credentialed"); dispatch(&mut host, &keys);
    let before = host.revision();
    assert_eq!(host.publish(before, 1), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), before);
    assert_eq!(host.publish_checked(before, 1, Some(&keys.inputs), snapshot(), ElapsedTick(1)),
        Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.revision(), before);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);

    let permit = host.bind_credential_pair(host.revision(), &inventory(), &binding(),
        broker(b"secret"), provider(b"secret")).unwrap();
    let published = host.publish_checked_with_credential(host.revision(), 1, Some(&keys.inputs),
        snapshot(), ElapsedTick(1), &permit).unwrap();
    assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(published.basis, PublicationBasis::Revalidated);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn mismatched_live_pair_refuses_before_any_publication_transition() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root); enable(&mut host);
    let keys = ready(&mut host, &reviewer, 1, b"credentialed"); dispatch(&mut host, &keys);
    let before = host.revision();
    assert!(matches!(host.bind_credential_pair(before, &inventory(), &binding(),
        broker(b"broker"), provider(b"provider")), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(host.revision(), before);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn credential_outage_does_not_prevent_restrictive_sealing_and_refund_after_reconciliation() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root); enable(&mut host);
    let keys = ready(&mut host, &reviewer, 1, b"credentialed"); dispatch(&mut host, &keys);
    // No credential pair is installed. Missing current evidence is restrictive,
    // so the old checked-publication path may seal but can never execute.
    let result = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(1)).unwrap();
    assert_eq!(result.outcome, EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    assert_eq!(result.basis, PublicationBasis::Rejected(Error::Incomplete));
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().control.ledger.charged, 0);
}

#[test]
fn guard_contract_survives_reopen_but_old_secret_capability_does_not() {
    let root = Directory::new(); let (mut host, _reviewer) = create(&root); enable(&mut host);
    let policy = host.credential_policy().unwrap().clone();
    let old = host.bind_credential_pair(host.revision(), &inventory(), &binding(),
        broker(b"secret"), provider(b"secret")).unwrap();
    drop(host);

    let (mut reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(reopened.credential_policy(), Some(&policy));
    // Issuer branding is checked before attempt lookup or evidence handling.
    assert!(matches!(reopened.publish_checked_with_credential(reopened.revision(), 999, None,
        Snapshot::default(), ElapsedTick(2), &old), Err(JournalError::Contract(Error::Binding))));
    let revision = reopened.revision();
    let _fresh = reopened.bind_credential_pair(revision, &inventory(), &binding(),
        broker(b"secret"), provider(b"secret")).unwrap();
    assert_eq!(reopened.revision(), revision);
    assert_eq!(reopened.inspect().executions, 0);
}

#[test]
fn ambiguous_or_wrong_route_contract_refuses_before_journal_mutation() {
    let root = Directory::new(); let (mut host, _) = create(&root); let before = host.revision();
    let ambiguous = String::from_utf8(format!(r#"{{"version":1,"families":[{{
      "scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"publication",
      "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
      "credentials":[{{"credential":"one","holder":"broker"}},{{"credential":"two","holder":"broker"}}],
      "routes":[{{"route":"adapter:file-oversight","effect":"file_write",
      "profile":{{"id":"{}","generation":1}},"trust_path":["actor","enforcement"],
      "threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},
      "mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["reference"]}}],
      "residual_nonclaims":["reference"]}}]}}"#, FILE_OVERSIGHT_CREDENTIAL_PROFILE).into_bytes()).unwrap();
    let ambiguous = LoadedPerimeterInventory::from_json_bytes(ambiguous.as_bytes()).unwrap();
    assert!(matches!(host.enable_credential_guard(before, &ambiguous, &binding()),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(host.revision(), before);
    assert!(host.credential_policy().is_none());
    assert!(!host.publication_guard_required());
}

#[test]
fn guard_is_bootstrap_only_and_cannot_be_added_after_effect_work_exists() {
    let root = Directory::new(); let (mut host, _) = create(&root);
    host.propose(host.revision(), 1, spec(&host, b"already-started"), snapshot()).unwrap();
    let before = host.revision();
    assert!(matches!(host.enable_credential_guard(before, &inventory(), &binding()),
        Err(JournalError::Contract(Error::WrongState))));
    assert_eq!(host.revision(), before);
    assert!(host.credential_policy().is_none());
}
