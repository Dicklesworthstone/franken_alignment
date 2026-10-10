//! Real coarse uncertainty, retained residuals and fresh native-model decisions.
//! Numerical fixtures establish this bounded path, not safety-detector quality.
use super::*;
use fa_reference::action::consequence::activation::probe::learned::{KvGroup, KvRow, RESIDUAL_HEADER_BYTES};
use fa_reference::action::consequence::activation::tensor::kv::{experiment::KvSide,
    decoder::MAX_DECODER_PRODUCTS};
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::sidecar::FileLearnedSidecarFinish,
    driver::native_learned::FileNativeSupervisedDriver,
};
use fa_reference::action::consequence::oversight::sidecar::{SidecarRefinementOutcome,
    receiver::native::SidecarEvaluationStatus};
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::{collections::BTreeMap, path::PathBuf};

const FIRST: u64 = 101;
const SECOND: u64 = 102;
// Two channels, each at most one 8-byte changed word after the original header.
const RESIDUAL_BYTES_BOUND: usize = RESIDUAL_HEADER_BYTES + 2 * 8;
// Admission, two probes, commitment and reveal; no first-round native inference.
const COARSE_POLLS: usize = 5;
const REFINED_INPUT_BOUND: usize = NATIVE_INPUT_BYTES + 8 + RESIDUAL_BYTES_BOUND;
// The whole path fits the original 1024-event source history even if BPE never
// merges a byte. Both native reviewers share each original policy observation.
const SOURCE_EVENTS_BOUND: usize = REFINED_INPUT_BOUND + NATIVE_POLL_OVERHEAD + COARSE_POLLS + 32;

