//! An actual governed native prefix survives explicit independently guarded
//! suffix recovery. Model fixtures test custody and execution, not accuracy.
#[path = "governed_recovery_promotion.rs"]
mod promotion_tests;
#[path = "governed_requalification.rs"]
mod requalification_tests;

use super::super::super::anchors as custody;
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    anchored::{FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES},
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use std::{io::Cursor, os::unix::fs::PermissionsExt};

const AFTER: u64 = REQUESTS[1];
const RECOVERED_CHILD_ROOT: &str = "FA_NATIVE_RECOVERED_GOVERNOR_ROOT";
const RECOVERED_CHILD_TEST: &str = "workflow::actor_service::native::sequence::tests::governor_tests::recovery_tests::independent_recovered_governor_process";

struct Prefix {
    config: Config,
    reviewer: PeerProfile,
    reader: Reader,
    packets: Vec<ReviewPacket>,
    archive: PathBuf,
    earlier: Vec<u8>,
}

fn history(
    config: &Config,
    entries: &[Entry],
    reviewer: &PeerProfile,
    selected_operator: Option<&operator::Selection>,
    after: Option<u64>,
    input: Option<&Path>,
    output: &Path,
) -> custody::Selection {
    let mut args = [
        "serve-native-generated",
        "config.json",
        "sequence.json",
        "reviewer.json",
    ]
    .map(str::to_owned)
    .to_vec();
    if let Some(input) = input {
        args.extend([
            "--history-anchor".into(),
            input.to_str().unwrap().to_owned(),
        ]);
    }
    args.extend([
        "--retain-history-anchor".into(),
        output.to_str().unwrap().to_owned(),
    ]);
    custody::take_options(&args)
        .unwrap()
        .1
        .admit(
            config,
            &entries
                .iter()
                .map(|entry| (&entry.actor, entry.precheck.as_ref()))
                .collect::<Vec<_>>(),
            reviewer,
            selected_operator,
            after,
            None,
        )
        .unwrap()
}

fn archive(path: &Path) -> FileHistoryAnchor {
    assert_eq!(fs::metadata(path).unwrap().permissions().mode() & 0o077, 0);
    FileHistoryAnchor::read_trusted(
        &mut Cursor::new(fs::read(path).unwrap()),
        MAX_HISTORY_ANCHOR_BYTES,
    )
    .unwrap()
}

fn current_evidence(scope: fa_reference::action::Scope) -> Vec<u8> {
    EvidenceSnapshot::new(
        EvidenceIdentity {
            scope,
            source: 51,
            generation: 2,
        },
        Snapshot {
            semantic_epoch: 2,
            complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        },
        ["alpha", "beta"]
            .into_iter()
            .map(|member| {
                (
                    member.to_owned(),
                    format!("review this fixture: {member}; {FRESH_SOURCE}").into_bytes(),
                )
            })
            .collect(),
    )
    .unwrap()
    .encode()
}

fn recovered_config(root: &Root) -> Config {
    let mut config = fixture::configured(root);
    config.programs = helper_programs(root);
    write_complete(
        config.source.path(),
        current_evidence(config.profile.delivery.scope),
    );
    config
}

