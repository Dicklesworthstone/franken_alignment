//! A numerical/evaluation profile cannot silently adopt native accounting.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedRecoveryStatus;

#[test]
fn evaluated_only_open_refuses_accounting_before_replay_even_with_an_exact_anchor() {
    let root = Directory::new();
    let config = config(0);
    let declared = guards();
    let (mut host, roles) = accounted(&root, config.clone(), &declared);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    fresh_identity(&mut host, &roles.oversight, 1, 1);
    let (_, input) = staged(&mut host, 1);
    reviewed(&mut host, 1, 101, &input, Verdict::Hold).unwrap();
    let ticket = host.evaluation_ticket(101).unwrap();
    let revision = host.revision();
    roles
        .evaluator
        .assess(
            &mut host,
            revision,
            &ticket,
            Assessment {
                origin: 201,
                evidence_id: [21; 32],
                truth: GroundTruth::Benign,
            },
        )
        .unwrap();
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
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].total_cost, 7);
    let expected = requirements(&host, declared);
    let anchor = host.history_anchor().unwrap();
    let bytes = root.bytes();
    drop(host);

    assert_eq!(
        FileOversight::begin_open_evaluated_guarded_with_learned_generation(
            root.store(),
            profile(),
            &expected,
            &protocol(),
            &config,
        )
        .err(),
        Some(Error::Binding.into()),
    );
    assert_eq!(root.bytes(), bytes);
    assert_eq!(
        FileOversight::begin_open_evaluated_guarded_anchored_with_learned_generation(
            root.store(),
            profile(),
            &expected,
            &protocol(),
            &config,
            &anchor,
        )
        .err(),
        Some(Error::Binding.into()),
    );
    assert_eq!(root.bytes(), bytes);
    assert_eq!(
        FileOversight::open_evaluated_guarded_with_learned_generation(
            root.store(),
            profile(),
            &expected,
            &protocol(),
            &config,
        )
        .err(),
        Some(Error::Binding.into()),
    );
    assert_eq!(root.bytes(), bytes);
}

#[test]
fn original_evaluated_only_numerical_recovery_still_returns_its_separate_roles() {
    for anchored in [false, true] {
        let root = Directory::new();
        let config = config(0);
        let declared = guards();
        let (mut host, _) = FileOversight::create_evaluated_guarded_with_learned_generation(
            root.store(),
            profile(),
            &declared,
            None,
            protocol(),
            config.clone(),
        )
        .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        step(&mut host).unwrap();
        let expected = requirements(&host, declared);
        let anchor = host.history_anchor().unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert!(numerical.work.admitted_tokens > 0);
        let report = host.credibility_report().unwrap();
        let bytes = root.bytes();
        drop(host);
        let mut recovery = if anchored {
            FileOversight::begin_open_evaluated_guarded_anchored_with_learned_generation(
                root.store(),
                profile(),
                &expected,
                &protocol(),
                &config,
                &anchor,
            )
        } else {
            FileOversight::begin_open_evaluated_guarded_with_learned_generation(
                root.store(),
                profile(),
                &expected,
                &protocol(),
                &config,
            )
        }
        .unwrap();
        assert_eq!(recovery.progress().replayed_events, 0);
        while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
            let completed = recovery.progress().replayed_events;
            recovery.advance(completed, 1).unwrap();
        }
        assert_eq!(recovery.progress().status, FileLearnedRecoveryStatus::Ready);
        assert_eq!(root.bytes(), bytes);
        let (host, roles) = recovery.finish().unwrap();
        assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
        assert_eq!(host.andon_config().unwrap(), None);
        assert_eq!(host.credibility_report().unwrap(), report);
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical
        );
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.clock_ready());
        assert_eq!(host.inspect().executions, 0);
        assert!(roles.oversight.identity_observer.is_some());
        assert!(roles.oversight.policy_governor.is_some());
        roles.evaluator.check_owner(&host).unwrap();
    }
}
