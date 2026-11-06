//! Real-binary, real-file boundaries of the independent operator command.
//! These tests exercise CLI admission and reporting; native reducer agreement
//! remains the separate independent_rights corpus, not an external-effect claim.
#![forbid(unsafe_code)]

// Only JSON syntax is shared when checking complete machine-readable responses.
#[allow(dead_code)]
#[path = "../src/json.rs"]
mod json;

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use json::Json;

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        for _ in 0..128 {
            let serial = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("fa-verify-command-{}-{serial}", std::process::id(),));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create owned test directory: {error}"),
            }
        }
        panic!("test directory namespace exhausted");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove owned test directory");
    }
}

fn hex(encoded: &str) -> Vec<u8> {
    let encoded = encoded.trim();
    assert_eq!(encoded.len() % 2, 0);
    (0..encoded.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16).unwrap())
        .collect()
}

fn v1() -> Vec<u8> {
    hex(include_str!(
        "../../crates/fa-journal/tests/fixtures/revoke_epoch_v1.hex"
    ))
}

fn v2() -> Vec<u8> {
    hex(include_str!(
        "../../crates/fa-journal/tests/fixtures/recovery_reserve_v2.hex"
    ))
}

fn context(version: u8, total: u64) -> String {
    let (limits, recovery) = if version == 1 {
        (
            "{\"max_frame_bytes\":4096,\"max_records\":64,\"max_total_bytes\":16384}",
            "null",
        )
    } else {
        assert_eq!(version, 2);
        (
            "{\"max_frame_bytes\":1024,\"max_records\":9,\"max_total_bytes\":1024}",
            "{\"records\":5,\"bytes\":160}",
        )
    };
    format!(
        "{{\"schema_version\":\"fa.rights_verification_context/1\",\"run\":\"{}\",\"policy\":\"{}\",\"total_units\":{total},\"limits\":{limits},\"recovery\":{recovery}}}",
        "01".repeat(32),
        "02".repeat(32),
    )
}

struct Fixture {
    scratch: Scratch,
    context: PathBuf,
    journal: PathBuf,
    anchor: PathBuf,
}

impl Fixture {
    fn new(version: u8) -> Self {
        let scratch = Scratch::new();
        let context_path = scratch.0.join("context.json");
        let journal = scratch.0.join("history.farj");
        let anchor = scratch.0.join("retained-prefix.farj");
        let bytes = if version == 1 { v1() } else { v2() };
        fs::write(&context_path, context(version, 100)).unwrap();
        fs::write(&journal, &bytes).unwrap();
        fs::write(&anchor, &bytes[..if version == 1 { 84 } else { 120 }]).unwrap();
        Self {
            scratch,
            context: context_path,
            journal,
            anchor,
        }
    }

    fn arguments(&self) -> Vec<OsString> {
        vec![
            "rights-journal".into(),
            "--context".into(),
            self.context.clone().into_os_string(),
            "--journal".into(),
            self.journal.clone().into_os_string(),
            "--anchor".into(),
            self.anchor.clone().into_os_string(),
        ]
    }

    fn run(&self) -> Output {
        invoke(&self.scratch.0, &self.arguments())
    }

    fn inputs(&self) -> [Vec<u8>; 3] {
        [
            fs::read(&self.context).unwrap(),
            fs::read(&self.journal).unwrap(),
            fs::read(&self.anchor).unwrap(),
        ]
    }
}

fn invoke(directory: &Path, arguments: &[OsString]) -> Output {
    assert!(!directory.join("Cargo.toml").exists());
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("verify")
        .args(arguments)
        .current_dir(directory)
        .output()
        .expect("execute the actual xtask binary outside the checkout")
}

fn document(output: &Output, success: bool) -> Json {
    assert_eq!(
        output.status.success(),
        success,
        "exit {:?}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "response must be one JSON document on stdout: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    assert_eq!(
        output.stdout.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    let value = json::parse(
        &output.stdout,
        json::Limits {
            max_bytes: 32 * 1024,
            max_depth: 16,
            max_items: 4096,
            max_string_bytes: 4096,
        },
    )
    .unwrap_or_else(|error| panic!("complete response must be JSON: {error}"));
    assert_eq!(
        text(&value, &["schema_version"]),
        "fa.rights_verification_result/1"
    );
    assert_eq!(text(&value, &["profile"]), "rights-journal");
    value
}

fn at<'a>(value: &'a Json, keys: &[&str]) -> &'a Json {
    keys.iter().fold(value, |current, key| {
        current.get(key).unwrap_or_else(|| panic!("missing {key}"))
    })
}

