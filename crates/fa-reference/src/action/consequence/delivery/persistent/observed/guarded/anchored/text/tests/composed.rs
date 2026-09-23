//! Original higher-guard roles and validators composed with real text recovery.
use super::*;
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
use crate::action::consequence::delivery::persistent::observed::consistency::{
    FileConsistencyConfig, FileConsistencyParameters,
};
use crate::action::consequence::delivery::persistent::observed::guarded::{
    evaluation::FileEvaluatedOversightRoles,
    predictive::{FilePredictiveRequirements, FilePredictiveRoles},
    mediated::{FileMediatedRequirements, FileMediatedRoles, FileTopologyRequirement},
};
use crate::action::consequence::delivery::persistent::observed::mediation::{FileMediationObserver, FileMediationUpdate};
use crate::action::consequence::mediation::{
    AuthorityGraph, Channel, Completeness, CutCheck, Edge, Enforcer, GraphSpec, Node, NodeKind,
    MAX_CHECK_EDGE_VISITS,
};
use crate::action::consequence::oversight::credibility::{EvaluationProtocol, Fraction};
use crate::perimeter::*;
use std::rc::Rc;

fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 2, period: 3,
        minimum_violation_origins: 1, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
fn capture() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 }
}
fn prediction(age: u64) -> FileConsistencyConfig {
    let pair = BinaryForecast::new(16384, 49152).unwrap();
    FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 1, probe_generation: 1,
        profile: capture(), weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 19, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 16,
        max_prediction_age_ticks: age }).unwrap()
}
fn graph(generation: u64) -> AuthorityGraph {
    let p = host_profile().delivery;
    AuthorityGraph::new(GraphSpec { generation, inventory_generation: generation, scope: p.scope, target: p.target,
        family: EffectFamilyRecord { scope: PerimeterScope { tenant: 1, principal: 2, purpose: 1 },
            family: "publication".into(), trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publication".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared topology only".into()] },
        completeness: Completeness::DeclaredComplete { inventory_generation: generation },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer }, Node { id: 3, kind: NodeKind::Sink }],
        edges: vec![Edge { id: 1, from: 1, to: 2, channel: Channel::Dispatch, route: "publication".into(), provenance: 1 },
            Edge { id: 2, from: 2, to: 3, channel: Channel::Dispatch, route: "publication".into(), provenance: 2 }],
        enforcers: vec![Enforcer { node: 2, adapter: p.target.adapter, contract_version: 1, generation: 1, provenance: 9 }],
    }).unwrap()
}
fn certify(host: &mut FileOversight, role: &FileMediationObserver) {
    let proposal = host.mediation_snapshot().unwrap().graph.propose_cut(&[2]).unwrap();
    let revision = host.revision(); let epoch = host.inspect().control.ledger.epoch;
    assert!(matches!(role.certify(host, revision, epoch, &proposal, MAX_CHECK_EDGE_VISITS)
        .unwrap().unwrap(), CutCheck::Verified(_)));
}
fn initialize(host: &mut FileOversight) {
    host.enable_decoder_tokenizer(host.revision(), tokenizer(false)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
}
fn generate(host: &mut FileOversight) {
    let intent = command(host, 7, request(b"ab", 2));
    let receipt = host.generate_decoder_text(host.revision(), intent).unwrap();
    assert_eq!(receipt.result().unwrap().bytes().unwrap(), b"AA");
}
fn preserved(host: &FileOversight, revision: u64) {
    assert_eq!(host.revision(), revision + 1);
    assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
    assert!(host.publication_guard_required()); assert!(host.policy_campaigns_required());
    assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"AA");
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
    assert_eq!(host.inspect().executions, 0);
}
fn base_roles(host: &FileOversight, roles: &FileOversightRoles) {
    assert!(roles.policy_governor.is_some()); assert!(roles.identity_observer.is_none());
    assert!(Rc::ptr_eq(&roles.human.issuer, &host.issuer));
}
fn evaluated_roles(host: &FileOversight, roles: &FileEvaluatedOversightRoles) {
    base_roles(host, &roles.oversight);
    assert!(Rc::ptr_eq(&roles.evaluator.issuer, &host.issuer));
}
fn predictive_roles(host: &FileOversight, roles: &FilePredictiveRoles, evaluated: bool) {
    base_roles(host, &roles.oversight);
    assert!(Rc::ptr_eq(&roles.consistency_observer.issuer, &host.issuer));
    assert_eq!(roles.evaluator.is_some(), evaluated);
    if let Some(role) = &roles.evaluator { assert!(Rc::ptr_eq(&role.issuer, &host.issuer)); }
}
fn mediated_roles(host: &FileOversight, roles: &FileMediatedRoles, predicted: bool, evaluated: bool) {
    base_roles(host, &roles.oversight);
    assert!(Rc::ptr_eq(&roles.topology_observer.issuer, &host.issuer));
    assert_eq!(roles.consistency_observer.is_some(), predicted);
    assert_eq!(roles.evaluator.is_some(), evaluated);
    if let Some(role) = &roles.consistency_observer { assert!(Rc::ptr_eq(&role.issuer, &host.issuer)); }
    if let Some(role) = &roles.evaluator { assert!(Rc::ptr_eq(&role.issuer, &host.issuer)); }
}

