//! Resident original learned ownership across authenticated message boundaries.
//! Real imported numerical work, native/process review and fresh human peers
//! exercise custody and accounting. Synthetic weights and neutral forecasts do
//! not establish deployment model quality or empirical detector calibration.
#![cfg(target_os = "linux")]
use super::*;
use super::continuation as native;
use super::peer_intake as peers;
use super::super::sequence as resident;
use crate::workflow::actor_service::Profile;
use crate::workflow::control::socket_path;
use fa_reference::action::consequence::delivery::{
    stream::ReleaseFrame,
    persistent::{FileDeliverySnapshot, observed::reviewer::{
        wire::{ReviewDecision, ReviewPacket},
        peer::{VerifiedReviewerSocket, control::{StopClientProgress, StopControlReceipt}},
    }},
};
use fa_reference::action::consequence::oversight::{
    actor_wire::{WireResponse, client::{
        ActorClientState, ClientError, ClientProgress, ClientSessionLimits, drive::ClientDriveBudget}},
    helper_workers::HelperLimits,
};
use std::{collections::BTreeSet, io::Write, os::unix::net::UnixStream,
    path::{Path, PathBuf}, thread::{self, JoinHandle}};

const DUE: [ElapsedTick; 3] = [ElapsedTick(50_000), ElapsedTick(49_000), ElapsedTick(48_000)];

fn selected(root: &Root, mode: Option<bool>, predictions: Option<usize>)
    -> (Config, Vec<PathBuf>, Vec<resident::Entry>)
{
    let config = configured(root);
    let first = native::write(root);
    if let Some(owned) = mode {
        super::forecast::install(root, &first, owned, 60_000);
        if let Some(limit) = predictions {
            let path = root.0.join("forecast.json");
            let original = fs::read_to_string(&path).unwrap();
            assert!(original.contains("\"max_predictions\":8"));
            fs::write(path, original.replacen("\"max_predictions\":8",
                &format!("\"max_predictions\":{limit}"), 1)).unwrap();
        }
    }
    evidence(root, &config, 1, true, false);
    let initial = recipe::load(&first, &config, true).unwrap();
    let (second, message) = native::next(root, &config, 2);
    let (last, finish) = native::next(root, &config, 3);
    assert_eq!(initial.generation, message.generation);
    assert_eq!(initial.generation, finish.generation);
    let entries = [initial, message, finish].into_iter().enumerate().map(|(index, loaded)| {
        let mut actor = peers::actor_profile(root, &config, loaded.request);
        actor.socket = root.0.join(format!("peers/actor-{}.sock", loaded.request));
        resident::Entry { loaded, actor, release: if index == 2 {
            LearnedTextRelease::Finish
        } else { LearnedTextRelease::Message } }
    }).collect();
    (config, vec![first, second, last], entries)
}

/// Read-only decoding uses the exact independently loaded generation. It never
/// opens a writer, acquires an observer/reviewer, fences or clears staged bytes.
struct Reader { config: Config, loaded: Loaded }
impl Reader {
    fn new(root: &Root, path: &Path) -> Self {
        let config = configured(root);
        let loaded = recipe::load(path, &config, false).unwrap();
        Self { config, loaded }
    }
    fn snapshot(&self) -> FileDeliverySnapshot {
        if let Some(predictor) = self.loaded.generation.required_owned_pre_output_forecast() {
            FileOversight::read_publication_with_owned_learned_consistency(&self.config.store,
                &self.config.profile, &self.loaded.generation, predictor).unwrap().publication
        } else {
            FileOversight::read_publication_with_learned_generation(&self.config.store,
                &self.config.profile, &self.loaded.generation).unwrap()
        }
    }
}

#[derive(Clone, Copy)]
enum Actor { Observe, Cancel }