/// This uses the original resident entry point, actual numerical generations,
/// helper child processes and independently authenticated effect humans.
/// The independent governor approves a complete original corpus and the Tower
/// applies it between two separately approved, actually published Messages.
fn prefix(root: &Root) -> Prefix {
    let (mut config, mut entries, reviewer) = selected(root, false);
    entries.truncate(2);
    config.programs = helper_programs(root);
    let reader = Reader::new(&config, &entries[0]);
    let peer_store = config.store.clone();
    let peer_profile = config.profile.clone();
    let ready = barrier(root, &mut entries[1]);
    let mut child = GovernorChild::start(root);
    let (governor, selected_operator) = selections(root, &entries[0].actor, child.0.id());
    let path = root.0.join("governed-prefix.anchor");
    let retained = history(
        &config,
        &entries,
        &reviewer,
        Some(&selected_operator),
        None,
        None,
        &path,
    );
    let mut output = GovernedOutput::new(&entries);
    let release_actor = output.released.clone();
    let refreshed = output.refreshed.clone();
    let mut humans = Humans::new(
        &reviewer,
        &reader,
        [Some(ReviewDecision::Approve), None, None],
    );
    let second_human = Arc::new(AtomicBool::new(false));
    let release_human = second_human.clone();
    let root_path = root.0.clone();
    let evidence_path = config.source.path().to_owned();
    let operator_settings = selected_operator.settings.clone();
    let human_socket = reviewer.socket(AFTER);
    let scope = entries[0].actor.scope;
    let evidence = current_evidence(scope);
    let peer = thread::spawn(move || {
        // Publication inspection needs only the original store and profile;
        // keep the decoder's thread-local weight buffers in the service thread.
        let publication = || FileOversight::read_publication(&peer_store, &peer_profile).unwrap();
        wait(&ready);
        let original = publication();
        assert_eq!(original.executions, 1);
        assert_eq!(original.control.ledger.epoch, 0);
        let earlier = fs::read(root_path.join("store/delivery.bin")).unwrap();
        fs::remove_file(&evidence_path).unwrap();
        let mut socket = operator_tests::connect(&operator_settings);
        let mut request = TowerRequest::new(
            "recoverable-native-promotion",
            Address::parse(&format!(
                "fa://{}/run/{}@{GENERATION}",
                scope.tenant, scope.run
            ))
            .unwrap(),
            Verb::Next,
        );
        request.arguments = ReadArguments::PolicyPromotionNext {
            campaign: CAMPAIGN,
            operation: OPERATION,
        };
        let request = encode_request(&request).unwrap();
        write_complete(&root_path.join("ready"), b"real native prefix is ready");
        wait(&root_path.join("approved"));
        let next = operator_exchange(&mut socket, &request);
        let planned = operator_known(&next, "policy-promotion-next");
        let action = live_governor::protocol::json(field(planned, "action"));
        operator_known(&operator_exchange(&mut socket, &action), "policy-promotion");
        let promoted = publication();
        assert_eq!(promoted.control.ledger.epoch, 1);
        assert_eq!(
            promoted.control.ledger.charged,
            original.control.ledger.charged
        );
        assert_eq!(promoted.executions, 1);
        let started = Instant::now();
        while !refreshed.load(Ordering::Acquire) {
            assert!(started.elapsed() < Duration::from_secs(25));
            pause(1);
        }
        assert!(!ready.exists());
        write_complete(&evidence_path, evidence);
        release_actor.store(true, Ordering::Release);
        wait(&human_socket);
        assert_eq!(publication().executions, 1);
        release_human.store(true, Ordering::Release);
        earlier
    });
    let result = run_with_governor(
        config,
        entries,
        &reviewer,
        None,
        (None, Some(&selected_operator)),
        None,
        Some(&retained),
        Some(&governor),
        || {
            if second_human.load(Ordering::Acquire) {
                humans.decisions[1] = Some(ReviewDecision::Approve);
            }
            humans.poll();
            ElapsedTick(1000)
        },
        &mut output,
    );
    let earlier = peer.join().unwrap();
    child.finish();
    assert!(result.is_ok(), "governed prefix: {result:?}");
    let profiles = output.profiles.clone();
    assert_eq!(output.clients.len(), 2);
    assert!(output.join().iter().all(executed));
    let packets = humans.join().into_iter().flatten().collect::<Vec<_>>();
    assert_eq!(packets.len(), 2);
    assert_eq!(packets[0].action().spec().policy_epoch, 0);
    assert_eq!(packets[1].action().spec().policy_epoch, 1);
    assert_eq!(packets[1].policy_generation(), 2);
    assert_ne!(packets[0].binding().session, packets[1].binding().session);
    let retained = reader.stream();
    assert_eq!(retained.publication.executions, 2);
    assert_eq!(retained.publication.control.ledger.epoch, 1);
    assert_eq!(retained.publication.payload, b"AA");
    assert_eq!(retained.confirmed, retained.published);
    assert!(!retained.confirmed.finished());
    assert_eq!(archive(&path).revision(), retained.publication.revision);
    closed(&profiles, &reviewer);
    Prefix {
        config: recovered_config(root),
        reviewer,
        reader,
        packets,
        archive: path,
        earlier,
    }
}