fn key() -> KvGroup {
    KvGroup { row: KvRow { layer: 1, side: KvSide::Key, position: 3 }, head: 0 }
}
fn write(root: &Root, spelling: &[u8], count: usize) -> PathBuf {
    assert!((2..=3).contains(&count));
    let path = fixture::write(root, spelling);
    // The actor's exact y coordinates at positions 0..3 stay below 1.3.
    // The frozen rank-one fit's coarse key envelope at position3 straddles it.
    // The original actor monitor may use its retained residual; congress must
    // separately BUY that residual before its fresh native owners can judge.
    let ordinary = fixture::monitoring(100.0);
    let monitor = ordinary.replacen(
        "\"weights\":[1.0,0.0],\"bias\":0.0,\"threshold\":100",
        "\"weights\":[0.0,1.0],\"bias\":0.0,\"threshold\":1.3", 1);
    assert_ne!(monitor, ordinary);
    fs::write(root.0.join("learned-monitor.json"), monitor).unwrap();
    let ids = if count == 2 { "[101,102]" } else { "[101,102,103]" };
    let groups = if count == 2 {
        r#"[{"layer":1,"side":"key","position":3,"head":0}]"#
    } else {
        r#"[{"layer":1,"side":"key","position":3,"head":0},{"layer":1,"side":"value","position":3,"head":0}]"#
    };
    let contents = fixture::recipe(root)
        .replacen("fa.learned-publication/1", "fa.learned-publication/2", 1)
        .replacen("\"rounds\":1", &format!("\"rounds\":{count}"), 1)
        .replacen("\"sidecar\":{", &format!("\"sidecar\":{{\"priority\":{groups},"), 1)
        .replacen("\"native_review\":{", &format!("\"native_review\":{{\"round_ids\":{ids},"), 1)
        .replacen("\"evaluations\":2", &format!("\"evaluations\":{}", count * 2), 1)
        .replacen(&format!("\"scalar_products\":{}", MAX_DECODER_PRODUCTS * 2),
            &format!("\"scalar_products\":{}", MAX_DECODER_PRODUCTS * 2 * count as u64), 1)
        .replacen("\"sampling_entries\":8192", &format!("\"sampling_entries\":{}", 8192 * count), 1);
    fs::write(&path, contents).unwrap();
    path
}
fn prepared(root: &Root, spelling: &[u8], count: usize)
    -> (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire, Deadline)
{
    let mut config = configured(root);
    let path = write(root, spelling, count);
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let deadline = deadline(); let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000)).unwrap());
    assert_eq!(driver.supervisor().host().unwrap().learned_text_message(loaded.evidence).unwrap().bytes(), b"A");
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    (config, loaded, reviewer, driver, wire, deadline)
}
fn through_coarse(run: &mut FileNativeSupervisedDriver, config: &mut Config) {
    for _ in 0..COARSE_POLLS {
        assert_eq!(run.review().round().round, FIRST);
        run.step_from_policy_file(&mut config.source, || ElapsedTick(1000), None).result.unwrap();
    }
    assert_eq!(run.review().round().round, SECOND);
    assert_eq!(run.review().polls(), COARSE_POLLS);
    assert_eq!(run.review().history().len(), 1);
    assert!(matches!(run.review().history()[0], FileLearnedSidecarFinish::Refined { group, .. } if group == key()));
    let fresh = &run.review().records()[&SECOND];
    assert!(fresh.started); assert_eq!(fresh.polls, 0);
    assert!(fresh.members.values().flatten().all(|member|
        member.progress.completed_probes == 0 && !member.progress.native_started
            && member.progress.native.work.decoder.tokens == 0));
}
fn assert_refined(history: &FileNativeSidecarSequence, verdict: Verdict) {
    assert_eq!(history.status(), NativeReviewStatus::Finished);
    assert_eq!(history.history().len(), 2);
    let FileLearnedSidecarFinish::Refined { group, input_revision, archive } = &history.history()[0]
        else { panic!("actual uncertainty must purchase an original residual"); };
    assert_eq!(*group, key()); assert!(*input_revision > archive.input_revision);
    assert_eq!(history.history()[1].archive().input_revision, *input_revision);
    for view in archive.inputs.views().values() {
        assert_eq!(view.actual_input().submitted_bytes().len(), NATIVE_INPUT_BYTES);
    }
    let coarse = &history.records()[&FIRST];
    assert!(coarse.completed); assert_eq!(coarse.polls, COARSE_POLLS);
    for member in coarse.members.values().flatten() {
        assert_eq!(member.progress.basis, Some(SidecarDecisionBasis::NumericalUncertainty));
        assert_eq!(member.progress.status, SidecarEvaluationStatus::Judged(Verdict::Abstain));
        assert!(!member.progress.native_started); assert_eq!(member.progress.native.work.decoder.tokens, 0);
        assert!(member.commitment_queued && member.reveal_queued);
    }
    let rich = &history.records()[&SECOND];
    assert!(rich.completed); assert_eq!(rich.members.len(), 2);
    for member in rich.members.values().flatten() {
        assert_eq!(member.progress.basis, Some(SidecarDecisionBasis::NativeModel));
        assert_eq!(member.progress.status, SidecarEvaluationStatus::Judged(verdict));
        assert_eq!(member.progress.completed_probes, 2);
        assert_eq!(member.progress.native.reviewed_prompt_tokens, member.progress.native.requested_prompt_tokens);
        assert_eq!(member.progress.native.work.decoder.tokens,
            member.progress.native.requested_prompt_tokens as u64 + 2);
        assert_eq!(member.progress.native.work.sampled_draws, 2);
        assert!(member.commitment_queued && member.reveal_queued);
        assert!(member.failure.is_none() && !member.interrupted);
    }
    let prompt = rich.members.values().flatten().map(|member|
        member.progress.native.requested_prompt_tokens).max().unwrap();
    for view in history.input().views().values() {
        let length = view.actual_input().submitted_bytes().len();
        assert!(length > NATIVE_INPUT_BYTES && length <= REFINED_INPUT_BOUND);
        assert!(prompt <= length);
    }
    assert_eq!(rich.polls, prompt + NATIVE_POLL_OVERHEAD);
    assert_eq!(history.polls(), coarse.polls + rich.polls);
}

