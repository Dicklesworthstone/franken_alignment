//! Independent causal negatives for the registry-core structural checker.
//!
//! Each case supplies the live registry documents, changing exactly one field
//! in one document. The unmodified real input must be clean before a mutation
//! can establish its own causal refusal.

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::registry_checks::{RegistryInputs, Report, check_inputs};

const INVARIANTS: &[u8] = include_bytes!("../../registry/invariants.json");
const ROADMAP: &[u8] = include_bytes!("../../registry/roadmap.json");
const CLAIMS: &[u8] = include_bytes!("../../registry/claims.json");
const SOURCES: &[u8] = include_bytes!("../../registry/sources.json");
const FOUNDING: &[u8] = include_bytes!("../../registry/founding_concordance.json");

static NEXT_SANDBOX: AtomicU64 = AtomicU64::new(0);

struct Sandbox {
    base: PathBuf,
    root: PathBuf,
    outside: PathBuf,
}

impl Sandbox {
    fn from_workspace() -> Self {
        let source = workspace_root();
        let mut attempts = 0_u8;
        let base = loop {
            assert!(
                attempts < 64,
                "cannot allocate an isolated registry sandbox"
            );
            let serial = NEXT_SANDBOX.fetch_add(1, Ordering::Relaxed);
            let candidate = std::env::temp_dir().join(format!(
                "franken-alignment-registry-controls-{}-{serial}",
                std::process::id()
            ));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempts += 1;
                }
                Err(error) => panic!(
                    "create isolated registry sandbox {}: {error}",
                    candidate.display()
                ),
            }
        };
        let root = base.join("root");
        let outside = base.join("outside");
        fs::create_dir(&outside).expect("create owned sandbox outside directory");
        copy_tree(&source.join("artifacts"), &root.join("artifacts"));
        copy_file(
            &source.join("docs/RESEARCH_AGENDA.md"),
            &root.join("docs/RESEARCH_AGENDA.md"),
        );
        copy_file(
            &source.join("registry/foundation_audit.json"),
            &root.join("registry/foundation_audit.json"),
        );
        copy_tree(
            &source.join("crates/fa-reference/src"),
            &root.join("crates/fa-reference/src"),
        );
        Self {
            base,
            root,
            outside,
        }
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("sandbox file has a parent"))
            .expect("create sandbox file parent");
        fs::write(path, contents).expect("write sandbox file");
    }

    #[cfg(unix)]
    fn symlink_to_outside(&self, relative: &str, outside_name: &str, contents: &str) {
        let outside = self.outside.join(outside_name);
        fs::write(&outside, contents).expect("write external symlink target");
        let link = self.root.join(relative);
        fs::create_dir_all(link.parent().expect("sandbox symlink has a parent"))
            .expect("create sandbox symlink parent");
        symlink(outside, link).expect("create symlink escaping the sandbox root");
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.base).expect("remove owned isolated test sandbox");
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    for entry in fs::read_dir(source).expect("read source tree for registry control test") {
        let entry = entry.expect("read source tree entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if source_path.is_dir() {
            copy_tree(&source_path, &destination_path);
        } else {
            copy_file(&source_path, &destination_path);
        }
    }
}

fn copy_file(source: &Path, destination: &Path) {
    fs::create_dir_all(destination.parent().expect("copied file has a parent"))
        .expect("create copied file parent");
    fs::copy(source, destination).expect("copy current registry support file");
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a workspace parent")
        .to_path_buf()
}

fn report(
    invariants: &[u8],
    roadmap: &[u8],
    claims: &[u8],
    sources: &[u8],
    founding: &[u8],
) -> Report {
    report_at(
        &workspace_root(),
        invariants,
        roadmap,
        claims,
        sources,
        founding,
    )
}

fn report_at(
    root: &Path,
    invariants: &[u8],
    roadmap: &[u8],
    claims: &[u8],
    sources: &[u8],
    founding: &[u8],
) -> Report {
    check_inputs(
        root,
        RegistryInputs {
            invariants,
            roadmap,
            claims,
            sources,
            founding,
        },
    )
}

fn real_report() -> Report {
    report(INVARIANTS, ROADMAP, CLAIMS, SOURCES, FOUNDING)
}

fn mutate_once(input: &[u8], before: &str, after: &str) -> Vec<u8> {
    let text = std::str::from_utf8(input).expect("the checked registry is UTF-8 JSON");
    assert_eq!(
        text.matches(before).count(),
        1,
        "mutation anchor must occur exactly once; refusing an ambiguous mutation"
    );
    text.replacen(before, after, 1).into_bytes()
}

