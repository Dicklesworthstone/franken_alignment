//! Operator source-snapshot verification (FA-053 evidence input).
//!
//! Verifies that the reviewed package source and build inputs on disk are
//! byte-for-byte the ones a reviewed manifest names, using the operator's own
//! SHA-256 tool. `docs/DEPENDENCY_CONSTITUTION.md` places Git, rustup, the
//! linker and local host automation outside the product, so the hashing tool is
//! operator infrastructure and **no cryptography is authored here**.
//!
//! What a clean report establishes:
//!
//! * every regular file under the reviewed scan roots, plus the named
//!   build-input files, plus the registry files the crate embeds at compile
//!   time with `include_bytes!`, is present, is a regular file, lies inside the
//!   root, and hashes to the digest the manifest records;
//! * the discovered set and the manifest set are **equal in both directions**,
//!   so a new unlisted file is a refusal rather than a silent pass.
//!
//! What it does **not** establish: the authenticity of the hashing binary, that
//! the tool computed a genuine SHA-256, any runtime semantics of the hashed
//! source, or anything at all about package dependencies.
//! The outer operator must freeze the tree: path checks and subprocess hashing
//! are separate observations, not an atomic snapshot against concurrent edits.
//!
//! The manifest itself and `registry/dependency_policy.json` are operator data
//! outside every scan root, so nothing here ever hashes its own input.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use crate::json::{Json, Limits as JsonLimits, parse};

/// Manifest schema identity.
pub const SNAPSHOT_SCHEMA: &str = "fa.source_snapshot/1";

/// Directories whose every regular file is reviewed source.
pub const SCAN_ROOTS: [&str; 4] = [
    "crates/fa-reference/src",
    "crates/fa-reference/tests",
    "xtask/src",
    "xtask/tests",
];

/// Individually named build inputs outside the scan roots.
pub const EXPLICIT_FILES: [&str; 6] = [
    "crates/fa-reference/Cargo.toml",
    "xtask/Cargo.toml",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "LICENSE",
];

/// Registry files embedded into the crate at compile time by `include_bytes!`.
///
/// These are real compile inputs: their bytes are baked into the test binary,
/// so a change to any of them changes what was compiled. They belong in the
/// reviewed snapshot exactly as much as a `.rs` file.
///
/// Sites, verified in source: `registry_negative_tests.rs` embeds `invariants`,
/// `roadmap`, `claims`, `sources` and `founding_concordance`; `json_tests.rs`
/// embeds `vocabulary`.
///
/// `registry/dependency_policy.json` is deliberately **not** here and is not
/// embedded anywhere: `main.rs` reads it at run time with `fs::read`, so it is
/// operator input rather than a compile input. Excluding it, and the manifest
/// itself, is what keeps this verifier free of self-reference.
pub const EMBEDDED_INPUTS: [&str; 6] = [
    "registry/invariants.json",
    "registry/roadmap.json",
    "registry/claims.json",
    "registry/sources.json",
    "registry/founding_concordance.json",
    "registry/vocabulary.json",
];

// Hard ceilings. A caller cannot raise a bound past these, so a mistaken or
// hostile `Limits` cannot turn traversal or manifest parsing unbounded.
const MAX_FILES_CEILING: usize = 4096;
const MAX_DEPTH_CEILING: usize = 16;
const MAX_MANIFEST_BYTES_CEILING: usize = 4 * 1024 * 1024;
const MAX_VISITED_ENTRIES: usize = 4096;

/// Bounds on traversal and manifest size.
///
/// Every field is clamped to a hard ceiling before use; raising one past its
/// ceiling has no effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum number of files considered, discovered or listed.
    pub max_files: usize,
    /// Maximum path components below the workspace root.
    pub max_depth: usize,
    /// Maximum manifest size in bytes.
    pub max_manifest_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_files: 512,
            max_depth: 8,
            max_manifest_bytes: 1024 * 1024,
        }
    }
}

impl Limits {
    /// Apply the hard ceilings.
    fn clamped(self) -> Limits {
        Limits {
            max_files: self.max_files.min(MAX_FILES_CEILING),
            max_depth: self.max_depth.min(MAX_DEPTH_CEILING),
            max_manifest_bytes: self.max_manifest_bytes.min(MAX_MANIFEST_BYTES_CEILING),
        }
    }
}

/// The stage a finding was produced in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    /// Shape of the manifest document.
    Manifest,
    /// Walking the tree and comparing the file sets.
    Discovery,
    /// Comparing bytes to a recorded digest.
    Digest,
    /// Availability and behaviour of the operator hashing tool.
    Tool,
}

