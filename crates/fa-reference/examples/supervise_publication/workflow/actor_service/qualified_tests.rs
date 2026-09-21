//! Real native actor/helper/reviewer exchanges with synthetic held-out labels.
//! These fixtures test enforcement, not model accuracy or evaluator authenticity.
use super::*;
use fa_reference::action::consequence::congress::{CredibilityBinding, CredibilityRequirements};
use fa_reference::action::consequence::congress::credibility::{
    Campaign, CaseSpec, CredibilityLedger, EvaluationLabel, EvaluationScope,
    HelperGeneration, LabelSource, LabelVerdict, Observation,
};
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewClientProgress;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{ReviewDecision, ReviewPacket};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use fa_reference::action::consequence::oversight::actor_wire::{
    Command, WireError, WireResponse, decode_command, decode_response, encode_command,
};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::{self, JoinHandle};

const TEMPLATE: &[u8] = include_bytes!("../../../../fixtures/supervised_publication.json");
const CHILD: &str = "workflow::actor_service::qualified_tests::synthetic_helper_process";
const MEMBER: &str = "FA_LIVE_QUALIFIED_MEMBER";
const EPOCH: &str = "FA_LIVE_QUALIFIED_EPOCH";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-live-qual-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        for name in ["actors", "reviewers"] {
            fs::DirBuilder::new().mode(0o750).create(path.join(name)).unwrap();
        }
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("qualified live fixture cleanup: {error}"); }
    }
}

fn configured(root: &Directory, epoch: u64) -> Config {
    let mut config = Config::decode(TEMPLATE).unwrap();
    config.store = root.0.join("store");
    config.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51,
        config.profile.delivery.scope, 1_048_576).unwrap();
    config.timing.poll_ms = 1;
    config.timing.runtime_ms = 15_000;
    config.timing.cleanup_ms = 2_000;
    config.programs = ["alpha", "beta"].into_iter().map(|member| (member.to_owned(), HelperProgram::new(
        std::env::current_exe().unwrap(), root.0.clone(),
        vec!["--exact".into(), CHILD.into(), "--nocapture".into()],
        BTreeMap::from([(OsString::from(MEMBER), OsString::from(member)),
            (OsString::from(EPOCH), OsString::from(epoch.to_string()))])).unwrap())).collect();
    config
}
fn scope(config: &Config) -> String {
    let s = config.profile.delivery.scope;
    format!(r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority)
}
fn profiles(root: &Directory, config: &Config, request: u64) -> (Profile, PeerProfile) {
    let (socket, _) = UnixStream::pair().unwrap();
    let id = PeerCredentials::observe(&socket).unwrap();
    let identity = format!(r#"{{"uid":{},"gid":{},"pid":{}}}"#, id.uid(), id.gid(), id.pid());
    let actor = format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":{request},"scope":{},"socket":"{}/actors/actor.sock","supervisor":{identity},"actor":{identity},"candidate_limit":8,"connection_limit":4,"exchange_limit":1024,"runtime_ms":15000,"poll_ms":1,"reply_ms":200}}"#,
        scope(config), root.0.display());
    let reviewer = format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{},"reviewer_id":77,"socket_directory":"{}/reviewers","supervisor":{identity},"reviewer":{identity},"candidate_limit":8,"runtime_ms":15000,"poll_ms":1}}"#,
        scope(config), root.0.display());
    (Profile::decode(actor.as_bytes()).unwrap(), PeerProfile::decode(reviewer.as_bytes()).unwrap())
}
fn evidence(root: &Directory, config: &Config) {
    let source = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1,
        scope: config.profile.delivery.scope }, Snapshot { semantic_epoch: 1, complete: true,
        values: BTreeMap::from([(7, b"ok".to_vec())]) }, ["alpha", "beta"].into_iter()
        .map(|name| (name.to_owned(), format!("live qualification fixture: {name}").into_bytes())).collect()).unwrap();
    fs::write(root.0.join("evidence.json"), source.encode()).unwrap();
}
fn wait(path: &Path) {
    let started = Instant::now();
    while !path.exists() {
        assert!(started.elapsed() < Duration::from_secs(10), "missing {path:?}");
        pause(1);
    }
}
fn reviewer(profile: PeerProfile, request: u64) -> JoinHandle<ReviewPacket> {
    thread::spawn(move || {
        wait(&profile.socket(request));
        let mut client = profile.connect_client(request).unwrap();
        let started = Instant::now();
        let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    packet = Some(client.packet().unwrap().clone());
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => pause(1),
            }
        }
    })
}
fn actor(profile: Profile, document: PathBuf) -> JoinHandle<(Result<(), String>, WireResponse)> {
    thread::spawn(move || {
        wait(&profile.socket);
        let mut out = Vec::new();
        let result = client::submit(&profile, &document, &mut out);
        let response = decode_response(out.strip_suffix(b"\n").expect("original actor response")).unwrap();
        (result, response)
    })
}
fn executed(response: &WireResponse) -> bool {
    matches!(&response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}

#[test]
fn synthetic_helper_process() {
    let Ok(member) = std::env::var(MEMBER) else { return; };
    let epoch = std::env::var(EPOCH).unwrap().parse::<u64>().unwrap();
    let config = Config::decode(TEMPLATE).unwrap();
    let mut client = HelperClient::from_process_stdin(
        config.profile.committee.members()[&member].profile_at(epoch)).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let bytes = input.actual_input().submitted_bytes();
            let expected = format!("live qualification fixture: {member}");
            assert!(bytes.windows(expected.len()).any(|part| part == expected.as_bytes()));
            assert!(!bytes.windows(b"independent-evaluator".len()).any(|part| part == b"independent-evaluator"));
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        pause(1);
    }
}

