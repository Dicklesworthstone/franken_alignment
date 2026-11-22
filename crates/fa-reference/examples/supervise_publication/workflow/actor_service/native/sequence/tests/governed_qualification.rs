//! Explicit requalification belongs to the same guarded native owner and live
//! governor invocation. Independent labels are synthetic contract fixtures.
use super::*;
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;

#[allow(dead_code)]
mod evidence {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/supervise_publication/workflow/actor_service/native/qualification/tests/fixtures.rs"
    ));
}

const QUALIFIED_ROOT: &str = "FA_GOVERNED_NATIVE_QUALIFIED_ROOT";
const QUALIFIED_TEST: &str = "workflow::actor_service::native::sequence::tests::governor_tests::recovery_tests::governed_qualification_tests::independent_governor_reviews_after_requalification";
const FRESH_CAMPAIGN: u64 = 705;

fn activation(config: &Config, before: &FileDeliverySnapshot) -> CredibilityActivation {
    let mut fresh = evidence::activation(
        config,
        before.control.sequence,
        before.control.ledger.epoch + 1, // the separately required recovery Fence
        801,
        2,
        2,
    );
    // The bootstrap profile deliberately still says one. The independent
    // expected recovery policy, and this explicit capsule, both require two.
    fresh.policy_generation = 2;
    fresh
}

fn next_campaign(scope: &str, cut: &Json, campaign: u64) -> String {
    format!(
        r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"policy-replay","operation":"{campaign}","expected":{{"journal_revision":{},"control_sequence":{},"authority_epoch":{},"policy_generation":"2"}},"candidate":{{"schema_version":"fa.policy-replay-request/1","expected_policy_generation":2,"limits":{{"cases":16,"input_bytes":1048576,"output_bytes":65536}},"candidate":{{"generation":3,"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":256}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}}}}}}"#,
        live_governor::protocol::json(field(cut, "journal_revision")),
        live_governor::protocol::json(field(cut, "control_sequence")),
        live_governor::protocol::json(field(cut, "authority_epoch")),
    )
}

/// This process gets no effect key. Its original recovered governor role must
/// remain valid after the independent qualification fences authority again.
#[test]
fn independent_governor_reviews_after_requalification() {
    let Some(root) = std::env::var_os(QUALIFIED_ROOT) else {
        return;
    };
    let root = PathBuf::from(root);
    wait(&root.join("qualified.ready"));
    let settings = live_governor::Settings::read(&root.join("governor.json")).unwrap();
    let scope = live_governor::protocol::scope_json(settings.scope);
    let inspected = inspect(&root, &scope, 1);
    let before = known(&inspected, "inspect");
    assert_eq!(number(before, "authority_epoch"), 3);
    assert_eq!(number(before, "policy_generation"), 2);
    assert_eq!(
        number(field(before, "numerical_replay"), "remaining_events"),
        8192
    );
    let reviewed = exchange(
        &root,
        "qualified-review.json",
        next_campaign(&scope, before, FRESH_CAMPAIGN),
    );
    let review = known(&reviewed, "review");
    let report = field(review, "reviewed_report").as_str().unwrap();
    let decoded = json(report.as_bytes());
    let replay = field(field(&decoded, "knowledge"), "value");
    assert_eq!(field(field(replay, "cut"), "executions").as_u64(), Some(2));
    assert_eq!(field(replay, "requires_shadow").as_bool(), Some(false));
    let inspected = inspect(&root, &scope, 2);
    let pending = known(&inspected, "inspect");
    assert_eq!(
        number(pending, "journal_revision"),
        number(before, "journal_revision") + 1
    );
    let charged = number(before, "journal_revision") + 1;
    assert_eq!(
        number(field(pending, "numerical_replay"), "remaining_events"),
        8192 - charged
    );
    let canonical = fs::read(root.join("store/delivery.bin")).unwrap();
    let exhausted = exchange(
        &root,
        "qualified-exhausted.json",
        next_campaign(&scope, pending, FRESH_CAMPAIGN + 1),
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
        "qualified-approve.json",
        format!(
            r#"{{"schema":"fa.policy-governor-request/1","scope":{scope},"verb":"promote","phase":"approve","campaign":"{FRESH_CAMPAIGN}","expected_journal_revision":{},"reviewed_report":{},"accept_newly_reviewable":false}}"#,
            live_governor::protocol::json(field(review, "journal_revision")),
            live_governor::protocol::quote(report),
        ),
    );
    let approval = known(&approved, "approve");
    assert_eq!(field(approval, "effect_authority").as_bool(), Some(false));
    assert_eq!(field(approval, "policy_changed").as_bool(), Some(false));
    let inspected = inspect(&root, &scope, 3);
    let after = known(&inspected, "inspect");
    let remaining = 8192 - charged - number(pending, "journal_revision") - 1;
    assert_eq!(number(after, "authority_epoch"), 3);
    assert_eq!(number(after, "policy_generation"), 2);
    assert_eq!(
        number(field(after, "numerical_replay"), "remaining_events"),
        remaining
    );
    write_complete(
        &root.join("qualified-human-0.ready"),
        b"separate governor review has no effect key",
    );
    if std::env::var_os("FA_GOVERNED_NATIVE_QUALIFIED_REJECT").is_none() {
        wait(&root.join("qualified.handoff"));
        let inspected = inspect(&root, &scope, 4);
        let handoff = known(&inspected, "inspect");
        assert_eq!(number(handoff, "authority_epoch"), 3);
        assert_eq!(
            number(field(handoff, "numerical_replay"), "remaining_events"),
            remaining,
            "the next resident entry must retain both replay charges"
        );
        write_complete(
            &root.join("qualified-human-1.ready"),
            b"same live governor and spent allowance",
        );
    }
}

