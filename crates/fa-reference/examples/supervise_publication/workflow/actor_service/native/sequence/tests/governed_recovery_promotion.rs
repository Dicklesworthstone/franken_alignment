//! New live policy work after an acknowledged recovery must still traverse the
//! original independent governor, numerical operator and human effect gates.
use super::*;

const NEXT_CAMPAIGN: u64 = 703;
const NEXT_OPERATION: u64 = 704;
const PROMOTION_ROOT: &str = "FA_NATIVE_RECOVERED_PROMOTION_ROOT";
const PROMOTION_TEST: &str = "workflow::actor_service::native::sequence::tests::governor_tests::recovery_tests::promotion_tests::independent_governor_approves_after_recovery";

fn promotion_child(root: &Root) -> GovernorChild {
    GovernorChild(
        ProcessCommand::new(std::env::current_exe().unwrap())
            .args(["--exact", PROMOTION_TEST, "--nocapture"])
            .env(PROMOTION_ROOT, &root.0)
            .spawn()
            .unwrap(),
    )
}

fn next_campaign(scope: &str, cut: &Json, campaign: u64) -> String {
    format!(
        r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"policy-replay","operation":"{campaign}","expected":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{},"policy_generation":"2"}},"candidate":{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":2,"limits":{{"cases":16,"input_bytes":1048576,"output_bytes":65536}},"candidate":{{"generation":3,"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":256}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}}}}}}"#,
        live_governor::protocol::json(field(cut, "journal_revision")),
        live_governor::protocol::json(field(cut, "control_sequence")),
        live_governor::protocol::json(field(cut, "authority_epoch")),
    )
}

/// This custodian receives no approval handle from the parent. Its new
/// authenticated session reviews and approves the COMPLETE recovered report.
#[test]
fn independent_governor_approves_after_recovery() {
    let Some(root) = std::env::var_os(PROMOTION_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    wait(&root.join("recovered-ready"));
    let settings = live_governor::Settings::read(&root.join("governor.json")).unwrap();
    let scope = live_governor::protocol::scope_json(settings.scope);
    let inspected = inspect(&root, &scope, 1);
    let before = known(&inspected, "inspect");
    assert_eq!(number(before, "authority_epoch"), 2);
    assert_eq!(number(before, "policy_generation"), 2);
    assert_eq!(
        number(field(before, "numerical_replay"), "remaining_events"),
        8192
    );
    assert!(!root.join("evidence.json").exists());
    let reviewed = exchange(
        &root,
        "recovered-review.json",
        next_campaign(&scope, before, NEXT_CAMPAIGN),
    );
    let value = known(&reviewed, "review");
    let report = field(value, "reviewed_report").as_str().unwrap();
    let decoded = json(report.as_bytes());
    let replay = field(field(&decoded, "knowledge"), "value");
    // The second Message contributes its proposal/review under current policy
    // two. The first Message used policy one; the absent request adds no case.
    assert_eq!(field(replay, "case_count").as_u64(), Some(2));
    assert_eq!(field(field(replay, "cut"), "executions").as_u64(), Some(2));
    assert_eq!(field(replay, "requires_shadow").as_bool(), Some(false));
    assert!(
        field(replay, "newly_reviewable")
            .as_array()
            .unwrap()
            .is_empty()
    );

    let inspected = inspect(&root, &scope, 2);
    let pending = known(&inspected, "inspect");
    assert_eq!(
        number(pending, "journal_revision"),
        number(before, "journal_revision") + 1
    );
    assert_eq!(
        number(field(pending, "numerical_replay"), "remaining_events"),
        8192 - number(before, "journal_revision") - 1,
    );
    let canonical = fs::read(root.join("store/delivery.bin")).unwrap();
    let exhausted = exchange(
        &root,
        "recovered-exhausted.json",
        next_campaign(&scope, pending, NEXT_CAMPAIGN + 1),
    );
    assert_eq!(
        field(field(&exhausted, "error"), "code").as_str(),
        Some("capacity")
    );
    assert_eq!(
        fs::read(root.join("store/delivery.bin")).unwrap(),
        canonical
    );

    let approved = exchange(
        &root,
        "recovered-approve.json",
        format!(
            r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"promote","phase":"approve","campaign":"{NEXT_CAMPAIGN}","expected_journal_revision":{},"reviewed_report":{},"accept_newly_reviewable":false}}"#,
            live_governor::protocol::json(field(value, "journal_revision")),
            live_governor::protocol::quote(report),
        ),
    );
    let approval = known(&approved, "approve");
    assert_eq!(field(approval, "effect_authority").as_bool(), Some(false));
    assert_eq!(field(approval, "policy_changed").as_bool(), Some(false));
    let inspected = inspect(&root, &scope, 3);
    let after = known(&inspected, "inspect");
    assert_eq!(number(after, "authority_epoch"), 2);
    assert_eq!(number(after, "policy_generation"), 2);
    assert_eq!(
        number(field(after, "numerical_replay"), "remaining_events"),
        8192 - (number(before, "journal_revision") + 1) - (number(pending, "journal_revision") + 1),
        "reconnect must retain both entered replay charges",
    );
    write_complete(
        &root.join("recovered-approved"),
        live_governor::protocol::json(&approved),
    );
    wait(&root.join("recovered-complete"));
}

