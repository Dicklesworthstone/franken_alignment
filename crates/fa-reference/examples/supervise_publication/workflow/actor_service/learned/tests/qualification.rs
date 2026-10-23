//! Actual learned messages and native/human reviews across held-out recovery.
//! Campaign labels and model weights are synthetic enforcement fixtures. They
//! establish neither statistical calibration nor independent label authenticity.
use super::*;
use super::continuation as native;
use super::super::{continuation, prediction, qualification};
use fa_reference::action::consequence::congress::{CredibilityBinding, CredibilityRequirements};
use fa_reference::action::consequence::congress::credibility::{Campaign, CaseSpec,
    CredibilityLedger, EvaluationLabel, EvaluationScope, HelperGeneration,
    LabelSource, LabelVerdict, Observation};
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanPermit, decoder::learned::FileLearnedInspection,
    consistency::{FileConsistencySnapshot, learned::FileLearnedConsistencySnapshot},
};
use std::{collections::{BTreeMap, BTreeSet}, path::{Path, PathBuf}};

struct Seed {
    config: Config,
    first_recipe: PathBuf,
    next_recipe: PathBuf,
    old_capsule: PathBuf,
    first_human: FileHumanPermit,
    revision: u64,
}

fn activation(config: &Config, sequence: u64, epoch: u64, operation: u64,
    generation: u64) -> CredibilityActivation {
    // Same explicitly synthetic sequence mapping as the original actor-service
    // qualification fixture. Promotion checks the resulting sequence, so the
    // first real review at sequence1 can admit a campaign sealed at frontier2.
    let frontier = sequence.max(2);
    let helpers = config.profile.delivery.congress.members.iter().map(|(id, member)|
        (id.clone(), HelperGeneration {
            generation: config.profile.committee.members()[id].profile_at(0).model_epoch,
            cohort: member.cohort.clone(),
        })).collect::<BTreeMap<_, _>>();
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: operation, model_generation: 1,
            evaluator_generation: 1, held_out_manifest: [42; 32] },
        label_owner: "independent-evaluator".into(), helpers: helpers.clone(),
        strata: BTreeSet::from(["publication".into()]),
        cases: (1..=2).map(|id| CaseSpec { id, stratum: "publication".into(),
            evidence_root: [id as u8; 32], dispatch_sequence: frontier }).collect(),
    }).unwrap();
    for id in 1..=2 {
        let observation = if id == 1 { Observation::Clear }
            else { Observation::Hold { first_sequence: frontier - 1 } };
        ledger.record_observations(id, helpers.keys().map(|name|
            (name.clone(), observation)).collect()).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "independent-evaluator".into(),
            evaluator_generation: 1, source: LabelSource::IndependentEvaluation,
            evidence_root: [id as u8; 32], recorded_sequence: frontier,
            verdict: if id == 1 { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    let snapshot = ledger.seal(frontier).unwrap();
    CredibilityActivation { operation, expected_control_sequence: sequence,
        expected_epoch: epoch, scope: config.profile.delivery.scope,
        policy_generation: config.profile.delivery.policy.generation(),
        actor_profile: config.profile.delivery.actor.profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(),
            label_owner: snapshot.label_owner().into(), helpers: snapshot.helpers().clone(),
            strata: snapshot.strata().clone(), reducer_generation: generation },
        stratum: "publication".into(), requirements: CredibilityRequirements {
            minimum_safe_cases: 1, minimum_violation_cases: 1,
            minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 5, lead_bonus_weight: 0,
            lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}

fn save(root: &Root, request: &CredibilityActivation) -> PathBuf {
    let path = root.0.join(format!("qualification-{}.bin", request.operation));
    fs::write(&path, request.encode_reference().unwrap()).unwrap(); path
}

fn seed(root: &Root, mode: Option<bool>) -> Seed {
    let mut config = configured(root);
    let first_recipe = native::write(root);
    if let Some(owned) = mode { super::forecast::install(root, &first_recipe, owned, 60_000); }
    evidence(root, &config, 1, true, false);
    let first = recipe::load(&first_recipe, &config, true).unwrap();
    let (host, reviewer, observer) = recovery::create_selected(&config, &first).unwrap();
    let mut prediction = observer.map(|observer| prediction::Session::new(1, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port); let deadline = deadline();
    let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate_selected(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000), prediction.as_mut()).unwrap());
    submit(&mut driver, &mut wire, &mut config, &first, &deadline,
        &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) = native::review_current(&mut config, &first,
        &reviewer, driver, &deadline);
    let first_human = native::approve(&mut driver, &mut config, &reviewer, 1);
    native::publish(&mut driver, &mut config, &first_human);
    assert!(executed(&mut wire, 1));
    let old_capsule = {
        let mut host = driver.supervisor_mut().host_mut().unwrap();
        let state = host.inspect();
        let old = activation(&config, state.control.sequence, state.control.ledger.epoch, 20, 2);
        let revision = host.revision();
        host.activate_credibility(revision, old.clone(), &config.profile.committee).unwrap();
        host.check_credibility().unwrap();
        assert_eq!(host.credibility_change(20).unwrap().reducer_generation, 2);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(host.inspect().executions, 1);
        save(root, &old)
    };
    let revision = driver.supervisor().host().unwrap().revision();
    let (next_recipe, next) = native::next(root, &config, 2);
    assert_eq!(first.generation, next.generation);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(history); drop(prediction); drop(wire);
    Seed { config, first_recipe, next_recipe, old_capsule, first_human, revision }
}

struct Live {
    loaded: Loaded,
    reviewer: FileHumanReviewer,
    prediction: Option<prediction::Session>,
    driver: FileSupervisedDriver,
    wire: Wire,
}

fn open(seed: &Seed) -> Live {
    let loaded = recipe::load(&seed.next_recipe, &seed.config, true).unwrap();
    let (host, reviewer, observer) = recovery::open_selected(&seed.config, &loaded).unwrap();
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(host.check_credibility().is_err(), "recovery must invalidate actual prior qualification");
    let prediction = observer.map(|observer| prediction::Session::new(2, observer));
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    Live { loaded, reviewer, prediction,
        driver: FileSupervisedDriver::new(supervisor), wire: ActorWire::new(port) }
}

fn fresh(root: &Root, seed: &Seed, live: &Live, operation: u64, generation: u64)
    -> (PathBuf, CredibilityActivation) {
    let state = live.driver.supervisor().host().unwrap().inspect();
    let request = activation(&seed.config, state.control.sequence,
        state.control.ledger.epoch, operation, generation);
    (save(root, &request), request)
}

fn consistency(host: &FileOversight) -> Option<FileConsistencySnapshot> {
    host.action_consistency_required().then(|| {
        let mut snapshot = host.action_consistency_snapshot().unwrap();
        snapshot.journal_revision = 0; snapshot
    })
}
fn owned_cost(host: &FileOversight) -> Option<FileLearnedConsistencySnapshot> {
    host.owned_learned_action_consistency_required().then(|| {
        let mut snapshot = host.learned_action_consistency_snapshot().unwrap();
        snapshot.consistency.journal_revision = 0; snapshot
    })
}
fn same_numerical(host: &FileOversight, before: &FileLearnedInspection) {
    let actual = host.learned_generation_inspection().unwrap();
    assert_eq!(actual.numerical, before.numerical);
    assert_eq!(actual.pending, before.pending);
    assert_eq!(actual.paused, before.paused);
}

#[test]
fn fresh_held_out_evidence_requalifies_original_native_reviewed_learned_continuation() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let mut seed = seed(&root, mode); let mut live = open(&seed);
        let deadline = deadline(); let mut control = Control::new(&seed.config, 2, None).unwrap();
        let (path, request) = fresh(&root, &seed, &live, 21, 3);
        let (before, original, forecast, cost) = {
            let host = live.driver.supervisor().host().unwrap();
            (host.learned_generation_inspection().unwrap(), host.inspect(),
                consistency(&host), owned_cost(&host))
        };
        let bytes = fs::read(seed.config.store.join("delivery.bin")).unwrap();
        for capsule in [None, Some(seed.old_capsule.as_path())] {
            assert!(qualification::prepare(&mut live.driver, &seed.config, capsule,
                (&mut control, &live.reviewer, &deadline), &mut || ElapsedTick(1000)).is_err());
            assert_eq!(fs::read(seed.config.store.join("delivery.bin")).unwrap(), bytes);
            same_numerical(&live.driver.supervisor().host().unwrap(), &before);
        }
        assert!(qualification::prepare(&mut live.driver, &seed.config, Some(&path),
            (&mut control, &live.reviewer, &deadline), &mut || ElapsedTick(1000)).unwrap());
        {
            let host = live.driver.supervisor().host().unwrap();
            host.check_credibility().unwrap(); same_numerical(&host, &before);
            assert_eq!(consistency(&host), forecast); assert_eq!(owned_cost(&host), cost);
            let change = host.credibility_change(21).unwrap();
            assert_eq!(change.reducer_generation, 3);
            assert_eq!(change.sequence, request.expected_control_sequence + 1);
            assert_eq!(change.revocation_floor, request.expected_epoch + 1);
            assert_eq!(host.inspect().executions, original.executions);
            assert_eq!(host.inspect().control.ledger.charged, original.control.ledger.charged);
            assert!(!host.clock_ready(), "qualification is not a fresh source/time observation");
        }
        let command = live.driver.supervisor().host().unwrap()
            .prepare_learned_text_stream_continuation(1, 2).unwrap();
        assert!(continuation::prepare(&mut live.driver, &live.reviewer, &mut seed.config,
            command, &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
        assert!(generate_selected(&mut live.driver, &live.reviewer, &mut seed.config,
            &mut control, &deadline, &mut || ElapsedTick(1000), live.prediction.as_mut()).unwrap());
        submit(&mut live.driver, &mut live.wire, &mut seed.config, &live.loaded,
            &deadline, &mut || ElapsedTick(1000)).unwrap();
        assert!(live.driver.request_learned_human_approval_from_policy_file(&mut seed.config.source,
            702, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
        let (driver, history) = native::review_current(&mut seed.config, &live.loaded,
            &live.reviewer, live.driver, &deadline);
        live.driver = driver;
        assert!(matches!(live.driver.step_computed_from_policy_file(&mut seed.config.source,
            || ElapsedTick(1000), None).result.unwrap(), FileDriverEvent::AwaitingHuman { request: 2 }));
        let human = native::approve(&mut live.driver, &mut seed.config, &live.reviewer, 2);
        assert!(live.driver.step_computed_from_policy_file(&mut seed.config.source,
            || ElapsedTick(1000), Some(&seed.first_human)).result.is_err());
        assert_eq!(live.driver.supervisor().host().unwrap().inspect().executions, 1);
        native::publish(&mut live.driver, &mut seed.config, &human);
        assert!(executed(&mut live.wire, 2));
        {
            let host = live.driver.supervisor().host().unwrap(); let stream = host.stream_snapshot().unwrap();
            assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
            assert_eq!(stream.confirmed, stream.published); assert_eq!(stream.publication.executions, 2);
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.sampled_draws, 4); assert_eq!(numerical.position, 6);
            assert_eq!(host.learned_text_message(live.loaded.evidence).unwrap().evidence().tokens(),
                &[120, 121, 65, 256, 65, 256]);
            let charge = host.request_action(1).unwrap().spec().units
                + host.request_action(2).unwrap().spec().units;
            assert_eq!(stream.publication.control.ledger.charged, charge);
            assert_eq!(stream.publication.control.ledger.reserved, 0);
            if mode.is_some() {
                let forecast = host.action_consistency_snapshot().unwrap();
                assert_eq!(forecast.evidence.samples(), 2);
                assert!(!forecast.coverage_lost); assert_eq!(forecast.pending_attempt, None);
            }
        }
        assert_eq!(cleanup(live.driver, 1000, 1), 0); drop(history);
    }
}

#[test]
fn held_out_requalification_preserves_lost_pending_forecasts_and_original_numerical_intent() {
    for owned in [false, true] {
        let root = Root::new(); let mut seed = seed(&root, Some(owned)); let mut live = open(&seed);
        let deadline = deadline(); let mut control = Control::new(&seed.config, 2, None).unwrap();
        let (path, _) = fresh(&root, &seed, &live, 21, 3);
        assert!(qualification::prepare(&mut live.driver, &seed.config, Some(&path),
            (&mut control, &live.reviewer, &deadline), &mut || ElapsedTick(1000)).unwrap());
        let command = live.driver.supervisor().host().unwrap()
            .prepare_learned_text_stream_continuation(1, 2).unwrap();
        assert!(continuation::prepare(&mut live.driver, &live.reviewer, &mut seed.config,
            command, &mut control, &deadline, &mut || ElapsedTick(1000)).unwrap());
        let (before, pending, cost) = {
            let mut host = live.driver.supervisor_mut().host_mut().unwrap();
            live.prediction.as_mut().unwrap().begin_message(&mut host).unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            let revision = host.revision();
            host.begin_learned_step(revision, numerical.actor_revision, numerical.position).unwrap();
            let before = host.learned_generation_inspection().unwrap();
            assert!(before.pending.is_some()); assert_eq!(before.numerical.sampled_draws, 2);
            (before, host.action_consistency_deadline().unwrap().unwrap(), owned_cost(&host))
        };
        assert_eq!(cleanup(live.driver, 1000, 1), 0);
        drop(live.prediction); drop(live.wire);
        let mut live = open(&seed);
        let (path, _) = fresh(&root, &seed, &live, 22, 4);
        let frozen = {
            let host = live.driver.supervisor().host().unwrap();
            assert_eq!(host.learned_generation_inspection().unwrap().numerical, before.numerical);
            assert_eq!(host.learned_generation_inspection().unwrap().pending, before.pending);
            assert_eq!(host.action_consistency_deadline().unwrap(), Some(pending));
            let forecast = consistency(&host).unwrap();
            assert!(forecast.coverage_lost); assert_eq!(forecast.evidence.samples(), 1);
            if let Some(cost) = cost.as_ref() {
                let actual = owned_cost(&host).unwrap();
                assert_eq!(actual.work, cost.work);
                assert_eq!(actual.retained_source_bytes, cost.retained_source_bytes);
            }
            (host.learned_generation_inspection().unwrap(), forecast, owned_cost(&host))
        };
        assert!(qualification::prepare(&mut live.driver, &seed.config, Some(&path),
            (&mut control, &live.reviewer, &deadline), &mut || ElapsedTick(1000)).unwrap());
        {
            let host = live.driver.supervisor().host().unwrap();
            host.check_credibility().unwrap(); same_numerical(&host, &frozen.0);
            assert_eq!(consistency(&host), Some(frozen.1)); assert_eq!(owned_cost(&host), frozen.2);
            assert_eq!(host.action_consistency_deadline().unwrap(), Some(pending));
        }
        let bytes = fs::read(seed.config.store.join("delivery.bin")).unwrap();
        assert!(generate_selected(&mut live.driver, &live.reviewer, &mut seed.config,
            &mut control, &deadline, &mut || panic!("lost coverage must refuse before clock or source"),
            live.prediction.as_mut()).is_err());
        assert_eq!(fs::read(seed.config.store.join("delivery.bin")).unwrap(), bytes);
        same_numerical(&live.driver.supervisor().host().unwrap(), &frozen.0);
        assert_eq!(cleanup(live.driver, 1000, 1), 0);
    }
}

#[test]
fn missing_malformed_oversized_and_wrong_predecessor_capsules_do_not_mutate_recovered_history() {
    let root = Root::new(); let seed = seed(&root, None); let mut live = open(&seed);
    let deadline = deadline(); let mut control = Control::new(&seed.config, 2, None).unwrap();
    let (valid, original) = fresh(&root, &seed, &live, 21, 3);
    let malformed = root.0.join("malformed.bin"); fs::write(&malformed, b"not a credibility capsule").unwrap();
    let oversized = root.0.join("oversized.bin");
    fs::File::create(&oversized).unwrap()
        .set_len(CredibilityActivation::MAX_ENCODED_BYTES as u64 + 1).unwrap();
    let mut wrong = original.clone(); wrong.expected_epoch += 1; wrong.operation = 22;
    let stale = save(&root, &wrong);
    let mut wrong = original; wrong.scope.tenant += 1; wrong.operation = 23;
    let scope = save(&root, &wrong);
    let before = live.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
    let bytes = fs::read(seed.config.store.join("delivery.bin")).unwrap();
    for path in [root.0.join("missing.bin"), malformed, oversized, stale, scope] {
        assert!(qualification::prepare(&mut live.driver, &seed.config, Some(&path),
            (&mut control, &live.reviewer, &deadline), &mut || ElapsedTick(1000)).is_err(), "{path:?}");
        assert_eq!(fs::read(seed.config.store.join("delivery.bin")).unwrap(), bytes);
        let host = live.driver.supervisor().host().unwrap(); same_numerical(&host, &before);
        assert!(host.check_credibility().is_err()); assert_eq!(host.inspect().executions, 1);
    }
    assert_eq!(qualification::prepare(&mut live.driver, &seed.config,
        Some(&root.0.join("deadline-must-precede-this-missing-file")),
        (&mut control, &live.reviewer, &deadline), &mut || deadline.logical)
        .err().as_deref(), Some("one-shot workflow deadline reached"));
    assert_eq!(fs::read(seed.config.store.join("delivery.bin")).unwrap(), bytes);
    // The exact valid file through the same reader and original owner succeeds.
    assert!(qualification::prepare(&mut live.driver, &seed.config, Some(&valid),
        (&mut control, &live.reviewer, &deadline), &mut || ElapsedTick(1000)).unwrap());
    same_numerical(&live.driver.supervisor().host().unwrap(), &before);
    assert_eq!(cleanup(live.driver, 1000, 1), 0);
}

#[test]
fn deadline_and_policy_source_are_observed_again_after_held_out_activation() {
    for expired in [false, true] {
        let root = Root::new(); let mut seed = seed(&root, None); let mut live = open(&seed);
        let deadline = deadline(); let mut control = Control::new(&seed.config, 2, None).unwrap();
        let (path, _) = fresh(&root, &seed, &live, 21, 3);
        let before = live.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
        let mut calls = 0;
        let result = qualification::prepare(&mut live.driver, &seed.config, Some(&path),
            (&mut control, &live.reviewer, &deadline), &mut || {
                calls += 1;
                if calls == 2 {
                    if expired { return deadline.logical; }
                    fs::remove_file(root.0.join("evidence.json")).unwrap();
                }
                ElapsedTick(1000)
            });
        assert_eq!(calls, 2);
        if expired { assert_eq!(result.err().as_deref(), Some("one-shot workflow deadline reached")); }
        else {
            assert!(result.unwrap());
            let command = live.driver.supervisor().host().unwrap()
                .prepare_learned_text_stream_continuation(1, 2).unwrap();
            assert!(continuation::prepare(&mut live.driver, &live.reviewer, &mut seed.config,
                command, &mut control, &deadline, &mut || ElapsedTick(1000)).is_err());
        }
        let host = live.driver.supervisor().host().unwrap();
        assert_eq!(host.credibility_change(21).unwrap().reducer_generation, 3);
        same_numerical(&host, &before); assert_eq!(host.inspect().executions, 1);
        drop(host); assert_eq!(cleanup(live.driver, 1000, 1), 0);
    }
}

#[test]
fn actual_learned_entry_can_retry_omitted_qualification_then_checks_time_after_activation() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let seed = seed(&root, mode);
        let loaded = recipe::load(&seed.next_recipe, &seed.config, true).unwrap();
        let generation = loaded.generation.clone();
        let peers = fixture::peers(&root, &seed.config);
        let profile = seed.config.profile.clone(); let store = seed.config.store.clone();
        // Omission keeps the old recover-then-refuse behavior. It must not
        // terminally stop a history that can be retried with fresh evidence.
        let result = run_qualified(configured(&root), loaded, &peers, true,
            None, Some(1), None, || ElapsedTick(1000));
        assert!(result.err().is_some_and(|error| error.contains("Stale")));
        let before = FileOversight::read_publication_with_learned_generation(&store,
            &profile, &generation).unwrap();
        assert_eq!(before.revision, seed.revision + 1);
        assert!(before.stop.is_none()); assert_eq!(before.executions, 1);
        let request = activation(&seed.config, before.control.sequence,
            before.control.ledger.epoch + 1, 21, 3); // the next original recovery fence
        let path = save(&root, &request);
        let loaded = recipe::load(&seed.next_recipe, &seed.config, true).unwrap();
        let mut calls = 0_u64;
        // Start + pre-open + N event checkpoints + Ready + the helper's two
        // checks. Expiry at N+5 is after the native activation was acknowledged.
        let expire_at = before.revision + 5;
        let result = run_qualified(configured(&root), loaded, &peers, true,
            None, Some(1), Some(&path), || {
                calls += 1;
                if calls >= expire_at { ElapsedTick(61_000) } else { ElapsedTick(1000) }
            }).unwrap();
        assert!(calls >= expire_at);
        assert!(result.failure.as_deref().is_some_and(|error|
            error.starts_with("one-shot workflow deadline reached")), "{:?}", result.failure);
        assert_eq!(result.cleanup_pending, 0);
        let config = configured(&root);
        let loaded = recipe::load(&seed.next_recipe, &config, true).unwrap();
        let (host, _reviewer, _observer) = recovery::open_selected(&config, &loaded).unwrap();
        assert_eq!(host.credibility_change(21).unwrap().reducer_generation, 3,
            "the actual entry must reach the supplied original activation before expiry");
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, 4);
        assert!(host.inspect().stop.is_some()); assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, before.control.ledger.charged);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
    }
}

