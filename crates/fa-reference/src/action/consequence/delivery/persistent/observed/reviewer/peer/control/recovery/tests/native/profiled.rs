//! Original complete predictive and mediated wrappers, including retained
//! independent requirements. No weaker recovery cursor substitutes for them.
use super::*;
use crate::action::{ResolvedTarget, Scope};
use crate::action::consequence::activation::{
    consistency::{BinaryForecast, ErrorBudget, ForecastRegistration},
    monitor::learned::LearnedMonitorBudget,
    probe::learned::MAX_CHECKED_KV_BYTES,
    tensor::kv::experiment::KvSide,
};
use crate::action::consequence::delivery::persistent::{
    observed::{consistency::{FileConsistencyConfig, FileConsistencyParameters,
        learned::FileLearnedConsistencyConfig}, guarded::{
        predictive::{FilePredictiveRequirements, owned::FileOwnedPredictiveRequirements},
        mediated::{FileMediatedRequirements, FileTopologyRequirement,
            predictive::{FileMediatedPredictor, FilePredictiveMediatedRequirements}},
    }}, requests::actor::{FileActorSupervisor, LearnedTextProposal},
};
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::action::consequence::mediation::*;
use crate::perimeter::*;
use crate::Snapshot;

#[allow(dead_code)]
#[path = "../../../../../../decoder/learned/config/text/intake/tests/fixture.rs"]
mod text_fixture;

fn graph(scope: Scope, target: ResolvedTarget, generation: u64, bypass: bool) -> AuthorityGraph {
    let mut edges = vec![edge(1, 1, 2), edge(2, 2, 3)];
    if bypass { edges.push(edge(3, 1, 3)); }
    AuthorityGraph::new(GraphSpec { generation, inventory_generation: generation, scope, target,
        family: EffectFamilyRecord {
            scope: PerimeterScope { tenant: scope.tenant, principal: scope.principal, purpose: 1 },
            family: "file-publication".into(),
            trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "file-owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publication".into(), threat: Some(ThreatClass::DirectCredentialOrEgress),
                mediation: Mediation::BrokeredEffects, bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared graph only; no operating-system isolation claim".into()],
        },
        completeness: Completeness::DeclaredComplete { inventory_generation: generation },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer }, Node { id: 3, kind: NodeKind::Sink }],
        edges,
        enforcers: vec![Enforcer { node: 2, adapter: target.adapter, contract_version: target.contract_version,
            generation: target.generation, provenance: 90 }],
    }).unwrap()
}
fn edge(id: u64, from: u64, to: u64) -> Edge {
    Edge { id, from, to, channel: Channel::Dispatch, route: "publication".into(), provenance: id + 100 }
}

