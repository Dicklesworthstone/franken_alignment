//! Explicit remaining suffixes after an actual reviewed native prefix. Recovery
//! uses the original cooperative opener, source guard and new post-Fence keys.
use super::*;

const AFTER: u64 = 92;

struct Seed {
    config: Config,
    reviewer: PeerProfile,
    reader: Reader,
    packets: Vec<ReviewPacket>,
}

fn seed(root: &Root, messages: usize) -> Seed {
    let (config, mut entries, reviewer) = selected(root, false);
    entries.truncate(2);
    for entry in &mut entries {
        entry.inputs.stream = StreamProfile::new(9, 1, messages, 64, 256).unwrap();
    }
    let reader = Reader::new(&config, &entries[0]);
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
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
    assert!(result.is_ok(), "actual prefix creation failed: {result:?}");
    assert_eq!(output.documents.len(), 2);
    assert!(output.join().iter().all(executed));
    let packets = humans.join().into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(packets.len(), 2);
    closed(&actor_profiles, &reviewer);
    let stream = reader.stream();
    assert_eq!(stream.confirmed.message_count(), 2);
    assert!(!stream.confirmed.finished());
    assert_eq!(stream.publication.payload, b"AA");
    assert_eq!(stream.publication.executions, 2);
    assert_eq!(stream.publication.control.ledger.epoch, 0);
    assert_eq!(reader.progress(8).numerical.numerical.position, 8);

    // The independently configured helpers expect the ONE recovery Fence.
    // Their policy generation is not inferred from their incoming requests.
    let mut config = fixture::configured(root);
    config.programs = fixture::native_programs_at_epoch(root, 1);
    Seed {
        config,
        reviewer,
        reader,
        packets,
    }
}

fn entry(
    root: &Root,
    config: &Config,
    reader: &Reader,
    request: u64,
    generation: u64,
    release: Release,
) -> Entry {
    let (mut actor, _) = profiles(root, config);
    actor.request = request;
    actor.socket = root.0.join(format!("actor-{request}.sock"));
    let mut inputs = recipe::load(
        &root.0.join("recipe.json"),
        config.profile.delivery.scope.tenant,
        config.profile.delivery.limits.bytes,
    )
    .unwrap();
    inputs.generation = generation;
    inputs.stream = reader.stream;
    Entry {
        actor,
        inputs,
        release,
    }
}

fn suffix(root: &Root, config: &Config, reader: &Reader) -> Vec<Entry> {
    vec![
        entry(root, config, reader, 93, 9, Release::Message),
        entry(root, config, reader, 94, 9, Release::Finish),
    ]
}

struct Reviewers {
    reviewer: PeerProfile,
    store: PathBuf,
    profile: FileOversightProfile,
    requests: Vec<u64>,
    clients: Vec<Option<JoinHandle<ReviewPacket>>>,
    seen: Vec<bool>,
}

impl Reviewers {
    fn new(reviewer: &PeerProfile, reader: &Reader, entries: &[Entry]) -> Self {
        Self {
            reviewer: reviewer.clone(),
            store: reader.store.clone(),
            profile: reader.profile.clone(),
            requests: entries.iter().map(|entry| entry.actor.request).collect(),
            clients: std::iter::repeat_with(|| None)
                .take(entries.len())
                .collect(),
            seen: vec![false; entries.len()],
        }
    }

    fn poll(&mut self) {
        for (index, request) in self.requests.iter().copied().enumerate() {
            if self.reviewer.socket(request).exists() {
                self.seen[index] = true;
                if self.clients[index].is_none() {
                    self.clients[index] = Some(human(
                        self.reviewer.clone(),
                        request,
                        self.store.clone(),
                        self.profile.clone(),
                        2 + index as u64,
                        ReviewDecision::Approve,
                    ));
                }
            }
        }
    }

    fn join(self) -> Vec<ReviewPacket> {
        self.clients
            .into_iter()
            .map(|client| {
                client
                    .expect("each successful suffix release needs its human key")
                    .join()
                    .unwrap()
            })
            .collect()
    }
}

