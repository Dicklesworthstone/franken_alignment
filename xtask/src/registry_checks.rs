//! Fail-closed structural checks for the owned registry core (roadmap FA-003).
//!
//! This module establishes repository-internal consistency only.  It does not
//! promote a roadmap packet, prove a production invariant, or replace the
//! dedicated founding-concordance checker.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path};

use crate::json::{parse, Json, Limits};

const INVARIANTS: &str = "registry/invariants.json";
const ROADMAP: &str = "registry/roadmap.json";
const CLAIMS: &str = "registry/claims.json";
const SOURCES: &str = "registry/sources.json";
const FOUNDING: &str = "registry/founding_concordance.json";

/// One causal registry refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Repository-relative registry input that caused the refusal.
    pub file: &'static str,
    /// Object identity, when it could be read without inventing one.
    pub id: Option<String>,
    /// Stable machine-readable refusal cause.
    pub code: &'static str,
    /// Field-level location inside `file`.
    pub location: String,
    /// Actionable explanation of the observed defect.
    pub detail: String,
}

impl Finding {
    fn new(
        file: &'static str,
        id: Option<&str>,
        code: &'static str,
        location: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            file,
            id: id.map(str::to_string),
            code,
            location: location.into(),
            detail: detail.into(),
        }
    }
}

/// The complete result of one registry-core pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Every detected structural refusal in deterministic order.
    pub findings: Vec<Finding>,
}

impl Report {
    /// A registry pass is clean only when it found no structural refusal.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }
}

struct Documents {
    invariants: Json,
    roadmap: Json,
    claims: Json,
    sources: Json,
    founding: Json,
}

/// Read the current registry inputs and validate the FA-003 core boundaries.
///
/// I/O and JSON parse failures are returned as findings rather than becoming
/// an implicit skipped pass.  Callers must treat a non-clean [`Report`] as a
/// failed gate.
#[must_use]
pub fn check_repository(root: &Path) -> Report {
    let mut findings = Vec::new();
    let invariants = read_json(root, INVARIANTS, &mut findings);
    let roadmap = read_json(root, ROADMAP, &mut findings);
    let claims = read_json(root, CLAIMS, &mut findings);
    let sources = read_json(root, SOURCES, &mut findings);
    let founding = read_json(root, FOUNDING, &mut findings);

    match (invariants, roadmap, claims, sources, founding) {
        (Some(invariants), Some(roadmap), Some(claims), Some(sources), Some(founding)) => {
            check_documents(
                root,
                Documents {
                    invariants,
                    roadmap,
                    claims,
                    sources,
                    founding,
                },
                findings,
            )
        }
        _ => finish(findings),
    }
}

fn read_json(root: &Path, file: &'static str, findings: &mut Vec<Finding>) -> Option<Json> {
    let path = root.join(file);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            findings.push(Finding::new(
                file,
                None,
                "read_failed",
                "$",
                format!("cannot read {}: {error}", path.display()),
            ));
            return None;
        }
    };
    match parse(&bytes, Limits::default()) {
        Ok(value) => Some(value),
        Err(error) => {
            findings.push(Finding::new(
                file,
                None,
                "invalid_json",
                "$",
                format!("strict JSON parse failed: {error}"),
            ));
            None
        }
    }
}