fn fresh_evidence(scope: fa_reference::action::Scope) -> Vec<u8> {
    EvidenceSnapshot::new(
        EvidenceIdentity {
            scope,
            source: 51,
            generation: 3,
        },
        Snapshot {
            semantic_epoch: 3,
            complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]),
        },
        ["alpha", "beta"]
            .into_iter()
            .map(|member| {
                (
                    member.to_owned(),
                    format!("review this fixture: {member}; {FRESH_SOURCE}; generation three")
                        .into_bytes(),
                )
            })
            .collect(),
    )
    .unwrap()
    .encode()
}

/// No actor submits the first source reference. A real policy promotion must
/// emit its one refreshed epoch before the independently selected actor acts.
struct PromotedOutput {
    root: PathBuf,
    actor: Profile,
    pending: Vec<u8>,
    documents: Vec<Vec<u8>>,
    released: Arc<AtomicBool>,
    refreshed: Arc<AtomicBool>,
    client: Option<JoinHandle<WireResponse>>,
}

impl PromotedOutput {
    fn new(root: &Root, entry: &Entry) -> Self {
        Self {
            root: root.0.clone(),
            actor: entry.actor.clone(),
            pending: Vec::new(),
            documents: Vec::new(),
            released: Arc::new(AtomicBool::new(false)),
            refreshed: Arc::new(AtomicBool::new(false)),
            client: None,
        }
    }
}

impl Write for PromotedOutput {
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
            panic!("recovered generation must emit its original source reference");
        };
        assert_eq!(request, 93);
        assert_eq!(
            proposal.payload.len(),
            FileGeneratedTextActorPort::INTENT_BYTES
        );
        match self.documents.len() {
            0 => {
                assert_eq!(proposal.expected_policy_epoch, 2);
                write_complete(&self.root.join("recovered-offered"), &document);
            }
            1 => {
                assert_eq!(proposal.expected_policy_epoch, 3);
                let Command::Submit {
                    proposal: original, ..
                } = decode_command(&self.documents[0]).unwrap()
                else {
                    unreachable!()
                };
                assert_eq!(proposal.payload, original.payload);
                assert_eq!(
                    (proposal.target, proposal.units, proposal.deadline),
                    (original.target, original.units, original.deadline)
                );
                let profile = self.actor.clone();
                let released = self.released.clone();
                let submitted = document.clone();
                self.client = Some(thread::spawn(move || {
                    let started = Instant::now();
                    while !released.load(Ordering::Acquire) {
                        assert!(started.elapsed() < Duration::from_secs(25));
                        pause(1);
                    }
                    actor(profile, submitted).join().unwrap()
                }));
                self.refreshed.store(true, Ordering::Release);
            }
            _ => panic!("one promotion must not repeatedly renew the same original intent"),
        }
        self.documents.push(document);
        Ok(())
    }
}

#[test]
fn recovered_governor_promotes_again_before_a_fresh_human_approved_native_effect() {
    recovered_promotion(ReviewDecision::Approve);
}

#[test]
fn recovered_governor_promotion_cannot_override_a_fresh_human_rejection() {
    recovered_promotion(ReviewDecision::Reject);
}