fn text<'a>(value: &'a Json, keys: &[&str]) -> &'a str {
    at(value, keys).as_str().expect("string field")
}

fn number(value: &Json, keys: &[&str]) -> u64 {
    at(value, keys)
        .as_u64()
        .expect("exact unsigned JSON integer")
}

fn rejected(output: &Output, code: &str, input: &str) -> Json {
    let value = document(output, false);
    assert_eq!(text(&value, &["status"]), "rejected");
    assert_eq!(text(&value, &["reason_code"]), code);
    assert_eq!(text(&value, &["error", "input"]), input);
    for field in ["context", "result", "assurances"] {
        assert_eq!(text(&value, &[field, "state"]), "Unknown");
        assert!(
            at(&value, &[field]).get("value").is_none(),
            "no successful partial {field}"
        );
    }
    value
}

fn successful(output: &Output) -> Json {
    let value = document(output, true);
    assert_eq!(text(&value, &["status"]), "verified");
    assert_eq!(text(&value, &["reason_code"]), "bounded_profile_verified");
    assert!(at(&value, &["error"]).is_null());
    for field in ["context", "result", "assurances"] {
        assert_eq!(text(&value, &[field, "state"]), "Known");
        assert!(
            at(&value, &[field])
                .get("type")
                .and_then(Json::as_str)
                .is_some()
        );
    }
    value
}

#[test]
fn actual_binary_verifies_v1_outside_checkout_and_keeps_all_proof_boundaries() {
    let fixture = Fixture::new(1);
    let inputs = fixture.inputs();
    let first = fixture.run();
    let value = successful(&first);
    assert_eq!(number(&value, &["result", "value", "format_version"]), 1);
    assert_eq!(
        number(&value, &["result", "value", "prefix", "head", "sequence"]),
        0
    );
    assert_eq!(number(&value, &["result", "value", "prefix", "bytes"]), 84);
    assert_eq!(
        number(&value, &["result", "value", "final", "head", "sequence"]),
        1
    );
    assert_eq!(
        number(&value, &["result", "value", "final", "head", "checksum"]),
        0xec85_9e14
    );
    assert_eq!(
        number(&value, &["result", "value", "final", "bytes"]),
        v1().len() as u64
    );
    let accounting = at(&value, &["result", "value", "accounting"])
        .as_object()
        .unwrap();
    assert_eq!(
        accounting.len(),
        14,
        "every native Accounting member is rendered"
    );
    for (key, expected) in [
        ("total_units", 100),
        ("available_units", 100),
        ("spent_units", 0),
        ("reserved_units", 0),
        ("dispatched_units", 0),
        ("unknown_units", 0),
        ("epoch", 1),
        ("incident_count", 0),
        ("reservations", 0),
        ("reserved_count", 0),
        ("dispatched_count", 0),
        ("unknown_count", 0),
        ("committed_count", 0),
        ("aborted_count", 0),
    ] {
        assert_eq!(accounting[key].as_u64(), Some(expected), "{key}");
    }
    assert_eq!(
        text(&value, &["result", "basis", "claim_class"]),
        "BoundedModel"
    );
    for property in [
        "authentic_origin",
        "available_for_replay",
        "anchor_custody",
        "global_generation",
        "global_control_sequence",
    ] {
        assert_eq!(
            text(&value, &["result", "basis", property, "state"]),
            "Unknown"
        );
    }
    for property in ["complete_for_contract", "semantically_valid"] {
        assert_eq!(
            text(&value, &["result", "basis", property, "state"]),
            "Known"
        );
        assert_eq!(
            at(&value, &["result", "basis", property, "value"]).as_bool(),
            Some(true)
        );
        assert_eq!(
            text(
                &value,
                &["result", "basis", property, "basis", "claim_class"]
            ),
            "BoundedModel"
        );
    }
    let assurances: BTreeMap<_, _> = at(&value, &["assurances", "value"])
        .as_array()
        .unwrap()
        .iter()
        .map(|claim| {
            (
                text(claim, &["property"]),
                text(claim, &["knowledge", "state"]),
            )
        })
        .collect();
    assert_eq!(assurances.len(), 10);
    for property in [
        "canonical_framing",
        "context_binding",
        "sequence_continuity",
        "exact_prefix_agreement",
        "rights_accounting",
    ] {
        assert_eq!(assurances[property], "Known", "{property}");
    }
    for property in [
        "authentication",
        "latest_acknowledged_history",
        "external_occurrence",
        "full_permit_law",
        "capture_completeness",
    ] {
        assert_eq!(assurances[property], "Unknown", "{property}");
    }
    let second = fixture.run();
    successful(&second);
    assert_eq!(
        first.stdout, second.stdout,
        "repeated observation is deterministic"
    );
    assert_eq!(
        fixture.inputs(),
        inputs,
        "no verifier invocation rewrites an input"
    );
}

