//! Actual durable proposal/review corpus; no hand-selected replay archives.
use super::*;
use fa_reference::action::consequence::oversight::{CommitteeInput, ReviewWindow, action_frame};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, VERSION};
use fa_reference::evidence_view::{
    AuthorizationProjection, EvidencePartView, EvidenceViewManifest, OriginalIdentity,
    RedactionMetadata, WindowMetadata,
};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
use fa_reference::round::{Verdict, commitment};
use fa_reference::{
    Snapshot,
    strict_json::{self, Json, Limits},
};
use std::{
    collections::BTreeMap,
    fs, io,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

const TEMPLATE: &str = include_str!("../../../fixtures/supervised_publication.json");
const SIMPLE: &str = r#"{"op":"payload_at_most","maximum":128}"#;
const ROOT: [u8; 32] = [9; 32];
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(crate::test_directory::create(format!(
            "fa-policy-replay-{}-{time}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
struct Case {
    config: Config,
    config_file: PathBuf,
    candidate_file: PathBuf,
    host: FileOversight,
}
fn snapshot() -> Snapshot {
    Snapshot {
        semantic_epoch: 1,
        complete: true,
        values: BTreeMap::from([(7, b"ok".to_vec())]),
    }
}
fn spec(config: &Config, payload: &[u8]) -> ActionSpec {
    ActionSpec {
        version: VERSION,
        scope: config.profile.delivery.scope,
        target: Some(config.profile.delivery.target),
        payload: payload.to_vec(),
        required_witnesses: Vec::new(),
        policy_epoch: 0,
        deadline: ElapsedTick(100_000),
        units: payload.len().max(1) as u64,
    }
}
fn inputs(config: &Config, action: &FrozenAction) -> CommitteeInput {
    let mut views = BTreeMap::new();
    for (member, helper) in config.profile.committee.members() {
        let mut bytes = action_frame(action);
        let question = bytes.len();
        bytes.extend_from_slice(helper.question());
        let source = bytes.len();
        let evidence = b"retained original evidence";
        bytes.extend_from_slice(evidence);
        let end = bytes.len();
        let actual = ActualHelperInput::new(
            bytes,
            helper.profile_at(action.spec().policy_epoch),
            vec![
                SubmittedPart {
                    kind: PartKind::Other,
                    span: ByteSpan {
                        start: 0,
                        end: question,
                    },
                },
                SubmittedPart {
                    kind: PartKind::Question,
                    span: ByteSpan {
                        start: question,
                        end: source,
                    },
                },
                SubmittedPart {
                    kind: PartKind::Evidence {
                        source_id: 7,
                        transform_id: 1,
                    },
                    span: ByteSpan { start: source, end },
                },
            ],
            Vec::new(),
        )
        .unwrap();
        let original = OriginalIdentity {
            tenant_id: action.spec().scope.tenant,
            object_id: 7,
            generation: 1,
        };
        views.insert(
            member.clone(),
            EvidenceViewManifest::new(
                actual,
                AuthorizationProjection {
                    projection_id: helper.projection_id(),
                    policy_epoch: action.spec().policy_epoch,
                    projected_originals: vec![original],
                },
                vec![EvidencePartView {
                    input_part_index: 2,
                    original,
                    transform_id: 1,
                    redaction: RedactionMetadata::None,
                    window: WindowMetadata {
                        original_byte_len: evidence.len() as u64,
                        window_start: 0,
                        window_len: evidence.len() as u64,
                        truncated: false,
                    },
                }],
            )
            .unwrap(),
        );
    }
    CommitteeInput::capture(action, &config.profile.committee, views).unwrap()
}
fn setup(root: &Directory) -> Case {
    let config_file = root.0.join("config.json");
    fs::write(
        &config_file,
        TEMPLATE
            .replace("/operator", root.0.to_str().unwrap())
            .replacen("\"maximum\": 512", "\"maximum\": 4", 1),
    )
    .unwrap();
    let config = Config::read(&config_file).unwrap();
    let (mut host, _) = FileOversight::create(&config.store, config.profile.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let action = host
        .propose(host.revision(), 1, spec(&config, b"ok"), snapshot())
        .unwrap();
    let input = inputs(&config, &action);
    host.record_inputs(host.revision(), 1, 0, input.clone())
        .unwrap();
    host.begin_review(
        host.revision(),
        1,
        101,
        ROOT,
        ReviewWindow {
            commit_by: ElapsedTick(6),
            reveal_by: ElapsedTick(11),
        },
        snapshot(),
    )
    .unwrap();
    for member in ["alpha", "beta"] {
        let digest = commitment(101, member, &ROOT, Verdict::Allow, member.as_bytes()).unwrap();
        host.commit_review(host.revision(), 101, member, digest)
            .unwrap();
    }
    host.open_reveals(host.revision(), 101).unwrap();
    for member in ["alpha", "beta"] {
        host.reveal_review(
            host.revision(),
            101,
            member,
            Verdict::Allow,
            member.as_bytes().to_vec(),
        )
        .unwrap();
    }
    host.finish_review(host.revision(), 101, Some(&input), snapshot())
        .unwrap()
        .unwrap();
    host.propose(host.revision(), 2, spec(&config, b"blocked"), snapshot())
        .unwrap();
    let candidate_file = root.0.join("candidate.json");
    fs::write(&candidate_file, request(SIMPLE, 16, 1_048_576, 1_048_576)).unwrap();
    Case {
        config,
        config_file,
        candidate_file,
        host,
    }
}
fn request(nodes: &str, cases: usize, input_bytes: usize, output_bytes: usize) -> String {
    format!(
        r#"{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":1,"limits":{{"cases":{cases},"input_bytes":{input_bytes},"output_bytes":{output_bytes}}},"candidate":{{"generation":2,"nodes":[{nodes}]}}}}"#
    )
}
fn args(case: &Case) -> Vec<String> {
    vec![
        "policy-replay".into(),
        case.config_file.to_str().unwrap().into(),
        case.candidate_file.to_str().unwrap().into(),
    ]
}
fn run(case: &Case) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    command_with_elapsed(&args(case), &mut || Duration::ZERO, &mut out)?;
    Ok(out)
}
fn json(bytes: &[u8]) -> Json {
    strict_json::parse(bytes, Limits::default()).unwrap()
}
fn value(document: &Json) -> &Json {
    document.get("knowledge").unwrap().get("value").unwrap()
}
fn stored(case: &Case) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(&case.config.store)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

#[test]
fn operator_reports_all_original_proposals_reviews_and_exact_relaxations_read_only() {
    let root = Directory::new();
    let case = setup(&root);
    let before = case.host.inspect();
    let files = stored(&case);
    assert!(!root.0.join("evidence.json").exists());
    assert!(!root.0.join("helpers").exists());
    // A live writer still holds the owner lock, so acquiring/recovering a writer
    // or starting helpers instead of the native historical reader would fail.
    let output = run(&case).unwrap();
    let document = json(&output);
    let report = value(&document);
    assert_eq!(
        document.get("schema_version").unwrap().as_str(),
        Some("fa.policy-replay/1")
    );
    assert_eq!(report.get("case_count").unwrap().as_u64(), Some(3));
    assert_eq!(
        report.get("replay_grade").unwrap().as_str(),
        Some("decision_replay")
    );
    assert_eq!(
        report.get("authority").unwrap().as_str(),
        Some("not_issued")
    );
    assert_eq!(report.get("policy_changed").unwrap().as_bool(), Some(false));
    assert_eq!(
        report.get("requires_shadow").unwrap().as_bool(),
        Some(false)
    );
    assert_eq!(
        report.get("newly_reviewable").unwrap(),
        &json(br#"[{"kind":"proposal","attempt":2}]"#)
    );
    assert_eq!(report.get("newly_blocked").unwrap(), &json(b"[]"));
    let cases = report.get("cases").unwrap().as_array().unwrap();
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.get("id").unwrap().get("kind").unwrap().as_str() == Some("review"))
            .count(),
        1
    );
    assert_eq!(case.host.inspect(), before);
    assert_eq!(stored(&case), files);
    assert_eq!(run(&case).unwrap(), output);
}

#[test]
fn tightening_reports_newly_blocked_proposal_and_review_without_rewriting_history() {
    let root = Directory::new();
    let case = setup(&root);
    fs::write(
        &case.candidate_file,
        request(
            r#"{"op":"payload_at_most","maximum":1}"#,
            16,
            1_048_576,
            1_048_576,
        ),
    )
    .unwrap();
    let before = stored(&case);
    let document = json(&run(&case).unwrap());
    let report = value(&document);
    assert_eq!(
        report
            .get("newly_blocked")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(
        report
            .get("newly_reviewable")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    let cases = report.get("cases").unwrap().as_array().unwrap();
    assert_eq!(
        cases
            .iter()
            .filter(|case| case.get("delta").unwrap().as_str() == Some("unchanged"))
            .count(),
        1
    );
    assert_eq!(stored(&case), before);
}

#[test]
fn missing_original_observation_requires_shadow_even_when_another_any_branch_passes() {
    let root = Directory::new();
    let case = setup(&root);
    let nodes = format!(r#"{SIMPLE},{{"op":"absent","key":99}},{{"op":"any","children":[0,1]}}"#);
    fs::write(
        &case.candidate_file,
        request(&nodes, 16, 1_048_576, 1_048_576),
    )
    .unwrap();
    let document = json(&run(&case).unwrap());
    let report = value(&document);
    assert_eq!(report.get("requires_shadow").unwrap().as_bool(), Some(true));
    assert!(
        report
            .get("newly_reviewable")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty()
    );
    for case in report.get("cases").unwrap().as_array().unwrap() {
        assert_eq!(case.get("delta").unwrap().as_str(), Some("requires_shadow"));
        assert_eq!(case.get("candidate").unwrap(), &json(br#"{"kind":"unknown","reason_code":"MissingRequiredObservation","missing_nodes":[1]}"#));
    }
}

#[test]
fn resource_exhaustion_and_wrong_predecessor_return_no_prefix_or_mutation() {
    let root = Directory::new();
    let case = setup(&root);
    let before = stored(&case);
    for document in [
        request(SIMPLE, 2, 1_048_576, 1_048_576),
        request(SIMPLE, 16, 1, 1_048_576),
        request(SIMPLE, 16, 1_048_576, 1),
        request(SIMPLE, 16, 1_048_576, 1_048_576).replace("generation\":1", "generation\":0"),
    ] {
        fs::write(&case.candidate_file, document).unwrap();
        let mut out = Vec::new();
        assert!(command_with_elapsed(&args(&case), &mut || Duration::ZERO, &mut out).is_err());
        assert!(out.is_empty());
        assert_eq!(stored(&case), before);
    }
}

#[test]
fn malformed_candidates_and_unsupported_options_fail_closed() {
    let good = request(SIMPLE, 16, 1_048_576, 1_048_576);
    assert!(input::Request::decode(good.as_bytes()).is_ok());
    for bad in [
        good.replace("\"candidate\":", "\"permit\":true,\"candidate\":"),
        good.replace("\"cases\":16", "\"cases\":16,\"cases\":16"),
        good.replace("\"cases\":16", "\"cases\":16.0"),
        good.replace("\"cases\":16", "\"cases\":0"),
        good.replace("\"cases\":16", "\"cases\":5121"),
        good.replace("\"input_bytes\":1048576", "\"input_bytes\":33554433"),
        good.replace("\"output_bytes\":1048576", "\"output_bytes\":67108865"),
        good.replace("\"generation\":2", "\"generation\":1"),
        good.replace("\"maximum\":128", "\"maximum\":128,\"unchecked\":true"),
        request(r#"{"op":"not","child":0}"#, 16, 1_048_576, 1_048_576),
        request(
            r#"{"op":"payload_is","value_hex":"FF"}"#,
            16,
            1_048_576,
            1_048_576,
        ),
        good.replace("fa.policy-replay-request/1", "fa.policy-replay-request/2"),
    ] {
        assert!(
            input::Request::decode(bad.as_bytes()).is_err(),
            "accepted {bad}"
        );
    }
    for option in [
        "--guarded",
        "--credibility-activation",
        "--retain-history-anchor",
        "--approve",
    ] {
        let args = [
            "policy-replay",
            "/missing-config",
            "/missing-candidate",
            option,
            "/missing",
        ]
        .map(str::to_owned);
        assert_eq!(
            command_with_elapsed(&args, &mut || Duration::ZERO, &mut Vec::new()),
            Err(USAGE.into())
        );
        assert_eq!(crate::command(args.to_vec()), Err(USAGE.into()));
    }
    assert_eq!(command(&[], &mut Vec::new()), Err(USAGE.into()));
}

#[test]
fn deadline_checks_include_ingestion_replay_and_encoding_without_partial_output() {
    let root = Directory::new();
    let case = setup(&root);
    let before = stored(&case);
    for expire_at in 0..4 {
        let mut call = 0;
        let mut elapsed = || {
            let answer = if call >= expire_at {
                Duration::from_millis(case.config.timing.runtime_ms)
            } else {
                Duration::ZERO
            };
            call += 1;
            answer
        };
        let mut out = Vec::new();
        assert_eq!(
            command_with_elapsed(&args(&case), &mut elapsed, &mut out),
            Err(DEADLINE.into())
        );
        assert!(out.is_empty());
        assert_eq!(stored(&case), before);
    }
}

#[test]
fn empty_truncated_or_wrong_profile_histories_are_unavailable_without_writes() {
    let root = Directory::new();
    let case = setup(&root);
    let canonical = case.config.store.join("delivery.bin");
    let bytes = fs::read(&canonical).unwrap();
    fs::write(&canonical, &bytes[..bytes.len() - 1]).unwrap();
    let broken = stored(&case);
    assert!(run(&case).is_err());
    assert_eq!(stored(&case), broken);
    fs::write(&canonical, &bytes).unwrap();
    let config_bytes = fs::read_to_string(&case.config_file).unwrap();
    fs::write(
        &case.config_file,
        config_bytes.replace("\"total_units\": 4096", "\"total_units\": 4097"),
    )
    .unwrap();
    assert!(run(&case).is_err());
    fs::write(&case.config_file, config_bytes).unwrap();
    assert!(run(&case).is_ok());
    let empty_root = Directory::new();
    let empty_config_file = empty_root.0.join("config.json");
    fs::write(
        &empty_config_file,
        TEMPLATE.replace("/operator", empty_root.0.to_str().unwrap()),
    )
    .unwrap();
    let empty_config = Config::read(&empty_config_file).unwrap();
    let (_owner, _) =
        FileOversight::create(&empty_config.store, empty_config.profile.clone()).unwrap();
    let args = vec![
        "policy-replay".into(),
        empty_config_file.to_str().unwrap().into(),
        case.candidate_file.to_str().unwrap().into(),
    ];
    let mut out = Vec::new();
    assert!(
        command_with_elapsed(&args, &mut || Duration::ZERO, &mut out)
            .unwrap_err()
            .contains("Incomplete")
    );
    assert!(out.is_empty());
}

#[test]
fn absent_oversized_or_symlink_candidate_is_rejected_without_store_mutation() {
    use std::os::unix::fs::symlink;
    let root = Directory::new();
    let case = setup(&root);
    let before = stored(&case);
    fs::remove_file(&case.candidate_file).unwrap();
    assert!(run(&case).is_err());
    fs::write(&case.candidate_file, vec![b' '; input::CANDIDATE_BYTES + 1]).unwrap();
    assert!(run(&case).is_err());
    fs::remove_file(&case.candidate_file).unwrap();
    symlink(&case.config_file, &case.candidate_file).unwrap();
    assert!(run(&case).is_err());
    assert_eq!(stored(&case), before);
}

#[test]
fn failed_output_does_not_mutate_history_or_claim_publication() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let root = Directory::new();
    let case = setup(&root);
    let before = stored(&case);
    let error =
        command_with_elapsed(&args(&case), &mut || Duration::ZERO, &mut Broken).unwrap_err();
    assert!(error.contains("no authority or effect was changed"));
    assert_eq!(stored(&case), before);
}

#[test]
fn replay_survives_reopen_and_round_trips_every_closed_candidate_predicate() {
    let root = Directory::new();
    let mut case = setup(&root);
    case.host
        .propose(
            case.host.revision(),
            3,
            spec(&case.config, b"\0\"\\\n"),
            snapshot(),
        )
        .unwrap();
    let before = json(&run(&case).unwrap());
    drop(case.host);
    let (host, _) = FileOversight::open(&case.config.store, case.config.profile.clone()).unwrap();
    case.host = host;
    let after = json(&run(&case).unwrap());
    assert_eq!(value(&before).get("cases"), value(&after).get("cases"));
    assert_eq!(
        value(&after)
            .get("cases")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|case| case.get("id").unwrap().get("attempt").unwrap().as_u64() == Some(3))
            .unwrap()
            .get("action")
            .unwrap()
            .get("payload_hex")
            .unwrap()
            .as_str(),
        Some("00225c0a")
    );
    let nodes = concat!(
        "{\"op\":\"target_is\",\"target\":{\"adapter\":10,\"object\":11,\"contract_version\":1,\"expected_version\":1,\"generation\":1}},",
        "{\"op\":\"payload_is\",\"value_hex\":\"00225c0a\"},",
        "{\"op\":\"payload_at_most\",\"maximum\":128},",
        "{\"op\":\"units_at_most\",\"maximum\":128},",
        "{\"op\":\"exact_value\",\"key\":7,\"value_hex\":\"6f6b\"},",
        "{\"op\":\"absent\",\"key\":99},",
        "{\"op\":\"empty_range\",\"start\":100,\"end\":101},",
        "{\"op\":\"not\",\"child\":1},",
        "{\"op\":\"any\",\"children\":[0,1,2,3,4,5,6,7]},",
        "{\"op\":\"all\",\"children\":[8]}"
    );
    let request = request(nodes, 16, 1_048_576, 1_048_576);
    let expected = input::Request::decode(request.as_bytes())
        .unwrap()
        .candidate;
    fs::write(&case.candidate_file, request).unwrap();
    let document = json(&run(&case).unwrap());
    let actual =
        crate::config::policy_from_json(value(&document).get("candidate_policy").unwrap().clone())
            .unwrap();
    assert_eq!(actual, expected);
}
