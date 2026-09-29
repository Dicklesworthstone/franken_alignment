//! Computed identity must reproduce model execution, not import reassuring frames.
use super::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderLayerWeights;
use fa_reference::action::consequence::delivery::persistent::observed::identity::{
    FileComputedIdentityObservation, FileLearnedIdentityInput};

fn input(c: &FileIdentityChallenge) -> FileLearnedIdentityInput {
    FileLearnedIdentityInput { measurement_sequence: 700 + c.id(),
        budget: DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS },
        observed_manifest: c.evidence().passport().manifest().clone() }
}
fn compute(host: &mut FileOversight, observer: &FileIdentityObserver, c: &FileIdentityChallenge,
    completed_at: u64) -> FileComputedIdentityObservation
{
    let revision = host.revision();
    observer.observe_computed_learned(host, revision, c, input(c), || ElapsedTick(completed_at)).unwrap()
}
fn install(host: &mut FileOversight, c: &FileIdentityChallenge) {
    host.apply_identity_check(host.revision(), c, c.evidence().control_sequence(),
        c.evidence().revocation_epoch()).unwrap();
}

#[test]
fn computed_measurements_match_the_existing_stepped_engine_and_leave_installation_separate() {
    let a = Directory::new(); let b = Directory::new();
    let (mut host, roles, config, passport) = setup(&a, fixture::model(1.0));
    let (mut control, control_roles, _, _) = setup(&b, fixture::model(1.0));
    step(&mut host); step(&mut control);
    let observer = roles.identity_observer.as_ref().unwrap();
    let c = challenge(&mut host, 1);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let revision = host.revision(); let outcome = compute(&mut host, observer, &c, 1);
    assert_eq!(host.revision(), revision + 1);
    assert_eq!(outcome.observation.measurement.as_ref().unwrap().outcome, IdentityOutcome::Matched);
    assert!(outcome.observation.containment.is_none());
    assert_eq!(outcome.work.entered_tokens, 4); assert_eq!(outcome.work.measured_anchors, 2);
    assert!(host.identity_installation(1).unwrap().is_none());
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert!(host.propose(host.revision(), 1, spec(&host), snapshot()).is_err());
    let other = control_roles.identity_observer.as_ref().unwrap();
    let mut run = start(&mut control, other, 1);
    while control.identity_report(1).unwrap().outcome == IdentityOutcome::Collecting {
        run.step_with_clock(&mut control, other, || ElapsedTick(1)).unwrap();
    }
    assert_eq!(outcome.work, run.work());
    assert_eq!(host.identity_report(1).unwrap(), control.identity_report(1).unwrap());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    install(&mut host, &c);
    let historical = host.identity_report(1).unwrap();
    let expected = requirements(&host, &passport);
    assert_eq!(FileOversight::read_publication_with_learned_generation(a.store(), &profile(), &config).unwrap().executions, 0);
    drop(host);
    let (mut recovered, fresh) = FileOversight::open_guarded_with_learned_generation(
        a.store(), profile(), &expected, &config).unwrap();
    recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(recovered.identity_report(1).unwrap(), historical);
    assert_eq!(recovered.identity_status().unwrap(), IdentityStatus::Missing);
    assert!(recovered.learned_generation_inspection().unwrap().paused);
    assert!(fresh.identity_observer.as_ref().unwrap().observe_computed_learned(&mut recovered,
        revision, &c, input(&c), || ElapsedTick(2)).is_err());
}