struct Peers {
    profiles: [Profile; 3],
    release: [LearnedTextRelease; 3],
    reviewer: PeerProfile,
    actors: [Option<JoinHandle<WireResponse>>; 3],
    humans: [Option<JoinHandle<ReviewPacket>>; 3],
    seen_actor: [bool; 3],
    seen_human: [bool; 3],
}
impl Peers {
    fn new(root: &Root, config: &Config, entries: &[resident::Entry]) -> Self {
        assert_eq!(entries.len(), 3);
        Self {
            profiles: std::array::from_fn(|index| entries[index].actor.clone()),
            release: std::array::from_fn(|index| entries[index].release),
            reviewer: fixture::peers(root, config),
            actors: std::array::from_fn(|_| None),
            humans: std::array::from_fn(|_| None),
            seen_actor: [false; 3], seen_human: [false; 3],
        }
    }
    fn poll(&mut self, actors: [Option<Actor>; 3], decisions: [Option<ReviewDecision>; 3]) {
        for index in 0..3 {
            if self.profiles[index].socket.exists() {
                self.seen_actor[index] = true;
                if self.actors[index].is_none() {
                    if let Some(actor) = actors[index] {
                        let profile = self.profiles[index].clone();
                        self.actors[index] = Some(match actor {
                            Actor::Observe => peers::actor(profile, self.release[index], DUE[index], peers::Behavior::Observe),
                            Actor::Cancel => peers::actor(profile, self.release[index], DUE[index], peers::Behavior::Cancel),
                        });
                    }
                }
            }
            if self.reviewer.socket(self.profiles[index].request).exists() {
                self.seen_human[index] = true;
                if self.humans[index].is_none() {
                    if let Some(decision) = decisions[index] {
                        self.humans[index] = Some(peers::human(self.reviewer.clone(),
                            self.profiles[index].request, decision));
                    }
                }
            }
        }
    }
    fn join(self) -> ([Option<WireResponse>; 3], [Option<ReviewPacket>; 3]) {
        (self.actors.map(|peer| peer.map(|peer| peer.join().unwrap())),
            self.humans.map(|peer| peer.map(|peer| peer.join().unwrap())))
    }
}

/// This fault-only peer preserves an actual ambiguous receipt OR lost transport.
/// A failed original store may close the observation service before the peer's
/// next Poll arrives. Neither outcome reconnects, cancels or resubmits an effect.
fn actor_until_unknown(profile: Profile, release: LearnedTextRelease, due: ElapsedTick)
    -> JoinHandle<Result<WireResponse, ClientError>>
{
    thread::spawn(move || {
        let socket = UnixStream::connect(&profile.socket).unwrap();
        profile.supervisor.verify(&socket).unwrap();
        let mut client = ActorClientState::new(ClientSessionLimits::default()).unwrap()
            .connect_unix(socket).unwrap();
        let proposal = FileLearnedTextStreamActorPort::encode_release(profile.request, release, due).unwrap();
        client.submit(profile.request, &proposal).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(25), "ambiguous original actor receipt timed out");
            match client.drive(ClientDriveBudget::default())?.progress {
                ClientProgress::Response(response) => {
                    if matches!(&response.result, Ok(Knowledge::Unknown { .. } | Knowledge::Known { .. })) {
                        return Ok(response);
                    }
                    assert!(matches!(&response.result, Ok(Knowledge::Pending { .. })), "{response:?}");
                    client.poll(profile.request).unwrap();
                }
                _ => {},
            }
            thread::sleep(Duration::from_millis(1));
        }
    })
}