fn suffix(root: &Root, config: &Config, reader: &Reader, finish_only: bool) -> Vec<Entry> {
    let (actor, _) = profiles(root, config);
    let make = |request: u64, generation: u64, release: Release| {
        let mut actor = actor.clone();
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
            precheck: None,
        }
    };
    if finish_only {
        vec![make(93, 8, Release::Finish)]
    } else {
        vec![make(93, 9, Release::Message), make(94, 9, Release::Finish)]
    }
}

fn expected(
    config: &Config,
    entry: &Entry,
    governor: &live_governor::Settings,
    cut: &FileDeliverySnapshot,
) -> FileRecoveryRequirements {
    FileRecoveryRequirements {
        guards: FileGuardSet {
            stream: Some(entry.inputs.stream),
            decoder: Some(entry.inputs.decoder.clone()),
            decoder_stop: None,
            source: Some(config.source_policy),
            identity: None,
            campaigns: Some(governor.campaign_requirement()),
            credential: None,
        },
        effective_policy: Policy::new(
            2,
            vec![
                Predicate::ExactValue {
                    key: 7,
                    value: b"ok".to_vec(),
                },
                Predicate::PayloadAtMost(256),
                Predicate::UnitsAtMost(512),
                Predicate::All(vec![0, 1, 2]),
            ],
        )
        .unwrap(),
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: cut.revision,
            control_sequence: cut.control.sequence,
            authority_epoch: cut.control.ledger.epoch,
        },
    }
}

struct RecoveredHumans {
    reviewer: PeerProfile,
    reader: Reader,
    requests: Vec<u64>,
    decision: ReviewDecision,
    clients: Vec<Option<JoinHandle<ReviewPacket>>>,
}

impl RecoveredHumans {
    fn new(
        reviewer: &PeerProfile,
        config: &Config,
        entries: &[Entry],
        decision: ReviewDecision,
    ) -> Self {
        Self {
            reviewer: reviewer.clone(),
            reader: Reader::new(config, &entries[0]),
            requests: entries.iter().map(|entry| entry.actor.request).collect(),
            decision,
            clients: std::iter::repeat_with(|| None)
                .take(entries.len())
                .collect(),
        }
    }

    fn poll(&mut self) {
        for (index, request) in self.requests.iter().copied().enumerate() {
            if self.reviewer.socket(request).exists() && self.clients[index].is_none() {
                self.clients[index] = Some(human(
                    self.reviewer.clone(),
                    request,
                    self.reader.store.clone(),
                    self.reader.profile.clone(),
                    2 + index as u64,
                    self.decision,
                ));
            }
        }
    }

    fn join(self) -> Vec<ReviewPacket> {
        self.clients
            .into_iter()
            .flatten()
            .map(|client| client.join().unwrap())
            .collect()
    }
}

