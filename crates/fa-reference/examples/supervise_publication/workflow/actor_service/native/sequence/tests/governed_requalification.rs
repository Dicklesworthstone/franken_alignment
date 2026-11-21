//! A recovered joint profile keeps the original governor and human roles while
//! one independently selected capsule requalifies its native resident suffix.
//! Synthetic labels exercise enforcement, never evaluator quality or identity.
use super::*;
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::producer::FilePublicationProducer;

const JOINT_ROOT: &str = "FA_NATIVE_REQUALIFIED_GOVERNOR_ROOT";
const JOINT_CHILD: &str = "workflow::actor_service::native::sequence::tests::governor_tests::recovery_tests::requalification_tests::independent_joint_governor_after_requalification";

struct JointSeed {
    config: Config,
    reviewer: PeerProfile,
    reader: Reader,
    publication: PublicationProfile,
    _producer: FilePublicationProducer,
    previous: PathBuf,
    old: CredibilityActivation,
    fresh: CredibilityActivation,
}

/// Joint GOVERNED first images already have an original library constructor.
/// Seed it explicitly, then drive two real releases through the same native
/// entry lifecycle, authenticated actor/human sockets and helper processes.
/// This does not open the intentionally unsupported CLI creation selection.
fn seed_joint(root: &Root) -> JointSeed {
    let (mut config, mut entries, reviewer) = selected(root, false);
    entries.truncate(2);
    config.programs = helper_programs(root);
    let reader = Reader::new(&config, &entries[0]);
    let (publication, producer) = checked::joint_publication(root, &reader);
    let (governor, _) = selections(root, &entries[0].actor, std::process::id() + 10_000);
    let selected = bootstrap::selection(&config, &entries[0].inputs, Some(&publication)).unwrap();
    let (host, human_role, _governor_role) = bootstrap::prepare_governed(
        &config,
        &entries[0].inputs,
        selected,
        governor.campaign_requirement(),
    )
    .unwrap();
    assert!(host.held_out_joint_policy().unwrap().is_some());
    assert!(host.policy_campaigns_required());
    let previous = root.0.join("joint-prefix.anchor");
    let history = history(&config, &entries, &reviewer, None, None, None, &previous);
    let mut retained = custody::Retention::new(Some(&history));
    let (port, supervisor) = host.into_generated_text_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut control = Control::new(&config, REQUESTS[0], Some(&reviewer)).unwrap();
    let deadline = Deadline {
        logical: ElapsedTick(16_000),
        started: Instant::now(),
        wall: Duration::from_secs(15),
    };
    let mut output = Output::new(&entries);
    let mut humans = Humans::new(&reviewer, &reader, [Some(ReviewDecision::Approve); 3]);
    let programs = config.programs.clone();
    let mut time = || {
        humans.poll();
        ElapsedTick(1000)
    };
    let mut previous_request = None;
    for entry in &entries {
        if let Some(previous) = previous_request {
            assert!(
                retire(
                    &mut driver,
                    previous,
                    &config,
                    (&mut control, &human_role, &deadline),
                    &mut time
                )
                .unwrap()
            );
            config.programs = programs.clone();
        }
        let socket = BoundSocket::bind(&entry.actor.socket, None).unwrap();
        entry.actor.secure_socket().unwrap();
        let inbox = FileActorInbox::new(
            entry.actor.actor,
            ActorWire::new(port.clone()),
            entry.actor.channels(),
            entry.actor.connections,
        )
        .unwrap();
        let pool = FileActorPool::for_requests(vec![(entry.actor.request, inbox)]).unwrap();
        let mut ingress = Intake {
            socket,
            pool,
            profile: entry.actor.clone(),
            attempted: 0,
        };
        assert!(
            execute_entry(
                &mut driver,
                &mut config,
                entry,
                previous_request,
                None,
                EntryServices {
                    control: &mut control,
                    reviewer: &human_role,
                    peers: &reviewer,
                    publication: Some(&publication),
                    deadline: &deadline,
                    ingress: &mut ingress,
                    governed: false
                },
                &mut time,
                &mut output
            )
            .unwrap()
        );
        assert!(
            !reply_grace(
                &mut driver,
                &mut ingress,
                (&mut control, &human_role, &deadline),
                true,
                &mut time
            )
            .unwrap()
        );
        ingress.pool.revoke_all();
        previous_request = Some(entry.actor.request);
    }
    assert!(
        retire(
            &mut driver,
            AFTER,
            &config,
            (&mut control, &human_role, &deadline),
            &mut time
        )
        .unwrap()
    );
    let (old, fresh) = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let before = host.inspect();
        assert_eq!(before.executions, 2);
        assert_eq!(before.payload, b"AA");
        let old = qualification_tests::evidence::activation(
            &config,
            before.control.sequence,
            before.control.ledger.epoch,
            20,
            2,
            2,
        );
        host.activate_credibility(host.revision(), old.clone(), &config.profile.committee)
            .unwrap();
        host.check_credibility().unwrap();
        assert!(host.held_out_joint_report(old.operation).unwrap().is_some());
        let current = host.inspect().control;
        let fresh = qualification_tests::evidence::activation(
            &config,
            current.sequence,
            current.ledger.epoch + 1,
            21,
            3,
            2,
        );
        (old, fresh)
    };
    retained.observe_driver(&driver);
    assert_eq!(
        cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms),
        0
    );
    retained.finish(Ok(())).unwrap();
    assert!(output.join().iter().all(executed));
    let packets = humans.join();
    assert!(packets[0].is_some() && packets[1].is_some() && packets[2].is_none());
    assert_eq!(archive(&previous).revision(), reader.publication().revision);
    assert_eq!(reader.progress(8).numerical.numerical.sampled_draws, 4);
    config.programs = programs;
    write_complete(
        config.source.path(),
        current_evidence(config.profile.delivery.scope),
    );
    JointSeed {
        config,
        reviewer,
        reader,
        publication,
        _producer: producer,
        previous,
        old,
        fresh,
    }
}

