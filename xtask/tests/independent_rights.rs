//! Differential tests against the ORIGINAL native Journal writer and replay.
//! The producer links the same production source files included by the crates;
//! it does not substitute a Rights implementation or manufacture authority.
//! The independent verifier executable/library never links these native cores.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use xtask::independent_verify::{
    ExpectedContext, Limits, RecoveryReserve, VerificationReport, verify,
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        for attempt in 0..100 {
            let path = std::env::temp_dir().join(format!(
                "fa-independent-native-{}-{nonce}-{attempt}", std::process::id(),
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create native fixture scratch: {error}"),
            }
        }
        panic!("cannot allocate native fixture scratch");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked(command: &mut Command) -> String {
    let output = command.output().unwrap_or_else(|error| panic!("launch {command:?}: {error}"));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stdout.is_empty() {
        eprint!("{stdout}");
    }
    if !stderr.is_empty() {
        eprint!("{stderr}");
    }
    assert!(output.status.success(), "{command:?} failed with {}", output.status);
    stdout.into_owned()
}

fn dependency(name: &str, path: &Path) -> OsString {
    let mut argument = OsString::from(format!("{name}="));
    argument.push(path);
    argument
}

fn compiler(rustc: &OsString, scratch: &Path) -> Command {
    let mut command = Command::new(rustc);
    command.args(["--edition=2024", "-Funsafe-code", "-Cdebuginfo=0", "-Copt-level=0"]);
    command.env("TMPDIR", scratch);
    command
}

struct Fixture {
    bytes: Vec<u8>,
    anchor: Vec<u8>,
    context: ExpectedContext,
    version: u8,
    accepted: bool,
    stats: BTreeMap<String, u64>,
}

fn properties(text: &str) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line.split_once('=').expect("native fixture property");
        assert!(result.insert(key.to_owned(), value.to_owned()).is_none());
    }
    result
}

fn number(values: &BTreeMap<String, String>, key: &str) -> u64 {
    values[key].parse().unwrap()
}

fn identity(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    let mut output = [0; 32];
    for (index, byte) in output.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    output
}

fn fixtures() -> &'static BTreeMap<String, Fixture> {
    static FIXTURES: OnceLock<BTreeMap<String, Fixture>> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let scratch = Scratch::new();
        let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
        let version = checked(Command::new(&rustc).arg("-vV"));
        assert!(version.contains("nightly"), "select the qualified nightly with RUSTC");
        let reference = scratch.0.join("libfa_reference.rlib");
        let journal = scratch.0.join("libfa_journal.rlib");
        let producer = scratch.0.join(format!("native-rights-fixture{}", std::env::consts::EXE_SUFFIX));

        // These are exact production entry files, not copied/sliced source or
        // test implementations. The full native crates include these files.
        checked(compiler(&rustc, &scratch.0)
            .args(["--crate-name=fa_reference", "--crate-type=rlib"])
            .arg(root.join("crates/fa-reference/src/logical_core.rs"))
            .arg("-o").arg(&reference));
        checked(compiler(&rustc, &scratch.0)
            .args(["--crate-name=fa_journal", "--crate-type=rlib"])
            .arg(root.join("crates/fa-journal/src/journal_core.rs"))
            .arg("--extern").arg(dependency("fa_reference", &reference))
            .arg("-o").arg(&journal));
        let mut search = OsString::from("dependency=");
        search.push(&scratch.0);
        checked(compiler(&rustc, &scratch.0)
            .arg("--crate-name=native_rights_fixture")
            .arg(root.join("xtask/tests/support/native_rights_fixture.rs"))
            .arg("--extern").arg(dependency("fa_reference", &reference))
            .arg("--extern").arg(dependency("fa_journal", &journal))
            .arg("-L").arg(search)
            .arg("-o").arg(&producer));
        let directory = scratch.0.join("fixtures");
        fs::create_dir(&directory).unwrap();
        checked(Command::new(&producer).arg(&directory));

        let catalog = fs::read_to_string(directory.join("catalog.tsv")).unwrap();
        let mut output = BTreeMap::new();
        for line in catalog.lines() {
            let (name, disposition) = line.split_once('\t').unwrap();
            assert!(name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
            let accepted = match disposition {
                "accept" => true,
                "reject" => false,
                other => panic!("unknown native fixture outcome: {other}"),
            };
            let context = properties(&fs::read_to_string(directory.join(format!("{name}.context"))).unwrap());
            let version: u8 = number(&context, "version").try_into().unwrap();
            assert!(matches!(version, 1 | 2));
            let stats = if accepted {
                properties(&fs::read_to_string(directory.join(format!("{name}.stats"))).unwrap())
                    .into_iter().map(|(key, value)| (key, value.parse().unwrap())).collect()
            } else {
                BTreeMap::new()
            };
            let value = Fixture {
                bytes: fs::read(directory.join(format!("{name}.journal"))).unwrap(),
                anchor: fs::read(directory.join(format!("{name}.anchor"))).unwrap(),
                version,
                accepted,
                stats,
                context: ExpectedContext {
                    run: identity(&context["run"]),
                    policy: identity(&context["policy"]),
                    total_units: number(&context, "total_units"),
                    limits: Limits {
                        max_frame_bytes: number(&context, "max_frame_bytes").try_into().unwrap(),
                        max_records: number(&context, "max_records"),
                        max_total_bytes: number(&context, "max_total_bytes"),
                    },
                    recovery: (version == 2).then(|| RecoveryReserve {
                        records: number(&context, "reserve_records"),
                        bytes: number(&context, "reserve_bytes"),
                    }),
                },
            };
            assert!(output.insert(name.to_owned(), value).is_none());
        }
        assert_eq!(output.len(), 51, "all declared native scenarios must be generated");
        // Native build products and byte files are no longer needed; only the
        // bounded corpus remains in memory and is reused by all tests.
        output
    })
}

