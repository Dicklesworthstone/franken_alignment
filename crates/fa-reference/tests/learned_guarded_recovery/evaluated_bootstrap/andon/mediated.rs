//! Actual original numerical/effect APIs with complete independent topology and
//! accounting recovery. Declared graphs are not OS containment evidence.
use super::*;
#[path = "../../../support/file_mediation.rs"]
mod topology_fixture;
use fa_reference::action::consequence::delivery::persistent::observed::{
    decoder::learned::{FileLearnedPublicationProfile, FileLearnedRecoveryStatus},
    guarded::{
        anchored::FileHistoryAnchor,
        evaluation::andon::{
            FileAndonRecoveryRequirements, learned::FileMediatedAndonLearnedRecovery,
        },
        mediated::{FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement},
    },
    mediation::{FileMediationObserver, FileMediationUpdate},
};
use fa_reference::action::consequence::delivery::publication_gate::PublicationLimits;
use fa_reference::action::consequence::mediation::{
    AuthorityGraph, CutCheck, MAX_CHECK_EDGE_VISITS,
};
use fa_reference::action::consequence::oversight::joint_credibility::{
    JointPromotionPolicy, JointReplayBudget,
};
use fa_reference::witness::refinement::RefinementBudget;
use std::cell::Cell;

fn graph(generation: u64) -> AuthorityGraph {
    topology_fixture::graph(
        profile().delivery.scope,
        profile().delivery.target,
        generation,
        false,
    )
}

fn joint() -> JointPromotionPolicy {
    let rate = Fraction {
        numerator: 1,
        denominator: 2,
    };
    JointPromotionPolicy::new(71, 1, rate, rate, JointReplayBudget::default()).unwrap()
}

fn create(
    root: &Directory,
    recipe: FileLearnedConfig,
    joint: Option<JointPromotionPolicy>,
) -> (FileOversight, FileMediatedRoles) {
    match joint {
        Some(joint) => {
            FileOversight::create_joint_mediated_andon_guarded_with_learned_generation_reserved(
                root.store(),
                profile(),
                &guards(),
                None,
                graph(1),
                protocol(),
                joint,
                &accounting(),
                recipe,
                RecoveryReserve::terminal(),
            )
        }
        None => FileOversight::create_mediated_andon_guarded_with_learned_generation_reserved(
            root.store(),
            profile(),
            &guards(),
            None,
            graph(1),
            protocol(),
            &accounting(),
            recipe,
            RecoveryReserve::terminal(),
        ),
    }
    .unwrap()
}

fn retained(
    host: &FileOversight,
    joint: Option<JointPromotionPolicy>,
) -> (FileAndonRecoveryRequirements, FileTopologyRequirement) {
    let mut expected = FileAndonRecoveryRequirements::new(
        requirements(host, guards()),
        accounting(),
        host.andon_ledger()
            .unwrap()
            .windows()
            .values()
            .map(|window| window.budget)
            .collect(),
        host.andon_source().unwrap().revision(),
    )
    .unwrap();
    if let Some(joint) = joint {
        expected = expected.require_joint(joint).unwrap();
    }
    if let Some(limits) = host.publication_validation_profile().unwrap() {
        expected = expected.require_publication(limits).unwrap();
    }
    let topology = host.mediation_snapshot().unwrap();
    (
        expected,
        FileTopologyRequirement {
            initial: graph(1),
            current: topology.graph,
            available: topology.available,
        },
    )
}

fn begin(
    root: &Directory,
    expected: &FileAndonRecoveryRequirements,
    topology: &FileTopologyRequirement,
    recipe: &FileLearnedConfig,
    anchor: &FileHistoryAnchor,
) -> Result<FileMediatedAndonLearnedRecovery, JournalError> {
    FileOversight::begin_open_mediated_andon_guarded_anchored_with_learned_generation(
        root.store(),
        profile(),
        expected,
        topology,
        &protocol(),
        recipe,
        anchor,
    )
}

fn drive(run: &mut FileMediatedAndonLearnedRecovery) -> Result<(), JournalError> {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1)?;
        assert_eq!(after.replayed_events, before.replayed_events + 1);
        assert_eq!(after.total_events, before.total_events);
    }
    Ok(())
}

