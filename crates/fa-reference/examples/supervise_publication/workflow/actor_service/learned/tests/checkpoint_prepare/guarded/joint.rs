//! Joint baseline preparation through the actual guarded operator command.
//! Synthetic identity intervals and held-out labels test enforcement, not risk.
use super::super::super::{identity_tests as identity_fixture, qualification};
use super::*;
use fa_reference::action::consequence::activation::identity::wire as identity_wire;
use fa_reference::action::consequence::oversight::identity::{IdentityMismatch, IdentityOutcome};

fn select(root: &Root, case: &Case) {
    identity_fixture::provision(root, &case.recipe, true);
    select_guards(root, case, true, true);
}

fn fresh_identity(case: &Case) {
    let original = fs::read_to_string(&case.recipe).unwrap();
    let selected = original
        .replacen("\"check\":901", "\"check\":902", 1)
        .replacen(
            "\"measurement_sequence\":1",
            "\"measurement_sequence\":2",
            1,
        );
    assert_ne!(selected, original);
    fs::write(&case.recipe, selected).unwrap();
}

#[test]
fn joint_identity_and_both_guards_prepare_a_baseline_then_require_both_live_review_keys() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let case = Case::new(&root, false);
        select(&root, &case);
        let helper = root.0.join("helper-weights.safetensors");
        let weights = fs::read(&helper).unwrap();
        fs::remove_file(&helper).unwrap();
        assert!(!root.0.join("producer").exists());
        assert!(!root.0.join("broker.bin").exists());
        assert!(!root.0.join("provider.bin").exists());
        let prepared = case.prepare();
        verify_prepared(&case, &prepared);
        assert_eq!(number(&prepared, "control_sequence"), 0);
        assert_eq!(number(&prepared, "authority_epoch"), 0);
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        let generation = case.reader().generation;
        let joint = generation.required_held_out_joint().unwrap();
        assert!(
            generation
                .required_computed_identity_registration()
                .is_some()
        );
        assert!(generation.required_publication_validation().is_some());
        assert!(generation.required_credential_policy().is_some());

        fs::write(helper, weights).unwrap();
        let _producer = producer(&root, &case);
        secrets(&root);
        fresh_identity(&case);
        // A pinned joint policy guards promotions. No activation has happened,
        // so the original baseline continues without inventing a capsule.
        let (result, packet, retained) =
            continue_prepared(&root, &case, decision, &mut |_| ElapsedTick(1001));
        let packet = packet.expect("baseline and identity do not replace human review");
        original_packet(&packet);
        assert_eq!(
            result.is_ok(),
            decision == ReviewDecision::Approve,
            "{result:?}"
        );
        let view = case.view();
        assert_eq!(
            view.publication.executions,
            u64::from(decision == ReviewDecision::Approve)
        );
        assert_eq!(
            view.publication.control.ledger.charged,
            if decision == ReviewDecision::Approve {
                packet.action().spec().units
            } else {
                0
            }
        );
        assert_eq!(view.publication.control.ledger.reserved, 0);
        assert_eq!(view.consistency.consistency.evidence.samples(), 1);
        assert!(view.consistency.work.probe_coordinates > 0);
        no_secret(&case.disk());
        let mut loaded = case.reader();
        loaded.recovery_anchor = Some(custody::load(&retained).unwrap());
        let (host, _, forecast, identity) = recovery::open_selected(&case.config, &loaded).unwrap();
        assert!(forecast.is_some() && identity.is_some());
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(joint));
        host.check_credibility().unwrap();
        assert_eq!(
            host.identity_report(901).unwrap().outcome,
            IdentityOutcome::Matched
        );
        assert_eq!(
            host.identity_report(902).unwrap().outcome,
            IdentityOutcome::Matched
        );
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 4);
        assert_eq!(numerical.sampled_draws, 2);
        assert_eq!(numerical.cumulative_work.admitted_tokens, 4);
        assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
        assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        assert_eq!(
            host.credential_status().unwrap().unwrap().policy,
            policy(&case.config)
        );
    }
}

#[test]
fn prepared_joint_guard_pins_every_policy_field_and_work_limit_before_recovery_mutation() {
    let root = Root::new();
    let case = Case::new(&root, false);
    let bare = fs::read_to_string(&case.recipe).unwrap();
    identity_fixture::provision(&root, &case.recipe, false);
    let identity_only = fs::read_to_string(&case.recipe).unwrap();
    fs::write(&case.recipe, bare).unwrap();
    select(&root, &case);
    let original = fs::read_to_string(&case.recipe).unwrap();
    let without_joint = credential_envelope(&root, &publication_envelope(&root, &identity_only));
    let prepared = case.prepare();
    let canonical = case.disk();
    let anchor = fs::read(&case.anchor).unwrap();
    // A staging image must survive independently mismatched recovery too.
    let staging = case.config.store.join("delivery.pending");
    fs::write(&staging, b"preserve unrelated pending image").unwrap();
    let mut alternatives = vec![without_joint];
    for (field, before, after) in [
        ("id", 71, 72),
        ("generation", 1, 2),
        ("minimum_safe_roots", 1, 2),
        ("minimum_violation_roots", 1, 2),
        ("maximum_escape_ppm", 0, 1),
        ("maximum_false_stop_ppm", 0, 1),
        ("max_cases", 2, 3),
        ("max_member_outcomes", 8, 9),
    ] {
        let changed = original.replacen(
            &format!("\"{field}\":{before}"),
            &format!("\"{field}\":{after}"),
            1,
        );
        assert_ne!(changed, original, "{field}");
        alternatives.push(changed);
    }
    for document in alternatives {
        fs::write(&case.recipe, document).unwrap();
        let mut loaded = case.reader();
        loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
        assert!(
            recovery::stream_checkpoint::execute(
                &case.config,
                &loaded,
                &capture_request(&prepared),
                &mut |_| Ok(())
            )
            .is_err()
        );
        assert!(recovery::open_selected(&case.config, &loaded).is_err());
        assert_eq!(case.disk(), canonical);
        assert_eq!(fs::read(&case.anchor).unwrap(), anchor);
        assert_eq!(
            fs::read(&staging).unwrap(),
            b"preserve unrelated pending image"
        );
    }
    fs::write(&case.recipe, original).unwrap();
    verify_prepared(&case, &prepared);
    assert_eq!(case.disk(), canonical);
}

