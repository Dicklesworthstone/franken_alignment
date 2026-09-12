#![cfg(target_os = "linux")]

#[path = "support/actor_gateway.rs"]
#[allow(dead_code)]
mod fixture;

use fa_reference::action::consequence::oversight::actor::{ActorOutcome, IntakeLimits, Knowledge};
use fa_reference::action::consequence::oversight::actor_peer::{
    MAX_PEER_CONNECTIONS, PeerCredentials, PeerPolicy, PeerRefusal, PeerSession,
};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits, Command, WireError, encode_command};
use fa_reference::Error;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

fn credentials() -> PeerCredentials {
    let (socket, _peer) = UnixStream::pair().unwrap();
    PeerCredentials::observe(&socket).unwrap()
}

fn policy() -> PeerPolicy {
    let c = credentials();
    PeerPolicy::new(c.uid(), c.gid(), Some(c.pid())).unwrap()
}

fn session(wire: ActorWire, policy: PeerPolicy, connections: u64) -> PeerSession {
    PeerSession::new(policy, wire, ChannelLimits::default(), connections).unwrap()
}

fn connect(session: &mut PeerSession) -> UnixStream {
    let (server, client) = UnixStream::pair().unwrap();
    client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let admission = session.attach(server).unwrap();
    assert_eq!(admission.credentials, credentials());
    client
}

fn exchange(session: &mut PeerSession, client: &mut UnixStream, command: &Command) -> String {
    let mut bytes = encode_command(command).unwrap();
    bytes.push(b'\n');
    client.write_all(&bytes).unwrap();
    assert_eq!(session.drive(DriveBudget::default()).unwrap().progress.frames, 1);
    let mut reply = String::new();
    BufReader::new(client).read_line(&mut reply).unwrap();
    reply
}

fn submit(id: u64) -> Command {
    Command::Submit { request: id, proposal: fixture::proposal() }
}

#[test]
fn real_credentials_precede_original_actor_intake() {
    let c = credentials();
    assert_eq!(c.pid(), std::process::id());
    let (port, mut supervisor, endpoint) = fixture::fixture(IntakeLimits::default());
    let mut session = session(ActorWire::new(port), policy(), 2);
    let mut client = connect(&mut session);
    assert!(session.socket_fd().is_some());
    assert!(exchange(&mut session, &mut client, &submit(41)).contains("pending"));
    let intake = supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    assert_eq!(intake.request, 41);
    let proposal = intake.result.unwrap().unwrap();
    assert_eq!(proposal.action.spec().scope.principal, 2);
    assert_eq!(supervisor.broker().inspect().ledger.reserved, 0);
    assert_eq!(endpoint.execution_count(), 0);
    assert_eq!(session.status().connections_admitted, 1);
}

#[test]
fn each_credential_mismatch_refuses_before_reading_buffered_commands() {
    let c = credentials();
    let other_uid = if c.uid() == 0 { 1 } else { 0 };
    let other_gid = if c.gid() == 0 { 1 } else { 0 };
    let other_pid = if c.pid() == 1 { 2 } else { 1 };
    for wrong in [PeerPolicy::new(other_uid, c.gid(), Some(c.pid())).unwrap(),
        PeerPolicy::new(c.uid(), other_gid, Some(c.pid())).unwrap(),
        PeerPolicy::new(c.uid(), c.gid(), Some(other_pid)).unwrap()]
    {
        let (port, mut supervisor, _) = fixture::fixture(IntakeLimits::default());
        let mut session = session(ActorWire::new(port), wrong, 2);
        let before = session.status();
        let (server, mut client) = UnixStream::pair().unwrap();
        let mut bytes = encode_command(&submit(1)).unwrap(); bytes.push(b'\n');
        client.write_all(&bytes).unwrap();
        assert_eq!(session.attach(server), Err(PeerRefusal::CredentialsRejected));
        assert_eq!(session.status(), before);
        assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
        assert_eq!(supervisor.broker().inspect().ledger.available, 100);
    }
    let (port, mut supervisor, _) = fixture::fixture(IntakeLimits::default());
    let mut allowed = session(ActorWire::new(port), PeerPolicy::new(c.uid(), c.gid(), None).unwrap(), 1);
    let mut client = connect(&mut allowed);
    exchange(&mut allowed, &mut client, &submit(1));
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_some());
}

#[test]
fn a_matching_second_connection_cannot_displace_the_active_session() {
    let (port, mut supervisor, _) = fixture::fixture(IntakeLimits::default());
    let mut session = session(ActorWire::new(port), policy(), 2);
    let mut original = connect(&mut session);
    let before = session.status();
    let (server, _contender) = UnixStream::pair().unwrap();
    assert_eq!(session.attach(server), Err(PeerRefusal::Busy));
    assert_eq!(session.status(), before);
    exchange(&mut session, &mut original, &submit(1));
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_some());
}