impl Phase {
    /// Stable lowercase name used in reports.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Manifest => "manifest",
            Phase::Discovery => "discovery",
            Phase::Digest => "digest",
            Phase::Tool => "tool",
        }
    }
}

/// One reason verification was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// Which stage produced it.
    pub phase: Phase,
    /// Stable machine-readable code.
    pub code: &'static str,
    /// The repo-relative path it concerns, or the manifest location.
    pub path: String,
    /// What was expected versus observed, including any tool stderr.
    pub detail: String,
}

impl Finding {
    fn new(
        phase: Phase,
        code: &'static str,
        path: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Finding {
            phase,
            code,
            path: path.into(),
            detail: detail.into(),
        }
    }
}

/// A reviewed path-to-digest manifest.
///
/// Opaque: verification is the only consumer, so it exposes no accessors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    entries: BTreeMap<String, String>,
}

/// One operator hashing tool. Private: callers never choose or inspect it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Tool {
    program: String,
    args: Vec<String>,
}

impl Tool {
    fn new(program: &str, args: &[&str]) -> Self {
        Tool {
            program: program.to_string(),
            args: args.iter().map(|a| (*a).to_string()).collect(),
        }
    }

    /// Ordered candidates for this host.
    ///
    /// Read-only: constructs no process and installs nothing. Linux prefers
    /// `sha256sum`; macOS prefers `shasum -a 256`, which is in the base system.
    fn candidates_for_host() -> Vec<Tool> {
        if cfg!(target_os = "macos") {
            vec![
                Tool::new("shasum", &["-a", "256"]),
                Tool::new("sha256sum", &[]),
            ]
        } else {
            vec![
                Tool::new("sha256sum", &[]),
                Tool::new("shasum", &["-a", "256"]),
            ]
        }
    }

    fn rendered(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

/// Why a hash attempt did not yield a digest.
enum HashError {
    /// The tool was not found. Only this advances to the next
    /// candidate: a tool that is absent has said nothing about the file.
    NotSpawnable(String),
    /// The tool ran and its result is unusable. This **fails closed**: a tool
    /// that ran and refused, or emitted something unparsable, is a real answer
    /// about this file, and retrying a different tool would be shopping for a
    /// second opinion until one agrees.
    Refusal(Finding),
}

/// The result of one verification run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    /// Every refusal reason, in phase order.
    pub findings: Vec<Finding>,
    /// Repo-relative paths whose bytes matched, sorted.
    pub verified: Vec<String>,
    /// The invocation actually used, once one was pinned.
    pub tool: Option<String>,
}

impl Report {
    /// `true` only when nothing was refused and at least one file was checked.
    ///
    /// An empty run is not a pass: a manifest naming nothing would otherwise
    /// verify trivially.
    #[must_use]
    pub fn is_verified(&self) -> bool {
        self.findings.is_empty() && !self.verified.is_empty()
    }

    /// Render for the gate log.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "SNAPSHOT tool: {}",
            self.tool.as_deref().unwrap_or("none pinned")
        );
        let _ = writeln!(out, "SNAPSHOT verified files: {}", self.verified.len());
        if self.findings.is_empty() && !self.verified.is_empty() {
            let _ = writeln!(
                out,
                "PASS source_snapshot: {} reviewed files match the manifest byte for byte, and the \
                 discovered set equals the manifest set. Digests come from the operator's SHA-256 \
                 tool; no cryptography is implemented here. Not a claim about the hashing binary's \
                 authenticity, runtime semantics, or package dependencies.",
                self.verified.len()
            );
        } else {
            for finding in &self.findings {
                let _ = writeln!(
                    out,
                    "FAIL source_snapshot[{}] {}: {} -- {}",
                    finding.phase.as_str(),
                    finding.code,
                    finding.path,
                    finding.detail
                );
            }
        }
        out
    }
}

