//! Scripted authenticated sockets exercise real CLI/transport failure boundaries.
//! Replies are fixtures, not evidence that a policy or source is true.
use super::*;
use fa_reference::action::{Purpose, Scope};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    PrecheckWireError, encode_precheck_request,
};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use std::fs;
use std::io::{self, Read};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    actor: Profile,
    settings: Settings,
    actor_file: PathBuf,
    settings_file: PathBuf,
    documents: Vec<PathBuf>,
}
impl Fixture {
    fn new(count: usize) -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let root = crate::test_directory::create(format!("fa-pb-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let (socket, _) = UnixStream::pair().unwrap();
        let id = PeerCredentials::observe(&socket).unwrap();
        let policy = PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap();
        let actor = Profile {
            request: 7,
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4,
                authority: 5, purpose: Purpose::Effect },
            socket: root.join("actor.sock"), supervisor: policy, actor: policy,
            candidates: 16, connections: 16, exchanges: 16, runtime_ms: 15_000,
            poll_ms: 1, reply_ms: 50,
        };
        let mut settings = settings(); settings.socket = root.join("precheck.sock");
        let settings_json = format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":0,"disclosure_id":81,"query_limit":16,"exchange_limit":16,"candidate_limit":16,"connection_limit":16,"runtime_ms":15000}}"#, settings.socket.display());
        let actor_json = format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":7,"scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5}},"socket":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"actor":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":16,"connection_limit":16,"exchange_limit":16,"runtime_ms":15000,"poll_ms":1,"reply_ms":50}}"#,
            actor.socket.display(), id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid());
        let actor_file = root.join("actor.json"); let settings_file = root.join("precheck.json");
        fs::write(&actor_file, actor_json).unwrap(); fs::write(&settings_file, settings_json).unwrap();
        let documents = (0..count).map(|index| {
            let path = root.join(format!("query-{index}.json"));
            fs::write(&path, encode_precheck_request(&request(97 + index as u64, 0)).unwrap()).unwrap();
            path
        }).collect();
        Self { root, actor, settings, actor_file, settings_file, documents }
    }
    fn args(&self, mode: Mode) -> Vec<String> {
        let mut args = vec!["actor-precheck".into(), match mode {
            Mode::All => "--batch".into(), Mode::FirstPass => "--first-pass".into(),
        }, self.actor_file.to_str().unwrap().into(), self.settings_file.to_str().unwrap().into()];
        args.extend(self.documents.iter().map(|path| path.to_str().unwrap().to_owned()));
        args
    }
    fn paths(&self) -> Vec<&Path> { self.documents.iter().map(PathBuf::as_path).collect() }
    fn listener(&self) -> UnixListener {
        let listener = UnixListener::bind(&self.settings.socket).unwrap();
        listener.set_nonblocking(true).unwrap(); listener
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("batch fixture cleanup: {error}"); }
    }
}
fn frame(response: &PrecheckWireResponse) -> Vec<u8> {
    let mut bytes = response.encode().unwrap(); bytes.push(b'\n'); bytes
}
type Peer = thread::JoinHandle<(UnixListener, Vec<Vec<u8>>)>;
fn server(listener: UnixListener, replies: Vec<Vec<u8>>) -> Peer {
    thread::spawn(move || {
        let mut received = Vec::new();
        for reply in replies {
            let started = Instant::now();
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(started.elapsed() < Duration::from_secs(5));
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => panic!("scripted batch accept: {error}"),
                }
            };
            socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut bytes = Vec::new();
            while bytes.len() <= MAX_PRECHECK_FRAME_BYTES {
                let mut byte = [0];
                match socket.read(&mut byte).unwrap() {
                    0 => break,
                    1 => { bytes.push(byte[0]); if byte[0] == b'\n' { break; } }
                    _ => unreachable!(),
                }
            }
            if !bytes.is_empty() {
                socket.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
                socket.write_all(&reply).unwrap(); socket.flush().unwrap();
            }
            received.push(bytes);
        }
        (listener, received)
    })
}
fn assert_requests(peer: Peer, queries: &[u64]) {
    let (listener, received) = peer.join().unwrap();
    let expected: Vec<_> = queries.iter().map(|query| {
        let mut bytes = encode_precheck_request(&request(*query, 0)).unwrap();
        bytes.push(b'\n'); bytes
    }).collect();
    assert_eq!(received, expected);
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}
fn dispatch(fixture: &Fixture, mode: Mode, out: &mut impl Write) -> Result<(), String> {
    // Enter the existing single-query dispatcher, not the batch helper directly.
    super::super::super::command(&fixture.args(mode), out)
}

