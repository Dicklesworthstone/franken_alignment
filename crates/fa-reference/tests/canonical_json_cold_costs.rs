//! Fresh-process cold-parse measurements for the three manual FA-005 goldens.
//!
//! A normal all-tests run is not process-cold evidence. Only the root-selected
//! exact release invocation of one test per fresh process qualifies this narrow
//! measurement; it makes no OS page-cache or CPU-cold claim.

use std::time::Instant;

use fa_reference::canonical_json::{SchemaVariant, decode_canonical, encode};

const ACTION: &[u8] = include_bytes!("fixtures/canonical-json-v01/action.json");
const CAPABILITY: &[u8] = include_bytes!("fixtures/canonical-json-v01/capability.json");
const EVIDENCE: &[u8] = include_bytes!("fixtures/canonical-json-v01/evidence.json");

fn measure_once(bytes: &[u8], schema: &str, expected_variant: SchemaVariant) {
    let started = Instant::now();
    let document = decode_canonical(bytes).expect("manual fixture must decode canonically");
    let elapsed_ns = started.elapsed().as_nanos();

    assert_eq!(document.schema_variant(), expected_variant);
    assert_eq!(
        encode(&document),
        bytes,
        "decoded document changed fixture bytes"
    );
    println!(
        "FA005_CANONICAL_COLD schema={schema} bytes={} elapsed_ns={elapsed_ns} samples=1 proof_scope=fresh_process_exact_release_invocation_only no_claim=os_page_cache_or_cpu_cold",
        bytes.len(),
    );
}

#[test]
fn cold_decode_action_v01_manual_fixture() {
    measure_once(ACTION, "fa.action/0.1", SchemaVariant::ActionProposalV01);
}

#[test]
fn cold_decode_capability_v01_manual_fixture() {
    measure_once(
        CAPABILITY,
        "fa.capabilities/0.1",
        SchemaVariant::CapabilityManifestV01,
    );
}

#[test]
fn cold_decode_evidence_v01_manual_fixture() {
    measure_once(EVIDENCE, "fa.claim/0.1", SchemaVariant::EvidenceClaimV01);
}
