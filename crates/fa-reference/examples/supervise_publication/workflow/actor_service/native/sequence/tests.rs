//! Real resident native inference, original helper processes and authenticated
//! actor/reviewer/Stop sockets. Synthetic weights and votes establish workflow
//! behavior only; they do not qualify a deployment model or detector.
#![cfg(target_os = "linux")]

use super::super::recipe;
use super::*;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, tokenizer::ByteBpe,
};
use fa_reference::action::consequence::delivery::{
    persistent::{
        FileDeliverySnapshot,
        observed::{
            FileOversightProfile,
            decoder::{FileDecoderConfig, text::progress::FileTextProgressSnapshot},
            reviewer::{
                client::ReviewClientProgress,
                peer::control::{StopClientProgress, StopControlReceipt},
                wire::{ReviewDecision, ReviewPacket},
            },
            stream::FileStreamSnapshot,
        },
    },
    stream::{ReleaseFrame, StreamProfile},
};
use fa_reference::action::consequence::oversight::{
    actor::{ActorOutcome, BasisSource, Knowledge, UnknownReason},
    actor_wire::{
        Command, WireResponse,
        client::{ActorExchange, ClientIoBudget, ClientIoLimits, ClientProgress},
        decode_command,
    },
};
use std::{
    collections::BTreeSet,
    io::{self, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    thread::{self, JoinHandle},
};

mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/supervise_publication/workflow/actor_service/native/tests/fixture.rs"
    ));
}
use fixture::{Root, inputs, profiles};

#[path = "tests/checked.rs"]
mod checked;

const REQUESTS: [u64; 3] = [91, 92, 93];

fn selected(root: &Root, finish: bool) -> (Config, Vec<Entry>, PeerProfile) {
    let config = fixture::configured(root);
    let (actor, reviewer) = profiles(root, &config);
    let first = inputs(root, &config);
    let mut second = recipe::load(
        &root.0.join("recipe.json"),
        config.profile.delivery.scope.tenant,
        config.profile.delivery.limits.bytes,
    )
    .unwrap();
    second.generation = 8;
    let mut third = recipe::load(
        &root.0.join("recipe.json"),
        config.profile.delivery.scope.tenant,
        config.profile.delivery.limits.bytes,
    )
    .unwrap();
    third.generation = if finish { 8 } else { 9 };
    let entries = [first, second, third]
        .into_iter()
        .enumerate()
        .map(|(index, inputs)| {
            let mut actor = actor.clone();
            actor.request = REQUESTS[index];
            actor.socket = root.0.join(format!("actor-{}.sock", actor.request));
            Entry {
                actor,
                inputs,
                release: if finish && index == 2 {
                    Release::Finish
                } else {
                    Release::Message
                },
            }
        })
        .collect();
    (config, entries, reviewer)
}

/// Read-only replay never opens a replacement writer or appends a recovery
/// Fence. The original generation is kept independently of the service owner.
struct Reader {
    store: PathBuf,
    profile: FileOversightProfile,
    decoder: FileDecoderConfig,
    tokenizer: ByteBpe,
    stream: StreamProfile,
}

impl Reader {
    fn new(config: &Config, entry: &Entry) -> Self {
        Self {
            store: config.store.clone(),
            profile: config.profile.clone(),
            decoder: entry.inputs.decoder.clone(),
            tokenizer: entry.inputs.tokenizer.clone(),
            stream: entry.inputs.stream,
        }
    }

    fn publication(&self) -> FileDeliverySnapshot {
        FileOversight::read_publication(&self.store, &self.profile).unwrap()
    }

    fn stream(&self) -> FileStreamSnapshot {
        FileOversight::read_stream_publication(&self.store, &self.profile, self.stream).unwrap()
    }

    fn progress(&self, generation: u64) -> FileTextProgressSnapshot {
        FileOversight::read_decoder_text_progress(
            &self.store,
            &self.profile,
            &self.decoder,
            &self.tokenizer,
            generation,
        )
        .unwrap()
    }

    fn no_generation(&self, generation: u64) {
        assert!(matches!(
            FileOversight::read_decoder_text_progress(
                &self.store,
                &self.profile,
                &self.decoder,
                &self.tokenizer,
                generation,
            ),
            Err(JournalError::Contract(Error::Missing))
        ));
    }
}

