#![allow(dead_code)]
#[path = "file_oversight.rs"] pub mod ordinary;
#[path = "file_identity.rs"] pub mod identity;

pub use ordinary::Directory;
use fa_reference::action::consequence::delivery::credential_broker::{BrokerCredential, BrokerRouteBinding, ProviderCredential};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile};
use fa_reference::action::consequence::delivery::persistent::observed::credential::{FileCredentialPermit, FileCredentialPolicy, FILE_OVERSIGHT_CREDENTIAL_PROFILE};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::action::ElapsedTick;
use fa_reference::perimeter_inventory::LoadedPerimeterInventory;

pub fn guards() -> FileGuardSet {
    FileGuardSet {
        stream: None, decoder: None, source: None, credential: None,
        identity: Some(FileIdentityRequirement { passport: identity::passport(), policy: identity::policy() }),
        campaigns: Some(FileCampaignRequirement {
            limits: ReplayLimits { cases: 32, input_bytes: 1_048_576 }, max_campaigns: 8,
        }),
    }
}
pub fn source_policy() -> FileSourcePolicy {
    FileSourcePolicy {
        source: StateSource { scope: ordinary::profile().delivery.scope, source: 17, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(50).unwrap(),
    }
}
pub fn credential_policy() -> FileCredentialPolicy {
    FileCredentialPolicy { family: "publication".into(), route: "adapter:file-oversight".into(),
        credential: "oversight-token".into(), profile_generation: 1 }
}
pub fn binding() -> BrokerRouteBinding {
    BrokerRouteBinding { family: "publication".into(), route: "adapter:file-oversight".into() }
}
pub fn inventory() -> LoadedPerimeterInventory {
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
pub fn all_guards() -> FileGuardSet {
    let mut result = guards();
    result.stream = Some(StreamProfile::new(7, 1, 4, 1024, 4096).unwrap());
    result.source = Some(source_policy());
    result.credential = Some(credential_policy());
    result
}
pub fn profile_for(guards: &FileGuardSet) -> FileOversightProfile {
    let mut p = ordinary::profile();
    if guards.stream.is_some() { p.delivery.initial_payload.clear(); p.delivery.total = 4096; }
    p
}
pub fn create(root: &Directory, guards: &FileGuardSet) -> (FileOversight, FileOversightRoles) {
    let profile = profile_for(guards);
    let (mut host, human) = match guards.stream {
        Some(stream) => FileOversight::create_stream(root.store(), profile, stream).unwrap(),
        None => FileOversight::create(root.store(), profile).unwrap(),
    };
    if guards.stream.is_none() { host.enable_publication_guard(host.revision()).unwrap(); }
    if let Some(source) = guards.source { host.enable_file_source(host.revision(), source).unwrap(); }
    let identity_observer = guards.identity.as_ref().map(|expected|
        host.enable_identity_checks(host.revision(), expected.passport.clone(), expected.policy).unwrap());
    let policy_governor = guards.campaigns.map(|expected|
        host.enable_policy_campaigns(host.revision(), expected.limits, expected.max_campaigns).unwrap());
    if let Some(expected) = &guards.credential {
        assert_eq!(host.enable_credential_guard(host.revision(), &inventory(), &binding()).unwrap(), *expected);
    }
    if let Some(config) = &guards.decoder { host.enable_decoder(host.revision(), config.clone()).unwrap(); }
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, FileOversightRoles { human, identity_observer, policy_governor })
}
/// Simulate an operator retaining the requirements while the original owner is
/// healthy. The guard inventory itself is independently constructed above.
pub fn requirements(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let state = host.inspect();
    FileRecoveryRequirements {
        guards, effective_policy: host.current_policy().unwrap().clone(),
        credential_epoch: host.credential_status().unwrap().map(|status| FileCredentialEpoch {
            generation: status.generation, revoked: status.revoked,
        }),
        minimum: FileRecoveryFloor { journal_revision: state.revision,
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch },
    }
}
pub fn credential(host: &FileOversight) -> FileCredentialPermit {
    host.bind_credential_pair(host.revision(), &inventory(), &binding(),
        BrokerCredential::new(b"guarded-fixture".to_vec()).unwrap(),
        ProviderCredential::new(b"guarded-fixture".to_vec()).unwrap()).unwrap()
}