#[test]
fn progressive_command_buys_exact_residual_then_fresh_native_and_human_keys_publish_once() {
    let root = Root::new();
    let (mut config, loaded, reviewer, driver, mut wire, deadline) = prepared(&root, b"allow", 2);
    let numerical = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    let residual = driver.supervisor().host().unwrap().learned_text_message(loaded.evidence).unwrap()
        .evidence().audit().source().residual_bytes(key()).unwrap().len();
    assert!(residual > RESIDUAL_HEADER_BYTES && residual <= RESIDUAL_BYTES_BOUND);
    let mut control = Control::new(&config, 1, None).unwrap();
    let reads = config.source.status().read_attempts;
    let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
        &deadline, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    let mut driver = reviewed.driver; let history = reviewed.history.unwrap();
    assert_refined(&history, Verdict::Allow);
    assert_eq!(history.reservation(), loaded.native_limits.native);
    assert_eq!(config.source.status().read_attempts - reads, history.polls() as u64 + 1);
    assert_eq!(driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        let saved = host.retained_learned_sidecar(1).unwrap();
        assert_eq!(saved.packet.selected_groups(), &[key()]);
        assert_eq!(saved.packet.work().rounds, 2);
        assert_eq!(saved.packet.work().residual_bytes, residual);
        assert_eq!(saved.packet.work().committee_bytes,
            history.history().iter().map(|result| result.archive().inputs.logical_bytes()).sum::<usize>());
        let coarse = &history.history()[0].archive().inputs;
        let revision = host.revision(); let bytes = fs::read(config.store.join("delivery.bin")).unwrap();
        assert!(host.authorize(revision, 1, coarse, Snapshot { semantic_epoch: 1, complete: true,
            values: BTreeMap::from([(7, b"ok".to_vec())]) }).is_err());
        assert_eq!(host.revision(), revision); assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), bytes);
        assert_eq!(host.inspect().executions, 0);
    }
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 1 }));
    let offer = driver.request_learned_human_approval_from_policy_file(&mut config.source,
        701, ElapsedTick(50_000), || ElapsedTick(1000)).result.unwrap();
    let human = {
        let mut host = driver.supervisor_mut().host_mut().unwrap(); let revision = host.revision();
        reviewer.approve(&mut host, revision, &offer).unwrap()
    };
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), Some(&human)).result.unwrap(), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(driver.step_computed_from_policy_file(&mut config.source,
        || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::Reconciled { .. }));
    assert!(executed(&mut wire, 1)); assert!(executed(&mut wire, 1));
    {
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"A");
        let capture = host.file_source_status().unwrap().capture;
        assert!(SOURCE_EVENTS_BOUND < config.source_policy.limits.events);
        assert!(config.source.status().read_attempts <= SOURCE_EVENTS_BOUND as u64);
        assert!(capture.retained_events <= SOURCE_EVENTS_BOUND && capture.fault.is_none());
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.inspect().control.ledger.charged,
            host.request_action(1).unwrap().spec().payload.len() as u64);
    }
    for (index, id) in [FIRST, SECOND].into_iter().enumerate() {
        let saved = FileOversight::read_learned_sidecar_outcome(&config.store, &config.profile,
            &loaded.generation, id).unwrap();
        assert_eq!(saved.result.archive(), history.history()[index].archive());
    }
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    fs::remove_file(root.0.join("native-roster.json")).unwrap();
    let outer = fs::read_to_string(root.0.join("recipe.json")).unwrap();
    fs::write(root.0.join("recipe.json"), outer.replace("\"ttl_ms\":60000", "\"ttl_ms\":1")).unwrap();
    let receipt_only = recipe::load(&root.0.join("recipe.json"), &config, false).unwrap();
    let result = recovery::resume(config, receipt_only, || panic!("receipt cannot resume refinement")).unwrap();
    assert!(result.failure.is_none());
    assert!(matches!(result.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
}

#[test]
fn progressive_command_one_byte_short_holds_without_refunding_or_running_successor() {
    let root = Root::new();
    let (mut config, mut loaded, reviewer, driver, mut wire, deadline) = prepared(&root, b"allow", 2);
    let required = driver.supervisor().host().unwrap().learned_text_message(loaded.evidence).unwrap()
        .evidence().audit().source().residual_bytes(key()).unwrap().len();
    loaded.disclosure.residual_bytes = required - 1;
    let mut control = Control::new(&config, 1, None).unwrap();
    let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
        &deadline, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    let mut driver = reviewed.driver; let history = reviewed.history.unwrap();
    assert_eq!(history.status(), NativeReviewStatus::Finished);
    assert_eq!(history.history().len(), 1);
    assert!(matches!(history.history()[0], FileLearnedSidecarFinish::Applied {
        outcome: Some(SidecarRefinementOutcome::BudgetExhausted { .. }), .. }));
    assert_eq!(history.records()[&FIRST].polls, COARSE_POLLS);
    assert!(!history.records()[&SECOND].started);
    assert_eq!(history.reservation(), loaded.native_limits.native);
    assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
        701, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
    assert!(!executed(&mut wire, 1));
    let host = driver.supervisor().host().unwrap();
    assert_eq!(host.retained_learned_sidecar(1).unwrap().packet.work().residual_bytes, 0);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 0);
    drop(host); assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
}

#[test]
fn progressive_native_denial_after_refinement_never_runs_the_unused_third_roster() {
    let root = Root::new();
    let (mut config, loaded, reviewer, driver, mut wire, deadline) = prepared(&root, b"deny", 3);
    let mut control = Control::new(&config, 1, None).unwrap();
    let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
        &deadline, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.is_none(), "{:?}", reviewed.failure);
    let mut driver = reviewed.driver; let history = reviewed.history.unwrap();
    assert_refined(&history, Verdict::Deny);
    assert_eq!(history.reservation(), loaded.native_limits.native);
    assert!(!history.records()[&103].started);
    assert!(history.records()[&103].members.values().all(Option::is_none));
    assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
        701, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
    assert!(!executed(&mut wire, 1));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
}