#[test]
fn batch_emits_all_native_results_and_keeps_submission_unopened() {
    let fixture = Fixture::new(3);
    let replies = vec![frame(&known(97, 1, FilePrecheckOutcome::DeniedByPolicy)),
        frame(&known(98, 1, FilePrecheckOutcome::PrecheckPassed)),
        frame(&known(99, 1, FilePrecheckOutcome::DeniedByPolicy))];
    let expected = replies.concat(); let peer = server(fixture.listener(), replies);
    let mut out = Vec::new(); dispatch(&fixture, Mode::All, &mut out).unwrap();
    assert_eq!(out, expected); assert_requests(peer, &[97, 98, 99]);
    assert!(!fixture.actor.socket.exists()); assert!(!fixture.root.join("store").exists());
}

#[test]
fn first_pass_stops_after_the_first_successful_native_response() {
    let fixture = Fixture::new(3);
    let replies = vec![frame(&known(97, 1, FilePrecheckOutcome::DeniedByPolicy)),
        frame(&known(98, 1, FilePrecheckOutcome::PrecheckPassed))];
    let expected = replies.concat(); let peer = server(fixture.listener(), replies);
    let mut out = Vec::new(); dispatch(&fixture, Mode::FirstPass, &mut out).unwrap();
    assert_eq!(out, expected); assert_requests(peer, &[97, 98]);
}

#[test]
fn all_denied_is_not_success_in_either_mode() {
    for mode in [Mode::All, Mode::FirstPass] {
        let fixture = Fixture::new(2);
        let replies = vec![frame(&known(97, 1, FilePrecheckOutcome::DeniedByPolicy)),
            frame(&known(98, 1, FilePrecheckOutcome::DeniedByPolicy))];
        let expected = replies.concat(); let peer = server(fixture.listener(), replies);
        let mut out = Vec::new();
        assert!(dispatch(&fixture, mode, &mut out).unwrap_err().contains("no passing candidate"));
        assert_eq!(out, expected); assert_requests(peer, &[97, 98]);
    }
}

#[test]
fn unknown_withheld_stale_and_capacity_stop_without_spending_a_later_query() {
    for result in [
        Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }),
        Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" }),
        Ok(Knowledge::Stale { generation: 0, current: 1 }),
        Err(PrecheckWireError::Capacity),
    ] {
        let fixture = Fixture::new(3);
        let replies = vec![frame(&known(97, 1, FilePrecheckOutcome::DeniedByPolicy)),
            frame(&PrecheckWireResponse { query: Some(98), result })];
        let expected = replies.concat(); let peer = server(fixture.listener(), replies);
        let mut out = Vec::new();
        assert!(dispatch(&fixture, Mode::FirstPass, &mut out).unwrap_err().contains("incomplete"));
        assert_eq!(out, expected); assert_requests(peer, &[97, 98]);
    }
}

#[test]
fn invalid_later_inputs_prevent_even_the_first_connection() {
    for variant in 0..5 {
        let mut fixture = Fixture::new(2); let listener = fixture.listener();
        match variant {
            0 => fs::write(&fixture.documents[1], b"{}").unwrap(),
            1 => fs::write(&fixture.documents[1], encode_precheck_request(&request(97, 0)).unwrap()).unwrap(),
            2 => fs::write(&fixture.documents[1], encode_precheck_request(&request(98, 1)).unwrap()).unwrap(),
            3 => fs::write(&fixture.documents[1], vec![b' '; MAX_PRECHECK_FRAME_BYTES + 1]).unwrap(),
            _ => {
                let alias = fixture.root.join("alias.json");
                std::os::unix::fs::symlink(&fixture.documents[1], &alias).unwrap();
                fixture.documents[1] = alias;
            }
        }
        let mut out = Vec::new(); assert!(dispatch(&fixture, Mode::FirstPass, &mut out).is_err());
        assert!(out.is_empty());
        assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
    }
}

