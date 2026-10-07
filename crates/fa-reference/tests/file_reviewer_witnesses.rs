//! Real policy-derived dependencies, native human requests, and original dispatch.
#![cfg(unix)]
#[path = "support/file_oversight.rs"]
mod fixture;
use fixture::*;
use fa_reference::{Error, ReadWitness};
use fa_reference::action::{ElapsedTick, FrozenAction};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanPermit, FileHumanRequest, FileHumanReviewer, FileOversight,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::{
    ReviewApplication, ReviewerConnection, ReviewerProgress,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::client::{
    ReviewClientPhase, ReviewerClient, ReviewerExpectation,
};
use fa_reference::action::consequence::delivery::persistent::observed::reviewer::wire::{
    OFFER_HEADER_BYTES, ReviewDecision, ReviewPacket,
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::consequence::oversight::human::HumanDisposition;
use std::io::Write;
use std::os::unix::net::UnixStream;

struct NativeReview {
    host: FileOversight,
    reviewer: FileHumanReviewer,
    request: Option<FileHumanRequest>,
    action: FrozenAction,
    inputs: CommitteeInput,
    _root: Directory,
}
impl NativeReview {
    fn new(witnessed: bool) -> Self {
        let root = Directory::new();
        let mut p = profile();
        let nodes = if witnessed {
            vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
                Predicate::Absent { key: 8 }, Predicate::EmptyRange { start: 20, end: 30 },
                Predicate::PayloadAtMost(128), Predicate::All(vec![0, 1, 2, 3])]
        } else { vec![Predicate::PayloadAtMost(128)] };
        p.delivery.policy = Policy::new(1, nodes).unwrap();
        let (mut host, reviewer) = FileOversight::create(root.store(), p).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let (action, inputs) = reviewed(&mut host, 1, b"witnessed publication");
        let request = host.request_human_approval(host.revision(), 1001, 1, &inputs, ElapsedTick(31)).unwrap();
        Self { host, reviewer, request: Some(request), action, inputs, _root: root }
    }
    fn packet(&self) -> ReviewPacket {
        ReviewPacket::capture(self.request.as_ref().unwrap(), profile().delivery.clock_domain,
            self.host.revision(), HumanDisposition::Pending, [19; 32]).unwrap()
    }
    fn approve(&mut self) -> FileHumanPermit {
        let (server, client) = UnixStream::pair().unwrap();
        // This test provisions a private local channel; Linux peer authentication
        // remains covered independently by file_reviewer_peer, not faked here.
        let mut server = ReviewerConnection::from_unix(&self.host, &self.reviewer,
            self.request.take().unwrap(), server, [19; 32]).unwrap();
        let mut client = ReviewerClient::from_unix(client, expected()).unwrap();
        let before = self.host.inspect();
        for _ in 0..128 {
            server.step(&mut self.host, &self.reviewer, || panic!("no human decision yet")).unwrap();
            client.step().unwrap();
            if client.phase() == ReviewClientPhase::NeedsDecision { break; }
        }
        assert_eq!(client.phase(), ReviewClientPhase::NeedsDecision);
        assert_eq!(client.packet().unwrap().action(), &self.action);
        assert_eq!(client.packet().unwrap().views(), self.inputs.views());
        assert_eq!(self.host.inspect(), before);
        client.respond(ReviewDecision::Approve).unwrap();
        let application = (0..128).find_map(|_| {
            client.step().unwrap();
            match server.step(&mut self.host, &self.reviewer, || ElapsedTick(1)).unwrap() {
                ReviewerProgress::Applied(application) => Some(application),
                _ => None,
            }
        }).expect("native approval within bounded pump");
        let ReviewApplication { receipt, approval } = application;
        for _ in 0..128 {
            assert!(!matches!(server.step(&mut self.host, &self.reviewer,
                || panic!("receipt must not repeat approval")).unwrap(), ReviewerProgress::Applied(_)));
            client.step().unwrap();
            if client.phase() == ReviewClientPhase::Complete { break; }
        }
        assert_eq!(client.receipt(), Some(receipt));
        approval.unwrap()
    }
}
fn expected() -> ReviewerExpectation {
    ReviewerExpectation { reviewer: profile().human.reviewer_id,
        scope: profile().delivery.scope, clock_domain: profile().delivery.clock_domain }
}
fn witness_offset(packet: &ReviewPacket) -> usize {
    // Header; ten request integers; disposition; action version; scope; target;
    // payload length/bytes; policy epoch, deadline and units. No struct-layout ABI.
    OFFER_HEADER_BYTES + 10 * 8 + 1 + 4 + 5 * 8 + 1 + 5 * 8 + 4
        + packet.action().spec().payload.len() + 3 * 8
}
fn fix_length(bytes: &mut [u8]) {
    let length = bytes.len() as u64;
    bytes[8..16].copy_from_slice(&length.to_be_bytes());
}

#[test]
fn original_compiled_witnesses_round_trip_with_the_entire_action_and_helper_views() {
    let f = NativeReview::new(true);
    let required = &f.action.spec().required_witnesses;
    assert_eq!(required.len(), 3);
    assert!(required.contains(&ReadWitness::Exact { key: 7, value: Some(b"ok".to_vec()) }));
    assert!(required.contains(&ReadWitness::Exact { key: 8, value: None }));
    assert!(required.contains(&ReadWitness::EmptyRange { start: 20, end: 30 }));
    let before = f.host.inspect(); let packet = f.packet(); let bytes = packet.encode().unwrap();
    assert_eq!(&bytes[..8], b"FAHRVW\0\x02");
    let decoded = ReviewPacket::decode(&bytes).unwrap();
    assert_eq!(decoded, packet); assert_eq!(decoded.action(), &f.action);
    assert_eq!(decoded.views(), f.inputs.views());
    assert_eq!(decoded.encode().unwrap(), bytes); assert_eq!(f.host.inspect(), before);
}

#[test]
fn legacy_empty_actions_remain_v1_and_an_empty_v2_alias_is_not_accepted() {
    let f = NativeReview::new(false); let packet = f.packet();
    assert!(packet.action().spec().required_witnesses.is_empty());
    let bytes = packet.encode().unwrap();
    assert_eq!(&bytes[..8], b"FAHRVW\0\x01");
    assert_eq!(ReviewPacket::decode(&bytes).unwrap(), packet);
    assert_eq!(ReviewPacket::decode(&bytes).unwrap().encode().unwrap(), bytes);
    let mut alias = bytes; alias[7] = 2;
    alias.splice(witness_offset(&packet)..witness_offset(&packet), 0_u32.to_be_bytes());
    fix_length(&mut alias);
    assert_eq!(ReviewPacket::decode(&alias), Err(Error::Binding));
}

#[test]
fn witnesses_cannot_be_silently_downgraded_truncated_or_given_an_unbounded_count() {
    let f = NativeReview::new(true); let packet = f.packet(); let bytes = packet.encode().unwrap();
    let mut downgraded = bytes.clone(); downgraded[7] = 1;
    assert!(ReviewPacket::decode(&downgraded).is_err());
    let mut count = bytes.clone();
    count[witness_offset(&packet)..witness_offset(&packet) + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(ReviewPacket::decode(&count), Err(Error::Limit));
    for end in [OFFER_HEADER_BYTES, witness_offset(&packet), bytes.len() - 32, bytes.len() - 1] {
        let mut truncated = bytes[..end].to_vec(); fix_length(&mut truncated);
        assert!(ReviewPacket::decode(&truncated).is_err());
    }
    let mut trailing = bytes; trailing.push(0); fix_length(&mut trailing);
    assert!(ReviewPacket::decode(&trailing).is_err());
}

#[test]
fn fragmented_witnessed_offer_never_exposes_a_partial_packet_or_chooses_a_decision() {
    let f = NativeReview::new(true); let packet = f.packet(); let bytes = packet.encode().unwrap();
    let before = f.host.inspect(); let (mut sender, receiver) = UnixStream::pair().unwrap();
    let mut client = ReviewerClient::from_unix(receiver, expected()).unwrap();
    for (index, byte) in bytes.iter().enumerate() {
        sender.write_all(&[*byte]).unwrap(); client.step().unwrap();
        if index + 1 < bytes.len() {
            assert!(client.packet().is_none());
            assert_eq!(client.respond(ReviewDecision::Approve), Err(Error::WrongState));
        }
    }
    assert_eq!(client.phase(), ReviewClientPhase::NeedsDecision);
    assert_eq!(client.packet(), Some(&packet)); assert_eq!(client.decision(), None);
    assert!(!client.outcome_unknown()); assert_eq!(f.host.inspect(), before);
}

#[test]
fn human_review_of_required_witnesses_reaches_original_two_key_publication() {
    let mut f = NativeReview::new(true); let human = f.approve();
    assert_eq!(f.host.inspect().executions, 0);
    assert!(f.host.publish(f.host.revision(), 1).is_err());
    let automatic = f.host.authorize(f.host.revision(), 1, &f.inputs, snapshot()).unwrap();
    let mut unrelated = snapshot(); unrelated.values.insert(999, b"unrelated".to_vec());
    f.host.dispatch(f.host.revision(), &automatic, &human, &f.action, &f.inputs, unrelated).unwrap();
    assert_eq!(f.host.publish(f.host.revision(), 1).unwrap(), EndpointOutcome::Executed { resulting_version: 2 });
    f.host.reconcile(f.host.revision(), 1).unwrap();
    assert_eq!(f.host.inspect().executions, 1); assert_eq!(f.host.inspect().control.ledger.charged, 16);
}

#[test]
fn reviewed_dependencies_still_refuse_changed_values_phantoms_and_incomplete_snapshots() {
    for mutation in 0..6 {
        let mut f = NativeReview::new(true); let human = f.approve();
        let automatic = f.host.authorize(f.host.revision(), 1, &f.inputs, snapshot()).unwrap();
        let mut changed = snapshot();
        match mutation {
            0 => { changed.values.remove(&7); }
            1 => { changed.values.insert(7, b"changed".to_vec()); }
            2 => { changed.values.insert(8, Vec::new()); }
            3 => { changed.values.insert(25, b"phantom".to_vec()); }
            4 => changed.semantic_epoch += 1,
            _ => changed.complete = false,
        }
        assert!(f.host.dispatch(f.host.revision(), &automatic, &human, &f.action, &f.inputs, changed).is_err());
        assert_eq!(f.host.inspect().executions, 0);
        assert_eq!(f.host.inspect().control.ledger.reserved, 16);
    }
}