fn confirmed(response: &WireResponse) -> bool {
    matches!(&response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
fn quiescent(root: &Root) {
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
}
fn only_first(peers: &Peers) {
    assert_eq!(peers.seen_actor, [true, false, false]);
    assert!(!peers.seen_human[1] && !peers.seen_human[2]);
}

fn completed(reader: &Reader, result: &resident::SequenceResult,
    replies: [Option<WireResponse>; 3], packets: [Option<ReviewPacket>; 3], mode: Option<bool>)
{
    assert!(result.failure.is_none(), "{:?}", result.failure);
    assert_eq!(result.cleanup_pending, 0);
    assert_eq!(result.responses.len(), 3);
    let mut sessions = BTreeSet::new();
    let mut charged = 0;
    for (index, (reply, packet)) in replies.into_iter().zip(packets).enumerate() {
        let reply = reply.expect("every request used its authenticated actor");
        assert!(confirmed(&reply));
        assert_eq!(reply.result, result.responses[index].result);
        let packet = packet.expect("every release required a fresh human decision");
        assert_eq!(packet.action().spec().deadline, DUE[index]);
        assert_eq!(packet.views().len(), 2);
        assert!(sessions.insert(packet.binding().session));
        charged += packet.action().spec().units;
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), vec!["A"; index.min(2)].as_slice());
        assert_eq!(frame.message(), if index < 2 { Some("A") } else { None });
    }
    // Observe BEFORE independent offline recovery appends its original fence.
    let state = reader.snapshot();
    assert_eq!(state.control.ledger.epoch, 0);
    assert_eq!(state.executions, 3);
    assert_eq!(state.payload, b"AA");
    assert_eq!(state.control.ledger.charged, charged);
    assert_eq!(state.control.ledger.reserved, 0);
    assert!(state.stop.is_none());
    let (host, _reviewer, _observer) =
        recovery::open_selected(&reader.config, &reader.loaded).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 6);
    assert_eq!(numerical.sampled_draws, 4);
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert!(stream.confirmed.finished());
    assert_eq!(stream.confirmed, stream.published);
    assert_eq!(stream.pending, None);
    assert_eq!(host.learned_text_stream_finish_request(2, 3).unwrap().deadline, DUE[2]);
    if let Some(owned) = mode {
        let prediction = host.action_consistency_snapshot().unwrap();
        assert_eq!(prediction.evidence.samples(), 3);
        assert_eq!(prediction.pending_attempt, None);
        assert!(!prediction.coverage_lost && !prediction.evidence.crossed());
        if owned {
            let original = host.learned_action_consistency_snapshot().unwrap();
            assert!(original.work.probe_coordinates > 0 && original.retained_source_bytes > 0);
            assert!(!original.has_unreported_work);
        }
    }
}

#[test]
fn resident_native_messages_and_finish_keep_one_generation_forecast_history_and_authority() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, paths, entries) = selected(&root, mode, None);
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let mut cuts = [false; 2];
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            // The next real listener itself is the observation boundary: its
            // predecessor is already confirmed under the original epoch.
            for index in 1..3 {
                if !cuts[index - 1] && peers.profiles[index].socket.exists() {
                    let state = reader.snapshot();
                    assert_eq!(state.executions, index as u64);
                    assert_eq!(state.control.ledger.reserved, 0);
                    assert_eq!(state.control.ledger.epoch, 0);
                    cuts[index - 1] = true;
                }
            }
            peers.poll([Some(Actor::Observe); 3], [Some(ReviewDecision::Approve); 3]);
            ElapsedTick(1000)
        }).unwrap();
        assert_eq!(cuts, [true; 2]);
        assert_eq!(peers.seen_actor, [true; 3]);
        assert_eq!(peers.seen_human, [true; 3]);
        let (replies, packets) = peers.join();
        completed(&reader, &result, replies, packets, mode);
        quiescent(&root);
    }
}

fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Each selected helper is the existing real process fixture, using the original
/// inherited socket protocol and a distinct retained input trace for this round.
fn process_second(root: &Root, config: &Config, path: &Path, mode: Option<bool>) -> Loaded {
    let executable = quote(std::env::current_exe().unwrap().to_str().unwrap());
    let directory = quote(root.0.to_str().unwrap());
    let child = "workflow::actor_service::learned::tests::processes::process_child";
    let rounds = [21, 22, 23].into_iter().map(|round| {
        let members = ["alpha", "beta"].into_iter().map(|name| {
            let trace = quote(root.0.join(format!("process-{round}-{name}.input")).to_str().unwrap());
            format!(r#""{name}":{{"executable":{executable},"directory":{directory},"arguments":["--exact","{child}","--nocapture"],"environment":{{"FA_LEARNED_PUBLICATION_PROCESS":"allow","FA_LEARNED_PUBLICATION_MEMBER":"{name}","FA_LEARNED_PUBLICATION_TRACE":{trace}}}}}"#)
        }).collect::<Vec<_>>().join(",");
        format!(r#"{{"id":{round},"members":{{{members}}}}}"#)
    }).collect::<Vec<_>>().join(",");
    let mut raw = fs::read_to_string(path).unwrap()
        .replace("fa.learned-publication/4", "fa.learned-publication/6")
        .replace("fa.learned-publication/5", "fa.learned-publication/6")
        .replace(&format!(r#","native_roster":"{}/native-roster.json""#, root.0.display()), "");
    if mode.is_none() { raw = raw.replacen("\"files\":{", "\"files\":{\"forecast\":null,", 1); }
    let (prefix, _) = raw.split_once(r#","native_review":"#).unwrap();
    fs::write(path, format!(r#"{prefix},"process_review":{{"rounds":[{rounds}],"polls":4096,"input_bytes":{},"salt_bytes":32,"starts":6}}}}"#,
        HelperLimits::default().input_bytes)).unwrap();
    let loaded = recipe::load(path, config, true).unwrap();
    assert!(loaded.process.is_some() && loaded.native.is_none());
    assert_eq!(loaded.rounds, [21, 22, 23]);
    loaded
}

#[test]
fn resident_sequence_can_cross_native_process_native_review_boundaries_without_reopening() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, paths, mut entries) = selected(&root, mode, None);
        entries[1].loaded = process_second(&root, &config, &paths[1], mode);
        assert_eq!(entries[0].loaded.generation, entries[1].loaded.generation);
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            peers.poll([Some(Actor::Observe); 3], [Some(ReviewDecision::Approve); 3]);
            ElapsedTick(1000)
        }).unwrap();
        let (replies, packets) = peers.join();
        completed(&reader, &result, replies, packets, mode);
        for name in ["alpha", "beta"] {
            let input = fs::read(root.0.join(format!("process-21-{name}.input"))).unwrap();
            assert!(!input.is_empty());
        }
        quiescent(&root);
    }
}

#[test]
fn actor_cancellation_ends_the_resident_sequence_without_starting_the_next_request_or_refunding_work() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, paths, entries) = selected(&root, mode, None);
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            peers.poll([Some(Actor::Cancel), None, None], [None; 3]);
            ElapsedTick(1000)
        }).unwrap();
        assert!(result.failure.is_some());
        assert_eq!(result.cleanup_pending, 0);
        assert_eq!(result.responses.len(), 1);
        assert!(matches!(&result.responses[0].result,
            Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        only_first(&peers);
        let (replies, packets) = peers.join();
        assert_eq!(replies[0].as_ref().unwrap().result, result.responses[0].result);
        assert!(packets.iter().all(Option::is_none));
        let state = reader.snapshot();
        assert_eq!(state.executions, 0);
        assert_eq!(state.control.ledger.charged, 0);
        assert_eq!(state.control.ledger.reserved, 0);
        assert!(state.stop.is_none(), "a request-local cancellation is not a run-wide Stop");
        let (host, _reviewer, _observer) =
            recovery::open_selected(&reader.config, &reader.loaded).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 4);
        assert_eq!(numerical.sampled_draws, 2);
        assert!(matches!(host.request_status(2), Err(JournalError::Contract(Error::Missing))));
        if mode.is_some() { assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1); }
        drop(host);
        quiescent(&root);
    }
}