#[test]
fn joint_prepared_baseline_cannot_reuse_its_old_identity_for_live_continuation() {
    for missing in [false, true] {
        let root = Root::new();
        let case = Case::new(&root, false);
        select(&root, &case);
        case.prepare();
        let _producer = producer(&root, &case);
        secrets(&root);
        fresh_identity(&case);
        let observed = root.0.join("observed.manifest");
        if missing {
            fs::remove_file(observed).unwrap();
        } else {
            let mut manifest =
                identity_wire::decode_manifest(&fs::read(&observed).unwrap()).unwrap();
            manifest.weights = [9; 32];
            fs::write(observed, identity_wire::encode_manifest(&manifest)).unwrap();
        }
        let (result, packet, retained) =
            continue_prepared(&root, &case, ReviewDecision::Approve, &mut |_| {
                ElapsedTick(1001)
            });
        assert!(result.is_err());
        assert!(packet.is_none());
        let view = case.view();
        assert!(view.publication.stop.is_some());
        assert_eq!(view.publication.executions, 0);
        assert_eq!(view.publication.control.ledger.charged, 0);
        assert_eq!(view.publication.control.ledger.reserved, 0);
        assert_eq!(view.consistency.consistency.evidence.samples(), 0);
        assert_eq!(view.consistency.work, LearnedMonitorWork::default());
        assert!(view.pending_request.is_none());
        let mut loaded = case.reader();
        loaded.recovery_anchor = Some(custody::load(&retained).unwrap());
        let (host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
        assert_eq!(
            host.held_out_joint_policy().unwrap(),
            loaded.generation.required_held_out_joint()
        );
        assert_eq!(
            host.identity_report(901).unwrap().outcome,
            IdentityOutcome::Matched
        );
        if missing {
            assert!(host.identity_report(902).is_err());
        } else {
            assert_eq!(
                host.identity_report(902).unwrap().outcome,
                IdentityOutcome::Mismatch(IdentityMismatch::Manifest)
            );
        }
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 2);
        assert_eq!(numerical.sampled_draws, 0);
        assert_eq!(numerical.cumulative_work.admitted_tokens, 2);
        assert_eq!(numerical.cumulative_work.sampling_attempts, 0);
        assert_eq!(
            host.learned_stream_checkpoint(71).unwrap().info().position,
            2
        );
    }
}

#[test]
fn joint_preparation_cannot_activate_or_manufacture_first_sequence_evidence() {
    let root = Root::new();
    let case = Case::new(&root, false);
    select(&root, &case);
    let path = root.0.join("first-activation.bin");
    let error = prepare::command(
        &case.args(),
        Some(&path),
        || panic!("preparation activation must refuse before time and owner creation"),
        &mut Vec::new(),
    )
    .unwrap_err();
    assert!(error.contains("cannot activate credibility"), "{error}");
    assert!(!case.config.store.exists());
    assert!(!case.anchor.exists());
    let prepared = case.prepare();
    assert_eq!(number(&prepared, "control_sequence"), 0);
    let mut loaded = case.reader();
    loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
    let (mut host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
    let cut = host.inspect();
    let before = host.learned_generation_inspection().unwrap();
    let canonical = case.disk();
    let request = qualification::activation(
        &case.config,
        cut.control.sequence,
        cut.control.ledger.epoch,
        21,
        2,
    );
    assert_eq!(request.expected_control_sequence, 0);
    fs::write(&path, request.encode_reference().unwrap()).unwrap();
    // Use the actual recovered owner cut. Recovery advances its epoch but does
    // not invent control events that could make a positive early Hold timely.
    let error =
        super::super::super::super::super::qualification::activate(&mut host, &case.config, &path)
            .unwrap_err();
    assert!(error.contains("Stale"), "{error}");
    assert_eq!(host.inspect(), cut);
    assert_eq!(host.learned_generation_inspection().unwrap(), before);
    assert_eq!(case.disk(), canonical);
    assert!(host.credibility_change(21).is_err());
    host.check_credibility().unwrap();
    assert_eq!(host.inspect().executions, 0);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 2);
    assert_eq!(numerical.sampled_draws, 0);
    assert_eq!(numerical.cumulative_work.sampling_attempts, 0);
}
