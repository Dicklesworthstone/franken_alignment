//! Original learned startup with an independent evaluator and native accounting.
use super::*;
use fa_reference::action::consequence::delivery::persistent::RecoveryReserve;
use fa_reference::action::consequence::delivery::persistent::observed::{
    credibility::andon::FileAndonConfig,
    guarded::evaluation::FileEvaluatedOversightRoles,
};
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};

fn accounting() -> FileAndonConfig {
    FileAndonConfig::new(
        LedgerIdentity {
            deployment: 51,
            evaluator: 52,
        },
        vec![AndonBudget {
            window: 1,
            start: 0,
            end: 100,
            false_stops_allowed: 1,
            cost_per_stop: 7,
        }],
    )
    .unwrap()
}

fn accounted(
    root: &Directory,
    config: FileLearnedConfig,
    guards: &FileGuardSet,
) -> (FileOversight, FileEvaluatedOversightRoles) {
    FileOversight::create_andon_guarded_with_learned_generation_reserved(
        root.store(),
        profile(),
        guards,
        None,
        protocol(),
        &accounting(),
        config,
        RecoveryReserve::terminal(),
    )
    .unwrap()
}

#[test]
fn complete_first_image_retains_independent_roles_accounting_and_terminal_capacity() {
    for mask in 0..4 {
        let root = Directory::new();
        let mut selected = guards();
        if mask & 1 == 0 {
            selected.identity = None;
        }
        if mask & 2 == 0 {
            selected.campaigns = None;
        }
        let (host, roles) = accounted(&root, config(0), &selected);
        assert_eq!(
            host.revision(),
            5 + u64::from(mask & 1 != 0) + u64::from(mask & 2 != 0)
        );
        assert!(host.publication_guard_required() && host.learned_generation_required());
        assert_eq!(roles.oversight.identity_observer.is_some(), mask & 1 != 0);
        assert_eq!(roles.oversight.policy_governor.is_some(), mask & 2 != 0);
        assert_eq!(host.andon_config().unwrap(), Some(&accounting()));
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
        assert_eq!(host.credibility_report().unwrap().protocol, protocol());
        assert_eq!(host.credibility_report().unwrap().retained_cases, 0);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 0);
        assert_eq!(numerical.work.admitted_tokens, 0);
        let capacity = host.journal_capacity().unwrap();
        assert_eq!(capacity.reserve(), Some(RecoveryReserve::terminal()));
        assert!(capacity.terminal_space_remaining());
        assert!(!host.clock_ready());
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.available, 100);
    }
}

