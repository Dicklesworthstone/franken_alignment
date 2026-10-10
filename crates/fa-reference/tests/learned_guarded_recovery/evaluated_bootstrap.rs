//! First-image startup through the original numerical and file-backed APIs.
use super::*;
use fa_reference::action::consequence::oversight::credibility::{
    Assessment, EvaluationProtocol, Fraction, GroundTruth,
};
#[allow(dead_code)]
#[path = "../support/learned_text_model.rs"]
mod text_fixture;

pub(super) fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 31, stratum: 2, period: 3,
        minimum_violation_origins: 1, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 },
        false_stop_budget: 10 }
}

#[test]
fn first_image_contains_exact_evaluator_recipe_and_every_declared_role() {
    for mask in 0..4 {
        let root = Directory::new(); let c = config(0); let mut g = guards();
        if mask & 1 == 0 { g.identity = None; }
        if mask & 2 == 0 { g.campaigns = None; }
        let (host, roles) = FileOversight::create_evaluated_guarded_with_learned_generation(
            root.store(), profile(), &g, None, protocol(), c.clone()).unwrap();
        assert_eq!(host.revision(), 3 + u64::from(mask & 1 != 0) + u64::from(mask & 2 != 0));
        assert!(host.publication_guard_required() && host.learned_generation_required());
        assert_eq!(host.identity_checks_required(), mask & 1 != 0);
        assert_eq!(host.policy_campaigns_required(), mask & 2 != 0);
        assert_eq!(roles.oversight.identity_observer.is_some(), mask & 1 != 0);
        assert_eq!(roles.oversight.policy_governor.is_some(), mask & 2 != 0);
        let report = host.credibility_report().unwrap();
        assert_eq!(report.protocol, protocol()); assert_eq!(report.retained_cases, 0);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 0); assert_eq!(numerical.work.admitted_tokens, 0);
        assert_eq!(host.inspect().executions, 0); assert!(!host.clock_ready());
        let expected = requirements(&host, g); let bytes = root.bytes(); drop(host);
        // Neither the evaluator contract nor the numerical recipe can be
        // omitted when recovering this very first acknowledged image.
        assert!(FileOversight::open_guarded_with_learned_generation(
            root.store(), profile(), &expected, &c).is_err());
        assert!(FileOversight::open_evaluated_guarded(
            root.store(), profile(), &expected, &protocol()).is_err());
        assert_eq!(root.bytes(), bytes);
        let (host, fresh) = FileOversight::open_evaluated_guarded_with_learned_generation(
            root.store(), profile(), &expected, &protocol(), &c).unwrap();
        assert_eq!(host.revision(), expected.minimum.journal_revision + 1);
        assert_eq!(host.credibility_report().unwrap(), report);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(fresh.oversight.identity_observer.is_some(), mask & 1 != 0);
        assert_eq!(fresh.oversight.policy_governor.is_some(), mask & 2 != 0);
    }
}

#[test]
fn invalid_evaluation_and_incompatible_numerical_binding_never_create_storage() {
    let root = Directory::new(); let mut invalid = protocol(); invalid.recall_floor.denominator = 0;
    assert!(FileOversight::create_evaluated_guarded_with_learned_generation(
        root.store(), profile(), &guards(), None, invalid, config(0)).is_err());
    assert!(!root.store().exists());
    for field in 0..2 {
        let root = Directory::new(); let mut p = profile(); let mut g = guards();
        g.identity = None; g.campaigns = None;
        if field == 0 {
            let mut actor = p.delivery.actor.profile(); actor.model_generation += 1;
            p.delivery.actor = ActorState::new(actor, Vec::new(), vec![0], vec![0], 0).unwrap();
        } else { p.delivery.scope.tenant += 1; }
        assert!(FileOversight::create_evaluated_guarded_with_learned_generation(
            root.store(), p, &g, None, protocol(), config(0)).is_err());
        assert!(!root.store().exists());
    }
    let root = Directory::new(); let mut invalid = guards();
    invalid.identity.as_mut().unwrap().policy.observer_id = 0;
    assert!(FileOversight::create_evaluated_guarded_with_learned_generation(
        root.store(), profile(), &invalid, None, protocol(), config(0)).is_err());
    assert!(!root.store().exists());
}