fn child(root: &Root, decision: ReviewDecision) -> GovernorChild {
    let mut command = ProcessCommand::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", QUALIFIED_TEST, "--nocapture"])
        .env(QUALIFIED_ROOT, &root.0);
    if decision == ReviewDecision::Reject {
        command.env("FA_GOVERNED_NATIVE_QUALIFIED_REJECT", "1");
    }
    GovernorChild(command.spawn().unwrap())
}

struct OnceOutput {
    inner: Output,
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
            fs::remove_file(&self.capsule)?;
            self.removed = true;
        }
        Ok(())
    }
}

fn qualified_suffix(decision: ReviewDecision) {
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
    let entries = suffix(&root, &config, &reader, false);
    let mut child = child(&root, decision);
    let (governor, selected_operator) = selections(&root, &entries[0].actor, child.0.id());
    let requirements = expected(&config, &entries[0], &governor, &before.publication);
    let capsule = root.0.join("governed-qualification.bin");
    fs::write(
        &capsule,
        activation(&config, &before.publication)
            .encode_reference()
            .unwrap(),
    )
    .unwrap();
    let next = root.0.join("qualified-successor.anchor");
    let retained = history(
        &config,
        &entries,
        &reviewer,
        Some(&selected_operator),
        Some(AFTER),
        Some(&previous),
        &next,
    );
    let mut output = OnceOutput {
        inner: Output::new(&entries),
        capsule: capsule.clone(),
        removed: false,
    };
    let profiles = output.inner.profiles.clone();
    let mut humans = RecoveredHumans::new(&reviewer, &config, &entries, decision);
    let result = run_with_governed_recovery(
        config,
        entries,
        &reviewer,
        None,
        (Some(AFTER), Some(&selected_operator)),
        Some(&capsule),
        Some(&retained),
        Some(&governor),
        Some(&requirements),
        || {
            for (index, request) in [93, 94].into_iter().enumerate() {
                if reviewer.socket(request).exists() {
                    let signal = root.0.join(if index == 0 {
                        "qualified.ready"
                    } else {
                        "qualified.handoff"
                    });
                    if !signal.exists() {
                        write_complete(&signal, b"fresh human key remains outstanding");
                    }
                    if root
                        .0
                        .join(format!("qualified-human-{index}.ready"))
                        .exists()
                        && humans.clients[index].is_none()
                    {
                        humans.clients[index] = Some(human(
                            reviewer.clone(),
                            request,
                            reader.store.clone(),
                            reader.profile.clone(),
                            2 + index as u64,
                            decision,
                        ));
                    }
                }
            }
            ElapsedTick(1001)
        },
        &mut output,
    );
    child.finish();
    let approved = decision == ReviewDecision::Approve;
    assert_eq!(result.is_ok(), approved, "{result:?}");
    assert!(
        output.removed && !capsule.exists(),
        "a handoff may not reread the consumed capsule"
    );
    let documents = output.inner.documents.clone();
    let replies = output.inner.join();
    let packets = humans.join();
    assert_eq!(replies.len(), if approved { 2 } else { 1 });
    assert_eq!(packets.len(), replies.len());
    assert_eq!(replies.iter().all(executed), approved);
    let mut sessions = original
        .iter()
        .map(|packet| packet.binding().session)
        .collect::<BTreeSet<_>>();
    for (packet, document) in packets.iter().zip(&documents) {
        assert!(sessions.insert(packet.binding().session));
        assert_eq!(packet.policy_generation(), 2);
        assert_eq!(packet.action().spec().policy_epoch, 3);
        let Command::Submit { proposal, .. } = decode_command(document).unwrap() else {
            panic!("original source intent");
        };
        assert_eq!(proposal.expected_policy_epoch, 3);
        assert_eq!(
            proposal.payload.len(),
            FileGeneratedTextActorPort::INTENT_BYTES
        );
    }
    let after = reader.stream();
    assert_eq!(after.publication.control.ledger.epoch, 3);
    assert_eq!(after.publication.executions, if approved { 4 } else { 2 });
    assert_eq!(
        after.publication.payload,
        if approved {
            b"AAA".to_vec()
        } else {
            b"AA".to_vec()
        }
    );
    assert_eq!(after.confirmed, after.published);
    assert_eq!(after.confirmed.finished(), approved);
    assert_eq!(after.publication.control.ledger.reserved, 0);
    assert_eq!(
        after.publication.control.ledger.charged,
        before.publication.control.ledger.charged
            + if approved {
                packets
                    .iter()
                    .map(|packet| packet.action().spec().units)
                    .sum::<u64>()
            } else {
                0
            }
    );
    assert_eq!(archive(&next).revision(), after.publication.revision);
    assert_eq!(fs::read(&previous).unwrap(), previous_bytes);
    reader.no_generation(10);
    assert!(!governor.socket.exists() && !selected_operator.settings.socket.exists());
    closed(&profiles, &reviewer);
}