fn requirements(
    seed: &JointSeed,
    entry: &Entry,
    governor: &live_governor::Settings,
) -> FileRecoveryRequirements {
    let mut requirements = expected(&seed.config, entry, governor, &seed.reader.publication());
    requirements.effective_policy = seed.config.profile.delivery.policy.clone();
    requirements
}

fn joint_child(root: &Root) -> GovernorChild {
    GovernorChild(
        ProcessCommand::new(std::env::current_exe().unwrap())
            .args(["--exact", JOINT_CHILD, "--nocapture"])
            .env(JOINT_ROOT, &root.0)
            .spawn()
            .unwrap(),
    )
}

#[test]
fn independent_joint_governor_after_requalification() {
    let Some(root) = std::env::var_os(JOINT_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    wait(&root.join("joint-ready"));
    let settings = live_governor::Settings::read(&root.join("governor.json")).unwrap();
    let scope = live_governor::protocol::scope_json(settings.scope);
    let inspected = inspect(&root, &scope, 1);
    let cut = known(&inspected, "inspect");
    assert_eq!(number(cut, "authority_epoch"), 3);
    assert_eq!(number(cut, "policy_generation"), 1);
    let reviewed = exchange(
        &root,
        "joint-review.json",
        campaign_request(&scope, cut, CAMPAIGN),
    );
    let review = known(&reviewed, "review");
    let report = field(review, "reviewed_report").as_str().unwrap();
    let report_value = json(report.as_bytes());
    let replay = field(field(&report_value, "knowledge"), "value");
    assert_eq!(field(replay, "case_count").as_u64(), Some(4));
    assert_eq!(field(field(replay, "cut"), "executions").as_u64(), Some(2));
    assert_eq!(field(replay, "requires_shadow").as_bool(), Some(false));
    let approved = exchange(
        &root,
        "joint-approval.json",
        format!(
            r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"promote","phase":"approve","campaign":"{CAMPAIGN}","expected_journal_revision":{},"reviewed_report":{},"accept_newly_reviewable":false}}"#,
            live_governor::protocol::json(field(review, "journal_revision")),
            live_governor::protocol::quote(report)
        ),
    );
    let approval = known(&approved, "approve");
    assert_eq!(field(approval, "effect_authority").as_bool(), Some(false));
    assert_eq!(field(approval, "policy_changed").as_bool(), Some(false));
    let inspected = inspect(&root, &scope, 2);
    let current = known(&inspected, "inspect");
    assert_eq!(number(current, "authority_epoch"), 3);
    assert_eq!(number(current, "policy_generation"), 1);
    assert!(number(field(current, "numerical_replay"), "remaining_events") < 8192);
    write_complete(
        &root.join("joint-approved"),
        b"independent complete-corpus approval",
    );
}

/// First release waits for independently authenticated governance. Removing the
/// capsule after that first original reference proves later handoffs cannot
/// silently reactivate it. A policy-promotion variant sends only a freshly
/// emitted reference; it never fabricates a replacement epoch itself.
struct JointOutput {
    profiles: Vec<Profile>,
    pending: Vec<u8>,
    documents: Vec<Vec<u8>>,
    clients: Vec<JoinHandle<WireResponse>>,
    directory: PathBuf,
    capsule: PathBuf,
    released: Arc<AtomicBool>,
    promote: bool,
}