#[test]
fn reconnect_keeps_tickets_and_does_not_reset_mailbox_capacity() {
    let (port, mut supervisor, _) = fixture::fixture(IntakeLimits { requests: 1, payload_bytes: 16 });
    let ticket = port.submit(7, &fixture::proposal()).unwrap();
    let mut session = session(ActorWire::new(port.clone()), policy(), 2);
    let mut client = connect(&mut session);
    exchange(&mut session, &mut client, &submit(7));
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    assert!(session.disconnect());
    assert!(!session.disconnect());
    drop(client);
    let mut client = connect(&mut session);
    assert!(exchange(&mut session, &mut client, &Command::Poll { request: 7 }).contains("pending"));
    assert!(exchange(&mut session, &mut client, &submit(7)).contains("pending"));
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    let mut changed = fixture::proposal(); changed.payload.push(0);
    assert!(exchange(&mut session, &mut client, &Command::Submit { request: 7, proposal: changed })
        .contains("idempotency_conflict"));
    assert!(exchange(&mut session, &mut client, &submit(8)).contains("capacity"));
    exchange(&mut session, &mut client, &Command::Cancel { request: 7 });
    supervisor.synchronize().unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
}

#[test]
fn unfinished_frames_are_discarded_without_admitting_a_request() {
    let (port, mut supervisor, _) = fixture::fixture(IntakeLimits::default());
    let mut session = session(ActorWire::new(port), policy(), 2);
    let mut client = connect(&mut session);
    let document = encode_command(&submit(1)).unwrap();
    client.write_all(&document).unwrap();
    assert_eq!(session.drive(DriveBudget::default()).unwrap().progress.frames, 0);
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
    session.disconnect(); drop(client);
    let mut client = connect(&mut session);
    exchange(&mut session, &mut client, &submit(1));
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_some());
}

#[test]
fn revoking_ingress_is_not_cancelling_an_accepted_effect() {
    let (port, mut supervisor, _) = fixture::fixture(IntakeLimits::default());
    let ticket = port.submit(1, &fixture::proposal()).unwrap();
    let mut session = session(ActorWire::new(port.clone()), policy(), 2);
    let mut client = connect(&mut session);
    exchange(&mut session, &mut client, &submit(1));
    supervisor.accept_next(&fixture::snapshot()).unwrap().unwrap();
    let inputs = fixture::review(&mut supervisor, 1, 1);
    let _permit = supervisor.authorize_request(1, Some(&inputs), &fixture::snapshot()).unwrap();
    let before = supervisor.broker().inspect();
    assert!(session.revoke()); assert!(!session.revoke());
    assert_eq!(session.drive(DriveBudget::default()), Err(WireError::Withheld));
    let (server, _client) = UnixStream::pair().unwrap();
    assert_eq!(session.attach(server), Err(PeerRefusal::Revoked));
    assert_eq!(supervisor.broker().inspect(), before);
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { .. }));
    port.cancel(&ticket).unwrap(); supervisor.synchronize().unwrap();
    assert_eq!(supervisor.broker().inspect().ledger.available, 100);
}

#[test]
fn exhausted_connection_quota_is_not_a_fresh_session() {
    let (port, mut supervisor, _) = fixture::fixture(IntakeLimits::default());
    let mut session = session(ActorWire::new(port), policy(), 1);
    let mut client = connect(&mut session);
    exchange(&mut session, &mut client, &submit(1));
    session.disconnect();
    let before = session.status();
    let (server, _client) = UnixStream::pair().unwrap();
    assert_eq!(session.attach(server), Err(PeerRefusal::Capacity));
    assert_eq!(session.status(), before);
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_some());
    assert!(supervisor.accept_next(&fixture::snapshot()).unwrap().is_none());
}

#[test]
fn client_can_check_its_server_without_trusting_a_claimed_id() {
    let (server, client) = UnixStream::pair().unwrap();
    assert_eq!(policy().verify(&client).unwrap(), credentials());
    let c = credentials();
    let wrong = PeerPolicy::new(c.uid(), c.gid(), Some(if c.pid() == 1 { 2 } else { 1 })).unwrap();
    assert_eq!(wrong.verify(&client).unwrap_err().kind(), std::io::ErrorKind::PermissionDenied);
    server.set_nonblocking(true).unwrap();
    let mut reader = &server;
    let mut byte = [0];
    assert_eq!(std::io::Read::read(&mut reader, &mut byte).unwrap_err().kind(), std::io::ErrorKind::WouldBlock);
}

#[test]
fn invalid_policies_and_limits_refuse_before_a_connection_exists() {
    for (uid, gid, pid) in [(u32::MAX, 1, None), (1, u32::MAX, None), (1, 1, Some(0)), (1, 1, Some(u32::MAX))] {
        assert_eq!(PeerPolicy::new(uid, gid, pid), Err(Error::InvalidInput));
    }
    let (port, _supervisor, _) = fixture::fixture(IntakeLimits::default());
    assert!(matches!(PeerSession::new(policy(), ActorWire::new(port.clone()), ChannelLimits::default(), 0), Err(Error::InvalidInput)));
    assert!(matches!(PeerSession::new(policy(), ActorWire::new(port.clone()), ChannelLimits::default(), MAX_PEER_CONNECTIONS + 1), Err(Error::Limit)));
    assert!(matches!(PeerSession::new(policy(), ActorWire::new(port), ChannelLimits { frame_bytes: 0, exchanges: 1 }, 1), Err(Error::InvalidInput)));
}
