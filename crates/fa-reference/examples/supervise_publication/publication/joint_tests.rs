//! Real supervisor, child-helper, reviewer and producer paths. Labels and
//! helper responses are synthetic; these tests measure enforcement, not skill.
use super::*;
use crate::config::{Config, CLOCK_DOMAIN};
use crate::workflow;
use fa_reference::action::consequence::congress::{CredibilityBinding, CredibilityRequirements};
use fa_reference::action::consequence::congress::credibility::{Campaign, CaseSpec, CredibilityLedger,
    EvaluationLabel, EvaluationScope, HelperGeneration, LabelSource, LabelVerdict, Observation};
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::FilePublicationInputs;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::FilePublicationProducer;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{ReviewerClient, ReviewerExpectation, ReviewClientProgress};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const TEMPLATE: &[u8] = include_bytes!("../../../fixtures/supervised_publication.json");
const MEMBER: &str = "FA_JOINT_PUBLICATION_MEMBER";
const EPOCH: &str = "FA_JOINT_PUBLICATION_EPOCH";
const JOINT: &str = r#"{"id":71,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":2,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":3,"max_member_outcomes":12}"#;
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("fa-joint-supervisor-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap(); Self(root)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("joint supervisor cleanup: {error}"); }
    }
}
fn configured(root: &Directory, epoch: u64, hold: u64) -> Config {
    let mut config = Config::decode(TEMPLATE).unwrap();
    config.store = root.0.join("store");
    config.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, config.profile.delivery.scope, 1_048_576).unwrap();
    config.timing.poll_ms = 1; config.timing.runtime_ms = 15_000; config.timing.cleanup_ms = 2_000;
    let congress = &mut config.profile.delivery.congress;
    for member in congress.members.values_mut() { member.weight = 10; }
    congress.caps.per_member = 10; congress.caps.per_cohort = 10;
    congress.continue_minimum = 5; congress.continue_hold_maximum = hold;
    congress.narrow_at = 20; congress.suspend_at = 30;
    config.programs = ["alpha", "beta"].into_iter().map(|member| (member.into(), HelperProgram::new(
        std::env::current_exe().unwrap(), root.0.clone(),
        vec!["--exact".into(), "publication::joint_tests::helper_process".into(), "--nocapture".into()],
        BTreeMap::from([(OsString::from(MEMBER), OsString::from(member)),
            (OsString::from(EPOCH), OsString::from(epoch.to_string()))])).unwrap())).collect();
    config
}
fn scope(config: &Config) -> String {
    let s = config.profile.delivery.scope;
    format!(r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#, s.tenant, s.principal, s.run, s.branch, s.authority)
}
fn inner(root: &Directory, config: &Config, fallback: bool) -> String {
    let (schema, history) = if fallback { ("fa.supervised-whole-input/2", r#","history":"exact_current_snapshot""#) }
        else { ("fa.supervised-whole-input/1", "") };
    format!(r#"{{"schema":"{schema}"{history},"source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[]}}"#,
        root.0.display(), scope(config))
}
fn wrapper(inner: &str) -> String {
    format!(r#"{{"schema":"fa.supervised-joint-publication/1","joint":{JOINT},"publication":{inner}}}"#)
}
fn inputs(epoch: u64, model: u64) -> FilePublicationInputs {
    FilePublicationInputs::new(None, Some(ActualHelperInput::new(b"Q".to_vec(), InputProfileBinding {
        profile_id: 1, profile_bytes: vec![], tokenizer_epoch: 1, policy_epoch: epoch, model_epoch: model },
        vec![SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: 0, end: 1 } }], vec![]).unwrap()))
}
fn evidence(root: &Directory, config: &Config) {
    let data = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: config.profile.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|member| (member.into(), format!("joint pipeline: {member}").into_bytes())).collect()).unwrap();
    fs::write(root.0.join("evidence.json"), data.encode()).unwrap();
}
fn document(id: u64, epoch: u64, target: fa_reference::action::ResolvedTarget) -> Vec<u8> {
    encode_command(&Command::Submit { request: id, proposal: ActorProposal {
        target, payload: b"publish".to_vec(), units: 7, deadline: ElapsedTick(100_000), expected_policy_epoch: epoch,
    } }).unwrap()
}

