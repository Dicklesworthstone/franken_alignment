//! Scripted replies establish CLI behavior, not truth of provider observations.
use super::*;
use fa_reference::action::{Purpose, Scope};
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    PrecheckWireError, discovery::decode_capabilities_response,
};
use fa_reference::action::consequence::oversight::actor::UnknownReason;
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use std::fs;
use std::io::{self, Read};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf, actor: Profile, settings: Settings,
    actor_file: PathBuf, settings_file: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = crate::test_directory::create(format!("fa-dc-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let (socket, _) = UnixStream::pair().unwrap(); let id = PeerCredentials::observe(&socket).unwrap();
        let policy = PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap();
        let actor = Profile { request: 7,
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            socket: root.join("actor.sock"), supervisor: policy, actor: policy,
            candidates: 4, connections: 2, exchanges: 8, runtime_ms: 15_000, poll_ms: 1, reply_ms: 50 };
        let settings_json = format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}/precheck.sock","expected_policy_epoch":0,"disclosure_id":81,"query_limit":2,"exchange_limit":2,"candidate_limit":4,"connection_limit":2,"runtime_ms":15000}}"#, root.display());
        let settings = Settings::decode(settings_json.as_bytes()).unwrap();
        let actor_json = format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":7,"scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5}},"socket":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"actor":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":4,"connection_limit":2,"exchange_limit":8,"runtime_ms":15000,"poll_ms":1,"reply_ms":50}}"#,
            actor.socket.display(), id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid());
        let actor_file = root.join("actor.json"); let settings_file = root.join("precheck.json");
        fs::write(&actor_file, actor_json).unwrap(); fs::write(&settings_file, settings_json).unwrap();
        Self { root, actor, settings, actor_file, settings_file }
    }
    fn args(&self) -> Vec<String> {
        vec!["actor-precheck".into(), "--capabilities".into(), self.actor_file.to_str().unwrap().into(),
            self.settings_file.to_str().unwrap().into(), "97".into()]
    }
    fn listener(&self) -> UnixListener {
        let listener = UnixListener::bind(&self.settings.socket).unwrap();
        listener.set_nonblocking(true).unwrap(); listener
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) { eprintln!("discovery CLI cleanup: {error}"); }
    }
}
fn known(remaining: usize) -> PrecheckCapabilitiesResponse {
    // Decode a scripted observation through the actual closed public grammar.
    let json = format!(r#"{{"schema":"fa.actor-precheck-capabilities-response/1","informational":true,"query":"97","status":"ok","knowledge":{{"state":"known","value":{{"policy_disclosure":"exact_predicates","precheck_scope":"exact_policy","policy_contents":"withheld","helper_feedback":"withheld","source_values":"withheld","witness_cache":"unavailable","source_refresh":"unavailable","source_semantics":"captured_cut","correlation":"labels_only","timing":"observable","signed_profile":false,"effect_authority":false,"review_required":true,"remaining_queries":"{remaining}"}},"basis":{{"disclosure":"81","query":"97","policy_epoch":"0","observed_at":"1"}}}}}}"#);
    decode_capabilities_response(json.as_bytes(), 97, 0).unwrap()
}
fn frame(response: &PrecheckCapabilitiesResponse) -> Vec<u8> {
    let mut bytes = response.encode().unwrap(); bytes.push(b'\n'); bytes
}
type Peer = thread::JoinHandle<(UnixListener, Vec<u8>)>;
fn server(listener: UnixListener, reply: Vec<u8>) -> Peer {
    thread::spawn(move || {
        let started = Instant::now();
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(started.elapsed() < Duration::from_secs(5)); thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("discovery accept: {error}"),
            }
        };
        socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap(); let mut received = Vec::new();
        while received.len() <= MAX_CAPABILITIES_REQUEST_BYTES {
            let mut byte = [0];
            match socket.read(&mut byte).unwrap() {
                0 => break,
                1 => { received.push(byte[0]); if byte[0] == b'\n' { break; } }
                _ => unreachable!(),
            }
        }
        if !received.is_empty() {
            socket.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            socket.write_all(&reply).unwrap(); socket.flush().unwrap();
        }
        (listener, received)
    })
}
fn assert_single(peer: Peer) {
    let (listener, received) = peer.join().unwrap();
    let mut expected = PrecheckCapabilitiesRequest { query: 97, expected_policy_epoch: 0 }.encode().unwrap();
    expected.push(b'\n'); assert_eq!(received, expected);
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}
fn dispatch(fixture: &Fixture, out: &mut impl Write) -> Result<(), String> {
    super::super::command(&fixture.args(), out)
}

#[test]
fn public_profiles_emit_one_native_observation_without_opening_proposals_or_submission() {
    for remaining in [0, 1] {
        let fixture = Fixture::new(); let response = known(remaining);
        let peer = server(fixture.listener(), frame(&response)); let mut out = Vec::new();
        dispatch(&fixture, &mut out).unwrap(); assert_eq!(out, frame(&response)); assert_single(peer);
        assert!(!fixture.actor.socket.exists() && !fixture.root.join("store").exists());
        assert_ne!(fixture.actor.request, 97);
    }
}

#[test]
fn unknown_withheld_stale_and_native_refusals_render_without_success_or_retry() {
    for result in [
        Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }),
        Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" }),
        Ok(Knowledge::Stale { generation: 0, current: 1 }), Err(PrecheckWireError::Capacity),
    ] {
        let fixture = Fixture::new(); let response = PrecheckCapabilitiesResponse { query: Some(97), result };
        let peer = server(fixture.listener(), frame(&response)); let mut out = Vec::new();
        assert!(dispatch(&fixture, &mut out).unwrap_err().contains("capabilities unavailable"));
        assert_eq!(out, frame(&response)); assert_single(peer);
    }
}