impl JointOutput {
    fn new(root: &Root, entries: &[Entry], capsule: &Path, promote: bool) -> Self {
        Self {
            profiles: entries.iter().map(|entry| entry.actor.clone()).collect(),
            pending: Vec::new(),
            documents: Vec::new(),
            clients: Vec::new(),
            directory: root.0.clone(),
            capsule: capsule.to_path_buf(),
            released: Arc::new(AtomicBool::new(false)),
            promote,
        }
    }

    fn join(self) -> Vec<WireResponse> {
        assert!(self.pending.is_empty());
        self.clients
            .into_iter()
            .map(|client| client.join().unwrap())
            .collect()
    }
}

impl Write for JointOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let document = std::mem::take(&mut self.pending);
        let Command::Submit { request, proposal } = decode_command(&document).unwrap() else {
            panic!("original native source reference required");
        };
        if self.documents.is_empty() {
            assert_eq!(proposal.expected_policy_epoch, 3);
            fs::remove_file(&self.capsule)?;
            write_complete(
                &self.directory.join("joint-ready"),
                b"original activated source intent",
            );
        }
        if !self.promote || proposal.expected_policy_epoch == 4 {
            let profile = self
                .profiles
                .iter()
                .find(|profile| profile.request == request)
                .unwrap()
                .clone();
            let released = self.released.clone();
            let submitted = document.clone();
            self.clients.push(thread::spawn(move || {
                let started = Instant::now();
                while !released.load(Ordering::Acquire) {
                    assert!(started.elapsed() < Duration::from_secs(25));
                    pause(1);
                }
                actor(profile, submitted).join().unwrap()
            }));
        }
        self.documents.push(document);
        Ok(())
    }
}

fn qualified_suffix(finish_only: bool, decision: ReviewDecision, promote: bool) {
    let root = Root::new();
    let seed = seed_joint(&root);
    let before = seed.reader.progress(8);
    let previous_bytes = fs::read(&seed.previous).unwrap();
    let entries = suffix(&root, &seed.config, &seed.reader, finish_only);
    let mut child = joint_child(&root);
    let (governor, selected_operator) = selections(&root, &entries[0].actor, child.0.id());
    let requirements = requirements(&seed, &entries[0], &governor);
    let capsule = root.0.join("joint-qualification.bin");
    fs::write(&capsule, seed.fresh.encode_reference().unwrap()).unwrap();
    let next = root.0.join("joint-successor.anchor");
    let retained = history(
        &seed.config,
        &entries,
        &seed.reviewer,
        Some(&selected_operator),
        Some(AFTER),
        Some(&seed.previous),
        &next,
    );
    let mut output = JointOutput::new(&root, &entries, &capsule, promote);
    let released = output.released.clone();
    let directory = root.0.clone();
    let settings = selected_operator.settings.clone();
    let scope = entries[0].actor.scope;
    let peer_store = seed.config.store.clone();
    let peer_profile = seed.config.profile.clone();
    let peer = thread::spawn(move || {
        wait(&directory.join("joint-approved"));
        let before = FileOversight::read_publication(&peer_store, &peer_profile).unwrap();
        assert_eq!(before.executions, 2);
        assert_eq!(before.control.ledger.epoch, 3);
        let mut socket = operator_tests::connect(&settings);
        let mut request = TowerRequest::new(
            "joint-policy-next",
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
        let planned = operator_exchange(&mut socket, &encode_request(&request).unwrap());
        let next = operator_known(&planned, "policy-promotion-next");
        assert_eq!(field(next, "can_promote").as_bool(), Some(true));
        if promote {
            let action = live_governor::protocol::json(field(next, "action"));
            operator_known(&operator_exchange(&mut socket, &action), "policy-promotion");
        }
        let after = FileOversight::read_publication(&peer_store, &peer_profile).unwrap();
        assert_eq!(after.control.ledger.epoch, 3 + u64::from(promote));
        assert_eq!(
            (after.executions, after.control.ledger.charged),
            (before.executions, before.control.ledger.charged)
        );
        released.store(true, Ordering::Release);
    });
    let mut humans = RecoveredHumans::new(&seed.reviewer, &seed.config, &entries, decision);
    let profiles = output.profiles.clone();
    let result = run_with_governed_recovery(
        seed.config,
        entries,
        &seed.reviewer,
        Some(&seed.publication),
        (Some(AFTER), Some(&selected_operator)),
        Some(&capsule),
        Some(&retained),
        Some(&governor),
        Some(&requirements),
        || {
            humans.poll();
            ElapsedTick(1001)
        },
        &mut output,
    );
    peer.join().unwrap();
    child.finish();
    assert!(
        !capsule.exists(),
        "qualification must precede the first source reference"
    );
    assert_eq!(fs::read(&seed.previous).unwrap(), previous_bytes);
    let documents = output.documents.clone();
    let replies = output.join();
    let packets = humans.join();
    let after = seed.reader.stream();
    let approved = !promote && decision == ReviewDecision::Approve;
    if approved {
        assert!(result.is_ok(), "{result:?}");
        let releases = if finish_only { 1 } else { 2 };
        assert_eq!(replies.len(), releases);
        assert_eq!(packets.len(), releases);
        assert!(replies.iter().all(executed));
        assert_eq!(after.publication.executions, 2 + releases as u64);
        assert_eq!(
            after.publication.payload,
            if finish_only {
                b"AA".to_vec()
            } else {
                b"AAA".to_vec()
            }
        );
        assert!(after.confirmed.finished());
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged
                + packets
                    .iter()
                    .map(|packet| packet.action().spec().units)
                    .sum::<u64>()
        );
    } else {
        assert!(result.is_err());
        assert!(replies.iter().all(|reply| !executed(reply)));
        assert_eq!(after.publication.executions, 2);
        assert_eq!(after.publication.payload, b"AA");
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged
        );
        assert!(!after.confirmed.finished());
        if !promote {
            assert_eq!(packets.len(), 1);
        }
    }
    assert_eq!(after.confirmed, after.published);
    assert_eq!(after.publication.control.ledger.reserved, 0);
    assert_eq!(
        after.publication.control.ledger.epoch,
        3 + u64::from(promote)
    );
    assert_eq!(archive(&next).revision(), after.publication.revision);
    if finish_only {
        assert_eq!(
            seed.reader.progress(8).numerical.numerical,
            before.numerical.numerical
        );
        seed.reader.no_generation(9);
    } else {
        assert_eq!(seed.reader.progress(9).numerical.numerical.sampled_draws, 6);
        seed.reader.no_generation(10);
    }
    for packet in &packets {
        assert_eq!(packet.action().spec().policy_epoch, 3);
    }
    for document in &documents {
        let Command::Submit { proposal, .. } = decode_command(document).unwrap() else {
            unreachable!()
        };
        assert!(
            proposal.expected_policy_epoch == 3 || (promote && proposal.expected_policy_epoch == 4)
        );
    }
    assert!(!governor.socket.exists() && !selected_operator.settings.socket.exists());
    closed(&profiles, &seed.reviewer);
}