#[test]
fn helper_process() {
    let Ok(member) = std::env::var(MEMBER) else { return; };
    let epoch = std::env::var(EPOCH).unwrap().parse::<u64>().unwrap();
    let config = Config::decode(TEMPLATE).unwrap();
    let mut client = HelperClient::from_process_stdin(config.profile.committee.members()[&member].profile_at(epoch)).unwrap();
    let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let expected = format!("joint pipeline: {member}");
            assert!(client.input().unwrap().actual_input().submitted_bytes().windows(expected.len()).any(|bytes| bytes == expected.as_bytes()));
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn human(config: &Config, request: u64) -> std::thread::JoinHandle<()> {
    let path = config.socket(request);
    let expectation = ReviewerExpectation { reviewer: config.profile.human.reviewer_id,
        scope: config.profile.delivery.scope, clock_domain: CLOCK_DOMAIN };
    std::thread::spawn(move || {
        let start = Instant::now();
        let stream = loop {
            assert!(start.elapsed() < Duration::from_secs(10), "no independent reviewer offer");
            match UnixStream::connect(&path) {
                Ok(stream) => break stream,
                Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) =>
                    std::thread::sleep(Duration::from_millis(1)),
                Err(error) => panic!("reviewer connection: {error}"),
            }
        };
        let mut client = ReviewerClient::from_unix(stream, expectation).unwrap();
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(ReviewDecision::Approve).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}
fn executed(result: &workflow::RunResult) -> bool {
    matches!(&result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn assert_executed(result: &workflow::RunResult) {
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0); assert!(executed(result));
}
fn start(root: &Directory, hold: u64) -> (PublicationProfile, FilePublicationProducer) {
    let config = configured(root, 0, hold); evidence(root, &config);
    let (mut producer, _) = FilePublicationProducer::create(root.0.join("producer"), PublicationProducerProfile {
        source: 91, scope: config.profile.delivery.scope, feed: 41, clock_domain: CLOCK_DOMAIN, after: 0,
    }, inputs(0, 1), ElapsedTick(1_000)).unwrap();
    let publication = PublicationProfile::decode(wrapper(&inner(root, &config, false)).as_bytes()).unwrap();
    let first = document(1, 0, config.profile.delivery.target); let reviewer = human(&config, 1);
    let result = workflow::run_with_publication(config, &first, false, None, Some(&publication), || ElapsedTick(1_000)).unwrap();
    reviewer.join().unwrap(); assert_executed(&result);
    let config = configured(root, 1, hold);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(before.control.sequence, 1);
    producer.publish(1, inputs(1, 1), ElapsedTick(1_001)).unwrap();
    let second = document(2, 1, before.target); let reviewer = human(&config, 2);
    let result = workflow::continuation::submit_existing(config, &second, None, Some(&publication), || ElapsedTick(1_001)).unwrap();
    reviewer.join().unwrap(); assert_executed(&result);
    let config = configured(root, 3, hold);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(before.control.sequence, 2); assert_eq!(before.executions, 2);
    assert_eq!(before.control.ledger.charged, 14);
    (publication, producer)
}
fn activation(config: &Config) -> CredibilityActivation {
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: 10, model_generation: 1, evaluator_generation: 1, held_out_manifest: [8; 32] },
        label_owner: "independent".into(), helpers: ["alpha", "beta"].into_iter().map(|name| (name.into(), HelperGeneration { generation: 1, cohort: name.into() })).collect(),
        strata: BTreeSet::from(["publication".into()]), cases: (1..=3).map(|id| CaseSpec {
            id, stratum: "publication".into(), evidence_root: [id as u8; 32], dispatch_sequence: 2 }).collect(),
    }).unwrap();
    for id in 1..=3 {
        ledger.record_observations(id, ["alpha", "beta"].into_iter().map(|name| (name.into(),
            if id == 1 && name == "alpha" || id == 2 && name == "beta" { Observation::Hold { first_sequence: 1 } }
            else { Observation::Clear })).collect()).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "independent".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [id as u8; 32], recorded_sequence: 2,
            verdict: if id == 3 { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    let snapshot = ledger.seal(2).unwrap();
    CredibilityActivation { operation: 10, expected_control_sequence: before.control.sequence,
        expected_epoch: before.control.ledger.epoch + 1, scope: config.profile.delivery.scope,
        policy_generation: config.profile.delivery.policy.generation(), actor_profile: config.profile.delivery.actor.profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().into(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(), reducer_generation: 2 },
        stratum: "publication".into(), requirements: CredibilityRequirements { minimum_safe_cases: 1,
            minimum_violation_cases: 2, minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 500_000,
            maximum_false_positive_ppm: 0, base_weight: 10, lead_bonus_weight: 0, lead_saturation_sequences: 0,
            maximum_evidence_age: 100, maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}

#[test]
fn wrapped_profiles_create_complete_gates_and_pin_them_without_opening_sources() {
    for fallback in [false, true] {
        let root = Directory::new(); let config = configured(&root, 0, 0);
        // There is deliberately no producer or evidence file on this path.
        let base = inner(&root, &config, fallback);
        let publication = PublicationProfile::decode(wrapper(&base).as_bytes()).unwrap();
        let expected = publication.joint_policy().unwrap();
        let (host, _) = publication.create(&config.store, config.profile.clone()).unwrap();
        assert_eq!(host.revision(), if fallback { 5 } else { 4 });
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(expected.joint));
        let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
        host.check_joint_publication_profile(expected).unwrap();
        assert_eq!(FileOversight::read_joint_publication(&config.store, &config.profile, expected).unwrap().journal, host.inspect());
        drop(host);
        fs::write(config.store.join("delivery.pending"), b"preserve mismatch staging").unwrap();
        for wrong in [wrapper(&base).replace("\"id\":71", "\"id\":72"),
            wrapper(&base).replace("\"source\":41", "\"source\":42"),
            wrapper(&base).replace("\"max_age_ms\":5000", "\"max_age_ms\":4999")] {
            let wrong = PublicationProfile::decode(wrong.as_bytes()).unwrap();
            assert!(wrong.open(&config.store, config.profile.clone()).is_err());
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
            assert!(config.store.join("delivery.pending").exists());
        }
        let (mut host, _) = publication.open(&config.store, config.profile.clone()).unwrap();
        assert!(!host.clock_ready()); assert!(!config.store.join("delivery.pending").exists());
        let before = host.inspect();
        let wrong = PublicationProfile::decode(wrapper(&base).replace("\"id\":71", "\"id\":72").as_bytes()).unwrap();
        assert!(wrong.prepare(&mut host, 1).err().unwrap().contains("Binding"));
        assert_eq!(host.inspect(), before);
    }
}

#[test]
fn malformed_joint_fields_or_nested_wrappers_never_select_a_legacy_fallback() {
    let root = Directory::new(); let config = configured(&root, 0, 0);
    let base = inner(&root, &config, false); let valid = wrapper(&base);
    assert!(PublicationProfile::decode(valid.as_bytes()).is_ok());
    for invalid in [
        valid.replace("\"id\":71", "\"id\":0"), valid.replace("\"id\":71", "\"id\":71,\"id\":72"),
        valid.replace("\"id\":71", "\"id\":71,\"allow\":true"),
        valid.replace("\"max_cases\":3", "\"max_cases\":4097"),
        valid.replace("\"maximum_escape_ppm\":0", "\"maximum_escape_ppm\":1000001"),
        valid.replace("\"minimum_safe_roots\":1", "\"minimum_safe_roots\":0"),
        valid.replace("\"max_member_outcomes\":12", "\"max_member_outcomes\":-1"),
        valid.replace("\"generation\":1", "\"generation\":1,\"weights\":{}"),
        valid.replace("\"joint\":", "\"unknown\":"), wrapper(&valid),
        valid.replace("\"requests\":[]", r#""requests":[{"kind":"absent_key","key":1}]"#),
    ] { assert!(PublicationProfile::decode(invalid.as_bytes()).is_err(), "{invalid}"); }
    assert!(PublicationProfile::decode(&vec![b' '; PROFILE_BYTES + 1]).is_err());
    let legacy = PublicationProfile::decode(base.as_bytes()).unwrap();
    assert!(legacy.joint_policy().is_none());
    let (host, _) = legacy.create(&config.store, config.profile.clone()).unwrap();
    assert_eq!(host.held_out_joint_policy().unwrap(), None); drop(host);
    let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
    assert!(PublicationProfile::decode(valid.as_bytes()).unwrap().open(&config.store, config.profile.clone()).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
}

#[test]
fn qualified_supervision_keeps_final_view_validation_and_receipt_only_retries() {
    for changed in [false, true] {
        let root = Directory::new(); let (publication, mut producer) = start(&root, 0);
        let config = configured(&root, 3, 0); let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        let capsule = root.0.join("qualification.bin"); fs::write(&capsule, activation(&config).encode_reference().unwrap()).unwrap();
        let next = document(3, before.control.ledger.epoch + 2, before.target);
        producer.publish(2, inputs(3, 1), ElapsedTick(1_002)).unwrap();
        let reviewer = human(&config, 3); let store = config.store.clone(); let bootstrap = config.profile.clone();
        let mut observed_dispatch = false;
        let result = workflow::continuation::submit_with_credibility(config, &next, None, Some(&publication), Some(&capsule), || {
            if !observed_dispatch {
                let disk = FileOversight::read_publication(&store, &bootstrap).unwrap();
                if disk.control.ledger.charged == 21 && disk.executions == 2 {
                    producer.publish(3, inputs(3, if changed { 2 } else { 1 }), ElapsedTick(1_002)).unwrap();
                    observed_dispatch = true;
                }
            }
            ElapsedTick(1_002)
        }).unwrap();
        reviewer.join().unwrap(); assert!(observed_dispatch);
        assert_eq!(executed(&result), !changed); assert_eq!(result.cleanup_pending, 0);
        if !changed { assert_executed(&result); }
        let config = configured(&root, 4, 0);
        let history = FileOversight::read_joint_publication(&config.store, &config.profile, publication.joint_policy().unwrap()).unwrap();
        assert_eq!(history.promotions.len(), 1); assert!(history.promotions[0].1.qualified());
        assert_eq!(history.journal.executions, if changed { 2 } else { 3 });
        assert_eq!(history.journal.control.ledger.charged, if changed { 14 } else { 21 });
        assert_eq!(history.journal.control.ledger.reserved, 0);
        if !changed {
            drop(producer);
            fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
            fs::remove_file(root.0.join("evidence.json")).unwrap(); fs::remove_file(&capsule).unwrap();
            let mut config = configured(&root, 4, 0); config.programs.clear();
            let replay = workflow::continuation::submit_with_credibility(config, &next, None, Some(&publication), Some(&capsule), || ElapsedTick(200_000)).unwrap();
            assert_executed(&replay);
            let config = configured(&root, 4, 0);
            let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
            assert_eq!(after.executions, 3); assert_eq!(after.control.ledger.charged, 21);
            assert!(!config.socket(3).exists());
        }
    }
}

#[test]
fn joint_regression_refuses_before_helper_launch_or_an_actor_request_is_recorded() {
    let root = Directory::new(); let (publication, _producer) = start(&root, 5);
    let mut config = configured(&root, 3, 5);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let next = document(3, before.control.ledger.epoch + 2, before.target);
    let capsule = root.0.join("qualification.bin"); fs::write(&capsule, activation(&config).encode_reference().unwrap()).unwrap();
    config.programs.clear();
    assert!(workflow::continuation::submit_with_credibility(config, &next, None, Some(&publication), Some(&capsule), || ElapsedTick(1_002)).is_err());
    let config = configured(&root, 3, 5);
    let history = FileOversight::read_joint_publication(&config.store, &config.profile, publication.joint_policy().unwrap()).unwrap();
    assert!(history.promotions.is_empty());
    assert_eq!(history.journal.executions, before.executions);
    assert_eq!(history.journal.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(history.journal.control.ledger.stages.len(), before.control.ledger.stages.len());
    assert!(!config.socket(3).exists());
}
