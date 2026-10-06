//! Preinstalled identity requirements and unchanged actor-gateway custody.
use super::*;
use crate::action::consequence::activation::identity::{ModelManifest, ModelPassport, IdentityAnchor};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use crate::action::consequence::delivery::persistent::observed::identity::FileLearnedIdentityInput;
use crate::action::consequence::oversight::{identity::{IdentityPolicy, IdentityOutcome},
    actor::{ActorProposal, ActorOutcome, Knowledge}};

#[test]
fn provisioned_computed_identity_and_required_forecast_keep_both_original_observer_roles() {
    let expected = pinned().with_required_computed_identity().unwrap();
    let refused = Directory::new();
    assert!(FileOversight::create_with_pre_output_forecast(refused.store(), profile(), expected.clone()).is_err());
    assert!(!refused.store().exists(), "the convenience constructor cannot omit prior identity provisioning");
    let root = Directory::new(); let (mut host, _reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    // Declared manifests remain trusted provisioning inputs; matching these
    // synthetic digests is not authentication or an independent model passport.
    let manifest = ModelManifest { tenant: 1, model: 2, model_generation: 3, host_generation: 1,
        tokenizer_generation: 4, weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
        architecture: [4; 32], numeric_profile: [5; 32] };
    let (model, _, _) = recipe();
    let passport = ModelPassport::new(51, 1, manifest.clone(), vec![IdentityAnchor::new(10,
        model.residual_contract(1).unwrap().profile(), 5, vec![u32::from(b'p')],
        &[[-2.0, 2.0], [-2.0, 2.0]]).unwrap()]).unwrap();
    let identity = host.enable_identity_checks(host.revision(), passport,
        IdentityPolicy { observer_id: 99, timeout_ticks: 10, validity_ticks: 20, max_checks: 8 }).unwrap();
    let before_basis = host.identity_basis().unwrap(); let before = host.revision();
    let observer = host.enable_learned_generation_with_pre_output_forecast(before, expected.clone()).unwrap();
    assert_eq!(host.revision(), before + 1); assert_eq!(host.identity_basis().unwrap(), before_basis);
    assert!(host.machine.learned_contract().unwrap().requires_computed_identity());
    assert_eq!(host.machine.consistency.as_deref(), expected.required_pre_output_forecast());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let control = host.inspect().control;
    let challenge = host.begin_identity_check(host.revision(), 901, control.sequence, numerical.actor_revision).unwrap().unwrap();
    let revision = host.revision();
    let observed = identity.observe_computed_learned(&mut host, revision, &challenge,
        FileLearnedIdentityInput { measurement_sequence: 1,
            budget: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }, observed_manifest: manifest },
        || ElapsedTick(1)).unwrap();
    assert_eq!(observed.work.entered_tokens, 1); assert_eq!(observed.work.measured_anchors, 1);
    assert_eq!(observed.observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
    let basis = challenge.evidence();
    host.apply_identity_check(host.revision(), &challenge, basis.control_sequence(), basis.revocation_epoch()).unwrap();
    assert!(host.identity_installation(challenge.id()).unwrap().is_some());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical,
        "identity anchors are computed independently without advancing the live generator");
    step(&mut host);
    let prompt = host.learned_generation_inspection().unwrap().numerical;
    assert!(host.begin_learned_step(host.revision(), prompt.actor_revision, prompt.position).is_err());
    let _forecast = begin(&mut host, &observer); step(&mut host); step(&mut host);
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    assert_eq!(host.inspect().executions, 0); independent(&host, &expected);
    let bytes = disk(&host); drop(host);
    assert!(FileOversight::open_with_learned_generation(root.store(), profile(), &pinned()).is_err());
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), bytes);
    let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &expected).unwrap();
    let revision = recovered.revision();
    assert!(recovered.enable_learned_generation_with_pre_output_forecast(revision, expected).is_err());
    assert_eq!(recovered.revision(), revision);
    assert!(recovered.learned_generation_inspection().unwrap().paused);
    assert!(recovered.action_consistency_snapshot().unwrap().coverage_lost);
}

#[test]
fn existing_actor_gateway_remains_the_only_request_owner_after_pinned_attachment() {
    let root = Directory::new(); let expected = pinned();
    let (host, _reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    let (port, mut supervisor) = host.into_actor_gateway();
    let observer = {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.enable_learned_generation_with_pre_output_forecast(revision, expected.clone()).unwrap()
    };
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.observe_time(revision, ElapsedTick(1)).unwrap(); step(&mut host);
        let _forecast = begin(&mut host, &observer); step(&mut host); step(&mut host);
    }
    observe(&mut supervisor, snapshot());
    let p = proposal();
    let ticket = port.submit(71, &ActorProposal { target: p.target, payload: b"aa".to_vec(),
        expected_policy_epoch: p.expected_policy_epoch, deadline: p.deadline, units: p.units }).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Pending { request: 71 }));
    port.cancel(&ticket).unwrap();
    assert!(matches!(port.poll(&ticket), Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
    let host = supervisor.host().unwrap();
    assert_eq!(host.retained_requests(), 1); assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    independent(&host, &expected);
}

#[test]
fn stale_unpinned_or_invalid_attachment_cannot_mutate_the_existing_authority() {
    let root = Directory::new(); let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let before = disk(&host); let state = host.inspect();
    assert!(matches!(host.enable_learned_generation_with_pre_output_forecast(host.revision() + 1, pinned()),
        Err(JournalError::Contract(Error::Stale))));
    assert!(matches!(host.enable_learned_generation_with_pre_output_forecast(host.revision(), config()),
        Err(JournalError::Contract(Error::Binding))));
    let mut wrong = parameters(); wrong.stream += 1;
    let wrong = FileConsistencyConfig::new(wrong).unwrap().with_hosted_residual(1).unwrap().with_pre_output_forecast().unwrap();
    let wrong = config().with_required_pre_output_forecast(wrong).unwrap();
    assert!(host.enable_learned_generation_with_pre_output_forecast(host.revision(), wrong).is_err());
    assert_eq!(disk(&host), before); assert_eq!(host.inspect(), state);
    assert!(!host.learned_generation_required()); assert!(!host.action_consistency_required());
    assert!(host.storage_failure().is_none());
    let expected = pinned();
    let observer = host.enable_learned_generation_with_pre_output_forecast(host.revision(), expected.clone()).unwrap();
    step(&mut host); let _forecast = begin(&mut host, &observer); step(&mut host);
    independent(&host, &expected);
}
