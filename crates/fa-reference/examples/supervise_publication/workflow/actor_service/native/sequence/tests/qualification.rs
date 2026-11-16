//! A real confirmed native prefix, explicit held-out requalification and fresh
//! authenticated actor/helper/human publication. Synthetic labels establish
//! contract enforcement only, never evaluator authentication or detector quality.
use super::*;
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use std::fs;

#[allow(dead_code)]
mod evidence {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/supervise_publication/workflow/actor_service/native/qualification/tests/fixtures.rs"
    ));
}

const AFTER: u64 = 92;

struct Seed {
    config: Config,
    reviewer: PeerProfile,
    reader: Reader,
    before: FileTextProgressSnapshot,
    old: CredibilityActivation,
    fresh: CredibilityActivation,
}

/// Publish two original Messages with both keys, then install an actual native
/// qualification. The next recovery must invalidate it; no restored score can
/// authorize the suffix. This setup uses the same original journal throughout.
fn seed(root: &Root) -> Seed {
    let (config, mut entries, reviewer) = selected(root, false);
    entries.truncate(2);
    let reader = Reader::new(&config, &entries[0]);
    let mut output = Output::new(&entries);
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
    assert!(result.is_ok(), "{result:?}");
    assert!(output.join().iter().all(executed));
    let packets = humans.join();
    assert!(packets[0].is_some() && packets[1].is_some());
    assert!(packets[2].is_none());

    let mut config = fixture::configured(root);
    let native = inputs(root, &config);
    let selected = bootstrap::selection(&config, &native, None).unwrap();
    let (mut host, _) = bootstrap::prepare(&config, &native, selected, true).unwrap();
    let control = host.inspect().control;
    let old = evidence::activation(&config, control.sequence, control.ledger.epoch, 20, 2, 2);
    host.activate_credibility(host.revision(), old.clone(), &config.profile.committee)
        .unwrap();
    host.check_credibility().unwrap();
    let control = host.inspect().control;
    let fresh = evidence::activation(
        &config,
        control.sequence,
        control.ledger.epoch + 1, // one original recovery Fence
        21,
        3,
        2,
    );
    drop(host);
    config.programs = fixture::native_programs_at_epoch(root, fresh.expected_epoch + 1);
    let before = reader.progress(8);
    assert_eq!(before.publication.payload, b"AA");
    assert_eq!(before.publication.executions, 2);
    assert_eq!(before.numerical.numerical.position, 8);
    Seed { config, reviewer, reader, before, old, fresh }
}

fn suffix(root: &Root, config: &Config, reader: &Reader, finish_only: bool) -> Vec<Entry> {
    let choices = if finish_only {
        vec![(8, Release::Finish)]
    } else {
        vec![(9, Release::Message), (10, Release::Message), (10, Release::Finish)]
    };
    choices
        .into_iter()
        .enumerate()
        .map(|(index, (generation, release))| {
            let (mut actor, _) = profiles(root, config);
            actor.request = 93 + index as u64;
            actor.socket = root.0.join(format!("actor-{}.sock", actor.request));
            let mut inputs = recipe::load(
                &root.0.join("recipe.json"),
                config.profile.delivery.scope.tenant,
                config.profile.delivery.limits.bytes,
            )
            .unwrap();
            inputs.generation = generation;
            inputs.stream = reader.stream;
            Entry { actor, inputs, release, precheck: None }
        })
        .collect()
}

struct Reviewers {
    reviewer: PeerProfile,
    store: PathBuf,
    profile: FileOversightProfile,
    requests: Vec<u64>,
    decisions: Vec<ReviewDecision>,
    clients: Vec<Option<JoinHandle<ReviewPacket>>>,
}

impl Reviewers {
    fn new(
        reviewer: &PeerProfile,
        reader: &Reader,
        entries: &[Entry],
        reject: Option<usize>,
    ) -> Self {
        Self {
            reviewer: reviewer.clone(),
            store: reader.store.clone(),
            profile: reader.profile.clone(),
            requests: entries.iter().map(|entry| entry.actor.request).collect(),
            decisions: (0..entries.len())
                .map(|index| if reject == Some(index) {
                    ReviewDecision::Reject
                } else {
                    ReviewDecision::Approve
                })
                .collect(),
            clients: std::iter::repeat_with(|| None).take(entries.len()).collect(),
        }
    }

    fn poll(&mut self) {
        for (index, request) in self.requests.iter().copied().enumerate() {
            if self.reviewer.socket(request).exists() && self.clients[index].is_none() {
                self.clients[index] = Some(human(
                    self.reviewer.clone(),
                    request,
                    self.store.clone(),
                    self.profile.clone(),
                    2 + index as u64,
                    self.decisions[index],
                ));
            }
        }
    }

    fn join(self) -> Vec<Option<ReviewPacket>> {
        self.clients.into_iter().map(|client| {
            client.map(|client| client.join().unwrap())
        }).collect()
    }
}

struct OnceOutput {
    inner: precheck_tests::CheckedOutput,
    capsule: PathBuf,
    removed: bool,
}