fn mutate_row_once(input: &[u8], id: &str, before: &str, after: &str) -> Vec<u8> {
    let text = std::str::from_utf8(input).expect("the checked registry is UTF-8 JSON");
    let start = format!("    {{\n      \"id\": \"{id}\"");
    assert_eq!(
        text.matches(&start).count(),
        1,
        "mutation scope anchor must occur exactly once"
    );
    let (prefix, rest) = text
        .split_once(&start)
        .expect("the counted mutation scope anchor must be present");
    let (row, suffix) = rest
        .split_once("\n    },\n    {")
        .expect("the counted mutation row end anchor must be present");
    let changed_row = mutate_once(row.as_bytes(), before, after);
    let changed_row =
        std::str::from_utf8(&changed_row).expect("a UTF-8 registry mutation remains UTF-8");
    format!("{prefix}{start}{changed_row}\n    }},\n    {{{suffix}").into_bytes()
}

fn mutate_row_id_once(input: &[u8], id: &str, replacement: &str) -> Vec<u8> {
    let text = std::str::from_utf8(input).expect("the checked registry is UTF-8 JSON");
    let start = format!("    {{\n      \"id\": \"{id}\"");
    assert_eq!(
        text.matches(&start).count(),
        1,
        "row identity mutation scope must occur exactly once"
    );
    text.replacen(
        &start,
        &format!("    {{\n      \"id\": \"{replacement}\""),
        1,
    )
    .into_bytes()
}

fn append_duplicate_packet(input: &[u8], id: &str) -> Vec<u8> {
    let text = std::str::from_utf8(input).expect("the checked registry is UTF-8 JSON");
    let start = format!("    {{\n      \"id\": \"{id}\"");
    assert_eq!(
        text.matches(&start).count(),
        1,
        "packet identity must be unique before duplication"
    );
    let (_, rest) = text
        .split_once(&start)
        .expect("the counted packet identity must be present");
    let (row, _) = rest
        .split_once("\n    },\n    {")
        .expect("the selected packet must have a following packet");
    let packet = format!("{start}{row}\n    }}");
    let close = "\n  ]\n}";
    assert_eq!(
        text.matches(close).count(),
        1,
        "roadmap packets array close must be unique"
    );
    text.replacen(close, &format!(",\n{packet}{close}"), 1)
        .into_bytes()
}

fn assert_clean(report: &Report) {
    assert!(
        report.is_clean(),
        "the unmodified real registry must be clean before a mutation is causal: {:#?}",
        report.findings
    );
}

fn assert_only_finding(report: &Report, file: &str, id: Option<&str>, code: &str) {
    assert_eq!(
        report.findings.len(),
        1,
        "one-field mutation must produce one causal finding: {:#?}",
        report.findings
    );
    let finding = &report.findings[0];
    assert_eq!(finding.file, file);
    assert_eq!(finding.id.as_deref(), id);
    assert_eq!(finding.code, code);
}

fn invariants_with_reference(reference: &str) -> Vec<u8> {
    mutate_row_once(
        INVARIANTS,
        "FA-INV-001",
        "crates/fa-reference/src/lib.rs::tests::effect_binding_and_one_shot_dispatch",
        reference,
    )
}

#[test]
fn current_registry_core_is_clean_before_negative_mutations() {
    assert_clean(&real_report());
}

#[test]
fn duplicate_packet_id_is_rejected_as_duplicate_id() {
    assert_clean(&real_report());
    // Copying the complete FA-002 row keeps every existing ID and edge valid;
    // the duplicate identifier is the only planted graph defect.
    let roadmap = append_duplicate_packet(ROADMAP, "FA-002");
    assert_only_finding(
        &report(INVARIANTS, &roadmap, CLAIMS, SOURCES, FOUNDING),
        "registry/roadmap.json",
        Some("FA-002"),
        "duplicate_id",
    );
}

#[test]
fn missing_dag_node_is_rejected_as_unknown_dependency() {
    assert_clean(&real_report());
    let roadmap = mutate_row_once(
        ROADMAP,
        "FA-003",
        "\"depends_on\": [\n        \"FA-001\",\n        \"FA-002\"\n      ]",
        "\"depends_on\": [\n        \"FA-001\",\n        \"FA-999\"\n      ]",
    );
    assert_only_finding(
        &report(INVARIANTS, &roadmap, CLAIMS, SOURCES, FOUNDING),
        "registry/roadmap.json",
        Some("FA-003"),
        "unknown_dependency",
    );
}

#[test]
fn roadmap_cycle_is_rejected_as_dependency_cycle() {
    assert_clean(&real_report());
    let roadmap = mutate_row_once(
        ROADMAP,
        "FA-001",
        "\"depends_on\": [],\n      \"invariants\": [\n        \"FA-INV-001\"",
        "\"depends_on\": [\"FA-003\"],\n      \"invariants\": [\n        \"FA-INV-001\"",
    );
    assert_only_finding(
        &report(INVARIANTS, &roadmap, CLAIMS, SOURCES, FOUNDING),
        "registry/roadmap.json",
        Some("FA-001"),
        "dependency_cycle",
    );
}

