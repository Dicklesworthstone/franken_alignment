//! Public boundary tests for the strict FA-002 perimeter-inventory loader.
//!
//! These tests load a real, owned temporary regular file and inspect the
//! declared classification and per-route metadata together. A passing result
//! remains an inventory declaration only: it neither exercises an endpoint nor
//! establishes that a broker prevents any effect or bypass.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use fa_reference::{
    Error,
    perimeter::{BypassDisposition, Mediation, PerimeterScope, ThreatClass, TrustDomain},
    perimeter_inventory::{
        ActorCredentialDisposition, EffectKind, InventoryLoadError, LoadedPerimeterInventory,
        MAX_ROUTE_NONCLAIMS,
    },
    strict_json::ErrorKind,
};

static TEMP_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct OwnedTempDirectory(PathBuf);

impl OwnedTempDirectory {
    fn create() -> Self {
        let root = std::env::temp_dir();
        for _ in 0..64 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let directory = root.join(format!(
                "fa-reference-perimeter-inventory-{}-{sequence}",
                std::process::id()
            ));
            match fs::create_dir(&directory) {
                Ok(()) => return Self(directory),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create owned temporary directory: {error}"),
            }
        }
        panic!("could not allocate an owned temporary directory")
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).expect("write owned inventory fixture");
        path
    }

    fn missing(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for OwnedTempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn route_nonclaims(count: usize) -> String {
    (0..count)
        .map(|index| format!(r#""route-residual-{index}""#))
        .collect::<Vec<_>>()
        .join(",")
}

fn complete_inventory(route_nonclaim_count: usize) -> String {
    format!(
        r#"{{
  "version":1,
  "families":[{{
    "scope":{{"tenant":41,"principal":42,"purpose":43}},
    "family":"publication",
    "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
    "credentials":[{{"credential":"broker-only-publication","holder":"broker"}}],
    "routes":[{{
      "route":"adapter:owned-fixture",
      "effect":"network_request",
      "profile":{{"id":"reference-profile","generation":7}},
      "trust_path":["actor","enforcement"],
      "threat":"direct_credential_or_egress",
      "actor_credential":{{"kind":"broker_mediated"}},
      "mediation":"brokered_effects",
      "bypass":"blocked",
      "residual_nonclaims":[{}]
    }}],
    "residual_nonclaims":["No endpoint or broker prevention is proved"]
  }}]
}}"#,
        route_nonclaims(route_nonclaim_count)
    )
}

fn exact_scope() -> PerimeterScope {
    PerimeterScope {
        tenant: 41,
        principal: 42,
        purpose: 43,
    }
}

#[test]
fn owned_regular_file_loads_and_returns_exact_route_classification_and_metadata() {
    let temporary = OwnedTempDirectory::create();
    let path = temporary.file("inventory.json", complete_inventory(1).as_bytes());

    let loaded = LoadedPerimeterInventory::load_path(&path).expect("load owned regular file");
    let route = loaded
        .route_for(exact_scope(), "publication", "adapter:owned-fixture")
        .expect("look up exact declared route");

    assert_eq!(loaded.family_count(), 1);
    assert_eq!(route.record().mediation, Mediation::BrokeredEffects);
    assert_eq!(route.record().bypass, BypassDisposition::Blocked);
    assert_eq!(
        route.record().threat,
        Some(ThreatClass::DirectCredentialOrEgress)
    );
    assert_eq!(route.metadata().effect(), EffectKind::NetworkRequest);
    assert_eq!(route.metadata().profile().id(), "reference-profile");
    assert_eq!(route.metadata().profile().generation(), 7);
    assert_eq!(
        route.metadata().trust_path(),
        &[TrustDomain::Actor, TrustDomain::Enforcement]
    );
    assert!(matches!(
        route.metadata().actor_credential(),
        ActorCredentialDisposition::BrokerMediated
    ));
    assert_eq!(
        route.metadata().residual_nonclaims(),
        &["route-residual-0".to_owned()]
    );
}

#[test]
fn missing_and_nonregular_paths_refuse_before_any_inventory_is_returned() {
    let temporary = OwnedTempDirectory::create();

    assert_eq!(
        LoadedPerimeterInventory::load_path(&temporary.missing("absent.json")),
        Err(InventoryLoadError::Unreadable)
    );
    assert_eq!(
        LoadedPerimeterInventory::load_path(temporary.path()),
        Err(InventoryLoadError::Unreadable)
    );
}

#[test]
fn malformed_bytes_are_a_strict_syntax_refusal_not_a_schema_or_lookup_result() {
    let temporary = OwnedTempDirectory::create();
    let path = temporary.file("malformed.json", b"{");

    assert!(matches!(
        LoadedPerimeterInventory::load_path(&path),
        Err(InventoryLoadError::Parse(error)) if error.kind == ErrorKind::UnexpectedEof
    ));
}

#[test]
fn unknown_effect_and_unexpected_field_each_refuse_a_complete_control_document() {
    let unknown_effect = complete_inventory(1).replace(
        "\"effect\":\"network_request\"",
        "\"effect\":\"teleportation\"",
    );
    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(unknown_effect.as_bytes()),
        Err(InventoryLoadError::Schema(Error::InvalidInput))
    );

    let unexpected_field =
        complete_inventory(1).replace("\"version\":1,", "\"version\":1,\"unregistered\":true,");
    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(unexpected_field.as_bytes()),
        Err(InventoryLoadError::Schema(Error::InvalidInput))
    );
}

#[test]
fn missing_mandatory_route_field_refuses_without_reclassifying_the_record() {
    let missing_effect =
        complete_inventory(1).replace("      \"effect\":\"network_request\",\n", "");

    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(missing_effect.as_bytes()),
        Err(InventoryLoadError::Schema(Error::InvalidInput))
    );
}

#[test]
fn route_nonclaim_cap_passes_and_cap_plus_one_refuses() {
    let at_cap = complete_inventory(MAX_ROUTE_NONCLAIMS);
    let loaded = LoadedPerimeterInventory::from_json_bytes(at_cap.as_bytes())
        .expect("exact route residual cap remains admitted");
    assert_eq!(
        loaded
            .route_for(exact_scope(), "publication", "adapter:owned-fixture")
            .unwrap()
            .metadata()
            .residual_nonclaims()
            .len(),
        MAX_ROUTE_NONCLAIMS
    );

    let cap_plus_one = complete_inventory(MAX_ROUTE_NONCLAIMS + 1);
    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(cap_plus_one.as_bytes()),
        Err(InventoryLoadError::Schema(Error::Limit))
    );
}

#[test]
fn direct_actor_credential_refuses_the_same_brokered_family_profile() {
    let direct_credential = complete_inventory(1)
        .replace(
            "\"credential\":\"broker-only-publication\",\"holder\":\"broker\"",
            "\"credential\":\"actor-visible-publication\",\"holder\":\"actor_direct\"",
        )
        .replace(
            "\"actor_credential\":{\"kind\":\"broker_mediated\"}",
            "\"actor_credential\":{\"kind\":\"actor_direct\",\"credential\":\"actor-visible-publication\"}",
        );

    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(direct_credential.as_bytes()),
        Err(InventoryLoadError::Schema(Error::Binding))
    );
}

#[test]
fn unmodeled_bypass_refuses_the_same_brokered_route_profile() {
    let unmodeled =
        complete_inventory(1).replace("\"bypass\":\"blocked\"", "\"bypass\":\"unmodeled\"");

    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(unmodeled.as_bytes()),
        Err(InventoryLoadError::Schema(Error::Binding))
    );
}