#[test]
fn text_anchor_evaluated_recovery_preserves_native_text_and_separate_evaluator_custody() {
    let root = Directory::new(); let c = config(3.0, 65); let g = guards(&c); let p = protocol();
    let (mut host, old) = FileOversight::create_evaluated_guarded(root.store(), host_profile(), &g, None, p.clone()).unwrap();
    initialize(&mut host); generate(&mut host);
    let before = host.credibility_report().unwrap();
    let expected = requirements(&host, g); let anchor = host.history_anchor().unwrap();
    let revision = host.revision(); drop(host);
    let (host, roles) = FileOversight::open_evaluated_guarded_text_anchored(root.store(), host_profile(),
        &expected, &p, &tokenizer(false), &anchor).unwrap();
    preserved(&host, revision); evaluated_roles(&host, &roles);
    assert_eq!(host.credibility_protocol().unwrap(), Some(&p));
    assert_eq!(host.credibility_report().unwrap(), before);
    assert!(!Rc::ptr_eq(&old.evaluator.issuer, &host.issuer));
    assert!(!Rc::ptr_eq(&old.oversight.human.issuer, &host.issuer));
}

#[test]
fn text_anchor_predictive_recovery_retains_unanswered_forecast_and_withdraws_old_observer() {
    for evaluated in [false, true] {
        let root = Directory::new(); let c = config(3.0, 65); let g = guards(&c);
        let prediction = prediction(10); let evaluation = evaluated.then(protocol);
        let (mut host, old) = FileOversight::create_predictive_guarded(root.store(), host_profile(),
            &g, None, prediction.clone(), evaluation.clone()).unwrap();
        initialize(&mut host); generate(&mut host);
        let numerical = host.decoder_inspection().unwrap().numerical;
        // This is the original externally supplied capture profile, explicitly
        // separate from actual hosted residual measurement. Bind its CURRENT
        // native actor revision/position; startup revision zero is now stale.
        let frame = SourceFrame::capture(FrameIdentity { profile: capture(), stream: 17, sequence: 1,
            position: numerical.position.checked_sub(1).unwrap() }, &[-1.0]).unwrap();
        let revision = host.revision();
        old.consistency_observer.forecast_action(&mut host, revision, 1, numerical.actor_revision, &frame).unwrap().unwrap();
        assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        let expected = FilePredictiveRequirements { oversight: requirements(&host, g), prediction, evaluation };
        let anchor = host.history_anchor().unwrap(); let revision = host.revision(); drop(host);
        let (mut host, roles) = FileOversight::open_predictive_guarded_text_anchored(root.store(), host_profile(),
            &expected, &tokenizer(false), &anchor).unwrap();
        preserved(&host, revision); predictive_roles(&host, &roles, evaluated);
        let current = host.action_consistency_snapshot().unwrap();
        assert!(current.coverage_lost); assert_eq!(current.pending_attempt, Some(1));
        assert_eq!(current.evidence, evidence);
        let revision = host.revision(); let original = bytes(&host);
        assert_eq!(old.consistency_observer.unavailable(&mut host, revision), Err(JournalError::Contract(Error::Binding)));
        assert_eq!(bytes(&host), original);
        roles.consistency_observer.unavailable(&mut host, revision).unwrap();
        assert!(host.action_consistency_snapshot().unwrap().coverage_lost);
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
    }
}

