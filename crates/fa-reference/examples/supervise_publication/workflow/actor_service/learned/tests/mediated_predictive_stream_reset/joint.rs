//! Real preparation/containment/reset followed by one original recovered owner.
//! Direct qualification exposes unchanged state on refusal; the command-level
//! baseline and independently authenticated review paths are tested separately.
use super::*;
use super::super::{continuation as native, identity_tests as identity_fixture};
use super::super::super::{identity as identity_check, prediction, qualification, topology};
use crate::workflow::continuation::checkpoint::Output;
use fa_reference::action::consequence::delivery::persistent::{
    credibility::CredibilityActivation,
    observed::credibility::held_out_joint::{HeldOutJointBudget, HeldOutJointPolicy},
};
use fa_reference::action::consequence::gate::containment::session::policy::controller::credibility::joint::evaluate;
use fa_reference::action::consequence::oversight::identity::IdentityOutcome;

#[path = "joint/campaign.rs"]
mod campaign;

fn configured_case(root: &Root) -> Case {
    let mut case = Case::new(root);
    let original = fs::read_to_string(&case.config_path).unwrap();
    let config = original
        .replace("\"continue_minimum\": 8", "\"continue_minimum\": 4")
        .replace(
            "\"continue_hold_maximum\": 0",
            "\"continue_hold_maximum\": 4",
        );
    assert_ne!(config, original);
    fs::write(&case.config_path, config).unwrap();
    case.config = Config::read_learned_generation(&case.config_path).unwrap();
    assert_eq!(case.config.profile.delivery.congress.continue_minimum, 4);
    assert_eq!(
        case.config.profile.delivery.congress.continue_hold_maximum,
        4
    );
    // This causal congress is frozen before owner creation. Both the good and
    // regressing campaigns below face these same thresholds and original caps.
    identity_fixture::provision(root, &case.recipe, true);
    let original = fs::read_to_string(&case.recipe).unwrap();
    fs::write(
        &case.recipe,
        original
            .replacen(
                "\"minimum_violation_roots\":1",
                "\"minimum_violation_roots\":2",
                1,
            )
            .replacen("\"max_cases\":2", "\"max_cases\":3", 1)
            .replacen("\"max_member_outcomes\":8", "\"max_member_outcomes\":12", 1),
    )
    .unwrap();
    evidence(root, &case.config, 1, true, false);
    case
}

fn select_fresh_identity(case: &Case) {
    let original = fs::read_to_string(&case.recipe).unwrap();
    fs::write(
        &case.recipe,
        original
            .replacen("\"check\":901", "\"check\":902", 1)
            .replacen(
                "\"measurement_sequence\":1",
                "\"measurement_sequence\":2",
                1,
            ),
    )
    .unwrap();
}

struct Live {
    loaded: Loaded,
    driver: FileSupervisedDriver,
    reviewer: FileHumanReviewer,
    prediction: Option<prediction::Session>,
    identity: Option<identity_check::Session>,
    wire: Wire,
    control: Control,
    deadline: Deadline,
    output: Output,
    pair: Pair,
}