fn check_documents(root: &Path, docs: Documents, mut findings: Vec<Finding>) -> Report {
    let invariant_rows = rows(&docs.invariants, INVARIANTS, "invariants", &mut findings);
    let roadmap_rows = rows(&docs.roadmap, ROADMAP, "packets", &mut findings);
    let claim_rows = rows(&docs.claims, CLAIMS, "claims", &mut findings);
    let source_rows = rows(&docs.sources, SOURCES, "sources", &mut findings);
    let founding_rows = rows(&docs.founding, FOUNDING, "founding_ideas", &mut findings);

    let invariant_ids = ids(
        invariant_rows.as_deref(),
        INVARIANTS,
        "FA-INV-",
        &mut findings,
    );
    let roadmap_ids = ids(roadmap_rows.as_deref(), ROADMAP, "FA-", &mut findings);
    let claim_ids = ids(claim_rows.as_deref(), CLAIMS, "H", &mut findings);
    let source_ids = ids(source_rows.as_deref(), SOURCES, "", &mut findings);
    let founding_ids = ids(founding_rows.as_deref(), FOUNDING, "FOUNDING", &mut findings);

    if let Some(rows) = roadmap_rows.as_deref() {
        check_roadmap(rows, &roadmap_ids, &invariant_ids, &mut findings);
        check_artifact_references(rows, ROADMAP, "result_artifacts", root, &mut findings);
    }
    if let Some(rows) = invariant_rows.as_deref() {
        check_string_references(
            rows,
            INVARIANTS,
            "founding_ideas",
            &founding_ids,
            "unknown_founding_idea",
            &mut findings,
        );
        check_artifact_references(rows, INVARIANTS, "evidence_artifacts", root, &mut findings);
        check_reference_checks(rows, root, &mut findings);
    }
    if let Some(rows) = claim_rows.as_deref() {
        check_string_references(
            rows,
            CLAIMS,
            "source_refs",
            &source_ids,
            "unknown_source",
            &mut findings,
        );
        check_string_references(
            rows,
            CLAIMS,
            "founding_ideas",
            &founding_ids,
            "unknown_founding_idea",
            &mut findings,
        );
        check_file_references(rows, CLAIMS, "experiment_card", root, &mut findings);
        check_artifact_references(rows, CLAIMS, "result_artifacts", root, &mut findings);
    }
    if let Some(rows) = founding_rows.as_deref() {
        check_single_string_references(
            rows,
            FOUNDING,
            "source",
            &source_ids,
            "unknown_source",
            &mut findings,
        );
    }
    if let Some(source_document) = docs.sources.as_object()
        && let Some(value) = source_document.get("revision_0_2_source_inventory")
    {
        match value.as_str() {
            Some(path) => check_existing_file(
                SOURCES,
                None,
                "revision_0_2_source_inventory",
                path,
                root,
                &mut findings,
            ),
            None => findings.push(type_finding(
                SOURCES,
                None,
                "expected_string",
                "$.revision_0_2_source_inventory",
                "string",
                value,
            )),
        }
    }

    finish(findings)
}

fn finish(mut findings: Vec<Finding>) -> Report {
    findings.sort_by(|left, right| {
        left.file
            .cmp(right.file)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.code.cmp(right.code))
            .then_with(|| left.location.cmp(&right.location))
    });
    Report { findings }
}

fn rows<'a>(
    document: &'a Json,
    file: &'static str,
    key: &'static str,
    findings: &mut Vec<Finding>,
) -> Option<&'a [Json]> {
    let object = match document.as_object() {
        Some(object) => object,
        None => {
            findings.push(type_finding(file, None, "expected_object", "$", "object", document));
            return None;
        }
    };
    let value = match object.get(key) {
        Some(value) => value,
        None => {
            findings.push(Finding::new(
                file,
                None,
                "missing_field",
                "$",
                format!("required top-level field `{key}` is absent"),
            ));
            return None;
        }
    };
    match value.as_array() {
        Some(rows) => Some(rows),
        None => {
            findings.push(type_finding(
                file,
                None,
                "expected_array",
                format!("$.{key}"),
                "array",
                value,
            ));
            None
        }
    }
}

fn ids(
    rows: Option<&[Json]>,
    file: &'static str,
    prefix: &str,
    findings: &mut Vec<Finding>,
) -> BTreeSet<String> {
    let mut known = BTreeSet::new();
    let Some(rows) = rows else {
        return known;
    };
    for (index, row) in rows.iter().enumerate() {
        let location = format!("$[{index}]");
        let Some(object) = row.as_object() else {
            findings.push(type_finding(file, None, "expected_object", location, "object", row));
            continue;
        };
        let Some(value) = object.get("id") else {
            findings.push(Finding::new(
                file,
                None,
                "missing_field",
                format!("{location}.id"),
                "registry rows require a string `id`",
            ));
            continue;
        };
        let Some(id) = value.as_str() else {
            findings.push(type_finding(
                file,
                None,
                "expected_string",
                format!("{location}.id"),
                "string",
                value,
            ));
            continue;
        };
        if !well_formed_id(id, prefix) {
            findings.push(Finding::new(
                file,
                Some(id),
                "malformed_id",
                format!("{location}.id"),
                format!("`{id}` is not a well-formed identifier for this registry"),
            ));
        }
        if !known.insert(id.to_string()) {
            findings.push(Finding::new(
                file,
                Some(id),
                "duplicate_id",
                format!("{location}.id"),
                format!("identifier `{id}` occurs more than once"),
            ));
        }
    }
    known
}

