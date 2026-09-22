//! Actual actor/helper/reviewer sockets and original publication journals.
//! The helper fixture is synthetic; this is not model-accuracy evidence.
use super::*;
use crate::config::Config;
use crate::peers::PeerProfile;
use crate::workflow::actor_service::series::{self, Options};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewClientProgress;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{PeerPolicy as ReviewerPolicy, VerifiedReviewerSocket};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::control::StopClientProgress;
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::{WireResponse, decode_response, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("fa-actor-batch-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&root).unwrap();
        for name in ["actors", "reviewers"] { fs::DirBuilder::new().mode(0o750).create(root.join(name)).unwrap(); }
        Self(root)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("actor batch cleanup: {error}"); } }
}
fn config(root: &Directory) -> Config {
    let mut c = Config::decode(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"))).unwrap();
    c.store = root.0.join("store");
    c.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, c.profile.delivery.scope, 1_048_576).unwrap();
    c.timing.poll_ms = 1; c.timing.runtime_ms = 15_000; c.timing.cleanup_ms = 2_000;
    c.programs = ["alpha", "beta"].into_iter().map(|name| (name.into(), HelperProgram::new(
        std::env::current_exe().unwrap(), root.0.clone(),
        vec!["--exact".into(), "tests::synthetic_helper_process".into(), "--nocapture".into()],
        BTreeMap::from([(OsString::from("FA_EXAMPLE_HELPER_MEMBER"), OsString::from(name))])).unwrap())).collect();
    c
}
fn credentials() -> PeerCredentials {
    let (socket, _) = UnixStream::pair().unwrap(); PeerCredentials::observe(&socket).unwrap()
}
fn profiles(root: &Directory, c: &Config) -> (Profile, PeerProfile) {
    let id = credentials(); let s = c.profile.delivery.scope;
    let scope = format!(r#"{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority);
    let identity = format!(r#"{{"uid":{},"gid":{},"pid":{}}}"#, id.uid(), id.gid(), id.pid());
    let actor = format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":7,"scope":{scope},"socket":"{}/actors/actor.sock","supervisor":{identity},"actor":{identity},"candidate_limit":1,"connection_limit":1,"exchange_limit":1024,"runtime_ms":15000,"poll_ms":1,"reply_ms":200}}"#, root.0.display());
    let reviewer = format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{scope},"reviewer_id":77,"socket_directory":"{}/reviewers","supervisor":{identity},"reviewer":{identity},"candidate_limit":8,"runtime_ms":15000,"poll_ms":1}}"#, root.0.display());
    (Profile::decode(actor.as_bytes()).unwrap(), PeerProfile::decode(reviewer.as_bytes()).unwrap())
}
fn evidence(root: &Directory, c: &Config) {
    let value = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: c.profile.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|m| (m.into(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    fs::write(root.0.join("evidence.json"), value.encode()).unwrap();
}
fn document(c: &Config, request: u64, version: u64) -> Vec<u8> {
    let mut target = c.profile.delivery.target; target.expected_version = version;
    encode_command(&Command::Submit { request, proposal: ActorProposal {
        target, payload: b"network".to_vec(), units: 7, deadline: ElapsedTick(100_000), expected_policy_epoch: 0,
    } }).unwrap()
}
fn documents(root: &Directory, c: &Config, second_version: u64) -> Vec<PathBuf> {
    [(7, 1), (8, second_version)].into_iter().map(|(key, version)| {
        let path = root.0.join(format!("{key}.json")); fs::write(&path, document(c, key, version)).unwrap(); path
    }).collect()
}
fn wait(path: &Path) {
    let started = Instant::now();
    while !path.exists() { assert!(started.elapsed() < Duration::from_secs(10), "missing {path:?}"); super::super::pause(1); }
}
fn human(p: PeerProfile, request: u64) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        wait(&p.socket(request)); let mut client = p.connect_client(request).unwrap(); let start = Instant::now();
        loop {
            assert!(start.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => client.respond(ReviewDecision::Approve).unwrap(),
                ReviewClientProgress::Complete => return,
                _ => super::super::pause(1),
            }
        }
    })
}
fn actor(p: Profile, paths: Vec<PathBuf>) -> std::thread::JoinHandle<(Result<(), String>, Vec<u8>)> {
    std::thread::spawn(move || {
        wait(&p.socket); let mut output = Vec::new();
        let result = submit(&p, &paths.iter().map(PathBuf::as_path).collect::<Vec<_>>(), &mut output);
        (result, output)
    })
}
fn responses(bytes: &[u8]) -> Vec<WireResponse> {
    bytes.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()).map(|line| decode_response(line).unwrap()).collect()
}
fn is_executed(response: &WireResponse) -> bool {
    matches!(&response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn run_pair(root: &Directory) -> Vec<PathBuf> {
    let c = config(root); evidence(root, &c); let (p, reviewer) = profiles(root, &c);
    let docs = documents(root, &c, 2);
    let child = actor(p.clone(), docs.clone());
    let humans = [human(reviewer.clone(), 7), human(reviewer.clone(), 8)];
    series::serve(c, &p, &reviewer, None, Options { requests: &[7, 8], open: false, credibility: None }, || ElapsedTick(1_000)).unwrap();
    for human in humans { human.join().unwrap(); }
    let (result, out) = child.join().unwrap(); assert!(result.is_ok(), "{result:?}");
    let out = responses(&out); assert_eq!(out.len(), 2);
    assert_eq!(out.iter().map(|r| r.request).collect::<Vec<_>>(), vec![Some(7), Some(8)]);
    assert!(out.iter().all(is_executed));
    let c = config(root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 2); assert_eq!(disk.control.ledger.charged, 14);
    assert_eq!(disk.control.ledger.reserved, 0); assert_eq!(disk.control.ledger.epoch, 0);
    docs
}

#[test]
fn sequential_client_executes_on_one_connection_and_retrieves_history_without_sources() {
    let root = Directory::new(); let docs = run_pair(&root);
    let mut c = config(&root); let (p, reviewer) = profiles(&root, &c);
    fs::remove_file(root.0.join("evidence.json")).unwrap(); c.programs.clear();
    let missing = root.0.join("no-capsule.bin");
    let child = actor(p.clone(), docs);
    series::serve(c, &p, &reviewer, None, Options { requests: &[7, 8], open: true, credibility: Some(&missing) }, || ElapsedTick(200_000)).unwrap();
    let (result, out) = child.join().unwrap(); assert!(result.is_ok(), "{result:?}");
    let out = responses(&out); assert_eq!(out.len(), 2); assert!(out.iter().all(is_executed));
    let c = config(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 2); assert_eq!(disk.control.ledger.charged, 14);
    assert!(!reviewer.socket(7).exists()); assert!(!reviewer.socket(8).exists());
}

#[test]
fn lifetime_exchange_budget_cannot_reset_for_a_second_historical_document() {
    let root = Directory::new(); let docs = run_pair(&root);
    let mut c = config(&root); c.programs.clear(); let (p, reviewer) = profiles(&root, &c);
    let mut limited = p.clone(); limited.exchanges = 1;
    let child = actor(limited, docs);
    series::serve(c, &p, &reviewer, None, Options { requests: &[7, 8], open: true, credibility: None }, || ElapsedTick(200_000)).unwrap();
    let (result, out) = child.join().unwrap(); assert!(result.unwrap_err().contains("Limit"));
    let out = responses(&out); assert_eq!(out.len(), 1); assert_eq!(out[0].request, Some(7)); assert!(is_executed(&out[0]));
    let c = config(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 2); assert_eq!(disk.control.ledger.charged, 14);
}

#[test]
fn stale_second_target_is_not_repaired_after_first_execution() {
    let root = Directory::new(); let c = config(&root); evidence(&root, &c); let (p, reviewer) = profiles(&root, &c);
    let docs = documents(&root, &c, 1); let original = fs::read(&docs[1]).unwrap();
    let child = actor(p.clone(), docs.clone()); let human = human(reviewer.clone(), 7);
    series::serve(c, &p, &reviewer, None, Options { requests: &[7, 8], open: false, credibility: None }, || ElapsedTick(1_000)).unwrap();
    human.join().unwrap(); let (result, out) = child.join().unwrap(); assert!(result.is_err());
    let out = responses(&out); assert_eq!(out.len(), 2); assert!(is_executed(&out[0])); assert!(!is_executed(&out[1]));
    assert_eq!(fs::read(&docs[1]).unwrap(), original);
    let c = config(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.control.ledger.charged, 7); assert!(!reviewer.socket(8).exists());
}

fn stop(p: &PeerProfile, operation: u64) {
    let path = crate::workflow::control::socket_path(p, operation); wait(&path); let id = credentials();
    let socket = VerifiedReviewerSocket::verify(UnixStream::connect(path).unwrap(),
        ReviewerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap()).unwrap();
    let mut client = socket.into_stop_client(p.expected, operation).unwrap(); let start = Instant::now();
    loop {
        assert!(start.elapsed() < Duration::from_secs(10));
        match client.step().unwrap() {
            StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
            StopClientProgress::Complete => { assert!(client.receipt().unwrap().acknowledged()); return; }
            _ => super::super::pause(1),
        }
    }
}
#[test]
fn lost_output_does_not_send_the_next_document_or_refund_execution() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let root = Directory::new(); let c = config(&root); evidence(&root, &c); let (p, reviewer) = profiles(&root, &c);
    let docs = documents(&root, &c, 2); let peer = p.clone(); let stop_profile = reviewer.clone();
    let child = std::thread::spawn(move || {
        wait(&peer.socket);
        let error = submit(&peer, &docs.iter().map(PathBuf::as_path).collect::<Vec<_>>(), &mut Broken).unwrap_err();
        assert!(error.contains("output failed")); stop(&stop_profile, 8);
    });
    let human = human(reviewer.clone(), 7);
    series::serve(c, &p, &reviewer, None, Options { requests: &[7, 8], open: false, credibility: None }, || ElapsedTick(1_000)).unwrap();
    human.join().unwrap(); child.join().unwrap();
    let c = config(&root); let disk = FileOversight::read_publication(&c.store, &c.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.control.ledger.charged, 7);
    assert_eq!(disk.control.ledger.stages.len(), 1); assert_eq!(disk.control.ledger.reserved, 0);
    assert_eq!(disk.stop.unwrap().request().operation, 8); assert!(!reviewer.socket(8).exists());
}

#[test]
fn complete_batch_preflight_refuses_bad_later_inputs_before_connecting() {
    let root = Directory::new(); let c = config(&root); let (p, _) = profiles(&root, &c);
    let docs = documents(&root, &c, 2); let listener = UnixListener::bind(&p.socket).unwrap(); listener.set_nonblocking(true).unwrap();
    let valid = fs::read(&docs[1]).unwrap();
    for invalid in [b"{}".to_vec(), document(&c, 7, 2), encode_command(&Command::Poll { request: 8 }).unwrap(),
        vec![b' '; MAX_FRAME_BYTES + 1]] {
        fs::write(&docs[1], invalid).unwrap(); let mut out = Vec::new();
        assert!(submit(&p, &[&docs[0], &docs[1]], &mut out).is_err()); assert!(out.is_empty());
        assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
    }
    fs::write(&docs[1], valid).unwrap();
    let prepared = read_documents(&p, &[&docs[0], &docs[1]]).unwrap();
    assert_eq!(prepared.iter().map(Command::request).collect::<Vec<_>>(), vec![7, 8]);
    assert!(submit(&p, &vec![docs[0].as_path(); MAX_SEQUENCE_DOCUMENTS + 1], &mut Vec::new()).is_err());
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock); assert!(!c.store.exists());
}

