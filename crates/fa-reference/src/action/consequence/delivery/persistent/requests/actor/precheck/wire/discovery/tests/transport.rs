//! Real original owners behind the shared framing/session, never a second grant.
use super::*;
use super::super::super::{FilePrecheckChannel, FilePrecheckWire, MAX_PRECHECK_FRAME_BYTES,
    PrecheckRequest, decode_precheck_response, encode_precheck_request};
use crate::action::consequence::oversight::actor_wire::{ChannelState, CloseReason};

fn request(query: u64) -> PrecheckCapabilitiesRequest {
    PrecheckCapabilitiesRequest { query, expected_policy_epoch: 0 }
}
fn ordinary(query: u64) -> PrecheckRequest { PrecheckRequest { query, proposal: proposal() } }
fn line(mut bytes: Vec<u8>) -> Vec<u8> { bytes.push(b'\n'); bytes }

#[test]
fn mixed_wire_reads_spend_the_original_frame_and_native_query_allowances() {
    let (fixture, host) = Fixture::new(); let (_, supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    let (port, grant) = supervisor.disclose_exact_precheck(revision, 81, 2).unwrap();
    let mut wire = FilePrecheckWire::new(port, 3).unwrap(); let files = fixture.files();
    let bytes = wire.exchange_frame(&request(7).encode().unwrap()).unwrap();
    let Knowledge::Known { value, .. } = decode_capabilities_response(&bytes, 7, 0).unwrap().result.unwrap()
        else { panic!("expected capabilities") };
    assert_eq!(value.remaining_queries(), 1);
    let bytes = wire.exchange_frame(&encode_precheck_request(&ordinary(8)).unwrap()).unwrap();
    assert!(matches!(decode_precheck_response(&bytes, 8, 0).unwrap().result,
        Ok(Knowledge::Known { .. })));
    let bytes = wire.exchange_frame(&request(9).encode().unwrap()).unwrap();
    assert_eq!(decode_capabilities_response(&bytes, 9, 0).unwrap().result, Err(PrecheckWireError::Capacity));
    assert_eq!((wire.remaining_exchanges(), grant.remaining_queries()), (0, 0));
    let bytes = wire.exchange_frame(&request(10).encode().unwrap()).unwrap();
    let refusal = decode_precheck_response(&bytes, 10, 0).unwrap();
    assert_eq!(refusal.query, None); assert_eq!(refusal.result, Err(PrecheckWireError::Capacity));
    assert!(decode_capabilities_response(&bytes, 10, 0).is_err());
    assert_eq!(fixture.files(), files); assert_eq!(supervisor.host().unwrap().retained_requests(), 0);
}

#[test]
fn malformed_discovery_charges_parser_work_without_native_reads_or_query_echo() {
    let (_fixture, host) = Fixture::new(); let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.disclose_exact_precheck(supervisor.host().unwrap().revision(), 81, 2).unwrap();
    let mut wire = FilePrecheckWire::new(port, 5).unwrap();
    let valid = request(7).encode().unwrap();
    let text = String::from_utf8(valid.clone()).unwrap();
    let mut oversized = valid.clone(); oversized.resize(MAX_CAPABILITIES_REQUEST_BYTES + 1, b' ');
    for bytes in [oversized, text.replace("capabilities\"", "submit\"").into_bytes(),
        text.replacen('{', "{\"payload_hex\":\"\",", 1).into_bytes()] {
        let response = wire.exchange_frame(&bytes).unwrap();
        let refusal = decode_precheck_response(&response, 7, 0).unwrap();
        assert_eq!(refusal.query, None); assert!(refusal.result.is_err());
        assert_eq!(grant.remaining_queries(), 2);
    }
    // The legacy typed API stays precheck-only, not a covert discovery fallback.
    assert_eq!(wire.exchange(&valid).result, Err(PrecheckWireError::UnsupportedVersion));
    assert_eq!(grant.remaining_queries(), 2);
    assert!(decode_capabilities_response(&wire.exchange_frame(&valid).unwrap(), 7, 0).is_ok());
    assert_eq!((wire.remaining_exchanges(), grant.remaining_queries()), (0, 1));
}

#[test]
fn mixed_frames_respect_fragmentation_flush_backpressure_and_reconnect_accounting() {
    let (_fixture, host) = Fixture::new(); let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.disclose_exact_precheck(supervisor.host().unwrap().revision(), 81, 3).unwrap();
    let wire = FilePrecheckWire::new(port, 3).unwrap();
    let mut channel = FilePrecheckChannel::new(wire, MAX_PRECHECK_FRAME_BYTES).unwrap();
    let discovery = line(request(7).encode().unwrap());
    for byte in &discovery[..discovery.len() - 1] {
        assert_eq!(channel.feed(&[*byte]).state, ChannelState::Reading);
    }
    let precheck = line(encode_precheck_request(&ordinary(8)).unwrap());
    let mut suffix = vec![b'\n']; suffix.extend_from_slice(&precheck);
    assert_eq!(channel.feed(&suffix).consumed, 1);
    assert_eq!(grant.remaining_queries(), 2);
    let bytes = channel.pending_output().to_vec();
    assert!(decode_capabilities_response(bytes.strip_suffix(b"\n").unwrap(), 7, 0).is_ok());
    assert_eq!(channel.feed(&precheck).consumed, 0);
    channel.acknowledge_written(bytes.len()).unwrap();
    assert_eq!(channel.feed(&precheck).consumed, 0);
    channel.acknowledge_flushed().unwrap();
    assert_eq!(channel.feed(&precheck).consumed, precheck.len());
    assert_eq!(grant.remaining_queries(), 1);
    // Discard an undelivered reply, preserving both spent allowances.
    let mut replacement = FilePrecheckChannel::new(channel.into_wire(), MAX_PRECHECK_FRAME_BYTES).unwrap();
    replacement.feed(&discovery);
    let bytes = replacement.pending_output().to_vec();
    let Knowledge::Known { value, .. } = decode_capabilities_response(bytes.strip_suffix(b"\n").unwrap(), 7, 0)
        .unwrap().result.unwrap() else { panic!("known") };
    assert_eq!(value.remaining_queries(), 0);
    replacement.acknowledge_written(bytes.len()).unwrap();
    assert_eq!(replacement.acknowledge_flushed().unwrap(), ChannelState::Closed(CloseReason::ExchangeLimit));
}

#[cfg(target_os = "linux")]
mod sockets {
    use super::*;
    use super::super::super::super::FilePrecheckPeerSession;
    use super::super::super::super::client::{PrecheckCapabilitiesClient, PrecheckPeerClient,
        PrecheckClientError, PrecheckClientPhase};
    use crate::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy, PeerRefusal};
    use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveProgress};
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    fn policy(socket: &UnixStream) -> PeerPolicy {
        let peer = PeerCredentials::observe(socket).unwrap();
        PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap()
    }
    fn drive<R>(client: &mut PrecheckPeerClient<R>, session: &mut FilePrecheckPeerSession) -> R {
        let tiny = DriveBudget { read_bytes: 1, write_bytes: 1, io_calls: 1, frames: 1 };
        for _ in 0..25_000 {
            let report = client.drive(tiny).unwrap();
            assert!(report.progress.read_bytes <= 1 && report.progress.written_bytes <= 1);
            assert!(report.progress.io_calls <= 1 && report.progress.frames <= 1);
            assert!(report.status.failure.is_none(), "{:?}", report.status);
            if report.status.closed() { break; }
            let report = session.drive(tiny).unwrap();
            assert!(report.status.failure.is_none(), "{:?}", report.status);
        }
        assert_eq!(client.status().phase, PrecheckClientPhase::Complete);
        assert!(client.socket_fd().is_none());
        assert_eq!(client.drive(tiny).unwrap().progress, DriveProgress::default());
        let response = client.take_response().expect("completed typed response");
        assert!(client.take_response().is_none()); response
    }

    #[test]
    fn authenticated_discovery_then_precheck_preserves_original_review_and_durable_state() {
        let (fixture, host) = Fixture::new(); let (actor, mut supervisor) = host.into_actor_gateway();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let (port, grant) = supervisor.disclose_exact_precheck(revision, 81, 3).unwrap();
        let (socket, server) = UnixStream::pair().unwrap(); let policy = policy(&socket);
        let mut session = FilePrecheckPeerSession::new(policy, FilePrecheckWire::new(port, 3).unwrap(),
            MAX_PRECHECK_FRAME_BYTES, 2).unwrap();
        session.attach(server).unwrap();
        let mut client = PrecheckCapabilitiesClient::connect_capabilities(socket, policy, request(7), 0).unwrap();
        let files = fixture.files(); let reads = fixture.source.status().read_attempts;
        let before = supervisor.host().unwrap().inspect();
        let Knowledge::Known { value, basis } = drive(&mut client, &mut session).result.unwrap()
            else { panic!("known capabilities") };
        assert_eq!(value.remaining_queries(), 2); assert_eq!(basis.disclosure, 81);
        assert!(session.disconnect());
        let (socket, server) = UnixStream::pair().unwrap(); session.attach(server).unwrap();
        let mut client = PrecheckPeerClient::connect(socket, policy, ordinary(8), 0).unwrap();
        assert!(matches!(drive(&mut client, &mut session).result, Ok(Knowledge::Known { .. })));
        assert_eq!(grant.remaining_queries(), 1);
        assert_eq!(session.status().connections_admitted, 2);
        assert_eq!(fixture.files(), files); assert_eq!(fixture.source.status().read_attempts, reads);
        assert_eq!(supervisor.host().unwrap().inspect(), before);
        let ticket = actor.submit(7, &proposal()).unwrap();
        assert_eq!(actor.poll(&ticket), Knowledge::Pending { request: 7 });
        assert_eq!(supervisor.host().unwrap().inspect().control.ledger.stages[&1], ActionState::Reviewing);
    }

    #[test]
    fn discovery_authentication_epoch_and_zero_query_fail_before_request_bytes() {
        for variant in 0..3 {
            let (socket, mut server) = UnixStream::pair().unwrap(); let original = policy(&socket);
            let wrong = PeerPolicy::new(original.uid(), original.gid(),
                Some(if original.pid() == Some(1) { 2 } else { 1 })).unwrap();
            let error = PrecheckCapabilitiesClient::connect_capabilities(socket,
                if variant == 0 { wrong } else { original },
                request(if variant == 2 { 0 } else { 7 }), if variant == 1 { 1 } else { 0 }).unwrap_err();
            assert_eq!(error, match variant {
                0 => PrecheckClientError::Peer(PeerRefusal::CredentialsRejected),
                1 => PrecheckClientError::ExpectedPolicyEpoch,
                _ => PrecheckClientError::Request(PrecheckWireError::MalformedRequest),
            });
            server.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut received = Vec::new(); server.read_to_end(&mut received).unwrap();
            assert!(received.is_empty());
        }
    }

    #[test]
    fn lost_discovery_reply_cannot_refund_native_work_on_explicit_reconnect() {
        let (fixture, host) = Fixture::new(); let (_, supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.disclose_exact_precheck(supervisor.host().unwrap().revision(), 81, 2).unwrap();
        let (socket, server) = UnixStream::pair().unwrap(); let policy = policy(&socket);
        let mut session = FilePrecheckPeerSession::new(policy, FilePrecheckWire::new(port, 2).unwrap(),
            MAX_PRECHECK_FRAME_BYTES, 2).unwrap(); session.attach(server).unwrap();
        let mut client = PrecheckCapabilitiesClient::connect_capabilities(socket, policy, request(7), 0).unwrap();
        let files = fixture.files();
        client.drive(DriveBudget { read_bytes: 0, ..DriveBudget::default() }).unwrap();
        for _ in 0..4096 {
            session.drive(DriveBudget { write_bytes: 0, ..DriveBudget::default() }).unwrap();
            if grant.remaining_queries() == 1 { break; }
        }
        assert_eq!(grant.remaining_queries(), 1); assert!(client.take_response().is_none());
        drop(client); assert!(session.disconnect());
        let (socket, server) = UnixStream::pair().unwrap(); session.attach(server).unwrap();
        let mut client = PrecheckCapabilitiesClient::connect_capabilities(socket, policy, request(7), 0).unwrap();
        let Knowledge::Known { value, .. } = drive(&mut client, &mut session).result.unwrap()
            else { panic!("known") };
        assert_eq!(value.remaining_queries(), 0); assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(fixture.files(), files);
    }

    #[test]
    fn discovery_client_rejects_wrong_protocol_correlation_and_transport_boundaries() {
        let good = line(response(7, 0, 1).encode().unwrap());
        let (fixture, host) = Fixture::new(); let (_, supervisor) = host.into_actor_gateway();
        let (port, _grant) = supervisor.disclose_exact_precheck(supervisor.host().unwrap().revision(), 81, 2).unwrap();
        let mut wire = FilePrecheckWire::new(port, 2).unwrap();
        let pass = line(wire.exchange(&encode_precheck_request(&ordinary(7)).unwrap()).encode().unwrap());
        for reply in [Vec::new(), good[..good.len() - 1].to_vec(),
            [good.clone(), b"x".to_vec()].concat(), vec![b'x'; 1026], pass,
            line(response(8, 0, 1).encode().unwrap()), line(response(7, 1, 1).encode().unwrap())] {
            let (socket, mut server) = UnixStream::pair().unwrap(); let policy = policy(&socket);
            let mut client = PrecheckCapabilitiesClient::connect_capabilities(socket, policy, request(7), 0).unwrap();
            server.write_all(&reply).unwrap(); server.shutdown(std::net::Shutdown::Write).unwrap();
            for _ in 0..4096 {
                client.drive(DriveBudget::default()).unwrap();
                if client.status().closed() { break; }
            }
            assert_eq!(client.status().phase, PrecheckClientPhase::Failed);
            assert!(client.take_response().is_none() && client.socket_fd().is_none());
            assert_eq!(client.drive(DriveBudget::default()).unwrap().progress, DriveProgress::default());
        }
        // Conversely, capabilities never become a passing exact precheck.
        let (socket, mut server) = UnixStream::pair().unwrap(); let policy = policy(&socket);
        let mut client = PrecheckPeerClient::connect(socket, policy, ordinary(7), 0).unwrap();
        server.write_all(&good).unwrap(); server.shutdown(std::net::Shutdown::Write).unwrap();
        for _ in 0..4096 {
            client.drive(DriveBudget::default()).unwrap(); if client.status().closed() { break; }
        }
        assert_eq!(client.status().failure, Some(PrecheckClientError::Response(PrecheckResponseError::UnsupportedVersion)));
        assert!(client.take_response().is_none()); drop(fixture);
    }
}
