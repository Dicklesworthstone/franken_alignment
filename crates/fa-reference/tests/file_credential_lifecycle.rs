//! Durable credential generation and terminal revocation for FileOversight.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason,
    credential_broker::{BrokerCredential, BrokerRouteBinding, CredentialRevocationRequest,
        CredentialRotationRequest, ProviderCredential}};
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
      "residual_nonclaims":["reference"]}}]}}"#, FILE_OVERSIGHT_CREDENTIAL_PROFILE).as_bytes()).unwrap()
}
fn broker(bytes: &[u8]) -> BrokerCredential { BrokerCredential::new(bytes.to_vec()).unwrap() }
fn provider(bytes: &[u8]) -> ProviderCredential { ProviderCredential::new(bytes.to_vec()).unwrap() }
fn enable(host: &mut FileOversight) {
    host.enable_credential_guard(host.revision(), &inventory(), &binding()).unwrap();
    let status = host.credential_status().unwrap().unwrap();
    assert_eq!(status.generation, 1); assert!(!status.revoked); assert_eq!(status.retained_changes, 0);
}

#[test]
fn durable_rotation_invalidates_old_live_permit_but_not_the_already_charged_effect() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root); enable(&mut host);
    let keys = ready(&mut host, &reviewer, 1, b"rotated"); dispatch(&mut host, &keys);
    let old = host.bind_credential_pair(host.revision(), &inventory(), &binding(), broker(b"one"), provider(b"one")).unwrap();
    let request = CredentialRotationRequest { operation: 41, expected_generation: 1, next_generation: 2 };
    let receipt = host.rotate_credential_guard(host.revision(), request).unwrap();
    assert_eq!(receipt.generation, 2); assert_eq!(host.inspect().control.ledger.charged, 16);
    assert!(matches!(host.publish_checked_with_credential(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1), &old),
        Err(JournalError::Contract(Error::Binding))));
    let revision = host.revision();
    assert_eq!(host.rotate_credential_guard(0, request).unwrap(), receipt);
    assert_eq!(host.revision(), revision);
    let current = host.bind_credential_pair(revision, &inventory(), &binding(), broker(b"two"), provider(b"two")).unwrap();
    let published = host.publish_checked_with_credential(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1), &current).unwrap();
    assert_eq!(published.outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(published.basis, PublicationBasis::Revalidated);
}

#[test]
fn conflicting_or_stale_rotation_cannot_change_generation_or_journal_revision() {
    let root = Directory::new(); let (mut host, _) = create(&root); enable(&mut host);
    let original = CredentialRotationRequest { operation: 9, expected_generation: 1, next_generation: 2 };
    host.rotate_credential_guard(host.revision(), original).unwrap();
    let revision = host.revision();
    assert_eq!(host.rotate_credential_guard(0, CredentialRotationRequest { operation: 9, expected_generation: 1, next_generation: 3 }),
        Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.rotate_credential_guard(revision, CredentialRotationRequest { operation: 10, expected_generation: 1, next_generation: 2 }),
        Err(JournalError::Contract(Error::Stale)));
    assert_eq!(host.revision(), revision);
    assert_eq!(host.credential_status().unwrap().unwrap().generation, 2);
}

#[test]
fn terminal_revocation_blocks_all_live_credentials_but_preserves_safe_sealing() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root); enable(&mut host);
    let keys = ready(&mut host, &reviewer, 1, b"revoked"); dispatch(&mut host, &keys);
    let old = host.bind_credential_pair(host.revision(), &inventory(), &binding(), broker(b"one"), provider(b"one")).unwrap();
    let request = CredentialRevocationRequest { operation: 77, expected_generation: 1 };
    let receipt = host.revoke_credential_guard(host.revision(), request).unwrap();
    assert!(receipt.revoked);
    assert!(matches!(host.bind_credential_pair(host.revision(), &inventory(), &binding(), broker(b"new"), provider(b"new")),
        Err(JournalError::Contract(Error::WrongState))));
    assert!(matches!(host.publish_checked_with_credential(host.revision(), 1, Some(&keys.inputs), snapshot(), ElapsedTick(1), &old),
        Err(JournalError::Contract(Error::WrongState))));
    let sealed = host.publish_checked(host.revision(), 1, None, Snapshot::default(), ElapsedTick(1)).unwrap();
    assert_eq!(sealed.outcome, EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed });
    assert_eq!(host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
    assert_eq!(host.inspect().control.ledger.available, 100);
    let revision = host.revision();
    assert_eq!(host.revoke_credential_guard(0, request).unwrap(), receipt);
    assert_eq!(host.revision(), revision);
}

#[test]
fn generation_and_terminal_revocation_survive_complete_file_authority_reopen() {
    let root = Directory::new(); let (mut host, _) = create(&root); enable(&mut host);
    let permit = host.bind_credential_pair(host.revision(), &inventory(), &binding(), broker(b"one"), provider(b"one")).unwrap();
    host.rotate_credential_guard(host.revision(), CredentialRotationRequest { operation: 1, expected_generation: 1, next_generation: 2 }).unwrap();
    host.revoke_credential_guard(host.revision(), CredentialRevocationRequest { operation: 2, expected_generation: 2 }).unwrap();
    drop(host);
    let (mut reopened, _) = FileOversight::open(root.store(), profile()).unwrap();
    let status = reopened.credential_status().unwrap().unwrap();
    assert_eq!(status.generation, 2); assert!(status.revoked); assert_eq!(status.retained_changes, 2);
    assert_eq!(reopened.credential_change(1).unwrap().generation, 2);
    assert!(reopened.credential_change(2).unwrap().revoked);
    assert!(matches!(reopened.publish_checked_with_credential(reopened.revision(), 999, None, Snapshot::default(), ElapsedTick(2), &permit),
        Err(JournalError::Contract(Error::Binding))));
}
