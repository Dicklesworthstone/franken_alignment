//! Synthetic helper decisions exercise the real process/socket/native workflow.
//! These are not trained-model accuracy or institutional independence claims.
use super::*;
use crate::config::Config;
use crate::proposal::Basis;
use fa_reference::Snapshot;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileRecoveryFloor,
    anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
    evaluation::andon::FileNativeAndonSnapshot,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{
    ReviewClientProgress, ReviewerClient, ReviewerExpectation,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, Knowledge};
use fa_reference::action::consequence::oversight::credibility::{Assessment, GroundTruth};
use fa_reference::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource,
};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::round::Verdict;
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs,
    io::{self, Cursor},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const TEMPLATE: &[u8] = include_bytes!("../../../../../fixtures/supervised_publication.json");
const HELPER: &str = "workflow::continuation::andon::tests::helper_process";
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = crate::test_directory::create(format!(
            "fa-andon-publish-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self(path)
    }
    fn bytes(&self) -> Vec<u8> {
        fs::read(self.0.join("store/delivery.bin")).unwrap()
    }
    fn launches(&self) -> usize {
        fs::read_to_string(self.0.join("helpers.log"))
            .unwrap_or_default()
            .lines()
            .count()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("Andon workflow cleanup: {error}");
        }
    }
}
fn settings() -> Settings {
    Settings::decode(br#"{
      "schema_version":"fa.native-andon-operator/1",
      "evaluation":{"domain":1,"stratum":2,"period":3,"minimum_violation_origins":1,"minimum_benign_origins":1,
        "precision_floor":{"numerator":1,"denominator":2},"recall_floor":{"numerator":1,"denominator":2},
        "false_positive_ceiling":{"numerator":1,"denominator":2},"false_stop_budget":10},
      "accounting":{"deployment":101,"evaluator":202,"initial_windows":[
        {"window":1,"start":0,"end":18446744073709551615,"false_stops_allowed":2,"cost_per_stop":7}]},
      "current_windows":[{"window":1,"start":0,"end":18446744073709551615,"false_stops_allowed":2,"cost_per_stop":7}],
      "minimum_evaluation_revision":0
    }"#).unwrap()
}
fn configured(root: &Directory, hold: bool, epoch: u64) -> Config {
    let mut config = Config::decode(TEMPLATE).unwrap();
    config.store = root.0.join("store");
    config.source = FileEvidenceSource::new(
        root.0.join("evidence.json"),
        51,
        config.profile.delivery.scope,
        1048576,
    )
    .unwrap();
    config.timing.runtime_ms = 15000;
    config.timing.poll_ms = 1;
    config.timing.cleanup_ms = 2000;
    config.programs = ["alpha", "beta"]
        .into_iter()
        .map(|name| {
            (
                name.into(),
                HelperProgram::new(
                    std::env::current_exe().unwrap(),
                    root.0.clone(),
                    vec!["--exact".into(), HELPER.into(), "--nocapture".into()],
                    BTreeMap::from([
                        (OsString::from("FA_ANDON_TEST_MEMBER"), OsString::from(name)),
                        (
                            OsString::from("FA_ANDON_TEST_EPOCH"),
                            OsString::from(epoch.to_string()),
                        ),
                        (
                            OsString::from("FA_ANDON_TEST_HOLD"),
                            OsString::from(if hold { "1" } else { "0" }),
                        ),
                        (
                            OsString::from("FA_ANDON_TEST_LOG"),
                            root.0.join("helpers.log").into_os_string(),
                        ),
                    ]),
                )
                .unwrap(),
            )
        })
        .collect();
    config
}
fn evidence(root: &Directory, config: &Config) {
    let source = EvidenceSnapshot::new(
        EvidenceIdentity {
            source: 51,
            generation: 1,
            scope: config.profile.delivery.scope,
        },
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        },
        ["alpha", "beta"]
            .into_iter()
            .map(|name| {
                (
                    name.into(),
                    format!("independent fixture input for {name}").into_bytes(),
                )
            })
            .collect(),
    )
    .unwrap();
    fs::write(root.0.join("evidence.json"), source.encode()).unwrap();
}
#[test]
fn helper_process() {
    let Ok(member) = std::env::var("FA_ANDON_TEST_MEMBER") else {
        return;
    };
    let config = Config::decode(TEMPLATE).unwrap();
    let epoch = std::env::var("FA_ANDON_TEST_EPOCH")
        .unwrap()
        .parse()
        .unwrap();
    let profile = config.profile.committee.members()[&member].profile_at(epoch);
    let mut client = HelperClient::from_process_stdin(profile).unwrap();
    let mut log = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(std::env::var("FA_ANDON_TEST_LOG").unwrap())
        .unwrap();
    writeln!(log, "{member}").unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            assert_eq!(client.input().unwrap().member(), member);
            let verdict =
                if member == "alpha" && std::env::var("FA_ANDON_TEST_HOLD").unwrap() == "1" {
                    Verdict::Hold
                } else {
                    Verdict::Allow
                };
            client.respond(verdict, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn reviewer(
    config: &Config,
    request: u64,
    decision: ReviewDecision,
) -> std::thread::JoinHandle<()> {
    let socket = config.socket(request);
    let expected = ReviewerExpectation {
        reviewer: config.profile.human.reviewer_id,
        scope: config.profile.delivery.scope,
        clock_domain: CLOCK_DOMAIN,
    };
    std::thread::spawn(move || {
        let started = Instant::now();
        let stream = loop {
            assert!(started.elapsed() < Duration::from_secs(12));
            match UnixStream::connect(&socket) {
                Ok(stream) => break stream,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                    ) =>
                {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(error) => panic!("reviewer connect: {error}"),
            }
        };
        let mut client = ReviewerClient::from_unix(stream, expected).unwrap();
        loop {
            assert!(started.elapsed() < Duration::from_secs(14));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(decision).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn document(config: &Config, request: u64, basis: Basis) -> Vec<u8> {
    crate::proposal::document(
        config,
        request,
        b"published".to_vec(),
        14000,
        ElapsedTick(1000),
        basis,
    )
    .unwrap()
}
fn run_case(
    config: Config,
    document: &[u8],
    settings: &Settings,
    mode: Mode<'_>,
) -> Result<RunResult, String> {
    let started = Instant::now();
    run(
        config,
        Submission {
            document,
            settings,
            mode,
            peers: None,
            publication: None,
        },
        started,
        || ElapsedTick(1000),
        &mut || started.elapsed(),
    )
}
fn executed(result: &RunResult) -> bool {
    matches!(
        &result.response.result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    )
}
fn anchor(path: &Path) -> FileHistoryAnchor {
    FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap()
}
fn floor(config: &Config) -> FileRecoveryFloor {
    let view = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    FileRecoveryFloor {
        journal_revision: view.revision,
        control_sequence: view.control.sequence,
        authority_epoch: view.control.ledger.epoch,
    }
}
fn inspect(config: &Config, settings: &Settings, retained: &Path) -> FileNativeAndonSnapshot {
    FileOversight::read_andon_guarded(
        &config.store,
        &config.profile,
        &settings.requirements(config, floor(config)).unwrap(),
        &settings.protocol,
        &anchor(retained),
    )
    .unwrap()
}
fn recovery(config: &Config, input: &Path, output: &Path) -> SubmitProfile {
    let floor = floor(config);
    SubmitProfile::decode(format!(r#"{{"schema_version":"fa.guarded-submit/1","minimum":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{}}},"history_anchor":{:?},"retain_history_anchor":{:?}}}"#,
        floor.journal_revision, floor.control_sequence, floor.authority_epoch, input, output).as_bytes()).unwrap()
}

#[test]
fn real_helper_hold_becomes_a_durable_case_and_independent_false_stop_after_restart() {
    let root = Directory::new();
    let config = configured(&root, true, 0);
    evidence(&root, &config);
    let document = document(&config, 7, Basis::Bootstrap);
    let settings = settings();
    let retained = root.0.join("first.anchor");
    let result = run_case(config, &document, &settings, Mode::Create(&retained)).unwrap();
    assert!(!executed(&result));
    assert_eq!(result.cleanup_pending, 0);
    assert_eq!(root.launches(), 2);
    let config = configured(&root, true, 0);
    let before = inspect(&config, &settings, &retained);
    assert_eq!(before.credibility.report.retained_cases, 1);
    assert_eq!(before.credibility.report.pending_cases, 1);
    assert_eq!(before.credibility.journal.executions, 0);
    assert_eq!(before.accounting.windows()[&1].false_stops, 0);
    let (mut host, roles) = FileOversight::open_andon_guarded(
        &config.store,
        config.profile.clone(),
        &settings.requirements(&config, floor(&config)).unwrap(),
        &settings.protocol,
        &anchor(&retained),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1001))
        .unwrap();
    let ticket = host.evaluation_ticket(7).unwrap();
    assert!(ticket.case().stopped);
    let control = host.inspect().control;
    roles
        .evaluator
        .assess(
            &mut host,
            before.credibility.journal.revision + 2,
            &ticket,
            Assessment {
                origin: 7,
                evidence_id: [17; 32],
                truth: GroundTruth::Benign,
            },
        )
        .unwrap();
    assert_eq!(host.inspect().control, control);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(
        host.andon_receipt(7)
            .unwrap()
            .confirmation()
            .helper_generations
            .len(),
        1
    );
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(root.launches(), 2);
}

#[test]
fn publication_still_requires_independent_human_approval_and_records_native_case() {
    for choice in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Directory::new();
        let config = configured(&root, false, 0);
        evidence(&root, &config);
        let document = document(&config, 7, Basis::Bootstrap);
        let settings = settings();
        let retained = root.0.join("first.anchor");
        let human = reviewer(&config, 7, choice);
        let result = run_case(config, &document, &settings, Mode::Create(&retained)).unwrap();
        human.join().unwrap();
        assert_eq!(executed(&result), choice == ReviewDecision::Approve);
        assert_eq!(result.cleanup_pending, 0);
        assert_eq!(root.launches(), 2);
        let snapshot = inspect(&configured(&root, false, 0), &settings, &retained);
        assert_eq!(
            snapshot.credibility.journal.executions,
            u64::from(choice == ReviewDecision::Approve)
        );
        assert_eq!(snapshot.credibility.report.retained_cases, 1);
        assert_eq!(snapshot.credibility.report.pending_cases, 1);
        assert_eq!(snapshot.accounting.windows()[&1].false_stops, 0);
    }
}

#[test]
fn anchored_new_work_preserves_old_cases_and_uses_the_original_next_epoch_document() {
    let root = Directory::new();
    let config = configured(&root, true, 0);
    evidence(&root, &config);
    let first = document(&config, 7, Basis::Bootstrap);
    let settings = settings();
    let input = root.0.join("first.anchor");
    run_case(config, &first, &settings, Mode::Create(&input)).unwrap();
    let config = configured(&root, false, 1);
    let second = document(&config, 8, Basis::Existing);
    let output = root.0.join("second.anchor");
    let profile = recovery(&config, &input, &output);
    let old_bytes = fs::read(&input).unwrap();
    let human = reviewer(&config, 8, ReviewDecision::Approve);
    let result = run_case(config, &second, &settings, Mode::Existing(&profile)).unwrap();
    human.join().unwrap();
    assert!(executed(&result));
    assert_eq!(root.launches(), 4);
    assert_eq!(fs::read(&input).unwrap(), old_bytes);
    let after = inspect(&configured(&root, false, 1), &settings, &output);
    assert_eq!(after.credibility.report.retained_cases, 2);
    assert_eq!(after.credibility.report.pending_cases, 2);
    assert_eq!(after.credibility.journal.executions, 1);
    assert_eq!(after.accounting.windows()[&1].false_stops, 0);
}

#[test]
fn existing_request_retry_is_query_only_even_without_source_or_helpers() {
    let root = Directory::new();
    let config = configured(&root, true, 0);
    evidence(&root, &config);
    let document = document(&config, 7, Basis::Bootstrap);
    let settings = settings();
    let input = root.0.join("first.anchor");
    let first = run_case(config, &document, &settings, Mode::Create(&input)).unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut config = configured(&root, false, 1);
    config.programs.clear();
    let output = root.0.join("retry.anchor");
    let profile = recovery(&config, &input, &output);
    let repeated = run_case(config, &document, &settings, Mode::Existing(&profile)).unwrap();
    match (&repeated.response.result, &first.response.result) {
        (
            Ok(Knowledge::Known {
                value: repeated, ..
            }),
            Ok(Knowledge::Known {
                value: original, ..
            }),
        ) => assert_eq!(repeated, original),
        other => panic!("expected original terminal outcomes: {other:?}"),
    }
    assert_eq!(root.launches(), 2);
    assert_eq!(repeated.cleanup_pending, 0);
    let snapshot = inspect(&configured(&root, false, 1), &settings, &output);
    assert_eq!(snapshot.credibility.report.retained_cases, 1);
    assert_eq!(snapshot.credibility.journal.executions, 0);
}

#[test]
fn stale_new_actor_epoch_is_not_rewritten_and_does_not_start_another_review() {
    let root = Directory::new();
    let config = configured(&root, true, 0);
    evidence(&root, &config);
    let first = document(&config, 7, Basis::Bootstrap);
    let settings = settings();
    let input = root.0.join("first.anchor");
    run_case(config, &first, &settings, Mode::Create(&input)).unwrap();
    let config = configured(&root, false, 1);
    let stale = document(&config, 8, Basis::Bootstrap);
    let output = root.0.join("refused.anchor");
    let profile = recovery(&config, &input, &output);
    let result = run_case(config, &stale, &settings, Mode::Existing(&profile)).unwrap();
    assert!(!executed(&result));
    assert_eq!(root.launches(), 2);
    let snapshot = inspect(&configured(&root, false, 1), &settings, &output);
    assert_eq!(snapshot.credibility.report.retained_cases, 1);
    assert_eq!(snapshot.credibility.journal.executions, 0);
}

#[test]
fn expired_or_wrong_inventory_admission_preserves_canonical_and_staging() {
    let root = Directory::new();
    let config = configured(&root, true, 0);
    let settings = settings();
    let first = document(&config, 7, Basis::Bootstrap);
    let input = root.0.join("first.anchor");
    let started = Instant::now();
    assert!(
        run(
            config,
            Submission {
                document: &first,
                settings: &settings,
                mode: Mode::Create(&input),
                peers: None,
                publication: None
            },
            started,
            || panic!("expired invocation clock"),
            &mut || Duration::from_secs(100)
        )
        .is_err()
    );
    assert!(!root.0.join("store").exists());
    let config = configured(&root, true, 0);
    evidence(&root, &config);
    run_case(config, &first, &settings, Mode::Create(&input)).unwrap();
    let config = configured(&root, false, 1);
    let output = root.0.join("wrong.anchor");
    let profile = recovery(&config, &input, &output);
    let before = root.bytes();
    fs::write(root.0.join("store/delivery.pending"), b"diagnostic staging").unwrap();
    let mut wrong = self::settings();
    wrong.protocol.period += 1;
    assert!(run_case(config, &first, &wrong, Mode::Existing(&profile)).is_err());
    assert_eq!(root.bytes(), before);
    assert_eq!(
        fs::read(root.0.join("store/delivery.pending")).unwrap(),
        b"diagnostic staging"
    );
    assert!(!output.exists());
    assert_eq!(root.launches(), 2);
}

#[test]
fn closed_command_routes_before_shared_options_and_output_failure_keeps_the_anchor() {
    for mode in ["resume", "create-checked", "serve-open", "inspect"] {
        let args: Vec<String> = [
            mode,
            "/missing/config",
            "/missing/document",
            "--andon",
            "/missing/profile",
            "--retain-history-anchor",
            "/missing/anchor",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(crate::command(args).unwrap_err(), USAGE);
    }
    let extra: Vec<String> = [
        "create",
        "/missing/config",
        "/missing/document",
        "--andon",
        "/missing/profile",
        "--retain-history-anchor",
        "/missing/anchor",
        "--credibility-activation",
        "/missing/evidence",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(crate::command(extra).unwrap_err(), USAGE);
    let root = Directory::new();
    let config = configured(&root, true, 0);
    evidence(&root, &config);
    let first = document(&config, 7, Basis::Bootstrap);
    let settings = settings();
    let retained = root.0.join("first.anchor");
    let result = run_case(config, &first, &settings, Mode::Create(&retained)).unwrap();
    let before = root.bytes();
    struct Fails;
    impl Write for Fails {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    assert!(super::super::guarded::emit(result, &mut Fails).is_err());
    assert_eq!(root.bytes(), before);
    assert!(retained.is_file());
    assert_eq!(root.launches(), 2);
}