fn seed(root: &Directory, publication: Option<&PublicationProfile>) {
    let config = configured(root, 0);
    evidence(root, &config);
    let (_, peer) = profiles(root, &config, 1);
    let document = encode_command(&Command::Submit { request: 1, proposal: ActorProposal {
        target: config.profile.delivery.target, payload: b"seed".to_vec(), units: 4,
        deadline: ElapsedTick(100_000), expected_policy_epoch: 0,
    }}).unwrap();
    let human = reviewer(peer.clone(), 1);
    let result = crate::workflow::run_with_publication(config, &document, false,
        Some(&peer), publication, || ElapsedTick(1000)).unwrap();
    human.join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    assert!(executed(&result.response));
}
fn activation(config: &Config, sequence: u64, epoch: u64, operation: u64, generation: u64)
    -> CredibilityActivation
{
    let frontier = sequence.max(2);
    let helpers = config.profile.delivery.congress.members.iter().map(|(id, member)| (id.clone(),
        HelperGeneration { generation: config.profile.committee.members()[id].profile_at(0).model_epoch,
            cohort: member.cohort.clone() })).collect::<BTreeMap<_, _>>();
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: operation, model_generation: 1, evaluator_generation: 1,
            held_out_manifest: [42; 32] }, label_owner: "independent-evaluator".into(), helpers: helpers.clone(),
        strata: BTreeSet::from(["publication".into()]), cases: (1..=2).map(|id| CaseSpec {
            id, stratum: "publication".into(), evidence_root: [id as u8; 32], dispatch_sequence: frontier,
        }).collect(),
    }).unwrap();
    for id in 1..=2 {
        let observation = if id == 1 { Observation::Clear } else { Observation::Hold { first_sequence: frontier - 1 } };
        ledger.record_observations(id, helpers.keys().map(|name| (name.clone(), observation)).collect()).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "independent-evaluator".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [id as u8; 32], recorded_sequence: frontier,
            verdict: if id == 1 { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    let snapshot = ledger.seal(frontier).unwrap();
    CredibilityActivation { operation, expected_control_sequence: sequence, expected_epoch: epoch,
        scope: config.profile.delivery.scope, policy_generation: config.profile.delivery.policy.generation(),
        actor_profile: config.profile.delivery.actor.profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().into(),
            helpers: snapshot.helpers().clone(), strata: snapshot.strata().clone(), reducer_generation: generation },
        stratum: "publication".into(), requirements: CredibilityRequirements { minimum_safe_cases: 1,
            minimum_violation_cases: 1, minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 5, lead_bonus_weight: 0, lead_saturation_sequences: 0,
            maximum_evidence_age: 100, maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}
fn prepared(root: &Directory, request: u64, operation: u64, generation: u64)
    -> (Config, Profile, PeerProfile, PathBuf, PathBuf, CredibilityActivation)
{
    let config = configured(root, 0);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let epoch = before.control.ledger.epoch.checked_add(1).unwrap();
    let config = configured(root, epoch + 1);
    let qualification = activation(&config, before.control.sequence, epoch, operation, generation);
    let capsule = root.0.join(format!("qualification-{operation}.bin"));
    fs::write(&capsule, qualification.encode_reference().unwrap()).unwrap();
    let document = crate::proposal::document_qualified(&config, request, b"network".to_vec(),
        14_000, ElapsedTick(1000), &qualification).unwrap();
    let path = root.0.join(format!("submit-{request}.json"));
    fs::write(&path, document).unwrap();
    let (actor, peer) = profiles(root, &config, request);
    (config, actor, peer, path, capsule, qualification)
}
fn qualified(root: &Directory) -> (PathBuf, PathBuf) {
    seed(root, None);
    let (config, actor_profile, peer, document, capsule, qualification) = prepared(root, 7, 100, 2);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let original = fs::read(&document).unwrap();
    let submitted = actor(actor_profile.clone(), document.clone());
    let human = reviewer(peer.clone(), 7);
    serve_with_credibility(config, &actor_profile, &peer, None, true,
        Some(&capsule), || ElapsedTick(1001)).unwrap();
    let packet = human.join().unwrap();
    let (status, response) = submitted.join().unwrap();
    assert!(status.is_ok(), "{status:?}");
    assert!(executed(&response));
    assert_eq!(packet.action().spec().policy_epoch, qualification.expected_epoch + 1);
    assert_eq!(packet.action().spec().payload, b"network");
    assert_eq!(fs::read(&document).unwrap(), original);
    let config = configured(root, 2);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 2);
    assert_eq!(after.payload, b"network");
    assert_eq!(after.control.sequence, before.control.sequence + 2);
    assert_eq!(after.control.ledger.epoch, before.control.ledger.epoch + 2);
    assert_eq!(after.control.ledger.charged, 11);
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(after.control.ledger.available, config.profile.delivery.total - 11);
    assert!(!actor_profile.socket.exists());
    (document, capsule)
}

#[test]
fn qualified_live_service_consumes_both_keys_and_preserves_original_actor_bytes() {
    qualified(&Directory::new());
}

#[test]
fn recorded_live_requests_skip_missing_qualification_and_source_even_for_conflicts() {
    let root = Directory::new();
    let (document, capsule) = qualified(&root);
    fs::remove_file(&capsule).unwrap();
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    for conflict in [false, true] {
        let mut config = configured(&root, 0); config.programs.clear();
        let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        let (actor_profile, peer) = profiles(&root, &config, 7);
        if conflict {
            let Command::Submit { request, mut proposal } = decode_command(&fs::read(&document).unwrap()).unwrap() else { panic!("submit"); };
            proposal.payload[0] ^= 1;
            fs::write(&document, encode_command(&Command::Submit { request, proposal }).unwrap()).unwrap();
        }
        let submitted = actor(actor_profile.clone(), document.clone());
        serve_with_credibility(config, &actor_profile, &peer, None, true,
            Some(&capsule), || ElapsedTick(200_000)).unwrap();
        let (status, response) = submitted.join().unwrap();
        if conflict {
            assert!(status.is_err());
            assert_eq!(response.result, Err(WireError::IdempotencyConflict));
        } else { assert!(status.is_ok()); assert!(executed(&response)); }
        let config = configured(&root, 0);
        let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(after.executions, before.executions);
        assert_eq!(after.control.sequence, before.control.sequence);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.payload, before.payload);
        assert!(!peer.socket(7).exists());
    }
}

