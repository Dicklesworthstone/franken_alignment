//! Install original independent approval on the surviving authenticated ingress.
use super::*;

fn drain_pending(peer: &mut PublicationPeerSession, socket: &mut UnixStream) -> Json {
    let mut bytes = Vec::new();
    let mut buffer = [0; 4096];
    for _ in 0..64 {
        peer.drive(DriveBudget::default()).unwrap();
        loop {
            match socket.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => bytes.extend_from_slice(&buffer[..count]),
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) => panic!("pending operator reply read failed: {error}"),
            }
            assert!(bytes.len() <= 64 * 1024);
        }
        if bytes.last() == Some(&b'\n') {
            break;
        }
    }
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    strict_json::parse(&bytes, KNOWLEDGE_JSON_LIMITS).unwrap()
}

#[test]
fn host_installs_original_approval_before_or_after_attach_without_resetting_session_or_pending_reply()
 {
    for before_attach in [false, true] {
        let root = Directory::new();
        let (actor, mut supervisor, _, governor, _) = guarded(&root);
        let (key, _) = approve(&mut supervisor, &governor, same_policy(2));
        let (port, grant) = supervisor
            .policy_promotion_port(key, 4, REPLAY_EVENTS)
            .unwrap();
        let mut facade = tower(actor);
        facade.set_policy_promotion(Some(port.clone())).unwrap();
        let command = action(&next(&mut facade, OPERATION));
        facade.set_policy_promotion(None).unwrap();
        let identity = credentials();
        let policy = PeerPolicy::new(identity.uid(), identity.gid(), Some(identity.pid())).unwrap();
        let mut peer = session(facade, policy, 4);
        if before_attach {
            let status = peer.status();
            peer.set_policy_promotion(Some(port.clone())).unwrap();
            assert_eq!(peer.status(), status);
            assert_eq!(peer.remaining_exchanges(), 4);
        }
        let mut socket = attach(&mut peer);
        let initial = peer.status();
        if !before_attach {
            peer.set_policy_promotion(Some(port.clone())).unwrap();
            assert_eq!(peer.status(), initial);
        }
        assert_eq!(peer.status().connections_admitted, 1);
        assert_eq!(peer.remaining_exchanges(), 4);
        let saved = image(&root);
        socket.write_all(&frame(&query(OPERATION))).unwrap();
        let entered = peer
            .drive(DriveBudget {
                frames: 1,
                write_bytes: 0,
                ..DriveBudget::default()
            })
            .unwrap();
        assert_eq!(entered.progress.frames, 1);
        let pending = peer.status();
        assert!(pending.transport.unwrap().pending_output_bytes > 0);
        assert_eq!(peer.remaining_exchanges(), 3);
        assert_eq!(
            (grant.remaining_queries(), grant.remaining_operations()),
            (2, 1)
        );
        peer.set_policy_promotion(None).unwrap();
        assert_eq!(peer.status(), pending);
        peer.set_policy_promotion(Some(port.clone())).unwrap();
        assert_eq!(peer.status(), pending);
        assert_eq!(peer.remaining_exchanges(), 3);
        assert_eq!(
            (grant.remaining_queries(), grant.remaining_operations()),
            (2, 1)
        );
        assert_eq!(image(&root), saved);
        let reply = drain_pending(&mut peer, &mut socket);
        assert_eq!(field(field(&reply, "envelope"), "reason_code"), &Json::Null);
        let observed = field(
            field(field(field(&reply, "result"), "value"), "knowledge"),
            "value",
        );
        assert_eq!(
            field(observed, "action"),
            &strict_json::parse(
                encode_request(&command).unwrap().as_bytes(),
                KNOWLEDGE_JSON_LIMITS
            )
            .unwrap()
        );
        let promoted = exchange(&mut peer, &mut socket, &command);
        assert_eq!(
            field(field(&promoted, "envelope"), "reason_code"),
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
        assert_eq!(peer.remaining_exchanges(), 2);
        let acknowledged = image(&root);
        peer.set_policy_promotion(Some(port.clone())).unwrap();
        assert_eq!(peer.remaining_exchanges(), 2);
        let retry = exchange(&mut peer, &mut socket, &command);
        assert_eq!(retry, promoted);
        assert_eq!(image(&root), acknowledged);
        assert_eq!(
            (grant.remaining_queries(), grant.remaining_operations()),
            (2, 0)
        );
        assert_eq!(peer.status().connections_admitted, 1);
        assert_eq!(peer.remaining_exchanges(), 1);
        assert!(peer.revoke());
        let retired = peer.status();
        assert_eq!(
            peer.set_policy_promotion(Some(port)),
            Err(Error::WrongState)
        );
        assert_eq!(peer.set_policy_promotion(None), Err(Error::WrongState));
        assert_eq!(peer.status(), retired);
        assert_eq!(peer.remaining_exchanges(), 1);
        let (server, _client) = UnixStream::pair().unwrap();
        assert_eq!(peer.attach(server), Err(PeerRefusal::Revoked));
        assert_eq!(image(&root), acknowledged);
    }
}

#[test]
fn foreign_install_withdraws_only_the_selected_grant_and_preserves_original_pending_acknowledgment()
{
    let root = Directory::new();
    let (actor, mut supervisor, _, governor, _) = guarded(&root);
    let (key, _) = approve(&mut supervisor, &governor, same_policy(2));
    let (port, grant) = supervisor
        .policy_promotion_port(key, 4, REPLAY_EVENTS)
        .unwrap();
    let mut facade = tower(actor);
    facade.set_policy_promotion(Some(port.clone())).unwrap();
    let command = action(&next(&mut facade, OPERATION));
    let identity = credentials();
    let policy = PeerPolicy::new(identity.uid(), identity.gid(), Some(identity.pid())).unwrap();
    let mut peer = session(facade, policy, 4);
    let mut socket = attach(&mut peer);
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
    let pending = peer.status();
    assert!(pending.transport.unwrap().pending_output_bytes > 0);
    let saved = image(&root);

    let foreign_root = Directory::new();
    let (_, mut foreign_supervisor, _, foreign_governor, _) = guarded(&foreign_root);
    let (foreign_key, _) = approve(&mut foreign_supervisor, &foreign_governor, same_policy(2));
    let (foreign, foreign_grant) = foreign_supervisor
        .policy_promotion_port(foreign_key, 4, REPLAY_EVENTS)
        .unwrap();
    let foreign_image = image(&foreign_root);
    assert_eq!(
        peer.set_policy_promotion(Some(foreign)),
        Err(Error::Binding)
    );
    assert_eq!(peer.status(), pending);
    assert_eq!(peer.remaining_exchanges(), 3);
    assert_eq!(
        (
            foreign_grant.remaining_queries(),
            foreign_grant.remaining_operations()
        ),
        (4, 1)
    );
    let acknowledged = drain_pending(&mut peer, &mut socket);
    assert_eq!(
        field(field(&acknowledged, "envelope"), "reason_code"),
        &Json::Null
    );
    assert_eq!(image(&root), saved);
    let refused = exchange(&mut peer, &mut socket, &command);
    assert_eq!(
        field(field(&refused, "envelope"), "reason_code"),
        &Json::String("CapabilityUnsupported".into())
    );
    assert_eq!(image(&root), saved);
    assert_eq!(image(&foreign_root), foreign_image);
    assert_eq!(peer.remaining_exchanges(), 2);
    let status = peer.status();
    peer.set_policy_promotion(Some(port)).unwrap();
    assert_eq!(peer.status(), status);
    let retry = exchange(&mut peer, &mut socket, &command);
    assert_eq!(retry, acknowledged);
    assert_eq!(image(&root), saved);
    assert_eq!(
        (grant.remaining_queries(), grant.remaining_operations()),
        (3, 0)
    );
    assert_eq!(peer.remaining_exchanges(), 1);
    assert_eq!(peer.status().connections_admitted, 1);
}
