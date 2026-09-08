//! Independent real-operator tests for source snapshot verification.
//!
//! The baseline manifests are produced by the host SHA-256 utility, not by a
//! digest fixture or a test-local hash implementation. Each negative starts
//! from that verified byte identity and makes exactly one source-tree or
//! manifest mutation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::source_snapshot::{Limits, Report, parse_manifest, verify, verify_manifest_identity};

const SCHEMA: &str = "fa.source_snapshot/1";

#[test]
fn review_regression_operator_digest_rejects_non_regular_inputs_before_hashing() {
    let sandbox = Sandbox::new();
    let relative = "operator-input";
    let path = sandbox.root.join(relative);
    sandbox.write(relative, "reviewed regular bytes\n");
    assert_eq!(
        super::source_snapshot::operator_file_digest(&sandbox.root, relative),
        Ok(operator_sha256(&path))
    );
    fs::remove_file(&path).expect("replace only owned fixture input");
    fs::create_dir(&path).expect("create non-regular fixture at identical path");
    let error = super::source_snapshot::operator_file_digest(&sandbox.root, relative)
        .expect_err("directories and special files are not hashable operator input");
    assert!(error.contains("not_regular_file"), "{error}");
    assert!(!error.contains("hash_tool_failed"), "{error}");
}

const CLAIMS: &[u8] = include_bytes!("../../registry/claims.json");
const FOUNDING_CONCORDANCE: &[u8] = include_bytes!("../../registry/founding_concordance.json");
const INVARIANTS: &[u8] = include_bytes!("../../registry/invariants.json");
const ROADMAP: &[u8] = include_bytes!("../../registry/roadmap.json");
const SOURCES: &[u8] = include_bytes!("../../registry/sources.json");
const VOCABULARY: &[u8] = include_bytes!("../../registry/vocabulary.json");

const EMBEDDED_INPUTS: &[&str] = &[
    "registry/claims.json",
    "registry/founding_concordance.json",
    "registry/invariants.json",
    "registry/roadmap.json",
    "registry/sources.json",
    "registry/vocabulary.json",
];

const SNAPSHOT_PATHS: &[&str] = &[
    "COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md",
    "Cargo.lock",
    "Cargo.toml",
    "LICENSE",
    "crates/fa-reference/Cargo.toml",
    "crates/fa-reference/src/lib.rs",
    "crates/fa-reference/tests/reference.rs",
    "registry/claims.json",
    "registry/founding_concordance.json",
    "registry/invariants.json",
    "registry/roadmap.json",
    "registry/sources.json",
    "registry/vocabulary.json",
    "rust-toolchain.toml",
    "xtask/Cargo.toml",
    "xtask/src/main.rs",
    "xtask/tests/reference.rs",
];

static NEXT_SANDBOX: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    base: PathBuf,
    root: PathBuf,
    outside: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        for _ in 0..128 {
            let serial = NEXT_SANDBOX.fetch_add(1, Ordering::Relaxed);
            let base = std::env::temp_dir().join(format!(
                "franken_alignment_source_snapshot_tests-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&base) {
                Ok(()) => {
                    let root = base.join("root");
                    let outside = base.join("outside");
                    fs::create_dir(&root).expect("create owned test root");
                    fs::create_dir(&outside).expect("create owned test outside directory");
                    let sandbox = Self {
                        base,
                        root,
                        outside,
                    };
                    sandbox.write_baseline();
                    return sandbox;
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create source snapshot sandbox {base:?}: {error}"),
            }
        }
        panic!("cannot allocate an owned source snapshot sandbox");
    }

    fn write_baseline(&self) {
        for (path, contents) in [
            (
                "COMPREHENSIVE_PLAN_FOR_THE_DESIGN_OF_FRANKENALIGNMENT.md",
                "## 2. Test plan contract\n",
            ),
            ("Cargo.toml", "[workspace]\nmembers = []\n"),
            ("Cargo.lock", "version = 4\n"),
            (
                "rust-toolchain.toml",
                "[toolchain]\nchannel = \"nightly\"\n",
            ),
            ("LICENSE", "snapshot test license\n"),
            (
                "crates/fa-reference/Cargo.toml",
                "[package]\nname = \"fa-reference\"\n",
            ),
            ("crates/fa-reference/src/lib.rs", "pub fn reference() {}\n"),
            (
                "crates/fa-reference/tests/reference.rs",
                "#[test]\nfn reference() {}\n",
            ),
            ("xtask/Cargo.toml", "[package]\nname = \"xtask\"\n"),
            ("xtask/src/main.rs", "fn main() {}\n"),
            ("xtask/tests/reference.rs", "#[test]\nfn reference() {}\n"),
        ] {
            self.write(path, contents);
        }
        for (path, contents) in [
            ("registry/claims.json", CLAIMS),
            ("registry/founding_concordance.json", FOUNDING_CONCORDANCE),
            ("registry/invariants.json", INVARIANTS),
            ("registry/roadmap.json", ROADMAP),
            ("registry/sources.json", SOURCES),
            ("registry/vocabulary.json", VOCABULARY),
        ] {
            self.write_bytes(path, contents);
        }
    }

    fn write(&self, relative: &str, contents: &str) {
        self.write_bytes(relative, contents.as_bytes());
    }

    fn write_bytes(&self, relative: &str, contents: &[u8]) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("sandbox file has a parent"))
            .expect("create sandbox file parent");
        fs::write(path, contents).expect("write sandbox source input");
    }

    fn manifest_bytes(&self) -> Vec<u8> {
        let entries = SNAPSHOT_PATHS
            .iter()
            .map(|relative| {
                format!(
                    "\"{relative}\":\"{}\"",
                    operator_sha256(&self.root.join(relative))
                )
            })
            .collect::<Vec<_>>();
        format!(
            "{{\"schema\":\"{SCHEMA}\",\"files\":{{{}}}}}",
            entries.join(",")
        )
        .into_bytes()
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).expect("remove owned source snapshot sandbox");
    }
}

