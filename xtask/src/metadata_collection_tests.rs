//! Real child-process boundaries for Cargo metadata collection.
//!
//! These tests intentionally call the collector's actual `cargo` subprocess.
//! They do not replace Cargo, alter `PATH`, or make a fixture stand in for the
//! collection boundary. The central RCH test batch is responsible for running
//! them against the frozen workspace.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
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
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask manifest must have a workspace parent")
        .to_path_buf()
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
fn real_cargo_metadata_for_current_workspace_parses_to_closed_package_inventory() {
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
        2,
        "the real metadata inventory must not contain an unapproved extra package"
    );
    assert_eq!(
        package_names(&document),
        BTreeSet::from(["fa-reference".to_owned(), "xtask".to_owned()]),
        "the real metadata collection must expose exactly the closed local package inventory"
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
