use super::*;
use super::super::{FilePrecheckPeerSession, MAX_PRECHECK_FRAME_BYTES, PrecheckWireError,
    decode_precheck_response};
use crate::Error;
use crate::action::consequence::oversight::actor::{ActorOutcome, UnknownReason};
use crate::action::consequence::oversight::actor_peer::{MAX_PEER_CONNECTIONS,
    PeerCredentials, PeerPolicy, PeerRefusal};
use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveProgress};
use crate::action::consequence::oversight::actor_wire::{ChannelState, CloseReason};
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

fn credentials(socket: &UnixStream) -> (PeerCredentials, PeerPolicy) {
    let observed = PeerCredentials::observe(socket).unwrap();
    let policy = PeerPolicy::new(observed.uid(), observed.gid(), Some(observed.pid())).unwrap();
    (observed, policy)
}

fn exchange(session: &mut FilePrecheckPeerSession, client: &mut UnixStream,
    request: &PrecheckRequest) -> super::super::PrecheckWireResponse
{
    client.write_all(&frame(request)).unwrap();
    let report = session.drive(DriveBudget::default()).unwrap();
    assert_eq!(report.progress.frames, 1);
    assert_eq!(report.status.pending_output_bytes, 0);
    client.set_nonblocking(true).unwrap();
    let mut bytes = Vec::new();
    let mut buffer = [0; 2048];
    for _ in 0..16 {
        match client.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => bytes.extend_from_slice(&buffer[..count]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("unexpected peer read: {error}"),
        }
    }
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert_eq!(bytes.iter().filter(|byte| **byte == b'\n').count(), 1);
    decode_precheck_response(&bytes[..bytes.len() - 1], request.query,
        request.proposal.expected_policy_epoch).unwrap()
}