#[test]
fn v2_selects_exact_recovery_profile_and_accepts_identical_independent_files() {
    let fixture = Fixture::new(2);
    assert_eq!(
        fs::read(&fixture.journal).unwrap(),
        fs::read(&fixture.anchor).unwrap()
    );
    let inputs = fixture.inputs();
    let value = successful(&fixture.run());
    assert_eq!(number(&value, &["result", "value", "format_version"]), 2);
    for position in ["prefix", "final"] {
        assert_eq!(number(&value, &["result", "value", position, "bytes"]), 120);
        assert_eq!(
            number(&value, &["result", "value", position, "head", "sequence"]),
            0
        );
    }
    assert_eq!(fixture.inputs(), inputs);
    let original = context(2, 100);
    for changed in [
        original.replace("\"max_total_bytes\":1024", "\"max_total_bytes\":1025"),
        original.replace("\"bytes\":160", "\"bytes\":161"),
        original.replace(
            "\"recovery\":{\"records\":5,\"bytes\":160}",
            "\"recovery\":null",
        ),
    ] {
        fs::write(&fixture.context, &changed).unwrap();
        let inputs = fixture.inputs();
        rejected(&fixture.run(), "context_mismatch", "journal");
        assert_eq!(fixture.inputs(), inputs);
    }
}

#[test]
fn argument_admission_rejects_missing_duplicate_unknown_and_non_utf8_flags_before_io() {
    let scratch = Scratch::new();
    for (arguments, code) in [
        (vec![], "unsupported_profile"),
        (vec!["receipt"], "unsupported_profile"),
        (vec!["rights-journal"], "invalid_arguments"),
        (
            vec![
                "rights-journal",
                "--context",
                "missing",
                "--journal",
                "missing",
                "--context",
                "missing",
            ],
            "invalid_arguments",
        ),
        (
            vec![
                "rights-journal",
                "--context",
                "missing",
                "--journal",
                "missing",
                "--unknown",
                "missing",
            ],
            "invalid_arguments",
        ),
        (
            vec![
                "rights-journal",
                "--context",
                "",
                "--journal",
                "missing",
                "--anchor",
                "missing",
            ],
            "invalid_arguments",
        ),
        (
            vec![
                "rights-journal",
                "--context",
                "missing",
                "--journal",
                "missing",
                "--anchor",
            ],
            "invalid_arguments",
        ),
    ] {
        let arguments = arguments
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        rejected(&invoke(&scratch.0, &arguments), code, "arguments");
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut arguments = vec![
            "rights-journal",
            "--context",
            "missing",
            "--journal",
            "missing",
            "--anchor",
            "missing",
        ]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
        arguments[1] = OsString::from_vec(b"--cont\xffext".to_vec());
        rejected(
            &invoke(&scratch.0, &arguments),
            "invalid_arguments",
            "arguments",
        );
    }
}

