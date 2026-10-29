//! Real sockets with scripted responses test the operator boundary, not truth of
//! provider observations. Source-backed sidecar integration is tested separately.
use super::*;
use fa_reference::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FilePrecheckBasis;
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    PrecheckWireError, decode_precheck_response, encode_precheck_request,
};
use fa_reference::action::consequence::oversight::actor::{ActorProposal, UnknownReason};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_transport::IoDirection;
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
    document: PathBuf,
    actor_file: PathBuf,
    settings_file: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
            .unwrap().as_nanos();
        let root = crate::test_directory::create(format!("fa-pc-{}-{stamp}-{}",
            std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let (socket, _) = UnixStream::pair().unwrap();
        let id = PeerCredentials::observe(&socket).unwrap();
        let policy = PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap();
        let actor = Profile {
            request: 7,
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            socket: root.join("actor.sock"), supervisor: policy, actor: policy,
            candidates: 4, connections: 2, exchanges: 8, runtime_ms: 15_000,
            poll_ms: 1, reply_ms: 50,
        };
        let settings_json = format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}/precheck.sock","expected_policy_epoch":0,"disclosure_id":81,"query_limit":2,"exchange_limit":2,"candidate_limit":4,"connection_limit":2,"runtime_ms":15000}}"#, root.display());
        let settings = Settings::decode(settings_json.as_bytes()).unwrap();
        let actor_json = format!(r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":7,"scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5}},"socket":"{}","supervisor":{{"uid":{},"gid":{},"pid":{}}},"actor":{{"uid":{},"gid":{},"pid":{}}},"candidate_limit":4,"connection_limit":2,"exchange_limit":8,"runtime_ms":15000,"poll_ms":1,"reply_ms":50}}"#,
            actor.socket.display(), id.uid(), id.gid(), id.pid(), id.uid(), id.gid(), id.pid());
        let actor_file = root.join("actor.json");
        let settings_file = root.join("precheck.json");
        let document = root.join("query.json");
        fs::write(&actor_file, actor_json).unwrap();
        fs::write(&settings_file, settings_json).unwrap();
        fs::write(&document, encode_precheck_request(&request()).unwrap()).unwrap();
        Self { root, actor, settings, document, actor_file, settings_file }
    }
    fn args(&self) -> Vec<String> {
        vec!["actor-precheck".into(), self.actor_file.to_str().unwrap().into(),
            self.settings_file.to_str().unwrap().into(), self.document.to_str().unwrap().into()]
    }
    fn listener(&self) -> UnixListener { UnixListener::bind(&self.settings.socket).unwrap() }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("precheck client fixture cleanup: {error}");
        }
    }
}

