//! Descriptive regular-file load costs for the FA-002 perimeter inventory.
//!
//! These fixed samples assert their outcomes before printing elapsed time. They
//! measure neither endpoint mediation, operator authentication, OS page cache,
//! allocation/RSS, nor production broker performance.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

use fa_reference::{
    Error,
    perimeter::{BypassDisposition, Mediation, PerimeterScope, ThreatClass, TrustDomain},
    perimeter_inventory::{
        ActorCredentialDisposition, EffectKind, InventoryLoadError, LoadedPerimeterInventory,
        MAX_INVENTORY_BYTES, MAX_ROUTE_NONCLAIMS,
    },
};

const SAMPLES: usize = 4;
static FILE_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

struct OwnedRegularFile {
    path: PathBuf,
}

impl OwnedRegularFile {
    fn create(label: &str, bytes: &[u8]) -> Self {
        for _ in 0..64 {
            let sequence = FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fa-reference-perimeter-cost-{}-{sequence}-{label}.json",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    file.write_all(bytes)
                        .expect("write owned regular-file inventory sample");
                    file.sync_all()
                        .expect("flush owned regular-file inventory sample");
                    return Self { path };
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create owned regular-file inventory sample: {error}"),
            }
        }
        panic!("could not allocate an owned regular-file inventory sample");
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

fn scope() -> PerimeterScope {
    PerimeterScope {
        tenant: 61,
        principal: 62,
        purpose: 63,
    }
}

fn route_nonclaims(count: usize) -> String {
    (0..count)
        .map(|index| format!(r#""residual-{index}""#))
        .collect::<Vec<_>>()
        .join(",")
}

fn inventory_bytes(profile_generation: u64, mediation: &str, nonclaim_count: usize) -> Vec<u8> {
    format!(
        r#"{{
  "version":1,
  "families":[{{
    "scope":{{"tenant":61,"principal":62,"purpose":63}},
    "family":"publication",
    "trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],
    "credentials":[{{"credential":"broker-only-publication","holder":"broker"}}],
    "routes":[{{
      "route":"adapter:regular-file",
      "effect":"network_request",
      "profile":{{"id":"fixed-reference-profile","generation":{profile_generation}}},
      "trust_path":["actor","enforcement"],
      "threat":"direct_credential_or_egress",
      "actor_credential":{{"kind":"broker_mediated"}},
      "mediation":"{mediation}",
      "bypass":"blocked",
      "residual_nonclaims":[{}]
    }}],
    "residual_nonclaims":["no broker or endpoint prevention proof"]
  }}]
}}"#,
        route_nonclaims(nonclaim_count)
    )
    .into_bytes()
}

fn valid_inventory_at_exact_byte_cap() -> Vec<u8> {
    let mut bytes = inventory_bytes(7, "brokered_effects", 1);
    assert!(bytes.len() <= MAX_INVENTORY_BYTES);
    bytes.resize(MAX_INVENTORY_BYTES, b' ');
    bytes
}

fn assert_exact_loaded_route(
    loaded: &LoadedPerimeterInventory,
    expected: Mediation,
    generation: u64,
) {
    let route = loaded
        .route_for(scope(), "publication", "adapter:regular-file")
        .expect("exact regular-file route lookup");
    assert_eq!(route.record().mediation, expected);
    assert_eq!(route.record().bypass, BypassDisposition::Blocked);
    assert_eq!(
        route.record().threat,
        Some(ThreatClass::DirectCredentialOrEgress)
    );
    assert_eq!(route.metadata().effect(), EffectKind::NetworkRequest);
    assert_eq!(route.metadata().profile().id(), "fixed-reference-profile");
    assert_eq!(route.metadata().profile().generation(), generation);
    assert_eq!(
        route.metadata().trust_path(),
        &[TrustDomain::Actor, TrustDomain::Enforcement]
    );
    assert!(matches!(
        route.metadata().actor_credential(),
        ActorCredentialDisposition::BrokerMediated
    ));
}

fn run_file_case(
    case: &str,
    bytes: &[u8],
    expected_successes: usize,
    expected_refusals: usize,
    mut outcome: impl FnMut(Result<LoadedPerimeterInventory, InventoryLoadError>) -> (usize, usize),
) {
    let mut timings = Vec::with_capacity(SAMPLES);
    let mut successes = 0;
    let mut refusals = 0;
    for _ in 0..SAMPLES {
        let file = OwnedRegularFile::create(case, bytes);
        let started = Instant::now();
        let result = LoadedPerimeterInventory::load_path(file.path());
        timings.push(started.elapsed().as_nanos());
        let (sample_successes, sample_refusals) = outcome(result);
        successes += sample_successes;
        refusals += sample_refusals;
    }
    assert_eq!(
        successes,
        expected_successes * SAMPLES,
        "{case} dropped a success"
    );
    assert_eq!(
        refusals,
        expected_refusals * SAMPLES,
        "{case} dropped a refusal"
    );
    timings.sort_unstable();
    let median = (timings[SAMPLES / 2 - 1] + timings[SAMPLES / 2]) / 2;
    println!(
        "FA002_PERIMETER_INVENTORY_FILE_LOAD case={case} samples={SAMPLES} successes={successes} refusals={refusals} observed_errors=0 logical_input_bytes={} elapsed_ns_min={} elapsed_ns_median={} elapsed_ns_max={} elapsed_ns_total={} timing_scope=owned_regular_file_open_bounded_read_strict_parse_inventory_validation_only_lookup_assertions_drop_excluded memory_measurement=unavailable_reference_profile no_claim=endpoint_broker_authentication_or_os_cache_cold",
        bytes.len(),
        timings[0],
        median,
        timings[SAMPLES - 1],
        timings.iter().sum::<u128>(),
    );
}

#[test]
fn fixed_regular_file_inventory_load_costs_assert_lookup_churn_failures_and_bounds() {
    let baseline = inventory_bytes(7, "brokered_effects", 1);
    run_file_case("baseline_brokered_lookup", &baseline, 1, 0, |result| {
        let loaded = result.expect("baseline regular-file inventory must load");
        assert_exact_loaded_route(&loaded, Mediation::BrokeredEffects, 7);
        (1, 0)
    });

    let declared_churn = inventory_bytes(8, "cooperative_gate", 1);
    run_file_case(
        "declared_mediation_profile_churn",
        &declared_churn,
        1,
        0,
        |result| {
            let loaded = result.expect("changed declared profile remains parseable inventory data");
            assert_exact_loaded_route(&loaded, Mediation::CooperativeGate, 8);
            (1, 0)
        },
    );

    run_file_case("malformed_syntax", b"{", 0, 1, |result| {
        assert!(matches!(result, Err(InventoryLoadError::Parse(_))));
        (0, 1)
    });

    let at_cap = inventory_bytes(7, "brokered_effects", MAX_ROUTE_NONCLAIMS);
    run_file_case("route_nonclaims_at_cap", &at_cap, 1, 0, |result| {
        let loaded = result.expect("route residual nonclaims at the exact cap must load");
        let route = loaded
            .route_for(scope(), "publication", "adapter:regular-file")
            .expect("exact loaded route at nonclaim cap");
        assert_eq!(
            route.metadata().residual_nonclaims().len(),
            MAX_ROUTE_NONCLAIMS
        );
        (1, 0)
    });

    let one_over = inventory_bytes(7, "brokered_effects", MAX_ROUTE_NONCLAIMS + 1);
    run_file_case("route_nonclaims_one_over", &one_over, 0, 1, |result| {
        assert_eq!(result, Err(InventoryLoadError::Schema(Error::Limit)));
        (0, 1)
    });

    let exact_cap = valid_inventory_at_exact_byte_cap();
    run_file_case(
        "valid_json_exact_input_byte_cap",
        &exact_cap,
        1,
        0,
        |result| {
            let loaded = result.expect("valid JSON padded to the exact byte cap must load");
            assert_exact_loaded_route(&loaded, Mediation::BrokeredEffects, 7);
            (1, 0)
        },
    );
    let mut one_past_valid_cap = exact_cap;
    one_past_valid_cap.push(b' ');
    run_file_case(
        "same_valid_json_one_byte_over_input_cap",
        &one_past_valid_cap,
        0,
        1,
        |result| {
            assert_eq!(result, Err(InventoryLoadError::Schema(Error::Limit)));
            (0, 1)
        },
    );

    let oversized = vec![b' '; MAX_INVENTORY_BYTES + 1];
    run_file_case("input_bytes_one_over", &oversized, 0, 1, |result| {
        assert_eq!(result, Err(InventoryLoadError::Schema(Error::Limit)));
        (0, 1)
    });
}

/// This test is intended for central selection as one exact test in a fresh
/// process. The owned file is created before timing; there is no loader call
/// before `Instant::now`. A normal suite run is not OS/page-cache/CPU-cold
/// evidence, and this reference loader does not prove an endpoint or broker.
#[test]
fn first_regular_file_load_is_selectable_for_fresh_process_measurement() {
    let bytes = inventory_bytes(7, "brokered_effects", 1);
    let file = OwnedRegularFile::create("first-load", &bytes);
    let started = Instant::now();
    let loaded = LoadedPerimeterInventory::load_path(file.path());
    let elapsed = started.elapsed().as_nanos();
    let loaded = loaded.expect("first owned regular-file inventory load must succeed");
    assert_exact_loaded_route(&loaded, Mediation::BrokeredEffects, 7);
    println!(
        "FA002_PERIMETER_INVENTORY_FILE_LOAD case=first_regular_file_load samples=1 successes=1 refusals=0 observed_errors=0 logical_input_bytes={} elapsed_ns={elapsed} timing_scope=fresh_process_exact_release_invocation_only_owned_regular_file_open_bounded_read_strict_parse_inventory_validation_only_lookup_assertions_drop_excluded_no_loader_preflight memory_measurement=unavailable_reference_profile no_claim=os_page_cache_cpu_cold_endpoint_or_broker",
        bytes.len(),
    );
}
