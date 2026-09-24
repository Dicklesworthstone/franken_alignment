//! Real files/listeners and original helper/human workflows; synthetic judgments.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::ReviewClientProgress;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::ReviewDecision;
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-multi-{}-{}-{}", std::process::id(),
            crate::workflow::clock().0, NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
        fs::DirBuilder::new().mode(0o750).create(path.join("peers")).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(e) = fs::remove_dir_all(&self.0) { eprintln!("multi-peer cleanup: {e}"); } }
}
fn credentials() -> PeerCredentials {
    let (socket, _other) = UnixStream::pair().unwrap(); PeerCredentials::observe(&socket).unwrap()
}
fn configured(root: &Directory) -> Config {
    let mut c = Config::decode(include_bytes!("../../../../../fixtures/supervised_publication.json")).unwrap();
    c.store = root.0.join("store");
    c.source = FileEvidenceSource::new(root.0.join("evidence.json"), 51, c.profile.delivery.scope, 1048576).unwrap();
    c.timing.poll_ms = 1; c.timing.runtime_ms = 15000; c.timing.cleanup_ms = 2000;
    c.programs = ["alpha", "beta"].into_iter().map(|name| {
        let program = HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), "tests::synthetic_helper_process".into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from("FA_EXAMPLE_HELPER_MEMBER"), OsString::from(name))])).unwrap();
        (name.to_owned(), program)
    }).collect();
    let observed = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope: c.profile.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
    fs::write(root.0.join("evidence.json"), observed.encode()).unwrap();
    c
}
fn actor(root: &Directory, config: &Config, request: u64) -> Profile {
    let id = credentials();
    let identity = PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap();
    Profile { request, scope: config.profile.delivery.scope, socket: root.0.join(format!("actor-{request}.sock")),
        supervisor: identity, actor: identity, candidates: 4, connections: 4, exchanges: 256,
        runtime_ms: 15000, poll_ms: 1, reply_ms: 100 }
}
fn reviewer(root: &Directory, config: &Config) -> PeerProfile {
    let id = credentials(); let s = config.profile.delivery.scope;
    let json = format!(r#"{{"version":1,"clock":"unix_milliseconds","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},"reviewer_id":{},"socket_directory":"{}/peers","supervisor":{{"uid":{},"gid":{},"pid":{}}},"reviewer":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":2,"runtime_ms":5000,"poll_ms":1}}"#,
        s.tenant, s.principal, s.run, s.branch, s.authority, config.profile.human.reviewer_id, root.0.display(),
        id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid());
    PeerProfile::decode(json.as_bytes()).unwrap()
}
fn document(config: &Config, request: u64, version: u64) -> Vec<u8> {
    let mut target = config.profile.delivery.target; target.expected_version = version;
    encode_command(&Command::Submit { request, proposal: ActorProposal { target,
        payload: format!("publication-{request}").into_bytes(), units: 32,
        deadline: ElapsedTick(100000), expected_policy_epoch: 0,
    }}).unwrap()
}
fn wait(path: &Path) {
    let start = Instant::now();
    while !path.exists() { assert!(start.elapsed() < Duration::from_secs(12), "missing {path:?}"); pause(1); }
}
fn approving(profile: PeerProfile, requests: Vec<u64>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for request in requests {
            wait(&profile.socket(request));
            let mut client = profile.connect_client(request).unwrap(); let start = Instant::now();
            loop {
                assert!(start.elapsed() < Duration::from_secs(12));
                match client.step().unwrap() {
                    ReviewClientProgress::NeedsDecision => client.respond(ReviewDecision::Approve).unwrap(),
                    ReviewClientProgress::Complete => break,
                    _ => pause(1),
                }
            }
        }
    })
}
fn submitting(profile: Profile, document: PathBuf, after: Option<PathBuf>, done: Option<PathBuf>)
    -> std::thread::JoinHandle<Vec<u8>>
{
    std::thread::spawn(move || {
        if let Some(path) = after { wait(&path); }
        wait(&profile.socket);
        let mut output = Vec::new();
        super::super::client::submit(&profile, &document, &mut output).unwrap();
        if let Some(path) = done { fs::write(path, b"response returned").unwrap(); }
        output
    })
}