#[test]
fn text_anchor_mediated_recovery_returns_exact_roles_but_never_resurrects_a_cut() {
    for predicted in [false, true] { for evaluated in [false, true] {
        let root = Directory::new(); let c = config(3.0, 65); let g = guards(&c);
        let prediction = predicted.then(|| prediction(10)); let evaluation = evaluated.then(protocol);
        let (mut host, old) = FileOversight::create_mediated_guarded(root.store(), host_profile(), &g,
            None, graph(1), prediction.clone(), evaluation.clone()).unwrap();
        initialize(&mut host); certify(&mut host, &old.topology_observer); generate(&mut host);
        assert!(host.mediation_snapshot().unwrap().accepted.is_some());
        let expected = FileMediatedRequirements { oversight: requirements(&host, g),
            topology: FileTopologyRequirement { initial: graph(1), current: graph(1), available: true },
            prediction, evaluation };
        let anchor = host.history_anchor().unwrap(); let revision = host.revision(); drop(host);
        let (mut host, roles) = FileOversight::open_mediated_guarded_text_anchored(root.store(), host_profile(),
            &expected, &tokenizer(false), &anchor).unwrap();
        preserved(&host, revision); mediated_roles(&host, &roles, predicted, evaluated);
        let topology = host.mediation_snapshot().unwrap();
        assert!(!topology.available); assert!(topology.accepted.is_none()); assert_eq!(topology.graph, graph(1));
        let update = FileMediationUpdate { operation: 7, expected_generation: 1,
            expected_authority_epoch: host.inspect().control.ledger.epoch, next: Some(graph(2)) };
        let revision = host.revision(); let original = bytes(&host);
        assert_eq!(old.topology_observer.update(&mut host, revision, &update), Err(JournalError::Contract(Error::Binding)));
        assert_eq!(bytes(&host), original);
        roles.topology_observer.update(&mut host, revision, &update).unwrap();
        certify(&mut host, &roles.topology_observer);
        assert!(host.mediation_snapshot().unwrap().accepted.is_some());
        assert!(host.decoder_inspection().unwrap().paused); // topology is not numerical resume
        assert!(!host.clock_ready()); assert_eq!(host.inspect().executions, 0);
    } }
}

#[test]
fn text_anchor_lower_profile_openers_never_discard_installed_higher_guards() {
    let root = Directory::new(); let c = config(3.0, 65); let g = guards(&c); let p = protocol(); let pred = prediction(10);
    let (mut host, _) = FileOversight::create_mediated_guarded(root.store(), host_profile(), &g,
        None, graph(1), Some(pred.clone()), Some(p.clone())).unwrap();
    initialize(&mut host);
    let expected = requirements(&host, g); let anchor = host.history_anchor().unwrap();
    let original = bytes(&host); drop(host);
    assert!(matches!(reopen(&root, &expected, &anchor), Err(JournalError::Contract(Error::Binding))));
    assert!(matches!(FileOversight::open_evaluated_guarded_text_anchored(root.store(), host_profile(),
        &expected, &p, &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Binding))));
    let predicted = FilePredictiveRequirements { oversight: expected.clone(), prediction: pred.clone(), evaluation: Some(p.clone()) };
    assert!(matches!(FileOversight::open_predictive_guarded_text_anchored(root.store(), host_profile(),
        &predicted, &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), original);
    let full = FileMediatedRequirements { oversight: expected,
        topology: FileTopologyRequirement { initial: graph(1), current: graph(1), available: true },
        prediction: Some(pred), evaluation: Some(p) };
    assert!(FileOversight::open_mediated_guarded_text_anchored(root.store(), host_profile(),
        &full, &tokenizer(false), &anchor).is_ok());
}

#[test]
fn text_anchor_mediated_pins_both_graphs_availability_prediction_and_evaluation() {
    let root = Directory::new(); let c = config(3.0, 65); let g = guards(&c);
    let (mut host, roles) = FileOversight::create_mediated_guarded(root.store(), host_profile(), &g,
        None, graph(1), Some(prediction(10)), Some(protocol())).unwrap();
    initialize(&mut host);
    let update = FileMediationUpdate { operation: 7, expected_generation: 1,
        expected_authority_epoch: host.inspect().control.ledger.epoch, next: Some(graph(2)) };
    let revision = host.revision(); roles.topology_observer.update(&mut host, revision, &update).unwrap();
    certify(&mut host, &roles.topology_observer); generate(&mut host);
    let expected = FileMediatedRequirements { oversight: requirements(&host, g),
        topology: FileTopologyRequirement { initial: graph(1), current: graph(2), available: true },
        prediction: Some(prediction(10)), evaluation: Some(protocol()) };
    let anchor = host.history_anchor().unwrap(); let original = bytes(&host); drop(host);
    for mode in 0..7 {
        let mut wrong = expected.clone();
        match mode {
            0 => wrong.topology.initial = graph(2),
            1 => wrong.topology.current = graph(1),
            2 => wrong.topology.available = false,
            3 => wrong.prediction = None,
            4 => wrong.prediction = Some(prediction(11)),
            5 => wrong.evaluation = None,
            _ => wrong.evaluation.as_mut().unwrap().period += 1,
        }
        assert!(FileOversight::open_mediated_guarded_text_anchored(root.store(), host_profile(),
            &wrong, &tokenizer(false), &anchor).is_err());
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), original);
    }
    let (host, roles) = FileOversight::open_mediated_guarded_text_anchored(root.store(), host_profile(),
        &expected, &tokenizer(false), &anchor).unwrap();
    assert_eq!(host.mediation_snapshot().unwrap().graph, graph(2));
    mediated_roles(&host, &roles, true, true);
}

