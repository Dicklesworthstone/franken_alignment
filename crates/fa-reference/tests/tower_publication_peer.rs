//! Real credential-bound sockets and the original file source/actor owner.
#![cfg(target_os = "linux")]

#[path = "support/file_source_intake.rs"]
mod fixture;

use std::io::{self, Read, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::requests::actor::source_wire::FileActorPeerDriveError;
use fa_reference::action::consequence::oversight::actor::{Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_peer::{
    PeerCredentials, PeerPolicy, PeerRefusal,
};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{
    ChannelLimits, ChannelState, CloseReason, WireError, WireResponse,
};
use fa_reference::action::consequence::oversight::policy_state::StateLimits;
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::tower::publication::peer::PublicationPeerSession;
use fa_reference::tower::publication::{PublicationLimits, PublicationTower};
use fa_reference::tower::{
    MAX_REQUEST_BYTES, ReadArguments, TowerAccess, TowerRequest, Verb, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use fixture::{FileRig, cold, driver};

fn credentials() -> PeerCredentials {
    let (socket, _peer) = UnixStream::pair().unwrap();
    PeerCredentials::observe(&socket).unwrap()
}

fn policy() -> PeerPolicy {
    let observed = credentials();
    PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid())).unwrap()
}

fn session(
    file: &FileRig,
    exchanges: u64,
    policy: PeerPolicy,
) -> (PublicationPeerSession, Address, Address) {
    let scope = driver::profile().delivery.scope;
    let mount = Address::new(
        scope.tenant.to_string(),
        Noun::Run,
        scope.run.to_string(),
        Some(41),
    )
    .unwrap();
    let tower = PublicationTower::new(
        mount.clone(),
        file.rig.port.clone(),
        PublicationLimits::default(),
    )
    .unwrap();
    let request = tower.request_address(1).unwrap();
    let access = TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        false,
        vec![PurposeTag::Permit],
        0,
    );
    let session = PublicationPeerSession::new(
        policy,
        tower,
        access,
        ChannelLimits {
            frame_bytes: MAX_REQUEST_BYTES,
            exchanges,
        },
        3,
    )
    .unwrap();
    (session, mount, request)
}

fn submit(file: &FileRig, mount: &Address, key: u64) -> Vec<u8> {
    let mut request = TowerRequest::new("socket-propose", mount.clone(), Verb::Propose);
    request.arguments = ReadArguments::Propose {
        request: key,
        proposal: file.rig.proposal(),
    };
    request.idempotency_key = Some(key.to_string());
    format!("{}\n", encode_request(&request).unwrap()).into_bytes()
}

fn attach(session: &mut PublicationPeerSession) -> UnixStream {
    let (server, client) = UnixStream::pair().unwrap();
    session.attach(server).unwrap();
    client.set_nonblocking(true).unwrap();
    client
}

fn replies(socket: &mut UnixStream) -> Vec<Json> {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        match socket.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                assert!(bytes.len() <= 64 * 1024);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("unexpected socket read failure: {error}"),
        }
    }
    assert_eq!(bytes.last(), Some(&b'\n'), "a complete reply is required");
    bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| {
            strict_json::parse(
                line,
                Limits {
                    max_bytes: 16 * 1024,
                    max_depth: 32,
                    max_items: 2048,
                    max_string_bytes: 16 * 1024,
                },
            )
            .unwrap()
        })
        .collect()
}

fn image(file: &FileRig) -> Vec<u8> {
    std::fs::read(file.rig.root.store().join("delivery.bin")).unwrap()
}