fn well_formed_id(id: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return !id.is_empty()
            && id
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-');
    }
    if prefix == "FOUNDING" {
        if let Some(number) = id.strip_prefix("FS-") {
            return number.len() == 2 && number.bytes().all(|byte| byte.is_ascii_digit());
        }
        let Some(suffix) = id.strip_prefix("FI-") else {
            return false;
        };
        let (family, number) = suffix.split_at(1.min(suffix.len()));
        return matches!(family, "A" | "I" | "S")
            && number.len() == 2
            && number.bytes().all(|byte| byte.is_ascii_digit());
    }
    let Some(suffix) = id.strip_prefix(prefix) else {
        return false;
    };
    match prefix {
        "FA-INV-" => suffix.len() == 3 && suffix.bytes().all(|byte| byte.is_ascii_digit()),
        "FA-" => suffix.len() == 3 && suffix.bytes().all(|byte| byte.is_ascii_digit()),
        "H" => !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()),
        _ => false,
    }
}

fn check_single_string_references(
    rows: &[Json],
    file: &'static str,
    key: &'static str,
    known: &BTreeSet<String>,
    code: &'static str,
    findings: &mut Vec<Finding>,
) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        if !object.contains_key(key) {
            continue;
        }
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(value) = object.get(key) else {
            findings.push(Finding::new(
                file,
                id,
                "missing_field",
                format!("{base}.{key}"),
                format!("required field `{key}` is absent"),
            ));
            continue;
        };
        let Some(value) = value.as_str() else {
            findings.push(type_finding(
                file,
                id,
                "expected_string",
                format!("{base}.{key}"),
                "string",
                value,
            ));
            continue;
        };
        if !known.contains(value) {
            findings.push(Finding::new(
                file,
                id,
                code,
                format!("{base}.{key}"),
                format!("`{value}` is not present in its referenced registry"),
            ));
        }
    }
}

fn check_roadmap(
    rows: &[Json],
    roadmap_ids: &BTreeSet<String>,
    invariant_ids: &BTreeSet<String>,
    findings: &mut Vec<Finding>,
) {
    let mut edges = BTreeMap::new();
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        let Some(id) = object.get("id").and_then(Json::as_str) else {
            continue;
        };
        let base = format!("$.packets[{index}]");
        let dependencies = string_array(object, ROADMAP, id, &base, "depends_on", findings);
        let invariants = string_array(object, ROADMAP, id, &base, "invariants", findings);
        if let Some(dependencies) = dependencies {
            for dependency in &dependencies {
                if !roadmap_ids.contains(dependency) {
                    findings.push(Finding::new(
                        ROADMAP,
                        Some(id),
                        "unknown_dependency",
                        format!("{base}.depends_on"),
                        format!("packet `{id}` depends on missing packet `{dependency}`"),
                    ));
                }
            }
            edges.insert(id.to_string(), dependencies);
        }
        if let Some(invariants) = invariants {
            for invariant in invariants {
                if !invariant_ids.contains(&invariant) {
                    findings.push(Finding::new(
                        ROADMAP,
                        Some(id),
                        "unknown_invariant",
                        format!("{base}.invariants"),
                        format!("packet `{id}` names missing invariant `{invariant}`"),
                    ));
                }
            }
        }
    }
    detect_cycles(&edges, findings);
}

fn detect_cycles(edges: &BTreeMap<String, Vec<String>>, findings: &mut Vec<Finding>) {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut stack = Vec::new();
    for node in edges.keys() {
        visit(node, edges, &mut visiting, &mut visited, &mut stack, findings);
    }
}

fn visit(
    node: &str,
    edges: &BTreeMap<String, Vec<String>>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    stack: &mut Vec<String>,
    findings: &mut Vec<Finding>,
) {
    if visited.contains(node) {
        return;
    }
    if !visiting.insert(node.to_string()) {
        let start = stack.iter().position(|item| item == node).unwrap_or(0);
        let mut cycle = stack[start..].to_vec();
        cycle.push(node.to_string());
        findings.push(Finding::new(
            ROADMAP,
            Some(node),
            "dependency_cycle",
            "$.packets.depends_on",
            format!("roadmap dependency cycle: {}", cycle.join(" -> ")),
        ));
        return;
    }
    stack.push(node.to_string());
    if let Some(dependencies) = edges.get(node) {
        for dependency in dependencies {
            if edges.contains_key(dependency) {
                visit(dependency, edges, visiting, visited, stack, findings);
            }
        }
    }
    stack.pop();
    visiting.remove(node);
    visited.insert(node.to_string());
}

fn check_string_references(
    rows: &[Json],
    file: &'static str,
    key: &'static str,
    known: &BTreeSet<String>,
    code: &'static str,
    findings: &mut Vec<Finding>,
) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        if !object.contains_key(key) {
            continue;
        }
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(values) = string_array(object, file, id.unwrap_or("<missing>"), &base, key, findings) else {
            continue;
        };
        for value in values {
            if !known.contains(&value) {
                findings.push(Finding::new(
                    file,
                    id,
                    code,
                    format!("{base}.{key}"),
                    format!("`{value}` is not present in its referenced registry"),
                ));
            }
        }
    }
}

