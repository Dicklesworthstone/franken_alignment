//! Learned startup composes the original joint replay and native accounting.
//! Synthetic cases test enforcement and recovery, not detector quality.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::{
    credibility::{
        FileCredibilityUpdate,
        held_out_joint::{HeldOutJointBudget, HeldOutJointPolicy},
    },
    decoder::learned::FileLearnedRecoveryStatus,
    guarded::{
        anchored::FileHistoryAnchor,
        evaluation::andon::{FileAndonRecoveryRequirements, learned::FileAndonLearnedRecovery},
    },
};
use fa_reference::action::consequence::oversight::joint_credibility::{
    JointPromotionPolicy, JointReplayBudget,
};

fn profile() -> FileOversightProfile {
    let mut selected = super::profile();
    // Perfect independent evidence can change the original candidate weights.
    // Keep both reductions in the same Continue/Hold bands for these cases.
    selected.delivery.congress.caps.per_member = 2;
    selected.delivery.congress.caps.per_cohort = 2;
    selected.delivery.congress.narrow_at = 3;
    selected.delivery.congress.suspend_at = 4;
    selected
}

fn fraction(numerator: u64, denominator: u64) -> Fraction {
    Fraction {
        numerator,
        denominator,
    }
}

fn joint(budget: JointReplayBudget) -> JointPromotionPolicy {
    JointPromotionPolicy::new(71, 1, fraction(1, 2), fraction(1, 2), budget).unwrap()
}

fn create(
    root: &Directory,
    recipe: FileLearnedConfig,
    policy: JointPromotionPolicy,
) -> (FileOversight, FileEvaluatedOversightRoles) {
    FileOversight::create_joint_andon_guarded_with_learned_generation_reserved(
        root.store(),
        profile(),
        &guards(),
        None,
        protocol(),
        policy,
        &accounting(),
        recipe,
        RecoveryReserve::terminal(),
    )
    .unwrap()
}

fn retained(host: &FileOversight, policy: JointPromotionPolicy) -> FileAndonRecoveryRequirements {
    FileAndonRecoveryRequirements::new(
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
    .unwrap()
    .require_joint(policy)
    .unwrap()
}

fn begin(
    root: &Directory,
    expected: &FileAndonRecoveryRequirements,
    recipe: &FileLearnedConfig,
    anchor: &FileHistoryAnchor,
) -> Result<FileAndonLearnedRecovery, JournalError> {
    FileOversight::begin_open_andon_guarded_anchored_with_learned_generation(
        root.store(),
        profile(),
        expected,
        &protocol(),
        recipe,
        anchor,
    )
}

fn drive(run: &mut FileAndonLearnedRecovery) {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1).unwrap();
        assert_eq!(after.replayed_events, before.replayed_events + 1);
        assert_eq!(after.total_events, before.total_events);
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
}

fn label(
    host: &mut FileOversight,
    roles: &FileEvaluatedOversightRoles,
    round: u64,
    origin: u64,
    truth: GroundTruth,
) {
    let ticket = host.evaluation_ticket(round).unwrap();
    let revision = host.revision();
    assert!(
        roles
            .evaluator
            .assess(
                host,
                revision,
                &ticket,
                Assessment {
                    origin,
                    evidence_id: [u8::try_from(round).unwrap(); 32],
                    truth,
                },
            )
            .unwrap()
    );
}

fn update(host: &FileOversight) -> FileCredibilityUpdate {
    let control = host.inspect().control;
    FileCredibilityUpdate {
        operation: 41,
        expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch,
        expected_evaluation_revision: host.credibility_report().unwrap().revision,
    }
}