fn assert_preserved_prefix(reader: &Reader, before: &FileTextProgressSnapshot) {
    let after = reader.progress(8);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(
        after.text.generation_revision(),
        before.text.generation_revision()
    );
    assert_eq!(after.publication.executions, before.publication.executions);
    assert_eq!(after.publication.payload, before.publication.payload);
    assert_eq!(
        after.publication.control.ledger.charged,
        before.publication.control.ledger.charged
    );
    assert_eq!(after.publication.control.ledger.reserved, 0);
    let stream = reader.stream();
    assert_eq!(stream.confirmed, stream.published);
    assert!(!stream.confirmed.finished());
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    reader.no_generation(9);
}

#[test]
fn recovered_native_suffix_continues_the_confirmed_prefix_once_and_finishes_with_fresh_keys() {
    let root = Root::new();
    let Seed {
        config,
        reviewer,
        reader,
        packets: original,
    } = seed(&root, 4);
    let before = reader.progress(8);
    let entries = suffix(&root, &config, &reader);
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let mut humans = Reviewers::new(&reviewer, &reader, &entries);
    let original_stop = crate::workflow::control::socket_path(&reviewer, 93);
    let mut saw_finish = false;
    let result = run_selected(
        config,
        entries,
        &reviewer,
        None,
        Some(AFTER),
        || {
            humans.poll();
            if reviewer.socket(94).exists() {
                saw_finish = true;
                assert!(original_stop.exists());
                assert!(!crate::workflow::control::socket_path(&reviewer, 94).exists());
            }
            ElapsedTick(1001)
        },
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    assert!(saw_finish);
    assert_eq!(output.documents.len(), 2);
    assert!(output.join().iter().all(executed));
    let packets = humans.join();
    let mut sessions = original
        .iter()
        .map(|packet| packet.binding().session)
        .collect::<BTreeSet<_>>();
    for (index, packet) in packets.iter().enumerate() {
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.action().spec().policy_epoch, 1);
        assert!(packet.action().spec().deadline <= ElapsedTick(16_001));
        assert_eq!(packet.views().len(), 2);
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), vec!["A"; 2 + index].as_slice());
        assert_eq!(frame.message(), if index == 0 { Some("A") } else { None });
        assert_eq!(frame.is_finish(), index == 1);
    }
    let stream = reader.stream();
    assert_eq!(stream.publication.control.ledger.epoch, 1);
    assert_eq!(stream.publication.executions, 4);
    assert_eq!(stream.publication.payload, b"AAA");
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert_eq!(
        stream.publication.control.ledger.charged,
        before.publication.control.ledger.charged
            + packets
                .iter()
                .map(|packet| packet.action().spec().units)
                .sum::<u64>()
    );
    assert!(stream.publication.stop.is_none());
    assert_eq!(stream.confirmed, stream.published);
    assert!(stream.confirmed.finished());
    assert!(stream.pending.is_none());
    let progress = reader.progress(9);
    assert_eq!(progress.text.command().position(), 8);
    assert_eq!(progress.text.generation_revision(), 4);
    assert_eq!(
        progress.text.finish(),
        Some(Ok(GenerationFinish::StopToken))
    );
    assert_eq!(progress.numerical.numerical.position, 12);
    assert_eq!(progress.numerical.numerical.sampled_draws, 6);
    reader.no_generation(10);
    closed(&actor_profiles, &reviewer);
}

