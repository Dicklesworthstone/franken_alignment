//! Original learned generation, native helper inference and human publication
//! under an immutable joint held-out policy. Synthetic labels test enforcement,
//! not deployment calibration, evaluator authentication or population risk.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::held_out_joint::{
    HeldOutJointBudget, HeldOutJointPolicy,
};
use fa_reference::action::consequence::gate::containment::session::policy::controller::credibility::joint::evaluate;

#[path = "joint/recipe.rs"] mod recipe_tests;
#[path = "joint/recovery.rs"] mod recovery_tests;

const POLICY: &str = r#"{"id":71,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":2,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":3,"max_member_outcomes":12}"#;

fn policy() -> HeldOutJointPolicy {
    HeldOutJointPolicy::new(71, 1, 1, 2, 0, 0,
        HeldOutJointBudget { cases: 3, member_outcomes: 12 }).unwrap()
}
fn envelope(recipe: &str) -> String {
    format!(r#"{{"schema":"fa.learned-joint-publication/1","joint":{POLICY},"recipe":{recipe}}}"#)
}
fn wrap(path: &Path) {
    fs::write(path, envelope(&fs::read_to_string(path).unwrap())).unwrap();
}

fn campaign(config: &Config, sequence: u64, epoch: u64, operation: u64,
    generation: u64, complementary: bool) -> CredibilityActivation {
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
        cases: (1..=3).map(|id| CaseSpec { id, stratum: "publication".into(),
            evidence_root: [id as u8; 32], dispatch_sequence: frontier }).collect(),
    }).unwrap();
    for id in 1..=3 {
        ledger.record_observations(id, helpers.keys().map(|name| {
            // Each helper catches a different violation. Promotion's capped
            // 5 -> 4 weights erase both formerly decisive complementary vetoes.
            let clear = id == 1 || complementary
                && ((id == 2 && name == "beta") || (id == 3 && name == "alpha"));
            (name.clone(), if clear { Observation::Clear }
                else { Observation::Hold { first_sequence: frontier - 1 } })
        }).collect()).unwrap();
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
            minimum_safe_cases: 1, minimum_violation_cases: 2,
            minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 500_000,
            maximum_false_positive_ppm: 0, base_weight: 8, lead_bonus_weight: 0,
            lead_saturation_sequences: 0, maximum_evidence_age: 100,
            maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}

fn seeded(root: &Root, mode: Option<bool>) -> Seed {
    seed_with(root, mode, |config, path| {
        // Frozen before creation; never lowered to rescue the bad campaign.
        config.profile.delivery.congress.continue_minimum = 4;
        config.profile.delivery.congress.continue_hold_maximum = 4;
        wrap(path);
    }, |config, sequence, epoch, operation, generation|
        campaign(config, sequence, epoch, operation, generation, false))
}

#[test]
fn joint_learned_requalification_blocks_complementary_veto_regression_then_publishes_with_both_keys() {
    for mode in [None, Some(false), Some(true)] {
        let root = Root::new(); let mut seed = seeded(&root, mode); let mut live = open(&seed);
        let deadline = deadline(); let mut control = Control::new(&seed.config, 2, None).unwrap();
        let (before, original, forecast, cost, baseline) = {
            let host = live.driver.supervisor().host().unwrap();
            assert_eq!(host.held_out_joint_policy().unwrap(), Some(policy()));
            let old = host.held_out_joint_report(20).unwrap().unwrap();
            assert!(old.qualified()); assert_eq!(old.policy, policy());
            (host.learned_generation_inspection().unwrap(), host.inspect(),
                consistency(&host), owned_cost(&host), old.candidate.clone())
        };
        let request = campaign(&seed.config, original.control.sequence,
            original.control.ledger.epoch, 21, 3, true);
        let marginal = baseline.promote_credibility(request.snapshot.clone(),
            &request.requirements, &request.binding, request.expected_control_sequence + 1).unwrap();
        assert!(baseline.members.values().all(|member| member.weight == 5));
        assert!(marginal.admitted_members().values().all(|member| member.weight == 4));
        let mut candidate = baseline.clone();
        candidate.generation = request.binding.reducer_generation;
        candidate.members = marginal.admitted_members().clone();
        let joint = evaluate(policy(), &baseline, &candidate, marginal.snapshot()).unwrap();
        assert!(!joint.qualified()); assert!(joint.cases[&2].regressed && joint.cases[&3].regressed);
        assert_eq!(joint.strata["publication"].baseline.escaped_roots, 0);
        assert_eq!(joint.strata["publication"].candidate.escaped_roots, 2);
        let bad = save(&root, &request);
        let journal = fs::read(seed.config.store.join("delivery.bin")).unwrap();
        let error = qualification::prepare(&mut live.driver, &seed.config, Some(&bad),
            (&mut control, &live.reviewer, &deadline), &mut || ElapsedTick(1000)).unwrap_err();
        assert!(error.contains("Incomplete"), "{error}");
        assert_eq!(fs::read(seed.config.store.join("delivery.bin")).unwrap(), journal);
        {
            let host = live.driver.supervisor().host().unwrap();
            same_numerical(&host, &before); assert_eq!(host.inspect(), original);
            assert_eq!(consistency(&host), forecast); assert_eq!(owned_cost(&host), cost);
            assert!(host.credibility_change(21).is_err());
            assert!(host.check_credibility().is_err()); assert!(host.request_status(2).is_err());
        }
        // Same policy, identities, requirements and original predecessor. Only
        // independently supplied retained helper observations change. The
        // failed operation has not been consumed and no token was regenerated.
        let good = campaign(&seed.config, original.control.sequence,
            original.control.ledger.epoch, 21, 3, false);
        let path = save(&root, &good);
        assert!(qualification::prepare(&mut live.driver, &seed.config, Some(&path),
            (&mut control, &live.reviewer, &deadline), &mut || ElapsedTick(1000)).unwrap());
        {
            let host = live.driver.supervisor().host().unwrap();
            host.check_credibility().unwrap(); same_numerical(&host, &before);
            assert_eq!(consistency(&host), forecast); assert_eq!(owned_cost(&host), cost);
            let report = host.held_out_joint_report(21).unwrap().unwrap();
            assert!(report.qualified()); assert_eq!(report.policy, policy());
            assert_eq!(report.work, HeldOutJointBudget { cases: 3, member_outcomes: 12 });
            assert_eq!(report.strata["publication"].candidate.escaped_roots, 0);
            assert_eq!(report.strata["publication"].candidate.false_stopped_roots, 0);
            assert_eq!(host.inspect().executions, 1);
            assert_eq!(host.inspect().control.ledger.charged, original.control.ledger.charged);
            assert!(!host.clock_ready());
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
            let charge = host.request_action(1).unwrap().spec().units + host.request_action(2).unwrap().spec().units;
            assert_eq!(stream.publication.control.ledger.charged, charge);
            assert_eq!(stream.publication.control.ledger.reserved, 0);
            if mode.is_some() {
                let forecast = host.action_consistency_snapshot().unwrap();
                assert_eq!(forecast.evidence.samples(), 2); assert!(!forecast.coverage_lost);
                assert_eq!(forecast.pending_attempt, None);
            }
        }
        assert_eq!(cleanup(live.driver, 1000, 1), 0); drop(history);
    }
}