fn pinned(owned: bool, altered: bool) -> FileLearnedConfig {
    let (model, _, _) = text_fixture::recipe(); let neutral = BinaryForecast::new(32768, 32768).unwrap();
    let capture = if owned { model.cache_profile().layers()[&1].values().profile() }
        else { model.residual_contract(1).unwrap().profile() };
    let prediction = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 91, probe_generation: 1, profile: capture, weights: vec![1.0, 0.0],
        bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 71, generation: 1, policy_generation: 1,
            event_prefix: b"aa".to_vec(), negative: neutral, at_threshold: neutral, positive: neutral },
        alpha: ErrorBudget::new(1, 2).unwrap(), stream: 21,
        max_predictions: 8, max_prediction_age_ticks: if altered { 11 } else { 10 },
    }).unwrap();
    if owned {
        let prediction = FileLearnedConsistencyConfig::new(prediction, 1, KvSide::Value,
            LearnedMonitorBudget::default(), LearnedMonitorBudget::default(), MAX_CHECKED_KV_BYTES)
            .unwrap().with_owned_generation().unwrap().with_pre_output_forecast().unwrap();
        text_fixture::config().with_required_owned_pre_output_forecast(prediction).unwrap()
    } else {
        text_fixture::config().with_required_pre_output_forecast(prediction
            .with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap()).unwrap()
    }
}
fn prediction(config: &FileLearnedConfig) -> FileMediatedPredictor {
    if let Some(raw) = config.required_pre_output_forecast() {
        FileMediatedPredictor::Raw(raw.clone())
    } else { FileMediatedPredictor::Owned(config.required_owned_pre_output_forecast().unwrap().clone()) }
}
fn predictive_target(root: &Directory, profile: &FileOversightProfile,
    expected: &FileRecoveryRequirements, topology: &FileTopologyRequirement,
    predictor: &FileMediatedPredictor, config: &FileLearnedConfig,
    anchor: &FileHistoryAnchor, mediated: bool) -> Result<FileRecoveryStop, JournalError>
{
    macro_rules! take {
        ($candidate:expr) => {{
            let mut run = ($candidate)?.require_recovery_reserve(RecoveryReserve::terminal())?;
            while run.progress().status == FileLearnedRecoveryStatus::Replaying {
                run.advance(run.progress().replayed_events, 1)?;
            }
            run.into_recovery_stop(701)
        }};
    }
    if mediated {
        let expected = FilePredictiveMediatedRequirements { oversight: expected.clone(),
            topology: topology.clone(), prediction: predictor.clone(), evaluation: None };
        return take!(FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
            root.store(), profile.clone(), &expected, config, anchor));
    }
    match predictor {
        FileMediatedPredictor::Raw(prediction) => {
            let expected = FilePredictiveRequirements { oversight: expected.clone(),
                prediction: prediction.clone(), evaluation: None };
            take!(FileOversight::begin_open_predictive_guarded_anchored_with_learned_generation(
                root.store(), profile.clone(), &expected, config, anchor))
        }
        FileMediatedPredictor::Owned(prediction) => {
            let expected = FileOwnedPredictiveRequirements { oversight: expected.clone(),
                prediction: prediction.clone(), evaluation: None };
            take!(FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
                root.store(), profile.clone(), &expected, config, anchor))
        }
    }
}

#[test]
fn raw_and_owned_predictive_targets_preserve_all_original_guards_with_and_without_topology() {
    for owned in [false, true] { for mediated in [false, true] {
        let root = Directory::new(); let p = text_fixture::profile(); let config = pinned(owned, false);
        let predictor = prediction(&config); let graph = graph(p.delivery.scope, p.delivery.target, 1, false);
        let (mut host, observer) = if mediated {
            let (host, roles) = FileOversight::create_predictive_mediated_guarded_with_learned_generation_reserved(
                root.store(), p.clone(), &guards(), None, graph.clone(), predictor.clone(), None,
                config.clone(), RecoveryReserve::terminal()).unwrap();
            (host, roles.consistency_observer.unwrap())
        } else {
            let (host, roles) = match &predictor {
                FileMediatedPredictor::Raw(raw) => FileOversight::create_predictive_guarded_with_learned_generation_reserved(
                    root.store(), p.clone(), &guards(), None, raw.clone(), None, config.clone(), RecoveryReserve::terminal()),
                FileMediatedPredictor::Owned(owned) => FileOversight::create_predictive_guarded_with_owned_learned_consistency_reserved(
                    root.store(), p.clone(), &guards(), None, owned.clone(), None, config.clone(), RecoveryReserve::terminal()),
            }.unwrap(); (host, roles.consistency_observer)
        };
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); text_fixture::step(&mut host);
        let n = host.learned_generation_inspection().unwrap().numerical;
        let revision = host.revision();
        if owned { observer.forecast_owned_learned_request(&mut host, revision, 71, n.actor_revision).unwrap().unwrap(); }
        else { observer.forecast_hosted_request(&mut host, revision, 71, n.actor_revision).unwrap().unwrap(); }
        let pending_prediction = host.action_consistency_snapshot().unwrap().pending_attempt;
        assert_eq!(pending_prediction, Some(1));
        assert_eq!(host.machine.consistency_request, Some((71, 1)));
        let expected = requirements(&host); let anchor = host.history_anchor().unwrap();
        let before = host.inspect();
        let available = mediated && host.mediation_snapshot().unwrap().available;
        assert_eq!(available, mediated); drop(host);
        fs::write(root.store().join("delivery.pending"), b"preserve until explicit Stop").unwrap();
        let unchanged = files(&root);
        let topology = FileTopologyRequirement { initial: graph.clone(), current: graph, available };
        let wrong_prediction = prediction(&pinned(owned, true));
        assert!(predictive_target(&root, &p, &expected, &topology, &wrong_prediction,
            &config, &anchor, mediated).is_err());
        let mut wrong = expected.clone(); wrong.minimum.authority_epoch += 1;
        assert!(predictive_target(&root, &p, &wrong, &topology, &predictor, &config, &anchor, mediated).is_err());
        if mediated {
            let mut wrong = topology.clone(); wrong.available = !topology.available;
            assert!(predictive_target(&root, &p, &expected, &wrong, &predictor, &config, &anchor, true).is_err());
        }
        assert_eq!(files(&root), unchanged);
        let target = predictive_target(&root, &p, &expected, &topology, &predictor, &config, &anchor, mediated).unwrap();
        assert_eq!(files(&root), unchanged);
        let (mut server, mut client) = connect(target); decide(&mut server, &mut client);
        client.request_stop().unwrap(); finish(&mut server, &mut client);
        let application = server.into_application().unwrap(); let history = application.result.unwrap();
        assert_eq!(history.appended_events(), 2); assert_eq!(history.snapshot().revision, before.revision + 2);
        assert_eq!(history.snapshot().control.ledger.elapsed, before.control.ledger.elapsed);
        assert_eq!(history.snapshot().executions, 0); assert_eq!(history.progress().charged_units, 0);
        assert_eq!(client.receipt(), Some(application.receipt)); assert!(application.receipt.acknowledged());
        let mut run = FileOversight::begin_open_with_learned_generation(root.store(), p, &config).unwrap();
        while run.progress().status == FileLearnedRecoveryStatus::Replaying {
            run.advance(run.progress().replayed_events, 1).unwrap();
        }
        let machine = run.verified_guarded_machine().unwrap();
        assert_eq!(machine.broker.hosted_learned_generation().unwrap(), n);
        let consistency = machine.consistency_snapshot(history.snapshot().revision).unwrap();
        assert!(consistency.coverage_lost); assert_eq!(consistency.pending_attempt, pending_prediction);
    }}
}