#[test]
fn actor_batch_authenticates_connected_server_before_sending_any_document() {
    let root = Directory::new(); let c = config(&root); let (mut p, _) = profiles(&root, &c);
    let docs = documents(&root, &c, 2); let id = credentials();
    p.supervisor = PeerPolicy::new(id.uid(), id.gid(), Some(id.pid() + 1)).unwrap();
    let listener = UnixListener::bind(&p.socket).unwrap();
    let child = actor(p, docs); let (mut socket, _) = listener.accept().unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    assert_eq!(socket.read(&mut [0; 1]).unwrap(), 0);
    let (result, out) = child.join().unwrap(); assert!(result.is_err()); assert!(out.is_empty()); assert!(!c.store.exists());
}

#[test]
fn aggregate_original_input_bytes_have_an_exact_bound_before_any_submission() {
    let root = Directory::new(); let c = config(&root); let (p, _) = profiles(&root, &c);
    let listener = UnixListener::bind(&p.socket).unwrap(); listener.set_nonblocking(true).unwrap();
    let per_file = MAX_SEQUENCE_INPUT_BYTES / MAX_SEQUENCE_DOCUMENTS;
    assert!(per_file < MAX_FRAME_BYTES);
    let paths: Vec<_> = (0..MAX_SEQUENCE_DOCUMENTS).map(|i| {
        let path = root.0.join(format!("batch-{i}.json"));
        let mut bytes = document(&c, 7 + i as u64, 1 + i as u64);
        assert!(bytes.len() < per_file); bytes.resize(per_file, b' ');
        fs::write(&path, bytes).unwrap(); path
    }).collect();
    let references: Vec<_> = paths.iter().map(PathBuf::as_path).collect();
    assert_eq!(read_documents(&p, &references).unwrap().len(), MAX_SEQUENCE_DOCUMENTS);
    fs::OpenOptions::new().append(true).open(paths.last().unwrap()).unwrap().write_all(b" ").unwrap();
    let mut out = Vec::new(); assert!(submit(&p, &references, &mut out).is_err());
    assert!(out.is_empty()); assert!(!c.store.exists());
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}