fn assert_accounting(name: &str, fixture: &Fixture, report: &VerificationReport) {
    assert_eq!(report.format_version, fixture.version, "{name}: version");
    let accounting = &report.accounting;
    let observed = [
        ("prefix_sequence", report.prefix_head.sequence),
        ("prefix_checksum", u64::from(report.prefix_head.checksum)),
        ("prefix_bytes", report.prefix_bytes),
        ("final_sequence", report.final_head.sequence),
        ("final_checksum", u64::from(report.final_head.checksum)),
        ("final_bytes", report.final_bytes),
        ("total_units", accounting.total_units),
        ("available_units", accounting.available_units),
        ("spent_units", accounting.spent_units),
        ("reserved_units", accounting.reserved_units),
        ("dispatched_units", accounting.dispatched_units),
        ("unknown_units", accounting.unknown_units),
        ("epoch", accounting.epoch),
        ("incident_count", accounting.incident_count),
        ("reservations", accounting.reservations),
        ("reserved_count", accounting.reserved_count),
        ("dispatched_count", accounting.dispatched_count),
        ("unknown_count", accounting.unknown_count),
        ("committed_count", accounting.committed_count),
        ("aborted_count", accounting.aborted_count),
    ];
    for (key, value) in observed {
        assert_eq!(value, fixture.stats[key], "{name}: {key}");
    }
}

#[test]
fn native_lifecycles_boundaries_and_hostile_histories_agree() {
    let mut accepted = 0;
    let mut rejected = 0;
    for (name, fixture) in fixtures() {
        let result = verify(fixture.bytes.as_slice(), fixture.anchor.as_slice(), fixture.context);
        if fixture.accepted {
            let report = result.unwrap_or_else(|error| panic!("{name}: {error:?}"));
            assert_accounting(name, fixture, &report);
            accepted += 1;
        } else {
            assert!(result.is_err(), "independent verifier admitted native refusal {name}");
            rejected += 1;
        }
    }
    assert_eq!((accepted, rejected), (12, 39));
}