#[test]
fn governed_joint_requalification_keeps_one_owner_for_freshly_reviewed_message_and_finish() {
    qualified_suffix(false, ReviewDecision::Approve, false);
}

#[test]
fn governed_joint_requalification_allows_singleton_finish_without_new_inference() {
    qualified_suffix(true, ReviewDecision::Approve, false);
}

#[test]
fn governor_approval_and_joint_requalification_cannot_replace_independent_human_approval() {
    qualified_suffix(false, ReviewDecision::Reject, false);
}

#[test]
fn a_later_policy_promotion_invalidates_joint_qualification_before_any_new_publication() {
    qualified_suffix(false, ReviewDecision::Approve, true);
}

#[test]
fn governed_requalification_needs_fresh_evidence_and_exact_disclosure_epoch() {
    for cause in [
        "unselected",
        "missing",
        "historical",
        "stale_predecessor",
        "wrong_policy",
        "wrong_precheck",
        "source_missing",
    ] {
        let root = Root::new();
        let seed = seed_joint(&root);
        let before = seed.reader.progress(8);
        let mut entries = suffix(&root, &seed.config, &seed.reader, false);
        let (governor, selected_operator) =
            selections(&root, &entries[0].actor, std::process::id() + 10_000);
        let requirements = requirements(&seed, &entries[0], &governor);
        let path = root.0.join("qualification.bin");
        let mut fresh = seed.fresh.clone();
        match cause {
            "unselected" | "missing" => {}
            "historical" => fs::write(&path, seed.old.encode_reference().unwrap()).unwrap(),
            "wrong_precheck" => {
                precheck_tests::select(&root, &mut entries, fresh.expected_epoch);
            }
            "source_missing" => {
                fs::write(&path, fresh.encode_reference().unwrap()).unwrap();
                fs::remove_file(seed.config.source.path()).unwrap();
            }
            _ => {
                if cause == "stale_predecessor" {
                    fresh.expected_control_sequence -= 1;
                } else {
                    fresh.policy_generation += 1;
                }
                fs::write(&path, fresh.encode_reference().unwrap()).unwrap();
            }
        }
        let next = root.0.join("failed-successor.anchor");
        let retained = history(
            &seed.config,
            &entries,
            &seed.reviewer,
            Some(&selected_operator),
            Some(AFTER),
            Some(&seed.previous),
            &next,
        );
        let mut output = Vec::new();
        let error = run_with_governed_recovery(
            seed.config,
            entries,
            &seed.reviewer,
            Some(&seed.publication),
            (Some(AFTER), Some(&selected_operator)),
            if cause == "unselected" {
                None
            } else {
                Some(&path)
            },
            Some(&retained),
            Some(&governor),
            Some(&requirements),
            || ElapsedTick(1001),
            &mut output,
        )
        .unwrap_err();
        if cause == "wrong_precheck" {
            assert!(error.contains("selected activation successor"), "{error}");
        }
        assert!(output.is_empty(), "{cause}");
        let after = seed.reader.progress(8);
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        assert_eq!(after.publication.payload, before.publication.payload);
        assert_eq!(after.publication.executions, before.publication.executions);
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged
        );
        assert_eq!(after.publication.control.ledger.reserved, 0);
        seed.reader.no_generation(9);
        assert_eq!(archive(&next).revision(), after.publication.revision);
    }
}