#[test]
fn text_anchor_evaluation_and_prediction_mismatch_refuse_before_invalid_suffix_replay() {
    for predictive in [false, true] {
        let root = Directory::new(); let c = config(3.0, 65); let g = guards(&c); let p = protocol();
        let mut host = if predictive {
            FileOversight::create_predictive_guarded(root.store(), host_profile(), &g, None,
                prediction(10), Some(p.clone())).unwrap().0
        } else {
            FileOversight::create_evaluated_guarded(root.store(), host_profile(), &g, None, p.clone()).unwrap().0
        };
        initialize(&mut host); generate(&mut host);
        let expected = requirements(&host, g); let anchor = host.history_anchor().unwrap();
        let original = bytes(&host); let mut events = host.events.clone();
        events.push(Event::Core(BaseEvent::Cancel(u64::MAX)));
        let invalid = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
        let path = root.store().join(storage::CANONICAL); drop(host); std::fs::write(&path, &invalid).unwrap();
        if predictive {
            let mut wrong = FilePredictiveRequirements { oversight: expected, prediction: prediction(11), evaluation: Some(p) };
            assert!(matches!(FileOversight::open_predictive_guarded_text_anchored(root.store(), host_profile(),
                &wrong, &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Binding))));
            wrong.prediction = prediction(10);
            assert!(matches!(FileOversight::open_predictive_guarded_text_anchored(root.store(), host_profile(),
                &wrong, &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Missing))));
            assert_eq!(std::fs::read(&path).unwrap(), invalid);
            std::fs::write(&path, &original).unwrap();
            assert!(FileOversight::open_predictive_guarded_text_anchored(root.store(), host_profile(),
                &wrong, &tokenizer(false), &anchor).is_ok());
        } else {
            let mut wrong = p.clone(); wrong.period += 1;
            assert!(matches!(FileOversight::open_evaluated_guarded_text_anchored(root.store(), host_profile(),
                &expected, &wrong, &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Binding))));
            assert!(matches!(FileOversight::open_evaluated_guarded_text_anchored(root.store(), host_profile(),
                &expected, &p, &tokenizer(false), &anchor), Err(JournalError::Contract(Error::Missing))));
            assert_eq!(std::fs::read(&path).unwrap(), invalid);
            std::fs::write(&path, &original).unwrap();
            assert!(FileOversight::open_evaluated_guarded_text_anchored(root.store(), host_profile(),
                &expected, &p, &tokenizer(false), &anchor).is_ok());
        }
    }
}