/// `true` when `text` is exactly 64 lowercase hexadecimal digits.
fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Validate a repo-relative manifest path.
fn check_relative_path(path: &str, limits: Limits) -> Result<(), (&'static str, String)> {
    // A newline in a path would let one file forge a second line of the
    // hashing tool's output, so the line-oriented parse below can never be
    // trusted for such a path. Reject before any tool sees it.
    if path.contains('\n') || path.contains('\r') {
        return Err((
            "path_has_newline",
            "paths may not contain a newline or carriage return: the hashing tool's output is \
             line-oriented and such a path could forge an additional output line"
                .to_string(),
        ));
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        return Err((
            "path_not_relative",
            "manifest paths are repo-relative; an absolute path is not comparable across hosts"
                .to_string(),
        ));
    }
    let mut depth = 0usize;
    for component in candidate.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir | Component::ParentDir => {
                return Err((
                    "path_has_dot_component",
                    "manifest paths may not contain `.` or `..`".to_string(),
                ));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err((
                    "path_not_normal",
                    "manifest paths may contain only normal components".to_string(),
                ));
            }
        }
    }
    if depth == 0 {
        return Err(("path_not_normal", "manifest path is empty".to_string()));
    }
    if depth > limits.max_depth {
        return Err((
            "path_depth_exceeded",
            format!(
                "path depth {depth} exceeds the limit of {}",
                limits.max_depth
            ),
        ));
    }
    Ok(())
}

/// Parse a reviewed manifest.
///
/// # Errors
///
/// Returns a message when the document cannot be parsed or is structurally
/// unusable. A manifest that cannot be read is a hard failure: verification
/// that cannot load its expectations has not passed.
pub fn parse_manifest(bytes: &[u8], limits: Limits) -> Result<Manifest, String> {
    let limits = limits.clamped();
    if bytes.len() > limits.max_manifest_bytes {
        return Err(format!(
            "manifest is {} bytes, over the {} byte limit",
            bytes.len(),
            limits.max_manifest_bytes
        ));
    }
    let document = parse(bytes, JsonLimits::default())
        .map_err(|error| format!("source snapshot manifest: {error}"))?;
    let root = document
        .as_object()
        .ok_or_else(|| "manifest root is not an object".to_string())?;

    let schema = root
        .get("schema")
        .and_then(Json::as_str)
        .ok_or_else(|| "manifest `schema` must be a string".to_string())?;
    if schema != SNAPSHOT_SCHEMA {
        return Err(format!(
            "manifest schema is `{schema}`, this verifier implements `{SNAPSHOT_SCHEMA}`"
        ));
    }

    let files = root
        .get("files")
        .ok_or_else(|| "manifest has no `files` object".to_string())?
        .as_object()
        .ok_or_else(|| "manifest `files` is not an object".to_string())?;
    if files.is_empty() {
        return Err(
            "manifest `files` is empty; an empty manifest would verify trivially".to_string(),
        );
    }
    if files.len() > limits.max_files {
        return Err(format!(
            "manifest names {} files, over the limit of {}",
            files.len(),
            limits.max_files
        ));
    }

    let mut entries: BTreeMap<String, String> = BTreeMap::new();
    let mut problems: Vec<String> = Vec::new();
    for (path, value) in files {
        if let Err((code, detail)) = check_relative_path(path, limits) {
            problems.push(format!("{path:?}: {code}: {detail}"));
            continue;
        }
        let Some(digest) = value.as_str() else {
            problems.push(format!(
                "{path}: expected_string: digest must be a string, found {}",
                value.kind()
            ));
            continue;
        };
        if !is_sha256_hex(digest) {
            problems.push(format!(
                "{path}: malformed_digest: expected exactly 64 lowercase hex digits, found `{digest}`"
            ));
            continue;
        }
        if entries.insert(path.clone(), digest.to_string()).is_some() {
            problems.push(format!("{path}: duplicate_manifest_path"));
        }
    }
    if !problems.is_empty() {
        return Err(format!(
            "source snapshot manifest is malformed: {}",
            problems.join("; ")
        ));
    }

    Ok(Manifest { entries })
}

/// Convert an absolute path inside `root` to its repo-relative form.
fn repo_relative(path: &Path, root: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rest.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?.to_string()),
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

// Check every component: symlink_metadata on the leaf alone follows linked
// ancestors. The canonical check also makes the containment premise explicit.
fn checked_path(root: &Path, relative: &str) -> Result<PathBuf, Finding> {
    let mut path = root.to_path_buf();
    for component in Path::new(relative).components() {
        let Component::Normal(component) = component else {
            return Err(Finding::new(
                Phase::Discovery,
                "escapes_root",
                relative,
                "path is not a normal repository-relative path",
            ));
        };
        path.push(component);
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            Finding::new(
                Phase::Discovery,
                "missing_file",
                relative,
                format!("cannot inspect path component {}: {error}", path.display()),
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(Finding::new(
                Phase::Discovery,
                "symlink_rejected",
                relative,
                format!("path component {} is a symlink", path.display()),
            ));
        }
    }
    let canonical = std::fs::canonicalize(&path).map_err(|error| {
        Finding::new(
            Phase::Discovery,
            "missing_file",
            relative,
            format!("cannot canonicalize source path: {error}"),
        )
    })?;
    if !canonical.starts_with(root) {
        return Err(Finding::new(
            Phase::Discovery,
            "escapes_root",
            relative,
            "canonical source path is outside workspace",
        ));
    }
    Ok(canonical)
}