#[test]
fn recovered_singleton_finish_closes_a_full_native_stream_without_new_inference() {
    let root = Root::new();
    let Seed {
        config,
        reviewer,
        reader,
        ..
    } = seed(&root, 2);
    let before = reader.progress(8);
    let entries = vec![entry(&root, &config, &reader, 93, 8, Release::Finish)];
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let mut humans = Reviewers::new(&reviewer, &reader, &entries);
    let result = run_selected(
        config,
        entries,
        &reviewer,
        None,
        Some(AFTER),
        || {
            humans.poll();
            ElapsedTick(1001)
        },
        &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(output.documents.len(), 1);
    assert!(output.join().iter().all(executed));
    let packets = humans.join();
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].action().spec().policy_epoch, 1);
    let frame = ReleaseFrame::decode(&packets[0].action().spec().payload).unwrap();
    assert!(frame.is_finish());
    assert_eq!(frame.prior_messages(), &["A", "A"]);
    let progress = reader.progress(8);
    assert_eq!(progress.numerical.numerical, before.numerical.numerical);
    assert_eq!(
        progress.text.generation_revision(),
        before.text.generation_revision()
    );
    assert_eq!(progress.publication.control.ledger.epoch, 1);
    assert_eq!(progress.publication.executions, 3);
    assert_eq!(progress.publication.payload, b"AA");
    assert_eq!(
        progress.publication.control.ledger.charged,
        before.publication.control.ledger.charged + packets[0].action().spec().units
    );
    assert_eq!(progress.publication.control.ledger.reserved, 0);
    let stream = reader.stream();
    assert_eq!(stream.confirmed, stream.published);
    assert!(stream.confirmed.finished());
    assert_eq!(
        stream.confirmed.message_count(),
        stream.confirmed.profile().max_messages()
    );
    reader.no_generation(9);
    closed(&actor_profiles, &reviewer);
}

#[test]
fn recovered_suffix_admits_all_historical_ids_and_the_exact_predecessor_before_new_work() {
    for scenario in 0..4 {
        let root = Root::new();
        let Seed {
            config,
            reviewer,
            reader,
            ..
        } = seed(&root, 4);
        let before = reader.progress(8);
        let mut entries = suffix(&root, &config, &reader);
        let after = match scenario {
            0 => 91,  // An earlier confirmed Message is not the latest prefix.
            1 => 999, // An unknown request is never an implicit predecessor.
            2 => {
                // A later suffix collision must refuse BEFORE its first fresh
                // generation/request, even though that first selection is valid.
                entries[1].actor.request = 91;
                entries[1].actor.socket = root.0.join("actor-91.sock");
                AFTER
            }
            _ => {
                entries[1] = entry(&root, &config, &reader, 94, 7, Release::Message);
                AFTER
            }
        };
        let mut output = Output::new(&entries);
        let actor_profiles = output.profiles.clone();
        let result = run_selected(
            config,
            entries,
            &reviewer,
            None,
            Some(after),
            || ElapsedTick(1001),
            &mut output,
        );
        assert!(result.is_err(), "accepted historical mismatch {scenario}");
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        assert_preserved_prefix(&reader, &before);
        // A valid original open already appended its ordinary Fence. Selection
        // refusal neither rolls it back nor adds a Stop/source/work transaction.
        let after = reader.publication();
        assert_eq!(after.revision, before.publication.revision + 1);
        assert_eq!(after.control.ledger.epoch, 1);
        assert!(after.stop.is_none());
        closed(&actor_profiles, &reviewer);
    }
}

#[test]
fn recovered_suffix_cannot_refill_the_original_remaining_stream_message_allowance() {
    let root = Root::new();
    let Seed {
        config,
        reviewer,
        reader,
        ..
    } = seed(&root, 4);
    let before = reader.progress(8);
    let entries = (0..3)
        .map(|index| {
            entry(
                &root,
                &config,
                &reader,
                93 + index,
                9 + index,
                Release::Message,
            )
        })
        .collect::<Vec<_>>();
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let result = run_selected(
        config,
        entries,
        &reviewer,
        None,
        Some(AFTER),
        || ElapsedTick(1001),
        &mut output,
    );
    assert!(
        result
            .unwrap_err()
            .contains("remaining stream message allowance")
    );
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    assert_preserved_prefix(&reader, &before);
    assert_eq!(
        reader.publication().revision,
        before.publication.revision + 1
    );
    reader.no_generation(10);
    reader.no_generation(11);
    closed(&actor_profiles, &reviewer);
}

fn replay_length(config: &Config, inputs: &Inputs) -> usize {
    let selected = bootstrap::selection(config, inputs, None).unwrap();
    let recovery = FileOversight::begin_open_generated_text_stream_from_source(
        &config.store,
        config.profile.clone(),
        &inputs.decoder,
        &inputs.tokenizer,
        selected,
    )
    .unwrap();
    recovery.progress().total_events
}

