//! Numerical work is separately granted; plain stream restriction is native.
use super::*;
use crate::action::consequence::activation::monitor::decoder::MonitoredStep;
use crate::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderIdentity, DecoderProfile, DecoderShape, MAX_DECODER_PRODUCTS,
};
use crate::action::consequence::delivery::persistent::observed::decoder::FileDecoderConfig;
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;

mod data {
    include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/decoder_inputs.rs"));
}

fn numerical_requires_paid_replay(host: FileOversight, root: &Root) {
    let before = host.inspect();
    let files = root.files();
    let decoder = host.decoder_inspection().ok();
    let predictor = host.action_consistency_snapshot().ok();
    let stream = host.stream_snapshot().ok();
    let mediation = host.mediation_snapshot().ok();
    let (_, mut supervisor) = host.into_actor_gateway();
    supervisor.set_snapshot(before.revision, Some(snapshot())).unwrap();
    let (port, grant) = supervisor.narrow_port(1, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let offered = plan(port.next(1, Vec::new()));
    assert_eq!(offered.block_reason(), Some(FileNarrowBlockReason::NumericalReplayUnsupported));
    assert_eq!(port.narrow(FileNarrowRequest::new(1, offered.basis(), Vec::new()).unwrap()), Err(FileNarrowError::Unsupported));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(grant.remaining_numerical_replay_events(), None);
    assert!(supervisor.owner.borrow().snapshot.is_some());
    {
        let host = supervisor.host().unwrap();
        assert_eq!(host.inspect(), before);
        assert_eq!(host.decoder_inspection().ok(), decoder);
        assert_eq!(host.action_consistency_snapshot().ok(), predictor);
        assert_eq!(host.stream_snapshot().ok(), stream);
        assert_eq!(host.mediation_snapshot().ok(), mediation);
    }
    assert_eq!(root.files(), files);

    let events = before.revision as usize + 1;
    let (paid, paid_grant) = supervisor.narrow_port_with_numerical_budget(
        2, 2, events + 1, events).unwrap();
    let input = plan(paid.next(1, Vec::new())).request().unwrap();
    let acknowledged = receipt(paid.narrow(input.clone()));
    assert_eq!(acknowledged.refunded_units(), 0);
    assert_eq!(paid_grant.remaining_numerical_replay_events(), Some(0));
    assert_eq!(paid_grant.remaining_operations(), 1);
    assert!(supervisor.owner.borrow().snapshot.is_none());
    {
        let host = supervisor.host().unwrap();
        let after = host.inspect();
        assert_eq!(after.control.ledger, before.control.ledger);
        assert_eq!(after.payload, before.payload);
        assert_eq!(after.executions, before.executions);
        if let Some(mut expected) = decoder {
            expected.journal_revision = after.revision;
            assert_eq!(host.decoder_inspection().unwrap(), expected);
        }
        if let Some(mut expected) = predictor {
            expected.journal_revision = after.revision;
            assert_eq!(host.action_consistency_snapshot().unwrap(), expected);
        }
    }
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let files = root.files();
    assert_eq!(receipt(paid.clone().narrow(input)), acknowledged);
    let blocked = plan(paid.next(2, Vec::new()));
    assert_eq!(blocked.block_reason(), Some(FileNarrowBlockReason::NumericalReplayLimit));
    assert_eq!(paid.narrow(FileNarrowRequest::new(2, blocked.basis(), Vec::new()).unwrap()),
        Err(FileNarrowError::Capacity));
    assert_eq!(paid_grant.remaining_operations(), 1);
    assert_eq!(paid_grant.remaining_numerical_replay_events(), Some(0));
    assert!(supervisor.owner.borrow().snapshot.is_some());
    assert_eq!(root.files(), files);
}

#[test]
fn actual_decoder_and_raw_predictor_require_paid_replay_and_preserve_numerical_state() {
    let root = Root::new();
    let decoder = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 9,
        model_generation: 1, tokenizer_generation: 1, profile_generation: 1 },
        DecoderShape { vocabulary: 2, hidden: 2, intermediate: 2, layers: 1,
            query_heads: 1, cache_heads: 1, context: 4 }, 1e-5, 10000.0).unwrap();
    let config = FileDecoderConfig::new(decoder, data::weights(), data::monitor(3.0),
        data::sampling(), 5, DecoderBindingLimits::default()).unwrap();
    let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_decoder(host.revision(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let cut = host.decoder_inspection().unwrap().numerical;
    assert!(matches!(host.advance_decoder_forced(host.revision(), cut.actor_revision, cut.position,
        0, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap().unwrap(), MonitoredStep::Released(_)));
    // A recovery pause is not a reason to withhold a monotone restriction.
    host.fence(host.revision()).unwrap();
    assert!(host.decoder_inspection().unwrap().paused);
    numerical_requires_paid_replay(host, &root);

    use crate::action::consequence::activation::consistency::{BinaryForecast, ErrorBudget, ForecastRegistration};
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
    use crate::action::consequence::delivery::persistent::observed::consistency::{FileConsistencyConfig, FileConsistencyParameters};
    use crate::action::consequence::gate::containment::ActorState;
    let root = Root::new();
    let mut selected = profile();
    selected.delivery.actor = ActorState::new(selected.delivery.actor.profile(),
        vec![0], vec![1], vec![2], 1).unwrap();
    let capture = CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 };
    let config = FileConsistencyConfig::new(FileConsistencyParameters {
        probe_id: 1, probe_generation: 1, profile: capture, weights: vec![1.0], bias: 0.0,
        threshold: 0.0, forecast: ForecastRegistration { domain: 19, generation: 1,
            policy_generation: 1, event_prefix: b"ok".to_vec(),
            negative: BinaryForecast::new(16384, 49152).unwrap(),
            at_threshold: BinaryForecast::new(32768, 32768).unwrap(),
            positive: BinaryForecast::new(49152, 16384).unwrap() },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 4,
        max_prediction_age_ticks: 10 }).unwrap();
    let (mut host, _) = FileOversight::create(root.store(), selected).unwrap();
    let observer = host.enable_action_consistency(host.revision(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let frame = SourceFrame::capture(FrameIdentity { profile: capture, stream: 17,
        sequence: 1, position: 0 }, &[1.0]).unwrap();
    let revision = host.revision();
    let actor_revision = host.actor_snapshot().unwrap().actor_revision;
    observer.forecast_request(&mut host, revision, 701, actor_revision, &frame).unwrap().unwrap();
    assert!(!host.decoder_required());
    assert!(host.action_consistency_required());
    numerical_requires_paid_replay(host, &root);
}

#[test]
fn plain_stream_restriction_needs_no_numerical_allowance_and_keeps_its_boundary() {
    let root = Root::new();
    let mut selected = profile();
    selected.delivery.initial_payload.clear();
    let (mut host, _) = FileOversight::create_stream(root.store(), selected,
        StreamProfile::new(9, 1, 4, 16, 64).unwrap()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let before = host.stream_snapshot().unwrap();
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.narrow_port(1, 1, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let input = plan(port.next(1, Vec::new())).request().unwrap();
    let restricted = receipt(port.narrow(input));
    assert!(restricted.cancelled().is_empty());
    assert_eq!(restricted.refunded_units(), 0);
    assert_eq!(grant.remaining_numerical_replay_events(), None);
    let after = supervisor.host().unwrap().stream_snapshot().unwrap();
    assert_eq!(after.confirmed, before.confirmed);
    assert_eq!(after.published, before.published);
    assert_eq!(after.pending, before.pending);
    assert_eq!(after.confirmed_target, before.confirmed_target);
    assert_eq!(after.publication.control.ledger, before.publication.control.ledger);
    assert_eq!(after.publication.executions, 0);
}

#[test]
fn mediated_restriction_does_not_invent_a_missing_cut_or_change_graph_availability() {
    let root = Root::new();
    let selected = profile();
    let graph = graph(selected.delivery.scope, selected.delivery.target);
    let (mut host, _) = FileOversight::create(root.store(), selected).unwrap();
    host.enable_mediation(host.revision(), graph).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let before = host.inspect();
    let mediation = host.mediation_snapshot().unwrap();
    assert!(mediation.available);
    assert!(mediation.accepted.is_none());
    let selected = native(&host, 1, Vec::new());
    let receipt = host.narrow_authority(host.revision(), selected).unwrap();
    assert_eq!(receipt.change().refunded_units, 0);
    assert!(receipt.change().cancelled.is_empty());
    let after = host.inspect();
    assert_eq!(after.revision, before.revision + 1);
    assert_eq!(after.control.sequence, before.control.sequence + 1);
    assert_eq!(after.control.ledger, before.control.ledger);
    assert!(after.control.ceiling.as_ref().unwrap().targets().is_empty());
    let current = host.mediation_snapshot().unwrap();
    assert_eq!(current.graph, mediation.graph);
    assert_eq!(current.available, mediation.available);
    assert!(current.accepted.is_none());
    assert_eq!(host.inspect().executions, 0);
}

fn graph(scope: Scope, target: ResolvedTarget) -> crate::action::consequence::mediation::AuthorityGraph {
    use crate::action::consequence::mediation::*;
    use crate::perimeter::*;
    AuthorityGraph::new(GraphSpec {
        generation: 1, inventory_generation: 1, scope, target,
        family: EffectFamilyRecord {
            scope: PerimeterScope { tenant: scope.tenant, principal: scope.principal, purpose: 1 },
            family: "file-publication".into(),
            trust_domains: vec![TrustDomain::Actor, TrustDomain::ObservationAndAnalysis,
                TrustDomain::Enforcement, TrustDomain::GovernanceAndInvestigation],
            credentials: vec![CredentialExposure { credential: "file-owner".into(), holder: CredentialHolder::Broker }],
            routes: vec![RouteRecord { route: "publication".into(),
                threat: Some(ThreatClass::DirectCredentialOrEgress), mediation: Mediation::BrokeredEffects,
                bypass: BypassDisposition::Blocked }],
            residual_nonclaims: vec!["Declared graph only".into()],
        },
        completeness: Completeness::DeclaredComplete { inventory_generation: 1 },
        nodes: vec![Node { id: 1, kind: NodeKind::Actor }, Node { id: 2, kind: NodeKind::Enforcer },
            Node { id: 3, kind: NodeKind::Sink }],
        edges: vec![Edge { id: 1, from: 1, to: 2, channel: Channel::Dispatch, route: "publication".into(), provenance: 101 },
            Edge { id: 2, from: 2, to: 3, channel: Channel::Dispatch, route: "publication".into(), provenance: 102 }],
        enforcers: vec![Enforcer { node: 2, adapter: target.adapter, contract_version: target.contract_version,
            generation: target.generation, provenance: 90 }],
    }).unwrap()
}