#[test]
fn a_resident_successor_cannot_refresh_the_original_prediction_lifetime_allowance() {
    for owned in [false, true] {
        let root = Root::new();
        let (config, paths, entries) = selected(&root, Some(owned), Some(1));
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            peers.poll([Some(Actor::Observe), None, None], [Some(ReviewDecision::Approve), None, None]);
            ElapsedTick(1000)
        }).unwrap();
        assert!(result.failure.is_some());
        assert_eq!(result.cleanup_pending, 0);
        assert!(confirmed(&result.responses[0]));
        assert!(result.responses.iter().skip(1).all(|response| !confirmed(response)));
        assert!(!peers.seen_human[1] && !peers.seen_human[2] && !peers.seen_actor[2]);
        let (replies, packets) = peers.join();
        assert!(confirmed(replies[0].as_ref().unwrap()));
        let charged = packets[0].as_ref().unwrap().action().spec().units;
        let state = reader.snapshot();
        assert_eq!(state.executions, 1);
        assert_eq!(state.payload, b"A");
        assert_eq!(state.control.ledger.charged, charged);
        assert_eq!(state.control.ledger.reserved, 0);
        let (host, _reviewer, _observer) =
            recovery::open_selected(&reader.config, &reader.loaded).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 4);
        assert_eq!(numerical.sampled_draws, 2);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
        assert!(matches!(host.request_status(2), Err(JournalError::Contract(Error::Missing))));
        drop(host);
        quiescent(&root);
    }
}

#[test]
fn ambiguous_dispatch_preserves_its_charge_and_cannot_open_a_resident_successor() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, paths, entries) = selected(&root, mode, None);
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let pending = config.store.join("delivery.pending");
        let canonical = config.store.join("delivery.bin");
        let mut cut = None;
        let mut fault_peer = None;
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            peers.poll([None; 3], [Some(ReviewDecision::Approve), None, None]);
            if fault_peer.is_none() && peers.profiles[0].socket.exists() {
                fault_peer = Some(actor_until_unknown(peers.profiles[0].clone(),
                    peers.release[0], DUE[0]));
            }
            if cut.is_none() && peers.humans[0].is_some() {
                let state = reader.snapshot();
                if state.control.ledger.charged > 0 && state.executions == 0 {
                    // The actual durable Dispatch has already charged liability.
                    // A new staging entry makes the next ORIGINAL atomic replace
                    // fail; it does not inject an outcome or alter canonical data.
                    let bytes = fs::read(&canonical).unwrap();
                    fs::OpenOptions::new().write(true).create_new(true).open(&pending).unwrap()
                        .write_all(b"test-only retained staging collision").unwrap();
                    cut = Some((state, bytes));
                }
            }
            ElapsedTick(1000)
        }).unwrap();
        let (before, bytes) = cut.expect("the real publication reached its durable Dispatch boundary");
        assert!(result.failure.is_some());
        assert_eq!(result.cleanup_pending, 0);
        assert_eq!(result.responses.len(), 1);
        assert!(matches!(&result.responses[0].result, Ok(Knowledge::Unknown { .. })));
        only_first(&peers);
        let remote = fault_peer.expect("the original actor submitted this real Dispatch").join().unwrap();
        match remote {
            Ok(response) => assert!(matches!(response.result, Ok(Knowledge::Unknown { .. }))),
            Err(ClientError::Io(std::io::ErrorKind::UnexpectedEof
                | std::io::ErrorKind::ConnectionAborted | std::io::ErrorKind::ConnectionReset
                | std::io::ErrorKind::BrokenPipe) | ClientError::Disconnected) => {},
            Err(error) => panic!("unexpected fault-peer failure: {error:?}"),
        }
        let (replies, packets) = peers.join();
        assert!(replies.iter().all(Option::is_none));
        assert_eq!(before.control.ledger.charged, packets[0].as_ref().unwrap().action().spec().units);
        assert_eq!(reader.snapshot(), before);
        assert_eq!(fs::read(&canonical).unwrap(), bytes);
        assert_eq!(fs::read(&pending).unwrap(), b"test-only retained staging collision");
        assert_eq!(before.control.ledger.reserved, 0);
        assert!(before.payload.is_empty());
        quiescent(&root);
    }
}