/// The recovered authority is usable only by the newly selected independent
/// custodian. Inspection consumes no replay allowance and cannot approve an
/// effect; the parent deliberately withholds its human key until this returns.
#[test]
fn independent_recovered_governor_process() {
    let Some(root) = std::env::var_os(RECOVERED_CHILD_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    wait(&root.join("reopened.ready"));
    let settings = live_governor::Settings::read(&root.join("governor.json")).unwrap();
    let scope = live_governor::protocol::scope_json(settings.scope);
    let inspected = inspect(&root, &scope, 91);
    let value = known(&inspected, "inspect");
    assert_eq!(number(value, "authority_epoch"), 2);
    assert_eq!(number(value, "policy_generation"), 2);
    let campaigns = field(value, "campaigns");
    assert_eq!(number(campaigns, "cases"), 16);
    assert_eq!(number(campaigns, "input_bytes"), 1_048_576);
    assert_eq!(number(campaigns, "max_campaigns"), 4);
    assert_eq!(number(value, "campaign_limit"), 1);
    assert_eq!(number(value, "report_bytes"), 65_536);
    assert_eq!(field(value, "stopped").as_bool(), Some(false));
    let numerical = field(value, "numerical_replay");
    assert_eq!(number(numerical, "max_replay_events"), 4096);
    assert_eq!(number(numerical, "events"), 8192);
    assert_eq!(number(numerical, "remaining_events"), 8192);
    write_complete(
        &root.join("reopened.inspected"),
        b"original recovered governor inspected",
    );
}

fn recovered_child(root: &Root) -> GovernorChild {
    GovernorChild(
        ProcessCommand::new(std::env::current_exe().unwrap())
            .args(["--exact", RECOVERED_CHILD_TEST, "--nocapture"])
            .env(RECOVERED_CHILD_ROOT, &root.0)
            .spawn()
            .unwrap(),
    )
}

fn resumed_suffix(finish_only: bool, decision: ReviewDecision, collide: bool) {
    let root = Root::new();
    let Prefix {
        config,
        reviewer,
        reader,
        packets: original,
        archive: previous,
        ..
    } = prefix(&root);
    let before = reader.progress(8);
    let previous_bytes = fs::read(&previous).unwrap();
    let entries = suffix(&root, &config, &reader, finish_only);
    let mut child = recovered_child(&root);
    let (governor, selected_operator) = selections(&root, &entries[0].actor, child.0.id());
    let requirements = expected(&config, &entries[0], &governor, &before.publication);
    let next = root.0.join("governed-successor.anchor");
    let retained = history(
        &config,
        &entries,
        &reviewer,
        Some(&selected_operator),
        Some(AFTER),
        Some(&previous),
        &next,
    );
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let mut humans = RecoveredHumans::new(&reviewer, &config, &entries, decision);
    let ready = root.0.join("reopened.ready");
    let inspected = root.0.join("reopened.inspected");
    let mut collided = false;
    let result = run_with_governed_recovery(
        config,
        entries,
        &reviewer,
        None,
        (Some(AFTER), Some(&selected_operator)),
        None,
        Some(&retained),
        Some(&governor),
        Some(&requirements),
        || {
            if collide && !collided {
                fs::write(&next, b"independent archive keeps this name").unwrap();
                collided = true;
            }
            if governor.socket.exists() && !ready.exists() {
                write_complete(&ready, b"newly selected independent custodian");
            }
            if inspected.exists() {
                humans.poll();
            }
            ElapsedTick(1001)
        },
        &mut output,
    );
    child.finish();
    assert!(
        inspected.exists(),
        "the new independent governor must observe the actual owner"
    );
    let documents = output.documents.clone();
    let replies = output.join();
    let packets = humans.join();
    let approved = decision == ReviewDecision::Approve;
    let releases = if finish_only { 1 } else { 2 };
    if approved {
        if collide {
            let error = result.unwrap_err();
            assert!(
                error.starts_with("history anchor retention failed:"),
                "{error}"
            );
        } else {
            assert!(result.is_ok(), "{result:?}");
        }
        assert_eq!(replies.len(), releases);
        assert!(replies.iter().all(executed));
        assert_eq!(packets.len(), releases);
    } else {
        assert!(result.is_err());
        assert_eq!(replies.len(), 1);
        assert!(!executed(&replies[0]));
        assert_eq!(packets.len(), 1);
    }
    assert_eq!(documents.len(), replies.len());
    let mut sessions = original
        .iter()
        .map(|packet| packet.binding().session)
        .collect::<BTreeSet<_>>();
    for (index, packet) in packets.iter().enumerate() {
        assert!(
            sessions.insert(packet.binding().session),
            "recovery must not reuse an old human key"
        );
        assert_eq!(packet.policy_generation(), 2);
        assert_eq!(packet.action().spec().policy_epoch, 2);
        assert!(packet.action().spec().deadline <= ElapsedTick(16_001));
        assert_eq!(packet.views().len(), 2);
        let Command::Submit { request, proposal } = decode_command(&documents[index]).unwrap()
        else {
            panic!("recovery emits an original source-only reference");
        };
        assert_eq!(request, 93 + index as u64);
        assert_eq!(proposal.expected_policy_epoch, 2);
        assert_eq!(
            proposal.payload.len(),
            FileGeneratedTextActorPort::INTENT_BYTES
        );
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.prior_messages(), vec!["A"; 2 + index].as_slice());
        assert_eq!(frame.is_finish(), finish_only || index == 1);
        assert_eq!(
            frame.message(),
            if finish_only || index == 1 {
                None
            } else {
                Some("A")
            }
        );
    }
    let stream = reader.stream();
    assert_eq!(stream.publication.control.ledger.epoch, 2);
    assert_eq!(stream.publication.control.ledger.reserved, 0);
    assert_eq!(
        stream.publication.control.ledger.charged,
        before.publication.control.ledger.charged
            + if approved {
                packets
                    .iter()
                    .map(|packet| packet.action().spec().units)
                    .sum::<u64>()
            } else {
                0
            },
    );
    assert_eq!(
        stream.publication.control.ledger.charged + stream.publication.control.ledger.available,
        reader.profile.delivery.total,
    );
    assert_eq!(
        stream.publication.executions,
        2 + if approved { releases as u64 } else { 0 }
    );
    assert_eq!(
        stream.publication.payload.as_slice(),
        if approved && !finish_only {
            b"AAA".as_slice()
        } else {
            b"AA".as_slice()
        },
    );
    assert_eq!(stream.confirmed, stream.published);
    assert_eq!(stream.confirmed.finished(), approved);
    assert!(stream.pending.is_none());
    assert!(stream.publication.stop.is_none());
    if finish_only {
        assert_eq!(
            reader.progress(8).numerical.numerical,
            before.numerical.numerical
        );
        reader.no_generation(9);
    } else {
        let progress = reader.progress(9);
        assert_eq!(progress.text.command().position(), 8);
        assert_eq!(
            progress.text.finish(),
            Some(Ok(GenerationFinish::StopToken))
        );
        assert_eq!(progress.numerical.numerical.position, 12);
        assert_eq!(progress.numerical.numerical.sampled_draws, 6);
        reader.no_generation(10);
    }
    assert_eq!(fs::read(&previous).unwrap(), previous_bytes);
    if collide {
        assert_eq!(
            fs::read(&next).unwrap(),
            b"independent archive keeps this name"
        );
    } else {
        assert_eq!(archive(&next).revision(), stream.publication.revision);
    }
    assert!(!governor.socket.exists());
    assert!(!selected_operator.settings.socket.exists());
    closed(&profiles, &reviewer);
}