#[test]
fn progressive_poll_budget_is_not_restarted_when_the_residual_is_purchased() {
    let root = Root::new();
    let (mut config, mut loaded, reviewer, driver, mut wire, deadline) = prepared(&root, b"allow", 2);
    loaded.native_limits.polls = COARSE_POLLS + 4; // intake, two probes, ONE actual native token.
    let mut control = Control::new(&config, 1, None).unwrap();
    let reviewed = review::run(driver, &mut config, &loaded, &reviewer, &mut control,
        &deadline, &mut || ElapsedTick(1000));
    assert!(reviewed.failure.as_ref().is_some_and(|error| error.contains("finite poll budget")));
    let driver = reviewed.driver; let history = reviewed.history.unwrap();
    assert_eq!(history.status(), NativeReviewStatus::Cancelled);
    assert_eq!(history.history().len(), 1); assert_eq!(history.polls(), COARSE_POLLS + 4);
    assert_eq!(history.records()[&SECOND].polls, 4);
    assert!(history.records()[&SECOND].members.values().flatten().all(|member|
        member.progress.native.work.decoder.tokens == 1 && !member.commitment_queued));
    assert_eq!(history.reservation(), loaded.native_limits.native);
    assert!(!executed(&mut wire, 1));
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    assert!(driver.supervisor().host().unwrap().retained_learned_sidecar(1).unwrap().packet.work().residual_bytes > 0);
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(history);
}

#[test]
fn progressive_policy_change_between_rounds_stops_before_successor_numerical_work() {
    let root = Root::new();
    let (mut config, loaded, _reviewer, driver, mut wire, deadline) = prepared(&root, b"allow", 2);
    let mut run = review::start(driver, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    through_coarse(&mut run, &mut config);
    evidence(&root, &config, 2, false, false);
    let polls = run.review().polls(); let reservation = run.review().reservation();
    assert!(run.step_from_policy_file(&mut config.source, || ElapsedTick(1001), None).result.is_err());
    assert_eq!(run.review().status(), NativeReviewStatus::Cancelled);
    assert_eq!(run.review().polls(), polls); assert_eq!(run.review().reservation(), reservation);
    assert!(run.review().records()[&SECOND].members.values().flatten().all(|member|
        member.progress.completed_probes == 0 && member.progress.native.work.decoder.tokens == 0));
    let handoff = run.into_handoff().unwrap();
    assert!(!executed(&mut wire, 1));
    assert_eq!(handoff.driver.supervisor().host().unwrap().inspect().executions, 0);
    assert_eq!(handoff.driver.supervisor().host().unwrap().file_source_status().unwrap().producer.unwrap().generation, 2);
    assert_eq!(cleanup(handoff.driver, 1000, 1), 0); drop(handoff.review);
}

#[test]
fn progressive_recipe_rejects_incomplete_frozen_schedules_and_aggregate_work_before_startup() {
    for change in 0..13 {
        let root = Root::new(); let config = configured(&root); let path = write(&root, b"allow", 2);
        let original = fs::read_to_string(&path).unwrap();
        let changed = match change {
            0 => original.replace("[101,102]", "[101,101]"),
            1 => original.replace("[101,102]", "[0,102]"),
            2 => original.replace("[101,102]", "[101]"),
            3 => original.replace("[101,102]", "[]"),
            4 => original.replace("\"head\":0", "\"head\":1"),
            5 => original.replace("\"position\":3", "\"position\":16"),
            6 => original.replace("\"evaluations\":4", "\"evaluations\":3"),
            7 => original.replace(&format!("\"scalar_products\":{}", MAX_DECODER_PRODUCTS * 4),
                &format!("\"scalar_products\":{}", MAX_DECODER_PRODUCTS * 4 - 1)),
            8 => original.replace("\"sampling_entries\":16384", "\"sampling_entries\":16383"),
            9 => original.replace("fa.learned-publication/2", "fa.learned-publication/1"),
            10 => original.replace("\"priority\":[{\"layer\":1,\"side\":\"key\",\"position\":3,\"head\":0}]",
                "\"priority\":[{\"layer\":1,\"side\":\"key\",\"position\":3,\"head\":0},{\"layer\":1,\"side\":\"key\",\"position\":3,\"head\":0}]"),
            11 => original.replace("\"rounds\":2", "\"rounds\":33"),
            12 => original.replace("\"priority\":[{\"layer\":1,\"side\":\"key\",\"position\":3,\"head\":0}]", "\"priority\":[]"),
            _ => unreachable!(),
        };
        assert_ne!(original, changed); fs::write(&path, changed).unwrap();
        assert!(recipe::load(&path, &config, true).is_err(), "change {change}");
        assert!(!config.store.exists(), "no startup image for change {change}");
    }
    // The configured runtime and recipe TTL share one original lifetime. The
    // entire schedule must fit before model admission or numerical generation.
    let root = Root::new(); let mut config = configured(&root);
    let path = write(&root, b"allow", 2);
    config.timing.runtime_ms = config.timing.reveal_ms * 2;
    assert_eq!(recipe::load(&path, &config, true).err().as_deref(),
        Some("complete learned review schedule must fit the original request lifetime"));
    assert!(!config.store.exists());
    config.timing.runtime_ms += 1;
    assert!(recipe::load(&path, &config, true).is_ok());
    assert!(!config.store.exists());
}

#[test]
fn progressive_unretained_priority_and_expired_future_window_cannot_begin_review() {
    for change in 0..2 {
        let root = Root::new();
        let (mut config, mut loaded, _reviewer, driver, _wire, mut deadline) = prepared(&root, b"allow", 2);
        if change == 0 { loaded.priority[0].row.position = 2; }
        else { deadline.logical = ElapsedTick(1000 + config.timing.reveal_ms * 2); }
        let before = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
        let (driver, _error) = review::start(driver, &mut config, &loaded,
            &deadline, &mut || ElapsedTick(1000)).unwrap_err();
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
        assert!(host.retained_learned_sidecar(1).is_err());
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 0);
        drop(host); assert_eq!(cleanup(driver, 1000, 1), 0);
    }
}

