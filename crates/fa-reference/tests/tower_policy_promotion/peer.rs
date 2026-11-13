//! Real SO_PEERCRED admission and cumulative reconnect accounting for Promote.
use super::*;
use fa_reference::action::consequence::oversight::actor_peer::{
    PeerCredentials, PeerPolicy, PeerRefusal,
};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::ChannelLimits;
use fa_reference::tower::MAX_REQUEST_BYTES;
use fa_reference::tower::publication::peer::PublicationPeerSession;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

#[path = "dynamic_install.rs"]
mod dynamic_install;

fn credentials() -> PeerCredentials {
    let (socket, _peer) = UnixStream::pair().unwrap();
    PeerCredentials::observe(&socket).unwrap()
}

fn session(facade: PublicationTower, policy: PeerPolicy, exchanges: u64) -> PublicationPeerSession {
    PublicationPeerSession::new(
        policy,
        facade,
        access(true, true),
        ChannelLimits {
            frame_bytes: MAX_REQUEST_BYTES,
            exchanges,
        },
        2,
    )
    .unwrap()
}

fn attach(peer: &mut PublicationPeerSession) -> UnixStream {
    let (server, client) = UnixStream::pair().unwrap();
    peer.attach(server).unwrap();
    client.set_nonblocking(true).unwrap();
    client
}

fn frame(request: &TowerRequest) -> Vec<u8> {
    format!("{}\n", encode_request(request).unwrap()).into_bytes()
}

fn exchange(
    peer: &mut PublicationPeerSession,
    socket: &mut UnixStream,
    request: &TowerRequest,
) -> Json {
    socket.write_all(&frame(request)).unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    for _ in 0..64 {
        peer.drive(DriveBudget::default()).unwrap();
        loop {
            match socket.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("operator socket read failed: {error}"),
            }
            assert!(bytes.len() <= 64 * 1024);
        }
        if bytes.last() == Some(&b'\n') {
            break;
        }
    }
    assert_eq!(
        bytes.last(),
        Some(&b'\n'),
        "complete bounded reply required"
    );
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    strict_json::parse(&bytes, KNOWLEDGE_JSON_LIMITS).unwrap()
}

#[test]
fn authenticated_promote_lost_reply_reconnects_to_the_same_one_use_acknowledgment() {
    let root = Directory::new();
    let (actor, mut supervisor, _, governor, _) = guarded(&root);
    let (key, _) = approve(&mut supervisor, &governor, same_policy(2));
    let (mut facade, grant) = granted(actor.clone(), &supervisor, key);
    let original = image(&root);
    let selected = next(&mut facade, OPERATION);
    let command = action(&selected);
    let identity = credentials();
    let policy = PeerPolicy::new(identity.uid(), identity.gid(), Some(identity.pid())).unwrap();
    let mut peer = session(facade, policy, 3);
    let mut socket = attach(&mut peer);
    let observed = exchange(&mut peer, &mut socket, &query(OPERATION));
    assert_eq!(
        field(field(&observed, "envelope"), "reason_code"),
        &Json::Null
    );
    let returned_action = field(
        field(field(field(&observed, "result"), "value"), "knowledge"),
        "value",
    );
    assert_eq!(
        field(returned_action, "action"),
        &strict_json::parse(
            encode_request(&command).unwrap().as_bytes(),
            KNOWLEDGE_JSON_LIMITS
        )
        .unwrap()
    );
    assert_eq!(image(&root), original);
    assert_eq!(
        (grant.remaining_queries(), grant.remaining_operations()),
        (30, 1)
    );
    socket.write_all(&frame(&command)).unwrap();
    let entered = peer
        .drive(DriveBudget {
            frames: 1,
            write_bytes: 0,
            ..DriveBudget::default()
        })
        .unwrap();
    assert_eq!(entered.progress.frames, 1);
    assert_eq!(grant.remaining_operations(), 0);
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .current_policy()
            .unwrap()
            .generation(),
        2
    );
    assert_eq!(peer.remaining_exchanges(), 1);
    assert!(peer.disconnect());
    drop(socket);
    // A newly admitted original request is not discarded by a retry of the
    // acknowledged policy change, including after the old reply was lost.
    let proposal = {
        let host = supervisor.host().unwrap();
        support::proposal(&oversight::spec(&host, b"after promotion"))
    };
    observe(&mut supervisor);
    let ticket = actor.submit(701, &proposal).unwrap();
    let saved = image(&root);
    let before_retry = actor.poll(&ticket);
    let mut socket = attach(&mut peer);
    let retried = exchange(&mut peer, &mut socket, &command);
    assert_eq!(
        field(field(&retried, "envelope"), "reason_code"),
        &Json::Null
    );
    assert_eq!(image(&root), saved);
    assert_eq!(actor.poll(&ticket), before_retry);
    assert_eq!(peer.remaining_exchanges(), 0);
    assert_eq!(peer.status().connections_admitted, 2);
    assert_eq!(
        (grant.remaining_queries(), grant.remaining_operations()),
        (30, 0)
    );
    assert!(peer.disconnect());
    let (server, _client) = UnixStream::pair().unwrap();
    assert_eq!(peer.attach(server), Err(PeerRefusal::Capacity));
    assert_eq!(image(&root), saved);
}

#[test]
fn rejected_kernel_identity_cannot_spend_or_disclose_the_real_governor_key() {
    let root = Directory::new();
    let (actor, mut supervisor, _, governor, _) = guarded(&root);
    let (key, _) = approve(&mut supervisor, &governor, same_policy(2));
    let (port, grant) = supervisor
        .policy_promotion_port(key, 4, REPLAY_EVENTS)
        .unwrap();
    let identity = credentials();
    let wrong_pid = if identity.pid() == i32::MAX as u32 {
        identity.pid() - 1
    } else {
        identity.pid() + 1
    };
    let rejected_policy = PeerPolicy::new(identity.uid(), identity.gid(), Some(wrong_pid)).unwrap();
    let mut facade = tower(actor.clone());
    facade.set_policy_promotion(Some(port.clone())).unwrap();
    let mut rejected = session(facade, rejected_policy, 2);
    let (server, mut intruder) = UnixStream::pair().unwrap();
    intruder.write_all(&frame(&query(OPERATION))).unwrap();
    let saved = image(&root);
    assert_eq!(
        rejected.attach(server),
        Err(PeerRefusal::CredentialsRejected)
    );
    assert_eq!(rejected.status().connections_admitted, 0);
    assert_eq!(rejected.remaining_exchanges(), 2);
    assert_eq!(
        (grant.remaining_queries(), grant.remaining_operations()),
        (4, 1)
    );
    assert_eq!(image(&root), saved);

    // The same original port and approval still work for the selected real
    // kernel identity. No governor was reconstructed or handed to either peer.
    let mut facade = tower(actor);
    facade.set_policy_promotion(Some(port)).unwrap();
    let selected = next(&mut facade, OPERATION);
    let command = action(&selected);
    let policy = PeerPolicy::new(identity.uid(), identity.gid(), Some(identity.pid())).unwrap();
    let mut admitted = session(facade, policy, 1);
    let mut socket = attach(&mut admitted);
    let response = exchange(&mut admitted, &mut socket, &command);
    assert_eq!(
        field(field(&response, "envelope"), "reason_code"),
        &Json::Null
    );
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .current_policy()
            .unwrap()
            .generation(),
        2
    );
    assert_eq!(grant.remaining_operations(), 0);
}
