//! Actual checked workflow over the existing executable-helper fixture. These
//! synthetic choices exercise credential plumbing, not human judgment quality.
use super::*;
use super::super::peers::PeerProfile;
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::PeerCredentials;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::process::{Child, Command as ProcessCommand};

fn credentials() -> PeerCredentials {
    let (stream, _peer) = UnixStream::pair().unwrap();
    PeerCredentials::observe(&stream).unwrap()
}
fn profile_json(c: &Config, directory: &std::path::Path, candidates: u32) -> String {
    let id = credentials(); let s = c.profile.delivery.scope;
    let directory = directory.to_str().unwrap().replace('\\', "\\\\").replace('"', "\\\"");
    format!(concat!("{{\"version\":1,\"clock\":\"unix_milliseconds\",",
        "\"scope\":{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{}}},",
        "\"reviewer_id\":{},\"socket_directory\":\"{}\",",
        "\"supervisor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
        "\"reviewer\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
        "\"candidate_limit\":{},\"runtime_ms\":15000,\"poll_ms\":1}}"),
        s.tenant, s.principal, s.run, s.branch, s.authority, c.profile.human.reviewer_id, directory,
        id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid(), candidates)
}
fn peer_profile(root: &Directory, c: &Config, candidates: u32) -> PeerProfile {
    let directory = root.0.join("reviewers"); fs::DirBuilder::new().mode(0o750).create(&directory).unwrap();
    let json = profile_json(c, &directory, candidates);
    assert!(!json.contains("environment")); assert!(!json.contains("executable")); assert!(!json.contains("evidence_path"));
    PeerProfile::decode(json.as_bytes()).unwrap()
}
const WRONG_CHILD: &str = "tests::peer_tests::wrong_reviewer_process";
const WRONG_SOCKET: &str = "FA_WRONG_REVIEWER_SOCKET";
#[test]
fn wrong_reviewer_process() {
    let Some(path) = std::env::var_os(WRONG_SOCKET) else { return; };
    let start = Instant::now();
    let mut stream = loop {
        match UnixStream::connect(&path) {
            Ok(stream) => break stream,
            Err(error) if matches!(error.kind(), io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused) => {}
            Err(error) => panic!("unexpected wrong-peer connect failure: {error}"),
        }
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    };
    // Attempt input before receiving an offer. It must not reach a decoder and
    // the supervisor must close this process without disclosing a packet byte.
    let _ = stream.write_all(&[1; 64]);
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    match stream.read(&mut [0; 1]) {
        Ok(0) => {}
        Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {}
        other => panic!("wrong reviewer received data or was not rejected: {other:?}"),
    }
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            if let Err(error) = self.0.kill() { eprintln!("peer fixture kill: {error}"); }
            if let Err(error) = self.0.wait() { eprintln!("peer fixture reap: {error}"); }
        }
    }
}
fn intrude(path: PathBuf) {
    let mut child = OwnedChild(ProcessCommand::new(std::env::current_exe().unwrap())
        .arg("--exact").arg(WRONG_CHILD).env(WRONG_SOCKET, path).spawn().unwrap());
    let start = Instant::now();
    loop {
        if let Some(status) = child.0.try_wait().unwrap() { assert!(status.success()); return; }
        assert!(start.elapsed() < Duration::from_secs(12));
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn checked_reviewer(profile: PeerProfile, decision: ReviewDecision, wrong_first: bool)
    -> std::thread::JoinHandle<ReviewPacket>
{
    std::thread::spawn(move || {
        if wrong_first { intrude(profile.socket(1)); }
        let start = Instant::now();
        while !profile.socket(1).exists() {
            assert!(start.elapsed() < Duration::from_secs(10)); std::thread::sleep(Duration::from_millis(1));
        }
        let mut client = profile.connect_client(1).unwrap();
        let mut seen = None;
        loop {
            assert!(start.elapsed() < Duration::from_secs(12));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    let packet = client.packet().unwrap().clone();
                    assert_eq!(packet.binding().request, 1);
                    let verb = if decision == ReviewDecision::Approve { "APPROVE" } else { "REJECT" };
                    let nonce: String = packet.binding().session.iter().map(|byte| format!("{byte:02x}")).collect();
                    let choice = format!("{verb} 1 {nonce}\n");
                    let selected = console::decide(&packet, &mut Cursor::new(choice), &mut Vec::new()).unwrap();
                    assert_eq!(selected, decision);
                    assert_eq!(fs::metadata(profile.socket(1)).unwrap().permissions().mode() & 0o777, 0o660);
                    client.respond(selected).unwrap(); seen = Some(packet);
                }
                ReviewClientProgress::Complete => return seen.unwrap(),
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}

#[test]
fn wrong_same_account_process_cannot_take_the_checked_offer_from_the_allowed_reviewer() {
    let root = Directory::new(); let config = configured(&root); evidence(&root, config.profile.delivery.scope);
    let profile = peer_profile(&root, &config, 3); let socket = profile.socket(1);
    let document = proposal(&config, 1, b"checked publication");
    let human = checked_reviewer(profile.clone(), ReviewDecision::Approve, true);
    let result = workflow::run_with_peers(config, &document, false, Some(&profile), || ElapsedTick(1000)).unwrap();
    let packet = human.join().unwrap();
    assert!(result.failure.is_none(), "{:?}", result.failure); assert_eq!(result.cleanup_pending, 0);
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    assert_eq!(packet.action().spec().payload, b"checked publication");
    assert!(!socket.exists());
    let config = configured(&root);
    let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
    assert_eq!(disk.executions, 1); assert_eq!(disk.control.ledger.reserved, 0);
    assert_eq!(fs::metadata(&config.store).unwrap().permissions().mode() & 0o077, 0);
    assert_eq!(fs::metadata(config.store.join("delivery.bin")).unwrap().permissions().mode() & 0o077, 0);
}

#[test]
fn checked_rejection_and_candidate_exhaustion_never_fall_back_to_unchecked_approval() {
    for exhaustion in [false, true] {
        let root = Directory::new(); let config = configured(&root); evidence(&root, config.profile.delivery.scope);
        let profile = peer_profile(&root, &config, 1); let document = proposal(&config, 1, b"declined");
        let wrong = if exhaustion { let path = profile.socket(1); Some(std::thread::spawn(move || intrude(path))) } else { None };
        let human = if !exhaustion { Some(checked_reviewer(profile.clone(), ReviewDecision::Reject, false)) } else { None };
        let result = workflow::run_with_peers(config, &document, false, Some(&profile), || ElapsedTick(1000)).unwrap();
        if let Some(thread) = wrong { thread.join().unwrap(); }
        if let Some(thread) = human { thread.join().unwrap(); }
        assert_eq!(result.cleanup_pending, 0);
        if exhaustion { assert!(result.failure.as_deref().unwrap().contains("candidate quota exhausted")); }
        else { assert!(result.failure.is_none()); }
        assert!(!matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let config = configured(&root);
        let disk = FileOversight::read_publication(&config.store, &config.profile).unwrap();
        assert_eq!(disk.executions, 0); assert_eq!(disk.control.ledger.charged, 0);
        assert_eq!(disk.control.ledger.reserved, 0); assert_eq!(disk.payload, b"initial");
        if exhaustion {
            let (host, _) = FileOversight::open(&config.store, config.profile).unwrap();
            assert!(host.human_status(1).is_err(), "a refused peer must not create a human request");
        }
    }
}

#[test]
fn audience_or_writable_socket_directory_refuses_before_store_helpers_and_clock() {
    for changed_audience in [false, true] {
        let root = Directory::new(); let config = configured(&root); let store = config.store.clone();
        let mut profile = peer_profile(&root, &config, 2);
        if changed_audience { profile.expected.reviewer += 1; }
        else { fs::set_permissions(root.0.join("reviewers"), fs::Permissions::from_mode(0o770)).unwrap(); }
        let document = proposal(&config, 1, b"never starts");
        assert!(workflow::run_with_peers(config, &document, false, Some(&profile), || panic!("preflight must precede clock")).is_err());
        assert!(!store.exists()); assert!(!profile.socket(1).exists());
    }
}

#[test]
fn malformed_peer_rules_and_unsupported_options_are_never_silently_ignored() {
    let root = Directory::new(); let config = configured(&root);
    let directory = root.0.join("unused"); let original = profile_json(&config, &directory, 2);
    assert!(PeerProfile::decode(original.as_bytes()).is_ok());
    let pid = credentials().pid();
    for invalid in [original.replacen("\"version\":1", "\"version\":2", 1),
        original.replacen("\"version\":1", "\"version\":1,\"extra\":0", 1),
        original.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
        original.replacen(&format!("\"pid\":{pid}"), "\"pid\":0", 1),
        original.replacen(&format!(",\"pid\":{pid}"), "", 1),
        original.replacen("\"candidate_limit\":2", "\"candidate_limit\":0", 1),
        original.replacen("\"candidate_limit\":2", "\"candidate_limit\":1025", 1),
        original.replacen("\"poll_ms\":1", "\"poll_ms\":0", 1),
        original.replacen("unix_milliseconds", "process_instant", 1)] {
        assert!(PeerProfile::decode(invalid.as_bytes()).is_err());
    }
    let nullable = original.replace(&format!("\"pid\":{pid}"), "\"pid\":null");
    assert!(PeerProfile::decode(nullable.as_bytes()).is_ok());
    assert!(super::super::command(vec!["resume".into(), "missing".into(), "missing".into(),
        "--reviewer-profile".into(), "missing".into()]).unwrap_err().contains("usage:"));
    assert!(!config.store.exists());
}

#[test]
fn reviewer_only_profile_rejects_a_fake_supervisor_without_access_to_private_configuration() {
    let root = Directory::new(); let config = configured(&root); let profile = peer_profile(&root, &config, 2);
    let fake_pid = if credentials().pid() == 1 { 2 } else { 1 };
    let json = profile_json(&config, &root.0.join("reviewers"), 2)
        .replacen(&format!("\"pid\":{}", credentials().pid()), &format!("\"pid\":{fake_pid}"), 1);
    let wrong_server = PeerProfile::decode(json.as_bytes()).unwrap();
    let listener = UnixListener::bind(profile.socket(1)).unwrap();
    // connect succeeds to the right path, but the kernel identity is wrong.
    // The original reviewer client is not constructed and no decision is sent.
    assert!(wrong_server.connect_client(1).unwrap_err().contains("Credentials"));
    let (mut stream, _) = listener.accept().unwrap();
    assert_eq!(stream.read(&mut [0; 1]).unwrap(), 0);
    assert!(!config.store.exists());
}