fn actor(profile: Profile, document: Vec<u8>) -> JoinHandle<WireResponse> {
    thread::spawn(move || {
        let mut command = decode_command(&document).unwrap();
        assert!(matches!(&command, Command::Submit { request, proposal }
            if *request == profile.request
                && proposal.payload.len() == FileGeneratedTextActorPort::INTENT_BYTES));
        let mut socket = UnixStream::connect(&profile.socket).unwrap();
        profile.supervisor.verify(&socket).unwrap();
        socket.set_nonblocking(true).unwrap();
        let mut budget = ClientIoBudget::new(ClientIoLimits::default()).unwrap();
        let started = Instant::now();
        loop {
            let mut exchange = ActorExchange::new(socket, command, &mut budget).unwrap();
            let response = loop {
                assert!(started.elapsed() < Duration::from_secs(30));
                match exchange.step(&mut budget).unwrap() {
                    ClientProgress::Response(response) => break response,
                    ClientProgress::Complete => panic!("missing original actor response"),
                    _ => pause(1),
                }
            };
            if !matches!(
                response.result,
                Ok(Knowledge::Pending { .. }
                    | Knowledge::Unknown {
                        reason: UnknownReason::OutcomeUnknown
                    })
            ) {
                return response;
            }
            socket = exchange.into_stream().unwrap();
            command = Command::Poll {
                request: profile.request,
            };
            pause(1);
        }
    })
}

#[derive(Clone, Copy)]
enum Substitution {
    FinishForSecondMessage,
    MessageForFinalFinish,
}

impl Substitution {
    fn index(self) -> usize {
        match self {
            Self::FinishForSecondMessage => 1,
            Self::MessageForFinalFinish => 2,
        }
    }
}

/// The service emits references only. Each flush starts an independently
/// authenticated client; no test helper invokes a native admission method.
struct Output {
    profiles: Vec<Profile>,
    pending: Vec<u8>,
    documents: Vec<Vec<u8>>,
    clients: Vec<JoinHandle<WireResponse>>,
    substitution: Option<Substitution>,
    substituted: bool,
}

impl Output {
    fn new(entries: &[Entry]) -> Self {
        Self {
            profiles: entries.iter().map(|entry| entry.actor.clone()).collect(),
            pending: Vec::new(),
            documents: Vec::new(),
            clients: Vec::new(),
            substitution: None,
            substituted: false,
        }
    }

    fn client_document(&mut self, document: &[u8]) -> Vec<u8> {
        let Some(substitution) = self
            .substitution
            .filter(|kind| kind.index() == self.documents.len())
        else {
            return document.to_vec();
        };
        let Command::Submit { request, proposal } = decode_command(document).unwrap() else {
            panic!("service must emit its original Submit reference");
        };
        let substitute = match substitution {
            Substitution::FinishForSecondMessage => {
                assert_ne!(
                    u64::from_be_bytes(proposal.payload[16..24].try_into().unwrap()),
                    0
                );
                FileGeneratedTextActorPort::encode_finish(
                    request,
                    proposal.target,
                    proposal.expected_policy_epoch,
                    proposal.deadline,
                )
                .unwrap()
            }
            Substitution::MessageForFinalFinish => {
                assert_eq!(&proposal.payload[16..32], &[0; 16]);
                let Command::Submit {
                    proposal: preceding,
                    ..
                } = decode_command(&self.documents[1]).unwrap()
                else {
                    panic!("the preceding actual Message supplied its retained source identity");
                };
                // The already completed numerical receipt remains current.
                // Reusing it under the new request is a well-formed native
                // Message, but it is not the independently selected Finish.
                FileGeneratedTextActorPort::encode_message(&FileTextMessageRequest {
                    request,
                    generation: u64::from_be_bytes(preceding.payload[16..24].try_into().unwrap()),
                    generation_revision: u64::from_be_bytes(
                        preceding.payload[24..32].try_into().unwrap(),
                    ),
                    target: proposal.target,
                    policy_epoch: proposal.expected_policy_epoch,
                    deadline: proposal.deadline,
                })
                .unwrap()
            }
        };
        assert_ne!(substitute.payload, proposal.payload);
        assert_eq!(substitute.target, proposal.target);
        assert_eq!(
            substitute.expected_policy_epoch,
            proposal.expected_policy_epoch
        );
        assert_eq!(substitute.deadline, proposal.deadline);
        self.substituted = true;
        encode_command(&Command::Submit {
            request,
            proposal: substitute,
        })
        .unwrap()
    }

    fn join(self) -> Vec<WireResponse> {
        assert!(self.pending.is_empty());
        self.clients
            .into_iter()
            .map(|client| client.join().unwrap())
            .collect()
    }
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.pending.is_empty() {
            let document = std::mem::take(&mut self.pending);
            let profile = self.profiles[self.documents.len()].clone();
            let submitted = self.client_document(&document);
            self.clients.push(actor(profile, submitted));
            self.documents.push(document);
        }
        Ok(())
    }
}