#[test]
fn context_semantics_are_strict_and_validated_before_either_history_is_opened() {
    let fixture = Fixture::new(1);
    fs::remove_file(&fixture.journal).unwrap();
    fs::remove_file(&fixture.anchor).unwrap();
    let original = context(1, 100);
    let candidates = [
        original.replace(
            "fa.rights_verification_context/1",
            "fa.rights_verification_context/2",
        ),
        original.replace(",\"recovery\":null", ""),
        original.replace("\"recovery\":null", "\"recovery\":null,\"extra\":1"),
        original.replace(
            "\"max_frame_bytes\":4096",
            "\"max_frame_bytes\":4096,\"extra\":1",
        ),
        original.replace("\"max_frame_bytes\":4096", "\"max_frame_bytes\":4294967296"),
        original.replace("\"max_frame_bytes\":4096", "\"max_frame_bytes\":12"),
        original.replace("\"max_records\":64,", ""),
        original.replace("\"max_records\":64", "\"max_records\":0"),
        original.replace("\"total_units\":100", "\"total_units\":-0"),
        original.replace("\"total_units\":100", "\"total_units\":1e2"),
        original.replace("\"total_units\":100", "\"total_units\":100.0"),
        original.replace("\"total_units\":100", "\"total_units\":\"100\""),
        original.replace("\"total_units\":100", "\"total_units\":true"),
        original.replace(
            "\"total_units\":100",
            "\"total_units\":18446744073709551616",
        ),
        original.replace(&"01".repeat(32), &"00".repeat(32)),
        original.replace(&"01".repeat(32), &"01".repeat(31)),
        original.replace(&"01".repeat(32), &"gg".repeat(32)),
        original.replace("\"recovery\":null", "\"recovery\":{\"records\":1}"),
        original.replace(
            "\"recovery\":null",
            "\"recovery\":{\"records\":1,\"bytes\":20}",
        ),
    ];
    for candidate in candidates {
        fs::write(&fixture.context, &candidate).unwrap();
        rejected(&fixture.run(), "invalid_context", "context");
        assert_eq!(fs::read(&fixture.context).unwrap(), candidate.as_bytes());
    }
    fs::write(&fixture.context, &original).unwrap();
    rejected(&fixture.run(), "input_io", "journal");
}

#[test]
fn malformed_duplicate_and_oversized_context_documents_fail_without_history_io() {
    let fixture = Fixture::new(1);
    fs::remove_file(&fixture.journal).unwrap();
    fs::remove_file(&fixture.anchor).unwrap();
    let original = context(1, 100);
    for candidate in [
        original
            .replace(
                "\"total_units\":100",
                "\"total_units\":100,\"total_units\":100",
            )
            .into_bytes(),
        original
            .replace(
                "\"total_units\":100",
                "\"total_units\":100,\"total_\\u0075nits\":100",
            )
            .into_bytes(),
        original
            .replace(
                "\"max_records\":64",
                "\"max_records\":64,\"max_records\":64",
            )
            .into_bytes(),
        format!("{original}{{}}").into_bytes(),
        vec![0xff],
    ] {
        fs::write(&fixture.context, &candidate).unwrap();
        let value = rejected(&fixture.run(), "invalid_context_json", "context");
        assert!(at(&value, &["error", "byte_offset"]).as_u64().is_some());
    }
    let mut exact = original.into_bytes();
    exact.resize(4096, b' ');
    fs::write(&fixture.context, &exact).unwrap();
    rejected(&fixture.run(), "input_io", "journal");
    exact.push(b' ');
    fs::write(&fixture.context, &exact).unwrap();
    let value = rejected(&fixture.run(), "input_limit_exceeded", "context");
    assert_eq!(number(&value, &["error", "byte_offset"]), 4096);
}

#[test]
fn independent_run_policy_budget_and_limits_are_never_inferred_from_history() {
    let fixture = Fixture::new(1);
    let original = context(1, 100);
    for candidate in [
        original.replace(&"01".repeat(32), &"03".repeat(32)),
        original.replace(&"02".repeat(32), &"03".repeat(32)),
        context(1, 0),
        context(1, 101),
    ] {
        fs::write(&fixture.context, candidate).unwrap();
        let inputs = fixture.inputs();
        rejected(&fixture.run(), "context_mismatch", "journal");
        assert_eq!(fixture.inputs(), inputs);
    }
    fs::write(
        &fixture.context,
        original.replace("\"max_total_bytes\":16384", "\"max_total_bytes\":84"),
    )
    .unwrap();
    rejected(&fixture.run(), "input_limit_exceeded", "journal");
}

#[test]
fn malformed_suffix_and_partial_or_changed_anchors_never_export_partial_accounting() {
    let fixture = Fixture::new(1);
    successful(&fixture.run());
    let mut bytes = v1();
    bytes.push(0);
    fs::write(&fixture.journal, &bytes).unwrap();
    let inputs = fixture.inputs();
    let value = rejected(&fixture.run(), "truncated_input", "journal");
    assert_eq!(number(&value, &["error", "record_sequence"]), 2);
    assert_eq!(fixture.inputs(), inputs);
    fs::write(&fixture.journal, v1()).unwrap();
    fs::write(&fixture.anchor, &v1()[..83]).unwrap();
    rejected(&fixture.run(), "anchor_too_short", "anchor");
    fs::write(&fixture.anchor, &v1()[..90]).unwrap();
    rejected(&fixture.run(), "anchor_not_record_boundary", "anchor");
    let mut changed = v1()[..84].to_vec();
    changed[8] ^= 1;
    fs::write(&fixture.anchor, changed).unwrap();
    rejected(&fixture.run(), "anchor_mismatch", "anchor");
    fs::write(&fixture.anchor, v1()).unwrap();
    successful(&fixture.run());
}