#[test]
fn missing_invariant_is_rejected_as_unknown_invariant() {
    assert_clean(&real_report());
    let roadmap = mutate_row_once(
        ROADMAP,
        "FA-001",
        "\"FA-INV-001\",\n        \"FA-INV-002\"",
        "\"FA-INV-999\",\n        \"FA-INV-002\"",
    );
    assert_only_finding(
        &report(INVARIANTS, &roadmap, CLAIMS, SOURCES, FOUNDING),
        "registry/roadmap.json",
        Some("FA-001"),
        "unknown_invariant",
    );
}

#[test]
fn missing_result_artifact_is_rejected_as_referenced_file_missing() {
    assert_clean(&real_report());
    let roadmap = mutate_row_once(
        ROADMAP,
        "FA-003",
        "\"artifacts/execution/2026-09-06-cargo-test.log\"",
        "\"artifacts/execution/does-not-exist.log\"",
    );
    assert_only_finding(
        &report(INVARIANTS, &roadmap, CLAIMS, SOURCES, FOUNDING),
        "registry/roadmap.json",
        Some("FA-003"),
        "referenced_file_missing",
    );
}

#[test]
fn missing_reference_test_symbol_is_rejected_as_reference_test_missing() {
    assert_clean(&real_report());
    let invariants = mutate_row_once(
        INVARIANTS,
        "FA-INV-001",
        "crates/fa-reference/src/lib.rs::tests::effect_binding_and_one_shot_dispatch",
        "crates/fa-reference/src/lib.rs::tests::does_not_exist",
    );
    assert_only_finding(
        &report(&invariants, ROADMAP, CLAIMS, SOURCES, FOUNDING),
        "registry/invariants.json",
        Some("FA-INV-001"),
        "reference_test_missing",
    );
}

#[test]
fn punctuation_only_source_id_is_rejected_as_malformed_id() {
    assert_clean(&real_report());
    // A-EPROCESS is deliberately unreferenced by the live claims/founding
    // inputs, so this changes source-ID grammar only, not a link target.
    let sources = mutate_row_id_once(SOURCES, "A-EPROCESS", "---");
    assert_only_finding(
        &report(INVARIANTS, ROADMAP, CLAIMS, &sources, FOUNDING),
        "registry/sources.json",
        Some("---"),
        "malformed_id",
    );
}

#[test]
fn commented_test_declaration_does_not_satisfy_reference_check() {
    assert_clean(&real_report());
    let sandbox = Sandbox::from_workspace();
    sandbox.write(
        "crates/fa-reference/src/comment_only.rs",
        "/* outer comment\n   /* nested comment */\n   #[test]\n   fn phantom() {}\n*/\n",
    );
    let invariants =
        invariants_with_reference("crates/fa-reference/src/comment_only.rs::tests::phantom");
    assert_only_finding(
        &report_at(
            sandbox.root(),
            &invariants,
            ROADMAP,
            CLAIMS,
            SOURCES,
            FOUNDING,
        ),
        "registry/invariants.json",
        Some("FA-INV-001"),
        "reference_test_missing",
    );
}

#[test]
fn raw_string_test_declaration_does_not_satisfy_reference_check() {
    assert_clean(&real_report());
    let sandbox = Sandbox::from_workspace();
    sandbox.write(
        "crates/fa-reference/src/raw_string_only.rs",
        "const NOTE: &str = r###\"#[test]\nfn phantom() {}\n\"###;\n",
    );
    let invariants =
        invariants_with_reference("crates/fa-reference/src/raw_string_only.rs::tests::phantom");
    assert_only_finding(
        &report_at(
            sandbox.root(),
            &invariants,
            ROADMAP,
            CLAIMS,
            SOURCES,
            FOUNDING,
        ),
        "registry/invariants.json",
        Some("FA-INV-001"),
        "reference_test_missing",
    );
}

#[cfg(unix)]
#[test]
fn artifact_symlink_leaf_escaping_root_is_rejected() {
    assert_clean(&real_report());
    let sandbox = Sandbox::from_workspace();
    sandbox.symlink_to_outside(
        "artifacts/execution/escaped-artifact.log",
        "artifact-outside.log",
        "outside the supplied root\n",
    );
    let invariants = mutate_row_once(
        INVARIANTS,
        "FA-INV-001",
        "artifacts/execution/2026-09-06-cargo-test.log",
        "artifacts/execution/escaped-artifact.log",
    );
    assert_only_finding(
        &report_at(
            sandbox.root(),
            &invariants,
            ROADMAP,
            CLAIMS,
            SOURCES,
            FOUNDING,
        ),
        "registry/invariants.json",
        Some("FA-INV-001"),
        "unsafe_file_reference",
    );
}