fn open(case: &Case, input: &Pair) -> Live {
    let pair = case.pair("qualified");
    let profile = case.profile(input, &pair, "qualify", Some((2, false)));
    let mut loaded = recipe::load(&case.recipe, &case.config, true).unwrap();
    assert!(
        loaded.native.is_some(),
        "live continuation loads both actual native reviewers"
    );
    loaded.recovery_anchor = Some(custody::load(&input.history).unwrap());
    loaded.anchor_output = Some(pair.history.clone());
    topology::admit(
        &case.config,
        &mut loaded,
        Some(&profile),
        topology::Mode::Live,
    )
    .unwrap();
    let deadline = deadline();
    let mut control = Control::new(&case.config, loaded.request, None).unwrap();
    let topology::Started {
        custody: (host, reviewer, predictor, observer),
        source,
        output,
    } = topology::start(
        &case.config,
        &loaded,
        true,
        &mut control,
        &deadline,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    assert!(predictor.is_some() && observer.is_some());
    let prediction = predictor.map(|observer| prediction::Session::new(loaded.request, observer));
    let identity = observer.map(identity_check::Session::new);
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    topology::attach(&mut driver, source).unwrap();
    Live {
        loaded,
        driver,
        reviewer,
        prediction,
        identity,
        wire: ActorWire::new(port),
        control,
        deadline,
        output: output.unwrap(),
        pair,
    }
}

fn source(live: &Live) -> TopologyCheckpoint {
    let host = live.driver.supervisor().host().unwrap();
    live.output.capture(&host, live.driver.topology()).unwrap()
}

fn activation(case: &Case, live: &Live, complementary: bool) -> CredibilityActivation {
    let state = live.driver.supervisor().host().unwrap().inspect();
    campaign::request(
        &case.config,
        state.control.sequence,
        state.control.ledger.epoch,
        complementary,
    )
}

fn qualify(case: &Case, live: &mut Live, request: &CredibilityActivation) -> Result<bool, String> {
    let path = case.root.join("qualification.bin");
    fs::write(&path, request.encode_reference().unwrap()).unwrap();
    qualification::prepare(
        &mut live.driver,
        &case.config,
        Some(&path),
        (&mut live.control, &live.reviewer, &live.deadline),
        &mut || ElapsedTick(1000),
    )
}

fn close(live: Live) {
    assert!(topology::retain(Some(&live.output), &live.driver, None).is_none());
    assert!(custody::retain(&live.loaded, &live.driver, None).is_none());
    assert_eq!(
        checkpoint(&live.pair.source).history_anchor(),
        &custody::load(&live.pair.history).unwrap()
    );
    assert_eq!(cleanup(live.driver, 1000, 1), 0);
    drop(live.prediction);
    drop(live.identity);
    drop(live.wire);
}

#[test]
fn prepared_joint_baseline_survives_real_containment_and_reset_then_qualifies_and_publishes() {
    let root = Root::new();
    let mut case = configured_case(&root);
    let (complete, _, restored) = contained(&case);
    assert_eq!(number(&restored, "sampled_draws"), 0);
    let retained_source = checkpoint(&complete.source);
    select_fresh_identity(&case);
    let mut live = open(&case, &complete);
    let (before, publication, forecast, baseline, policy) = {
        let host = live.driver.supervisor().host().unwrap();
        assert_eq!(
            host.identity_report(901).unwrap().outcome,
            IdentityOutcome::Matched
        );
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        (
            host.learned_generation_inspection().unwrap(),
            host.inspect(),
            host.learned_action_consistency_snapshot().unwrap(),
            case.config.profile.delivery.congress.clone(),
            host.held_out_joint_policy().unwrap().unwrap(),
        )
    };
    assert_eq!(
        policy,
        HeldOutJointPolicy::new(
            71,
            1,
            1,
            2,
            0,
            0,
            HeldOutJointBudget {
                cases: 3,
                member_outcomes: 12
            }
        )
        .unwrap()
    );
    assert!(publication.control.sequence >= 2);
    assert_eq!(before.numerical.position, 2);
    assert_eq!(before.numerical.sampled_draws, 0);
    let source_before = source(&live);
    assert_eq!(source_before.work(), retained_source.work());
    let bad = activation(&case, &live, true);
    let marginal = baseline
        .promote_credibility(
            bad.snapshot.clone(),
            &bad.requirements,
            &bad.binding,
            bad.expected_control_sequence + 1,
        )
        .unwrap();
    assert!(baseline.members.values().all(|member| member.weight == 5));
    assert!(
        marginal
            .admitted_members()
            .values()
            .all(|member| member.weight == 4)
    );
    let mut candidate = baseline.clone();
    candidate.generation = bad.binding.reducer_generation;
    candidate.members = marginal.admitted_members().clone();
    let regression = evaluate(policy, &baseline, &candidate, marginal.snapshot()).unwrap();
    assert!(!regression.qualified());
    assert!(regression.cases[&2].regressed && regression.cases[&3].regressed);
    assert_eq!(regression.strata["publication"].baseline.escaped_roots, 0);
    assert_eq!(regression.strata["publication"].candidate.escaped_roots, 2);
    let canonical = case.disk();
    let error = qualify(&case, &mut live, &bad).unwrap_err();
    assert!(error.contains("Incomplete"), "{error}");
    assert_eq!(case.disk(), canonical);
    assert_eq!(source(&live), source_before);
    {
        let host = live.driver.supervisor().host().unwrap();
        assert_eq!(host.inspect(), publication);
        assert_eq!(host.learned_generation_inspection().unwrap(), before);
        assert_eq!(
            host.learned_action_consistency_snapshot().unwrap(),
            forecast
        );
        assert!(host.credibility_change(21).is_err());
        assert!(host.request_status(1).is_err());
        // No activation existed before this attempt: failure has not withdrawn
        // a previously qualified reducer or created mandatory promotion.
        host.check_credibility().unwrap();
    }
    let good = activation(&case, &live, false);
    assert_eq!(
        good.expected_control_sequence,
        bad.expected_control_sequence
    );
    assert_eq!(good.expected_epoch, bad.expected_epoch);
    assert!(qualify(&case, &mut live, &good).unwrap());
    {
        let host = live.driver.supervisor().host().unwrap();
        let report = host.held_out_joint_report(21).unwrap().unwrap();
        assert!(report.qualified());
        assert_eq!(
            report.work,
            HeldOutJointBudget {
                cases: 3,
                member_outcomes: 12
            }
        );
        assert_eq!(report.strata["publication"].candidate.escaped_roots, 0);
        assert_eq!(
            report.strata["publication"].candidate.false_stopped_roots,
            0
        );
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            before.numerical
        );
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        host.check_credibility().unwrap();
    }
    assert_eq!(source(&live).work(), source_before.work());
    topology::certify(&mut live.driver, &live.deadline, &mut || ElapsedTick(1000)).unwrap();
    assert!(
        identity_check::prepare(
            &mut live.driver,
            &mut case.config,
            &live.loaded,
            &mut live.identity,
            (&mut live.control, &live.reviewer, &live.deadline),
            &mut || ElapsedTick(1000)
        )
        .unwrap()
    );
    assert_eq!(
        live.driver
            .supervisor()
            .host()
            .unwrap()
            .identity_report(902)
            .unwrap()
            .outcome,
        IdentityOutcome::Matched
    );
    assert!(
        generate_selected(
            &mut live.driver,
            &live.reviewer,
            &mut case.config,
            &mut live.control,
            &live.deadline,
            &mut || ElapsedTick(1000),
            live.prediction.as_mut()
        )
        .unwrap()
    );
    submit(
        &mut live.driver,
        &mut live.wire,
        &mut case.config,
        &live.loaded,
        &live.deadline,
        &mut || ElapsedTick(1000),
    )
    .unwrap();
    assert!(
        live.driver
            .request_learned_human_approval_from_policy_file(
                &mut case.config.source,
                701,
                ElapsedTick(50_000),
                || ElapsedTick(1000)
            )
            .result
            .is_err()
    );
    let (driver, history) = native::review_current(
        &mut case.config,
        &live.loaded,
        &live.reviewer,
        live.driver,
        &live.deadline,
    );
    live.driver = driver;
    assert!(matches!(
        live.driver
            .step_computed_from_policy_file(&mut case.config.source, || ElapsedTick(1000), None)
            .result
            .unwrap(),
        FileDriverEvent::AwaitingHuman { request: 1 }
    ));
    let human = native::approve(&mut live.driver, &mut case.config, &live.reviewer, 1);
    native::publish(&mut live.driver, &mut case.config, &human);
    assert!(executed(&mut live.wire, 1));
    {
        let host = live.driver.supervisor().host().unwrap();
        assert_eq!(
            host.stream_snapshot()
                .unwrap()
                .confirmed
                .messages()
                .collect::<Vec<_>>(),
            ["A"]
        );
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(
            host.inspect().control.ledger.charged,
            host.request_action(1).unwrap().spec().units
        );
        assert_eq!(host.inspect().control.ledger.reserved, 0);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 4);
        assert_eq!(numerical.sampled_draws, 2);
        assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
        assert_eq!(numerical.cumulative_work.admitted_tokens, 4);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        let consistency = host.learned_action_consistency_snapshot().unwrap();
        assert_eq!(consistency.consistency.evidence.samples(), 1);
        assert!(consistency.work.probe_coordinates > 0);
    }
    let after = source(&live);
    assert_eq!(after.expected().graph().spec().generation, 2);
    assert!(after.work().checks > retained_source.work().checks);
    assert!(after.last_operation() > retained_source.last_operation());
    close(live);
    drop(history);
}

