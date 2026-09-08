//! Public count and text-limit boundaries for the FA-002 perimeter loader.
//!
//! These are checked inventory declarations only. They do not exercise a
//! broker, an endpoint, operator authentication, or production enforcement.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use fa_reference::{
    Error,
    perimeter::{
        MAX_CREDENTIALS_PER_FAMILY, MAX_FAMILIES, MAX_FAMILY_TEXT_BYTES, MAX_RESIDUAL_NONCLAIMS,
        MAX_ROUTES_PER_FAMILY, MAX_TEXT_BYTES, PerimeterScope,
    },
    perimeter_inventory::{InventoryLoadError, LoadedPerimeterInventory},
};

static FILE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct OwnedRegularFile {
    path: PathBuf,
}

impl OwnedRegularFile {
    fn create(bytes: &[u8]) -> Self {
        for _ in 0..64 {
            let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fa-reference-perimeter-limits-{}-{sequence}.json",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(bytes)
                        .expect("write owned regular-file inventory");
                    file.sync_all().expect("flush owned regular-file inventory");
                    return Self { path };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create owned regular-file inventory: {error}"),
            }
        }
        panic!("could not allocate owned regular-file inventory")
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for OwnedRegularFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn scope(index: usize) -> PerimeterScope {
    PerimeterScope {
        tenant: 71,
        principal: 72,
        purpose: u64::try_from(index + 1).expect("bounded family index fits u64"),
    }
}

fn route_json(index: usize) -> String {
    format!(
        r#"{{"route":"route-{index}","effect":"network_request","profile":{{"id":"profile","generation":1}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["route-residual"]}}"#
    )
}

fn family_json(index: usize, route_count: usize) -> String {
    let routes = (0..route_count)
        .map(route_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"scope":{{"tenant":71,"principal":72,"purpose":{}}},"family":"family-{index}","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],"credentials":[{{"credential":"broker-credential","holder":"broker"}}],"routes":[{routes}],"residual_nonclaims":["family-residual"]}}"#,
        index + 1
    )
}

fn inventory_json(family_count: usize, route_count: usize) -> Vec<u8> {
    let families = (0..family_count)
        .map(|index| family_json(index, route_count))
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"version":1,"families":[{families}]}}"#).into_bytes()
}

fn text_boundary_family_json(family: &str) -> Vec<u8> {
    format!(
        r#"{{"version":1,"families":[{{"scope":{{"tenant":71,"principal":72,"purpose":1}},"family":"{family}","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],"credentials":[{{"credential":"broker","holder":"broker"}}],"routes":[{{"route":"route","effect":"network_request","profile":{{"id":"profile","generation":1}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["route-residual"]}}],"residual_nonclaims":["family-residual"]}}]}}"#
    )
    .into_bytes()
}

fn numbered_text(prefix: &str, index: usize, length: usize) -> String {
    let suffix = format!("{prefix}-{index}");
    assert!(suffix.len() <= length);
    format!("{suffix}{}", "x".repeat(length - suffix.len()))
}

fn aggregate_family_json(family_name_length: usize) -> Vec<u8> {
    const CREDENTIAL_COUNT: usize = MAX_CREDENTIALS_PER_FAMILY - 1;
    let credentials = (0..CREDENTIAL_COUNT)
        .map(|index| {
            format!(
                r#"{{"credential":"{}","holder":"broker"}}"#,
                numbered_text("credential", index, MAX_TEXT_BYTES)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let family_nonclaims = (0..MAX_RESIDUAL_NONCLAIMS)
        .map(|index| format!(r#""{}""#, numbered_text("nonclaim", index, MAX_TEXT_BYTES)))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"version":1,"families":[{{"scope":{{"tenant":71,"principal":72,"purpose":1}},"family":"{}","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],"credentials":[{credentials}],"routes":[{{"route":"r","effect":"network_request","profile":{{"id":"p","generation":1}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated"}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["n"]}}],"residual_nonclaims":[{family_nonclaims}]}}]}}"#,
        "f".repeat(family_name_length)
    )
    .into_bytes()
}

#[test]
fn route_count_at_cap_loads_and_one_over_refuses_via_public_bytes_api() {
    let at_cap = inventory_json(1, MAX_ROUTES_PER_FAMILY);
    let loaded = LoadedPerimeterInventory::from_json_bytes(&at_cap)
        .expect("independently generated route count at the exact cap must load");
    assert!(
        loaded
            .route_for(
                scope(0),
                "family-0",
                &format!("route-{}", MAX_ROUTES_PER_FAMILY - 1)
            )
            .is_ok()
    );

    let one_over = inventory_json(1, MAX_ROUTES_PER_FAMILY + 1);
    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(&one_over),
        Err(InventoryLoadError::Schema(Error::Limit))
    );
}

#[test]
fn family_count_at_cap_loads_and_one_over_refuses_via_public_file_api() {
    let at_cap = inventory_json(MAX_FAMILIES, 1);
    let file = OwnedRegularFile::create(&at_cap);
    let loaded = LoadedPerimeterInventory::load_path(file.path())
        .expect("independently generated family count at the exact cap must load");
    assert_eq!(loaded.family_count(), MAX_FAMILIES);
    assert!(
        loaded
            .route_for(
                scope(MAX_FAMILIES - 1),
                &format!("family-{}", MAX_FAMILIES - 1),
                "route-0"
            )
            .is_ok()
    );

    let one_over = inventory_json(MAX_FAMILIES + 1, 1);
    let file = OwnedRegularFile::create(&one_over);
    assert_eq!(
        LoadedPerimeterInventory::load_path(file.path()),
        Err(InventoryLoadError::Schema(Error::Limit))
    );
}

#[test]
fn individual_text_cap_and_one_over_refuse_with_same_generated_family() {
    let at_cap = text_boundary_family_json(&"f".repeat(MAX_TEXT_BYTES));
    assert!(LoadedPerimeterInventory::from_json_bytes(&at_cap).is_ok());

    let one_over = text_boundary_family_json(&"f".repeat(MAX_TEXT_BYTES + 1));
    // The strict reader enforces its string budget before schema admission.
    assert!(matches!(
        LoadedPerimeterInventory::from_json_bytes(&one_over),
        Err(InventoryLoadError::Parse(error))
            if error.kind == fa_reference::strict_json::ErrorKind::StringLimit
    ));
}

#[test]
fn aggregate_family_text_at_cap_loads_and_one_over_refuses() {
    const CREDENTIAL_COUNT: usize = MAX_CREDENTIALS_PER_FAMILY - 1;
    let at_cap_family_name_length = MAX_FAMILY_TEXT_BYTES
        - CREDENTIAL_COUNT * MAX_TEXT_BYTES
        - MAX_RESIDUAL_NONCLAIMS * MAX_TEXT_BYTES
        - 3;
    assert_eq!(
        at_cap_family_name_length
            + CREDENTIAL_COUNT * MAX_TEXT_BYTES
            + MAX_RESIDUAL_NONCLAIMS * MAX_TEXT_BYTES
            + 3,
        MAX_FAMILY_TEXT_BYTES
    );
    assert!(at_cap_family_name_length <= MAX_TEXT_BYTES);

    let at_cap = aggregate_family_json(at_cap_family_name_length);
    assert!(LoadedPerimeterInventory::from_json_bytes(&at_cap).is_ok());

    let one_over = aggregate_family_json(at_cap_family_name_length + 1);
    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(&one_over),
        Err(InventoryLoadError::Schema(Error::Limit))
    );
}