fn human(
    reviewer: PeerProfile,
    request: u64,
    store: PathBuf,
    profile: FileOversightProfile,
    prior_executions: u64,
    decision: ReviewDecision,
) -> JoinHandle<ReviewPacket> {
    thread::spawn(move || {
        let mut client = reviewer.connect_client(request).unwrap();
        let mut packet = None;
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(30));
            match client.step().unwrap() {
                ReviewClientProgress::NeedsDecision => {
                    // Even later numerical output and fresh helper agreement
                    // cannot replace this entry's independent human key.
                    assert_eq!(
                        FileOversight::read_publication(&store, &profile)
                            .unwrap()
                            .executions,
                        prior_executions
                    );
                    packet = Some(client.packet().unwrap().clone());
                    client.respond(decision).unwrap();
                }
                ReviewClientProgress::Complete => return packet.unwrap(),
                _ => pause(1),
            }
        }
    })
}

struct Humans {
    reviewer: PeerProfile,
    store: PathBuf,
    profile: FileOversightProfile,
    decisions: [Option<ReviewDecision>; 3],
    seen: [bool; 3],
    clients: [Option<JoinHandle<ReviewPacket>>; 3],
}

impl Humans {
    fn new(
        reviewer: &PeerProfile,
        reader: &Reader,
        decisions: [Option<ReviewDecision>; 3],
    ) -> Self {
        Self {
            reviewer: reviewer.clone(),
            store: reader.store.clone(),
            profile: reader.profile.clone(),
            decisions,
            seen: [false; 3],
            clients: std::array::from_fn(|_| None),
        }
    }

    fn poll(&mut self) {
        for (index, request) in REQUESTS.into_iter().enumerate() {
            if self.reviewer.socket(request).exists() {
                self.seen[index] = true;
                if self.clients[index].is_none() {
                    if let Some(decision) = self.decisions[index] {
                        self.clients[index] = Some(human(
                            self.reviewer.clone(),
                            request,
                            self.store.clone(),
                            self.profile.clone(),
                            index as u64,
                            decision,
                        ));
                    }
                }
            }
        }
    }

    fn join(self) -> [Option<ReviewPacket>; 3] {
        self.clients
            .map(|client| client.map(|client| client.join().unwrap()))
    }
}

fn executed(response: &WireResponse) -> bool {
    matches!(
        &response.result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    )
}

fn closed(profiles: &[Profile], reviewer: &PeerProfile) {
    for profile in profiles {
        assert!(!profile.socket.exists());
        assert!(!reviewer.socket(profile.request).exists());
        assert!(!crate::workflow::control::socket_path(reviewer, profile.request).exists());
    }
}

fn complete(reader: &Reader, replies: &[WireResponse], packets: &[Option<ReviewPacket>; 3]) {
    assert_eq!(replies.len(), 3);
    assert!(replies.iter().all(executed));
    let mut sessions = BTreeSet::new();
    let mut charged = 0;
    for (index, packet) in packets.iter().enumerate() {
        let packet = packet
            .as_ref()
            .expect("every release has a fresh human review");
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.action().spec().policy_epoch, 0);
        assert!(packet.action().spec().deadline <= ElapsedTick(16_000));
        assert_eq!(packet.views().len(), 2);
        charged += packet.action().spec().units;
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), vec!["A"; index.min(2)].as_slice());
        assert_eq!(frame.message(), if index < 2 { Some("A") } else { None });
        assert_eq!(frame.is_finish(), index == 2);
    }
    let stream = reader.stream();
    assert_eq!(stream.publication.executions, 3);
    assert_eq!(stream.publication.payload, b"AA");
    assert_eq!(stream.publication.control.ledger.epoch, 0);
    assert_eq!(stream.publication.control.ledger.charged, charged);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert!(stream.publication.stop.is_none());
    assert_eq!(stream.confirmed, stream.published);
    assert!(stream.confirmed.finished());
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert!(stream.pending.is_none());
    let progress = reader.progress(8);
    assert_eq!(progress.text.command().position(), 4);
    assert_eq!(progress.text.generation_revision(), 4);
    assert_eq!(
        progress.text.finish(),
        Some(Ok(GenerationFinish::StopToken))
    );
    // The second message resumes the SAME cache/RNG. Finish uses its confirmed
    // prefix without beginning a third generation or consuming another draw.
    assert_eq!(progress.numerical.numerical.position, 8);
    assert_eq!(progress.numerical.numerical.sampled_draws, 4);
    reader.no_generation(9);
}