fn request() -> PrecheckRequest {
    PrecheckRequest { query: 97, proposal: ActorProposal {
        target: ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 },
        payload: b"ok".to_vec(), units: 2, deadline: ElapsedTick(100_000), expected_policy_epoch: 0,
    } }
}
fn observed(value: FilePrecheckOutcome) -> PrecheckWireResponse {
    PrecheckWireResponse { query: Some(97), result: Ok(Knowledge::Known { value,
        basis: FilePrecheckBasis { disclosure: 81, query: 97, policy_epoch: 0, observed_at: ElapsedTick(1) },
    }) }
}
fn frame(response: &PrecheckWireResponse) -> Vec<u8> {
    let mut bytes = response.encode().unwrap(); bytes.push(b'\n'); bytes
}
fn read_query(socket: &mut UnixStream) -> Vec<u8> {
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
    bytes
}
fn server(listener: UnixListener, reply: Vec<u8>)
    -> thread::JoinHandle<(UnixListener, Vec<u8>)>
{
    thread::spawn(move || {
        // The caller binds before starting the client; no path polling is needed.
        listener.set_nonblocking(true).unwrap();
        let started = Instant::now();
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    assert!(started.elapsed() < Duration::from_secs(5)); pause(1);
                }
                Err(error) => panic!("scripted precheck accept: {error}"),
            }
        };
        let bytes = read_query(&mut socket);
        if !bytes.is_empty() {
            socket.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
            socket.write_all(&reply).unwrap(); socket.flush().unwrap();
        }
        drop(socket);
        (listener, bytes)
    })
}
fn assert_single_request(peer: thread::JoinHandle<(UnixListener, Vec<u8>)>) {
    let (listener, bytes) = peer.join().unwrap();
    let mut expected = encode_precheck_request(&request()).unwrap(); expected.push(b'\n');
    assert_eq!(bytes, expected);
    // Examine only after the command returns: any hidden reconnect is queued.
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn public_profiles_drive_exactly_one_query_and_only_native_pass_output() {
    let fixture = Fixture::new();
    let response = observed(FilePrecheckOutcome::PrecheckPassed);
    let peer = server(fixture.listener(), frame(&response));
    let mut output = Vec::new();
    command(&fixture.args(), &mut output).unwrap();
    assert_eq!(output, frame(&response));
    assert_eq!(decode_precheck_response(output.strip_suffix(b"\n").unwrap(), 97, 0).unwrap(), response);
    assert!(String::from_utf8(output).unwrap().contains("\"review_required\":true"));
    assert_ne!(fixture.actor.request, request().query);
    assert_single_request(peer);
    assert!(!fixture.actor.socket.exists());
    assert!(!fixture.root.join("store").exists());
}

#[test]
fn valid_denials_and_coarse_refusals_render_without_success_or_resend() {
    for response in [
        observed(FilePrecheckOutcome::DeniedByPolicy),
        PrecheckWireResponse { query: Some(97), result: Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" }) },
        PrecheckWireResponse { query: Some(97), result: Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }) },
        PrecheckWireResponse { query: Some(97), result: Ok(Knowledge::Stale { generation: 0, current: 1 }) },
        PrecheckWireResponse { query: Some(97), result: Err(PrecheckWireError::Capacity) },
        PrecheckWireResponse { query: None, result: Err(PrecheckWireError::Capacity) },
    ] {
        let fixture = Fixture::new(); let peer = server(fixture.listener(), frame(&response));
        let mut output = Vec::new();
        assert!(command(&fixture.args(), &mut output).unwrap_err().contains("did not pass"));
        assert_eq!(output, frame(&response));
        assert_single_request(peer);
    }
}

#[test]
fn wrong_query_epoch_or_selected_disclosure_never_reaches_stdout() {
    for field in ["query", "epoch", "disclosure"] {
        let fixture = Fixture::new(); let mut response = observed(FilePrecheckOutcome::PrecheckPassed);
        if let Ok(Knowledge::Known { basis, .. }) = &mut response.result {
            match field {
                "query" => { response.query = Some(98); basis.query = 98; }
                "epoch" => basis.policy_epoch = 1,
                _ => basis.disclosure = 82,
            }
        }
        let peer = server(fixture.listener(), frame(&response)); let mut output = Vec::new();
        assert!(command(&fixture.args(), &mut output).is_err());
        assert!(output.is_empty()); assert_single_request(peer);
    }
}

#[test]
fn malformed_truncated_oversized_and_extra_replies_never_become_passes() {
    let pass = frame(&observed(FilePrecheckOutcome::PrecheckPassed));
    let text = String::from_utf8(pass.clone()).unwrap();
    for bytes in [
        Vec::new(), pass[..pass.len() - 1].to_vec(), b"{}\n".to_vec(),
        text.replace("\"review_required\":true", "\"review_required\":false").into_bytes(),
        text.replace("fa.actor-precheck-response/1", "fa.actor-precheck-response/2").into_bytes(),
        [pass.clone(), b"x".to_vec()].concat(),
        vec![b'x'; MAX_PRECHECK_RESPONSE_BYTES + 2],
    ] {
        let fixture = Fixture::new(); let peer = server(fixture.listener(), bytes);
        let mut output = Vec::new(); assert!(command(&fixture.args(), &mut output).is_err());
        assert!(output.is_empty()); assert_single_request(peer);
    }
}