#[test]
fn governed_native_suffix_recovers_current_policy_and_independent_custody() {
    for finish_only in [false, true] {
        resumed_suffix(finish_only, ReviewDecision::Approve, false);
    }
}

#[test]
fn recovered_governor_cannot_replace_fresh_human_effect_approval() {
    resumed_suffix(false, ReviewDecision::Reject, false);
}

#[test]
fn successor_archive_race_preserves_confirmed_governed_native_publication() {
    resumed_suffix(true, ReviewDecision::Approve, true);
}

#[test]
fn governed_recovery_refuses_incomplete_or_changed_independent_contract_before_fence() {
    let root = Root::new();
    let Prefix {
        reviewer,
        reader,
        archive: previous,
        earlier,
        ..
    } = prefix(&root);
    let before = reader.publication();
    let canonical = fs::read(reader.store.join("delivery.bin")).unwrap();
    let previous_bytes = fs::read(&previous).unwrap();
    for variation in 0..10 {
        let config = recovered_config(&root);
        let entries = suffix(&root, &config, &reader, true);
        let (governor, selected_operator) = selections(
            &root,
            &entries[0].actor,
            std::process::id().checked_add(10_000).unwrap(),
        );
        let mut requirements = expected(&config, &entries[0], &governor, &before);
        match variation {
            0 => {
                requirements.effective_policy = Policy::new(
                    2,
                    vec![
                        Predicate::ExactValue {
                            key: 7,
                            value: b"ok".to_vec(),
                        },
                        Predicate::PayloadAtMost(255),
                        Predicate::UnitsAtMost(512),
                        Predicate::All(vec![0, 1, 2]),
                    ],
                )
                .unwrap()
            }
            1 => requirements.guards.campaigns = None,
            2 => {
                requirements
                    .guards
                    .campaigns
                    .as_mut()
                    .unwrap()
                    .max_campaigns += 1
            }
            3 => requirements.minimum.journal_revision += 1,
            4 => requirements.minimum.control_sequence += 1,
            5 => requirements.minimum.authority_epoch += 1,
            6 => requirements.guards.source = None,
            _ => {}
        }
        let next = root.0.join(format!("refused-{variation}.anchor"));
        let retained = history(
            &config,
            &entries,
            &reviewer,
            Some(&selected_operator),
            Some(AFTER),
            Some(&previous),
            &next,
        );
        // The old prefix is valid original history, but it predates an
        // independently retained anchor and the acknowledged policy promotion.
        let selected_bytes = if variation == 7 { &earlier } else { &canonical };
        fs::write(reader.store.join("delivery.bin"), selected_bytes).unwrap();
        let pending = reader.store.join("delivery.pending");
        fs::write(&pending, b"preserve unacknowledged governed staging").unwrap();
        let mut output = Output::new(&entries);
        let profiles = output.profiles.clone();
        let result = run_with_governed_recovery(
            config,
            entries,
            &reviewer,
            None,
            (Some(AFTER), Some(&selected_operator)),
            None,
            (variation != 8).then_some(&retained),
            Some(&governor),
            (variation != 9).then_some(&requirements),
            || ElapsedTick(1001),
            &mut output,
        );
        assert!(result.is_err(), "accepted independent mismatch {variation}");
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        assert_eq!(
            fs::read(reader.store.join("delivery.bin")).unwrap(),
            *selected_bytes
        );
        assert_eq!(
            fs::read(&pending).unwrap(),
            b"preserve unacknowledged governed staging"
        );
        assert_eq!(fs::read(&previous).unwrap(), previous_bytes);
        assert!(
            !next.exists(),
            "refused recovery must not export an acknowledged cut"
        );
        assert!(!governor.socket.exists());
        assert!(!selected_operator.settings.socket.exists());
        closed(&profiles, &reviewer);
        fs::write(reader.store.join("delivery.bin"), &canonical).unwrap();
    }
    assert_eq!(reader.publication(), before);
}

