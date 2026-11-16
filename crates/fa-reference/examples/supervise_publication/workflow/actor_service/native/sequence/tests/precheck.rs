//! Real resident native Message/Finish disclosure through authenticated peers.
//! Queries leave the original journal unchanged; publication still uses real
//! actor intake, process helpers, independent human approval and fresh source.
use super::*;
use crate::workflow::actor_service::precheck::Settings;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FilePrecheckOutcome,
    precheck_wire::{PrecheckRequest, PrecheckWireError, PrecheckWireResponse, client::PrecheckPeerClient},
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{
    actor_transport::DriveBudget,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot},
};
use std::{collections::BTreeMap, fs, path::Path};

pub(super) fn select(root: &Root, entries: &mut [Entry], epoch: u64) -> Vec<Option<PathBuf>> {
    entries.iter_mut().enumerate().map(|(index, entry)| {
        let socket = root.0.join(format!("check-{index}.sock"));
        entry.precheck = Some(exact_precheck::Selection::new(Settings::decode(format!(
            r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":{epoch},"disclosure_id":{},"query_limit":1,"exchange_limit":8,"candidate_limit":8,"connection_limit":8,"runtime_ms":15000}}"#,
            socket.display(), 81 + index,
        ).as_bytes()).unwrap()).unwrap());
        Some(socket)
    }).collect()
}

fn query(actor: &Profile, socket: &Path, proposal: ActorProposal, label: u64) -> PrecheckWireResponse {
    let epoch = proposal.expected_policy_epoch;
    let mut client = PrecheckPeerClient::connect(
        UnixStream::connect(socket).unwrap(), actor.supervisor,
        PrecheckRequest { query: label, proposal }, epoch,
    ).unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(25), "resident native precheck timed out");
        let report = client.drive(DriveBudget::default()).unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() { return response; }
        assert!(!report.status.closed(), "resident precheck closed without its original response");
        pause(1);
    }
}

pub(super) struct CheckedOutput {
    profiles: Vec<Profile>,
    sockets: Vec<Option<PathBuf>>,
    reviewer: PeerProfile,
    journal: PathBuf,
    epoch: u64,
    expected: Vec<FilePrecheckOutcome>,
    change_source: Option<(usize, PathBuf, Vec<u8>)>,
    pending: Vec<u8>,
    pub(super) documents: Vec<Vec<u8>>,
    clients: Vec<JoinHandle<WireResponse>>,
}

impl CheckedOutput {
    pub(super) fn new(entries: &[Entry], reader: &Reader, reviewer: &PeerProfile, epoch: u64) -> Self {
        Self {
            profiles: entries.iter().map(|entry| entry.actor.clone()).collect(),
            sockets: entries.iter().map(|entry| entry.precheck.as_ref().map(|selected| selected.settings.socket_path().to_owned())).collect(),
            reviewer: reviewer.clone(), journal: reader.store.join("delivery.bin"), epoch,
            expected: vec![FilePrecheckOutcome::PrecheckPassed; entries.len()],
            change_source: None, pending: Vec::new(), documents: Vec::new(), clients: Vec::new(),
        }
    }

    pub(super) fn join(self) -> Vec<WireResponse> {
        assert!(self.pending.is_empty());
        self.clients.into_iter().map(|client| client.join().unwrap()).collect()
    }
}

impl Write for CheckedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.pending.is_empty() { return Ok(()); }
        let index = self.documents.len();
        let document = std::mem::take(&mut self.pending);
        let profile = self.profiles[index].clone();
        let socket = self.sockets[index].clone();
        let preceding = self.sockets[..index].to_vec();
        let review = self.reviewer.socket(profile.request);
        let journal = self.journal.clone();
        let epoch = self.epoch;
        let expected = self.expected[index];
        let change = self.change_source.as_ref().filter(|(selected, _, _)| *selected == index).cloned();
        let submitted = document.clone();
        self.clients.push(thread::spawn(move || {
            assert!(preceding.iter().all(|path| path.as_ref().is_none_or(|path| !path.exists())),
                "the previous disclosure survived handoff");
            let Command::Submit { request, proposal } = decode_command(&submitted).unwrap() else {
                panic!("native output must remain a source-only Submit document");
            };
            assert_eq!(request, profile.request);
            assert_eq!(proposal.payload.len(), FileGeneratedTextActorPort::INTENT_BYTES);
            assert_eq!(proposal.expected_policy_epoch, epoch);
            if let Some(socket) = socket {
                let before = fs::read(&journal).unwrap();
                let response = query(&profile, &socket, proposal.clone(), 9);
                assert_eq!(response.query, Some(9));
                let Ok(Knowledge::Known { value, basis }) = response.result else {
                    panic!("expected the independently selected native release precheck: {response:?}");
                };
                assert_eq!(value, expected);
                assert_eq!((basis.query, basis.disclosure, basis.policy_epoch), (9, 81 + index as u64, epoch));
                assert_eq!(basis.observed_at, ElapsedTick(1000));
                assert_eq!(fs::read(&journal).unwrap(), before, "query changed numerical/source/authority state");
                assert!(!review.exists(), "precheck launched independent human review");
                let exhausted = query(&profile, &socket, proposal, 10);
                assert_eq!(exhausted.query, Some(10));
                assert_eq!(exhausted.result, Err(PrecheckWireError::Capacity), "reconnect replenished the grant");
                assert_eq!(fs::read(&journal).unwrap(), before);
            }
            if let Some((_, path, bytes)) = change { fs::write(path, bytes).unwrap(); }
            actor(profile, submitted).join().unwrap()
        }));
        self.documents.push(document);
        Ok(())
    }
}