#[test]
fn mediated_plain_target_rechecks_exact_graph_availability_and_independent_floors() {
    let root = Directory::new(); let config = config(false, 1); let p = profile();
    let graph = graph(p.delivery.scope, p.delivery.target, 1, false);
    let (mut host, _) = FileOversight::create_mediated_guarded_with_learned_generation_reserved(
        root.store(), p.clone(), &guards(), None, graph.clone(), None, config.clone(), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host).unwrap();
    let expected = FileMediatedRequirements { oversight: requirements(&host),
        topology: FileTopologyRequirement { initial: graph.clone(), current: graph,
            available: host.mediation_snapshot().unwrap().available },
        prediction: None, evaluation: None };
    let anchor = host.history_anchor().unwrap(); let before = host.inspect(); drop(host);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap(); let unchanged = files(&root);
    for mismatch in 0..3 {
        let mut wrong = expected.clone();
        match mismatch {
            0 => wrong.topology.current = self::graph(p.delivery.scope, p.delivery.target, 2, false),
            1 => wrong.topology.available = !expected.topology.available,
            _ => wrong.oversight.minimum.control_sequence += 1,
        }
        let candidate = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
            root.store(), p.clone(), &wrong, &config, &anchor);
        if mismatch == 0 {
            assert_eq!(candidate.err(), Some(Error::Binding.into()));
        } else {
            let mut run = candidate.unwrap();
            assert!(run.advance(0, run.progress().total_events).is_err());
            assert!(run.into_recovery_stop(701).is_err());
        }
        assert_eq!(files(&root), unchanged);
    }
    let mut run = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        root.store(), p, &expected, &config, &anchor).unwrap().require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let (mut server, mut client) = connect(run.into_recovery_stop(701).unwrap());
    decide(&mut server, &mut client); client.request_stop().unwrap(); finish(&mut server, &mut client);
    let history = server.into_application().unwrap().result.unwrap();
    assert_eq!(history.snapshot().revision, before.revision + 2); assert_eq!(history.appended_events(), 2);
    assert_eq!(history.snapshot().control.ledger.elapsed, before.control.ledger.elapsed);
    assert!(client.receipt().unwrap().acknowledged());
}