fn operator_sha256(path: &Path) -> String {
    #[cfg(target_os = "macos")]
    let command: (&str, &[&str]) = ("shasum", &["-a", "256"]);
    #[cfg(not(target_os = "macos"))]
    let command: (&str, &[&str]) = ("sha256sum", &[]);

    let output = Command::new(command.0)
        .args(command.1)
        .arg(path)
        .output()
        .expect("the host operator SHA-256 tool must start");
    assert!(
        output.status.success(),
        "the host operator SHA-256 tool must succeed for a regular sandbox file: {:?}",
        output.status
    );
    let stdout = String::from_utf8(output.stdout)
        .expect("the host operator SHA-256 tool must emit UTF-8 for an ASCII path");
    let digest = stdout
        .split_ascii_whitespace()
        .next()
        .expect("the host operator SHA-256 output must include a digest");
    assert!(
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "the host operator SHA-256 tool emitted an unexpected digest: {digest:?}"
    );
    digest.to_ascii_lowercase()
}

fn verified_manifest(bytes: &[u8]) -> super::source_snapshot::Manifest {
    parse_manifest(bytes, Limits::default()).expect("real operator-generated manifest must parse")
}

fn verify_sandbox(sandbox: &Sandbox, manifest: &[u8]) -> Report {
    verify(
        &sandbox.root,
        &verified_manifest(manifest),
        Limits::default(),
    )
    .expect("a valid manifest must reach source discovery and hashing")
}

fn assert_refusal(report: &Report, code: &str, path: &str) {
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == code && finding.path == path),
        "expected {code} for {path}; got {:?}",
        report.findings
    );
    assert!(
        !report.is_verified(),
        "a source-snapshot finding must refuse verification"
    );
}

#[cfg(unix)]
fn move_parent_outside_then_symlink(sandbox: &Sandbox, parent: &str) {
    use std::os::unix::fs::symlink;

    let original = sandbox.root.join(parent);
    let outside = sandbox.outside.join(parent);
    fs::rename(&original, &outside).expect("move the complete owned source parent outside root");
    symlink(&outside, &original).expect("replace original parent with an escaping symlink");
}

#[test]
fn real_operator_hashes_verify_exact_baseline_source_bytes() {
    let sandbox = Sandbox::new();
    let manifest = sandbox.manifest_bytes();

    let report = verify_sandbox(&sandbox, &manifest);

    assert!(
        report.findings.is_empty(),
        "baseline findings: {:?}",
        report.findings
    );
    assert!(
        report.is_verified(),
        "the exact real-hash baseline must verify"
    );
}

#[test]
fn changed_source_bytes_are_a_digest_mismatch() {
    let sandbox = Sandbox::new();
    let manifest = sandbox.manifest_bytes();
    sandbox.write("xtask/src/main.rs", "fn main() { changed(); }\n");

    assert_refusal(
        &verify_sandbox(&sandbox, &manifest),
        "digest_mismatch",
        "xtask/src/main.rs",
    );
}

#[test]
fn unlisted_new_rust_source_is_a_discovery_refusal() {
    let sandbox = Sandbox::new();
    let manifest = sandbox.manifest_bytes();
    sandbox.write("xtask/src/unlisted.rs", "pub fn newly_added() {}\n");

    assert_refusal(
        &verify_sandbox(&sandbox, &manifest),
        "unlisted_file",
        "xtask/src/unlisted.rs",
    );
}

#[test]
fn missing_listed_file_is_a_discovery_refusal() {
    let sandbox = Sandbox::new();
    let manifest = sandbox.manifest_bytes();
    fs::remove_file(sandbox.root.join("crates/fa-reference/tests/reference.rs"))
        .expect("remove exactly one listed source input");

    assert_refusal(
        &verify_sandbox(&sandbox, &manifest),
        "missing_file",
        "crates/fa-reference/tests/reference.rs",
    );
}