fn certify(host: &mut FileOversight, observer: &FileMediationObserver) {
    let proposal = host
        .mediation_snapshot()
        .unwrap()
        .graph
        .propose_cut(&[2])
        .unwrap();
    let revision = host.revision();
    let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(
        observer
            .certify(host, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS)
            .unwrap()
            .unwrap(),
        CutCheck::Verified(_)
    ));
}

fn update(
    host: &mut FileOversight,
    observer: &FileMediationObserver,
    operation: u64,
    generation: u64,
) {
    let request = FileMediationUpdate {
        operation,
        expected_generation: host.mediation_snapshot().unwrap().graph.spec().generation,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        next: Some(graph(generation)),
    };
    observer.update(host, host.revision(), &request).unwrap();
}

fn label(host: &mut FileOversight, roles: &FileMediatedRoles, round: u64, truth: GroundTruth) {
    let ticket = host.evaluation_ticket(round).unwrap();
    let revision = host.revision();
    let evidence = u8::try_from(round).unwrap()
        ^ if matches!(truth, GroundTruth::Censored) {
            128
        } else {
            0
        };
    assert!(
        roles
            .evaluator
            .as_ref()
            .unwrap()
            .assess(
                host,
                revision,
                &ticket,
                Assessment {
                    origin: 201,
                    evidence_id: [evidence; 32],
                    truth,
                }
            )
            .unwrap()
    );
}

fn effect(
    host: &mut FileOversight,
    roles: &FileMediatedRoles,
    attempt: u64,
    round: u64,
    tick: u64,
) {
    let (action, input, automatic, request) = prepared(host, attempt, round);
    label(host, roles, round, GroundTruth::Benign);
    assert!(
        host.publish_checked(
            host.revision(),
            attempt,
            Some(&input),
            snapshot(),
            ElapsedTick(tick)
        )
        .is_err()
    );
    let revision = host.revision();
    let human = roles
        .oversight
        .human
        .approve(host, revision, &request)
        .unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        &input,
        snapshot(),
    )
    .unwrap();
    assert!(matches!(
        host.publish_checked(
            host.revision(),
            attempt,
            Some(&input),
            snapshot(),
            ElapsedTick(tick)
        )
        .unwrap()
        .outcome,
        EndpointOutcome::Executed { .. }
    ));
    host.reconcile(host.revision(), attempt).unwrap();
}