#[test]
fn multi_peer_option_preserves_original_modes_and_rejects_ambiguous_schedules() {
    let args = |mode: &str| [mode, "config", "first", "reviewer", "--peers", "second"]
        .into_iter().map(str::to_owned).collect::<Vec<_>>();
    assert_eq!(arguments(&args("serve-create")).unwrap(), (4, false, false));
    assert_eq!(arguments(&args("serve-open")).unwrap(), (4, true, false));
    let mut checked = args("serve-open-checked"); checked.insert(4, "witness".into());
    assert_eq!(arguments(&checked).unwrap(), (5, true, true));
    for mode in ["actor-submit", "submit", "review-peer"] { assert!(arguments(&args(mode)).is_err()); }
    let mut maximum = args("serve-open"); maximum.extend((0..14).map(|i| format!("peer-{i}")));
    assert!(arguments(&maximum).is_ok()); maximum.push("one-over".into()); assert!(arguments(&maximum).is_err());
    for extra in [vec![], vec!["--peers"], vec!["--requests", "1,2"]] {
        let mut malformed = args("serve-open"); malformed.truncate(5);
        malformed.extend(extra.into_iter().map(str::to_owned)); assert!(arguments(&malformed).is_err());
    }
}

#[test]
fn multi_peer_preflight_refuses_cross_role_paths_before_any_store_or_socket_creation() {
    let root = Directory::new(); let c = configured(&root); let r = reviewer(&root, &c);
    let a = actor(&root, &c, 1); let b = actor(&root, &c, 2);
    validate(&c, &[a.clone(), b.clone()], &r).unwrap();
    for changed in [Profile { request: 1, ..b.clone() }, Profile { socket: a.socket.clone(), ..b.clone() },
        Profile { socket: r.socket(1), ..b.clone() },
        Profile { socket: super::super::super::control::socket_path(&r, 1), ..b.clone() }] {
        assert!(validate(&c, &[a.clone(), changed], &r).is_err());
        assert!(!c.store.exists()); assert!(!a.socket.exists()); assert!(!b.socket.exists());
    }
    let mut wrong = b.clone(); wrong.scope.run += 1;
    assert!(validate(&c, &[a, wrong], &r).is_err());
}

#[test]
fn multi_peer_partial_listener_setup_retains_foreign_paths_and_drops_only_owned_socket() {
    let root = Directory::new(); let c = configured(&root);
    let a = actor(&root, &c, 1); let b = actor(&root, &c, 2);
    fs::write(&b.socket, b"unrelated bytes").unwrap();
    assert!(Listeners::bind(&[a.clone(), b.clone()]).is_err());
    assert!(!a.socket.exists()); assert!(!c.store.exists());
    assert_eq!(fs::read(&b.socket).unwrap(), b"unrelated bytes");
}

#[test]
fn multi_peer_unterminated_first_frame_does_not_block_second_publication_admission() {
    let root = Directory::new(); let mut c = configured(&root); let r = reviewer(&root, &c);
    let actors = [actor(&root, &c, 1), actor(&root, &c, 2)]; validate(&c, &actors, &r).unwrap();
    let listeners = Listeners::bind(&actors).unwrap();
    let (host, _) = prepare_host(&c, None, false).unwrap(); let (port, mut driver) = host.into_supervised_driver();
    let mut intake = listeners.connect(port).unwrap();
    let mut first = UnixStream::connect(&actors[0].socket).unwrap(); first.write_all(b"{").unwrap();
    let mut second = UnixStream::connect(&actors[1].socket).unwrap();
    second.write_all(&document(&c, 2, 1)).unwrap(); second.write_all(b"\n").unwrap();
    let reads = c.source.status().read_attempts;
    intake.drive(&mut driver, &mut c.source, || ElapsedTick(1000)).unwrap();
    let ready = intake.pool.next_request(&driver).unwrap().unwrap();
    assert_eq!((ready.peer, ready.status.request), (2, 2));
    assert!(matches!(driver.supervisor().host().unwrap().request_status(1), Err(JournalError::Contract(Error::Missing))));
    assert_eq!(c.source.status().read_attempts, reads + 1);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    crate::workflow::stop(&mut driver, 1, &mut || ElapsedTick(1000)).unwrap();
    intake.pool.revoke_all(); assert_eq!(cleanup(driver, 2000, 1), 0);
}