fn recovered_promotion(decision: ReviewDecision) {
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
    let mut entries = suffix(&root, &config, &reader, false);
    entries.truncate(1);
    let peer_store = config.store.clone();
    let peer_profile = config.profile.clone();
    let peer_stream = entries[0].inputs.stream;
    let mut child = promotion_child(&root);
    let (governor, selected_operator) = selections(&root, &entries[0].actor, child.0.id());
    let requirements = expected(&config, &entries[0], &governor, &before.publication);
    assert_eq!(
        requirements.minimum.journal_revision,
        archive(&previous).revision()
    );
    let next = root.0.join("promoted-recovery-successor.anchor");
    let retained = history(
        &config,
        &entries,
        &reviewer,
        Some(&selected_operator),
        Some(AFTER),
        Some(&previous),
        &next,
    );
    let mut output = PromotedOutput::new(&root, &entries[0]);
    let release_actor = output.released.clone();
    let refreshed = output.refreshed.clone();
    let profile = entries[0].actor.clone();
    let mut humans = RecoveredHumans::new(&reviewer, &config, &entries, decision);
    let human_ready = Arc::new(AtomicBool::new(false));
    let release_human = human_ready.clone();
    let directory = root.0.clone();
    let source = config.source.path().to_owned();
    let scope = config.profile.delivery.scope;
    let fresh = fresh_evidence(scope);
    let settings = selected_operator.settings.clone();
    let human_socket = reviewer.socket(93);
    let operator = thread::spawn(move || {
        // Reconstruct the identical immutable decoder assets in this thread;
        // their Rc buffers must never cross the independent peer boundary.
        let inputs = recipe::load(
            &directory.join("recipe.json"),
            peer_profile.delivery.scope.tenant,
            peer_profile.delivery.limits.bytes,
        )
        .unwrap();
        let peer_reader = Reader {
            store: peer_store,
            profile: peer_profile,
            decoder: inputs.decoder,
            tokenizer: inputs.tokenizer,
            stream: peer_stream,
        };
        wait(&directory.join("recovered-offered"));
        let before = peer_reader.publication();
        assert_eq!((before.executions, before.control.ledger.epoch), (2, 2));
        assert_eq!(before.payload, b"AA");
        let numerical = peer_reader.progress(9).numerical.numerical;
        assert_eq!((numerical.position, numerical.sampled_draws), (12, 6));
        fs::remove_file(&source).unwrap();
        let mut socket = operator_tests::connect(&settings);
        let mut request = TowerRequest::new(
            "native-promotion-after-recovery",
            Address::parse(&format!(
                "fa://{}/run/{}@{GENERATION}",
                scope.tenant, scope.run
            ))
            .unwrap(),
            Verb::Next,
        );
        request.arguments = ReadArguments::PolicyPromotionNext {
            campaign: NEXT_CAMPAIGN,
            operation: NEXT_OPERATION,
        };
        let next = encode_request(&request).unwrap();
        let unavailable = operator_exchange(&mut socket, &next);
        assert_eq!(
            field(field(&unavailable, "envelope"), "reason_code").as_str(),
            Some("CapabilityUnsupported")
        );
        write_complete(
            &directory.join("recovered-ready"),
            b"original guarded recovery is complete",
        );
        wait(&directory.join("recovered-approved"));
        let approved = peer_reader.publication();
        assert_eq!(approved.revision, before.revision + 2);
        assert_eq!(approved.control.ledger.epoch, 2);
        let planned = operator_exchange(&mut socket, &next);
        let plan = operator_known(&planned, "policy-promotion-next");
        assert_eq!(field(plan, "can_promote").as_bool(), Some(true));
        let command = live_governor::protocol::json(field(plan, "action"));
        let promoted = operator_exchange(&mut socket, &command);
        operator_known(&promoted, "policy-promotion");
        let after = peer_reader.publication();
        assert_eq!(after.revision, approved.revision + 1);
        assert_eq!(after.control.ledger.epoch, 3);
        assert_eq!(
            (after.executions, after.control.ledger.charged),
            (before.executions, before.control.ledger.charged)
        );
        assert_eq!(peer_reader.progress(9).numerical.numerical, numerical);
        assert!(
            !source.exists(),
            "retained campaign replay must not read live evidence"
        );
        let started = Instant::now();
        while !refreshed.load(Ordering::Acquire) {
            assert!(started.elapsed() < Duration::from_secs(25));
            pause(1);
        }
        write_complete(&source, fresh);
        release_actor.store(true, Ordering::Release);
        wait(&human_socket);
        assert_eq!(peer_reader.publication().executions, 2);
        let canonical = fs::read(directory.join("store/delivery.bin")).unwrap();
        assert_eq!(operator_exchange(&mut socket, &command), promoted);
        assert_eq!(
            fs::read(directory.join("store/delivery.bin")).unwrap(),
            canonical
        );
        release_human.store(true, Ordering::Release);
    });
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
            if human_ready.load(Ordering::Acquire) {
                humans.poll();
            }
            ElapsedTick(1001)
        },
        &mut output,
    );
    operator.join().unwrap();
    assert_eq!(output.documents.len(), 2);
    assert!(output.pending.is_empty());
    let response = output.client.take().unwrap().join().unwrap();
    let packets = humans.join();
    assert_eq!(packets.len(), 1);
    let packet = &packets[0];
    assert_eq!(packet.policy_generation(), 3);
    assert_eq!(packet.action().spec().policy_epoch, 3);
    assert_eq!(packet.views().len(), 2);
    assert!(
        original
            .iter()
            .all(|old| old.binding().session != packet.binding().session)
    );
    let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["A", "A"]);
    assert_eq!(frame.message(), Some("A"));
    assert!(!frame.is_finish());
    let after = reader.stream();
    assert_eq!(reader.progress(9).numerical.numerical.position, 12);
    assert_eq!(reader.progress(9).numerical.numerical.sampled_draws, 6);
    reader.no_generation(10);
    assert_eq!(after.publication.control.ledger.epoch, 3);
    assert_eq!(after.publication.control.ledger.reserved, 0);
    assert_eq!(after.confirmed, after.published);
    assert!(!after.confirmed.finished());
    assert_eq!(archive(&next).revision(), after.publication.revision);
    assert_eq!(fs::read(&previous).unwrap(), previous_bytes);
    closed(&[profile], &reviewer);
    assert!(!governor.socket.exists());
    assert!(!selected_operator.settings.socket.exists());
    if decision == ReviewDecision::Reject {
        assert!(result.is_err());
        assert!(!executed(&response));
        assert_eq!(after.publication.executions, 2);
        assert_eq!(after.publication.payload, b"AA");
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged
        );
    } else {
        assert!(
            result.is_ok(),
            "recovered promotion publication failed: {result:?}"
        );
        assert!(executed(&response));
        assert_eq!(after.publication.executions, 3);
        assert_eq!(after.publication.payload, b"AAA");
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged + packet.action().spec().units
        );
    }
    write_complete(
        &root.0.join("recovered-complete"),
        b"fresh effect judgment is complete",
    );
    child.finish();
}