#[test]
fn original_numerical_effects_and_independent_labels_continue_after_complete_mediated_recovery() {
    for policy in [None, Some(joint())] {
        let root = Directory::new();
        let recipe = config(0);
        let (mut host, old) = create(&root, recipe.clone(), policy);
        assert_eq!(host.revision(), 8);
        assert!(old.evaluator.is_some() && old.consistency_observer.is_none());
        assert_eq!(host.joint_credibility_policy().unwrap(), policy);
        assert_eq!(host.andon_config().unwrap(), Some(&accounting()));
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert!(!host.clock_ready());
        assert_eq!(
            host.learned_generation_inspection()
                .unwrap()
                .numerical
                .work
                .admitted_tokens,
            0
        );
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        certify(&mut host, &old.topology_observer);
        step(&mut host).unwrap();
        fresh_identity(&mut host, &old.oversight, 1, 1);
        effect(&mut host, &old, 1, 101, 1);
        let (_, input) = staged(&mut host, 2);
        reviewed(&mut host, 2, 102, &input, Verdict::Hold).unwrap();
        label(&mut host, &old, 102, GroundTruth::Censored);
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
        host.register_andon_window(
            host.revision(),
            AndonBudget {
                window: 2,
                start: 100,
                end: 200,
                false_stops_allowed: 2,
                cost_per_stop: 9,
            },
        )
        .unwrap();
        let (expected, topology) = retained(&host, policy);
        let anchor = host.history_anchor().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let ledger = host.andon_ledger().unwrap().clone();
        let report = host.credibility_report().unwrap();
        let labels = host.evaluation_history(102).unwrap().to_vec();
        let bytes = root.bytes();
        drop(host);
        let pending = root.store().join("delivery.pending");
        std::fs::write(&pending, b"unacknowledged staging").unwrap();
        let mut run = begin(&root, &expected, &topology, &recipe, &anchor)
            .unwrap()
            .require_recovery_reserve(RecoveryReserve::terminal())
            .unwrap();
        assert_eq!(run.progress().replayed_events, 0);
        assert_eq!(run.advance(1, 1).err(), Some(Error::Stale.into()));
        assert_eq!(run.advance(0, 0).err(), Some(Error::InvalidInput.into()));
        assert_eq!(run.stop_request(990).err(), Some(Error::Incomplete.into()));
        assert_eq!(
            begin(&root, &expected, &topology, &recipe, &anchor).err(),
            Some(JournalError::Busy)
        );
        drive(&mut run).unwrap();
        assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
        assert_eq!(root.bytes(), bytes);
        drop(run);
        assert!(pending.exists());
        let mut run = begin(&root, &expected, &topology, &recipe, &anchor).unwrap();
        drive(&mut run).unwrap();
        let (mut host, fresh) = run.finish().unwrap();
        assert_eq!(
            host.revision(),
            expected.base().minimum.journal_revision + 1
        );
        assert!(!pending.exists());
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical
        );
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.andon_ledger().unwrap(), &ledger);
        assert_eq!(host.credibility_report().unwrap(), report);
        assert_eq!(host.evaluation_history(102).unwrap(), labels);
        assert_eq!(host.joint_credibility_policy().unwrap(), policy);
        assert_eq!(host.mediation_snapshot().unwrap().graph, topology.current);
        assert!(!host.mediation_snapshot().unwrap().available);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        assert!(!host.clock_ready());
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(
            host.journal_capacity().unwrap().reserve(),
            Some(RecoveryReserve::terminal())
        );
        fresh
            .evaluator
            .as_ref()
            .unwrap()
            .check_owner(&host)
            .unwrap();
        assert_eq!(
            old.evaluator.as_ref().unwrap().check_owner(&host).err(),
            Some(Error::Binding.into())
        );
        let proposal = topology.current.propose_cut(&[2]).unwrap();
        let revision = host.revision();
        let epoch = host.inspect().control.ledger.epoch;
        assert_eq!(
            old.topology_observer
                .certify(&mut host, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS)
                .err(),
            Some(Error::Binding.into())
        );
        assert!(
            host.resume_learned_generation(
                host.revision(),
                numerical.actor_revision,
                numerical.position
            )
            .is_err()
        );
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        update(&mut host, &fresh.topology_observer, 71, 2);
        certify(&mut host, &fresh.topology_observer);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        host.resume_learned_generation(
            host.revision(),
            numerical.actor_revision,
            numerical.position,
        )
        .unwrap();
        step(&mut host).unwrap();
        fresh_identity(&mut host, &fresh.oversight, 2, 2);
        label(&mut host, &fresh, 102, GroundTruth::Benign);
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].total_cost, 7);
        assert_eq!(host.andon_ledger().unwrap().windows()[&2].false_stops, 0);
        effect(&mut host, &fresh, 3, 103, 2);
        assert_eq!(host.inspect().executions, 2);
        assert_eq!(host.mediation_snapshot().unwrap().graph, graph(2));
    }
}

