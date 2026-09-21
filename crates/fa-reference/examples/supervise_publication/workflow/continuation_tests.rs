//! Synthetic independent helpers exercise actual process and socket plumbing.
//! They are not trained-model evaluations or reviewer authentication evidence.
use super::submit_existing;
use crate::config::{CLOCK_DOMAIN, Config};
use crate::workflow;
use fa_reference::Snapshot;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{
    ReviewClientProgress, ReviewerClient, ReviewerExpectation,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{
    ReviewDecision, ReviewPacket,
};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{Command, WireError, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource,
};
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const TEMPLATE: &[u8] = include_bytes!("../../../fixtures/supervised_publication.json");
const CHILD: &str = "workflow::continuation::tests::synthetic_helper_process";
const MEMBER: &str = "FA_CONTINUATION_HELPER_MEMBER";
const EPOCH: &str = "FA_CONTINUATION_HELPER_EPOCH";
static NEXT: AtomicU64 = AtomicU64::new(0);

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fa-continuation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.0) {
            eprintln!("continuation fixture cleanup: {error:?}");
        }
    }
}

fn configured(root: &Directory, epoch: u64) -> Config {
    let mut config = Config::decode(TEMPLATE).unwrap();
    config.store = root.0.join("store");
    config.source = FileEvidenceSource::new(
        root.0.join("evidence.json"), 51, config.profile.delivery.scope, 1_048_576,
    ).unwrap();
    config.timing.poll_ms = 1;
    config.timing.runtime_ms = 15_000;
    config.timing.cleanup_ms = 2_000;
    config.programs = ["alpha", "beta"].into_iter().map(|member| {
        let program = HelperProgram::new(
            std::env::current_exe().unwrap(),
            root.0.clone(),
            vec!["--exact".into(), CHILD.into(), "--nocapture".into()],
            BTreeMap::from([
                (OsString::from(MEMBER), OsString::from(member)),
                (OsString::from(EPOCH), OsString::from(epoch.to_string())),
            ]),
        ).unwrap();
        (member.to_owned(), program)
    }).collect();
    config
}

fn evidence(root: &Directory, config: &Config) {
    let snapshot = EvidenceSnapshot::new(
        EvidenceIdentity { source: 51, generation: 1, scope: config.profile.delivery.scope },
        Snapshot {
            semantic_epoch: 1,
            complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        },
        ["alpha", "beta"].into_iter().map(|member| (
            member.to_owned(), format!("continuation evidence: {member}").into_bytes(),
        )).collect(),
    ).unwrap();
    std::fs::write(root.0.join("evidence.json"), snapshot.encode()).unwrap();
}

fn document(request: u64, payload: &[u8], target: fa_reference::action::ResolvedTarget, epoch: u64) -> Vec<u8> {
    encode_command(&Command::Submit {
        request,
        proposal: ActorProposal {
            target,
            payload: payload.to_vec(),
            units: payload.len() as u64,
            deadline: ElapsedTick(100_000),
            expected_policy_epoch: epoch,
        },
    }).unwrap()
}

