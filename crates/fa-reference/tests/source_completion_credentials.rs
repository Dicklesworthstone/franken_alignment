//! Source-acquired completion preserves the existing credential capability.
#![cfg(unix)]
#[path = "support/file_publication_capture.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, credential_broker::{
    BrokerCredential, BrokerRouteBinding, ProviderCredential, CredentialRotationRequest, CredentialRevocationRequest,
}};
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::credential::{FileCredentialPermit, FILE_OVERSIGHT_CREDENTIAL_PROFILE};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::completion::CapturedCompletionKeys;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;
use fa_reference::Error;

const SECRET: &[u8] = b"SOURCE-COMPLETION-SECRET-NOT-FOR-THE-JOURNAL";
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
fn credential(host: &FileOversight) -> FileCredentialPermit {
    host.bind_credential_pair(host.revision(), &inventory(), &binding(),
        BrokerCredential::new(SECRET.to_vec()).unwrap(), ProviderCredential::new(SECRET.to_vec()).unwrap()).unwrap()
}

#[test]
fn required_credential_is_owner_bound_and_valid_source_completion_keeps_secrets_out_of_replay() {
    let root = Directory::new();
    let (mut host, reviewer) = source_host(&root);
    host.enable_credential_guard(host.revision(), &inventory(), &binding()).unwrap();
    let ready = source_keys(&mut host, &reviewer, &root, 1);
    let permit = credential(&host);
    let before = host.inspect();
    let missing = host.complete_publication_from_source(host.revision(), CapturedCompletionKeys {
        automatic: &ready.automatic, human: &ready.human, credential: None,
    }, &source(&root), || panic!("missing credential cannot reach the clock"), |_, _| panic!("missing credential cannot capture"));
    assert_eq!(missing.result, Err(JournalError::Contract(Error::Incomplete)));
    assert!(missing.reads.is_empty());
    assert_eq!(host.inspect(), before);
    let other_root = Directory::new();
    let (mut other, _) = source_host(&other_root);
    other.enable_credential_guard(other.revision(), &inventory(), &binding()).unwrap();
    let foreign = credential(&other);
    let refused = host.complete_publication_from_source(host.revision(), CapturedCompletionKeys {
        automatic: &ready.automatic, human: &ready.human, credential: Some(&foreign),
    }, &source(&root), || panic!("foreign credential cannot reach the clock"), |_, _| panic!("foreign credential cannot capture"));
    assert_eq!(refused.result, Err(JournalError::Contract(Error::Binding)));
    assert!(refused.reads.is_empty());
    assert_eq!(host.inspect(), before);
    let completed = host.complete_publication_from_source(host.revision(), CapturedCompletionKeys {
        automatic: &ready.automatic, human: &ready.human, credential: Some(&permit),
    }, &source(&root), || ElapsedTick(2), |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: Some(ready.inputs.clone()) }));
    assert_eq!(completed.reads.len(), 2);
    assert_eq!(completed.result.unwrap().outcome, EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Confirmed);
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let bytes = std::fs::read(root.store().join("delivery.bin")).unwrap();
    assert!(!bytes.windows(SECRET.len()).any(|window| window == SECRET));
    assert_eq!(FileOversight::read_publication(root.store(), &profile()).unwrap(), host.inspect());
}

#[test]
fn credential_rotation_and_revocation_refuse_old_keys_before_any_source_read() {
    for revoke in [false, true] {
        let root = Directory::new();
        let (mut host, reviewer) = source_host(&root);
        host.enable_credential_guard(host.revision(), &inventory(), &binding()).unwrap();
        let ready = source_keys(&mut host, &reviewer, &root, 1);
        let permit = credential(&host);
        if revoke {
            host.revoke_credential_guard(host.revision(), CredentialRevocationRequest {
                operation: 7, expected_generation: permit.generation(),
            }).unwrap();
        } else {
            host.rotate_credential_guard(host.revision(), CredentialRotationRequest {
                operation: 7, expected_generation: permit.generation(), next_generation: permit.generation() + 1,
            }).unwrap();
        }
        let before = host.inspect();
        let refused = host.complete_publication_from_source(host.revision(), CapturedCompletionKeys {
            automatic: &ready.automatic, human: &ready.human, credential: Some(&permit),
        }, &source(&root), || panic!("withdrawn credential cannot reach the clock"), |_, _| panic!("withdrawn credential cannot capture"));
        assert_eq!(refused.result, Err(JournalError::Contract(if revoke { Error::WrongState } else { Error::Binding })));
        assert!(refused.reads.is_empty());
        assert_eq!(host.inspect(), before);
        assert!(host.storage_failure().is_none());
        assert_eq!(host.inspect().executions, 0);
    }
}