#[test]
fn progressive_authenticated_stop_retires_partial_successor_and_keeps_paid_refinement() {
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
        PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
        control::StopClientProgress,
    };
    use std::os::unix::net::UnixStream;
    let root = Root::new();
    let (mut config, loaded, reviewer, driver, mut wire, deadline) = prepared(&root, b"allow", 3);
    let mut run = review::start(driver, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    through_coarse(&mut run, &mut config);
    for _ in 0..4 {
        run.step_from_policy_file(&mut config.source, || ElapsedTick(1000), None).result.unwrap();
    }
    let work: Vec<_> = run.review().records()[&SECOND].members.values().flatten()
        .map(|member| member.progress.native.work).collect();
    assert_eq!(work.len(), 2); assert!(work.iter().all(|work| work.decoder.tokens == 1));
    let reservation = run.review().reservation();
    let peers = fixture::peers(&root, &config);
    let mut control = Control::new(&config, 1, Some(&peers)).unwrap();
    let path = crate::workflow::control::socket_path(&peers, 1);
    // Start only after a real listener exists. Permission failures remain test
    // failures and leave no thread waiting for a listener that cannot be made.
    let client = std::thread::spawn(move || {
        let (identity, _other) = UnixStream::pair().unwrap();
        let identity = PeerCredentials::observe(&identity).unwrap();
        let checked = VerifiedReviewerSocket::verify(UnixStream::connect(path).unwrap(),
            PeerPolicy::new(identity.uid(), identity.gid(), Some(identity.pid())).unwrap()).unwrap();
        let mut client = checked.into_stop_client(peers.expected, 1).unwrap();
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10));
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    });
    assert!(review::wait_for_phase(&mut run, &mut control, 1, &deadline,
        &mut || ElapsedTick(1000)).unwrap());
    assert_eq!(run.review().status(), NativeReviewStatus::Cancelled);
    assert_eq!(run.review().reservation(), reservation);
    assert_eq!(run.review().records()[&SECOND].members.values().flatten()
        .map(|member| member.progress.native.work).collect::<Vec<_>>(), work);
    assert!(!run.review().records()[&103].started);
    let handoff = run.into_handoff().unwrap(); let mut driver = handoff.driver;
    assert!(control.checkpoint(&mut driver, &reviewer, &deadline, &mut || ElapsedTick(1000)).unwrap());
    let receipt = client.join().unwrap(); assert!(receipt.acknowledged() && receipt.drained());
    let host = driver.supervisor().host().unwrap();
    assert!(host.inspect().stop.is_some()); assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert!(host.retained_learned_sidecar(1).unwrap().packet.work().residual_bytes > 0);
    drop(host); assert!(!executed(&mut wire, 1));
    assert_eq!(cleanup(driver, 1000, 1), 0); drop(handoff.review);
}