#[test]
fn resident_native_messages_and_finish_preserve_one_owner_and_fresh_review_keys() {
    let root = Root::new();
    let (config, entries, reviewer) = selected(&root, true);
    let reader = Reader::new(&config, &entries[0]);
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
    let original_stop = crate::workflow::control::socket_path(&reviewer, REQUESTS[0]);
    let mut saw_second = false;
    let result = run(
        config,
        entries,
        &reviewer,
        None,
        || {
            humans.poll();
            if reviewer.socket(REQUESTS[1]).exists() {
                saw_second = true;
                assert!(original_stop.exists());
                assert!(!crate::workflow::control::socket_path(&reviewer, REQUESTS[1]).exists());
            }
            ElapsedTick(1000)
        },
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(saw_second);
    assert_eq!(output.documents.len(), 3);
    let packets = humans.join();
    let replies = output.join();
    complete(&reader, &replies, &packets);
    closed(&profiles, &reviewer);
}

#[test]
fn a_later_human_rejection_preserves_the_prefix_without_generating_a_successor() {
    let root = Root::new();
    let (config, entries, reviewer) = selected(&root, false);
    let reader = Reader::new(&config, &entries[0]);
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let mut humans = Humans::new(
        &reviewer,
        &reader,
        [
            Some(ReviewDecision::Approve),
            Some(ReviewDecision::Reject),
            None,
        ],
    );
    let result = run(
        config,
        entries,
        &reviewer,
        None,
        || {
            humans.poll();
            ElapsedTick(1000)
        },
        &mut output,
    );
    assert!(
        result.is_err(),
        "a refused second release cannot complete the sequence"
    );
    assert_eq!(output.documents.len(), 2);
    assert_eq!(humans.seen, [true, true, false]);
    let packets = humans.join();
    let replies = output.join();
    assert!(executed(&replies[0]));
    assert!(!executed(&replies[1]));
    let stream = reader.stream();
    assert_eq!(stream.publication.executions, 1);
    assert_eq!(stream.publication.payload, b"A");
    assert_eq!(stream.publication.control.ledger.epoch, 0);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert_eq!(
        stream.publication.control.ledger.charged,
        packets[0].as_ref().unwrap().action().spec().units
    );
    assert!(stream.publication.stop.is_none());
    assert_eq!(stream.confirmed, stream.published);
    assert!(!stream.confirmed.finished());
    assert_eq!(reader.progress(8).numerical.numerical.position, 8);
    reader.no_generation(9);
    closed(&profiles, &reviewer);
}

#[test]
fn an_admitted_native_source_of_the_wrong_release_kind_cannot_complete_a_scheduled_entry() {
    for substitution in [
        Substitution::FinishForSecondMessage,
        Substitution::MessageForFinalFinish,
    ] {
        let root = Root::new();
        let finish = matches!(substitution, Substitution::MessageForFinalFinish);
        let (config, entries, reviewer) = selected(&root, finish);
        let reader = Reader::new(&config, &entries[0]);
        let mut output = Output::new(&entries);
        output.substitution = Some(substitution);
        let profiles = output.profiles.clone();
        // Approve any actual review offer, including an incorrectly offered
        // substituted frame. The negative cannot pass merely because a test
        // human withheld the key that would expose a missing schedule guard.
        let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
        let result = run(
            config,
            entries,
            &reviewer,
            None,
            || {
                humans.poll();
                ElapsedTick(1000)
            },
            &mut output,
        );
        assert!(
            output.substituted,
            "the authenticated actor must send a valid alternative reference"
        );
        assert!(
            result.is_err(),
            "a different release cannot count as the scheduled success"
        );
        let mismatch = substitution.index();
        assert_eq!(output.documents.len(), mismatch + 1);
        assert!(!humans.seen[mismatch]);
        assert!(humans.seen[mismatch..].iter().all(|seen| !seen));
        let packets = humans.join();
        let replies = output.join();
        assert!(replies[..mismatch].iter().all(executed));
        assert!(
            matches!(
                &replies[mismatch].result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch,
                    basis,
                }) if basis.source == BasisSource::ControlLedger
                    && basis.request == REQUESTS[mismatch]
            ),
            "the original source gateway must admit the valid alternative before the schedule guard cancels it: {:?}",
            replies[mismatch]
        );
        let stream = reader.stream();
        assert_eq!(stream.publication.executions, mismatch as u64);
        assert_eq!(stream.publication.payload, vec![b'A'; mismatch]);
        assert_eq!(stream.publication.control.ledger.reserved, 0);
        assert_eq!(
            stream.publication.control.ledger.charged,
            packets
                .iter()
                .flatten()
                .map(|packet| packet.action().spec().units)
                .sum::<u64>()
        );
        assert_eq!(stream.confirmed, stream.published);
        assert!(!stream.confirmed.finished());
        assert!(stream.pending.is_none());
        assert_eq!(reader.progress(8).numerical.numerical.position, 8);
        assert_eq!(reader.progress(8).numerical.numerical.sampled_draws, 4);
        // The Message->Finish substitution also leaves a later scheduled
        // Message untouched; the Finish->Message case cannot resample one.
        reader.no_generation(9);
        closed(&profiles, &reviewer);
    }
}