#[cfg(unix)]
#[test]
fn symlinked_source_escaping_root_is_refused_without_hashing_its_target() {
    use std::os::unix::fs::symlink;

    let sandbox = Sandbox::new();
    let manifest = sandbox.manifest_bytes();
    let target = sandbox.outside.join("outside.rs");
    fs::write(&target, "pub fn outside() {}\n").expect("write symlink target outside root");
    let source = sandbox.root.join("xtask/src/main.rs");
    fs::remove_file(&source).expect("replace one listed source input with a symlink");
    symlink(&target, &source).expect("create source symlink escaping root");

    assert_refusal(
        &verify_sandbox(&sandbox, &manifest),
        "symlink_rejected",
        "xtask/src/main.rs",
    );
}

#[cfg(unix)]
#[test]
fn parent_symlink_to_identical_crate_bytes_is_refused_without_verifying_outside_source() {
    let sandbox = Sandbox::new();
    let manifest = sandbox.manifest_bytes();
    move_parent_outside_then_symlink(&sandbox, "crates");

    let report = verify_sandbox(&sandbox, &manifest);

    assert_refusal(&report, "symlink_rejected", "crates/fa-reference/src");
    assert!(
        !report
            .verified
            .iter()
            .any(|path| path.starts_with("crates/")),
        "source bytes reachable only through the escaping crates symlink must not verify: {:?}",
        report.verified
    );
}

#[cfg(unix)]
#[test]
fn parent_symlink_to_identical_registry_bytes_refuses_every_embedded_input() {
    let sandbox = Sandbox::new();
    let manifest = sandbox.manifest_bytes();
    move_parent_outside_then_symlink(&sandbox, "registry");

    let report = verify_sandbox(&sandbox, &manifest);

    for path in EMBEDDED_INPUTS {
        assert_refusal(&report, "symlink_rejected", path);
    }
    assert!(
        !report
            .verified
            .iter()
            .any(|path| path.starts_with("registry/")),
        "embedded bytes reachable only through the escaping registry symlink must not verify: {:?}",
        report.verified
    );
}

#[test]
fn more_than_4096_empty_source_directories_hits_the_global_traversal_limit() {
    let sandbox = Sandbox::new();
    let manifest = sandbox.manifest_bytes();
    let source_root = sandbox.root.join("xtask/src");
    for index in 0..4097 {
        fs::create_dir(source_root.join(format!("empty-{index:04}")))
            .expect("create one additional empty source directory");
    }

    let report = verify_sandbox(&sandbox, &manifest);

    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.code == "traversal_limit_exceeded"),
        "a global cap must refuse more than 4096 visited empty directories: {:?}",
        report.findings
    );
    assert!(
        !report.is_verified(),
        "a traversal-limit finding must never permit a source snapshot"
    );
}

#[test]
fn malformed_digest_is_rejected_before_source_tree_can_be_verified() {
    let sandbox = Sandbox::new();
    let manifest = String::from_utf8(sandbox.manifest_bytes()).expect("manifest is JSON UTF-8");
    let malformed = manifest.replacen(
        &operator_sha256(&sandbox.root.join("Cargo.toml")),
        &"A".repeat(64),
        1,
    );

    let error = parse_manifest(malformed.as_bytes(), Limits::default())
        .expect_err("an uppercase SHA-256 digest must not parse as an admitted manifest");
    assert!(
        error.contains("malformed_digest"),
        "the malformed digest must retain its causal refusal code: {error}"
    );
}

#[test]
fn real_operator_hash_binds_written_snapshot_manifest_bytes() {
    let sandbox = Sandbox::new();
    let relative = "registry/source_snapshot.json";
    sandbox.write_bytes(relative, &sandbox.manifest_bytes());
    let expected = operator_sha256(&sandbox.root.join(relative));

    assert_eq!(
        verify_manifest_identity(&sandbox.root, relative, &expected),
        Ok(())
    );
}

#[test]
fn harmless_whitespace_mutation_of_snapshot_manifest_breaks_binding() {
    let sandbox = Sandbox::new();
    let relative = "registry/source_snapshot.json";
    let mut manifest = sandbox.manifest_bytes();
    sandbox.write_bytes(relative, &manifest);
    let expected = operator_sha256(&sandbox.root.join(relative));
    manifest.extend_from_slice(b"\n \t");
    sandbox.write_bytes(relative, &manifest);

    let error = verify_manifest_identity(&sandbox.root, relative, &expected)
        .expect_err("a byte change to the snapshot manifest must break its policy binding");
    assert!(
        error.starts_with("snapshot_binding_mismatch:"),
        "the unchanged expected digest must report the causal binding mismatch: {error}"
    );
}

#[test]
fn malformed_snapshot_binding_digest_is_refused_before_operator_hashing() {
    let sandbox = Sandbox::new();
    let relative = "registry/source_snapshot.json";
    sandbox.write_bytes(relative, &sandbox.manifest_bytes());

    let error = verify_manifest_identity(&sandbox.root, relative, &"A".repeat(64))
        .expect_err("a noncanonical expected digest must be refused");
    assert_eq!(error, "snapshot_binding_digest: expected lowercase SHA-256");
}