#[test]
fn atomic_startup_requires_real_generation_identity_congress_and_human_before_publication() {
    let root = Directory::new(); let c = config(0);
    let (mut host, roles) = FileOversight::create_evaluated_guarded_with_learned_generation(
        root.store(), profile(), &guards(), None, protocol(), c.clone()).unwrap();
    let bytes = root.bytes();
    assert!(FileOversight::create_evaluated_guarded_with_learned_generation(
        root.store(), profile(), &guards(), None, protocol(), c).is_err());
    assert_eq!(root.bytes(), bytes);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(host.propose(host.revision(), 99, spec(&host), snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
    step(&mut host).unwrap(); fresh_identity(&mut host, &roles.oversight, 1, 1);
    let (action, input, automatic, request) = prepared(&mut host, 1, 101);
    let ticket = host.evaluation_ticket(101).unwrap(); let revision = host.revision();
    assert!(roles.evaluator.assess(&mut host, revision, &ticket, Assessment {
        origin: 201, evidence_id: [19; 32], truth: GroundTruth::Benign,
    }).unwrap());
    assert_eq!(host.credibility_report().unwrap().benign_origins, 1);
    assert_eq!(host.inspect().executions, 0);
    assert!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(1)).is_err());
    let revision = host.revision();
    let human = roles.oversight.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &input, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&input), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().executions, 1); assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn recipe_owned_policy_source_must_match_declaration_and_is_installed_once() {
    use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
    use fa_reference::action::consequence::oversight::policy_state::{
        StateSource, StateLimits, StateFreshness,
    };
    use fa_reference::action::consequence::oversight::evidence_source::{
        EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
    };
    let policy = FileSourcePolicy { source: StateSource {
        scope: profile().delivery.scope, source: 42, generation: 1,
    }, limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap() };
    let model = text_fixture::model(&[u32::from(b'a'), text_fixture::END]);
    let c = FileLearnedConfig::new_text(model.clone(), text_fixture::tokenizer(&model),
        text_fixture::config(&model), LearnedDecoderBindingLimits::default()).unwrap()
        .with_required_sidecar().unwrap().with_required_policy_source(policy).unwrap();
    let mut g = guards(); g.identity = None; g.campaigns = None; g.source = Some(policy);
    for field in 0..3 {
        let root = Directory::new(); let mut wrong = g.clone();
        match field {
            0 => wrong.source = None,
            1 => wrong.source.as_mut().unwrap().source.source += 1,
            _ => wrong.source.as_mut().unwrap().freshness = StateFreshness::new(6).unwrap(),
        }
        assert_eq!(FileOversight::create_evaluated_guarded_with_learned_generation(
            root.store(), profile(), &wrong, None, protocol(), c.clone()).err(), Some(Error::Binding.into()));
        assert!(!root.store().exists());
    }
    let root = Directory::new();
    let (mut host, _) = FileOversight::create_evaluated_guarded_with_learned_generation(
        root.store(), profile(), &g, None, protocol(), c.clone()).unwrap();
    assert_eq!(host.revision(), 3);
    assert!(host.file_source_required() && host.policy_only_file_source_required());
    assert!(host.learned_text_required() && host.learned_sidecar_required());
    assert_eq!(host.file_source_status().unwrap().policy, policy);
    assert!(host.file_source_status().unwrap().producer.is_none());
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 0); assert_eq!(numerical.work.admitted_tokens, 0);
    let observation = EvidenceSnapshot::new(EvidenceIdentity {
        source: 42, generation: 1, scope: profile().delivery.scope,
    }, snapshot(), BTreeMap::from([("reviewer".to_owned(), Vec::new())])).unwrap();
    let path = root.0.join("policy.json"); std::fs::write(&path, observation.encode()).unwrap();
    let mut reader = FileEvidenceSource::new(path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(1)).unwrap();
    assert_eq!(host.file_source_status().unwrap().producer, Some(observation.identity()));
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    let expected = requirements(&host, g); let report = host.credibility_report().unwrap(); drop(host);
    let (host, _) = FileOversight::open_evaluated_guarded_with_learned_generation(
        root.store(), profile(), &expected, &protocol(), &c).unwrap();
    assert!(host.policy_only_file_source_required() && host.learned_sidecar_required());
    assert!(host.learned_generation_inspection().unwrap().paused); assert!(!host.clock_ready());
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(host.inspect().executions, 0);
}