#[test]
fn text_anchor_higher_profiles_keep_old_or_complete_cuts_at_every_storage_barrier() {
    for kind in 0..3 { for barrier in BARRIERS {
        let root = Directory::new(); let c = config(3.0, 65); let g = guards(&c); let p = protocol();
        let mut host = match kind {
            0 => FileOversight::create_evaluated_guarded(root.store(), host_profile(), &g, None, p.clone()).unwrap().0,
            1 => FileOversight::create_predictive_guarded(root.store(), host_profile(), &g, None, prediction(10), Some(p.clone())).unwrap().0,
            _ => FileOversight::create_mediated_guarded(root.store(), host_profile(), &g, None, graph(1), Some(prediction(10)), Some(p.clone())).unwrap().0,
        };
        initialize(&mut host); generate(&mut host);
        let expected = requirements(&host, g); let anchor = host.history_anchor().unwrap();
        let original = bytes(&host); let revision = host.revision(); drop(host);
        let canonical = tokenizer_bytes(&expected, &tokenizer(false)).unwrap();
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let predictive = FilePredictiveRequirements { oversight: expected.clone(), prediction: prediction(10), evaluation: Some(p.clone()) };
        let mut mediated = FileMediatedRequirements { oversight: expected.clone(),
            topology: FileTopologyRequirement { initial: graph(1), current: graph(1), available: true },
            prediction: Some(prediction(10)), evaluation: Some(p.clone()) };
        let result = match kind {
            0 => FileOversight::open_evaluated_text_store(store, host_profile(), &expected, &p, &canonical, &anchor).map(|_| ()),
            1 => FileOversight::open_predictive_text_store(store, host_profile(), &predictive, &canonical, &anchor).map(|_| ()),
            _ => FileOversight::open_mediated_text_store(store, host_profile(), &mediated, &canonical, &anchor).map(|_| ()),
        };
        let JournalError::Io(failure) = result.unwrap_err() else { panic!("selected native fence barrier"); };
        assert_eq!(failure.operation, barrier);
        let disk = FileOversight::read_publication(root.store(), &host_profile()).unwrap();
        assert_eq!(disk.revision, revision + u64::from(barrier == JournalIo::DirectorySync));
        if barrier != JournalIo::DirectorySync { assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), original); }
        let recovered = match kind {
            0 => {
                let (host, roles) = FileOversight::open_evaluated_guarded_text_anchored(root.store(), host_profile(),
                    &expected, &p, &tokenizer(false), &anchor).unwrap(); evaluated_roles(&host, &roles); host
            }
            1 => {
                let (host, roles) = FileOversight::open_predictive_guarded_text_anchored(root.store(), host_profile(),
                    &predictive, &tokenizer(false), &anchor).unwrap(); predictive_roles(&host, &roles, true); host
            }
            _ => {
                // The requirement describes the ACTUAL pre-fence canonical cut.
                // An unacknowledged but visible fence already withdrew topology.
                mediated.topology.available = barrier != JournalIo::DirectorySync;
                let (host, roles) = FileOversight::open_mediated_guarded_text_anchored(root.store(), host_profile(),
                    &mediated, &tokenizer(false), &anchor).unwrap(); mediated_roles(&host, &roles, true, true); host
            }
        };
        preserved(&recovered, disk.revision);
    } }
}

#[test]
fn text_anchor_higher_profiles_reject_history_rollback_even_when_all_guards_still_match() {
    for kind in 0..3 {
        let root = Directory::new(); let c = config(3.0, 65); let g = guards(&c); let p = protocol();
        let mut host = match kind {
            0 => FileOversight::create_evaluated_guarded(root.store(), host_profile(), &g, None, p.clone()).unwrap().0,
            1 => FileOversight::create_predictive_guarded(root.store(), host_profile(), &g, None, prediction(10), Some(p.clone())).unwrap().0,
            _ => FileOversight::create_mediated_guarded(root.store(), host_profile(), &g, None, graph(1), None, None).unwrap().0,
        };
        initialize(&mut host); let old = bytes(&host); generate(&mut host);
        let mut expected = requirements(&host, g); zero_floor(&mut expected);
        let anchor = host.history_anchor().unwrap(); let latest = bytes(&host); drop(host);
        let path = root.store().join(storage::CANONICAL); std::fs::write(&path, &old).unwrap();
        let predictive = FilePredictiveRequirements { oversight: expected.clone(), prediction: prediction(10), evaluation: Some(p.clone()) };
        let mediated = FileMediatedRequirements { oversight: expected.clone(),
            topology: FileTopologyRequirement { initial: graph(1), current: graph(1), available: true }, prediction: None, evaluation: None };
        let result = match kind {
            0 => FileOversight::open_evaluated_guarded_text_anchored(root.store(), host_profile(), &expected, &p, &tokenizer(false), &anchor).map(|_| ()),
            1 => FileOversight::open_predictive_guarded_text_anchored(root.store(), host_profile(), &predictive, &tokenizer(false), &anchor).map(|_| ()),
            _ => FileOversight::open_mediated_guarded_text_anchored(root.store(), host_profile(), &mediated, &tokenizer(false), &anchor).map(|_| ()),
        };
        assert_eq!(result, Err(JournalError::Contract(Error::Stale)));
        assert_eq!(std::fs::read(&path).unwrap(), old); std::fs::write(&path, latest).unwrap();
        let host = match kind {
            0 => FileOversight::open_evaluated_guarded_text_anchored(root.store(), host_profile(), &expected, &p, &tokenizer(false), &anchor).unwrap().0,
            1 => FileOversight::open_predictive_guarded_text_anchored(root.store(), host_profile(), &predictive, &tokenizer(false), &anchor).unwrap().0,
            _ => FileOversight::open_mediated_guarded_text_anchored(root.store(), host_profile(), &mediated, &tokenizer(false), &anchor).unwrap().0,
        };
        assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"AA");
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
    }
}