fn stopping(profile: Profile, reviewer: PeerProfile) -> JoinHandle<StopControlReceipt> {
    thread::spawn(move || {
        let socket = VerifiedReviewerSocket::verify(
            UnixStream::connect(socket_path(&reviewer, 1)).unwrap(), profile.supervisor).unwrap();
        let mut client = socket.into_stop_client(reviewer.expected, 1).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(15), "resident Stop receipt timed out");
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => thread::sleep(Duration::from_millis(1)),
            }
        }
    })
}

#[test]
fn the_original_authenticated_stop_service_survives_into_the_second_resident_request() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, paths, entries) = selected(&root, mode, None);
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let original = socket_path(&reviewer, 1);
        let mut controller = None;
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            peers.poll([Some(Actor::Observe), Some(Actor::Observe), None],
                [Some(ReviewDecision::Approve), None, None]);
            if controller.is_none() && reviewer.socket(2).exists() {
                assert!(original.exists());
                assert!(!socket_path(&reviewer, 2).exists());
                let state = reader.snapshot();
                assert_eq!(state.executions, 1);
                assert_eq!(state.control.ledger.epoch, 0);
                controller = Some(stopping(peers.profiles[0].clone(), reviewer.clone()));
            }
            ElapsedTick(1000)
        }).unwrap();
        let receipt = controller.expect("second request reached its fresh human boundary").join().unwrap();
        assert!(receipt.acknowledged() && receipt.drained());
        assert_eq!(receipt.binding.operation, 1);
        assert_eq!(result.cleanup_pending, 0);
        assert_eq!(result.responses.len(), 2);
        assert!(confirmed(&result.responses[0]));
        assert!(!confirmed(&result.responses[1]));
        assert!(!peers.seen_actor[2] && !peers.seen_human[2]);
        let (replies, packets) = peers.join();
        assert!(confirmed(replies[0].as_ref().unwrap()));
        assert!(!confirmed(replies[1].as_ref().unwrap()));
        let state = reader.snapshot();
        assert_eq!(state.stop.unwrap().request().operation, 1);
        assert_eq!(state.executions, 1);
        assert_eq!(state.payload, b"A");
        assert_eq!(state.control.ledger.charged, packets[0].as_ref().unwrap().action().spec().units);
        assert_eq!(state.control.ledger.reserved, 0);
        quiescent(&root);
    }
}

#[test]
fn every_resident_entry_is_bound_before_time_socket_or_store_creation() {
    for mismatch in 0..3 {
        let root = Root::new();
        let (config, paths, mut entries) = selected(&root, None, None);
        match mismatch {
            0 => {
                // Each individual recipe is valid, but the later immutable
                // generation has a different independently admitted horizon.
                let original = fs::read_to_string(&paths[1]).unwrap();
                assert!(original.contains("\"max_new_tokens\":4"));
                fs::write(&paths[1], original.replacen("\"max_new_tokens\":4",
                    "\"max_new_tokens\":5", 1).replacen("\"vocabulary_scores\":1028",
                    "\"vocabulary_scores\":1285", 1)).unwrap();
                entries[1].loaded = recipe::load(&paths[1], &config, true).unwrap();
                assert_ne!(entries[0].loaded.generation, entries[1].loaded.generation);
            }
            1 => {
                entries[1].loaded.request = entries[0].loaded.request;
                entries[1].actor.request = entries[0].actor.request;
            }
            2 => {
                // Cross-entry role aliasing must be rejected even though each
                // actor's own request/reviewer pair is individually distinct.
                let reviewer = fixture::peers(&root, &config);
                entries[0].actor.socket = reviewer.socket(2);
            }
            _ => unreachable!(),
        }
        let reviewer = fixture::peers(&root, &config);
        let store = config.store.clone();
        let result = resident::run(config, entries, &reviewer, false, None, None,
            || panic!("foreign resident entry must refuse before time"));
        assert!(result.is_err());
        assert!(!store.exists());
        quiescent(&root);
    }
}