#[test]
fn exact_accounting_joint_publication_and_topology_cannot_be_omitted_or_changed() {
    let root = Directory::new();
    let limits = PublicationLimits {
        bindings: 4,
        validation: RefinementBudget {
            steps: 32,
            value_bytes: 128,
        },
    };
    let recipe = config(0)
        .with_required_publication_validation(FileLearnedPublicationProfile { limits, feed: None })
        .unwrap();
    let (mut host, roles) = create(&root, recipe.clone(), Some(joint()));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    certify(&mut host, &roles.topology_observer);
    step(&mut host).unwrap();
    update(&mut host, &roles.topology_observer, 71, 2);
    certify(&mut host, &roles.topology_observer);
    host.register_andon_window(
        host.revision(),
        AndonBudget {
            window: 2,
            start: 100,
            end: 200,
            false_stops_allowed: 2,
            cost_per_stop: 9,
        },
    )
    .unwrap();
    let (expected, topology) = retained(&host, Some(joint()));
    let anchor = host.history_anchor().unwrap();
    let bytes = root.bytes();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"preserve on failed binding").unwrap();
    for field in 0..8 {
        let mut selected_topology = topology.clone();
        let mut windows = expected.windows().to_vec();
        let mut selected_accounting = accounting();
        if field == 0 {
            selected_topology.initial = graph(2);
        }
        if field == 1 {
            selected_topology.current = graph(3);
        }
        if field == 2 {
            windows.pop();
        }
        if field == 3 {
            selected_accounting = FileAndonConfig::new(
                LedgerIdentity {
                    deployment: 52,
                    evaluator: 52,
                },
                accounting().windows().to_vec(),
            )
            .unwrap();
        }
        let mut selected = FileAndonRecoveryRequirements::new(
            expected.base().clone(),
            selected_accounting,
            windows,
            expected.minimum_evaluation_revision(),
        )
        .unwrap();
        if field != 4 {
            let policy = if field == 5 {
                let original = joint();
                let mut budget = original.budget();
                budget.cases -= 1;
                JointPromotionPolicy::new(
                    original.id(),
                    original.generation(),
                    original.maximum_escape_rate(),
                    original.maximum_benign_stop_rate(),
                    budget,
                )
                .unwrap()
            } else {
                joint()
            };
            selected = selected.require_joint(policy).unwrap();
        }
        if field != 6 {
            let mut selected_limits = limits;
            if field == 7 {
                selected_limits.validation.steps += 1;
            }
            selected = selected.require_publication(selected_limits).unwrap();
        }
        assert_eq!(
            begin(&root, &selected, &selected_topology, &recipe, &anchor).err(),
            Some(Error::Binding.into()),
            "field {field}"
        );
        assert_eq!(root.bytes(), bytes);
        assert!(pending.exists());
    }
    assert_eq!(
        begin(&root, &expected, &topology, &config(0), &anchor).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(
        FileOversight::begin_open_andon_guarded_anchored_with_learned_generation(
            root.store(),
            profile(),
            &expected,
            &protocol(),
            &recipe,
            &anchor,
        )
        .err(),
        Some(Error::Binding.into())
    );
    let ordinary = FileMediatedRequirements {
        oversight: expected.base().clone(),
        topology: topology.clone(),
        prediction: None,
        evaluation: Some(protocol()),
    };
    assert_eq!(
        FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
            root.store(),
            profile(),
            &ordinary,
            &recipe,
            &anchor,
        )
        .err(),
        Some(Error::Binding.into())
    );
    let other = Directory::new();
    let (foreign, _) = create(&other, recipe.clone(), Some(joint()));
    let foreign_anchor = foreign.history_anchor().unwrap();
    drop(foreign);
    assert_eq!(
        begin(&root, &expected, &topology, &recipe, &foreign_anchor).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(root.bytes(), bytes);
    let mut run = begin(&root, &expected, &topology, &recipe, &anchor).unwrap();
    drive(&mut run).unwrap();
    let (host, roles) = run.finish().unwrap();
    assert_eq!(host.andon_ledger().unwrap().windows().len(), 2);
    assert_eq!(host.publication_validation_profile().unwrap(), Some(limits));
    assert_eq!(host.joint_credibility_policy().unwrap(), Some(joint()));
    assert_eq!(host.mediation_snapshot().unwrap().graph, graph(2));
    roles.evaluator.unwrap().check_owner(&host).unwrap();
}