#[test]
fn original_learned_publication_and_joint_replay_survive_exact_guarded_recovery() {
    let root = Directory::new();
    let recipe = config(0);
    let policy = joint(JointReplayBudget::default());
    let (mut host, roles) = create(&root, recipe.clone(), policy);
    assert_eq!(host.revision(), 7);
    assert_eq!(host.joint_credibility_policy().unwrap(), Some(policy));
    assert_eq!(host.credibility_report().unwrap().protocol, protocol());
    assert_eq!(host.andon_config().unwrap(), Some(&accounting()));
    assert_eq!(
        host.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
    assert!(roles.oversight.identity_observer.is_some());
    assert!(roles.oversight.policy_governor.is_some());
    assert_eq!(
        host.learned_generation_inspection()
            .unwrap()
            .numerical
            .work
            .admitted_tokens,
        0
    );
    assert!(!host.clock_ready());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(
        host.propose(host.revision(), 99, spec(&host), snapshot())
            .is_err()
    );
    step(&mut host).unwrap();
    fresh_identity(&mut host, &roles.oversight, 1, 1);
    let (action, input, automatic, request) = prepared(&mut host, 1, 101);
    let pending = update(&host);
    let bytes = root.bytes();
    assert_eq!(
        host.joint_credibility_report(pending.expected_evaluation_revision)
            .err(),
        Some(Error::Incomplete.into())
    );
    assert_eq!(
        host.promote_credibility(host.revision(), &pending).err(),
        Some(Error::Incomplete.into())
    );
    assert_eq!(root.bytes(), bytes);
    label(&mut host, &roles, 101, 201, GroundTruth::Benign);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    assert!(
        host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1))
            .is_err()
    );
    assert_eq!(host.inspect().executions, 0);
    let revision = host.revision();
    let human = roles
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
    assert_eq!(
        host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2))
            .unwrap()
            .outcome,
        EndpointOutcome::Executed {
            resulting_version: 2
        }
    );
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);

    // A second original review supplies the independently labelled violation
    // stratum. The joint result is recomputed from both complete member records.
    let (_, input) = staged(&mut host, 2);
    reviewed(&mut host, 2, 102, &input, Verdict::Hold).unwrap();
    label(&mut host, &roles, 102, 202, GroundTruth::Violation);
    let marginal = host.credibility_report().unwrap();
    assert!(marginal.qualified());
    let before_preview = root.bytes();
    let control = host.inspect().control;
    let report = host.joint_credibility_report(marginal.revision).unwrap();
    assert!(report.qualified());
    assert_eq!(report.policy, policy);
    assert_eq!(report.work.cases, 2);
    assert_eq!(report.work.member_outcomes, 4);
    assert_eq!(
        report.origins.keys().copied().collect::<BTreeSet<_>>(),
        BTreeSet::from([201, 202])
    );
    assert_eq!(root.bytes(), before_preview);
    assert_eq!(host.inspect().control, control);

    let expected = retained(&host, policy);
    let anchor = host.history_anchor().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let accounting = host.andon_ledger().unwrap().clone();
    drop(host);
    let mut recovery = begin(&root, &expected, &recipe, &anchor)
        .unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal())
        .unwrap();
    drive(&mut recovery);
    assert_eq!(root.bytes(), before_preview);
    let (host, fresh) = recovery.finish().unwrap();
    assert_eq!(host.revision(), expected.base().minimum.journal_revision + 1);
    assert_eq!(host.joint_credibility_policy().unwrap(), Some(policy));
    assert_eq!(host.credibility_report().unwrap(), marginal);
    assert_eq!(host.joint_credibility_report(marginal.revision).unwrap(), report);
    assert_eq!(host.andon_ledger().unwrap(), &accounting);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 1);
    fresh.evaluator.check_owner(&host).unwrap();
    assert_eq!(
        roles.evaluator.check_owner(&host).err(),
        Some(Error::Binding.into())
    );
}