#[test]
#[cfg(target_os = "linux")]
fn authenticated_stop_finishes_before_any_qualification_file_read() {
    use crate::workflow::control::socket_path;
    use fa_reference::action::consequence::delivery::persistent::observed::reviewer::peer::{
        PeerCredentials, PeerPolicy, VerifiedReviewerSocket, control::StopClientProgress,
    };
    use std::os::unix::net::UnixStream;
    let root = Root::new(); let seed = seed(&root, None); let mut live = open(&seed);
    let peers = fixture::peers(&root, &seed.config);
    let mut control = Control::new(&seed.config, 2, Some(&peers)).unwrap();
    let (identity, _other) = UnixStream::pair().unwrap();
    let id = PeerCredentials::observe(&identity).unwrap();
    let socket = VerifiedReviewerSocket::verify(UnixStream::connect(socket_path(&peers, 2)).unwrap(),
        PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap()).unwrap();
    let mut client = socket.into_stop_client(peers.expected, 2).unwrap();
    // The native client is already queued before entering the helper, removing
    // a scheduling race between the checkpoint and this genuine admitted peer.
    let client = std::thread::spawn(move || {
        let started = Instant::now();
        loop {
            assert!(started.elapsed() < Duration::from_secs(10), "native stop exchange timed out");
            match client.step().unwrap() {
                StopClientProgress::NeedsDecision => client.request_stop().unwrap(),
                StopClientProgress::Complete => return client.receipt().unwrap(),
                _ => std::thread::sleep(Duration::from_millis(1)),
            }
        }
    });
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let before = live.driver.supervisor().host().unwrap().learned_generation_inspection().unwrap();
    let result = qualification::prepare(&mut live.driver, &seed.config,
        Some(&root.0.join("stop-must-precede-this-missing-capsule")),
        (&mut control, &live.reviewer, &deadline()), &mut || ElapsedTick(1000));
    let receipt = client.join().unwrap();
    assert!(receipt.acknowledged() && receipt.drained()); assert!(!result.unwrap());
    let host = live.driver.supervisor().host().unwrap();
    same_numerical(&host, &before); assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().stop.as_ref().unwrap().request().operation, 2);
    assert!(host.credibility_change(21).is_err());
    drop(host); assert_eq!(cleanup(live.driver, 1000, 1), 0);
}