#[test]
fn governed_recovery_retains_its_fence_when_a_superseded_predecessor_refuses_the_suffix() {
    let root = Root::new();
    let Prefix {
        config,
        reviewer,
        reader,
        archive: previous,
        ..
    } = prefix(&root);
    let before = reader.progress(8);
    let mut entries = suffix(&root, &config, &reader, false);
    entries.truncate(1);
    let (governor, selected_operator) = selections(
        &root,
        &entries[0].actor,
        std::process::id().checked_add(10_000).unwrap(),
    );
    let requirements = expected(&config, &entries[0], &governor, &before.publication);
    let next = root.0.join("refused-suffix-fence.anchor");
    let retained = history(
        &config,
        &entries,
        &reviewer,
        Some(&selected_operator),
        Some(REQUESTS[0]),
        Some(&previous),
        &next,
    );
    fs::remove_file(config.source.path()).unwrap();
    let previous_bytes = fs::read(&previous).unwrap();
    let mut output = Output::new(&entries);
    let profiles = output.profiles.clone();
    let error = run_with_governed_recovery(
        config,
        entries,
        &reviewer,
        None,
        (Some(REQUESTS[0]), Some(&selected_operator)),
        None,
        Some(&retained),
        Some(&governor),
        Some(&requirements),
        || ElapsedTick(1001),
        &mut output,
    )
    .unwrap_err();
    assert!(
        !error.contains("history anchor retention failed:"),
        "{error}"
    );
    assert!(output.documents.is_empty());
    assert!(output.join().is_empty());
    let after = reader.progress(8);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(after.publication.revision, before.publication.revision + 1);
    assert_eq!(
        after.publication.control.ledger.epoch,
        before.publication.control.ledger.epoch + 1
    );
    assert_eq!(after.publication.executions, before.publication.executions);
    assert_eq!(after.publication.payload, before.publication.payload);
    assert_eq!(
        after.publication.control.ledger.charged,
        before.publication.control.ledger.charged
    );
    assert_eq!(after.publication.control.ledger.reserved, 0);
    assert!(after.publication.stop.is_none());
    assert_eq!(archive(&next).revision(), after.publication.revision);
    assert_eq!(fs::read(&previous).unwrap(), previous_bytes);
    reader.no_generation(9);
    closed(&profiles, &reviewer);
    assert!(!governor.socket.exists());
    assert!(!selected_operator.settings.socket.exists());
}