#[cfg(unix)]
#[test]
fn reference_source_symlink_leaf_escaping_root_is_rejected() {
    assert_clean(&real_report());
    let sandbox = Sandbox::from_workspace();
    sandbox.symlink_to_outside(
        "crates/fa-reference/src/escaped_source.rs",
        "source-outside.rs",
        "mod tests {\n    #[test]\n    fn phantom() {}\n}\n",
    );
    let invariants =
        invariants_with_reference("crates/fa-reference/src/escaped_source.rs::tests::phantom");
    assert_only_finding(
        &report_at(
            sandbox.root(),
            &invariants,
            ROADMAP,
            CLAIMS,
            SOURCES,
            FOUNDING,
        ),
        "registry/invariants.json",
        Some("FA-INV-001"),
        "unsafe_reference_path",
    );
}

#[test]
fn lifetimes_before_a_real_tests_module_do_not_hide_the_registered_test() {
    assert_clean(&real_report());
    let sandbox = Sandbox::from_workspace();
    sandbox.write(
        "crates/fa-reference/src/lifetimes_then_test.rs",
        "const LABEL: &'static str = \"still code\";\n\
         fn borrow<'a>(value: &'a str) -> &'a str { value }\n\
         mod tests {\n    #[test]\n    fn phantom() {}\n}\n",
    );
    let invariants =
        invariants_with_reference("crates/fa-reference/src/lifetimes_then_test.rs::tests::phantom");
    assert_clean(&report_at(
        sandbox.root(),
        &invariants,
        ROADMAP,
        CLAIMS,
        SOURCES,
        FOUNDING,
    ));
}

#[test]
fn top_level_test_does_not_satisfy_a_tests_module_reference() {
    assert_clean(&real_report());
    let sandbox = Sandbox::from_workspace();
    sandbox.write(
        "crates/fa-reference/src/top_level_test.rs",
        "#[test]\nfn phantom() {}\n",
    );
    let invariants =
        invariants_with_reference("crates/fa-reference/src/top_level_test.rs::tests::phantom");
    assert_only_finding(
        &report_at(
            sandbox.root(),
            &invariants,
            ROADMAP,
            CLAIMS,
            SOURCES,
            FOUNDING,
        ),
        "registry/invariants.json",
        Some("FA-INV-001"),
        "reference_test_missing",
    );
}

#[test]
fn other_module_test_does_not_satisfy_a_tests_module_reference() {
    assert_clean(&real_report());
    let sandbox = Sandbox::from_workspace();
    sandbox.write(
        "crates/fa-reference/src/other_module_test.rs",
        "mod other {\n    #[test]\n    fn phantom() {}\n}\n",
    );
    let invariants =
        invariants_with_reference("crates/fa-reference/src/other_module_test.rs::tests::phantom");
    assert_only_finding(
        &report_at(
            sandbox.root(),
            &invariants,
            ROADMAP,
            CLAIMS,
            SOURCES,
            FOUNDING,
        ),
        "registry/invariants.json",
        Some("FA-INV-001"),
        "reference_test_missing",
    );
}

#[test]
fn macro_body_test_lookalike_does_not_satisfy_reference_check() {
    assert_clean(&real_report());
    let sandbox = Sandbox::from_workspace();
    sandbox.write(
        "crates/fa-reference/src/macro_lookalike.rs",
        "macro_rules! phantom_test {\n    () => { #[test] fn phantom() {} };\n}\n",
    );
    let invariants =
        invariants_with_reference("crates/fa-reference/src/macro_lookalike.rs::tests::phantom");
    assert_only_finding(
        &report_at(
            sandbox.root(),
            &invariants,
            ROADMAP,
            CLAIMS,
            SOURCES,
            FOUNDING,
        ),
        "registry/invariants.json",
        Some("FA-INV-001"),
        "reference_test_missing",
    );
}

#[test]
fn c_raw_string_test_lookalike_does_not_satisfy_reference_check() {
    assert_clean(&real_report());
    let sandbox = Sandbox::from_workspace();
    sandbox.write(
        "crates/fa-reference/src/c_raw_string_lookalike.rs",
        "const NOTE: &core::ffi::CStr = cr###\"#[test]\nfn phantom() {}\n\"###;\n",
    );
    let invariants = invariants_with_reference(
        "crates/fa-reference/src/c_raw_string_lookalike.rs::tests::phantom",
    );
    assert_only_finding(
        &report_at(
            sandbox.root(),
            &invariants,
            ROADMAP,
            CLAIMS,
            SOURCES,
            FOUNDING,
        ),
        "registry/invariants.json",
        Some("FA-INV-001"),
        "reference_test_missing",
    );
}