#[test]
fn availability_and_independent_floors_latch_failure_before_fence_or_stop_clock() {
    let root = Directory::new();
    let recipe = config(0);
    let (mut host, roles) = create(&root, recipe.clone(), Some(joint()));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    certify(&mut host, &roles.topology_observer);
    step(&mut host).unwrap();
    let (expected, topology) = retained(&host, Some(joint()));
    let anchor = host.history_anchor().unwrap();
    let bytes = root.bytes();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged staging").unwrap();
    let mut original = begin(&root, &expected, &topology, &recipe, &anchor).unwrap();
    drive(&mut original).unwrap();
    let stop = original.stop_request(990).unwrap();
    drop(original);
    for field in 0..8 {
        let mut base = expected.base().clone();
        let mut selected_topology = topology.clone();
        let mut floor = expected.minimum_evaluation_revision();
        match field {
            0 => selected_topology.available = !selected_topology.available,
            1 => base.minimum.journal_revision += 1,
            2 => base.minimum.control_sequence += 1,
            3 => base.minimum.authority_epoch += 1,
            4 => floor += 1,
            5 => base.guards.identity = None,
            6 => base.guards.campaigns = None,
            _ => {
                base.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap()
            }
        }
        let selected = FileAndonRecoveryRequirements::new(
            base,
            accounting(),
            expected.windows().to_vec(),
            floor,
        )
        .unwrap()
        .require_joint(joint())
        .unwrap();
        let mut run = begin(&root, &selected, &selected_topology, &recipe, &anchor).unwrap();
        assert!(drive(&mut run).is_err(), "field {field}");
        let FileLearnedRecoveryStatus::Failed(cause) = run.progress().status else {
            panic!("a complete-contract mismatch cannot expose Ready: field {field}");
        };
        assert_eq!(
            cause,
            if matches!(field, 1..=4) {
                Error::Stale
            } else {
                Error::Binding
            }
        );
        assert_eq!(
            run.advance(run.progress().replayed_events, 1).err(),
            Some(cause.into())
        );
        assert_eq!(run.stop_request(990).err(), Some(cause.into()));
        let clock_calls = Cell::new(0);
        assert_eq!(
            run.finish_stopped_with_anchor(stop, || {
                clock_calls.set(clock_calls.get() + 1);
                ElapsedTick(2)
            })
            .err(),
            Some(cause.into())
        );
        assert_eq!(clock_calls.get(), 0);
        assert_eq!(root.bytes(), bytes);
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging");
    }
    let mut run = begin(&root, &expected, &topology, &recipe, &anchor).unwrap();
    drive(&mut run).unwrap();
    let (host, _) = run.finish().unwrap();
    assert_eq!(
        host.revision(),
        expected.base().minimum.journal_revision + 1
    );
}

#[test]
fn complete_mediated_accounting_stop_preserves_unknown_charge_and_exact_retry() {
    let root = Directory::new();
    let recipe = config(0);
    let (mut host, old) = create(&root, recipe.clone(), Some(joint()));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    certify(&mut host, &old.topology_observer);
    step(&mut host).unwrap();
    fresh_identity(&mut host, &old.oversight, 1, 1);
    let (action, input, automatic, request) = prepared(&mut host, 1, 101);
    let revision = host.revision();
    let human = old
        .oversight
        .human
        .approve(&mut host, revision, &request)
        .unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        &input,
        snapshot(),
    )
    .unwrap();
    let (expected, topology) = retained(&host, Some(joint()));
    let anchor = host.history_anchor().unwrap();
    let bytes = root.bytes();
    drop(host);
    let mut run = begin(&root, &expected, &topology, &recipe, &anchor)
        .unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal())
        .unwrap();
    drive(&mut run).unwrap();
    let stop = run.stop_request(990).unwrap();
    assert_eq!(root.bytes(), bytes);
    let stopped = run.finish_admission_stop(stop).unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(stopped.snapshot().control.ledger.charged, 16);
    assert_eq!(
        stopped.snapshot().control.ledger.stages[&1],
        ActionState::Unknown
    );
    assert_eq!(stopped.snapshot().executions, 0);
    assert_eq!(
        stopped.snapshot().control.ledger.elapsed,
        Some(ElapsedTick(1))
    );
    let stopped_bytes = root.bytes();
    let withdrawn = FileTopologyRequirement {
        available: false,
        ..topology
    };
    let mut retry = begin(
        &root,
        &expected,
        &withdrawn,
        &recipe,
        stopped.history_anchor(),
    )
    .unwrap();
    drive(&mut retry).unwrap();
    let stop = retry.stop_request(990).unwrap();
    let confirmed = retry.finish_admission_stop(stop).unwrap();
    assert_eq!(confirmed.appended_events(), 0);
    assert_eq!(confirmed.snapshot(), stopped.snapshot());
    assert_eq!(confirmed.history_anchor(), stopped.history_anchor());
    assert_eq!(root.bytes(), stopped_bytes);
}