fn stopping_at(reviewer: &PeerProfile, operation: u64) -> JoinHandle<StopControlReceipt> {
    // Connect synchronously so the very next original replay checkpoint sees
    // this peer. Only its subsequent authenticated challenge can issue Stop.
    let mut client = reviewer.connect_stop_client(operation).unwrap();
    thread::spawn(move || {
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
fn recovered_suffix_stop_at_final_replay_checkpoint_precedes_fence_and_live_source_work() {
    let root = Root::new();
    let Seed {
        config,
        reviewer,
        reader,
        ..
    } = seed(&root, 4);
    let before = reader.progress(8);
    let canonical = std::fs::read(config.store.join("delivery.bin")).unwrap();
    let entries = suffix(&root, &config, &reader);
    let total = replay_length(&config, &entries[0].inputs);
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let mut calls = 0;
    let mut controller = None;
    let result = run_selected(
        config,
        entries,
        &reviewer,
        None,
        Some(AFTER),
        || {
            // One run-start sample precedes the opener's initial sample; then one
            // sample per replay event and the final Ready checkpoint. This is the
            // last checkpoint before the original normal Fence could be appended.
            if calls == total + 2 {
                assert_eq!(
                    std::fs::read(reader.store.join("delivery.bin")).unwrap(),
                    canonical
                );
                assert!(crate::workflow::control::socket_path(&reviewer, 93).exists());
                assert!(!crate::workflow::control::socket_path(&reviewer, AFTER).exists());
                controller = Some(stopping_at(&reviewer, 93));
            }
            calls += 1;
            ElapsedTick(1001)
        },
        &mut output,
    );
    assert!(
        result
            .unwrap_err()
            .starts_with("native admission Stop acknowledged at revision")
    );
    let receipt = controller
        .expect("the original recovery reached its final Ready checkpoint")
        .join()
        .unwrap();
    assert!(receipt.acknowledged() && receipt.drained());
    assert_eq!(receipt.binding.operation, 93);
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    assert_preserved_prefix(&reader, &before);
    let after = reader.publication();
    assert_eq!(after.revision, before.publication.revision + 2); // Stop and drainage, no normal Fence.
    assert_eq!(
        after.control.ledger.elapsed,
        before.publication.control.ledger.elapsed
    );
    assert_eq!(after.stop.unwrap().request().operation, 93);
    closed(&actor_profiles, &reviewer);
}

#[test]
fn recovered_suffix_deadline_at_ready_keeps_the_original_canonical_history() {
    let root = Root::new();
    let Seed {
        config,
        reviewer,
        reader,
        ..
    } = seed(&root, 4);
    let before = reader.progress(8);
    let canonical = std::fs::read(config.store.join("delivery.bin")).unwrap();
    let entries = suffix(&root, &config, &reader);
    let total = replay_length(&config, &entries[0].inputs);
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let mut calls = 0;
    let result = run_selected(
        config,
        entries,
        &reviewer,
        None,
        Some(AFTER),
        || {
            let current = calls;
            calls += 1;
            ElapsedTick(if current == total + 2 { 16_001 } else { 1001 })
        },
        &mut output,
    );
    assert!(result.is_err());
    assert_eq!(calls, total + 3);
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    assert_eq!(
        std::fs::read(reader.store.join("delivery.bin")).unwrap(),
        canonical
    );
    assert_preserved_prefix(&reader, &before);
    assert_eq!(reader.publication(), before.publication);
    closed(&actor_profiles, &reviewer);
}

#[test]
fn recovered_suffix_still_requires_its_original_live_source_before_new_numerical_work() {
    let root = Root::new();
    let Seed {
        config,
        reviewer,
        reader,
        ..
    } = seed(&root, 4);
    let before = reader.progress(8);
    let entries = suffix(&root, &config, &reader);
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut output = Output::new(&entries);
    let actor_profiles = output.profiles.clone();
    let result = run_selected(
        config,
        entries,
        &reviewer,
        None,
        Some(AFTER),
        || ElapsedTick(1001),
        &mut output,
    );
    assert!(result.is_err());
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    assert_preserved_prefix(&reader, &before);
    let after = reader.publication();
    assert!(after.revision > before.publication.revision);
    assert_eq!(after.stop.unwrap().request().operation, 93);
    closed(&actor_profiles, &reviewer);
}