impl Write for OnceOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()?;
        if !self.removed && !self.inner.documents.is_empty() {
            // The first disclosure is already backed by the acknowledged native
            // activation. No later handoff may reopen or reactivate this input.
            fs::remove_file(&self.capsule)?;
            self.removed = true;
        }
        Ok(())
    }
}

fn preserved(reader: &Reader, before: &FileTextProgressSnapshot) {
    let after = reader.progress(8);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(after.text.generation_revision(), before.text.generation_revision());
    assert_eq!(after.publication.payload, before.publication.payload);
    assert_eq!(after.publication.executions, before.publication.executions);
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
    assert_eq!(after.publication.control.ledger.reserved, 0);
    reader.no_generation(9);
}

#[test]
fn resident_requalification_is_once_for_the_original_suffix_and_still_needs_every_human_key() {
    for reject in [None, Some(1)] {
        let root = Root::new();
        let Seed { config, reviewer, reader, before, fresh, .. } = seed(&root);
        let epoch = fresh.expected_epoch + 1;
        let path = root.0.join("fresh-qualification.bin");
        fs::write(&path, fresh.encode_reference().unwrap()).unwrap();
        let mut entries = suffix(&root, &config, &reader, false);
        let sockets = precheck_tests::select(&root, &mut entries, epoch);
        let actor_profiles = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
        let mut humans = Reviewers::new(&reviewer, &reader, &entries, reject);
        let mut output = OnceOutput {
            inner: precheck_tests::CheckedOutput::new(&entries, &reader, &reviewer, epoch),
            capsule: path.clone(),
            removed: false,
        };
        let result = run_with_qualification(
            config,
            entries,
            &reviewer,
            None,
            (Some(AFTER), None),
            Some(&path),
            || {
                humans.poll();
                ElapsedTick(1000)
            },
            &mut output,
        );
        assert_eq!(result.is_ok(), reject.is_none(), "{result:?}");
        assert!(output.removed && !path.exists());
        let documents = output.inner.documents.len();
        let replies = output.inner.join();
        let packets = humans.join();
        assert_eq!(documents, if reject.is_none() { 3 } else { 2 });
        assert_eq!(replies.len(), documents);
        assert!(executed(&replies[0]));
        assert_eq!(replies.iter().all(executed), reject.is_none());
        assert!(packets[0].is_some() && packets[1].is_some());
        assert_eq!(packets[2].is_some(), reject.is_none());
        let mut sessions = BTreeSet::new();
        for packet in packets.iter().flatten() {
            assert!(sessions.insert(packet.binding().session));
            assert_eq!(packet.action().spec().policy_epoch, epoch);
        }
        let stream = reader.stream();
        assert_eq!(stream.publication.control.ledger.epoch, epoch);
        assert_eq!(stream.publication.executions, if reject.is_none() { 5 } else { 3 });
        assert_eq!(stream.publication.payload, if reject.is_none() { b"AAAA".to_vec() } else { b"AAA".to_vec() });
        assert_eq!(stream.confirmed.finished(), reject.is_none());
        assert_eq!(stream.confirmed, stream.published);
        assert_eq!(stream.publication.control.ledger.reserved, 0);
        let approved = packets.iter().enumerate().filter_map(|(index, packet)| {
            if reject == Some(index) {
                None
            } else {
                packet.as_ref().map(|packet| packet.action().spec().units)
            }
        }).sum::<u64>();
        assert_eq!(stream.publication.control.ledger.charged, before.publication.control.ledger.charged + approved);
        let progress = reader.progress(10);
        assert_eq!(progress.numerical.numerical.position, 16);
        assert_eq!(progress.numerical.numerical.sampled_draws, 8);
        reader.no_generation(11);
        assert!(sockets.iter().all(|socket| socket.as_ref().is_none_or(|path| !path.exists())));
        closed(&actor_profiles, &reviewer);
    }
}

#[test]
fn resident_requalification_allows_a_final_finish_without_any_new_inference() {
    let root = Root::new();
    let Seed { config, reviewer, reader, before, fresh, .. } = seed(&root);
    let epoch = fresh.expected_epoch + 1;
    let path = root.0.join("fresh-qualification.bin");
    fs::write(&path, fresh.encode_reference().unwrap()).unwrap();
    let mut entries = suffix(&root, &config, &reader, true);
    precheck_tests::select(&root, &mut entries, epoch);
    let mut humans = Reviewers::new(&reviewer, &reader, &entries, None);
    let mut output = precheck_tests::CheckedOutput::new(&entries, &reader, &reviewer, epoch);
    let result = run_with_qualification(
        config, entries, &reviewer, None, (Some(AFTER), None), Some(&path),
        || { humans.poll(); ElapsedTick(1000) }, &mut output,
    );
    assert!(result.is_ok(), "{result:?}");
    assert_eq!(output.documents.len(), 1);
    assert!(output.join().iter().all(executed));
    let packets = humans.join();
    let packet = packets[0].as_ref().unwrap();
    assert_eq!(packet.action().spec().policy_epoch, epoch);
    assert!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().is_finish());
    let after = reader.progress(8);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(after.text.generation_revision(), before.text.generation_revision());
    assert_eq!(after.publication.payload, b"AA");
    assert_eq!(after.publication.executions, 3);
    assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged + packet.action().spec().units);
    assert!(reader.stream().confirmed.finished());
    reader.no_generation(9);
}