fn poll(humans: &mut Humans, sockets: &[Option<PathBuf>]) {
    for (index, request) in REQUESTS.into_iter().enumerate() {
        if humans.reviewer.socket(request).exists() {
            assert!(sockets[index].as_ref().is_none_or(|path| !path.exists()),
                "the original disclosure must retire before independent human review");
        }
    }
    humans.poll();
}

#[test]
fn native_resident_exact_prechecks_cover_messages_finish_and_null_without_replenishing_queries() {
    for omit_middle in [false, true] {
        let root = Root::new();
        let (config, mut entries, reviewer) = selected(&root, true);
        let mut sockets = select(&root, &mut entries, 0);
        if omit_middle { entries[1].precheck = None; sockets[1] = None; }
        let reader = Reader::new(&config, &entries[0]);
        let mut output = CheckedOutput::new(&entries, &reader, &reviewer, 0);
        let profiles = output.profiles.clone();
        let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
        let result = run(config, entries, &reviewer, None, || {
            if omit_middle { assert!(!root.0.join("check-1.sock").exists()); }
            poll(&mut humans, &sockets);
            ElapsedTick(1000)
        }, &mut output);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(output.documents.len(), 3);
        let packets = humans.join();
        let replies = output.join();
        complete(&reader, &replies, &packets);
        assert!(sockets.iter().all(|path| path.as_ref().is_none_or(|path| !path.exists())));
        closed(&profiles, &reviewer);
    }
}

fn denied(config: &Config) -> Vec<u8> {
    EvidenceSnapshot::new(EvidenceIdentity {
        scope: config.profile.delivery.scope, source: 51, generation: 2,
    }, fa_reference::Snapshot {
        semantic_epoch: 2, complete: true, values: BTreeMap::from([(7, b"blocked".to_vec())]),
    }, BTreeMap::from([
        ("alpha".to_owned(), b"review this fixture: alpha".to_vec()),
        ("beta".to_owned(), b"review this fixture: beta".to_vec()),
    ])).unwrap().encode()
}

#[test]
fn later_native_precheck_pass_preserves_fresh_source_and_independent_human_boundaries() {
    for change_source in [false, true] {
        let root = Root::new();
        let (config, mut entries, reviewer) = selected(&root, true);
        let sockets = select(&root, &mut entries, 0);
        let reader = Reader::new(&config, &entries[0]);
        let mut output = CheckedOutput::new(&entries, &reader, &reviewer, 0);
        let profiles = output.profiles.clone();
        if change_source { output.change_source = Some((1, root.0.join("evidence.json"), denied(&config))); }
        let mut humans = Humans::new(&reviewer, &reader, [
            Some(ReviewDecision::Approve),
            (!change_source).then_some(ReviewDecision::Reject),
            None,
        ]);
        let result = run(config, entries, &reviewer, None, || {
            poll(&mut humans, &sockets);
            ElapsedTick(1000)
        }, &mut output);
        assert!(result.is_err(), "a precheck cannot replace the second release's fresh keys");
        assert_eq!(output.documents.len(), 2);
        assert_eq!(humans.seen, [true, !change_source, false]);
        let packets = humans.join();
        let replies = output.join();
        assert!(executed(&replies[0]));
        assert!(!executed(&replies[1]));
        let state = reader.stream();
        assert_eq!((state.publication.executions, state.publication.payload.as_slice()), (1, b"A".as_slice()));
        assert_eq!(state.publication.control.ledger.charged, packets[0].as_ref().unwrap().action().spec().units);
        assert_eq!(state.publication.control.ledger.reserved, 0);
        assert!(!state.confirmed.finished());
        assert_eq!(reader.progress(8).numerical.numerical.position, 8);
        assert!(sockets.iter().all(|path| path.as_ref().is_none_or(|path| !path.exists())));
        closed(&profiles, &reviewer);
    }
}

