//! Real source-backed mixed ingress. No caller-selected schema grants access.
use super::*;
use super::super::super::{FilePrecheckWire, FilePrecheckChannel,
    MAX_PRECHECK_FRAME_BYTES, MAX_PRECHECK_RESPONSE_BYTES, encode_precheck_request};
use super::super::super::discovery::{PrecheckCapabilitiesRequest, decode_capabilities_response};
use crate::action::consequence::oversight::actor_wire::{ChannelState, CloseReason};

fn document(query: u64) -> Vec<u8> {
    PolicyRequest { query, expected_policy_epoch: 0 }.encode().unwrap()
}
fn full_wire(fixture: &Fixture, count: usize) -> (FilePrecheckWire, FilePrecheckDisclosure) {
    let (port, grant) = fixture.supervisor.disclose_full_precheck(
        fixture.supervisor.host().unwrap().revision(), 82, count, None).unwrap();
    (FilePrecheckWire::with_policy(port, count).unwrap(), grant)
}

#[test]
fn exact_ingress_withholds_definition_but_full_ingress_returns_the_original_policy() {
    let (fixture, mut exact) = Fixture::new(3, 3);
    let (mut full, grant) = full_wire(&fixture, 3);
    assert_eq!(exact.response_limit(), MAX_PRECHECK_RESPONSE_BYTES);
    assert_eq!(full.response_limit(), MAX_POLICY_RESPONSE_BYTES);
    let files = fixture.root.files();
    let withheld = decode_policy_response(&exact.exchange_frame(&document(7)).unwrap(), 7, 0).unwrap();
    assert_eq!(withheld.result, Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" }));
    assert_eq!(fixture.remaining_queries(), 2);
    let result = decode_policy_response(&full.exchange_frame(&document(7)).unwrap(), 7, 0).unwrap();
    let Knowledge::Known { value, .. } = result.result.unwrap() else { panic!("definition") };
    assert_eq!(value.definition().nodes().len(), 3);
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(fixture.root.files(), files);
    assert_eq!(fixture.supervisor.host().unwrap().retained_requests(), 0);
}

#[test]
fn mixed_operations_spend_one_grant_and_discovery_reports_only_the_selected_capability() {
    let (fixture, mut exact) = Fixture::new(4, 4);
    let (mut full, grant) = full_wire(&fixture, 4);
    let discover = PrecheckCapabilitiesRequest { query: 7, expected_policy_epoch: 0 }.encode().unwrap();
    for (wire, expected) in [(&mut exact, false), (&mut full, true)] {
        let bytes = wire.exchange_frame(&discover).unwrap();
        assert!(bytes.len() <= MAX_PRECHECK_RESPONSE_BYTES);
        let Knowledge::Known { value, .. } = decode_capabilities_response(&bytes, 7, 0).unwrap().result.unwrap()
            else { panic!("discovery") };
        assert_eq!(value.policy_contents_available(), expected);
        assert_eq!(value.remaining_queries(), 3);
    }
    assert!(decode_policy_response(&full.exchange_frame(&document(8)).unwrap(), 8, 0)
        .unwrap().result.is_ok());
    let precheck = encode_precheck_request(&request(9, b"ok")).unwrap();
    assert!(matches!(decode_precheck_response(&full.exchange_frame(&precheck).unwrap(), 9, 0)
        .unwrap().result, Ok(Knowledge::Known { .. })));
    assert_eq!(grant.remaining_queries(), 1);
    // Legacy typed entry remains precheck-only, even when full access was selected.
    assert_eq!(full.exchange(&document(10)).result, Err(PrecheckWireError::UnsupportedVersion));
    assert_eq!((full.remaining_exchanges(), grant.remaining_queries()), (0, 1));
}

#[test]
fn invalid_policy_frames_spend_parser_allowance_without_disclosing_partial_labels() {
    let (fixture, _) = Fixture::new(4, 4); let (mut wire, grant) = full_wire(&fixture, 4);
    let good = String::from_utf8(document(7)).unwrap();
    let mut oversized = document(7); oversized.resize(MAX_POLICY_REQUEST_BYTES + 1, b' ');
    for bytes in [oversized, good.replacen('{', "{\"query\":\"9\",", 1).into_bytes(),
        good.replacen('{', "{\"helper\":true,", 1).into_bytes()] {
        let bytes = wire.exchange_frame(&bytes).unwrap();
        let response = decode_precheck_response(&bytes, 7, 0).unwrap();
        assert!(response.result.is_err()); assert_eq!(response.query, None);
        assert_eq!(grant.remaining_queries(), 4);
    }
    assert!(decode_policy_response(&wire.exchange_frame(&document(7)).unwrap(), 7, 0).is_ok());
    assert_eq!((wire.remaining_exchanges(), grant.remaining_queries()), (0, 3));
}

#[test]
fn policy_frame_fragments_and_lost_output_preserve_flush_barrier_and_spent_work() {
    let (fixture, _) = Fixture::new(3, 3); let (wire, grant) = full_wire(&fixture, 3);
    let mut channel = FilePrecheckChannel::new(wire, MAX_PRECHECK_FRAME_BYTES).unwrap();
    for byte in document(7) { assert_eq!(channel.feed(&[byte]).state, ChannelState::Reading); }
    let next = [document(8), b"\n".to_vec()].concat();
    let tail = [b"\n".to_vec(), next.clone()].concat();
    assert_eq!(channel.feed(&tail).consumed, 1);
    assert_eq!(grant.remaining_queries(), 2);
    let length = channel.pending_output().len();
    channel.acknowledge_written(length - 1).unwrap();
    assert!(channel.acknowledge_flushed().is_err());
    assert_eq!(channel.feed(&next).consumed, 0);
    channel.acknowledge_written(1).unwrap();
    assert_eq!(channel.feed(&next).consumed, 0);
    assert_eq!(channel.acknowledge_flushed().unwrap(), ChannelState::Reading);
    channel.feed(&next);
    assert_eq!(grant.remaining_queries(), 1);
    // Lost reply is not a free retry, nor permission to upgrade another wire.
    let mut replacement = FilePrecheckChannel::new(channel.into_wire(), MAX_PRECHECK_FRAME_BYTES).unwrap();
    replacement.feed(&next);
    assert_eq!(grant.remaining_queries(), 0);
    let response = replacement.pending_output().to_vec();
    assert!(decode_policy_response(response.strip_suffix(b"\n").unwrap(), 8, 0).is_ok());
    replacement.acknowledge_written(response.len()).unwrap();
    assert_eq!(replacement.acknowledge_flushed().unwrap(), ChannelState::Closed(CloseReason::ExchangeLimit));
}

#[cfg(target_os = "linux")]
mod sockets {
    use super::*;
    use super::super::super::super::FilePrecheckPeerSession;
    use super::super::super::super::client::{PrecheckPolicyClient, PrecheckPeerClient,
        PrecheckClientError, PrecheckClientPhase};
    use crate::action::consequence::oversight::actor_peer::{PeerCredentials, PeerPolicy, PeerRefusal};
    use crate::action::consequence::oversight::actor_transport::{DriveBudget, DriveProgress};
    use std::{io::Read, os::unix::net::UnixStream, time::Duration};

    fn policy(socket: &UnixStream) -> PeerPolicy {
        let id = PeerCredentials::observe(socket).unwrap();
        PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap()
    }
    fn drive<R>(client: &mut PrecheckPeerClient<R>, session: &mut FilePrecheckPeerSession) -> R {
        let tiny = DriveBudget { read_bytes: 1, write_bytes: 1, frames: 1, io_calls: 1 };
        for _ in 0..25_000 {
            let report = client.drive(tiny).unwrap();
            assert!(report.status.failure.is_none(), "{:?}", report.status);
            assert!(report.progress.read_bytes <= 1 && report.progress.written_bytes <= 1);
            if report.status.closed() { break; }
            let report = session.drive(tiny).unwrap();
            assert!(report.status.failure.is_none(), "{:?}", report.status);
            assert!(report.progress.read_bytes <= 1 && report.progress.written_bytes <= 1);
        }
        assert_eq!(client.status().phase, PrecheckClientPhase::Complete);
        assert!(client.socket_fd().is_none());
        assert_eq!(client.drive(tiny).unwrap().progress, DriveProgress::default());
        client.take_response().unwrap()
    }

    #[test]
    fn authenticated_policy_reads_then_original_submit_still_require_review() {
        let (fixture, _) = Fixture::new(4, 4); let (wire, grant) = full_wire(&fixture, 4);
        let (socket, server) = UnixStream::pair().unwrap(); let policy = policy(&socket);
        let mut session = FilePrecheckPeerSession::new(policy, wire, MAX_PRECHECK_FRAME_BYTES, 2).unwrap();
        session.attach(server).unwrap();
        let files = fixture.root.files(); let reads = fixture.source.status().read_attempts;
        let mut client = PrecheckPolicyClient::connect_policy(socket, policy,
            PolicyRequest { query: 7, expected_policy_epoch: 0 }, 0).unwrap();
        let response = drive(&mut client, &mut session);
        assert!(matches!(response.result, Ok(Knowledge::Known { .. })));
        assert_eq!(grant.remaining_queries(), 3);
        assert_eq!(fixture.root.files(), files); assert_eq!(fixture.source.status().read_attempts, reads);
        session.disconnect();
        let ticket = fixture.actor.submit(7, &request(7, b"ok").proposal).unwrap();
        assert_eq!(fixture.actor.poll(&ticket), Knowledge::Pending { request: 7 });
        assert_eq!(fixture.supervisor.host().unwrap().inspect().control.ledger.stages[&1], ActionState::Reviewing);
        // Same session, same old grant; a reconnect cannot revive it after Submit.
        let (socket, server) = UnixStream::pair().unwrap(); session.attach(server).unwrap();
        let mut client = PrecheckPolicyClient::connect_policy(socket, policy,
            PolicyRequest { query: 8, expected_policy_epoch: 0 }, 0).unwrap();
        assert!(matches!(drive(&mut client, &mut session).result, Ok(Knowledge::Unknown { .. })));
        assert_eq!(grant.remaining_queries(), 3);
    }

    #[test]
    fn full_client_authenticates_before_bytes_and_checks_independent_epoch() {
        for variant in 0..3 {
            let (socket, mut server) = UnixStream::pair().unwrap(); let original = policy(&socket);
            let wrong = PeerPolicy::new(original.uid(), original.gid(),
                Some(if original.pid() == Some(1) { 2 } else { 1 })).unwrap();
            let error = PrecheckPolicyClient::connect_policy(socket,
                if variant == 0 { wrong } else { original },
                PolicyRequest { query: if variant == 2 { 0 } else { 7 }, expected_policy_epoch: 0 },
                if variant == 1 { 1 } else { 0 }).unwrap_err();
            assert_eq!(error, match variant {
                0 => PrecheckClientError::Peer(PeerRefusal::CredentialsRejected),
                1 => PrecheckClientError::ExpectedPolicyEpoch,
                _ => PrecheckClientError::Request(PrecheckWireError::MalformedRequest),
            });
            server.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut bytes = Vec::new(); server.read_to_end(&mut bytes).unwrap(); assert!(bytes.is_empty());
        }
    }

    #[test]
    fn authenticated_exact_server_cannot_be_upgraded_by_the_policy_client() {
        let (fixture, wire) = Fixture::new(2, 2);
        let (socket, server) = UnixStream::pair().unwrap(); let policy = policy(&socket);
        let mut session = FilePrecheckPeerSession::new(policy, wire, MAX_PRECHECK_FRAME_BYTES, 1).unwrap();
        session.attach(server).unwrap();
        let mut client = PrecheckPolicyClient::connect_policy(socket, policy,
            PolicyRequest { query: 7, expected_policy_epoch: 0 }, 0).unwrap();
        assert_eq!(drive(&mut client, &mut session).result,
            Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" }));
        assert_eq!(fixture.remaining_queries(), 1);
    }
}