#[test]
fn synthetic_helper_process() {
    let Ok(member) = std::env::var(MEMBER) else { return; };
    let epoch = std::env::var(EPOCH).unwrap().parse::<u64>().unwrap();
    let profile = Config::decode(TEMPLATE).unwrap().profile;
    let mut client = HelperClient::from_process_stdin(
        profile.committee.members()[&member].profile_at(epoch),
    ).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let expected = format!("continuation evidence: {member}");
            assert!(input.actual_input().submitted_bytes().windows(expected.len())
                .any(|bytes| bytes == expected.as_bytes()));
            client.respond(Verdict::Allow, member.as_bytes()).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn reviewer(config: &Config, request: u64) -> std::thread::JoinHandle<ReviewPacket> {
    let path = config.socket(request);
    let expected = ReviewerExpectation {
        reviewer: config.profile.human.reviewer_id,
        scope: config.profile.delivery.scope,
        clock_domain: CLOCK_DOMAIN,
    };
    std::thread::spawn(move || {
        let started = Instant::now();
        let stream = loop {
            match UnixStream::connect(&path) {
                Ok(stream) => break stream,
                Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {
                    assert!(started.elapsed() < Duration::from_secs(10), "missing review offer: {error:?}");
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("review connection: {error:?}"),
            }
        };
        let mut client = ReviewerClient::from_unix(stream, expected).unwrap();
        let mut packet = None;
        loop {
            assert!(started.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    packet = Some(client.packet().unwrap().clone());
                    client.respond(ReviewDecision::Approve).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}

fn assert_executed(result: &workflow::RunResult) {
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    assert!(matches!(&result.response.result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
}

fn first_publication(root: &Directory) -> Vec<u8> {
    let config = configured(root, 0);
    evidence(root, &config);
    let original = document(1, b"first", config.profile.delivery.target, 0);
    let human = reviewer(&config, 1);
    let result = workflow::run(config, &original, false, || ElapsedTick(1_000)).unwrap();
    human.join().unwrap();
    assert_executed(&result);
    original
}

#[test]
fn successive_publications_keep_the_original_budget_history_and_require_fresh_keys() {
    let root = Directory::new();
    first_publication(&root);
    let mut charged = 5;
    for (request, payload) in [(2, b"second".as_slice()), (3, b"third".as_slice())] {
        let config = configured(&root, request - 1);
        let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        let epoch = before.control.ledger.epoch.checked_add(1).unwrap();
        let next = document(request, payload, before.target, epoch);
        let human = reviewer(&config, request);
        let result = submit_existing(config, &next, None, None, || ElapsedTick(1_000 + request)).unwrap();
        let packet = human.join().unwrap();
        assert_executed(&result);
        assert_eq!(packet.action().spec().payload, payload);
        assert_eq!(packet.action().spec().policy_epoch, epoch);
        let config = configured(&root, epoch);
        let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        charged += payload.len() as u64;
        assert_eq!(after.executions, request);
        assert_eq!(after.payload, payload);
        assert_eq!(after.target.expected_version, before.target.expected_version + 1);
        assert_eq!(after.control.ledger.epoch, epoch);
        assert_eq!(after.control.ledger.charged, charged);
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.control.ledger.available, config.profile.delivery.total - charged);
        assert_eq!(after.control.ledger.stages.len(), request as usize);
    }
}

#[test]
fn expired_exact_retry_is_query_only_without_source_helpers_or_reviewer() {
    let root = Directory::new();
    let original = first_publication(&root);
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut config = configured(&root, 1);
    config.programs.clear();
    let result = submit_existing(config, &original, None, None, || ElapsedTick(200_000)).unwrap();
    assert_executed(&result);
    let config = configured(&root, 2);
    let changed = document(1, b"changed", config.profile.delivery.target, 0);
    let conflict = submit_existing(config, &changed, None, None, || ElapsedTick(200_001)).unwrap();
    assert_eq!(conflict.response.result, Err(WireError::IdempotencyConflict));
    let config = configured(&root, 2);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, 1);
    assert_eq!(after.payload, b"first");
    assert_eq!(after.control.ledger.charged, 5);
    assert_eq!(after.control.ledger.reserved, 0);
}

#[test]
fn new_stale_epoch_is_not_rewritten_to_pass_recovery() {
    let root = Directory::new();
    first_publication(&root);
    let mut config = configured(&root, 0);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let stale = document(2, b"not authorized", before.target, before.control.ledger.epoch);
    config.programs.clear();
    let result = submit_existing(config, &stale, None, None, || ElapsedTick(1_001)).unwrap();
    assert!(!matches!(result.response.result,
        Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(result.cleanup_pending, 0);
    let config = configured(&root, 1);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after.executions, before.executions);
    assert_eq!(after.payload, before.payload);
    assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
    assert_eq!(after.control.ledger.reserved, 0);
    assert!(after.stop.is_none());
    assert!(!config.socket(2).exists());
}

#[test]
fn missing_store_and_malformed_documents_never_create_a_replacement() {
    let root = Directory::new();
    let config = configured(&root, 1);
    let store = config.store.clone();
    let next = document(1, b"new", config.profile.delivery.target, 1);
    assert!(submit_existing(config, &next, None, None, || ElapsedTick(1_001)).is_err());
    assert!(!store.exists());
    assert!(submit_existing(configured(&root, 1), b"{}", None, None, || ElapsedTick(1_001)).is_err());
    assert!(!store.exists());
}

#[test]
fn changed_bootstrap_cannot_refill_the_retained_budget() {
    let root = Directory::new();
    first_publication(&root);
    let mut config = configured(&root, 1);
    let before = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    let next = document(2, b"second", before.target, 1);
    config.profile.delivery.total += 1;
    assert!(submit_existing(config, &next, None, None, || ElapsedTick(1_001)).is_err());
    let config = configured(&root, 1);
    let after = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(after, before);
}

#[path = "qualified_tests.rs"]
mod qualified;