#[test]
fn governed_requalification_retains_live_governor_charges_across_fresh_human_effects() {
    qualified_suffix(ReviewDecision::Approve);
}

#[test]
fn qualified_governor_approval_cannot_override_fresh_human_rejection() {
    qualified_suffix(ReviewDecision::Reject);
}

#[test]
fn stale_or_absent_governed_capsule_retains_fence_and_stop_without_new_work() {
    for variation in 0..3 {
        let root = Root::new();
        let Prefix {
            config,
            reviewer,
            reader,
            archive: previous,
            ..
        } = prefix(&root);
        let before = reader.progress(8);
        let entries = suffix(&root, &config, &reader, false);
        let profiles = entries
            .iter()
            .map(|entry| entry.actor.clone())
            .collect::<Vec<_>>();
        let (governor, selected_operator) =
            selections(&root, &entries[0].actor, std::process::id() + 10_000);
        let requirements = expected(&config, &entries[0], &governor, &before.publication);
        let capsule = root.0.join("refused-capsule.bin");
        let mut fresh = activation(&config, &before.publication);
        match variation {
            0 => fresh.expected_epoch -= 1,
            1 => fresh.policy_generation = 1,
            _ => {}
        }
        if variation != 2 {
            fs::write(&capsule, fresh.encode_reference().unwrap()).unwrap();
        }
        let next = root.0.join("refused-qualified.anchor");
        let retained = history(
            &config,
            &entries,
            &reviewer,
            Some(&selected_operator),
            Some(AFTER),
            Some(&previous),
            &next,
        );
        // The capsule failure must win before acquiring this unavailable source.
        fs::remove_file(config.source.path()).unwrap();
        let mut output = Vec::new();
        let result = run_with_governed_recovery(
            config,
            entries,
            &reviewer,
            None,
            (Some(AFTER), Some(&selected_operator)),
            Some(&capsule),
            Some(&retained),
            Some(&governor),
            Some(&requirements),
            || ElapsedTick(1001),
            &mut output,
        );
        assert!(result.is_err(), "{result:?}");
        assert!(output.is_empty());
        let after = reader.progress(8);
        assert_eq!(after.numerical.numerical, before.numerical.numerical);
        assert_eq!(
            after.text.generation_revision(),
            before.text.generation_revision()
        );
        assert_eq!(after.publication.payload, before.publication.payload);
        assert_eq!(after.publication.executions, before.publication.executions);
        let stopped = after
            .publication
            .stop
            .expect("failed activation drains this invocation");
        assert_eq!(
            stopped.request().expected_authority_epoch,
            before.publication.control.ledger.epoch + 1,
            "only the recovery Fence may precede automatic Stop; a refused capsule cannot activate"
        );
        assert_eq!(
            after.publication.control.ledger.epoch,
            before.publication.control.ledger.epoch + 2
        );
        assert_eq!(
            after.publication.control.ledger.charged,
            before.publication.control.ledger.charged
        );
        assert_eq!(after.publication.control.ledger.reserved, 0);
        assert_eq!(archive(&next).revision(), after.publication.revision);
        reader.no_generation(9);
        assert!(!governor.socket.exists() && !selected_operator.settings.socket.exists());
        closed(&profiles, &reviewer);
    }
}