fn stopping(reviewer: PeerProfile) -> JoinHandle<StopControlReceipt> {
    thread::spawn(move || {
        let mut client = reviewer.connect_stop_client(REQUESTS[0]).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(30));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => pause(1),
            }
        }
    })
}

#[test]
fn original_stop_remains_live_at_a_later_native_human_boundary() {
    let root = Root::new();
    let (config, entries, reviewer) = selected(&root, false);
    let reader = Reader::new(&config, &entries[0]);
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let mut humans = Humans::new(
        &reviewer,
        &reader,
        [Some(ReviewDecision::Approve), None, None],
    );
    let mut controller = None;
    let result = run(
        config,
        entries,
        &reviewer,
        None,
        || {
            humans.poll();
            if controller.is_none() && reviewer.socket(REQUESTS[1]).exists() {
                assert!(crate::workflow::control::socket_path(&reviewer, REQUESTS[0]).exists());
                assert!(!crate::workflow::control::socket_path(&reviewer, REQUESTS[1]).exists());
                let before = reader.publication();
                assert_eq!(before.executions, 1);
                assert_eq!(before.control.ledger.epoch, 0);
                controller = Some(stopping(reviewer.clone()));
            }
            ElapsedTick(1000)
        },
        &mut output,
    );
    assert!(
        result.is_err(),
        "Stop must end the incomplete resident sequence"
    );
    let receipt = controller
        .expect("second request reached its human boundary")
        .join()
        .unwrap();
    assert!(receipt.acknowledged() && receipt.drained());
    assert_eq!(receipt.binding.operation, REQUESTS[0]);
    assert_eq!(output.documents.len(), 2);
    assert!(!humans.seen[2]);
    let packets = humans.join();
    let replies = output.join();
    assert!(executed(&replies[0]));
    assert!(!executed(&replies[1]));
    let after = reader.publication();
    assert_eq!(after.stop.unwrap().request().operation, REQUESTS[0]);
    assert_eq!(after.executions, 1);
    assert_eq!(after.payload, b"A");
    assert_eq!(after.control.ledger.reserved, 0);
    assert_eq!(
        after.control.ledger.charged,
        packets[0].as_ref().unwrap().action().spec().units
    );
    reader.no_generation(9);
    closed(&profiles, &reviewer);
}

#[test]
fn source_loss_after_a_confirmed_prefix_prevents_the_next_native_generation() {
    let root = Root::new();
    let (config, entries, reviewer) = selected(&root, false);
    let reader = Reader::new(&config, &entries[0]);
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let mut humans = Humans::new(
        &reviewer,
        &reader,
        [Some(ReviewDecision::Approve), None, None],
    );
    let source = root.0.join("evidence.json");
    let mut changed = false;
    let result = run(
        config,
        entries,
        &reviewer,
        None,
        || {
            humans.poll();
            if !changed
                && let Ok(image) = FileOversight::read_publication(&reader.store, &reader.profile)
                && image.executions == 1
            {
                // Change only the original live source after actual execution. The
                // preceding source capture cannot authorize new numerical work.
                std::fs::remove_file(&source).unwrap();
                changed = true;
            }
            ElapsedTick(1000)
        },
        &mut output,
    );
    assert!(
        changed,
        "the real first publication must precede source loss"
    );
    assert!(result.is_err());
    assert_eq!(output.documents.len(), 1);
    assert!(!humans.seen[1] && !humans.seen[2]);
    let packets = humans.join();
    let replies = output.join();
    assert!(executed(&replies[0]));
    let progress = reader.progress(7);
    assert_eq!(progress.numerical.numerical.position, 4);
    assert_eq!(progress.numerical.numerical.sampled_draws, 2);
    assert_eq!(progress.publication.executions, 1);
    assert_eq!(progress.publication.payload, b"A");
    assert_eq!(
        progress.publication.control.ledger.charged,
        packets[0].as_ref().unwrap().action().spec().units
    );
    assert_eq!(progress.publication.control.ledger.reserved, 0);
    reader.no_generation(8);
    reader.no_generation(9);
    closed(&profiles, &reviewer);
}