fn write_config(root: &Root) -> PathBuf {
    let text = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/supervised_publication.json"))
        .replace("/operator/publication-store", root.0.join("store").to_str().unwrap())
        .replace("/operator/evidence.json", root.0.join("evidence.json").to_str().unwrap())
        .replace("\"initial_payload_hex\": \"696e697469616c\"", "\"initial_payload_hex\": \"\"")
        .replace("\"events\": 256", "\"events\": 1024");
    let path = root.0.join("config.json"); fs::write(&path, text).unwrap(); path
}

#[test]
fn actual_receipt_command_ignores_missing_capsule_and_live_sources_after_qualification() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let seed = seed(&root, mode);
        let selected = recipe::load(&seed.first_recipe, &seed.config, false).unwrap();
        let before = FileOversight::read_publication_with_learned_generation(&seed.config.store,
            &seed.config.profile, &selected.generation).unwrap();
        let config_path = write_config(&root);
        let disk_config = Config::read(&config_path).unwrap();
        crate::config::assert_oversight_profiles_eq(&disk_config.profile, &seed.config.profile);
        assert_eq!(disk_config.source_policy, seed.config.source_policy);
        for name in ["evidence.json", "native-roster.json", "helper-model.json",
            "helper-weights.safetensors", "helper-monitor.json", "helper-tokenizer.bbpe",
            "helper-sampling.json", "qualification-20.bin"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        let missing = root.0.join("receipt-must-not-read-this-capsule");
        let args = ["create-learned-generated".to_owned(), config_path.to_str().unwrap().to_owned(),
            seed.first_recipe.to_str().unwrap().to_owned(), "--resume".to_owned(),
            "--credibility-activation".to_owned(), missing.to_str().unwrap().to_owned()];
        assert!(crate::command(args.to_vec()).is_ok());
        let after = FileOversight::read_publication_with_learned_generation(&seed.config.store,
            &seed.config.profile, &selected.generation).unwrap();
        assert_eq!(after.executions, 1); assert_eq!(after.payload, before.payload);
        assert_eq!(after.control.ledger.charged, before.control.ledger.charged);
        assert_eq!(after.control.ledger.reserved, 0);
        assert!(after.stop.is_none()); assert_eq!(after.revision, before.revision + 1);
        assert!(!Path::new(&missing).exists());
    }
}