#[test]
fn server_authentication_failure_sends_no_proposal_bytes() {
    let fixture = Fixture::new(); let mut actor = fixture.actor.clone();
    let original = actor.supervisor;
    actor.supervisor = PeerPolicy::new(original.uid(), original.gid(),
        Some(if original.pid() == Some(1) { 2 } else { 1 })).unwrap();
    let peer = server(fixture.listener(), Vec::new()); let mut output = Vec::new();
    assert!(query(&actor, &fixture.settings, &fixture.document, Instant::now(), &mut output).is_err());
    let (listener, bytes) = peer.join().unwrap();
    assert!(bytes.is_empty() && output.is_empty());
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn input_and_independent_epoch_errors_refuse_before_connecting() {
    for variant in 0..4 {
        let fixture = Fixture::new(); let listener = fixture.listener();
        listener.set_nonblocking(true).unwrap();
        let mut settings = fixture.settings.clone();
        match variant {
            0 => { settings.expected_policy_epoch = 1; }
            1 => fs::write(&fixture.document, b"{}").unwrap(),
            2 => fs::write(&fixture.document, vec![b' '; MAX_PRECHECK_FRAME_BYTES + 1]).unwrap(),
            _ => {
                let alias = fixture.root.join("alias.json");
                std::os::unix::fs::symlink(&fixture.document, &alias).unwrap();
                let mut output = Vec::new();
                assert!(query(&fixture.actor, &settings, &alias, Instant::now(), &mut output).is_err());
                assert!(output.is_empty());
                assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
                continue;
            }
        }
        let mut output = Vec::new();
        assert!(query(&fixture.actor, &settings, &fixture.document, Instant::now(), &mut output).is_err());
        assert!(output.is_empty());
        assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
    }
}

#[test]
fn closed_command_grammar_refuses_options_and_missing_or_extra_inputs_before_reads() {
    for args in [
        vec![], vec!["actor-precheck", "missing", "missing"],
        vec!["actor-submit", "missing", "missing", "missing"],
        vec!["actor-precheck", "missing", "missing", "missing", "extra"],
        vec!["actor-precheck", "--credibility-activation", "missing", "missing"],
        vec!["actor-precheck", "missing", "--durable", "missing"],
        vec!["actor-precheck", "missing", "missing", "--native-text"],
    ] {
        let args: Vec<_> = args.into_iter().map(str::to_owned).collect();
        let mut output = Vec::new();
        assert_eq!(command(&args, &mut output), Err(USAGE.to_owned()));
        assert!(output.is_empty());
    }
}

#[test]
fn executable_dispatch_preserves_actor_grammar_before_supervisor_option_removal() {
    for suffix in [
        vec!["--credibility-activation", "missing-evidence"],
        vec!["--andon", "missing-budget"],
        vec!["--precheck-profile", "missing-profile"],
        vec!["--durable", "missing-journal"],
    ] {
        let mut args: Vec<String> = ["actor-precheck", "missing-actor", "missing-precheck", "missing-query"]
            .into_iter().map(str::to_owned).collect();
        args.extend(suffix.into_iter().map(str::to_owned));
        assert_eq!(crate::command(args), Err(USAGE.to_owned()));
    }
}

#[test]
fn already_expired_cooperative_allowance_opens_no_query_or_socket() {
    let fixture = Fixture::new(); let listener = fixture.listener();
    listener.set_nonblocking(true).unwrap();
    let started = Instant::now().checked_sub(Duration::from_millis(15_001)).unwrap();
    let mut output = Vec::new();
    let error = query(&fixture.actor, &fixture.settings, &fixture.root.join("absent.json"), started, &mut output).unwrap_err();
    assert!(error.contains("elapsed allowance")); assert!(output.is_empty());
    assert_eq!(listener.accept().unwrap_err().kind(), io::ErrorKind::WouldBlock);
}

#[test]
fn lifetime_io_allowance_is_not_replenished_by_another_drive() {
    let fixture = Fixture::new(); let (socket, mut server) = UnixStream::pair().unwrap();
    let work = Work { io_calls: 3, ..Work::new() };
    let error = exchange(socket, &fixture.actor, &fixture.settings, request(), Instant::now(), work).unwrap_err();
    assert!(error.contains("lifetime I/O allowance"));
    let mut received = Vec::new(); server.read_to_end(&mut received).unwrap();
    let mut expected = encode_precheck_request(&request()).unwrap(); expected.push(b'\n');
    assert_eq!(received, expected);
}

#[test]
fn lifetime_byte_and_call_debits_are_monotone_and_atomic_on_overflow() {
    let mut work = Work::new();
    let initial = (work.read_bytes, work.write_bytes, work.io_calls);
    work.debit(DriveProgress { read_bytes: 7, written_bytes: 11, io_calls: 3,
        would_block: Some(IoDirection::Read), ..DriveProgress::default() }).unwrap();
    assert_eq!((work.read_bytes, work.write_bytes, work.io_calls),
        (initial.0 - 7, initial.1 - 11, initial.2 - 3));
    work.debit(DriveProgress { io_calls: 1, ..DriveProgress::default() }).unwrap();
    let before = (work.read_bytes, work.write_bytes, work.io_calls);
    assert!(work.debit(DriveProgress { read_bytes: work.read_bytes + 1,
        written_bytes: 1, io_calls: 1, ..DriveProgress::default() }).is_err());
    assert_eq!((work.read_bytes, work.write_bytes, work.io_calls), before);
    work.read_bytes = 0;
    assert!(work.budget(PrecheckClientPhase::Reading).is_err());
    assert!(work.budget(PrecheckClientPhase::Writing).is_ok());
    work.write_bytes = 0;
    assert!(work.budget(PrecheckClientPhase::Writing).is_err());
    assert!(work.budget(PrecheckClientPhase::Flushing).is_ok());
}

#[test]
fn unavailable_reply_times_out_without_polling_or_resending_the_query() {
    let fixture = Fixture::new();
    let (socket, mut server) = UnixStream::pair().unwrap();
    let client = PrecheckPeerClient::connect(socket, fixture.actor.supervisor, request(), 0).unwrap();
    let wall = Duration::from_secs(15); let mut checks = 0;
    let error = drive_client(client, 1, wall, Work::new(), || {
        checks += 1;
        if checks <= 2 { Duration::ZERO } else { wall }
    }).unwrap_err();
    assert!(error.contains("elapsed allowance"));
    let mut received = Vec::new(); server.read_to_end(&mut received).unwrap();
    let mut expected = encode_precheck_request(&request()).unwrap(); expected.push(b'\n');
    assert_eq!(received, expected);
}

#[test]
fn a_complete_result_observed_after_the_elapsed_boundary_is_not_success() {
    let fixture = Fixture::new();
    let (socket, mut server) = UnixStream::pair().unwrap();
    server.write_all(&frame(&observed(FilePrecheckOutcome::PrecheckPassed))).unwrap();
    let client = PrecheckPeerClient::connect(socket, fixture.actor.supervisor, request(), 0).unwrap();
    let wall = Duration::from_secs(15); let mut checks = 0;
    let error = drive_client(client, 1, wall, Work::new(), || {
        checks += 1;
        if checks == 1 { Duration::ZERO } else { wall }
    }).unwrap_err();
    assert!(error.contains("elapsed allowance"));
    let mut received = Vec::new(); server.read_to_end(&mut received).unwrap();
    let mut expected = encode_precheck_request(&request()).unwrap(); expected.push(b'\n');
    assert_eq!(received, expected);
}

struct BrokenOutput { bytes: Vec<u8>, flush_failure: bool }
impl Write for BrokenOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.flush_failure { return Err(io::ErrorKind::BrokenPipe.into()); }
        self.bytes.extend_from_slice(bytes); Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
}

#[test]
fn output_failure_after_received_result_never_retries_or_claims_success() {
    for flush_failure in [false, true] {
        let fixture = Fixture::new(); let response = observed(FilePrecheckOutcome::PrecheckPassed);
        let peer = server(fixture.listener(), frame(&response));
        let mut output = BrokenOutput { bytes: Vec::new(), flush_failure };
        let error = command(&fixture.args(), &mut output).unwrap_err();
        assert!(error.contains("output failed") && error.contains("no retry"));
        assert_eq!(output.bytes, if flush_failure { frame(&response) } else { Vec::new() });
        assert_single_request(peer);
    }
}