#[test]
fn missing_and_non_regular_files_identify_the_input_and_never_print_success() {
    let fixture = Fixture::new(1);
    let original = fixture.arguments();
    for (position, role) in [(2, "context"), (4, "journal"), (6, "anchor")] {
        let mut arguments = original.clone();
        arguments[position] = fixture.scratch.0.join("missing").into_os_string();
        let value = rejected(&invoke(&fixture.scratch.0, &arguments), "input_io", role);
        assert_eq!(text(&value, &["error", "io_kind"]), "NotFound");
        arguments[position] = fixture.scratch.0.clone().into_os_string();
        rejected(
            &invoke(&fixture.scratch.0, &arguments),
            "non_regular_input",
            role,
        );
    }
}

#[test]
fn same_file_cannot_be_its_own_retained_prefix_but_equal_independent_files_can() {
    let fixture = Fixture::new(1);
    let mut arguments = fixture.arguments();
    arguments[6] = fixture.journal.clone().into_os_string();
    rejected(
        &invoke(&fixture.scratch.0, &arguments),
        "aliased_inputs",
        "anchor",
    );
    fs::write(&fixture.anchor, v1()).unwrap();
    let inputs = fixture.inputs();
    successful(&fixture.run());
    assert_eq!(fixture.inputs(), inputs);
}

#[cfg(unix)]
#[test]
fn opened_hardlink_aliases_and_symbolic_link_inputs_are_refused() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new(1);
    let hardlink = fixture.scratch.0.join("hardlink.farj");
    fs::hard_link(&fixture.journal, &hardlink).unwrap();
    let mut arguments = fixture.arguments();
    arguments[6] = hardlink.into_os_string();
    rejected(
        &invoke(&fixture.scratch.0, &arguments),
        "aliased_inputs",
        "anchor",
    );
    for (position, role, target) in [
        (2, "context", &fixture.context),
        (4, "journal", &fixture.journal),
        (6, "anchor", &fixture.anchor),
    ] {
        let link = fixture.scratch.0.join(format!("{role}.link"));
        symlink(target, &link).unwrap();
        let mut arguments = fixture.arguments();
        arguments[position] = link.into_os_string();
        rejected(
            &invoke(&fixture.scratch.0, &arguments),
            "non_regular_input",
            role,
        );
    }
    successful(&fixture.run());
}

#[cfg(unix)]
#[test]
fn non_utf8_regular_file_paths_survive_argument_and_file_admission() {
    use std::os::unix::ffi::OsStringExt;
    let mut fixture = Fixture::new(1);
    let ordinary = fixture.run();
    successful(&ordinary);
    for (name, path) in [
        (b"context-\xff".as_slice(), &mut fixture.context),
        (b"history-\xfe".as_slice(), &mut fixture.journal),
        (b"anchor-\xfd".as_slice(), &mut fixture.anchor),
    ] {
        let renamed = fixture.scratch.0.join(OsString::from_vec(name.to_vec()));
        fs::rename(&*path, &renamed).unwrap();
        *path = renamed;
    }
    let inputs = fixture.inputs();
    let renamed = fixture.run();
    successful(&renamed);
    assert_eq!(
        ordinary.stdout, renamed.stdout,
        "reports do not lossy-convert paths into identities"
    );
    assert_eq!(fixture.inputs(), inputs);
}

// Small documented-wire fixtures below exercise CLI serialization, not native
// writer equivalence. The separate independent_rights target supplies that
// differential using the original Journal producer and Rights reducer.
fn crc32(bytes: &[u8]) -> u32 {
    let mut checksum = u32::MAX;
    for byte in bytes {
        checksum ^= u32::from(*byte);
        for _ in 0..8 {
            checksum = if checksum & 1 == 0 {
                checksum >> 1
            } else {
                (checksum >> 1) ^ 0xedb8_8320
            };
        }
    }
    !checksum
}