#[test]
fn computed_check_flows_through_both_original_keys_and_canonical_publication() {
    let root = Directory::new(); let (mut host, roles, config, _) = setup(&root, fixture::model(1.0));
    step(&mut host); step(&mut host);
    let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
    compute(&mut host, observer, &c, 1); install(&mut host, &c);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let action = host.propose(host.revision(), 1, spec(&host), snapshot()).unwrap();
    let packet = host.begin_learned_sidecar(host.revision(), 1, numerical.actor_revision,
        LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1001, generation: 1, transform_id: 7 },
            priority: Vec::new(), budget: SidecarCongressBudget::default() }).unwrap();
    let view = packet.packet.input();
    assert!(host.authorize(host.revision(), 1, view, snapshot()).is_err());
    host.begin_review(host.revision(), 1, 101, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(10), reveal_by: ElapsedTick(15) }, snapshot()).unwrap();
    host.commit_review(host.revision(), 101, "reviewer",
        commitment(101, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), 101).unwrap();
    host.reveal_review(host.revision(), 101, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), 101, Some(view), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), 1, view, snapshot()).unwrap();
    assert!(host.publish_checked(host.revision(), 1, Some(view), snapshot(), ElapsedTick(1)).is_err());
    let request = host.request_human_approval(host.revision(), 1001, 1, view, ElapsedTick(40)).unwrap();
    let revision = host.revision(); let human = roles.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, view, snapshot()).unwrap();
    assert_eq!(host.publish_checked(host.revision(), 1, Some(view), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(disk.payload, b"visible"); assert_eq!(disk.executions, 1); assert_eq!(disk.control.ledger.charged, 16);
}

#[test]
fn same_labels_cannot_substitute_quiet_parameters_for_the_model_replayed_from_the_recipe() {
    for changed in [false, true] {
        let root = Directory::new();
        let (mut host, roles, config, _) = setup(&root, fixture::model(if changed { 2.0 } else { 1.0 }));
        let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
        let result = compute(&mut host, observer, &c, 1);
        assert_eq!(result.work.completed_tokens, 4);
        let report = result.observation.measurement.unwrap();
        if changed {
            assert_eq!(report.outcome, IdentityOutcome::Mismatch(IdentityMismatch::Anchor { anchor: 10 }));
            assert_eq!(report.observations[&10].first_outlier().unwrap().observed_bits, 2.0_f32.to_bits());
            assert!(result.observation.containment.unwrap().is_ok()); assert!(host.inspect().control.suspended);
        } else { assert_eq!(report.outcome, IdentityOutcome::Matched); assert!(!host.inspect().control.suspended); }
        let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
        assert_eq!(disk.control.suspended, changed); assert_eq!(disk.executions, 0);
    }
}

#[test]
fn a_manifest_mismatch_uses_original_containment_without_entering_any_anchor_token() {
    let root = Directory::new(); let (mut host, roles, config, _) = setup(&root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
    let mut request = input(&c); request.observed_manifest.weights[0] ^= 1;
    let revision = host.revision();
    let result = observer.observe_computed_learned(&mut host, revision, &c, request, || ElapsedTick(1)).unwrap();
    assert_eq!(result.work.entered_tokens, 0); assert_eq!(result.work.measured_anchors, 0);
    assert_eq!(result.observation.measurement.unwrap().outcome, IdentityOutcome::Mismatch(IdentityMismatch::Manifest));
    assert!(result.observation.containment.unwrap().is_ok());
    assert!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap().control.suspended);
}

#[test]
fn exact_budget_admits_but_undersized_stale_foreign_and_mixed_input_calls_do_no_work() {
    let root = Directory::new(); let model = fixture::model(1.0);
    let (mut host, roles, _, passport) = setup(&root, model.clone());
    let other_root = Directory::new(); let (_, other_roles, _, _) = setup(&other_root, model.clone());
    let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
    let required = passport.anchors().values().map(|a| model.estimate(0, a.stimulus().len())
        .unwrap().scalar_products().unwrap()).sum::<u64>();
    let before = root.bytes(); let revision = host.revision(); let mut calls = 0;
    let mut small = input(&c); small.budget.scalar_products = required - 1;
    assert_eq!(observer.observe_computed_learned(&mut host, revision, &c, small,
        || { calls += 1; ElapsedTick(1) }).err(), Some(Error::Limit.into()));
    assert_eq!(observer.observe_computed_learned(&mut host, revision - 1, &c, input(&c),
        || { calls += 1; ElapsedTick(1) }).err(), Some(Error::Stale.into()));
    assert_eq!(other_roles.identity_observer.as_ref().unwrap().observe_computed_learned(&mut host,
        revision, &c, input(&c), || { calls += 1; ElapsedTick(1) }).err(), Some(Error::Binding.into()));
    assert_eq!(calls, 0); assert_eq!(root.bytes(), before);
    let mut exact = input(&c); exact.budget.scalar_products = required;
    let result = observer.observe_computed_learned(&mut host, revision, &c, exact, || ElapsedTick(1)).unwrap();
    assert_eq!(result.work.completed_scalar_products, required);
    let before = root.bytes(); let revision = host.revision();
    assert_eq!(observer.observe_computed_learned(&mut host, revision, &c, input(&c),
        || { calls += 1; ElapsedTick(1) }).err(), Some(Error::WrongState.into()));
    assert_eq!(calls, 0); assert_eq!(root.bytes(), before);
    let fresh = challenge(&mut host, 2);
    let revision = host.revision();
    observer.observe_manifest(&mut host, revision, &fresh, fresh.evidence().passport().manifest().clone(), ElapsedTick(1)).unwrap();
    let before = root.bytes(); let revision = host.revision();
    assert_eq!(observer.observe_computed_learned(&mut host, revision, &fresh, input(&fresh),
        || { calls += 1; ElapsedTick(1) }).err(), Some(Error::WrongState.into()));
    assert_eq!(calls, 0); assert_eq!(root.bytes(), before);
}

#[test]
fn receipt_time_crossing_withdraws_all_unpublished_measurements_without_backdating() {
    for expired in [false, true] {
        let root = Directory::new(); let (mut host, roles, config, _) = setup(&root, fixture::model(1.0));
        let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
        let end = c.evidence().deadline().0 - u64::from(!expired); let mut calls = 0;
        let revision = host.revision();
        let result = observer.observe_computed_learned(&mut host, revision, &c, input(&c), || {
            calls += 1; ElapsedTick(if calls == 1 { 1 } else { end })
        }).unwrap();
        assert_eq!(calls, 2); assert_eq!(result.work.completed_tokens, 4);
        assert_eq!(result.started_at, ElapsedTick(1)); assert_eq!(result.completed_at, ElapsedTick(end));
        if expired {
            assert_eq!(result.observation.measurement, Err(Error::Stale));
            assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Unavailable);
            assert!(host.identity_report(1).unwrap().observations.is_empty());
            assert!(host.apply_identity_check(host.revision(), &c, c.evidence().control_sequence(), c.evidence().revocation_epoch()).is_err());
        } else { assert_eq!(result.observation.measurement.unwrap().outcome, IdentityOutcome::Matched); install(&mut host, &c); }
        assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap().executions, 0);
    }
}