/// Walk one scan root, collecting regular files and recording refusals.
///
/// Nothing is silently skipped: a symlink, a non-regular entry, a newline in a
/// name, an escaping path or an unreadable directory is a finding.
fn walk(
    root: &Path,
    scan_root: &str,
    limits: Limits,
    discovered: &mut BTreeSet<String>,
    findings: &mut Vec<Finding>,
    visited: &mut usize,
) {
    let start = match checked_path(root, scan_root) {
        Ok(path) => path,
        Err(finding) => {
            findings.push(finding);
            return;
        }
    };
    match std::fs::symlink_metadata(&start) {
        Err(error) => {
            findings.push(Finding::new(
                Phase::Discovery,
                "scan_root_missing",
                scan_root,
                format!("cannot stat reviewed scan root: {error}"),
            ));
            return;
        }
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                findings.push(Finding::new(
                    Phase::Discovery,
                    "symlink_rejected",
                    scan_root,
                    "scan root is a symlink; it is not followed",
                ));
                return;
            }
            if !metadata.is_dir() {
                findings.push(Finding::new(
                    Phase::Discovery,
                    "not_regular_file",
                    scan_root,
                    "scan root is not a directory",
                ));
                return;
            }
        }
    }

    let mut stack: Vec<PathBuf> = vec![start];
    while let Some(directory) = stack.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                findings.push(Finding::new(
                    Phase::Discovery,
                    "scan_root_missing",
                    repo_relative(&directory, root).unwrap_or_else(|| scan_root.to_string()),
                    format!("cannot read directory: {error}"),
                ));
                continue;
            }
        };
        for entry in entries {
            if *visited >= MAX_VISITED_ENTRIES {
                findings.push(Finding::new(
                    Phase::Discovery,
                    "traversal_limit_exceeded",
                    scan_root,
                    format!("more than {MAX_VISITED_ENTRIES} directory entries encountered"),
                ));
                return;
            }
            *visited += 1;
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    findings.push(Finding::new(
                        Phase::Discovery,
                        "scan_root_missing",
                        scan_root,
                        format!("cannot read directory entry: {error}"),
                    ));
                    continue;
                }
            };
            let path = entry.path();
            let Some(relative) = repo_relative(&path, root) else {
                findings.push(Finding::new(
                    Phase::Discovery,
                    "escapes_root",
                    path.display().to_string(),
                    "discovered path is not inside the workspace root",
                ));
                continue;
            };
            if relative.contains('\n') || relative.contains('\r') {
                findings.push(Finding::new(
                    Phase::Discovery,
                    "path_has_newline",
                    relative.escape_debug().to_string(),
                    "a discovered path contains a newline; it is never hashed, because the \
                     hashing tool's output is line-oriented",
                ));
                continue;
            }
            // symlink_metadata does not follow: a link is reported as a link
            // rather than silently hashed through to its target.
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    findings.push(Finding::new(
                        Phase::Discovery,
                        "not_regular_file",
                        relative,
                        format!("cannot stat entry: {error}"),
                    ));
                    continue;
                }
            };
            let file_type = metadata.file_type();
            if file_type.is_symlink() {
                findings.push(Finding::new(
                    Phase::Discovery,
                    "symlink_rejected",
                    relative,
                    "symlinks are not followed and not hashed; reviewed source must be real files",
                ));
                continue;
            }
            if file_type.is_dir() {
                if relative.split('/').count() >= limits.max_depth {
                    findings.push(Finding::new(
                        Phase::Discovery,
                        "path_depth_exceeded",
                        relative,
                        format!("directory depth reaches the limit of {}", limits.max_depth),
                    ));
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                findings.push(Finding::new(
                    Phase::Discovery,
                    "not_regular_file",
                    relative,
                    "entry is neither a regular file nor a directory",
                ));
                continue;
            }
            if discovered.len() >= limits.max_files {
                findings.push(Finding::new(
                    Phase::Discovery,
                    "file_count_exceeded",
                    relative,
                    format!("more than {} files discovered", limits.max_files),
                ));
                return;
            }
            discovered.insert(relative);
        }
    }
}