#[test]
fn later_native_exact_precheck_evaluates_the_original_cumulative_frame_and_charge() {
    for limit_units in [false, true] {
        let root = Root::new();
        let (mut config, mut entries, reviewer) = selected(&root, true);
        // The independently computed first frame fits exactly. A second one-
        // byte Message also carries the original message boundary and prefix.
        let first_size = fa_reference::action::consequence::delivery::stream::StreamView::empty(entries[0].inputs.stream)
            .encode_message("A").unwrap().len();
        config.profile.delivery.policy = Policy::new(1, vec![
            Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
            Predicate::PayloadAtMost(if limit_units { 512 } else { first_size }),
            Predicate::UnitsAtMost(if limit_units { first_size as u64 } else { 512 }),
            Predicate::All(vec![0, 1, 2]),
        ]).unwrap();
        let sockets = select(&root, &mut entries, 0);
        let reader = Reader::new(&config, &entries[0]);
        let mut output = CheckedOutput::new(&entries, &reader, &reviewer, 0);
        output.expected[1] = FilePrecheckOutcome::DeniedByPolicy;
        let profiles = output.profiles.clone();
        let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve), None, None]);
        let result = run(config, entries, &reviewer, None, || {
            poll(&mut humans, &sockets);
            ElapsedTick(1000)
        }, &mut output);
        assert!(result.is_err());
        assert_eq!(output.documents.len(), 2);
        assert_eq!(humans.seen, [true, false, false]);
        let packets = humans.join();
        let replies = output.join();
        assert!(executed(&replies[0]));
        assert!(matches!(replies[1].result, Ok(Knowledge::Known { value: ActorOutcome::Denied, .. })));
        assert_eq!(reader.publication().executions, 1);
        assert_eq!(reader.publication().control.ledger.charged, packets[0].as_ref().unwrap().action().spec().units);
        assert!(!reader.stream().confirmed.finished());
        closed(&profiles, &reviewer);
    }
}

struct PartialQueryOutput {
    original: Output,
    socket: PathBuf,
    reviewer: PeerProfile,
    stop: Option<JoinHandle<StopControlReceipt>>,
}

impl Write for PartialQueryOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.original.write(bytes) }
    fn flush(&mut self) -> io::Result<()> {
        if self.original.documents.len() != 1 { return self.original.flush(); }
        self.original.documents.push(std::mem::take(&mut self.original.pending));
        let socket = self.socket.clone();
        let reviewer = self.reviewer.clone();
        self.stop = Some(thread::spawn(move || {
            let mut partial = UnixStream::connect(socket).unwrap();
            partial.write_all(b"{").unwrap();
            let receipt = stopping(reviewer).join().unwrap();
            drop(partial);
            receipt
        }));
        Ok(())
    }
}

#[test]
fn a_partial_later_native_precheck_never_blocks_the_original_stop_or_admits_a_successor() {
    let root = Root::new();
    let (config, mut entries, reviewer) = selected(&root, true);
    let sockets = select(&root, &mut entries, 0);
    entries[0].precheck = None;
    let reader = Reader::new(&config, &entries[0]);
    let mut output = PartialQueryOutput {
        original: Output::new(&entries), socket: sockets[1].clone().unwrap(),
        reviewer: reviewer.clone(), stop: None,
    };
    let profiles = output.original.profiles.clone();
    let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve), None, None]);
    let result = run(config, entries, &reviewer, None, || {
        humans.poll();
        ElapsedTick(1000)
    }, &mut output);
    assert!(result.is_err());
    assert_eq!(output.original.documents.len(), 2);
    assert_eq!(humans.seen, [true, false, false]);
    let receipt = output.stop.take().unwrap().join().unwrap();
    assert!(receipt.acknowledged() && receipt.drained());
    assert_eq!(receipt.binding.operation, REQUESTS[0]);
    let packets = humans.join();
    assert!(output.original.join().iter().all(executed));
    let state = reader.stream();
    assert_eq!((state.publication.executions, state.publication.payload.as_slice()), (1, b"A".as_slice()));
    assert_eq!(state.publication.control.ledger.charged, packets[0].as_ref().unwrap().action().spec().units);
    assert_eq!(state.publication.control.ledger.reserved, 0);
    assert!(state.publication.stop.is_some());
    assert!(!state.confirmed.finished());
    assert!(sockets.iter().all(|path| path.as_ref().is_none_or(|path| !path.exists())));
    closed(&profiles, &reviewer);
}
