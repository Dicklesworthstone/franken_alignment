//! Real child-process boundaries for Cargo metadata collection.
//!
//! These tests intentionally call the collector's actual `cargo` subprocess.
//! They do not replace Cargo, alter `PATH`, or make a fixture stand in for the
//! collection boundary. The central RCH test batch is responsible for running
//! them against the frozen workspace.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{collect_metadata, collect_metadata_using, json};

const COLLECTION_TARGET: &str = "aarch64-apple-darwin";

static NEXT_TEMPORARY_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(label: &str) -> Self {
        for _ in 0..128 {
            let serial = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "franken_alignment_metadata_collection_tests-{label}-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self { path },
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create owned temporary directory {path:?}: {error}"),
            }
        }
        panic!("unable to create a unique metadata-collection test directory");
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.path)
            .unwrap_or_else(|error| panic!("cannot remove owned temporary directory: {error}"));
    }
}

fn workspace_root() -> PathBuf {
    crate::workspace_root().expect("actual invocation workspace")
}

fn package_names(document: &json::Json) -> BTreeSet<String> {
    document
        .get("packages")
        .and_then(json::Json::as_array)
        .expect("cargo metadata must provide a packages array")
        .iter()
        .map(|package| {
            package
                .get("name")
                .and_then(json::Json::as_str)
                .expect("every collected package must carry a string name")
                .to_owned()
        })
        .collect()
}

#[test]
fn real_cargo_metadata_reports_the_complete_workspace_without_implying_admission() {
    let root = workspace_root();

    let bytes = collect_metadata(&root, COLLECTION_TARGET)
        .expect("real Cargo metadata must collect the current locked offline workspace");
    let document = json::parse(&bytes, json::Limits::default())
        .expect("real Cargo metadata stdout must be valid bounded JSON");
    let packages = document
        .get("packages")
        .and_then(json::Json::as_array)
        .expect("cargo metadata must provide a packages array");

    assert_eq!(
        packages.len(),
        3,
        "collection must expose every actual package, including unadmitted local source"
    );
    assert_eq!(
        package_names(&document),
        BTreeSet::from([
            "fa-journal".to_owned(),
            "fa-reference".to_owned(),
            "xtask".to_owned(),
        ]),
        "a collector must not hide the journal to make the admission inventory appear closed"
    );

    let package_id = |name| {
        packages
            .iter()
            .find(|package| package.get("name").and_then(json::Json::as_str) == Some(name))
            .and_then(|package| package.get("id"))
            .and_then(json::Json::as_str)
            .expect("actual package ID")
    };
    let journal = document
        .get("resolve")
        .and_then(|resolve| resolve.get("nodes"))
        .and_then(json::Json::as_array)
        .expect("complete resolved graph")
        .iter()
        .find(|node| node.get("id").and_then(json::Json::as_str) == Some(package_id("fa-journal")))
        .expect("the journal must remain visible as a resolved node");
    assert_eq!(
        journal.get("dependencies"),
        Some(&json::Json::Array(vec![json::Json::String(
            package_id("fa-reference").to_owned()
        ),])),
        "collection must retain the actual journal-to-reference dependency"
    );

    // Collection is observation, not permission. An independently frozen
    // two-package profile must still refuse this actual three-package graph.
    let closed = super::admission::parse_policy(include_bytes!(
        "../tests/fixtures/admission/policy-closed-inventory.json"
    ))
    .expect("the original closed profile remains a valid policy");
    let report = super::admission::check(&bytes, &closed, COLLECTION_TARGET)
        .expect("the real resolved graph must reach admission");
    assert!(
        !report.is_admitted(),
        "observing an extra local package cannot admit it"
    );
    assert!(
        report.findings.iter().any(
            |finding| finding.phase == super::admission::Phase::Inventory
                && finding.code == "local_package_not_admitted"
        ),
        "the journal must be refused at the package boundary: {:?}",
        report.findings
    );
}

#[test]
fn real_cargo_metadata_refuses_manifestless_current_directory_without_empty_success() {
    let directory = TemporaryDirectory::new("manifestless");

    let error = collect_metadata(&directory.path, COLLECTION_TARGET).expect_err(
        "real Cargo must reject a directory without Cargo.toml, never return empty metadata",
    );

    assert!(
        error.starts_with("Metadata collection for aarch64-apple-darwin failed:"),
        "a Cargo nonzero status must remain a collection failure: {error}"
    );
    assert!(
        error.contains("Unknown"),
        "a Cargo nonzero status must remain epistemically Unknown: {error}"
    );
}

#[test]
fn real_cargo_metadata_refuses_missing_root_without_empty_success() {
    let directory = TemporaryDirectory::new("missing-root-parent");
    let missing_root = directory.path.join("absent-root");

    let error = collect_metadata(&missing_root, COLLECTION_TARGET)
        .expect_err("a missing current directory must not produce default metadata");

    assert!(
        error.starts_with("Cannot collect metadata for aarch64-apple-darwin:"),
        "failure to start Cargo in a missing root must remain visible: {error}"
    );
    assert!(
        error.contains("Unknown"),
        "failure to start Cargo must remain epistemically Unknown: {error}"
    );
}

#[test]
fn absent_metadata_executable_is_unknown_and_refuses_closed_collection() {
    let directory = TemporaryDirectory::new("absent-executable");
    let absent_program = directory.path.join("missing-metadata-executable");
    assert!(
        !absent_program.exists(),
        "the real-OS negative requires an executable path that is actually absent"
    );

    let error = collect_metadata_using(
        &workspace_root(),
        COLLECTION_TARGET,
        absent_program.as_os_str(),
    )
    .expect_err("an absent metadata executable must not produce default metadata");

    assert!(
        error.starts_with("Cannot collect metadata for aarch64-apple-darwin:"),
        "an absent executable must preserve the collection-failure prefix: {error}"
    );
    assert!(
        error.contains("Unknown"),
        "an absent executable must fail closed as Unknown, not as empty metadata: {error}"
    );
    assert!(
        error.contains("spawn"),
        "an absent executable must retain the spawn-absence reason: {error}"
    );
    assert!(
        error.contains(absent_program.to_string_lossy().as_ref()),
        "the spawn failure must identify the actual absent executable: {error}"
    );
}