#[test]
fn qualified_prepared_reset_still_needs_a_current_cut_and_independent_identity_measurement() {
    for missing_identity in [false, true] {
        let root = Root::new();
        let mut case = configured_case(&root);
        let (complete, _, _) = contained(&case);
        select_fresh_identity(&case);
        let mut live = open(&case, &complete);
        let before = live
            .driver
            .supervisor()
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap();
        let request = activation(&case, &live, false);
        assert!(qualify(&case, &mut live, &request).unwrap());
        if missing_identity {
            topology::certify(&mut live.driver, &live.deadline, &mut || ElapsedTick(1000)).unwrap();
            fs::remove_file(case.root.join("observed.manifest")).unwrap();
            assert!(
                identity_check::prepare(
                    &mut live.driver,
                    &mut case.config,
                    &live.loaded,
                    &mut live.identity,
                    (&mut live.control, &live.reviewer, &live.deadline),
                    &mut || ElapsedTick(1000)
                )
                .is_err()
            );
        } else {
            fs::write(case.root.join("live-2.json"), fixture::graph(2, true)).unwrap();
            assert!(
                topology::certify(&mut live.driver, &live.deadline, &mut || ElapsedTick(1000))
                    .is_err()
            );
        }
        {
            let host = live.driver.supervisor().host().unwrap();
            assert!(host.held_out_joint_report(21).unwrap().unwrap().qualified());
            assert_eq!(
                host.identity_report(901).unwrap().outcome,
                IdentityOutcome::Matched
            );
            assert!(host.identity_report(902).is_err());
            assert_eq!(
                host.learned_generation_inspection().unwrap().numerical,
                before.numerical
            );
            assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.inspect().control.ledger.charged, 0);
            assert_eq!(host.inspect().control.ledger.reserved, 0);
            assert!(host.request_status(1).is_err());
            assert_eq!(
                host.action_consistency_snapshot()
                    .unwrap()
                    .evidence
                    .samples(),
                0
            );
        }
        stop(&mut live.driver, 1, &mut || ElapsedTick(1000)).unwrap();
        close(live);
    }
}