#[test]
fn invalid_qualification_refuses_before_new_actor_or_source_admission() {
    for defect in 0..6 {
        let root = Directory::new(); seed(&root, None);
        let (mut config, actor_profile, peer, _, capsule, mut request) = prepared(&root, 7, 100, 2);
        let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        match defect {
            0 => fs::remove_file(&capsule).unwrap(),
            1 => fs::write(&capsule, b"not a canonical capsule").unwrap(),
            2 => request.scope.tenant += 1,
            3 => request.expected_control_sequence += 1,
            4 => request.expected_epoch += 1,
            _ => request.requirements.minimum_safe_cases = 2,
        }
        if defect >= 2 { fs::write(&capsule, request.encode_reference().unwrap()).unwrap(); }
        config.programs.clear(); fs::remove_file(root.0.join("evidence.json")).unwrap();
        assert!(serve_with_credibility(config, &actor_profile, &peer, None, true,
            Some(&capsule), || ElapsedTick(1001)).is_err());
        let config = configured(&root, 0);
        let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(after.revision, before.revision + 1); // Recovery only, no source acquisition.
        assert_eq!(after.control.sequence, before.control.sequence);
        assert_eq!(after.control.ledger.stages, before.control.ledger.stages);
        assert_eq!(after.executions, 1);
        assert_eq!(after.control.ledger.charged, 4);
        assert!(!actor_profile.socket.exists());
        assert!(!peer.socket(7).exists());
    }
}