fn replay_length(
    config: &Config,
    inputs: &Inputs,
    requirements: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> usize {
    let selected = bootstrap::selection(config, inputs, None).unwrap();
    let recovery = FileOversight::begin_open_guarded_generated_text_stream_from_source_anchored(
        &config.store,
        config.profile.clone(),
        requirements,
        &inputs.decoder,
        &inputs.tokenizer,
        selected,
        anchor,
    )
    .unwrap();
    recovery.progress().total_events
}

fn stopping_at(reviewer: &PeerProfile, operation: u64) -> JoinHandle<StopControlReceipt> {
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
fn governed_replay_stop_keeps_original_liabilities_and_custody_without_issuing_live_roles() {
    for collide in [false, true] {
        let root = Root::new();
        let Prefix {
            config,
            reviewer,
            reader,
            archive: previous,
            ..
        } = prefix(&root);
        let before = reader.progress(8);
        let canonical = fs::read(reader.store.join("delivery.bin")).unwrap();
        let previous_bytes = fs::read(&previous).unwrap();
        let entries = suffix(&root, &config, &reader, false);
        let (governor, selected_operator) = selections(
            &root,
            &entries[0].actor,
            std::process::id().checked_add(10_000).unwrap(),
        );
        let requirements = expected(&config, &entries[0], &governor, &before.publication);
        let next = root.0.join("governed-stop.anchor");
        let retained = history(
            &config,
            &entries,
            &reviewer,
            Some(&selected_operator),
            Some(AFTER),
            Some(&previous),
            &next,
        );
        let total = replay_length(
            &config,
            &entries[0].inputs,
            &requirements,
            retained.input().unwrap(),
        );
        let stop_at = if collide { 2 } else { total + 2 };
        fs::remove_file(config.source.path()).unwrap();
        let mut output = Output::new(&entries);
        let profiles = output.profiles.clone();
        let mut calls = 0;
        let mut controller = None;
        let error = run_with_governed_recovery(
            config,
            entries,
            &reviewer,
            None,
            (Some(AFTER), Some(&selected_operator)),
            None,
            Some(&retained),
            Some(&governor),
            Some(&requirements),
            || {
                if collide && calls == 0 {
                    fs::write(&next, b"independent terminal archive wins the name").unwrap();
                }
                if calls == stop_at {
                    assert_eq!(
                        fs::read(reader.store.join("delivery.bin")).unwrap(),
                        canonical
                    );
                    assert!(!governor.socket.exists());
                    assert!(!selected_operator.settings.socket.exists());
                    controller = Some(stopping_at(&reviewer, 93));
                }
                calls += 1;
                ElapsedTick(1001)
            },
            &mut output,
        )
        .unwrap_err();
        assert!(
            error.starts_with("native admission Stop acknowledged at revision"),
            "{error}"
        );
        assert_eq!(error.contains("history anchor retention failed:"), collide);
        let receipt = controller
            .expect("original guarded replay accepted the Stop peer")
            .join()
            .unwrap();
        assert!(receipt.acknowledged() && receipt.drained());
        assert_eq!(receipt.binding.operation, 93);
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        let after = reader.publication();
        assert_eq!(after.revision, before.publication.revision + 2);
        assert_eq!(
            after.control.ledger.elapsed,
            before.publication.control.ledger.elapsed
        );
        assert_eq!(
            after.control.ledger.charged,
            before.publication.control.ledger.charged
        );
        assert_eq!(
            after.control.ledger.available,
            before.publication.control.ledger.available
        );
        assert_eq!(after.control.ledger.reserved, 0);
        assert_eq!(after.executions, 2);
        assert_eq!(after.payload, b"AA");
        assert_eq!(after.stop.unwrap().request().operation, 93);
        assert_eq!(
            reader.progress(8).numerical.numerical,
            before.numerical.numerical
        );
        let stream = reader.stream();
        assert_eq!(stream.confirmed, stream.published);
        assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
        assert!(!stream.confirmed.finished());
        reader.no_generation(9);
        assert_eq!(fs::read(&previous).unwrap(), previous_bytes);
        if collide {
            assert_eq!(
                fs::read(&next).unwrap(),
                b"independent terminal archive wins the name"
            );
        } else {
            assert_eq!(archive(&next).revision(), after.revision);
        }
        assert!(!governor.socket.exists());
        assert!(!selected_operator.settings.socket.exists());
        closed(&profiles, &reviewer);
    }
}

#[test]
fn governed_replay_deadline_at_an_event_or_ready_preserves_canonical_and_pending_history() {
    let root = Root::new();
    let Prefix {
        reviewer,
        reader,
        archive: previous,
        ..
    } = prefix(&root);
    let before = reader.publication();
    let canonical = fs::read(reader.store.join("delivery.bin")).unwrap();
    let previous_bytes = fs::read(&previous).unwrap();
    for at_ready in [false, true] {
        let config = recovered_config(&root);
        let entries = suffix(&root, &config, &reader, false);
        let (governor, selected_operator) = selections(
            &root,
            &entries[0].actor,
            std::process::id().checked_add(10_000).unwrap(),
        );
        let requirements = expected(&config, &entries[0], &governor, &before);
        let next = root.0.join(format!("governed-deadline-{at_ready}.anchor"));
        let retained = history(
            &config,
            &entries,
            &reviewer,
            Some(&selected_operator),
            Some(AFTER),
            Some(&previous),
            &next,
        );
        let total = replay_length(
            &config,
            &entries[0].inputs,
            &requirements,
            retained.input().unwrap(),
        );
        let stop_at = if at_ready { total + 2 } else { 2 };
        let pending = reader.store.join("delivery.pending");
        fs::write(&pending, b"deadline must retain original staging").unwrap();
        let mut output = Output::new(&entries);
        let profiles = output.profiles.clone();
        let mut calls = 0;
        let result = run_with_governed_recovery(
            config,
            entries,
            &reviewer,
            None,
            (Some(AFTER), Some(&selected_operator)),
            None,
            Some(&retained),
            Some(&governor),
            Some(&requirements),
            || {
                let current = calls;
                calls += 1;
                ElapsedTick(if current == stop_at { 16_001 } else { 1001 })
            },
            &mut output,
        );
        assert!(result.is_err());
        assert_eq!(calls, stop_at + 1);
        assert!(output.documents.is_empty());
        assert!(output.join().is_empty());
        assert_eq!(
            fs::read(reader.store.join("delivery.bin")).unwrap(),
            canonical
        );
        assert_eq!(
            fs::read(&pending).unwrap(),
            b"deadline must retain original staging"
        );
        assert_eq!(reader.publication(), before);
        assert_eq!(fs::read(&previous).unwrap(), previous_bytes);
        assert!(!next.exists());
        assert!(!governor.socket.exists());
        assert!(!selected_operator.settings.socket.exists());
        closed(&profiles, &reviewer);
    }
}