fn overflow_model(overflow: bool) -> DecoderModel {
    let profile = fixture::model(1.0).profile().clone();
    // Codec training visits only 0/1, whose second normalized coordinate is 0.
    // The registered identity stimulus visits 2, whose second coordinate is >1.
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2],
        queries: vec![0.0, if overflow { f32::MAX } else { 0.0 }, 0.0, 0.0],
        keys: vec![0.0; 4], values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    DecoderModel::new(profile, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], vec![layer.clone(), layer],
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}

#[test]
fn actual_numerical_failure_is_acknowledged_unavailable_and_cannot_be_retried_as_success() {
    for overflow in [false, true] {
        let root = Directory::new(); let (mut host, roles, config, _) = setup(&root, overflow_model(overflow));
        let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let result = compute(&mut host, observer, &c, 1);
        if overflow {
            assert_eq!(result.observation.measurement, Err(Error::Overflow));
            assert_eq!(result.work.entered_tokens, 1); assert_eq!(result.work.completed_tokens, 0);
            assert!(result.work.entered_scalar_product_bound > 0);
            assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Unavailable);
            let revision = host.revision(); let bytes = root.bytes();
            assert!(observer.observe_computed_learned(&mut host, revision, &c, input(&c), || ElapsedTick(1)).is_err());
            assert_eq!(root.bytes(), bytes);
        } else { assert_eq!(result.observation.measurement.unwrap().outcome, IdentityOutcome::Matched); }
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap().executions, 0);
    }
}

#[test]
fn post_inference_clock_unwind_or_backwards_time_poisons_without_exposing_a_candidate() {
    for unwind in [false, true] {
        let root = Directory::new(); let (mut host, roles, config, _) = setup(&root, fixture::model(1.0));
        let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
        let before = root.bytes(); let revision = host.revision(); let mut calls = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            observer.observe_computed_learned(&mut host, revision, &c, input(&c), || {
                calls += 1;
                if calls == 2 { if unwind { panic!("post-inference interruption"); } return ElapsedTick(0); }
                ElapsedTick(1)
            })
        }));
        assert_eq!(calls, 2);
        if unwind { assert!(result.is_err()); } else { assert_eq!(result.unwrap().err(), Some(Error::Stale.into())); }
        assert_eq!(root.bytes(), before);
        assert_eq!(host.identity_report(1).err(), Some(JournalError::Unavailable));
        assert_eq!(observer.observe_computed_learned(&mut host, revision, &c, input(&c), || ElapsedTick(1)).err(), Some(JournalError::Unavailable));
        assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap().executions, 0);
    }
}

#[test]
fn failed_canonical_write_returns_no_computed_report_and_recovery_withdraws_the_check() {
    let root = Directory::new(); let (mut host, roles, config, passport) = setup(&root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
    let expected = requirements(&host, &passport); let before = root.bytes(); let revision = host.revision();
    let mut calls = 0;
    let result = observer.observe_computed_learned(&mut host, revision, &c, input(&c), || {
        calls += 1;
        if calls == 2 { std::fs::write(root.store().join("delivery.pending"), b"staging conflict").unwrap(); }
        ElapsedTick(1)
    });
    assert_eq!(calls, 2); assert!(matches!(result, Err(JournalError::Io(_))));
    assert_eq!(root.bytes(), before); assert_eq!(host.identity_report(1).err(), Some(JournalError::Unavailable));
    drop(host);
    let (mut host, _) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Unavailable);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn changed_numerical_work_witnesses_are_rejected_instead_of_imported() {
    let root = Directory::new(); let (mut host, roles, config, _) = setup(&root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
    compute(&mut host, observer, &c, 1); install(&mut host, &c);
    let bytes = root.bytes(); let magic = b"FALIDP\0\x01";
    let positions: Vec<_> = bytes.windows(magic.len()).enumerate().filter(|(_, v)| *v == magic).map(|(i, _)| i).collect();
    assert_eq!(positions.len(), 1); let start = positions[0];
    drop(host);
    for offset in [8, 24, 40, 56] {
        let mut corrupt = bytes.clone(); corrupt[start + offset] ^= 1;
        std::fs::write(root.store().join("delivery.bin"), &corrupt).unwrap();
        assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).err(),
            Some(Error::Binding.into()));
    }
    std::fs::write(root.store().join("delivery.bin"), &bytes).unwrap();
    assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap().executions, 0);
}