fn check_artifact_references(
    rows: &[Json],
    file: &'static str,
    key: &'static str,
    root: &Path,
    findings: &mut Vec<Finding>,
) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        if !object.contains_key(key) {
            continue;
        }
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(paths) = string_array(object, file, id.unwrap_or("<missing>"), &base, key, findings) else {
            continue;
        };
        for path in paths {
            check_existing_file(file, id, &format!("{base}.{key}"), &path, root, findings);
        }
    }
}

fn check_file_references(
    rows: &[Json],
    file: &'static str,
    key: &'static str,
    root: &Path,
    findings: &mut Vec<Finding>,
) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        if !object.contains_key("reference_checks") {
            continue;
        }
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(value) = object.get(key) else {
            findings.push(Finding::new(
                file,
                id,
                "missing_field",
                format!("{base}.{key}"),
                format!("required field `{key}` is absent"),
            ));
            continue;
        };
        match value.as_str() {
            Some(path) => check_existing_file(file, id, &format!("{base}.{key}"), path, root, findings),
            None => findings.push(type_finding(
                file,
                id,
                "expected_string",
                format!("{base}.{key}"),
                "string",
                value,
            )),
        }
    }
}

fn check_reference_checks(rows: &[Json], root: &Path, findings: &mut Vec<Finding>) {
    for (index, row) in rows.iter().enumerate() {
        let Some(object) = row.as_object() else {
            continue;
        };
        let id = object.get("id").and_then(Json::as_str);
        let base = format!("$[{index}]");
        let Some(symbols) = string_array(object, INVARIANTS, id.unwrap_or("<missing>"), &base, "reference_checks", findings) else {
            continue;
        };
        for symbol in symbols {
            let Some((path, name)) = symbol.split_once("::tests::") else {
                findings.push(Finding::new(
                    INVARIANTS,
                    id,
                    "malformed_reference_check",
                    format!("{base}.reference_checks"),
                    format!("`{symbol}` must be `path::tests::test_name`"),
                ));
                continue;
            };
            if name.is_empty() || name.contains("::") {
                findings.push(Finding::new(
                    INVARIANTS,
                    id,
                    "malformed_reference_check",
                    format!("{base}.reference_checks"),
                    format!("`{symbol}` has no single test function name"),
                ));
                continue;
            }
            let Some(source_path) = safe_relative_path(path) else {
                findings.push(Finding::new(
                    INVARIANTS,
                    id,
                    "unsafe_reference_path",
                    format!("{base}.reference_checks"),
                    format!("`{symbol}` does not name a safe repository-relative source path"),
                ));
                continue;
            };
            let source_path = root.join(source_path);
            let source = match fs::read_to_string(&source_path) {
                Ok(source) => source,
                Err(error) => {
                    findings.push(Finding::new(
                        INVARIANTS,
                        id,
                        "reference_source_missing",
                        format!("{base}.reference_checks"),
                        format!("cannot read {}: {error}", source_path.display()),
                    ));
                    continue;
                }
            };
            if !declares_test(&source, name) {
                findings.push(Finding::new(
                    INVARIANTS,
                    id,
                    "reference_test_missing",
                    format!("{base}.reference_checks"),
                    format!("`{symbol}` does not resolve to a #[test] function"),
                ));
            }
        }
    }
}

fn declares_test(source: &str, name: &str) -> bool {
    let expected = format!("fn {name}(");
    let mut saw_test = false;
    for line in source.lines() {
        let line = line.trim();
        if line == "#[test]" {
            saw_test = true;
        } else if saw_test && !line.is_empty() {
            if line.starts_with(&expected) {
                return true;
            }
            saw_test = false;
        }
    }
    false
}

fn string_array(
    object: &BTreeMap<String, Json>,
    file: &'static str,
    id: &str,
    base: &str,
    key: &'static str,
    findings: &mut Vec<Finding>,
) -> Option<Vec<String>> {
    let value = match object.get(key) {
        Some(value) => value,
        None => {
            findings.push(Finding::new(
                file,
                Some(id),
                "missing_field",
                format!("{base}.{key}"),
                format!("required field `{key}` is absent"),
            ));
            return None;
        }
    };
    let Some(values) = value.as_array() else {
        findings.push(type_finding(
            file,
            Some(id),
            "expected_array",
            format!("{base}.{key}"),
            "array",
            value,
        ));
        return None;
    };
    let mut strings = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        match value.as_str() {
            Some(value) => strings.push(value.to_string()),
            None => {
                findings.push(type_finding(
                    file,
                    Some(id),
                    "expected_string",
                    format!("{base}.{key}[{index}]"),
                    "string",
                    value,
                ));
                return None;
            }
        }
    }
    Some(strings)
}