#[test]
fn complete_bootstrap_cannot_consume_the_terminal_event_tail() {
    let root = Directory::new();
    let (host, _) = accounted(&root, config(0), &guards());
    let complete = usize::try_from(host.revision()).unwrap();
    drop(host);

    let mut too_small = profile();
    too_small.delivery.limits.events = complete + RecoveryReserve::terminal().events - 1;
    let refused = Directory::new();
    assert_eq!(
        FileOversight::create_andon_guarded_with_learned_generation_reserved(
            refused.store(),
            too_small,
            &guards(),
            None,
            protocol(),
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
    let admitted = Directory::new();
    let (host, _) = FileOversight::create_andon_guarded_with_learned_generation_reserved(
        admitted.store(),
        exact,
        &guards(),
        None,
        protocol(),
        &accounting(),
        config(0),
        RecoveryReserve::terminal(),
    )
    .unwrap();
    let capacity = host.journal_capacity().unwrap();
    assert_eq!(capacity.ordinary_remaining().events, 0);
    assert_eq!(capacity.remaining().events, RecoveryReserve::terminal().events);
    assert!(capacity.terminal_space_remaining());
}

#[test]
fn malformed_reserve_protocol_and_role_bindings_leave_no_partial_store() {
    for field in 0..6 {
        let root = Directory::new();
        let mut selected = profile();
        let mut declared = guards();
        let mut evaluation = protocol();
        let mut reserve = RecoveryReserve::terminal();
        match field {
            0 => reserve.events -= 1,
            1 => reserve.bytes -= 1,
            2 => evaluation.recall_floor.denominator = 0,
            3 => declared.identity.as_mut().unwrap().policy.observer_id = 0,
            4 => selected.delivery.scope.tenant += 1,
            _ => {
                let mut actor = selected.delivery.actor.profile();
                actor.model_generation += 1;
                selected.delivery.actor =
                    ActorState::new(actor, Vec::new(), vec![0], vec![0], 0).unwrap();
            }
        }
        assert!(
            FileOversight::create_andon_guarded_with_learned_generation_reserved(
                root.store(),
                selected,
                &declared,
                None,
                evaluation,
                &accounting(),
                config(0),
                reserve,
            )
            .is_err()
        );
        assert!(!root.store().exists(), "field {field}");
    }
    let root = Directory::new();
    let (host, _) = accounted(&root, config(0), &guards());
    let bytes = root.bytes();
    assert!(
        FileOversight::create_andon_guarded_with_learned_generation_reserved(
            root.store(),
            profile(),
            &guards(),
            None,
            protocol(),
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
fn recipe_owned_source_is_exact_and_installed_once_before_accounting() {
    use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
    use fa_reference::action::consequence::oversight::policy_state::{
        StateFreshness, StateLimits, StateSource,
    };
    let source = FileSourcePolicy {
        source: StateSource {
            scope: profile().delivery.scope,
            source: 42,
            generation: 1,
        },
        limits: StateLimits::default(),
        freshness: StateFreshness::new(5).unwrap(),
    };
    let model = text_fixture::model(&[u32::from(b'a'), text_fixture::END]);
    let config = FileLearnedConfig::new_text(
        model.clone(),
        text_fixture::tokenizer(&model),
        text_fixture::config(&model),
        LearnedDecoderBindingLimits::default(),
    )
    .unwrap()
    .with_required_sidecar()
    .unwrap()
    .with_required_policy_source(source)
    .unwrap();
    let mut declared = guards();
    declared.identity = None;
    declared.campaigns = None;
    declared.source = Some(source);
    for field in 0..3 {
        let root = Directory::new();
        let mut wrong = declared.clone();
        match field {
            0 => wrong.source = None,
            1 => wrong.source.as_mut().unwrap().source.source += 1,
            _ => wrong.source.as_mut().unwrap().freshness = StateFreshness::new(6).unwrap(),
        }
        assert_eq!(
            FileOversight::create_andon_guarded_with_learned_generation_reserved(
                root.store(),
                profile(),
                &wrong,
                None,
                protocol(),
                &accounting(),
                config.clone(),
                RecoveryReserve::terminal(),
            )
            .err(),
            Some(Error::Binding.into())
        );
        assert!(!root.store().exists());
    }
    let root = Directory::new();
    let (host, _) = accounted(&root, config, &declared);
    assert_eq!(host.revision(), 5);
    assert!(host.policy_only_file_source_required() && host.learned_sidecar_required());
    assert_eq!(host.file_source_status().unwrap().policy, source);
    assert!(host.file_source_status().unwrap().producer.is_none());
    assert_eq!(host.andon_config().unwrap(), Some(&accounting()));
    assert!(!host.clock_ready());
}

#[test]
fn only_original_independent_benign_labels_charge_a_held_learned_case() {
    let root = Directory::new();
    let (mut host, roles) = accounted(&root, config(0), &guards());
    let foreign_root = Directory::new();
    let (_foreign, foreign_roles) = accounted(&foreign_root, config(0), &guards());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    fresh_identity(&mut host, &roles.oversight, 1, 1);
    let (_, input) = staged(&mut host, 1);
    reviewed(&mut host, 1, 101, &input, Verdict::Hold).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let before = root.bytes();
    let revision = host.revision();
    let ticket = host.evaluation_ticket(101).unwrap();
    let assessment = Assessment {
        origin: 201,
        evidence_id: [19; 32],
        truth: GroundTruth::Censored,
    };
    assert_eq!(
        foreign_roles
            .evaluator
            .assess(&mut host, revision, &ticket, assessment.clone())
            .err(),
        Some(Error::Binding.into())
    );
    assert_eq!(root.bytes(), before);
    assert!(
        roles
            .evaluator
            .assess(&mut host, revision, &ticket, assessment)
            .unwrap()
    );
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    assert!(host.andon_ledger().unwrap().windows()[&1].incident.is_none());
    let ticket = host.evaluation_ticket(101).unwrap();
    let benign = Assessment {
        origin: 201,
        evidence_id: [20; 32],
        truth: GroundTruth::Benign,
    };
    let revision = host.revision();
    assert!(
        roles
            .evaluator
            .assess(&mut host, revision, &ticket, benign.clone())
            .unwrap()
    );
    let window = &host.andon_ledger().unwrap().windows()[&1];
    assert_eq!(window.false_stops, 1);
    assert_eq!(window.total_cost, 7);
    assert_eq!(window.incident.as_ref().unwrap().opened_by_round, 101);
    assert_eq!(window.incident.as_ref().unwrap().rounds, BTreeSet::from([101]));
    let before_retry = root.bytes();
    let ticket = host.evaluation_ticket(101).unwrap();
    let revision = host.revision();
    assert!(
        !roles
            .evaluator
            .assess(&mut host, revision, &ticket, benign)
            .unwrap()
    );
    assert_eq!(root.bytes(), before_retry);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0);
    assert!(
        host.authorize(host.revision(), 1, &input, snapshot())
            .is_err()
    );
}

#[test]
fn accounting_does_not_replace_fresh_generation_or_the_independent_human_key() {
    let root = Directory::new();
    let (mut host, roles) = accounted(&root, config(0), &guards());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(host.propose(host.revision(), 99, spec(&host), snapshot()).is_err());
    step(&mut host).unwrap();
    fresh_identity(&mut host, &roles.oversight, 1, 1);
    let (action, input, automatic, request) = prepared(&mut host, 1, 101);
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
                evidence_id: [19; 32],
                truth: GroundTruth::Benign,
            },
        )
        .unwrap();
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
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.charged, 16);
}
