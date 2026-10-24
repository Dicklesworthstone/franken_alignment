//! Typed client intents through the original stream gate and actual Unix sockets.
//! Scheduling is deterministic; helper verdict fixtures do not qualify a model.
use super::*;
#[path = "../../../../../tests/support/file_stream_wire.rs"]
mod fixture;
use fixture::{Directory, snapshot};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
fn directory() -> Directory {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    Directory(crate::test_directory::create(format!(
        "fa-durable-peer-{}-{stamp}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )))
}
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::FileClientProfile;
use fa_reference::action::consequence::delivery::persistent::observed::stream::{
    FileStreamProposal,
    actor_wire::{encode_stream_proposal, validate_stream_proposal},
};
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversight,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorSupervisor;
use fa_reference::action::consequence::delivery::persistent::{MAX_JOURNAL_BYTES, Reconciliation};
use fa_reference::action::consequence::delivery::stream::{StreamProfile, StreamView};
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal};
use fa_reference::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy};
use fa_reference::action::consequence::oversight::actor_transport::{
    DriveBudget, UnixActorConnection,
};
use fa_reference::action::consequence::oversight::actor_wire::client::drive::ClientDriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::client::{
    ClientIoLimits, ClientSessionLimits,
};
use fa_reference::action::consequence::oversight::actor_wire::{
    ActorChannel, ActorRequestPort, ActorWire, ChannelLimits, encode_command,
};
use std::{cell::Cell, fs, os::unix::net::UnixListener};

type Supervisor = FileActorSupervisor<FileOversight>;
fn actor(root: &Directory) -> Profile {
    let (socket, _) = UnixStream::pair().unwrap();
    let c = PeerCredentials::observe(&socket).unwrap();
    let peer = PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap();
    Profile {
        request: 7,
        scope: fixture::profile().delivery.scope,
        socket: root.0.join("actor.sock"),
        supervisor: peer,
        actor: peer,
        candidates: 8,
        connections: 8,
        exchanges: 128,
        runtime_ms: 1000,
        poll_ms: 1,
        reply_ms: 10,
    }
}
fn raw_settings(root: &Directory) -> String {
    format!(
        concat!(
            "{{\"schema\":\"fa.durable-actor-client/1\",\"directory\":\"{}\",",
            "\"server_identity\":\"independent-stream-test-domain\",\"requests\":16,",
            "\"payload_bytes\":1048576,\"io_calls\":65536,\"write_bytes\":67108864,",
            "\"read_bytes\":67108864,\"image_bytes\":{},\"drive_steps\":16}}"
        ),
        root.0.join("client").display(),
        MAX_JOURNAL_BYTES
    )
}
fn stream_settings(root: &Directory) -> String {
    raw_settings(root).replace("fa.durable-actor-client/1", "fa.durable-actor-client/2")
        .replacen("{", "{\"stream\":{\"id\":7,\"generation\":1,\"max_messages\":4,\"max_message_bytes\":1024,\"max_stream_bytes\":4096},", 1)
}
fn configured(root: &Directory, actor: &Profile) -> Settings {
    Settings::decode(stream_settings(root).as_bytes(), actor).unwrap()
}
fn configure(root: &Directory, a: &Profile) {
    let s = a.scope;
    let bytes = format!(
        concat!(
            "{{\"schema\":\"fa.actor-service/1\",\"clock\":\"unix_milliseconds\",",
            "\"request\":{},\"scope\":{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{}}},",
            "\"socket\":\"{}\",\"supervisor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
            "\"actor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},\"candidate_limit\":8,",
            "\"connection_limit\":8,\"exchange_limit\":128,\"runtime_ms\":1000,\"poll_ms\":1,\"reply_ms\":10}}"
        ),
        a.request,
        s.tenant,
        s.principal,
        s.run,
        s.branch,
        s.authority,
        a.socket.display(),
        a.supervisor.uid(),
        a.supervisor.gid(),
        a.supervisor.pid().unwrap(),
        a.actor.uid(),
        a.actor.gid(),
        a.actor.pid().unwrap()
    );
    fs::write(root.0.join("actor.json"), bytes).unwrap();
    fs::write(root.0.join("client.json"), stream_settings(root)).unwrap();
}
fn args(root: &Directory, mode: &str, rest: &[&str]) -> Vec<String> {
    let mut args = vec![
        "actor-submit".into(),
        root.0.join("actor.json").display().to_string(),
        "--durable".into(),
        root.0.join("client.json").display().to_string(),
        mode.into(),
    ];
    args.extend(rest.iter().map(|s| (*s).to_owned()));
    args
}
fn transport<P: ActorRequestPort>(socket: UnixStream, port: P) -> UnixActorConnection<P> {
    UnixActorConnection::new(
        socket,
        ActorChannel::new(ActorWire::new(port), ChannelLimits::default()).unwrap(),
    )
    .unwrap()
}
fn proposal(message: Option<&str>) -> ActorProposal {
    encode_stream_proposal(
        fixture::stream(),
        &FileStreamProposal {
            target: fixture::profile().delivery.target,
            expected_policy_epoch: 0,
            deadline: ElapsedTick(101),
            message: message.map(str::to_owned),
        },
    )
    .unwrap()
}
fn publish(s: &mut Supervisor, human: &FileHumanReviewer, request: u64, attempt: u64) {
    let mut h = s.host_mut().unwrap();
    let keys = fixture::approve(&mut h, human, request, attempt);
    let r = h.revision();
    assert!(
        h.publish(r, attempt).is_err(),
        "no effect before both keys and dispatch"
    );
    fixture::ordinary::dispatch(&mut h, &keys);
    let r = h.revision();
    h.publish_checked(r, attempt, Some(&keys.inputs), snapshot(), ElapsedTick(1))
        .unwrap();
}
fn reconcile(s: &mut Supervisor, attempt: u64) {
    let mut h = s.host_mut().unwrap();
    let r = h.revision();
    assert!(matches!(
        h.reconcile(r, attempt).unwrap(),
        Reconciliation::Resolved(_)
    ));
}

#[test]
fn stream_contract_pins_every_field_and_preserves_the_original_raw_binding() {
    let root = directory();
    let a = actor(&root);
    let raw = Settings::decode(raw_settings(&root).as_bytes(), &a).unwrap();
    assert_eq!(raw.stream, None);
    // Independent original v1 binding vector, not another call to Settings.
    let mut binding = b"FAOPCLIENT\0raw-submit-v1\0".to_vec();
    let s = a.scope;
    for n in [
        s.tenant,
        s.principal,
        s.run,
        s.branch,
        s.authority,
        crate::config::CLOCK_DOMAIN,
    ] {
        binding.extend_from_slice(&n.to_be_bytes());
    }
    for part in [
        b"independent-stream-test-domain".as_slice(),
        a.socket.as_os_str().as_encoded_bytes(),
    ] {
        binding.extend_from_slice(&(part.len() as u32).to_be_bytes());
        binding.extend_from_slice(part);
    }
    let limits = ClientSessionLimits {
        requests: 16,
        retained_payload_bytes: 1048576,
        io: ClientIoLimits {
            exchanges: 128,
            calls: 65536,
            written_bytes: 67108864,
            read_bytes: 67108864,
        },
    };
    let original = FileClientProfile::new(binding, limits, MAX_JOURNAL_BYTES)
        .unwrap()
        .with_peer_policy(a.supervisor)
        .unwrap();
    assert_eq!(raw.profile, original);
    let c = configured(&root, &a);
    assert_eq!(c.stream, Some(fixture::stream()));
    assert_ne!(c.profile, raw.profile);
    drop(FilePeerClient::create(&c.directory, c.profile.clone()).unwrap());
    let canonical = fs::read(c.directory.join("delivery.bin")).unwrap();
    let pending = c.directory.join("delivery.pending");
    fs::write(&pending, b"retained staging").unwrap();
    let valid = stream_settings(&root);
    for (old, new) in [
        ("\"id\":7", "\"id\":8"),
        ("\"generation\":1", "\"generation\":2"),
        ("\"max_messages\":4", "\"max_messages\":5"),
        ("\"max_message_bytes\":1024", "\"max_message_bytes\":512"),
        ("\"max_stream_bytes\":4096", "\"max_stream_bytes\":2048"),
    ] {
        let wrong = Settings::decode(valid.replace(old, new).as_bytes(), &a).unwrap();
        assert_ne!(wrong.profile, c.profile);
        assert!(FilePeerClient::open(&c.directory, wrong.profile, 0).is_err());
        assert_eq!(fs::read(&pending).unwrap(), b"retained staging");
    }
    assert!(FilePeerClient::open(&c.directory, raw.profile, 0).is_err());
    assert_eq!(
        fs::read(c.directory.join("delivery.bin")).unwrap(),
        canonical
    );
    let mut next = a.clone();
    next.request += 1;
    assert_eq!(
        configured(&root, &next).profile,
        c.profile,
        "request selection is not a new server identity"
    );
    drop(FilePeerClient::open(&c.directory, c.profile, 0).unwrap());
    assert!(!pending.exists());
}

#[test]
fn malformed_intents_and_cumulative_frames_refuse_before_client_creation_or_connection() {
    let root = directory();
    let a = actor(&root);
    configure(&root, &a);
    let settings = configured(&root, &a);
    let valid = proposal(Some("hello"));
    let finish = proposal(None);
    validate_stream_proposal(fixture::stream(), &valid).unwrap();
    settings.validate_proposal(&finish).unwrap();
    let mut bad = Vec::new();
    let mut raw = valid.clone();
    raw.payload = b"raw effect".to_vec();
    raw.units = raw.payload.len() as u64;
    bad.push(raw);
    let mut cumulative = valid.clone();
    cumulative.payload = StreamView::empty(fixture::stream())
        .encode_message("hello")
        .unwrap();
    cumulative.units = cumulative.payload.len() as u64;
    bad.push(cumulative);
    let mut units = valid.clone();
    units.units += 1;
    bad.push(units);
    let mut utf8 = valid.clone();
    *utf8.payload.last_mut().unwrap() = 255;
    bad.push(utf8);
    let mut truncated = valid.clone();
    truncated.payload.pop();
    truncated.units -= 1;
    bad.push(truncated);
    let mut false_finish = valid.clone();
    false_finish.payload[36] = 1;
    bad.push(false_finish);
    let other = StreamProfile::new(7, 2, 4, 1024, 4096).unwrap();
    bad.push(
        encode_stream_proposal(
            other,
            &FileStreamProposal {
                target: valid.target,
                expected_policy_epoch: 0,
                deadline: valid.deadline,
                message: Some("hello".into()),
            },
        )
        .unwrap(),
    );
    let file = root.0.join("intent.json");
    for proposal in bad {
        assert!(settings.validate_proposal(&proposal).is_err());
        fs::write(
            &file,
            encode_command(&Command::Submit {
                request: a.request,
                proposal,
            })
            .unwrap(),
        )
        .unwrap();
        assert!(
            command_with(
                &args(&root, "create", &[file.to_str().unwrap()]),
                None,
                &mut || Duration::ZERO,
                &mut |_| panic!("invalid intent must not schedule I/O"),
                &mut Vec::new()
            )
            .is_err()
        );
        assert!(
            !settings.directory.exists(),
            "invalid input created durable request custody"
        );
        assert!(!a.socket.exists());
    }
}

#[test]
fn typed_client_runs_two_messages_and_explicit_finish_with_original_review_and_reconciliation() {
    for steps in [1, 16, 64] {
        let root = directory();
        let mut a = actor(&root);
        let mut c = configured(&root, &a);
        c.drive = ClientDriveBudget::new(steps).unwrap();
        let (port, mut s, human) = fixture::create(&root);
        let state = FilePeerClient::create(&c.directory, c.profile.clone()).unwrap();
        let (socket, other) = UnixStream::pair().unwrap();
        let mut client = state.connect(socket).unwrap();
        let mut peer = transport(other, port);
        let mut full_frame_charges = 0;
        for (request, attempt, message) in [
            (7, 1, Some("First 🌎")),
            (8, 2, Some("Second é")),
            (9, 3, None),
        ] {
            a.request = request;
            fixture::admit(&mut s);
            let cmd = fixture::command(&s.host().unwrap(), request, message);
            let mut published = false;
            let mut saw_unknown = false;
            let mut waits = 0;
            let result = exchange(
                &mut client,
                &a,
                &c,
                Some(&cmd),
                &mut || Duration::ZERO,
                &mut |_| {
                    waits += 1;
                    assert!(waits < 64);
                    peer.drive(DriveBudget::default()).unwrap();
                    if s.host().unwrap().request_status(request).is_ok() && !published {
                        full_frame_charges += s
                            .host()
                            .unwrap()
                            .request_action(request)
                            .unwrap()
                            .spec()
                            .units;
                        publish(&mut s, &human, request, attempt);
                        published = true;
                    }
                    let cut = FilePeerClient::read(&c.directory, &c.profile, 0).unwrap();
                    if !saw_unknown
                        && matches!(
                            cut.last_response(request).map(|r| &r.result),
                            Some(Ok(Knowledge::Unknown {
                                reason: UnknownReason::OutcomeUnknown
                            }))
                        )
                    {
                        saw_unknown = true;
                        assert_eq!(
                            s.host().unwrap().inspect().control.ledger.charged,
                            full_frame_charges
                        );
                        reconcile(&mut s, attempt);
                    }
                },
            )
            .unwrap();
            assert!(saw_unknown);
            assert!(matches!(
                result.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
            assert_eq!(
                s.host()
                    .unwrap()
                    .stream_snapshot()
                    .unwrap()
                    .confirmed
                    .finished(),
                message.is_none()
            );
        }
        let state = s.host().unwrap().stream_snapshot().unwrap();
        assert_eq!(
            state.confirmed.messages().collect::<Vec<_>>(),
            vec!["First 🌎", "Second é"]
        );
        assert!(state.confirmed.finished());
        assert_eq!(state.publication.executions, 3);
        assert_eq!(state.publication.control.ledger.charged, full_frame_charges);
        assert_eq!(client.state().unwrap().request_count(), 3);
    }
}

#[test]
fn document_free_stream_resume_recovers_an_old_intent_after_a_later_message() {
    let root = directory();
    let a = actor(&root);
    configure(&root, &a);
    let c = configured(&root, &a);
    let (port, mut s, human) = fixture::create(&root);
    fixture::admit(&mut s);
    let cmd = fixture::command(&s.host().unwrap(), 7, Some("First 🌎"));
    let file = root.0.join("intent.json");
    fs::write(&file, encode_command(&cmd).unwrap()).unwrap();
    let listener = UnixListener::bind(&a.socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let expired = Cell::new(false);
    let mut first_peer = None;
    let mut output = Vec::new();
    let result = command_with(
        &args(&root, "create", &[file.to_str().unwrap()]),
        None,
        &mut || Duration::from_millis(if expired.get() { 1000 } else { 0 }),
        &mut |_| {
            let (socket, _) = listener.accept().unwrap();
            let mut peer = transport(socket, port.clone());
            for _ in 0..8 {
                peer.drive(DriveBudget::default()).unwrap();
            }
            assert!(s.host().unwrap().request_status(7).is_ok());
            first_peer = Some(peer);
            expired.set(true);
        },
        &mut output,
    );
    assert_eq!(result, Err(EXPIRED.into()));
    drop(first_peer);
    let cut = FilePeerClient::read(&c.directory, &c.profile, 0).unwrap();
    let floor = cut.revision();
    assert!(cut.last_response(7).is_none());
    assert!(cut.original_proposal(7).is_some());
    publish(&mut s, &human, 7, 1);
    reconcile(&mut s, 1);
    let Command::Submit { proposal: next, .. } =
        fixture::command(&s.host().unwrap(), 8, Some("Later"))
    else {
        unreachable!()
    };
    fixture::admit(&mut s);
    port.submit(8, &next).unwrap();
    publish(&mut s, &human, 8, 2);
    reconcile(&mut s, 2);
    let before = s.host().unwrap().stream_snapshot().unwrap();
    fs::remove_file(&file).unwrap();
    let mut peer = None;
    let mut waits = 0;
    output.clear();
    command_with(
        &args(&root, "resume", &[&floor.to_string()]),
        None,
        &mut || Duration::ZERO,
        &mut |_| {
            waits += 1;
            assert!(waits < 32);
            if peer.is_none() {
                let (socket, _) = listener.accept().unwrap();
                peer = Some(transport(
                    socket,
                    port.clone().recovery_only().for_request(7).unwrap(),
                ));
            }
            peer.as_mut()
                .unwrap()
                .drive(DriveBudget::default())
                .unwrap();
        },
        &mut output,
    )
    .unwrap();
    assert!(!file.exists());
    assert_eq!(s.host().unwrap().stream_snapshot().unwrap(), before);
    assert_eq!(before.publication.executions, 2);
    assert_eq!(s.host().unwrap().retained_requests(), 2);
    let cut = FilePeerClient::read(&c.directory, &c.profile, floor).unwrap();
    assert!(matches!(
        cut.last_response(7).map(|r| &r.result),
        Some(Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }))
    ));
    let Command::Submit {
        proposal: original, ..
    } = cmd
    else {
        unreachable!()
    };
    assert_eq!(cut.original_proposal(7), Some(&original));
}

#[test]
fn saved_non_intent_is_rejected_before_cleanup_or_transmission_by_every_recovery_mode() {
    let root = directory();
    let a = actor(&root);
    configure(&root, &a);
    let c = configured(&root, &a);
    // Valid generic client history, deliberately not valid for the selected
    // operator stream profile. No fake server response is needed for this case.
    let state = FilePeerClient::create(&c.directory, c.profile.clone()).unwrap();
    let (socket, _) = UnixStream::pair().unwrap();
    let mut client = state.connect(socket).unwrap();
    let mut raw = proposal(Some("hello"));
    raw.payload = b"not an intent".to_vec();
    raw.units = raw.payload.len() as u64;
    client.submit(0, 7, &raw).unwrap();
    drop(client);
    let canonical = fs::read(c.directory.join("delivery.bin")).unwrap();
    let pending = c.directory.join("delivery.pending");
    fs::write(&pending, b"do not clean").unwrap();
    let file = root.0.join("intent.json");
    fs::write(
        &file,
        encode_command(&Command::Submit {
            request: 7,
            proposal: proposal(Some("hello")),
        })
        .unwrap(),
    )
    .unwrap();
    for mode in ["inspect", "resume", "cancel", "submit"] {
        let rest = if mode == "submit" {
            vec!["1", file.to_str().unwrap()]
        } else {
            vec!["1"]
        };
        let error = command_with(
            &args(&root, mode, &rest),
            None,
            &mut || Duration::ZERO,
            &mut |_| panic!("no socket work for an invalid retained intent"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(error.contains("MalformedProposal"));
        assert_eq!(
            fs::read(c.directory.join("delivery.bin")).unwrap(),
            canonical
        );
        assert_eq!(fs::read(&pending).unwrap(), b"do not clean");
        assert!(!a.socket.exists());
    }
}

#[test]
fn stream_schema_requires_complete_contract_without_implicit_raw_fallback() {
    let root = directory();
    let a = actor(&root);
    let valid = stream_settings(&root);
    assert!(Settings::decode(valid.as_bytes(), &a).is_ok());
    let raw = raw_settings(&root);
    assert!(Settings::decode(raw.as_bytes(), &a).is_ok());
    for bad in [
        valid.replace("fa.durable-actor-client/2", "fa.durable-actor-client/1"),
        raw.replace("fa.durable-actor-client/1", "fa.durable-actor-client/2"),
        valid.replace("\"max_messages\":4", "\"max_messages\":0"),
        valid.replace("\"generation\":1", "\"generation\":0"),
        valid.replace(
            "\"max_stream_bytes\":4096",
            "\"max_stream_bytes\":4096,\"fallback\":true",
        ),
        valid.replace("\"id\":7", "\"id\":7,\"id\":8"),
        valid.replace("\"max_message_bytes\":1024,", ""),
    ] {
        assert!(Settings::decode(bad.as_bytes(), &a).is_err());
    }
    assert!(!root.0.join("client").exists());
}

#[test]
fn stream_cancellation_keeps_unknown_dispatch_charged_and_never_finishes_the_stream() {
    for dispatched in [false, true] {
        let root = directory();
        let a = actor(&root);
        configure(&root, &a);
        let c = configured(&root, &a);
        let (port, mut s, human) = fixture::create(&root);
        fixture::admit(&mut s);
        let original = proposal(Some("hello"));
        let state = FilePeerClient::create(&c.directory, c.profile.clone()).unwrap();
        let (socket, other) = UnixStream::pair().unwrap();
        let mut client = state.connect(socket).unwrap();
        let mut first_peer = transport(other, port.clone());
        client.submit(0, 7, &original).unwrap();
        let mut acknowledged = false;
        for _ in 0..16 {
            let r = client.revision();
            let progress = client.drive(r, c.drive).unwrap();
            first_peer.drive(DriveBudget::default()).unwrap();
            if let ClientProgress::Response(response) = progress.progress {
                assert!(matches!(response.result, Ok(Knowledge::Pending { .. })));
                acknowledged = true;
                break;
            }
        }
        assert!(acknowledged);
        let floor = client.revision();
        drop(client);
        drop(first_peer);
        let keys = {
            let mut h = s.host_mut().unwrap();
            fixture::approve(&mut h, &human, 7, 1)
        };
        if dispatched {
            let mut h = s.host_mut().unwrap();
            fixture::ordinary::dispatch(&mut h, &keys);
        }
        let listener = UnixListener::bind(&a.socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut peer = None;
        let mut waits = 0;
        let result = command_with(
            &args(&root, "cancel", &[&floor.to_string()]),
            None,
            &mut || Duration::ZERO,
            &mut |_| {
                waits += 1;
                assert!(waits < 32);
                if peer.is_none() {
                    let (socket, _) = listener.accept().unwrap();
                    peer = Some(transport(socket, port.clone().recovery_only()));
                }
                peer.as_mut()
                    .unwrap()
                    .drive(DriveBudget::default())
                    .unwrap();
            },
            &mut Vec::new(),
        );
        let state = s.host().unwrap().stream_snapshot().unwrap();
        assert!(!state.confirmed.finished());
        assert!(!state.published.finished());
        assert_eq!(state.publication.executions, 0);
        assert_eq!(state.publication.control.ledger.reserved, 0);
        let saved = FilePeerClient::read(&c.directory, &c.profile, floor).unwrap();
        if dispatched {
            assert!(result.is_err());
            assert_eq!(
                state.publication.control.ledger.charged,
                keys.action.spec().units
            );
            assert_eq!(
                saved.last_response(7).unwrap().result,
                Ok(Knowledge::Unknown {
                    reason: UnknownReason::OutcomeUnknown
                })
            );
        } else {
            result.unwrap();
            assert_eq!(state.publication.control.ledger.charged, 0);
            assert!(matches!(
                saved.last_response(7).map(|r| &r.result),
                Some(Ok(Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch,
                    ..
                }))
            ));
        }
    }
}