fn check_existing_file(
    file: &'static str,
    id: Option<&str>,
    location: &str,
    referenced: &str,
    root: &Path,
    findings: &mut Vec<Finding>,
) {
    let Some(path) = safe_relative_path(referenced) else {
        findings.push(Finding::new(
            file,
            id,
            "unsafe_file_reference",
            location,
            format!("`{referenced}` is not a safe repository-relative path"),
        ));
        return;
    };
    if !root.join(path).is_file() {
        findings.push(Finding::new(
            file,
            id,
            "referenced_file_missing",
            location,
            format!("retained reference `{referenced}` does not exist as a file"),
        ));
    }
}

fn safe_relative_path(value: &str) -> Option<&Path> {
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            !matches!(component, Component::Normal(_))
        })
    {
        return None;
    }
    Some(path)
}

fn type_finding(
    file: &'static str,
    id: Option<&str>,
    code: &'static str,
    location: impl Into<String>,
    expected: &str,
    found: &Json,
) -> Finding {
    Finding::new(
        file,
        id,
        code,
        location,
        format!("expected {expected}, found {}", found.kind()),
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const DUPLICATE_ROADMAP: &str =
        include_str!("../tests/fixtures/registry/duplicate-roadmap-id.json");
    const DEPENDENCY_CYCLE: &str =
        include_str!("../tests/fixtures/registry/dependency-cycle-roadmap.json");
    const MISSING_INVARIANT: &str =
        include_str!("../tests/fixtures/registry/missing-invariant-roadmap.json");
    const MISSING_REFERENCE_SYMBOL: &str =
        include_str!("../tests/fixtures/registry/missing-reference-symbol-invariants.json");
    const MISSING_EVIDENCE_ARTIFACT: &str =
        include_str!("../tests/fixtures/registry/missing-evidence-artifact-invariants.json");

    fn repository_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask has a workspace parent")
            .to_path_buf()
    }

    fn fixture_documents(bytes: &str) -> Documents {
        let fixture = parse(bytes.as_bytes(), Limits::default()).expect("fixture parses");
        let object = fixture.as_object().expect("fixture root is an object");
        let document = |key: &str| {
            object
                .get(key)
                .unwrap_or_else(|| panic!("fixture lacks `{key}`"))
                .clone()
        };
        Documents {
            invariants: document("invariants"),
            roadmap: document("roadmap"),
            claims: document("claims"),
            sources: document("sources"),
            founding: document("founding"),
        }
    }

    fn fixture_report(bytes: &str) -> Report {
        check_documents(&repository_root(), fixture_documents(bytes), Vec::new())
    }

    fn assert_cause(report: &Report, file: &str, code: &str) {
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.file == file && finding.code == code),
            "expected causal refusal {file}:{code}; got {:?}",
            report.findings
        );
        assert!(!report.is_clean(), "a refusal cannot be a clean registry pass");
    }

    #[test]
    fn current_owned_registry_inputs_pass_the_core_checks() {
        let report = check_repository(&repository_root());
        assert!(
            report.is_clean(),
            "current registry inputs must satisfy the owned FA-003 core checks: {:?}",
            report.findings
        );
    }

    #[test]
    fn duplicate_roadmap_id_is_refused_as_an_identity_collision() {
        assert_cause(&fixture_report(DUPLICATE_ROADMAP), ROADMAP, "duplicate_id");
    }

    #[test]
    fn dependency_cycle_is_refused_as_a_dag_violation() {
        assert_cause(&fixture_report(DEPENDENCY_CYCLE), ROADMAP, "dependency_cycle");
    }

    #[test]
    fn unknown_roadmap_invariant_is_refused_at_the_link() {
        assert_cause(&fixture_report(MISSING_INVARIANT), ROADMAP, "unknown_invariant");
    }

    #[test]
    fn missing_reference_test_symbol_is_not_a_textual_near_match() {
        assert_cause(
            &fixture_report(MISSING_REFERENCE_SYMBOL),
            INVARIANTS,
            "reference_test_missing",
        );
    }

    #[test]
    fn missing_retained_evidence_artifact_is_refused() {
        assert_cause(
            &fixture_report(MISSING_EVIDENCE_ARTIFACT),
            INVARIANTS,
            "referenced_file_missing",
        );
    }
}