#[test]
fn wrong_query_epoch_or_selected_disclosure_is_not_printed() {
    for field in ["query", "epoch", "disclosure"] {
        let fixture = Fixture::new(); let mut response = known(1);
        if let Ok(Knowledge::Known { basis, .. }) = &mut response.result {
            match field {
                "query" => { response.query = Some(98); basis.query = 98; }
                "epoch" => basis.policy_epoch = 1,
                _ => basis.disclosure = 82,
            }
        }
        let peer = server(fixture.listener(), frame(&response)); let mut out = Vec::new();
        assert!(dispatch(&fixture, &mut out).is_err()); assert!(out.is_empty()); assert_single(peer);
    }
}

#[test]
fn malformed_truncated_extra_and_lost_responses_do_not_trigger_fallback_or_retry() {
    let good = frame(&known(1));
    for reply in [Vec::new(), b"{}\n".to_vec(), good[..good.len() - 1].to_vec(),
        [good.clone(), b"x".to_vec()].concat(), vec![b'x'; 1026]] {
        let fixture = Fixture::new(); let peer = server(fixture.listener(), reply); let mut out = Vec::new();
        assert!(dispatch(&fixture, &mut out).is_err()); assert!(out.is_empty()); assert_single(peer);
    }
}

#[test]
fn discovery_authenticates_before_bytes_and_expiry_refuses_before_connect() {
    let fixture = Fixture::new(); let mut actor = fixture.actor.clone(); let original = actor.supervisor;
    actor.supervisor = PeerPolicy::new(original.uid(), original.gid(),
        Some(if original.pid() == Some(1) { 2 } else { 1 })).unwrap();
    let peer = server(fixture.listener(), Vec::new()); let mut out = Vec::new();
    assert!(query(&actor, &fixture.settings, 97, Instant::now(), &mut out).is_err());
    let (listener, received) = peer.join().unwrap(); assert!(received.is_empty() && out.is_empty());
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
    let started = Instant::now().checked_sub(Duration::from_millis(15_001)).unwrap();
    assert!(query(&fixture.actor, &fixture.settings, 97, started, &mut out).unwrap_err().contains("elapsed allowance"));
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn grammar_and_full_width_query_validation_precede_all_file_reads_and_shared_options() {
    for suffix in [vec![], vec!["extra"], vec!["--credibility-activation", "missing"],
        vec!["--andon", "missing"], vec!["--durable", "missing"], vec!["--batch"],
        vec!["--precheck-profile", "missing"]] {
        let mut args: Vec<String> = ["actor-precheck", "--capabilities", "missing-actor", "missing-profile"]
            .into_iter().map(str::to_owned).collect();
        if !suffix.is_empty() { args.push("97".into()); }
        args.extend(suffix.into_iter().map(str::to_owned));
        assert_eq!(crate::command(args), Err(USAGE.to_owned()));
    }
    for id in ["", "0", "01", "+1", "-1", "1.0", "1e2", " 1", "18446744073709551616"] {
        let args = ["actor-precheck", "--capabilities", "missing-actor", "missing-profile", id]
            .into_iter().map(str::to_owned).collect();
        assert_eq!(crate::command(args), Err(USAGE.to_owned()));
    }
    let args: Vec<_> = ["actor-precheck", "--capabilities", "a", "p", "18446744073709551615"]
        .into_iter().map(str::to_owned).collect();
    assert_eq!(arguments(&args).unwrap(), u64::MAX);
}

#[test]
fn discovery_uses_one_lifetime_io_and_elapsed_allowance_without_resending() {
    let fixture = Fixture::new();
    for elapsed_limit in [false, true] {
        let (socket, mut server) = UnixStream::pair().unwrap();
        let client = PrecheckCapabilitiesClient::connect_capabilities(socket, fixture.actor.supervisor,
            PrecheckCapabilitiesRequest { query: 97, expected_policy_epoch: 0 }, 0).unwrap();
        let mut work = work(); if !elapsed_limit { work.io_calls = 3; }
        assert_eq!(work.write_bytes, MAX_CAPABILITIES_REQUEST_BYTES + 1);
        let wall = Duration::from_secs(15); let mut checks = 0;
        let error = drive_client_shared(client, 1, wall, &mut work, || {
            checks += 1; if elapsed_limit && checks > 2 { wall } else { Duration::ZERO }
        }).unwrap_err();
        assert!(error.contains(if elapsed_limit { "elapsed allowance" } else { "lifetime I/O allowance" }));
        server.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut received = Vec::new(); server.read_to_end(&mut received).unwrap();
        let mut expected = PrecheckCapabilitiesRequest { query: 97, expected_policy_epoch: 0 }.encode().unwrap();
        expected.push(b'\n'); assert_eq!(received, expected);
    }
}

struct BrokenOutput { bytes: Vec<u8> }
impl Write for BrokenOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.bytes.is_empty() { return Err(io::Error::from(io::ErrorKind::BrokenPipe)); }
        let count = bytes.len().min(3); self.bytes.extend_from_slice(&bytes[..count]); Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> { Err(io::Error::from(io::ErrorKind::BrokenPipe)) }
}
#[test]
fn partial_output_failure_leaves_a_prefix_but_never_resends_the_discovery() {
    let fixture = Fixture::new(); let reply = frame(&known(1));
    let peer = server(fixture.listener(), reply.clone()); let mut out = BrokenOutput { bytes: Vec::new() };
    assert!(dispatch(&fixture, &mut out).unwrap_err().contains("output failed"));
    assert_eq!(out.bytes, reply[..3]); assert_single(peer);
}