#[test]
fn resident_requalification_rejects_missing_historical_and_mismatched_evidence_before_new_work() {
    for cause in ["unselected", "missing", "historical", "stale_predecessor", "wrong_profile"] {
        let root = Root::new();
        let Seed { config, reviewer, reader, before, old, mut fresh } = seed(&root);
        let path = root.0.join("qualification.bin");
        match cause {
            "unselected" | "missing" => {}
            "historical" => fs::write(&path, old.encode_reference().unwrap()).unwrap(),
            _ => {
                if cause == "stale_predecessor" {
                    fresh.expected_control_sequence -= 1;
                } else {
                    fresh.policy_generation += 1;
                }
                fs::write(&path, fresh.encode_reference().unwrap()).unwrap();
            }
        }
        let entries = suffix(&root, &config, &reader, false);
        let actor_profiles = entries.iter().map(|entry| entry.actor.clone()).collect::<Vec<_>>();
        let mut output = Vec::new();
        let result = run_with_qualification(
            config, entries, &reviewer, None, (Some(AFTER), None),
            if cause == "unselected" { None } else { Some(&path) },
            || ElapsedTick(1000), &mut output,
        );
        assert!(result.is_err(), "{cause}");
        assert!(output.is_empty(), "{cause}");
        preserved(&reader, &before);
        closed(&actor_profiles, &reviewer);
    }
}

#[test]
fn resident_requalification_preserves_selected_disclosure_epoch_before_reading_the_capsule() {
    for offset in [0, 2] {
        let root = Root::new();
        let Seed { config, reviewer, reader, before, fresh, .. } = seed(&root);
        let mut entries = suffix(&root, &config, &reader, false);
        precheck_tests::select(&root, &mut entries, fresh.expected_epoch + offset);
        let missing = root.0.join("must-not-be-read");
        let mut output = Vec::new();
        let result = run_with_qualification(
            config, entries, &reviewer, None, (Some(AFTER), None), Some(&missing),
            || ElapsedTick(1000), &mut output,
        );
        assert!(result.unwrap_err().contains("selected activation successor"));
        assert!(output.is_empty());
        preserved(&reader, &before);
        let after = reader.publication();
        assert_eq!(after.revision, before.publication.revision + 1);
        assert_eq!(after.control.ledger.epoch, fresh.expected_epoch);
        assert!(after.stop.is_none());
    }
}

fn stop_client(reviewer: &PeerProfile, request: u64) -> JoinHandle<StopControlReceipt> {
    let mut client = reviewer.connect_stop_client(request).unwrap();
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
fn resident_requalification_rechecks_stop_and_deadline_before_any_post_activation_source_work() {
    for stop in [true, false] {
        let root = Root::new();
        let Seed { config, reviewer, reader, before, fresh, .. } = seed(&root);
        let epoch = fresh.expected_epoch + 1;
        let path = root.0.join("qualification.bin");
        fs::write(&path, fresh.encode_reference().unwrap()).unwrap();
        let entries = suffix(&root, &config, &reader, false);
        fs::remove_file(root.0.join("evidence.json")).unwrap();
        let mut client = None;
        let mut observed_activation = false;
        let mut output = Vec::new();
        let result = run_with_qualification(
            config, entries, &reviewer, None, (Some(AFTER), None), Some(&path),
            || {
                if !observed_activation && reader.publication().control.ledger.epoch == epoch {
                    observed_activation = true;
                    if stop {
                        client = Some(stop_client(&reviewer, 93));
                    }
                }
                if observed_activation && !stop {
                    ElapsedTick(100_000)
                } else {
                    ElapsedTick(1000)
                }
            },
            &mut output,
        );
        assert!(observed_activation);
        let error = result.unwrap_err();
        if stop {
            assert!(error.contains("native sequence ended after 0"), "{error}");
            let receipt = client.unwrap().join().unwrap();
            assert!(receipt.acknowledged() && receipt.drained());
        } else {
            assert!(error.contains("deadline"), "{error}");
        }
        assert!(output.is_empty());
        preserved(&reader, &before);
        assert!(reader.publication().stop.is_some());
    }
}

#[test]
fn resident_requalification_does_not_replace_fresh_source_evidence() {
    let root = Root::new();
    let Seed { config, reviewer, reader, before, fresh, .. } = seed(&root);
    let path = root.0.join("qualification.bin");
    fs::write(&path, fresh.encode_reference().unwrap()).unwrap();
    let entries = suffix(&root, &config, &reader, false);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut output = Vec::new();
    let result = run_with_qualification(
        config, entries, &reviewer, None, (Some(AFTER), None), Some(&path),
        || ElapsedTick(1000), &mut output,
    );
    assert!(result.is_err());
    assert!(output.is_empty());
    preserved(&reader, &before);
    assert!(reader.publication().stop.is_some());
}

#[path = "qualification/history.rs"]
mod history;