/// Hash one file with `tool`, returning its lowercase hex digest.
fn hash_one(tool: &Tool, root: &Path, relative: &str) -> Result<String, HashError> {
    let absolute = checked_path(root, relative).map_err(HashError::Refusal)?;
    let mut command = Command::new(&tool.program);
    for argument in &tool.args {
        command.arg(argument);
    }
    command.arg(&absolute);
    // stderr is captured rather than discarded so it can be quoted in the
    // finding: a refusal must carry the tool's own account of why.
    let output = match command.output() {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(HashError::NotSpawnable(format!(
                "`{}`: {error}",
                tool.rendered()
            )));
        }
        Err(error) => {
            return Err(HashError::Refusal(Finding::new(
                Phase::Tool,
                "hash_tool_failed",
                relative,
                format!("cannot start `{}`: {error}", tool.rendered()),
            )));
        }
    };
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let quoted = if stderr.is_empty() {
        "no stderr".to_string()
    } else {
        format!("stderr: {stderr}")
    };
    if !output.status.success() {
        return Err(HashError::Refusal(Finding::new(
            Phase::Tool,
            "hash_tool_failed",
            relative,
            format!("`{}` exited {}; {quoted}", tool.rendered(), output.status),
        )));
    }
    let Ok(text) = String::from_utf8(output.stdout) else {
        return Err(HashError::Refusal(Finding::new(
            Phase::Tool,
            "hash_tool_output_unparsable",
            relative,
            format!("tool output is not valid UTF-8; {quoted}"),
        )));
    };
    let mut lines = text.lines();
    let line = lines.next().unwrap_or_default();
    if lines.next().is_some() {
        return Err(HashError::Refusal(Finding::new(
            Phase::Tool,
            "hash_tool_output_unparsable",
            relative,
            format!("tool emitted more than one line for a single file; {quoted}"),
        )));
    }
    let Some((digest, echoed)) = line.split_once("  ") else {
        return Err(HashError::Refusal(Finding::new(
            Phase::Tool,
            "hash_tool_output_unparsable",
            relative,
            format!("expected `<64 hex><two spaces><path>`, got `{line}`; {quoted}"),
        )));
    };
    if !is_sha256_hex(digest) {
        return Err(HashError::Refusal(Finding::new(
            Phase::Tool,
            "hash_tool_output_unparsable",
            relative,
            format!("tool emitted `{digest}`, not 64 lowercase hex digits; {quoted}"),
        )));
    }
    if Path::new(echoed.trim_end()) != absolute.as_path() {
        return Err(HashError::Refusal(Finding::new(
            Phase::Tool,
            "hash_tool_path_mismatch",
            relative,
            format!(
                "asked for `{}`, tool reported `{}`; {quoted}",
                absolute.display(),
                echoed.trim_end()
            ),
        )));
    }
    Ok(digest.to_string())
}

/// Bind the manifest's own bytes to the reviewed operator policy. This is
/// outside the manifest's file set, so there is no self-referential digest.
pub fn verify_manifest_identity(root: &Path, relative: &str, expected: &str) -> Result<(), String> {
    check_relative_path(relative, Limits::default())
        .map_err(|(code, detail)| format!("{code}: {detail}"))?;
    if !is_sha256_hex(expected) {
        return Err("snapshot_binding_digest: expected lowercase SHA-256".into());
    }
    let root = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
    let mut absent = Vec::new();
    for tool in Tool::candidates_for_host() {
        match hash_one(&tool, &root, relative) {
            Ok(actual) if actual == expected => return Ok(()),
            Ok(actual) => {
                return Err(format!(
                    "snapshot_binding_mismatch: policy expects {expected}, operator computed {actual}"
                ));
            }
            Err(HashError::NotSpawnable(reason)) => absent.push(reason),
            Err(HashError::Refusal(finding)) => {
                return Err(format!(
                    "snapshot_binding_{}: {}",
                    finding.code, finding.detail
                ));
            }
        }
    }
    Err(format!(
        "snapshot_binding_tool_absent: {}",
        absent.join("; ")
    ))
}

/// Record the comparison of one computed digest against the manifest.
fn compare(
    manifest: &Manifest,
    relative: &str,
    digest: &str,
    verified: &mut Vec<String>,
    findings: &mut Vec<Finding>,
) {
    let expected = manifest
        .entries
        .get(relative)
        .map(String::as_str)
        .unwrap_or_default();
    if digest == expected {
        verified.push(relative.to_string());
    } else {
        findings.push(Finding::new(
            Phase::Digest,
            "digest_mismatch",
            relative,
            format!("manifest records {expected}, tool computed {digest}"),
        ));
    }
}

