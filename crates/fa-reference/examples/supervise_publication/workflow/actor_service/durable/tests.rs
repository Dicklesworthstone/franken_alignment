//! Real original client/server owners; only scheduling and elapsed time are injected.
use super::*;
#[path = "../../../../../tests/support/file_oversight.rs"]
mod fixture;
use fixture::{Directory, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{Reconciliation, MAX_JOURNAL_BYTES};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{FileActorPort, FileActorSupervisor};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_transport::{UnixActorConnection, DriveBudget};
use fa_reference::action::consequence::oversight::actor_wire::{ActorChannel, ActorWire, ChannelLimits, encode_command};
use fa_reference::strict_json::{self, Json, Limits};
use std::{cell::Cell, fs, io, os::unix::net::UnixListener};

type Port = FileActorPort<FileOversight>;
fn actor(root: &Directory) -> Profile {
    let (socket, _) = UnixStream::pair().unwrap();
    let c = PeerCredentials::observe(&socket).unwrap();
    let p = PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap();
    Profile { request: 9000, scope: fixture::profile().delivery.scope, socket: root.0.join("actor.sock"),
        supervisor: p, actor: p, candidates: 8, connections: 8, exchanges: 128,
        runtime_ms: 1000, poll_ms: 1, reply_ms: 10 }
}
fn settings_json(root: &Directory) -> String {
    format!(concat!("{{\"schema\":\"fa.durable-actor-client/1\",\"directory\":\"{}\",",
        "\"server_identity\":\"independently-selected-test-server\",\"requests\":16,",
        "\"payload_bytes\":1048576,\"io_calls\":65536,\"write_bytes\":67108864,",
        "\"read_bytes\":67108864,\"image_bytes\":{},\"drive_steps\":16}}"),
        root.0.join("client").display(), MAX_JOURNAL_BYTES)
}
fn settings(root: &Directory, actor: &Profile) -> Settings { Settings::decode(settings_json(root).as_bytes(), actor).unwrap() }
fn actor_json(a: &Profile) -> String {
    let s = a.scope;
    format!(concat!("{{\"schema\":\"fa.actor-service/1\",\"clock\":\"unix_milliseconds\",",
        "\"request\":{},\"scope\":{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{}}},",
        "\"socket\":\"{}\",\"supervisor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
        "\"actor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},\"candidate_limit\":8,",
        "\"connection_limit\":8,\"exchange_limit\":128,\"runtime_ms\":1000,\"poll_ms\":1,\"reply_ms\":10}}"),
        a.request, s.tenant, s.principal, s.run, s.branch, s.authority, a.socket.display(),
        a.supervisor.uid(), a.supervisor.gid(), a.supervisor.pid().unwrap(),
        a.actor.uid(), a.actor.gid(), a.actor.pid().unwrap())
}
fn configure(root: &Directory, a: &Profile) {
    fs::write(root.0.join("actor.json"), actor_json(a)).unwrap();
    fs::write(root.0.join("client.json"), settings_json(root)).unwrap();
}
fn args(root: &Directory, mode: &str, rest: &[&str]) -> Vec<String> {
    let mut a = vec!["actor-submit".into(), root.0.join("actor.json").display().to_string(),
        "--durable".into(), root.0.join("client.json").display().to_string(), mode.into()];
    a.extend(rest.iter().map(|v| (*v).to_owned())); a
}
fn setup(root: &Directory) -> (Port, FileActorSupervisor<FileOversight>, FileHumanReviewer, Command) {
    let (mut h, reviewer) = fixture::create(root); h.enable_publication_guard(h.revision()).unwrap();
    let proposal = ActorProposal { target: h.inspect().target, payload: b"original publication".to_vec(),
        expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 };
    let (p, mut s) = h.into_actor_gateway();
    let r = s.host().unwrap().revision(); s.set_snapshot(r, Some(snapshot())).unwrap();
    (p, s, reviewer, Command::Submit { request: 9000, proposal })
}
fn server(socket: UnixStream, port: Port) -> UnixActorConnection<Port> {
    UnixActorConnection::new(socket, ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap()).unwrap()
}
fn publish(s: &mut FileActorSupervisor<FileOversight>, reviewer: &FileHumanReviewer) {
    let mut h = s.host_mut().unwrap(); let a = h.request_action(9000).unwrap().clone();
    let r = h.revision(); assert!(h.publish(r, 1).is_err()); assert_eq!(h.inspect().executions, 0);
    let views = fixture::inputs(&a, b"original complete committee evidence");
    fixture::review_existing(&mut h, 1, 101, &views);
    let r = h.revision(); let automatic = h.authorize(r, 1, &views, snapshot()).unwrap();
    let r = h.revision(); let request = h.request_human_approval(r, 1001, 1, &views, ElapsedTick(30)).unwrap();
    let r = h.revision(); let key = reviewer.approve(&mut h, r, &request).unwrap();
    let r = h.revision(); h.dispatch(r, &automatic, &key, &a, &views, snapshot()).unwrap();
    let r = h.revision(); h.publish_checked(r, 1, Some(&views), snapshot(), ElapsedTick(1)).unwrap();
}
fn reconcile(s: &mut FileActorSupervisor<FileOversight>) {
    let mut h = s.host_mut().unwrap(); let r = h.revision();
    assert!(matches!(h.reconcile(r, 1).unwrap(), Reconciliation::Resolved(_)));
}
fn json(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }

#[test]
fn runnable_exchange_keeps_two_key_publication_and_unknown_until_native_reconciliation() {
    for steps in [1, 16, 64] {
        let root = Directory::new(); let a = actor(&root); let mut config = settings(&root, &a);
        config.drive = fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget::new(steps).unwrap();
        let (port, mut supervisor, reviewer, cmd) = setup(&root);
        let state = FilePeerClient::create(&config.directory, config.profile.clone()).unwrap();
        let (socket, endpoint) = UnixStream::pair().unwrap(); let mut peer = server(endpoint, port);
        let mut client = state.connect(socket).unwrap(); let mut published = false; let mut saw_unknown = false;
        let mut waits = 0;
        let result = exchange(&mut client, &a, &config, Some(&cmd), &mut || Duration::ZERO, &mut |_| {
            waits += 1; assert!(waits < 64, "operator exceeded bounded original exchange");
            peer.drive(DriveBudget::default()).unwrap();
            if supervisor.host().unwrap().request_status(9000).is_ok() && !published {
                publish(&mut supervisor, &reviewer); published = true;
            }
            let cut = FilePeerClient::read(&config.directory, &config.profile, 0).unwrap();
            if !saw_unknown && matches!(cut.last_response(9000).map(|r| &r.result),
                Some(Ok(Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }))) {
                assert_eq!(supervisor.host().unwrap().inspect().control.ledger.charged, 16);
                saw_unknown = true; reconcile(&mut supervisor);
            }
        }).unwrap();
        assert!(saw_unknown);
        assert!(matches!(result.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
        let cut = FilePeerClient::read(&config.directory, &config.profile, client.revision()).unwrap();
        assert_eq!(cut.last_response(9000), Some(&result)); assert!(cut.reserved_work().is_none());
        assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
        assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    }
}

#[test]
fn command_timeout_then_document_free_resume_returns_original_execution_without_resend() {
    let root = Directory::new(); let a = actor(&root); let config = settings(&root, &a); configure(&root, &a);
    let (port, mut supervisor, reviewer, cmd) = setup(&root);
    let doc = root.0.join("submit.json"); fs::write(&doc, encode_command(&cmd).unwrap()).unwrap();
    let listener = UnixListener::bind(&a.socket).unwrap(); listener.set_nonblocking(true).unwrap();
    let expired = Cell::new(false); let mut active = None;
    let mut out = Vec::new();
    let result = command_with(&args(&root, "create", &[doc.to_str().unwrap()]), None,
        &mut || Duration::from_millis(if expired.get() { 1000 } else { 0 }), &mut |_| {
            let (socket, _) = listener.accept().unwrap(); let mut peer = server(socket, port.clone());
            for _ in 0..8 { peer.drive(DriveBudget::default()).unwrap(); }
            assert!(supervisor.host().unwrap().request_status(9000).is_ok());
            active = Some(peer); expired.set(true);
        }, &mut out);
    assert_eq!(result, Err(EXPIRED.into()));
    let value = json(&out); let object = value.as_object().unwrap();
    assert_eq!(object["status"].as_str(), Some("interrupted")); assert_eq!(object["recorded_response"], Json::Null);
    let cut = FilePeerClient::read(&config.directory, &config.profile, 0).unwrap();
    let floor = cut.revision(); assert!(cut.original_proposal(9000).is_some()); assert!(cut.last_response(9000).is_none());
    drop(active.take()); publish(&mut supervisor, &reviewer); reconcile(&mut supervisor);
    let before = supervisor.host().unwrap().inspect(); fs::remove_file(&doc).unwrap();
    let mut peer = None; let mut waits = 0; out.clear();
    command_with(&args(&root, "resume", &[&floor.to_string()]), None, &mut || Duration::ZERO, &mut |_| {
        waits += 1; assert!(waits < 32);
        if peer.is_none() { let (socket, _) = listener.accept().unwrap(); peer = Some(server(socket, port.clone())); }
        peer.as_mut().unwrap().drive(DriveBudget::default()).unwrap();
    }, &mut out).unwrap();
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
    let value = json(&out); assert_eq!(value.as_object().unwrap()["status"].as_str(), Some("observed"));
    let saved = FilePeerClient::read(&config.directory, &config.profile, floor).unwrap();
    assert!(matches!(saved.last_response(9000).unwrap().result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
}

#[test]
fn offline_inspection_uses_no_socket_or_cleanup_and_output_failure_preserves_the_record() {
    let root = Directory::new(); let mut a = actor(&root); a.socket = root.0.join("absent").join("actor.sock");
    configure(&root, &a); let config = settings(&root, &a);
    let (port, _s, _reviewer, cmd) = setup(&root); drop(port);
    let state = FilePeerClient::create(&config.directory, config.profile.clone()).unwrap();
    let (socket, _other) = UnixStream::pair().unwrap(); let mut client = state.connect(socket).unwrap();
    let Command::Submit { proposal, .. } = &cmd else { unreachable!() };
    client.submit(0, 9000, proposal).unwrap();
    let pending = config.directory.join("delivery.pending"); fs::write(&pending, b"retained staging").unwrap();
    let canonical = fs::read(config.directory.join("delivery.bin")).unwrap();
    let mut out = Vec::new();
    command_with(&args(&root, "inspect", &["1"]), None, &mut || Duration::ZERO,
        &mut |_| panic!("inspection does not schedule socket work"), &mut out).unwrap();
    let value = json(&out); let o = value.as_object().unwrap();
    assert_eq!(o["status"].as_str(), Some("inspection")); assert_eq!(o["recorded_response"], Json::Null);
    assert_eq!(o["request_retained"], Json::Bool(true)); assert_eq!(o["historical"], Json::Bool(true));
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
        fn flush(&mut self) -> io::Result<()> { panic!("write already refused") }
    }
    let error = command_with(&args(&root, "inspect", &["1"]), None, &mut || Duration::ZERO,
        &mut |_| panic!("no socket"), &mut Broken).unwrap_err();
    assert!(error.contains("no reconnect or resend"));
    assert_eq!(fs::read(config.directory.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(fs::read(&pending).unwrap(), b"retained staging");
    assert_eq!(client.revision(), 1);
}

#[test]
fn exact_domain_scope_peer_and_lifetime_limits_are_pinned_but_request_selection_is_not() {
    let root = Directory::new(); let a = actor(&root); let config = settings(&root, &a);
    let client = FilePeerClient::create(&config.directory, config.profile.clone()).unwrap(); drop(client);
    let pending = config.directory.join("delivery.pending"); fs::write(&pending, b"unacknowledged").unwrap();
    let before = fs::read(config.directory.join("delivery.bin")).unwrap();
    for field in 0..5 {
        let mut other = a.clone();
        match field {
            0 => other.scope.run += 1,
            1 => other.socket = root.0.join("different.sock"),
            2 => other.supervisor = PeerPolicy::new(a.supervisor.uid(), a.supervisor.gid(), None).unwrap(),
            3 => other.exchanges -= 1,
            _ => {},
        }
        let bytes = if field == 4 { settings_json(&root).replace("independently-selected-test-server", "other-domain") }
            else { settings_json(&root) };
        let wrong = Settings::decode(bytes.as_bytes(), &other).unwrap();
        assert!(FilePeerClient::open(&config.directory, wrong.profile, 0).is_err());
        assert_eq!(fs::read(&pending).unwrap(), b"unacknowledged");
    }
    let mut next = a.clone(); next.request += 1;
    assert_eq!(settings(&root, &next).profile, config.profile);
    assert_eq!(fs::read(config.directory.join("delivery.bin")).unwrap(), before);
    assert!(FilePeerClient::open(&config.directory, config.profile, 0).is_ok()); assert!(!pending.exists());
}

#[test]
fn closed_options_and_documents_refuse_before_storage_or_connection() {
    let root = Directory::new(); let a = actor(&root); configure(&root, &a);
    let mut wrong = args(&root, "resume", &["00"]);
    assert!(Invocation::parse(&wrong, None).is_err());
    wrong = args(&root, "create", &["/missing-submit", "--peers", "/missing-peer"]);
    assert_eq!(super::super::command(&wrong, None), Err(USAGE.into()));
    assert_eq!(crate::command({ let mut v = args(&root, "create", &["/missing-submit"]);
        v.extend(["--credibility-activation".into(), "/missing-capsule".into()]); v }),
        Err("credibility activation requires submit, submit-checked, proposal-next, serve-open, serve-open-checked or learned recovery".into()));
    let file = root.0.join("wrong.json"); fs::write(&file, br#"{"version":1,"op":"poll","request":"9000"}"#).unwrap();
    assert!(command_with(&args(&root, "create", &[file.to_str().unwrap()]), None, &mut || Duration::ZERO,
        &mut |_| panic!("invalid document cannot connect"), &mut Vec::new()).is_err());
    assert!(!root.0.join("client").exists());
    let valid = settings_json(&root);
    assert!(Settings::decode(valid.replace("\"drive_steps\":16", "\"drive_steps\":0").as_bytes(), &a).is_err());
    assert!(Settings::decode(valid.replace("\"requests\":16", "\"requests\":0").as_bytes(), &a).is_err());
    assert!(Settings::decode(valid.replace("\"drive_steps\":16", "\"drive_steps\":16,\"fallback\":true").as_bytes(), &a).is_err());
    assert!(!root.0.join("client").exists());
}

#[test]
fn expiry_before_prepare_or_before_io_retains_the_exact_difference() {
    use std::io::Read;
    for cutoff in [0, 1] {
        let root = Directory::new(); let a = actor(&root); let config = settings(&root, &a);
        let (_port, _s, _reviewer, cmd) = setup(&root);
        let state = FilePeerClient::create(&config.directory, config.profile.clone()).unwrap();
        let (socket, mut other) = UnixStream::pair().unwrap(); other.set_nonblocking(true).unwrap();
        let mut client = state.connect(socket).unwrap(); let calls = Cell::new(0);
        let error = exchange(&mut client, &a, &config, Some(&cmd), &mut || {
            let n = calls.get(); calls.set(n + 1); Duration::from_millis(if n >= cutoff { 1000 } else { 0 })
        }, &mut |_| panic!("deadline must precede socket work")).unwrap_err();
        assert_eq!(error, EXPIRED); assert_eq!(client.revision(), u64::from(cutoff != 0));
        assert_eq!(other.read(&mut [0; 1]).unwrap_err().kind(), io::ErrorKind::WouldBlock);
        let cut = FilePeerClient::read(&config.directory, &config.profile, 0).unwrap();
        assert_eq!(cut.original_proposal(9000).is_some(), cutoff != 0);
        assert!(cut.last_response(9000).is_none()); assert!(cut.reserved_work().is_none());
        assert_eq!(cut.work().calls, 0);
    }
}