#[test]
fn exact_joint_recovery_rejects_every_changed_field_and_charges_the_original_window() {
    let root = Directory::new();
    let recipe = config(0);
    let policy = joint(JointReplayBudget::default());
    let (mut host, old) = create(&root, recipe.clone(), policy);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    fresh_identity(&mut host, &old.oversight, 1, 1);
    let (_, input) = staged(&mut host, 1);
    reviewed(&mut host, 1, 101, &input, Verdict::Hold).unwrap();
    label(&mut host, &old, 101, 201, GroundTruth::Censored);
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
    let old_ticket = host.evaluation_ticket(101).unwrap();
    let expected = retained(&host, policy);
    let windows = host.andon_ledger().unwrap().clone();
    let anchor = host.history_anchor().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let bytes = root.bytes();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged staging").unwrap();

    // Missing means marginal-only, never "accept whatever the journal says".
    for field in 0..9 {
        let mut id = policy.id();
        let mut generation = policy.generation();
        let mut escape = policy.maximum_escape_rate();
        let mut benign = policy.maximum_benign_stop_rate();
        let mut budget = policy.budget();
        match field {
            0 => {}
            1 => id += 1,
            2 => generation += 1,
            3 => escape.numerator = 0,
            4 => escape.denominator += 1,
            5 => benign.numerator = 0,
            6 => benign.denominator += 1,
            7 => budget.cases -= 1,
            _ => budget.member_outcomes -= 1,
        }
        let wrong = FileAndonRecoveryRequirements::new(
            expected.base().clone(),
            accounting(),
            windows.windows().values().map(|window| window.budget).collect(),
            expected.minimum_evaluation_revision(),
        )
        .unwrap();
        let wrong = if field == 0 {
            wrong
        } else {
            wrong
                .require_joint(
                    JointPromotionPolicy::new(id, generation, escape, benign, budget).unwrap(),
                )
                .unwrap()
        };
        assert_eq!(
            begin(&root, &wrong, &recipe, &anchor).err(),
            Some(Error::Binding.into()),
            "joint field {field}"
        );
        assert_eq!(root.bytes(), bytes, "joint field {field}");
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging");
    }

    let mut recovery = begin(&root, &expected, &recipe, &anchor).unwrap();
    drive(&mut recovery);
    assert_eq!(root.bytes(), bytes);
    assert!(pending.exists());
    let (mut host, fresh) = recovery.finish().unwrap();
    assert_eq!(host.revision(), expected.base().minimum.journal_revision + 1);
    assert!(!pending.exists());
    assert_eq!(host.joint_credibility_policy().unwrap(), Some(policy));
    assert_eq!(host.andon_ledger().unwrap(), &windows);
    assert_eq!(
        old.evaluator.check_owner(&host).err(),
        Some(Error::Binding.into())
    );
    let ticket = host.evaluation_ticket(101).unwrap();
    let benign = Assessment {
        origin: 201,
        evidence_id: [9; 32],
        truth: GroundTruth::Benign,
    };
    let revision = host.revision();
    let fenced = root.bytes();
    assert_eq!(
        old.evaluator.assess(&mut host, revision, &ticket, benign).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(
        fresh
            .evaluator
            .assess(&mut host, revision, &old_ticket, benign)
            .err(),
        Some(Error::Binding.into())
    );
    assert!(
        fresh
            .evaluator
            .assess(&mut host, revision, &ticket, benign)
            .is_err()
    );
    assert_eq!(root.bytes(), fenced);

    // A fresh clock is now in window 2; this label still charges window 1,
    // where the original applied empirical stop was observed.
    host.observe_time(host.revision(), ElapsedTick(101)).unwrap();
    let before = host.inspect().control;
    let revision = host.revision();
    assert!(
        fresh
            .evaluator
            .assess(&mut host, revision, &ticket, benign)
            .unwrap()
    );
    let first = &host.andon_ledger().unwrap().windows()[&1];
    assert_eq!(first.false_stops, 1);
    assert_eq!(first.total_cost, 7);
    assert_eq!(first.incident.as_ref().unwrap().opened_by_round, 101);
    assert_eq!(first.incident.as_ref().unwrap().rounds, BTreeSet::from([101]));
    assert_eq!(host.andon_ledger().unwrap().windows()[&2], windows.windows()[&2]);
    assert_eq!(host.inspect().control, before);
    let labelled = root.bytes();
    let revision = host.revision();
    assert!(
        !fresh
            .evaluator
            .assess(&mut host, revision, &ticket, benign)
            .unwrap()
    );
    assert_eq!(root.bytes(), labelled);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn learned_joint_replay_work_exhaustion_never_falls_back_to_marginal_promotion() {
    for budget in [
        JointReplayBudget { cases: 1, member_outcomes: 4 },
        JointReplayBudget { cases: 2, member_outcomes: 3 },
        JointReplayBudget { cases: 2, member_outcomes: 4 },
    ] {
        let root = Directory::new();
        let policy = joint(budget);
        let (mut host, roles) = create(&root, config(0), policy);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        step(&mut host).unwrap();
        fresh_identity(&mut host, &roles.oversight, 1, 1);
        for (attempt, round, verdict, truth) in [
            (1, 101, Verdict::Allow, GroundTruth::Benign),
            (2, 102, Verdict::Hold, GroundTruth::Violation),
        ] {
            let (_, input) = staged(&mut host, attempt);
            reviewed(&mut host, attempt, round, &input, verdict).unwrap();
            label(&mut host, &roles, round, 200 + attempt, truth);
        }
        let request = update(&host);
        assert!(host.credibility_report().unwrap().qualified());
        let bytes = root.bytes();
        let control = host.inspect().control;
        if budget.cases < 2 || budget.member_outcomes < 4 {
            assert_eq!(
                host.joint_credibility_report(request.expected_evaluation_revision).err(),
                Some(Error::Limit.into())
            );
            assert_eq!(
                host.promote_credibility(host.revision(), &request).err(),
                Some(Error::Limit.into())
            );
        } else {
            let report = host
                .joint_credibility_report(request.expected_evaluation_revision)
                .unwrap();
            assert!(report.qualified());
            assert_eq!(report.work, budget);
            assert_eq!(report.policy, policy);
        }
        assert_eq!(root.bytes(), bytes);
        assert_eq!(host.inspect().control, control);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    }
}

#[test]
fn malformed_or_incompatible_joint_bootstrap_never_creates_storage() {
    for field in 0..7 {
        let root = Directory::new();
        let mut selected = profile();
        let mut declared = guards();
        let mut evaluation = protocol();
        let mut reserve = RecoveryReserve::terminal();
        let mut recipe = config(0);
        match field {
            0 => reserve.events -= 1,
            1 => reserve.bytes -= 1,
            2 => evaluation.recall_floor.denominator = 0,
            3 => declared.identity.as_mut().unwrap().policy.observer_id = 0,
            4 => selected.delivery.scope.tenant += 1,
            5 => {
                let mut actor = selected.delivery.actor.profile();
                actor.model_generation += 1;
                selected.delivery.actor =
                    ActorState::new(actor, Vec::new(), vec![0], vec![0], 0).unwrap();
            }
            _ => {
                recipe = recipe
                    .with_required_held_out_joint(
                        HeldOutJointPolicy::new(
                            81, 1, 1, 1, 0, 0, HeldOutJointBudget::default(),
                        )
                        .unwrap(),
                    )
                    .unwrap();
            }
        }
        assert!(
            FileOversight::create_joint_andon_guarded_with_learned_generation_reserved(
                root.store(),
                selected,
                &declared,
                None,
                evaluation,
                joint(JointReplayBudget::default()),
                &accounting(),
                recipe,
                reserve,
            )
            .is_err(),
            "field {field}"
        );
        assert!(!root.store().exists(), "field {field}");
    }

    let root = Directory::new();
    let policy = joint(JointReplayBudget::default());
    let (host, _) = create(&root, config(0), policy);
    let bytes = root.bytes();
    assert!(
        FileOversight::create_joint_andon_guarded_with_learned_generation_reserved(
            root.store(),
            profile(),
            &guards(),
            None,
            protocol(),
            policy,
            &accounting(),
            config(0),
            RecoveryReserve::terminal(),
        )
        .is_err()
    );
    assert_eq!(root.bytes(), bytes);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn complete_joint_bootstrap_preserves_the_terminal_event_reserve_at_the_boundary() {
    let root = Directory::new();
    let policy = joint(JointReplayBudget::default());
    let (host, _) = create(&root, config(0), policy);
    let complete = usize::try_from(host.revision()).unwrap();
    drop(host);
    for extra in [0, 1] {
        let root = Directory::new();
        let mut selected = profile();
        selected.delivery.limits.events =
            complete + RecoveryReserve::terminal().events - 1 + extra;
        let result = FileOversight::create_joint_andon_guarded_with_learned_generation_reserved(
            root.store(),
            selected,
            &guards(),
            None,
            protocol(),
            policy,
            &accounting(),
            config(0),
            RecoveryReserve::terminal(),
        );
        if extra == 0 {
            assert_eq!(result.err(), Some(Error::Limit.into()));
            assert!(!root.store().exists());
        } else {
            let (host, _) = result.unwrap();
            let capacity = host.journal_capacity().unwrap();
            assert_eq!(capacity.ordinary_remaining().events, 0);
            assert_eq!(capacity.remaining().events, RecoveryReserve::terminal().events);
            assert!(capacity.terminal_space_remaining());
            assert_eq!(host.joint_credibility_policy().unwrap(), Some(policy));
        }
    }
}