#[test]
fn aggregate_input_limit_prevents_partial_batch_execution() {
    let fixture = Fixture::new(5); let listener = fixture.listener();
    for (index, document) in fixture.documents.iter().enumerate() {
        let mut bytes = encode_precheck_request(&request(97 + index as u64, 0)).unwrap();
        bytes.resize(MAX_PRECHECK_FRAME_BYTES, b' ');
        assert!(decode_precheck_request(&bytes).is_ok());
        fs::write(document, bytes).unwrap();
    }
    let mut out = Vec::new(); assert!(dispatch(&fixture, Mode::All, &mut out).is_err());
    assert!(out.is_empty());
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn lost_reply_does_not_retry_or_contact_the_next_candidate() {
    let fixture = Fixture::new(3); let peer = server(fixture.listener(), vec![Vec::new()]);
    let mut out = Vec::new(); assert!(dispatch(&fixture, Mode::All, &mut out).is_err());
    assert!(out.is_empty()); assert_requests(peer, &[97]);
}

struct PartialOutput { bytes: Vec<u8> }
impl Write for PartialOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.bytes.is_empty() { return Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture")); }
        let count = bytes.len().min(3); self.bytes.extend_from_slice(&bytes[..count]); Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}
#[test]
fn partial_output_failure_stops_both_batch_modes_even_after_a_pass() {
    for mode in [Mode::All, Mode::FirstPass] {
        let fixture = Fixture::new(3);
        let reply = frame(&known(97, 1, FilePrecheckOutcome::PrecheckPassed));
        let peer = server(fixture.listener(), vec![reply.clone()]);
        let mut out = PartialOutput { bytes: Vec::new() };
        assert!(dispatch(&fixture, mode, &mut out).unwrap_err().contains("output failed"));
        assert_eq!(out.bytes, reply[..3]); assert_requests(peer, &[97]);
    }
}

#[test]
fn native_correlation_and_disclosure_errors_never_reach_batch_stdout() {
    for field in ["query", "epoch", "disclosure"] {
        let fixture = Fixture::new(3);
        let mut response = known(97, 1, FilePrecheckOutcome::PrecheckPassed);
        if let Ok(Knowledge::Known { basis, .. }) = &mut response.result {
            match field {
                "query" => { response.query = Some(98); basis.query = 98; }
                "epoch" => basis.policy_epoch = 1,
                _ => basis.disclosure = 82,
            }
        }
        let peer = server(fixture.listener(), vec![frame(&response)]);
        let mut out = Vec::new(); assert!(dispatch(&fixture, Mode::All, &mut out).is_err());
        assert!(out.is_empty()); assert_requests(peer, &[97]);
    }
}

#[test]
fn visibly_changed_observation_stops_before_output_and_the_next_connection() {
    let fixture = Fixture::new(3);
    let first = frame(&known(97, 1, FilePrecheckOutcome::DeniedByPolicy));
    let peer = server(fixture.listener(), vec![first.clone(),
        frame(&known(98, 2, FilePrecheckOutcome::PrecheckPassed))]);
    let mut out = Vec::new();
    assert!(dispatch(&fixture, Mode::FirstPass, &mut out).unwrap_err().contains("observation changed"));
    assert_eq!(out, first); assert_requests(peer, &[97, 98]);
}

#[test]
fn authentication_failure_sends_no_candidate_and_does_not_continue() {
    let fixture = Fixture::new(3); let mut actor = fixture.actor.clone();
    let original = actor.supervisor;
    actor.supervisor = PeerPolicy::new(original.uid(), original.gid(),
        Some(if original.pid() == Some(1) { 2 } else { 1 })).unwrap();
    let peer = server(fixture.listener(), vec![Vec::new()]); let mut out = Vec::new();
    assert!(query(Mode::All, &actor, &fixture.settings, &fixture.paths(), Instant::now(), &mut out).is_err());
    let (listener, received) = peer.join().unwrap();
    assert_eq!(received, vec![Vec::<u8>::new()]); assert!(out.is_empty());
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn expired_batch_opens_neither_document_nor_connection() {
    let fixture = Fixture::new(2); let listener = fixture.listener();
    let started = Instant::now().checked_sub(Duration::from_millis(15_001)).unwrap();
    let missing = fixture.root.join("missing.json"); let mut out = Vec::new();
    let error = query(Mode::All, &fixture.actor, &fixture.settings, &[&missing], started, &mut out).unwrap_err();
    assert!(error.contains("elapsed allowance")); assert!(out.is_empty());
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

struct ChangeLaterInput { path: PathBuf, bytes: Vec<u8> }
impl Write for ChangeLaterInput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes); Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { fs::write(&self.path, b"{}") }
}
#[test]
fn prepared_candidates_do_not_change_when_files_are_rewritten_during_output() {
    let fixture = Fixture::new(2);
    let replies = vec![frame(&known(97, 1, FilePrecheckOutcome::DeniedByPolicy)),
        frame(&known(98, 1, FilePrecheckOutcome::PrecheckPassed))];
    let expected = replies.concat(); let peer = server(fixture.listener(), replies);
    let mut out = ChangeLaterInput { path: fixture.documents[1].clone(), bytes: Vec::new() };
    dispatch(&fixture, Mode::All, &mut out).unwrap();
    assert_eq!(out.bytes, expected); assert_eq!(fs::read(&out.path).unwrap(), b"{}");
    assert_requests(peer, &[97, 98]);
}

#[test]
fn executable_dispatch_preserves_both_closed_batch_grammars() {
    for mode in ["--batch", "--first-pass"] {
        for option in ["--credibility-activation", "--andon", "--precheck-profile", "--durable"] {
            let args = ["actor-precheck", mode, "missing-actor", "missing-profile", "missing-query", option, "missing"]
                .into_iter().map(str::to_owned).collect();
            assert_eq!(crate::command(args), Err(USAGE.to_owned()));
        }
    }
}