#[test]
fn governed_requalification_services_stop_before_source_work_and_retains_terminal_anchor() {
    let root = Root::new();
    let Prefix {
        config,
        reviewer,
        reader,
        archive: previous,
        ..
    } = prefix(&root);
    let before = reader.progress(8);
    let entries = suffix(&root, &config, &reader, false);
    let profiles = entries
        .iter()
        .map(|entry| entry.actor.clone())
        .collect::<Vec<_>>();
    let (governor, selected_operator) =
        selections(&root, &entries[0].actor, std::process::id() + 10_000);
    let requirements = expected(&config, &entries[0], &governor, &before.publication);
    let capsule = root.0.join("stopped-qualification.bin");
    let fresh = activation(&config, &before.publication);
    let qualified_epoch = fresh.expected_epoch + 1;
    fs::write(&capsule, fresh.encode_reference().unwrap()).unwrap();
    let next = root.0.join("stopped-qualified.anchor");
    let retained = history(
        &config,
        &entries,
        &reviewer,
        Some(&selected_operator),
        Some(AFTER),
        Some(&previous),
        &next,
    );
    fs::remove_file(config.source.path()).unwrap();
    let mut client = None;
    let mut observed = false;
    let mut output = Vec::new();
    let result = run_with_governed_recovery(
        config,
        entries,
        &reviewer,
        None,
        (Some(AFTER), Some(&selected_operator)),
        Some(&capsule),
        Some(&retained),
        Some(&governor),
        Some(&requirements),
        || {
            let state = reader.publication();
            if !observed && state.stop.is_none() && state.control.ledger.epoch == qualified_epoch {
                observed = true;
                client = Some(stopping_at(&reviewer, 93));
            }
            ElapsedTick(1001)
        },
        &mut output,
    );
    assert!(
        observed,
        "original activation must precede this Stop checkpoint"
    );
    let error = result.unwrap_err();
    assert!(error.contains("native sequence ended after 0"), "{error}");
    let receipt = client.unwrap().join().unwrap();
    assert!(receipt.acknowledged() && receipt.drained());
    assert!(output.is_empty());
    let after = reader.progress(8);
    assert_eq!(after.numerical.numerical, before.numerical.numerical);
    assert_eq!(after.publication.executions, before.publication.executions);
    assert_eq!(after.publication.payload, before.publication.payload);
    assert_eq!(
        after.publication.control.ledger.charged,
        before.publication.control.ledger.charged
    );
    assert_eq!(after.publication.control.ledger.reserved, 0);
    assert_eq!(
        after
            .publication
            .stop
            .unwrap()
            .request()
            .expected_authority_epoch,
        qualified_epoch
    );
    assert_eq!(archive(&next).revision(), after.publication.revision);
    reader.no_generation(9);
    assert!(!governor.socket.exists() && !selected_operator.settings.socket.exists());
    closed(&profiles, &reviewer);
}