#[test]
fn crc_equal_native_fork_requires_exact_retained_bytes() {
    let left = &fixtures()["collision_left"];
    let right = &fixtures()["collision_right"];
    assert_ne!(left.bytes, right.bytes);
    assert_eq!(left.stats["final_sequence"], right.stats["final_sequence"]);
    assert_eq!(left.stats["final_checksum"], right.stats["final_checksum"]);
    assert!(verify(left.bytes.as_slice(), left.anchor.as_slice(), left.context).is_ok());
    assert!(verify(right.bytes.as_slice(), right.anchor.as_slice(), right.context).is_ok());
    // The producer also checks that the original CRC-head-only replay accepts
    // this fork, while its exact HistoryAnchor replay rejects it.
    assert!(verify(right.bytes.as_slice(), left.anchor.as_slice(), left.context).is_err());
}

#[test]
fn every_truncation_behind_the_native_anchor_is_rejected() {
    for name in ["full_anchor_v1", "full_anchor_v2"] {
        let fixture = &fixtures()[name];
        for end in 0..fixture.bytes.len() {
            assert!(
                verify(&fixture.bytes[..end], fixture.anchor.as_slice(), fixture.context).is_err(),
                "{name}: truncated journal at {end}",
            );
        }
        let report = verify(fixture.bytes.as_slice(), fixture.anchor.as_slice(), fixture.context).unwrap();
        assert_accounting(name, fixture, &report);
    }
}

#[test]
fn legal_suffix_is_replayed_and_partial_anchor_or_suffix_refuses() {
    for version in [1, 2] {
        let name = format!("lifecycle_v{version}");
        let fixture = &fixtures()[&name];
        let report = verify(fixture.bytes.as_slice(), fixture.anchor.as_slice(), fixture.context).unwrap();
        assert!(report.final_head.sequence > report.prefix_head.sequence);
        assert_eq!(report.accounting.unknown_units, 10);
        assert_eq!(report.accounting.dispatched_units, 20);
        assert_eq!(report.accounting.reserved_units, 5);
        assert_eq!(report.accounting.spent_units, 15);
        assert_eq!(report.accounting.available_units, 50);
        assert_eq!(report.accounting.epoch, 1);
        assert_eq!(report.accounting.incident_count, 1);
        for name in [format!("partial_anchor_v{version}"), format!("partial_suffix_v{version}")] {
            let fixture = &fixtures()[&name];
            assert!(verify(fixture.bytes.as_slice(), fixture.anchor.as_slice(), fixture.context).is_err());
        }
    }
}

#[test]
fn native_golden_vectors_remain_identical() {
    for (name, encoded) in [
        ("v1_wire_vector", include_str!("../../crates/fa-journal/tests/fixtures/revoke_epoch_v1.hex")),
        ("v2_wire_vector", include_str!("../../crates/fa-journal/tests/fixtures/recovery_reserve_v2.hex")),
    ] {
        let encoded = encoded.trim();
        let expected: Vec<_> = (0..encoded.len()).step_by(2)
            .map(|index| u8::from_str_radix(&encoded[index..index + 2], 16).unwrap()).collect();
        let fixture = &fixtures()[name];
        assert_eq!(fixture.bytes, expected, "{name}: original wire compatibility");
        assert!(verify(fixture.bytes.as_slice(), fixture.anchor.as_slice(), fixture.context).is_ok());
    }
}

struct Fragmented<'a> {
    remaining: &'a [u8],
    quantum: usize,
    calls: usize,
}

impl Read for Fragmented<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        self.calls += 1;
        if self.calls % 3 == 0 {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let count = output.len().min(self.quantum).min(self.remaining.len());
        output[..count].copy_from_slice(&self.remaining[..count]);
        self.remaining = &self.remaining[count..];
        Ok(count)
    }
}

#[test]
fn fragmented_and_interrupted_streams_preserve_native_accounting() {
    for name in ["lifecycle_v1", "lifecycle_v2"] {
        let fixture = &fixtures()[name];
        for quantum in [1, 7, 64] {
            let journal = Fragmented { remaining: &fixture.bytes, quantum, calls: 0 };
            let anchor = Fragmented { remaining: &fixture.anchor, quantum, calls: 0 };
            let report = verify(journal, anchor, fixture.context).unwrap();
            assert_accounting(name, fixture, &report);
        }
    }
}
