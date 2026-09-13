#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanRequest, FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    ReviewApplication, ReviewerConnection, ReviewerError, ReviewerPhase, ReviewerProgress,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{
    ReviewDecision, ReviewPacket, DECISION_BYTES, MAX_REVIEW_BYTES, OFFER_HEADER_BYTES, RECEIPT_BYTES, offer_frame_len,
};
use fa_reference::action::consequence::oversight::{CommitteeInput, human::HumanDisposition};
use fa_reference::action::{ElapsedTick, FrozenAction};
use fa_reference::Error;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;

fn pending(host: &mut FileOversight) -> (FrozenAction, CommitteeInput, FileHumanRequest) {
    let (action, input) = reviewed(host, 1, b"human-reviewed publication");
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(20)).unwrap();
    (action, input, request)
}
fn connection(host: &FileOversight, role: &FileHumanReviewer, request: FileHumanRequest)
    -> (ReviewerConnection<UnixStream>, UnixStream)
{
    let (server, peer) = UnixStream::pair().unwrap();
    peer.set_nonblocking(true).unwrap();
    (ReviewerConnection::from_unix(host, role, request, server, [17; 32]).unwrap(), peer)
}
fn read_available(peer: &mut UnixStream, bytes: &mut Vec<u8>) {
    let mut scratch = [0; 4096];
    match peer.read(&mut scratch) {
        Ok(count) => bytes.extend_from_slice(&scratch[..count]),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {},
        Err(error) => panic!("review socket read failed: {error}"),
    }
}
fn offer(server: &mut ReviewerConnection<UnixStream>, host: &mut FileOversight,
    role: &FileHumanReviewer, peer: &mut UnixStream) -> ReviewPacket
{
    let mut bytes = Vec::new();
    for _ in 0..512 {
        server.step(host, role, || panic!("offering must not read the clock or approve")).unwrap();
        read_available(peer, &mut bytes);
        if bytes.len() >= OFFER_HEADER_BYTES && bytes.len() == offer_frame_len(&bytes[..OFFER_HEADER_BYTES]).unwrap() {
            assert_eq!(server.phase(), ReviewerPhase::AwaitingDecision);
            return ReviewPacket::decode(&bytes).unwrap();
        }
    }
    panic!("bounded original offer did not complete");
}
fn apply(server: &mut ReviewerConnection<UnixStream>, host: &mut FileOversight,
    role: &FileHumanReviewer, peer: &mut UnixStream, packet: &ReviewPacket, decision: ReviewDecision) -> ReviewApplication
{
    peer.write_all(&packet.decision_frame(decision)).unwrap();
    for _ in 0..32 {
        match server.step(host, role, || ElapsedTick(2)).unwrap() {
            ReviewerProgress::Applied(applied) => return applied,
            ReviewerProgress::Progress | ReviewerProgress::Blocked => {},
            ReviewerProgress::Complete => panic!("completed without exposing the committed native result"),
        }
    }
    panic!("decision did not reach its original durable operation");
}

#[test]
fn exact_original_review_packet_and_remote_decision_feed_two_key_publication() {
    let root = Directory::new();
    let (mut host, role) = create(&root);
    let (action, input, request) = pending(&mut host);
    let (mut server, mut peer) = connection(&host, &role, request.clone());
    let before = host.revision();
    let packet = offer(&mut server, &mut host, &role, &mut peer);
    assert_eq!(host.revision(), before);
    assert_eq!(packet.action(), request.evidence().action());
    assert_eq!(packet.views(), input.views());
    assert_eq!(packet.binding().reviewer, role.reviewer_id());
    assert_eq!(packet.binding().request, 1001);
    assert_eq!(packet.binding().attempt, 1);
    assert_eq!(packet.clock_domain(), profile().delivery.clock_domain);
    assert_eq!(packet.control_sequence(), request.evidence().control_sequence());
    assert_eq!(packet.policy_generation(), request.evidence().policy_generation());
    assert_eq!(packet.expires_at(), ElapsedTick(20));
    assert!(!format!("{packet:?}").contains("human-reviewed publication"));
    let applied = apply(&mut server, &mut host, &role, &mut peer, &packet, ReviewDecision::Approve);
    let human = applied.approval.unwrap();
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Approved);
    let mut receipt = Vec::new();
    for _ in 0..32 {
        server.step(&mut host, &role, || panic!("receipt delivery cannot reapprove")).unwrap();
        read_available(&mut peer, &mut receipt);
        if receipt.len() == RECEIPT_BYTES { break; }
    }
    assert_eq!(packet.receipt(&receipt).unwrap(), applied.receipt);
    assert_eq!(server.committed(), Some(applied.receipt));
    assert_eq!(server.phase(), ReviewerPhase::Complete);
    assert!(host.publish(host.revision(), 1).is_err());
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(host.publish(host.revision(), 1).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let revision = host.revision();
    assert!(matches!(server.step(&mut host, &role, || panic!("terminal")), Ok(ReviewerProgress::Complete)));
    assert_eq!(host.revision(), revision);
}