#[test]
fn authenticated_real_owner_returns_exact_knowledge_but_real_submit_still_reviews() {
    for (payload, expected) in [(b"ok".as_slice(), FilePrecheckOutcome::PrecheckPassed),
        (b"blocked".as_slice(), FilePrecheckOutcome::DeniedByPolicy)] {
        let (fixture, wire) = Fixture::new(3, 3);
        let (server, mut client) = UnixStream::pair().unwrap();
        let (observed, policy) = credentials(&server);
        let mut session = FilePrecheckPeerSession::new(policy, wire, MAX_PRECHECK_FRAME_BYTES, 2).unwrap();
        let before = fixture.supervisor.host().unwrap().inspect();
        let files = fixture.root.files();
        let reads = fixture.source.status().read_attempts;
        let admission = session.attach(server).unwrap();
        assert_eq!(admission.credentials, observed);
        assert_eq!(admission.connection, 1);
        assert_eq!(session.policy(), policy);
        assert!(session.socket_fd().is_some());
        let candidate = request(901, payload);
        known(exchange(&mut session, &mut client, &candidate), expected, 901);
        assert_eq!(fixture.remaining_queries(), 2);
        assert_eq!(fixture.supervisor.host().unwrap().inspect(), before);
        assert_eq!(fixture.root.files(), files);
        assert_eq!(fixture.source.status().read_attempts, reads);
        assert_eq!(fixture.supervisor.host().unwrap().retained_requests(), 0);
        let ticket = fixture.actor.submit(901, &candidate.proposal).unwrap();
        if expected == FilePrecheckOutcome::PrecheckPassed {
            assert_eq!(fixture.actor.poll(&ticket), Knowledge::Pending { request: 901 });
        } else {
            assert!(matches!(fixture.actor.poll(&ticket), Knowledge::Known { value: ActorOutcome::Denied, .. }));
        }
        let after = fixture.supervisor.host().unwrap().inspect();
        assert_eq!((after.control.ledger.reserved, after.executions), (0, 0));
        assert!(after.control.decisions.is_empty());
        // The original transition retires the grant; the socket cannot reissue it.
        assert_eq!(exchange(&mut session, &mut client, &request(902, b"ok")).result,
            Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
        assert_eq!(fixture.remaining_queries(), 2);
    }
}

#[test]
fn each_wrong_kernel_credential_refuses_queued_input_without_spending_or_admitting() {
    for field in 0..3 {
        let (fixture, wire) = Fixture::new(2, 2);
        let (server, mut client) = UnixStream::pair().unwrap();
        let (observed, _) = credentials(&server);
        let uid = if field == 0 { if observed.uid() == 0 { 1 } else { 0 } } else { observed.uid() };
        let gid = if field == 1 { if observed.gid() == 0 { 1 } else { 0 } } else { observed.gid() };
        let pid = if field == 2 { if observed.pid() == 1 { 2 } else { 1 } } else { observed.pid() };
        let wrong = PeerPolicy::new(uid, gid, Some(pid)).unwrap();
        let mut session = FilePrecheckPeerSession::new(wrong, wire, MAX_PRECHECK_FRAME_BYTES, 1).unwrap();
        client.write_all(&frame(&request(1, b"ok"))).unwrap();
        let before = fixture.root.files();
        assert_eq!(session.attach(server), Err(PeerRefusal::CredentialsRejected));
        assert_eq!(session.status().connections_admitted, 0);
        assert!(session.status().active.is_none());
        assert!(session.socket_fd().is_none());
        assert_eq!(session.drive(DriveBudget::default()), Err(PrecheckWireError::Unavailable));
        assert_eq!(fixture.remaining_queries(), 2);
        assert_eq!(fixture.root.files(), before);
    }
}

#[test]
fn reconnect_preserves_parser_allowance_and_busy_or_limit_refusals_preserve_custody() {
    let (fixture, wire) = Fixture::new(2, 4);
    let (server, mut client) = UnixStream::pair().unwrap();
    let (_, policy) = credentials(&server);
    let mut session = FilePrecheckPeerSession::new(policy, wire, MAX_PRECHECK_FRAME_BYTES, 3).unwrap();
    let active = session.attach(server).unwrap();
    let (other, _) = UnixStream::pair().unwrap();
    assert_eq!(session.attach(other), Err(PeerRefusal::Busy));
    assert_eq!(session.status().active, Some(active));
    client.write_all(b"not-json\n").unwrap();
    assert_eq!(session.drive(DriveBudget::default()).unwrap().progress.frames, 1);
    assert_eq!(fixture.remaining_queries(), 4);
    assert!(session.disconnect());
    assert!(!session.disconnect());
    let (server, mut client) = UnixStream::pair().unwrap();
    assert_eq!(session.attach(server).unwrap().connection, 2);
    known(exchange(&mut session, &mut client, &request(2, b"ok")), FilePrecheckOutcome::PrecheckPassed, 2);
    assert_eq!(fixture.remaining_queries(), 3);
    assert_eq!(session.status().transport.unwrap().channel, ChannelState::Closed(CloseReason::ExchangeLimit));
    session.disconnect();
    let (server, mut client) = UnixStream::pair().unwrap();
    assert_eq!(session.attach(server).unwrap().connection, 3);
    client.write_all(&frame(&request(3, b"ok"))).unwrap();
    let exhausted = session.drive(DriveBudget::default()).unwrap();
    assert_eq!(exhausted.progress, DriveProgress::default());
    assert_eq!(exhausted.status.channel, ChannelState::Closed(CloseReason::ExchangeLimit));
    assert_eq!(fixture.remaining_queries(), 3);
    session.disconnect();
    let (server, _) = UnixStream::pair().unwrap();
    assert_eq!(session.attach(server), Err(PeerRefusal::Capacity));
    assert_eq!(session.status().connections_admitted, 3);
}

#[test]
fn disclosure_revocation_and_wrong_epoch_stay_knowledge_not_new_authority() {
    let (mut fixture, wire) = Fixture::new(4, 4);
    let (server, mut client) = UnixStream::pair().unwrap();
    let (_, policy) = credentials(&server);
    let mut session = FilePrecheckPeerSession::new(policy, wire, MAX_PRECHECK_FRAME_BYTES, 2).unwrap();
    session.attach(server).unwrap();
    known(exchange(&mut session, &mut client, &request(1, b"ok")), FilePrecheckOutcome::PrecheckPassed, 1);
    let mut stale = request(2, b"ok"); stale.proposal.expected_policy_epoch = 1;
    assert_eq!(exchange(&mut session, &mut client, &stale).result,
        Ok(Knowledge::Stale { generation: 1, current: 0 }));
    assert_eq!(fixture.remaining_queries(), 2);
    let before = fixture.root.files();
    fixture.disclosure.take().unwrap().revoke();
    assert_eq!(exchange(&mut session, &mut client, &request(3, b"ok")).result,
        Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" }));
    assert_eq!(fixture.root.files(), before);
    assert!(session.revoke());
    assert!(!session.revoke());
    assert!(session.socket_fd().is_none());
    assert_eq!(session.drive(DriveBudget::default()), Err(PrecheckWireError::Withheld));
    let (server, _) = UnixStream::pair().unwrap();
    assert_eq!(session.attach(server), Err(PeerRefusal::Revoked));
    // Revoking read disclosure or ingress never cancels ordinary submission.
    let ticket = fixture.actor.submit(3, &request(3, b"ok").proposal).unwrap();
    assert_eq!(fixture.actor.poll(&ticket), Knowledge::Pending { request: 3 });
}

#[test]
fn session_configuration_is_finite_and_owner_transitions_never_refresh_disclosure() {
    let (server, _) = UnixStream::pair().unwrap();
    let (_, policy) = credentials(&server);
    for (frame_bytes, connections, error) in [
        (0, 1, Error::InvalidInput), (1, 0, Error::InvalidInput),
        (MAX_PRECHECK_FRAME_BYTES + 1, 1, Error::Limit),
        (1, MAX_PEER_CONNECTIONS + 1, Error::Limit),
    ] {
        let (fixture, wire) = Fixture::new(1, 1);
        assert_eq!(FilePrecheckPeerSession::new(policy, wire, frame_bytes, connections).err(), Some(error));
        assert_eq!(fixture.remaining_queries(), 1);
    }
    let (mut fixture, wire) = Fixture::new(2, 2);
    let (server, mut client) = UnixStream::pair().unwrap();
    let mut session = FilePrecheckPeerSession::new(policy, wire, MAX_PRECHECK_FRAME_BYTES, 1).unwrap();
    session.attach(server).unwrap();
    {
        let mut host = fixture.supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(2)).unwrap();
    }
    let files = fixture.root.files();
    let reads = fixture.source.status().read_attempts;
    assert_eq!(exchange(&mut session, &mut client, &request(7, b"ok")).result,
        Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }));
    assert_eq!(fixture.remaining_queries(), 2);
    assert_eq!(fixture.root.files(), files);
    assert_eq!(fixture.source.status().read_attempts, reads);
    let debug = format!("{session:?}");
    assert!(!debug.contains("private review input"));
    assert!(!debug.contains("review_required"));
}
