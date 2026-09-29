//! Real owned parameters and original anchor inference, not mock identity votes.
#![forbid(unsafe_code)]
#[path = "support/learned_identity_model.rs"]
mod fixture;
use fa_reference::Error;
use fa_reference::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use fa_reference::action::consequence::activation::identity::{ModelPassport,
    decoder::{DecoderIdentityMeasurement, DecoderIdentityProbe, IdentityProbeProgress}};
use fa_reference::action::consequence::activation::monitor::MonitorOutcome;
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderBudget, DecoderModel, MAX_DECODER_PRODUCTS,
    sampling::monitored::{GenerationEvent, GenerationStatus}};
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::{TargetCeiling, containment::{
    ActorState, RestartGrade, RestartProfile,
    session::policy::{Policy, Predicate, controller::ControllerConfig}}};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract, OversightBroker,
    decoder_monitoring::LearnedDecoderBindingLimits};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::reducer::Caps;
use std::collections::BTreeMap;
use std::rc::Rc;

fn empty_owner() -> (OversightBroker, PublicationEndpoint) {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let mut endpoint = PublicationEndpoint::new(target, b"initial".to_vec(), 1000, 8).unwrap();
    let contracts = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"owned-identity-test".to_vec(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 7, b"approve?".to_vec()).unwrap())])).unwrap();
    let mut owner = OversightBroker::new(ControllerConfig {
        scope, total: 100, max_attempts: 8,
        actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1, model_generation: 3,
            tokenizer_generation: 4, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart },
            Vec::new(), vec![0], vec![0], 0).unwrap(),
        suspend_at_incident: 3, policy: Policy::new(1, vec![Predicate::PayloadAtMost(128)]).unwrap(),
        congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
            MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]), caps: Caps { per_member: 1, per_cohort: 1 },
            continue_minimum: 1, continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3,
            minimum_members: 1, minimum_cohorts: 1 },
        narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
    }, &mut endpoint, contracts).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    (owner, endpoint)
}
fn owner(model: DecoderModel, alarm: bool) -> (OversightBroker, PublicationEndpoint) {
    let (mut owner, endpoint) = empty_owner();
    let source = fixture::source(&model, alarm);
    owner.own_learned_generation(model, source, LearnedDecoderBindingLimits::default()).unwrap();
    (owner, endpoint)
}
fn step(owner: &mut OversightBroker) -> Rc<GenerationEvent> {
    let n = owner.hosted_learned_generation().unwrap();
    owner.advance_hosted_learned(n.actor_revision, n.position).unwrap()
}
fn probe(owner: &OversightBroker, passport: &ModelPassport) -> DecoderIdentityProbe {
    owner.hosted_learned_identity_probe(owner.actor_revision(), passport, 701,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap()
}
fn drain(probe: &mut DecoderIdentityProbe) -> Vec<DecoderIdentityMeasurement> {
    let planned = probe.work().planned_tokens;
    let mut measured = Vec::new();
    for _ in 0..planned {
        let before = probe.work();
        match probe.advance().unwrap() {
            IdentityProbeProgress::Advanced => {}
            IdentityProbeProgress::Measured(value) => measured.push(*value),
            IdentityProbeProgress::Complete => panic!("completed without spending the declared original stimuli"),
        }
        assert_eq!(probe.work().entered_tokens, before.entered_tokens + 1);
        assert_eq!(probe.work().completed_tokens, before.completed_tokens + 1);
    }
    assert!(probe.complete()); assert_eq!(probe.work().measured_anchors, measured.len());
    let before = probe.work();
    assert!(matches!(probe.advance().unwrap(), IdentityProbeProgress::Complete));
    assert_eq!(probe.work(), before);
    measured
}

#[test]
fn owned_probe_matches_original_anchor_execution_without_touching_live_generation() {
    let model = fixture::model(1.0); let passport = fixture::passport(&model);
    let (mut tested, endpoint) = owner(model.clone(), false);
    let (mut control, _) = owner(model.clone(), false);
    step(&mut tested); step(&mut control); step(&mut tested); step(&mut control);
    let numerical = tested.hosted_learned_generation().unwrap(); let authority = tested.inspect();
    let mut measured = probe(&tested, &passport);
    assert_eq!(measured.work().entered_tokens, 0); assert_eq!(measured.work().planned_tokens, 4);
    let mut direct = DecoderIdentityProbe::new(model, &passport, 701,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
    let actual = drain(&mut measured); let expected = drain(&mut direct);
    assert_eq!(measured.work(), direct.work());
    for (actual, expected) in actual.iter().zip(&expected) {
        assert_eq!(actual.anchor(), expected.anchor());
        assert_eq!(actual.source().encode_initial(23).unwrap(), expected.source().encode_initial(23).unwrap());
        assert_eq!(passport.anchors()[&actual.anchor()].compare(actual.source()).unwrap().outside(), 0);
    }
    assert_eq!(tested.hosted_learned_generation().unwrap(), numerical);
    assert_eq!(tested.inspect(), authority); assert_eq!(endpoint.execution_count(), 0);
    while tested.hosted_learned_generation().unwrap().status.is_active() {
        let actual = step(&mut tested); let expected = step(&mut control);
        assert_eq!(actual.sample(), expected.sample()); assert_eq!(actual.status(), expected.status());
        assert_eq!(actual.audit().source().encode().unwrap(), expected.audit().source().encode().unwrap());
        assert_eq!(tested.hosted_learned_generation().unwrap(), control.hosted_learned_generation().unwrap());
    }
}

#[test]
fn identical_labels_and_passport_cannot_replace_the_owned_parameters() {
    let reference = fixture::model(1.0); let changed = fixture::model(2.0);
    assert_eq!(reference.profile(), changed.profile());
    let passport = fixture::passport(&reference);
    for (model, expected_outside) in [(reference, 0), (changed, 1)] {
        let (owner, endpoint) = owner(model, false);
        let before = owner.hosted_learned_generation().unwrap();
        let measurements = drain(&mut probe(&owner, &passport));
        let first = passport.anchors()[&10].compare(measurements[0].source()).unwrap();
        assert_eq!(first.outside(), expected_outside);
        if expected_outside != 0 {
            assert_eq!(first.first_outlier().unwrap().observed_bits, 2.0_f32.to_bits());
        }
        assert_eq!(passport.anchors()[&20].compare(measurements[1].source()).unwrap().outside(), 0);
        assert_eq!(owner.hosted_learned_generation().unwrap(), before);
        assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn full_roster_exact_budget_admits_but_one_less_cannot_start() {
    let model = fixture::model(1.0); let passport = fixture::passport(&model);
    let (owner, _) = owner(model, false); let before = owner.hosted_learned_generation().unwrap();
    let required = probe(&owner, &passport).work().planned_scalar_products;
    assert!(required > 0);
    assert_eq!(owner.hosted_learned_identity_probe(owner.actor_revision(), &passport, 701,
        DecoderBudget { scalar_products: required - 1 }).err(), Some(Error::Limit));
    let mut exact = owner.hosted_learned_identity_probe(owner.actor_revision(), &passport, 701,
        DecoderBudget { scalar_products: required }).unwrap();
    assert_eq!(drain(&mut exact).len(), passport.anchors().len());
    assert_eq!(exact.work().completed_scalar_products, required);
    assert_eq!(owner.hosted_learned_generation().unwrap(), before);
}

#[test]
fn missing_owner_stale_revision_sequence_and_wrong_profile_refuse_before_inference() {
    let model = fixture::model(1.0); let passport = fixture::passport(&model);
    let budget = DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS };
    let (empty, _) = empty_owner();
    assert_eq!(empty.hosted_learned_identity_probe(empty.actor_revision(), &passport, 1, budget).err(), Some(Error::Incomplete));
    let (owner, _) = owner(model, false); let before = owner.hosted_learned_generation().unwrap();
    assert_eq!(owner.hosted_learned_identity_probe(owner.actor_revision() + 1, &passport, 1, budget).err(), Some(Error::Stale));
    assert_eq!(owner.hosted_learned_identity_probe(owner.actor_revision(), &passport, 0, budget).err(), Some(Error::InvalidInput));
    let mut manifest = passport.manifest().clone(); manifest.tokenizer_generation += 1;
    let wrong = ModelPassport::new(passport.id(), passport.generation(), manifest,
        passport.anchors().values().cloned().collect()).unwrap();
    assert_eq!(owner.hosted_learned_identity_probe(owner.actor_revision(), &wrong, 1, budget).err(), Some(Error::Binding));
    assert_eq!(owner.hosted_learned_generation().unwrap(), before);
}

#[test]
fn diagnostic_probe_on_held_generation_does_not_clear_its_hold_or_refill_spending() {
    let model = fixture::model(1.0); let passport = fixture::passport(&model);
    let (mut owner, endpoint) = owner(model, true); step(&mut owner);
    assert_eq!(step(&mut owner).status(), GenerationStatus::Held(MonitorOutcome::Alarm));
    let before = owner.hosted_learned_generation().unwrap(); let authority = owner.inspect();
    let measured = drain(&mut probe(&owner, &passport));
    assert!(measured.iter().all(|value| passport.anchors()[&value.anchor()].compare(value.source()).unwrap().outside() == 0));
    assert_eq!(owner.hosted_learned_generation().unwrap(), before); assert_eq!(owner.inspect(), authority);
    assert_eq!(owner.advance_hosted_learned(before.actor_revision, before.position).err(), Some(Error::WrongState));
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn detached_probe_remains_observation_when_the_actor_advances() {
    let model = fixture::model(1.0); let passport = fixture::passport(&model);
    let (mut owner, endpoint) = owner(model, false);
    let revision = owner.actor_revision(); let mut pending = probe(&owner, &passport);
    assert!(matches!(pending.advance().unwrap(), IdentityProbeProgress::Advanced));
    step(&mut owner); let after = owner.hosted_learned_generation().unwrap();
    assert_ne!(after.actor_revision, revision);
    while !pending.complete() { pending.advance().unwrap(); }
    assert_eq!(pending.work().completed_tokens, 4);
    assert_eq!(owner.hosted_learned_generation().unwrap(), after);
    assert_eq!(owner.hosted_learned_identity_probe(revision, &passport, 2,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).err(), Some(Error::Stale));
    assert_eq!(endpoint.execution_count(), 0);
}