#[test]
fn malformed_or_wrong_offer_decisions_cannot_mint_the_second_key() {
    for offset in [0, 8, 40, 48, 56, 64, 72] {
        let root = Directory::new();
        let (mut host, role) = create(&root);
        let (_, _, request) = pending(&mut host);
        let (mut server, mut peer) = connection(&host, &role, request);
        let packet = offer(&mut server, &mut host, &role, &mut peer);
        let mut frame = packet.decision_frame(ReviewDecision::Approve);
        frame[offset] = 0xff;
        peer.write_all(&frame).unwrap();
        let revision = host.revision();
        assert!(server.step(&mut host, &role, || panic!("invalid frame cannot admit time")).is_err());
        assert_eq!(server.phase(), ReviewerPhase::Failed);
        assert_eq!(server.committed(), None);
        assert_eq!(host.revision(), revision);
        assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn full_receipt_time_not_peer_time_controls_expiry() {
    let root = Directory::new();
    let (mut host, role) = create(&root);
    let (_, _, request) = pending(&mut host);
    let (mut server, mut peer) = connection(&host, &role, request);
    let packet = offer(&mut server, &mut host, &role, &mut peer);
    let frame = packet.decision_frame(ReviewDecision::Approve);
    peer.write_all(&frame[..DECISION_BYTES - 1]).unwrap();
    assert!(matches!(server.step(&mut host, &role, || panic!("partial frame")), Ok(ReviewerProgress::Progress)));
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
    peer.write_all(&frame[DECISION_BYTES - 1..]).unwrap();
    let error = server.step(&mut host, &role, || ElapsedTick(20)).unwrap_err();
    assert_eq!(error, ReviewerError::Journal(JournalError::Contract(Error::Stale)));
    assert_eq!(host.inspect().control.ledger.elapsed, Some(ElapsedTick(20)));
    assert_eq!(server.committed(), None);
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
}

#[test]
fn changed_full_input_refuses_approval_instead_of_presenting_a_repaired_summary() {
    let root = Directory::new();
    let (mut host, role) = create(&root);
    let (action, _, request) = pending(&mut host);
    let (mut server, mut peer) = connection(&host, &role, request);
    let packet = offer(&mut server, &mut host, &role, &mut peer);
    let changed = inputs(&action, b"changed real provider observation");
    host.record_inputs(host.revision(), 1, host.input_revision(1).unwrap(), changed).unwrap();
    peer.write_all(&packet.decision_frame(ReviewDecision::Approve)).unwrap();
    assert!(server.step(&mut host, &role, || ElapsedTick(2)).is_err());
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
    assert_eq!(server.committed(), None);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn reject_and_revoke_remain_available_without_fresh_evidence_or_clock() {
    for decision in [ReviewDecision::Reject, ReviewDecision::Revoke] {
        let root = Directory::new();
        let (mut host, role) = create(&root);
        let (action, input, request) = pending(&mut host);
        let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
        let human = if decision == ReviewDecision::Revoke {
            let revision = host.revision();
            Some(role.approve(&mut host, revision, &request).unwrap())
        } else { None };
        let (mut server, mut peer) = connection(&host, &role, request);
        let packet = offer(&mut server, &mut host, &role, &mut peer);
        host.inputs_unavailable(host.revision(), 1, host.input_revision(1).unwrap()).unwrap();
        peer.write_all(&packet.decision_frame(decision)).unwrap();
        let ReviewerProgress::Applied(applied) = server.step(&mut host, &role, || panic!("withdrawal must not depend on a clock")).unwrap()
            else { panic!("withdrawal was not committed"); };
        assert!(applied.approval.is_none());
        assert_eq!(applied.receipt.decision, decision);
        let expected = if decision == ReviewDecision::Reject { HumanDisposition::Rejected } else { HumanDisposition::Revoked };
        assert_eq!(host.human_status(1001).unwrap().disposition, expected);
        assert_eq!(host.inspect().control.ledger.reserved, 16);
        if let Some(human) = human {
            assert!(host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).is_err());
        }
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn lost_acknowledgment_does_not_reissue_a_committed_approval() {
    let root = Directory::new();
    let (mut host, role) = create(&root);
    let (_, _, request) = pending(&mut host);
    let (mut server, mut peer) = connection(&host, &role, request.clone());
    let packet = offer(&mut server, &mut host, &role, &mut peer);
    let applied = apply(&mut server, &mut host, &role, &mut peer, &packet, ReviewDecision::Approve);
    assert!(applied.approval.is_some());
    let revision = host.revision();
    drop(peer);
    assert!(server.step(&mut host, &role, || panic!("ack cannot reapprove")).is_err());
    assert_eq!(server.committed(), Some(applied.receipt));
    assert!(server.step(&mut host, &role, || panic!("failed channel cannot retry")).is_err());
    assert_eq!(host.revision(), revision);
    let (mut retry, mut peer) = connection(&host, &role, request);
    let packet = offer(&mut retry, &mut host, &role, &mut peer);
    assert_eq!(packet.disposition(), HumanDisposition::Approved);
    peer.write_all(&packet.decision_frame(ReviewDecision::Approve)).unwrap();
    assert!(retry.step(&mut host, &role, || ElapsedTick(2)).is_err());
    assert_eq!(host.revision(), revision);
    assert_eq!(retry.committed(), None);
}

#[test]
fn partial_decision_eof_never_implies_consent() {
    let root = Directory::new();
    let (mut host, role) = create(&root);
    let (_, _, request) = pending(&mut host);
    let (mut server, mut peer) = connection(&host, &role, request);
    let packet = offer(&mut server, &mut host, &role, &mut peer);
    peer.write_all(&packet.decision_frame(ReviewDecision::Approve)[..DECISION_BYTES - 1]).unwrap();
    server.step(&mut host, &role, || panic!("incomplete consent")).unwrap();
    drop(peer);
    assert_eq!(server.step(&mut host, &role, || panic!("EOF" )).unwrap_err(), ReviewerError::Io(io::ErrorKind::UnexpectedEof));
    assert_eq!(host.human_status(1001).unwrap().disposition, HumanDisposition::Pending);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn storage_failure_returns_neither_key_nor_success_receipt_and_recovery_revokes() {
    let root = Directory::new();
    let (mut host, role) = create(&root);
    let (_, _, request) = pending(&mut host);
    let (mut server, mut peer) = connection(&host, &role, request);
    let packet = offer(&mut server, &mut host, &role, &mut peer);
    peer.write_all(&packet.decision_frame(ReviewDecision::Approve)).unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"inert staged bytes").unwrap();
    let revision = host.revision();
    assert!(matches!(server.step(&mut host, &role, || ElapsedTick(1)), Err(ReviewerError::Journal(JournalError::Io(_)))));
    assert_eq!(host.revision(), revision);
    assert_eq!(server.committed(), None);
    assert!(host.storage_failure().is_some());
    assert_eq!(peer.read(&mut [0; 1]).unwrap(), 0);
    drop(host);
    let (recovered, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(recovered.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
    assert_eq!(recovered.inspect().executions, 0);
}

#[test]
fn foreign_roles_and_reopened_owners_cannot_adopt_an_old_reviewer_channel() {
    let first = Directory::new(); let second = Directory::new();
    let (mut host, role) = create(&first); let (mut other, other_role) = create(&second);
    let (_, _, request) = pending(&mut host);
    let (mut server, mut peer) = connection(&host, &role, request);
    assert_eq!(server.step(&mut other, &other_role, || ElapsedTick(1)).unwrap_err(), ReviewerError::Protocol(Error::Binding));
    assert_eq!(server.phase(), ReviewerPhase::Offering);
    let _ = offer(&mut server, &mut host, &role, &mut peer);
    drop(host);
    let (mut recovered, recovered_role) = FileOversight::open(first.store(), profile()).unwrap();
    assert_eq!(server.step(&mut recovered, &recovered_role, || ElapsedTick(2)).unwrap_err(), ReviewerError::Protocol(Error::Binding));
    assert_eq!(recovered.human_status(1001).unwrap().disposition, HumanDisposition::Revoked);
}

#[test]
fn offer_decoder_bounds_length_before_body_allocation_and_rejects_extra_bytes() {
    let root = Directory::new();
    let (mut host, role) = create(&root);
    let (_, _, request) = pending(&mut host);
    let (mut server, mut peer) = connection(&host, &role, request);
    let packet = offer(&mut server, &mut host, &role, &mut peer);
    let encoded = packet.encode().unwrap();
    assert_eq!(ReviewPacket::decode(&encoded).unwrap(), packet);
    let mut header: [u8; OFFER_HEADER_BYTES] = encoded[..OFFER_HEADER_BYTES].try_into().unwrap();
    header[8..].copy_from_slice(&(MAX_REVIEW_BYTES as u64 + 1).to_be_bytes());
    assert_eq!(offer_frame_len(&header).unwrap_err(), Error::Limit);
    assert_eq!(ReviewPacket::decode(&header).unwrap_err(), Error::Limit);
    let mut extra = encoded.clone(); extra.push(0);
    assert!(ReviewPacket::decode(&extra).is_err());
    assert!(ReviewPacket::decode(&encoded[..encoded.len() - 1]).is_err());
}