#[test]
fn prepared_joint_requalification_keeps_the_original_case_and_member_work_limits() {
    for (field, old, limit) in [("max_cases", 3, 2), ("max_member_outcomes", 12, 11)] {
        let root = Root::new();
        let case = configured_case(&root);
        let original = fs::read_to_string(&case.recipe).unwrap();
        fs::write(
            &case.recipe,
            original.replacen(
                &format!("\"{field}\":{old}"),
                &format!("\"{field}\":{limit}"),
                1,
            ),
        )
        .unwrap();
        let (complete, _, _) = contained(&case);
        select_fresh_identity(&case);
        let mut live = open(&case, &complete);
        let request = activation(&case, &live, false);
        let source_before = source(&live);
        let (before, publication) = {
            let host = live.driver.supervisor().host().unwrap();
            (
                host.learned_generation_inspection().unwrap(),
                host.inspect(),
            )
        };
        let canonical = case.disk();
        let error = qualify(&case, &mut live, &request).unwrap_err();
        assert!(error.contains("Limit"), "{field}: {error}");
        assert_eq!(case.disk(), canonical);
        assert_eq!(source(&live), source_before);
        {
            let host = live.driver.supervisor().host().unwrap();
            assert_eq!(host.inspect(), publication);
            assert_eq!(host.learned_generation_inspection().unwrap(), before);
            assert!(host.credibility_change(21).is_err());
            assert_eq!(host.inspect().executions, 0);
            assert_eq!(
                host.action_consistency_snapshot()
                    .unwrap()
                    .evidence
                    .samples(),
                0
            );
            host.check_credibility().unwrap();
        }
        close(live);
    }
}