#[test]
fn pipelined_tower_frames_wait_for_reply_flush_and_only_new_submit_reads_source() {
    let mut file = cold(10, StateLimits::default());
    let (mut peer, mount, request) = session(&file, 4, policy());
    let mut socket = attach(&mut peer);
    let mut bytes = submit(&file, &mount, 1);
    bytes.extend_from_slice(
        format!(
            "{}\n",
            encode_request(&TowerRequest::new("socket-poll", request, Verb::Get)).unwrap()
        )
        .as_bytes(),
    );
    socket.write_all(&bytes).unwrap();
    let admitted = file
        .rig
        .driver
        .drive_tower_request_from_file(
            &mut peer,
            1,
            &mut file.source,
            || ElapsedTick(1),
            DriveBudget {
                frames: 1,
                write_bytes: 0,
                ..DriveBudget::default()
            },
        )
        .unwrap();
    assert_eq!(admitted.drive.progress.frames, 1);
    assert_eq!(admitted.intakes.len(), 1);
    assert!(admitted.intakes[0].result.is_ok());
    assert_eq!(admitted.drive.status.channel, ChannelState::ReplyReady);
    assert!(admitted.drive.status.buffered_input_bytes > 0);
    assert_eq!(file.source.status().read_attempts, 1);
    let accepted_image = image(&file);
    let blocked = file
        .rig
        .driver
        .drive_tower_request_from_file(
            &mut peer,
            1,
            &mut file.source,
            || panic!("pending output admitted another frame"),
            DriveBudget {
                write_bytes: 0,
                ..DriveBudget::default()
            },
        )
        .unwrap();
    assert_eq!(blocked.drive.progress.frames, 0);
    assert!(blocked.intakes.is_empty());
    let one_byte = file
        .rig
        .driver
        .drive_tower_request_observe(
            &mut peer,
            1,
            DriveBudget {
                write_bytes: 1,
                io_calls: 1,
                ..DriveBudget::default()
            },
        )
        .unwrap();
    assert_eq!(one_byte.progress.written_bytes, 1);
    assert_eq!(one_byte.progress.frames, 0);
    let observed = file
        .rig
        .driver
        .drive_tower_request_from_file(
            &mut peer,
            1,
            &mut file.source,
            || panic!("poll renewed source or time"),
            DriveBudget {
                frames: 1,
                ..DriveBudget::default()
            },
        )
        .unwrap();
    assert_eq!(observed.drive.progress.frames, 1);
    assert!(observed.intakes.is_empty());
    assert_eq!(image(&file), accepted_image);
    assert_eq!(file.source.status().read_attempts, 1);
    let replies = replies(&mut socket);
    assert_eq!(replies.len(), 2);
    for (reply, operation) in replies.iter().zip(["socket-propose", "socket-poll"]) {
        let envelope = reply.get("envelope").unwrap();
        assert_eq!(
            envelope.get("operation_id").and_then(Json::as_str),
            Some(operation)
        );
        assert_eq!(envelope.get("reason_code"), Some(&Json::Null));
    }
}

#[test]
fn lost_reply_reconnect_retains_exact_ticket_and_spent_exchange_allowance() {
    let mut file = cold(10, StateLimits::default());
    let (mut peer, mount, _) = session(&file, 2, policy());
    let bytes = submit(&file, &mount, 1);
    let mut socket = attach(&mut peer);
    socket.write_all(&bytes).unwrap();
    file.rig
        .driver
        .drive_tower_request_from_file(
            &mut peer,
            1,
            &mut file.source,
            || ElapsedTick(1),
            DriveBudget {
                frames: 1,
                write_bytes: 0,
                ..DriveBudget::default()
            },
        )
        .unwrap();
    let accepted_image = image(&file);
    assert!(peer.disconnect());
    drop(socket);
    assert_eq!(peer.remaining_exchanges(), 1);
    std::fs::remove_file(&file.path).unwrap();
    let mut socket = attach(&mut peer);
    socket.write_all(&bytes).unwrap();
    let retried = file
        .rig
        .driver
        .drive_tower_request_observe(&mut peer, 1, DriveBudget::default())
        .unwrap();
    assert_eq!(retried.progress.frames, 1);
    assert_eq!(
        retried.status.channel,
        ChannelState::Closed(CloseReason::ExchangeLimit)
    );
    assert_eq!(replies(&mut socket).len(), 1);
    assert_eq!(image(&file), accepted_image);
    assert_eq!(file.source.status().read_attempts, 1);
    assert_eq!(
        file.rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .retained_requests(),
        1
    );
    assert!(peer.disconnect());
    let (server, _client) = UnixStream::pair().unwrap();
    assert_eq!(peer.attach(server), Err(PeerRefusal::Capacity));
    assert_eq!(peer.status().connections_admitted, 2);
    assert_eq!(peer.remaining_exchanges(), 0);
    assert!(peer.revoke());
    assert!(!peer.revoke());
    assert_eq!(peer.drive(DriveBudget::default()), Err(WireError::Withheld));
}