fn header(total: u64) -> Vec<u8> {
    let mut bytes = v1()[..84].to_vec();
    bytes[72..80].copy_from_slice(&total.to_le_bytes());
    let checksum = crc32(&bytes[..80]);
    bytes[80..84].copy_from_slice(&checksum.to_le_bytes());
    bytes
}

fn append(bytes: &mut Vec<u8>, sequence: u64, event: &[u8]) {
    let predecessor = bytes[bytes.len() - 4..].to_vec();
    let mut frame = ((12 + event.len()) as u32).to_le_bytes().to_vec();
    frame.extend_from_slice(&sequence.to_le_bytes());
    frame.extend_from_slice(&predecessor);
    frame.extend_from_slice(event);
    let checksum = crc32(&frame);
    frame.extend_from_slice(&checksum.to_le_bytes());
    bytes.extend_from_slice(&frame);
}

fn effect(tag: u8, id: u64, units: u64) -> Vec<u8> {
    let mut event = vec![tag];
    event.extend_from_slice(&id.to_le_bytes());
    event.extend_from_slice(&units.to_le_bytes());
    for field in [
        b"actor".as_slice(),
        b"publication".as_slice(),
        b"payload".as_slice(),
    ] {
        event.extend_from_slice(&(field.len() as u32).to_le_bytes());
        event.extend_from_slice(field);
    }
    event
}

fn identified(tag: u8, id: u64) -> Vec<u8> {
    let mut event = vec![tag];
    event.extend_from_slice(&id.to_le_bytes());
    event
}

#[test]
fn full_lifecycle_reporting_keeps_every_bucket_and_never_refunds_unknown_work() {
    let fixture = Fixture::new(1);
    let mut bytes = header(100);
    let mut reconcile = identified(5, 2);
    reconcile.push(1);
    let mut reset = vec![7];
    reset.extend_from_slice(&3_u32.to_le_bytes());
    for id in [2_u64, 3, 4] {
        reset.extend_from_slice(&id.to_le_bytes());
    }
    let events = [
        effect(1, 0, 7),
        effect(2, 0, 7),
        identified(3, 0),
        effect(1, 1, 5),
        effect(2, 1, 5),
        effect(1, 2, 3),
        effect(2, 2, 3),
        reconcile,
        effect(1, 3, 2),
        identified(4, 3),
        effect(1, 4, 11),
        vec![6],
        reset,
    ];
    for (index, event) in events.iter().enumerate() {
        append(&mut bytes, index as u64 + 1, event);
    }
    fs::write(&fixture.journal, &bytes).unwrap();
    let inputs = fixture.inputs();
    let value = successful(&fixture.run());
    for (key, expected) in [
        ("total_units", 100),
        ("available_units", 74),
        ("spent_units", 3),
        ("reserved_units", 11),
        ("dispatched_units", 5),
        ("unknown_units", 7),
        ("epoch", 1),
        ("incident_count", 1),
        ("reservations", 5),
        ("reserved_count", 1),
        ("dispatched_count", 1),
        ("unknown_count", 1),
        ("committed_count", 1),
        ("aborted_count", 1),
    ] {
        assert_eq!(
            number(&value, &["result", "value", "accounting", key]),
            expected,
            "{key}"
        );
    }
    assert_eq!(fixture.inputs(), inputs);
    append(&mut bytes, events.len() as u64 + 1, &identified(4, 0));
    fs::write(&fixture.journal, &bytes).unwrap();
    let inputs = fixture.inputs();
    rejected(&fixture.run(), "invalid_transition", "journal");
    assert_eq!(fixture.inputs(), inputs);
}

#[test]
fn full_width_u64_values_and_zero_budget_are_preserved_in_json_without_coercion() {
    let fixture = Fixture::new(1);
    for total in [0, (1_u64 << 53) + 1, u64::MAX] {
        fs::write(&fixture.context, context(1, total)).unwrap();
        let bytes = header(total);
        fs::write(&fixture.journal, &bytes).unwrap();
        fs::write(&fixture.anchor, &bytes).unwrap();
        let inputs = fixture.inputs();
        let value = successful(&fixture.run());
        assert_eq!(number(&value, &["context", "value", "total_units"]), total);
        assert_eq!(
            number(&value, &["result", "value", "accounting", "total_units"]),
            total
        );
        assert_eq!(
            number(
                &value,
                &["result", "value", "accounting", "available_units"]
            ),
            total
        );
        assert_eq!(fixture.inputs(), inputs);
    }
}