#[test]
fn composed_bootstrap_reserves_the_original_terminal_tail_before_creating_storage() {
    let root = Directory::new();
    let (host, _) = create(&root, config(0), Some(joint()));
    let complete = usize::try_from(host.revision()).unwrap();
    drop(host);
    let mut too_small = profile();
    too_small.delivery.limits.events = complete + RecoveryReserve::terminal().events - 1;
    let refused = Directory::new();
    assert_eq!(
        FileOversight::create_joint_mediated_andon_guarded_with_learned_generation_reserved(
            refused.store(),
            too_small,
            &guards(),
            None,
            graph(1),
            protocol(),
            joint(),
            &accounting(),
            config(0),
            RecoveryReserve::terminal(),
        )
        .err(),
        Some(Error::Limit.into())
    );
    assert!(!refused.store().exists());
    let mut exact = profile();
    exact.delivery.limits.events = complete + RecoveryReserve::terminal().events;
    let accepted = Directory::new();
    let (host, roles) =
        FileOversight::create_joint_mediated_andon_guarded_with_learned_generation_reserved(
            accepted.store(),
            exact,
            &guards(),
            None,
            graph(1),
            protocol(),
            joint(),
            &accounting(),
            config(0),
            RecoveryReserve::terminal(),
        )
        .unwrap();
    assert_eq!(host.revision(), complete as u64);
    assert!(host.journal_capacity().unwrap().terminal_space_remaining());
    roles.evaluator.unwrap().check_owner(&host).unwrap();
}

#[test]
fn unsupported_predictor_and_held_out_custody_refuse_before_numerical_recovery_or_storage() {
    use fa_reference::action::consequence::activation::{
        consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
        monitor::learned::LearnedMonitorBudget,
        probe::learned::MAX_CHECKED_KV_BYTES,
        tensor::kv::experiment::KvSide,
    };
    use fa_reference::action::consequence::delivery::persistent::observed::{
        consistency::{
            FileConsistencyConfig, FileConsistencyParameters, learned::FileLearnedConsistencyConfig,
        },
        credibility::held_out_joint::{HeldOutJointBudget, HeldOutJointPolicy},
    };

    let recipe = config(0);
    let model = numerical::model();
    let neutral = BinaryForecast::new(32_768, 32_768).unwrap();
    let forecast = FileLearnedConsistencyConfig::new(
        FileConsistencyConfig::new(FileConsistencyParameters {
            probe_id: 91,
            probe_generation: 1,
            profile: model.cache_profile().layers()[&1].values().profile(),
            weights: vec![1.0, 0.0],
            bias: 0.0,
            threshold: 0.0,
            forecast: ForecastRegistration {
                domain: 71,
                generation: 1,
                policy_generation: 1,
                event_prefix: b"a".to_vec(),
                negative: neutral,
                at_threshold: neutral,
                positive: neutral,
            },
            alpha: ErrorBudget::new(1, 2).unwrap(),
            stream: 21,
            max_predictions: 8,
            max_prediction_age_ticks: 50,
        })
        .unwrap(),
        1,
        KvSide::Value,
        LearnedMonitorBudget::default(),
        LearnedMonitorBudget::default(),
        MAX_CHECKED_KV_BYTES,
    )
    .unwrap();
    let root = Directory::new();
    let (mut host, _) = create(&root, recipe.clone(), Some(joint()));
    host.enable_learned_action_consistency(host.revision(), forecast)
        .unwrap();
    let (expected, topology) = retained(&host, Some(joint()));
    let anchor = host.history_anchor().unwrap();
    let bytes = root.bytes();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"standalone predictor custody is required").unwrap();
    assert_eq!(
        begin(&root, &expected, &topology, &recipe, &anchor).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(root.bytes(), bytes);
    assert!(pending.exists());

    let held_out = recipe
        .with_required_held_out_joint(
            HeldOutJointPolicy::new(81, 1, 1, 1, 0, 0, HeldOutJointBudget::default()).unwrap(),
        )
        .unwrap();
    for policy in [None, Some(joint())] {
        let root = Directory::new();
        let result = match policy {
            None => FileOversight::create_mediated_andon_guarded_with_learned_generation_reserved(
                root.store(),
                profile(),
                &guards(),
                None,
                graph(1),
                protocol(),
                &accounting(),
                held_out.clone(),
                RecoveryReserve::terminal(),
            ),
            Some(joint) => {
                FileOversight::create_joint_mediated_andon_guarded_with_learned_generation_reserved(
                    root.store(),
                    profile(),
                    &guards(),
                    None,
                    graph(1),
                    protocol(),
                    joint,
                    &accounting(),
                    held_out.clone(),
                    RecoveryReserve::terminal(),
                )
            }
        };
        assert_eq!(result.err(), Some(Error::Binding.into()));
        assert!(!root.store().exists());
    }
}