#[test]
fn eof_fragment_and_unselected_proposal_never_acquire_source_or_use_old_snapshot() {
    let mut file = cold(10, StateLimits::default());
    let (mut peer, mount, _) = session(&file, 3, policy());
    let mut socket = attach(&mut peer);
    let bytes = submit(&file, &mount, 1);
    socket.write_all(&bytes[..bytes.len() - 1]).unwrap();
    socket.shutdown(Shutdown::Write).unwrap();
    let before = image(&file);
    let truncated = file
        .rig
        .driver
        .drive_tower_request_from_file(
            &mut peer,
            1,
            &mut file.source,
            || panic!("unterminated request acquired evidence"),
            DriveBudget::default(),
        )
        .unwrap();
    assert_eq!(truncated.drive.progress.frames, 0);
    assert_eq!(
        truncated.drive.status.channel,
        ChannelState::Closed(CloseReason::TruncatedFrame)
    );
    assert!(truncated.intakes.is_empty());
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status().read_attempts, 0);
    assert!(peer.disconnect());
    file.rig
        .driver
        .prepare_file_intake(&mut file.source, || ElapsedTick(1))
        .result
        .unwrap();
    let prepared_image = image(&file);
    let mut socket = attach(&mut peer);
    socket.write_all(&submit(&file, &mount, 2)).unwrap();
    let denied = file
        .rig
        .driver
        .drive_tower_request_from_file(
            &mut peer,
            1,
            &mut file.source,
            || panic!("unselected proposal acquired evidence"),
            DriveBudget::default(),
        )
        .unwrap();
    assert_eq!(denied.drive.progress.frames, 1);
    assert!(denied.intakes.is_empty());
    assert_eq!(image(&file), prepared_image);
    assert_eq!(file.source.status().read_attempts, 1);
    assert_eq!(
        file.rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .retained_requests(),
        0
    );
    assert_eq!(
        replies(&mut socket)[0]
            .get("envelope")
            .unwrap()
            .get("reason_code")
            .and_then(Json::as_str),
        Some("DeniedByPolicy")
    );
    // The ordinary native port can still use the explicitly prepared snapshot.
    assert!(file.rig.port.submit(1, &file.rig.proposal()).is_ok());
}

#[test]
fn credentials_and_original_owner_identity_gate_buffered_frames_before_work() {
    let mut first = cold(10, StateLimits::default());
    let second = cold(10, StateLimits::default());
    let observed = credentials();
    let wrong_uid = if observed.uid() == 0 { 1 } else { 0 };
    let denied_policy = PeerPolicy::new(wrong_uid, observed.gid(), None).unwrap();
    let (mut denied, _, _) = session(&first, 2, denied_policy);
    let (server, mut client) = UnixStream::pair().unwrap();
    client.write_all(b"{}\n").unwrap();
    assert_eq!(denied.attach(server), Err(PeerRefusal::CredentialsRejected));
    assert_eq!(denied.status().connections_admitted, 0);
    assert_eq!(denied.remaining_exchanges(), 2);
    assert_eq!(
        denied.drive(DriveBudget::default()),
        Err(WireError::Unavailable)
    );

    let (mut foreign, mount, _) = session(&second, 2, policy());
    let mut socket = attach(&mut foreign);
    socket.write_all(&submit(&second, &mount, 1)).unwrap();
    let first_image = image(&first);
    let second_image = image(&second);
    let result = first.rig.driver.drive_tower_request_from_file(
        &mut foreign,
        1,
        &mut first.source,
        || panic!("same-scope foreign owner acquired source"),
        DriveBudget::default(),
    );
    assert!(matches!(
        result,
        Err(FileActorPeerDriveError::Journal(JournalError::Contract(
            Error::Binding
        )))
    ));
    assert_eq!(foreign.remaining_exchanges(), 2);
    assert_eq!(foreign.status().transport.unwrap().buffered_input_bytes, 0);
    assert_eq!(image(&first), first_image);
    assert_eq!(image(&second), second_image);
    assert_eq!(first.source.status().read_attempts, 0);
}