/// Verify the reviewed source tree under `root` against `manifest`.
///
/// # Errors
///
/// Returns a message only when `root` cannot be canonicalized. Every other
/// refusal is a [`Finding`] in the returned [`Report`], so a run reports all of
/// its reasons rather than only the first.
pub fn verify(root: &Path, manifest: &Manifest, limits: Limits) -> Result<Report, String> {
    let limits = limits.clamped();
    let root = std::fs::canonicalize(root)
        .map_err(|error| format!("cannot canonicalize workspace root: {error}"))?;
    let mut findings: Vec<Finding> = Vec::new();
    let mut verified: Vec<String> = Vec::new();

    if manifest.entries.len() > limits.max_files {
        return Err(format!(
            "manifest exceeds verification limit of {} files",
            limits.max_files
        ));
    }

    if manifest.entries.is_empty() {
        findings.push(Finding::new(
            Phase::Manifest,
            "empty_manifest",
            "$.files",
            "manifest names no files; an empty manifest would verify trivially",
        ));
        return Ok(Report {
            findings,
            verified,
            tool: None,
        });
    }

    // -- Discovery -----------------------------------------------------------
    let mut discovered: BTreeSet<String> = BTreeSet::new();
    let mut visited = 0;
    for scan_root in SCAN_ROOTS {
        walk(
            &root,
            scan_root,
            limits,
            &mut discovered,
            &mut findings,
            &mut visited,
        );
        if findings
            .iter()
            .any(|finding| finding.code == "traversal_limit_exceeded")
        {
            return Ok(Report {
                findings,
                verified,
                tool: None,
            });
        }
    }
    for named in EXPLICIT_FILES.iter().chain(EMBEDDED_INPUTS.iter()) {
        let named = *named;
        let kind = if EMBEDDED_INPUTS.contains(&named) {
            "embedded compile input"
        } else {
            "named build input"
        };
        let path = match checked_path(&root, named) {
            Ok(path) => path,
            Err(finding) => {
                findings.push(finding);
                continue;
            }
        };
        match std::fs::symlink_metadata(path) {
            Err(error) => findings.push(Finding::new(
                Phase::Discovery,
                "missing_file",
                named,
                format!("{kind} cannot be stat'd: {error}"),
            )),
            Ok(metadata) => {
                let file_type = metadata.file_type();
                if file_type.is_symlink() {
                    findings.push(Finding::new(
                        Phase::Discovery,
                        "symlink_rejected",
                        named,
                        format!("{kind} is a symlink; it is not followed"),
                    ));
                } else if file_type.is_file() {
                    discovered.insert(named.to_string());
                } else {
                    findings.push(Finding::new(
                        Phase::Discovery,
                        "not_regular_file",
                        named,
                        format!("{kind} is not a regular file"),
                    ));
                }
            }
        }
    }

    // -- Set equality, both directions ---------------------------------------
    let listed: BTreeSet<String> = manifest.entries.keys().cloned().collect();
    for extra in discovered.difference(&listed) {
        findings.push(Finding::new(
            Phase::Discovery,
            "unlisted_file",
            extra.clone(),
            "file exists in the reviewed tree but the manifest does not name it; a new file is a \
             refusal, never a silent pass",
        ));
    }
    for absent in listed.difference(&discovered) {
        findings.push(Finding::new(
            Phase::Discovery,
            "missing_file",
            absent.clone(),
            "manifest names this file but it was not discovered in the reviewed tree",
        ));
    }

    // -- Digests -------------------------------------------------------------
    let to_hash: Vec<String> = discovered.intersection(&listed).cloned().collect();
    let mut pinned: Option<Tool> = None;
    if !to_hash.is_empty() {
        let candidates = Tool::candidates_for_host();
        let mut spawn_failures: Vec<String> = Vec::new();
        for candidate in &candidates {
            match hash_one(candidate, &root, &to_hash[0]) {
                Ok(digest) => {
                    compare(manifest, &to_hash[0], &digest, &mut verified, &mut findings);
                    pinned = Some(candidate.clone());
                    break;
                }
                // The tool ran and gave an unusable answer. That is this run's
                // answer: fail closed rather than ask a different tool until
                // one agrees.
                Err(HashError::Refusal(finding)) => {
                    findings.push(finding);
                    pinned = Some(candidate.clone());
                    break;
                }
                // Absent tool: it has said nothing, so the next candidate may.
                Err(HashError::NotSpawnable(reason)) => spawn_failures.push(reason),
            }
        }
        match &pinned {
            None => findings.push(Finding::new(
                Phase::Tool,
                "hash_tool_absent",
                to_hash[0].clone(),
                format!(
                    "no operator SHA-256 tool could be started: {}. Nothing is installed by this \
                     checker, and an unavailable tool is a refusal, never a skip",
                    spawn_failures.join("; ")
                ),
            )),
            Some(tool) => {
                // One manifest is never hashed by two different tools.
                for relative in to_hash.iter().skip(1) {
                    match hash_one(tool, &root, relative) {
                        Ok(digest) => {
                            compare(manifest, relative, &digest, &mut verified, &mut findings);
                        }
                        Err(HashError::Refusal(finding)) => findings.push(finding),
                        Err(HashError::NotSpawnable(reason)) => findings.push(Finding::new(
                            Phase::Tool,
                            "hash_tool_failed",
                            relative.clone(),
                            format!("pinned tool became unavailable mid-run: {reason}"),
                        )),
                    }
                }
            }
        }
    }

    verified.sort();
    findings.sort_by(|a, b| {
        a.phase
            .cmp(&b.phase)
            .then_with(|| a.code.cmp(b.code))
            .then_with(|| a.path.cmp(&b.path))
    });

    Ok(Report {
        findings,
        verified,
        tool: pinned.as_ref().map(Tool::rendered),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_shape_is_exact() {
        assert!(is_sha256_hex(
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        ));
        assert!(!is_sha256_hex(&"a".repeat(63)));
        assert!(!is_sha256_hex(&"a".repeat(65)));
        // Uppercase is rejected: the manifest records one canonical spelling.
        assert!(!is_sha256_hex(&"A".repeat(64)));
        assert!(!is_sha256_hex(&"g".repeat(64)));
    }

    #[test]
    fn limits_are_clamped_to_hard_ceilings() {
        let wide = Limits {
            max_files: usize::MAX,
            max_depth: usize::MAX,
            max_manifest_bytes: usize::MAX,
        }
        .clamped();
        assert_eq!(wide.max_files, MAX_FILES_CEILING);
        assert_eq!(wide.max_depth, MAX_DEPTH_CEILING);
        assert_eq!(wide.max_manifest_bytes, MAX_MANIFEST_BYTES_CEILING);
        // A caller may still tighten below a ceiling.
        let tight = Limits {
            max_files: 1,
            max_depth: 1,
            max_manifest_bytes: 1,
        }
        .clamped();
        assert_eq!(tight.max_files, 1);
        assert_eq!(tight.max_depth, 1);
        assert_eq!(tight.max_manifest_bytes, 1);
    }

    #[test]
    fn manifest_paths_must_be_relative_bounded_and_single_line() {
        let limits = Limits::default();
        assert!(check_relative_path("xtask/src/main.rs", limits).is_ok());
        assert!(check_relative_path("/etc/passwd", limits).is_err());
        assert!(check_relative_path("xtask/../etc/passwd", limits).is_err());
        assert!(check_relative_path("./xtask/src/main.rs", limits).is_err());
        assert!(check_relative_path("", limits).is_err());
        // A newline would let one entry forge a second tool output line.
        let newline = check_relative_path("a\nb.rs", limits).expect_err("must reject");
        assert_eq!(newline.0, "path_has_newline");
        let carriage = check_relative_path("a\rb.rs", limits).expect_err("must reject");
        assert_eq!(carriage.0, "path_has_newline");
    }

    #[test]
    fn newline_path_is_rejected_by_the_manifest_parser() {
        let digest = "a".repeat(64);
        let manifest = format!(
            "{{\"schema\":\"fa.source_snapshot/1\",\"files\":{{\"xtask/src/a\\nb.rs\":\"{digest}\"}}}}"
        );
        let error = parse_manifest(manifest.as_bytes(), Limits::default()).expect_err("must fail");
        assert!(error.contains("path_has_newline"), "{error}");
    }

    #[test]
    fn schema_mismatch_is_a_hard_error() {
        let manifest = br#"{"schema":"fa.source_snapshot/9","files":{"a.rs":"aa"}}"#;
        let error = parse_manifest(manifest, Limits::default()).expect_err("must fail");
        assert!(error.contains("schema"), "{error}");
    }

    #[test]
    fn empty_manifest_is_a_hard_error() {
        let manifest = br#"{"schema":"fa.source_snapshot/1","files":{}}"#;
        let error = parse_manifest(manifest, Limits::default()).expect_err("must fail");
        assert!(error.contains("empty"), "{error}");
    }

    #[test]
    fn malformed_digest_is_a_hard_error() {
        let manifest = br#"{"schema":"fa.source_snapshot/1","files":{"Cargo.toml":"nothex"}}"#;
        let error = parse_manifest(manifest, Limits::default()).expect_err("must fail");
        assert!(error.contains("malformed_digest"), "{error}");
    }

    #[test]
    fn absolute_manifest_path_is_a_hard_error() {
        let digest = "a".repeat(64);
        let manifest =
            format!(r#"{{"schema":"fa.source_snapshot/1","files":{{"/etc/passwd":"{digest}"}}}}"#);
        let error = parse_manifest(manifest.as_bytes(), Limits::default()).expect_err("must fail");
        assert!(error.contains("path_not_relative"), "{error}");
    }

    #[test]
    fn oversized_manifest_is_a_hard_error() {
        let limits = Limits {
            max_manifest_bytes: 8,
            ..Limits::default()
        };
        let error =
            parse_manifest(br#"{"schema":"fa.source_snapshot/1"}"#, limits).expect_err("must fail");
        assert!(error.contains("bytes"), "{error}");
    }

    #[test]
    fn embedded_compile_inputs_are_required_and_exclude_operator_data() {
        assert_eq!(EMBEDDED_INPUTS.len(), 6);
        for expected in [
            "registry/invariants.json",
            "registry/roadmap.json",
            "registry/claims.json",
            "registry/sources.json",
            "registry/founding_concordance.json",
            "registry/vocabulary.json",
        ] {
            assert!(
                EMBEDDED_INPUTS.contains(&expected),
                "{expected} must be snapshotted"
            );
        }
        // The policy is read at run time, never embedded, so snapshotting it
        // would make the verifier depend on its own configuration.
        assert!(!EMBEDDED_INPUTS.contains(&"registry/dependency_policy.json"));
        assert!(!EXPLICIT_FILES.contains(&"registry/dependency_policy.json"));
        for named in EMBEDDED_INPUTS {
            assert!(!EXPLICIT_FILES.contains(&named), "{named} listed twice");
        }
    }

    #[test]
    fn only_an_absent_operator_tool_allows_fallback() {
        let root = std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .expect("actual workspace root");
        // A directory exists but cannot execute: this is not an absent tool.
        let unexecutable = Tool::new(root.to_str().unwrap(), &[]);
        assert!(matches!(hash_one(&unexecutable, &root, "Cargo.toml"),
            Err(HashError::Refusal(finding)) if finding.code == "hash_tool_failed"));
        // A regular source file cannot be a directory containing an executable.
        let absent = Tool::new(root.join("Cargo.toml/no-tool").to_str().unwrap(), &[]);
        assert!(matches!(
            hash_one(&absent, &root, "Cargo.toml"),
            Err(HashError::Refusal(_))
        ));
        let absent_path = root.join(".fa-nonexistent-snapshot-hash-tool");
        assert!(!absent_path.exists(), "test requires absent operator path");
        let absent = Tool::new(absent_path.to_str().unwrap(), &[]);
        assert!(matches!(
            hash_one(&absent, &root, "Cargo.toml"),
            Err(HashError::NotSpawnable(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn an_executed_operator_refusal_is_preserved() {
        let root = std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap())
            .expect("actual workspace root");
        let refusing_operator = Tool::new("/usr/bin/false", &[]);
        assert!(matches!(hash_one(&refusing_operator, &root, "Cargo.toml"),
            Err(HashError::Refusal(finding)) if finding.code == "hash_tool_failed"));
    }

    #[test]
    fn an_empty_run_is_not_a_pass() {
        let report = Report {
            findings: Vec::new(),
            verified: Vec::new(),
            tool: None,
        };
        assert!(
            !report.is_verified(),
            "a manifest that verified nothing must not report success"
        );
    }

    #[test]
    fn report_states_its_claim_boundary() {
        let report = Report {
            findings: Vec::new(),
            verified: vec!["Cargo.toml".to_string()],
            tool: Some("shasum -a 256".to_string()),
        };
        let rendered = report.render();
        assert!(rendered.contains("PASS source_snapshot"));
        assert!(
            rendered.contains("Not a claim about the hashing binary's authenticity"),
            "a passing report must not read as tool or runtime attestation"
        );
        assert!(rendered.contains("no cryptography is implemented here"));
    }
}