#[test]
fn the_real_sequence_command_retains_one_final_topology_and_history_cut_for_all_three_releases() {
    use super::mediated_topology;
    use super::super::anchors;
    use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::TOPOLOGY_REFRESH_READ_BYTES;

    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, first, initial) = mediated_topology::selected(&root, mode, false);
        let (second, message) = native::next(&root, &config, 2);
        let (last, finish) = native::next(&root, &config, 3);
        let paths = [first, second, last];
        let entries = [initial, message, finish].into_iter().enumerate().map(|(index, loaded)| {
            let mut actor = peers::actor_profile(&root, &config, loaded.request);
            actor.socket = root.0.join(format!("peers/actor-{}.sock", loaded.request));
            resident::Entry { loaded, actor, release: if index == 2 {
                LearnedTextRelease::Finish
            } else { LearnedTextRelease::Message } }
        }).collect::<Vec<_>>();
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let mut selected = Vec::new();
        for (index, entry) in entries.iter().enumerate() {
            // The command loads each recipe and mandatory actor profile itself.
            // These are the existing independently provisioned peer credentials.
            let inputs = mediated_topology::command_inputs(&root, &config, &paths[index], &entry.actor);
            let actor = root.0.join(format!("sequence-actor-{}.json", entry.loaded.request));
            fs::rename(&inputs[5], &actor).unwrap();
            selected.push(format!(r#"{{"recipe":{},"actor_profile":{},"release":"{}"}}"#,
                quote(paths[index].to_str().unwrap()), quote(actor.to_str().unwrap()),
                if index == 2 { "finish" } else { "message" }));
        }
        let sequence = root.0.join("sequence.json");
        fs::write(&sequence, format!(r#"{{"schema":"fa.learned-publication-sequence/1","entries":[{}]}}"#,
            selected.join(","))).unwrap();
        let source_output = root.0.join("source-1.bin");
        let history_output = root.0.join("history-1.bin");
        let mut args = vec!["serve-learned-generated".to_owned(),
            root.0.join("command-host.json").display().to_string(),
            sequence.display().to_string(),
            root.0.join("command-reviewer.json").display().to_string()];
        let topology = ["--topology-profile".to_owned(),
            root.0.join("create-topology.json").display().to_string()];
        let anchor = ["--retain-history-anchor".to_owned(), history_output.display().to_string()];
        if mode == Some(false) { args.extend(anchor); args.extend(topology); }
        else { args.extend(topology); args.extend(anchor); }
        let mut cuts = [None; 2];
        let result = resident::command_with_clock(&args, None, || {
            // A final export cannot be consumed or create_new'd at an earlier
            // message boundary. Both original owners remain resident meanwhile.
            assert!(!source_output.exists() && !history_output.exists());
            for index in 1..3 {
                if cuts[index - 1].is_none() && peers.profiles[index].socket.exists() {
                    let state = reader.snapshot();
                    assert_eq!(state.executions, index as u64);
                    assert_eq!(state.control.ledger.epoch, 0);
                    assert_eq!(state.control.ledger.reserved, 0);
                    cuts[index - 1] = Some(state.revision);
                }
            }
            peers.poll([Some(Actor::Observe); 3], [Some(ReviewDecision::Approve); 3]);
            ElapsedTick(1000)
        });
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(peers.seen_actor, [true; 3]);
        assert_eq!(peers.seen_human, [true; 3]);
        let (replies, packets) = peers.join();
        let mut sessions = BTreeSet::new();
        let mut charged = 0;
        for (index, (reply, packet)) in replies.into_iter().zip(packets).enumerate() {
            assert!(confirmed(&reply.expect("each selected on-disk actor admitted its release")));
            let packet = packet.expect("each command entry required its own human approval");
            assert!(sessions.insert(packet.binding().session));
            assert_eq!(packet.action().spec().deadline, DUE[index]);
            assert_eq!(packet.views().len(), 2);
            charged += packet.action().spec().units;
            let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
            assert_eq!(frame.prior_messages(), vec!["A"; index.min(2)].as_slice());
            assert_eq!(frame.message(), if index < 2 { Some("A") } else { None });
        }
        let state = reader.snapshot();
        assert_eq!(state.executions, 3);
        assert_eq!(state.payload, b"AA");
        assert_eq!(state.control.ledger.epoch, 0);
        assert_eq!(state.control.ledger.charged, charged);
        assert_eq!(state.control.ledger.reserved, 0);
        assert!(state.stop.is_none());
        let saved = mediated_topology::checkpoint(&root, 1);
        let history = anchors::load(&history_output).unwrap();
        assert!(saved.available());
        assert!(saved.work().checks >= 6, "source work must accumulate over all three releases");
        assert_eq!(saved.work().reserved_read_bytes, saved.work().checks * TOPOLOGY_REFRESH_READ_BYTES);
        assert!(saved.last_operation() >= saved.work().checks);
        assert_eq!(saved.history_anchor(), &history);
        // The paired exports cover the final three-execution canonical cut,
        // including both previous receipts, rather than merely the first output.
        assert_eq!(history.revision(), state.revision);
        assert_eq!(saved.minimum().journal_revision, state.revision);
        assert_eq!(saved.minimum().control_sequence, state.control.sequence);
        assert_eq!(saved.minimum().authority_epoch, 0);
        assert!(cuts[0].unwrap() < cuts[1].unwrap());
        assert!(cuts[1].unwrap() < history.revision());
        quiescent(&root);
    }
}