#[test]
fn retained_peer_ticket_reports_original_owner_loss_after_journal_reopen() {
    let mut file = cold(10, StateLimits::default());
    let (mut peer, mount, request) = session(&file, 3, policy());
    let mut socket = attach(&mut peer);
    socket.write_all(&submit(&file, &mount, 1)).unwrap();
    let admitted = file
        .rig
        .driver
        .drive_tower_request_from_file(
            &mut peer,
            1,
            &mut file.source,
            || ElapsedTick(1),
            DriveBudget::default(),
        )
        .unwrap();
    assert_eq!(admitted.drive.progress.frames, 1);
    assert_eq!(admitted.intakes.len(), 1);
    assert!(admitted.intakes[0].result.is_ok());
    assert_eq!(file.source.status().read_attempts, 1);
    let admitted_reply = replies(&mut socket);
    assert_eq!(admitted_reply.len(), 1);
    assert_eq!(
        admitted_reply[0]
            .get("envelope")
            .unwrap()
            .get("reason_code"),
        Some(&Json::Null)
    );

    let FileRig { rig, source, path } = file;
    let driver::Rig {
        root,
        port: retained_port,
        driver: original_driver,
        reviewer,
        clients,
        inputs,
    } = rig;
    drop((original_driver, reviewer, clients, inputs, source, path));

    // The same peer, original ticket and weak native port remain alive. Opening
    // the durable owner again proves none of them retained its exclusive lock.
    let (recovered, _reviewer) = FileOversight::open(root.store(), driver::profile()).unwrap();
    assert_eq!(recovered.retained_requests(), 1);
    let recovered_image = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let poll = TowerRequest::new("owner-loss-poll", request, Verb::Get);
    socket
        .write_all(format!("{}\n", encode_request(&poll).unwrap()).as_bytes())
        .unwrap();
    let observed = peer.drive(DriveBudget::default()).unwrap();
    assert_eq!(observed.progress.frames, 1);
    assert_eq!(peer.remaining_exchanges(), 1);
    let observed_reply = replies(&mut socket);
    assert_eq!(observed_reply.len(), 1);
    let envelope = observed_reply[0].get("envelope").unwrap();
    assert_eq!(
        envelope.get("operation_id").and_then(Json::as_str),
        Some("owner-loss-poll")
    );
    assert_eq!(
        envelope.get("reason_code").and_then(Json::as_str),
        Some("LowerLayerUnverified")
    );
    let result = observed_reply[0].get("result").unwrap();
    assert_eq!(
        result.get("kind").and_then(Json::as_str),
        Some("publication-request")
    );
    let expected = WireResponse {
        request: Some(1),
        result: Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        }),
    };
    let expected = strict_json::parse(
        &expected.encode(),
        Limits {
            max_bytes: 16 * 1024,
            max_depth: 32,
            max_items: 2048,
            max_string_bytes: 16 * 1024,
        },
    )
    .unwrap();
    assert_eq!(result.get("value"), Some(&expected));
    assert_eq!(
        std::fs::read(root.store().join("delivery.bin")).unwrap(),
        recovered_image
    );
    assert_eq!(recovered.inspect().executions, 0);
    // Keep the extra original port through recovery and observation as well.
    drop(retained_port);
}

#[path = "tower_publication_peer/sequence.rs"]
mod sequence;