#[test]
fn multi_peer_two_real_workflows_run_out_of_order_then_recover_without_source_or_helpers() {
    let root = Directory::new(); let c = configured(&root); let r = reviewer(&root, &c);
    let actors = [actor(&root, &c, 1), actor(&root, &c, 2)];
    let bootstrap = c.profile.clone(); let store = c.store.clone();
    // Peer 1 arrives only after peer 2 completes. No predetermined peer can
    // block the other, and each new proposal names its own exact target version.
    let first = root.0.join("first.json"); let second = root.0.join("second.json");
    fs::write(&first, document(&c, 1, 2)).unwrap(); fs::write(&second, document(&c, 2, 1)).unwrap();
    let marker = root.0.join("second.done");
    let human = approving(r.clone(), vec![2, 1]);
    let b = submitting(actors[1].clone(), second.clone(), None, Some(marker.clone()));
    let a = submitting(actors[0].clone(), first.clone(), Some(marker), None);
    let result = serve(c, &actors, &r, None, false, None, || ElapsedTick(1000));
    assert!(!a.join().unwrap().is_empty()); assert!(!b.join().unwrap().is_empty()); human.join().unwrap();
    result.unwrap();
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.executions, 2); assert_eq!(state.payload, b"publication-1");
    assert_eq!(state.control.ledger.charged, 64); assert_eq!(state.control.ledger.reserved, 0);
    assert!(!super::super::super::control::socket_path(&r, 1).exists());
    assert!(actors.iter().all(|p| !p.socket.exists()));
    // Occupying the stop path detects an accidental fresh control listener.
    let stop_path = super::super::super::control::socket_path(&r, 1);
    let _foreign = UnixListener::bind(&stop_path).unwrap(); let inode = fs::symlink_metadata(&stop_path).unwrap().ino();
    let mut c = configured(&root); c.programs.clear(); fs::remove_file(root.0.join("evidence.json")).unwrap();
    let a = submitting(actors[0].clone(), first, None, None);
    let b = submitting(actors[1].clone(), second, None, None);
    let result = serve(c, &actors, &r, None, true, None, || ElapsedTick(2000));
    assert!(!a.join().unwrap().is_empty()); assert!(!b.join().unwrap().is_empty()); result.unwrap();
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert_eq!(state.executions, 2); assert_eq!(state.control.ledger.charged, 64);
    assert!(!root.0.join("evidence.json").exists());
    assert_eq!(fs::symlink_metadata(stop_path).unwrap().ino(), inode);
}

#[test]
fn multi_peer_shared_stop_transport_keeps_socket_identity_and_lifetime() {
    let root = Directory::new(); let c = configured(&root); let r = reviewer(&root, &c);
    // Private wrapper sharing never binds another listener. Holding one alias
    // across an original review must not unlink the service's live stop path.
    let (host, reviewer) = prepare_host(&c, None, false).unwrap();
    let (_port, mut driver) = host.into_supervised_driver();
    let original = Control::new(&c, 1, Some(&r)).unwrap(); let mut shared = original.share();
    let path = super::super::super::control::socket_path(&r, 1);
    let inode = fs::symlink_metadata(&path).unwrap().ino();
    let deadline = Deadline { logical: ElapsedTick(100000), started: Instant::now(), wall: Duration::from_secs(10) };
    assert!(!shared.checkpoint(&mut driver, &reviewer, &deadline, &mut || panic!("idle time read")).unwrap());
    drop(shared); assert_eq!(fs::symlink_metadata(&path).unwrap().ino(), inode);
    shared = original.share(); drop(original);
    assert!(!shared.checkpoint(&mut driver, &reviewer, &deadline, &mut || panic!("idle time read")).unwrap());
    assert_eq!(fs::symlink_metadata(&path).unwrap().ino(), inode);
    drop(shared); assert!(!path.exists());
    // A foreign existing path is never removed to make sharing/setup succeed.
    fs::write(&path, b"foreign").unwrap();
    assert!(Control::new(&c, 1, Some(&r)).is_err());
    assert_eq!(fs::read(path).unwrap(), b"foreign");
    assert_eq!(cleanup(driver, 2000, 1), 0);
}

#[test]
fn multi_peer_independent_stop_works_while_all_actors_and_source_are_absent() {
    let root = Directory::new(); let mut c = configured(&root); let r = reviewer(&root, &c);
    let actors = [actor(&root, &c, 1), actor(&root, &c, 2)];
    let store = c.store.clone(); let bootstrap = c.profile.clone();
    fs::remove_file(root.0.join("evidence.json")).unwrap(); c.programs.clear();
    let remote = r.clone();
    let controller = std::thread::spawn(move || {
        wait(&super::super::super::control::socket_path(&remote, 1));
        let mut output = Vec::new();
        crate::peers::stop::request(&remote, 1, &mut output).unwrap();
        output
    });
    let result = serve(c, &actors, &r, None, false, None, || ElapsedTick(1000));
    assert!(!controller.join().unwrap().is_empty()); result.unwrap();
    let state = FileOversight::read_publication(&store, &bootstrap).unwrap();
    assert!(state.stop.is_some()); assert_eq!(state.executions, 0);
    assert_eq!(state.payload, b"initial"); assert_eq!(state.control.ledger.charged, 0);
    assert_eq!(state.control.ledger.reserved, 0);
    assert!(actors.iter().all(|p| !p.socket.exists()));
    assert!(!root.0.join("evidence.json").exists());
}