#[test]
fn a_short_second_request_deadline_expires_with_a_stalled_clock_and_preserves_the_original_stop_operation() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new();
        let (config, paths, mut entries) = selected(&root, mode, None);
        // Keep the initial original generation/review outside this second-entry
        // window. No second actor is admitted, so no helper round is scheduled.
        entries[1].loaded.ttl_ms = 100;
        let reader = Reader::new(&root, &paths[0]);
        let mut peers = Peers::new(&root, &config, &entries);
        let reviewer = peers.reviewer.clone();
        let mut second_started = None;
        let result = resident::run(config, entries, &reviewer, false, None, None, || {
            if second_started.is_none() && peers.profiles[1].socket.exists() {
                let state = reader.snapshot();
                assert_eq!(state.executions, 1);
                assert_eq!(state.control.ledger.epoch, 0);
                second_started = Some(Instant::now());
            }
            peers.poll([Some(Actor::Observe), None, None],
                [Some(ReviewDecision::Approve), None, None]);
            ElapsedTick(1000)
        }).unwrap();
        assert!(second_started.expect("the first real release completed before its successor started")
            .elapsed() < Duration::from_secs(5),
            "the 100ms entry window must not inherit the 60s global wall ceiling");
        assert!(result.failure.is_some());
        assert_eq!(result.cleanup_pending, 0);
        assert!(confirmed(&result.responses[0]));
        assert!(result.responses.iter().skip(1).all(|response| !confirmed(response)));
        assert!(!peers.seen_actor[2] && !peers.seen_human[1] && !peers.seen_human[2]);
        let (replies, packets) = peers.join();
        assert!(confirmed(replies[0].as_ref().unwrap()));
        assert!(replies[1].is_none() && replies[2].is_none());
        let state = reader.snapshot();
        assert_eq!(state.executions, 1);
        assert_eq!(state.payload, b"A");
        assert_eq!(state.control.ledger.charged, packets[0].as_ref().unwrap().action().spec().units);
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.stop.as_ref().unwrap().request().operation, 1,
            "an operational failure at request2 must retain the run's original Stop binding");
        quiescent(&root);
    }
}