#[test]
fn governed_requalification_checks_the_original_deadline_after_activation_before_inference() {
    let root = Root::new();
    let seed = seed_joint(&root);
    let before = seed.reader.progress(8);
    let entries = suffix(&root, &seed.config, &seed.reader, false);
    let (governor, selected_operator) =
        selections(&root, &entries[0].actor, std::process::id() + 10_000);
    let requirements = requirements(&seed, &entries[0], &governor);
    let capsule = root.0.join("deadline-qualification.bin");
    fs::write(&capsule, seed.fresh.encode_reference().unwrap()).unwrap();
    let next = root.0.join("deadline-successor.anchor");
    let retained = history(
        &seed.config,
        &entries,
        &seed.reviewer,
        Some(&selected_operator),
        Some(AFTER),
        Some(&seed.previous),
        &next,
    );
    let mut observed = false;
    let mut output = Vec::new();
    let error = run_with_governed_recovery(
        seed.config,
        entries,
        &seed.reviewer,
        Some(&seed.publication),
        (Some(AFTER), Some(&selected_operator)),
        Some(&capsule),
        Some(&retained),
        Some(&governor),
        Some(&requirements),
        || {
            observed |= seed.reader.publication().control.ledger.epoch == 3;
            ElapsedTick(if observed { 100_000 } else { 1001 })
        },
        &mut output,
    )
    .unwrap_err();
    assert!(observed && error.contains("deadline"), "{error}");
    assert!(output.is_empty());
    let after = seed.reader.progress(8);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(after.publication.executions, before.publication.executions);
    assert_eq!(after.publication.payload, before.publication.payload);
    assert_eq!(
        after.publication.control.ledger.charged,
        before.publication.control.ledger.charged
    );
    assert!(after.publication.stop.is_some());
    assert_eq!(archive(&next).revision(), after.publication.revision);
    seed.reader.no_generation(9);
}

#[test]
fn governed_requalification_reserves_capsule_against_governor_endpoint_before_replay() {
    let root = Root::new();
    let seed = seed_joint(&root);
    let canonical = fs::read(seed.config.store.join("delivery.bin")).unwrap();
    let entries = suffix(&root, &seed.config, &seed.reader, false);
    let (governor, selected_operator) =
        selections(&root, &entries[0].actor, std::process::id() + 10_000);
    let requirements = requirements(&seed, &entries[0], &governor);
    let next = root.0.join("must-not-be-created.anchor");
    let retained = history(
        &seed.config,
        &entries,
        &seed.reviewer,
        Some(&selected_operator),
        Some(AFTER),
        Some(&seed.previous),
        &next,
    );
    let error = run_with_governed_recovery(
        seed.config,
        entries,
        &seed.reviewer,
        Some(&seed.publication),
        (Some(AFTER), Some(&selected_operator)),
        Some(&governor.socket),
        Some(&retained),
        Some(&governor),
        Some(&requirements),
        || panic!("capsule/listener collision reached time or replay"),
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(error.contains("conflict"), "{error}");
    assert_eq!(
        fs::read(seed.reader.store.join("delivery.bin")).unwrap(),
        canonical
    );
    assert!(
        !next.exists() && !governor.socket.exists() && !selected_operator.settings.socket.exists()
    );
}