#[test]
fn stale_actor_epoch_is_not_rewritten_after_operator_qualification() {
    let root = Directory::new(); seed(&root, None);
    let (config, actor_profile, peer, document, capsule, qualification) = prepared(&root, 7, 100, 2);
    let Command::Submit { request, mut proposal } = decode_command(&fs::read(&document).unwrap()).unwrap() else { panic!("submit"); };
    proposal.expected_policy_epoch = qualification.expected_epoch;
    let stale = encode_command(&Command::Submit { request, proposal }).unwrap();
    fs::write(&document, &stale).unwrap();
    let submitted = actor(actor_profile.clone(), document.clone());
    serve_with_credibility(config, &actor_profile, &peer, None, true,
        Some(&capsule), || ElapsedTick(1001)).unwrap();
    assert!(!executed(&submitted.join().unwrap().1));
    assert_eq!(fs::read(&document).unwrap(), stale);
    let config = configured(&root, 0);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.control.sequence, qualification.expected_control_sequence + 1);
    assert_eq!(after.executions, 1);
    assert_eq!(after.control.ledger.charged, 4);
    assert_eq!(after.control.ledger.reserved, 0);
    assert!(!peer.socket(7).exists());
}

#[test]
fn live_option_is_operator_only_and_creation_refuses_before_side_effects() {
    let flag = "--credibility-activation";
    for mode in ["serve-open", "serve-open-checked"] {
        let mut args = vec![mode.into(), "config".into(), "actor".into(), "reviewer".into()];
        if mode.ends_with("checked") { args.push("witness".into()); }
        let positionals = args.clone();
        args.extend([flag.into(), "capsule".into()]);
        assert_eq!(crate::qualification::take_option(&mut args).unwrap(), Some(PathBuf::from("capsule")));
        assert_eq!(args, positionals);
    }
    for mode in ["serve-create", "serve-create-checked", "actor-submit", "review-peer", "stop-peer"] {
        let mut args = vec![mode.into(), "config".into(), "actor".into(), "reviewer".into(), flag.into(), "capsule".into()];
        let before = args.clone();
        assert!(crate::qualification::take_option(&mut args).is_err());
        assert_eq!(args, before);
    }
    let root = Directory::new(); let config = configured(&root, 0);
    let (actor_profile, peer) = profiles(&root, &config, 7);
    assert!(serve_with_credibility(config, &actor_profile, &peer, None, false,
        Some(&root.0.join("missing-capsule")), || panic!("creation must refuse before observing time")).is_err());
    assert!(!root.0.join("store").exists());
    assert!(!actor_profile.socket.exists());
}