#[test]
fn real_quiet_frames_from_a_different_model_cannot_replace_the_computed_witness() {
    let quiet_root = Directory::new(); let changed_root = Directory::new();
    let (mut quiet, quiet_roles, quiet_config, _) = setup(&quiet_root, fixture::model(1.0));
    let (mut changed, changed_roles, changed_config, _) = setup(&changed_root, fixture::model(2.0));
    let a = challenge(&mut quiet, 1); let b = challenge(&mut changed, 1);
    let quiet_result = compute(&mut quiet, quiet_roles.identity_observer.as_ref().unwrap(), &a, 1);
    let changed_result = compute(&mut changed, changed_roles.identity_observer.as_ref().unwrap(), &b, 1);
    assert_eq!(quiet_result.observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
    assert!(matches!(changed_result.observation.measurement.unwrap().outcome, IdentityOutcome::Mismatch(_)));
    let quiet_bytes = quiet_root.bytes(); let changed_bytes = changed_root.bytes();
    drop(quiet); drop(changed);
    let magic = b"FALIDP\0\x01";
    let a = quiet_bytes.windows(magic.len()).position(|v| v == magic).unwrap();
    let b = changed_bytes.windows(magic.len()).position(|v| v == magic).unwrap();
    // Computed is the final record; its opaque witness is the final field.
    assert_eq!(quiet_bytes.len() - a, changed_bytes.len() - b);
    assert_ne!(&quiet_bytes[a..], &changed_bytes[b..]);
    let mut substituted = changed_bytes[..b].to_vec(); substituted.extend_from_slice(&quiet_bytes[a..]);
    std::fs::write(changed_root.store().join("delivery.bin"), &substituted).unwrap();
    assert_eq!(FileOversight::read_publication_with_learned_generation(changed_root.store(), &profile(), &changed_config).err(),
        Some(Error::Binding.into()));
    assert!(!FileOversight::read_publication_with_learned_generation(quiet_root.store(), &profile(), &quiet_config).unwrap().control.suspended);
    std::fs::write(changed_root.store().join("delivery.bin"), changed_bytes).unwrap();
    assert!(FileOversight::read_publication_with_learned_generation(changed_root.store(), &profile(), &changed_config).unwrap().control.suspended);
}


#[test]
fn changed_actor_or_withdrawn_basis_refuses_before_clock_and_fresh_challenge_still_works() {
    for actor in [false, true] {
        let root = Directory::new(); let (mut host, roles, _, _) = setup(&root, fixture::model(1.0));
        let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
        if actor { step(&mut host); }
        else { host.identity_unavailable(host.revision(), host.identity_basis().unwrap()).unwrap(); }
        let bytes = root.bytes(); let revision = host.revision(); let mut calls = 0;
        assert_eq!(observer.observe_computed_learned(&mut host, revision, &c, input(&c),
            || { calls += 1; ElapsedTick(1) }).err(), Some(Error::Stale.into()));
        assert_eq!(calls, 0); assert_eq!(root.bytes(), bytes);
        let fresh = challenge(&mut host, 2);
        assert_eq!(compute(&mut host, observer, &fresh, 1).observation.measurement.unwrap().outcome, IdentityOutcome::Matched);
    }
}

#[test]
fn expiry_before_entry_records_the_native_expiration_without_entering_a_token() {
    let root = Directory::new(); let (mut host, roles, config, _) = setup(&root, fixture::model(1.0));
    let observer = roles.identity_observer.as_ref().unwrap(); let c = challenge(&mut host, 1);
    let result = compute(&mut host, observer, &c, c.evidence().deadline().0);
    assert_eq!(result.work.entered_tokens, 0); assert_eq!(result.work.completed_tokens, 0);
    assert_eq!(result.observation.measurement, Err(Error::Stale));
    assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Expired);
    assert!(host.identity_report(1).unwrap().observations.is_empty());
    assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap().executions, 0);
}
