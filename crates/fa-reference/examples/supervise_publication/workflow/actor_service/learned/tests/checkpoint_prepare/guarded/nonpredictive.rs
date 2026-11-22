//! Ordinary prompt preparation uses the original numerical owner without
//! installing a predictor. These finite models test custody, not calibration.
use super::super::super::{identity_tests as identity_fixture, mediated_topology};
use super::*;
use fa_reference::action::consequence::oversight::identity::IdentityOutcome;

#[path = "nonpredictive/mediated.rs"]
mod mediated;

fn selected(root: &Root, campaign: bool) -> Case {
    let case = Case::selected(root, campaign, false);
    unpredicted(&case.reader());
    assert!(!root.0.join("forecast.json").exists());
    case
}

fn unpredicted(loaded: &Loaded) {
    assert!(loaded.generation.required_pre_output_forecast().is_none());
    assert!(
        loaded
            .generation
            .required_owned_pre_output_forecast()
            .is_none()
    );
    assert!(
        !loaded
            .generation
            .requires_owned_forecast_stream_restart_audit()
    );
}

fn view(case: &Case) -> FileDeliverySnapshot {
    let loaded = case.reader();
    unpredicted(&loaded);
    FileOversight::read_publication_with_learned_generation(
        &case.config.store,
        &case.config.profile,
        &loaded.generation,
    )
    .unwrap()
}

fn prepared(case: &Case, report: &Json) {
    for (name, expected) in [
        ("position", 2),
        ("sampled_draws", 0),
        ("sampling_attempts", 0),
        ("checkpoints", 1),
        ("restart_attempts", 0),
        ("control_sequence", 0),
        ("authority_epoch", 0),
    ] {
        assert_eq!(number(report, name), expected, "{name}");
    }
    assert_eq!(
        report
            .get("history_anchor_retained")
            .and_then(Json::as_bool),
        Some(true)
    );
    let state = view(case);
    quiet(&state);
    assert_eq!(state.revision, number(report, "revision"));
    assert_eq!(
        custody::load(&case.anchor).unwrap().revision(),
        state.revision
    );
    no_secret(&case.disk());
}

fn numerical(host: &FileOversight, position: u64, draws: u64) {
    assert!(!host.action_consistency_required());
    assert!(!host.learned_action_consistency_required());
    assert!(!host.owned_learned_action_consistency_required());
    let original = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(original.position, position);
    assert_eq!(original.sampled_draws, draws);
    assert_eq!(original.cumulative_work.admitted_tokens, position);
    assert_eq!(original.cumulative_work.sampling_attempts, draws);
    assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    let capture = host.learned_stream_checkpoint(71).unwrap();
    assert_eq!(capture.info().position, 2);
    assert_eq!(capture.info().sampled_draws, 0);
    assert_eq!(capture.info().authority_epoch, 0);
    assert_eq!(capture.info().control_sequence, 0);
}

#[test]
fn ordinary_preparation_retains_plain_or_fully_guarded_state_then_requires_fresh_review() {
    for (guarded, decision) in [
        (false, ReviewDecision::Approve),
        (true, ReviewDecision::Approve),
        (true, ReviewDecision::Reject),
    ] {
        let root = Root::new();
        let case = selected(&root, false);
        if guarded {
            identity_fixture::provision(&root, &case.recipe, true);
            select_guards(&root, &case, true, true);
        }
        let original = case.reader().generation;
        let helper = root.0.join("helper-weights.safetensors");
        let weights = fs::read(&helper).unwrap();
        fs::remove_file(&helper).unwrap();
        assert!(!root.0.join("producer").exists());
        assert!(!root.0.join("broker.bin").exists());
        assert!(!root.0.join("provider.bin").exists());
        let report = case.prepare();
        prepared(&case, &report);
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        // Reacquiring the same original capture is an exact historical retry;
        // it performs no Fence, helper work or additional prompt inference.
        let canonical = case.disk();
        let mut loaded = case.reader();
        loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
        loaded.anchor_output = Some(root.0.join("capture-retry.anchor"));
        let captured = recovery::stream_checkpoint::execute(
            &case.config,
            &loaded,
            &capture_request(&report),
            &mut |_| Ok(()),
        )
        .unwrap();
        assert_eq!(captured.appended_events(), 0);
        assert_eq!(captured.numerical().position, 2);
        assert_eq!(captured.numerical().cumulative_work.admitted_tokens, 2);
        assert_eq!(captured.numerical().cumulative_work.sampling_attempts, 0);
        assert_eq!(case.disk(), canonical);
        fs::write(helper, weights).unwrap();
        let _producer = guarded.then(|| producer(&root, &case));
        if guarded {
            secrets(&root);
            let original_recipe = fs::read_to_string(&case.recipe).unwrap();
            fs::write(
                &case.recipe,
                original_recipe
                    .replacen("\"check\":901", "\"check\":902", 1)
                    .replacen(
                        "\"measurement_sequence\":1",
                        "\"measurement_sequence\":2",
                        1,
                    ),
            )
            .unwrap();
        }
        let (result, packet, retained) =
            continue_prepared(&root, &case, decision, &mut |_| ElapsedTick(1001));
        let packet = packet.expect("prepared numerical state cannot replace a human decision");
        original_packet(&packet);
        assert_eq!(
            result.is_ok(),
            decision == ReviewDecision::Approve,
            "{result:?}"
        );
        let state = view(&case);
        assert_eq!(
            state.executions,
            u64::from(decision == ReviewDecision::Approve)
        );
        assert_eq!(
            state.control.ledger.charged,
            if decision == ReviewDecision::Approve {
                packet.action().spec().units
            } else {
                0
            }
        );
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(
            state.control.ledger.available + state.control.ledger.charged,
            case.config.profile.delivery.total
        );
        let mut loaded = case.reader();
        assert_eq!(loaded.generation, original);
        loaded.recovery_anchor = Some(custody::load(&retained).unwrap());
        let (host, _, forecast, identity) = recovery::open_selected(&case.config, &loaded).unwrap();
        assert!(forecast.is_none());
        assert_eq!(identity.is_some(), guarded);
        numerical(&host, 4, 2);
        assert_eq!(
            host.held_out_joint_policy().unwrap(),
            original.required_held_out_joint()
        );
        host.check_credibility().unwrap();
        if guarded {
            assert_eq!(
                host.identity_report(901).unwrap().outcome,
                IdentityOutcome::Matched
            );
            assert_eq!(
                host.identity_report(902).unwrap().outcome,
                IdentityOutcome::Matched
            );
            assert_eq!(
                host.credential_status().unwrap().unwrap().policy,
                policy(&case.config)
            );
        }
        let stream = host.stream_snapshot().unwrap();
        assert_eq!(
            stream.confirmed.messages().count(),
            usize::from(decision == ReviewDecision::Approve)
        );
        assert_eq!(stream.confirmed, stream.published);
        assert!(!stream.confirmed.finished());
        no_secret(&case.disk());
    }
}

#[test]
fn ordinary_preparation_keeps_campaign_inventory_exact_before_recovery_mutation() {
    let root = Root::new();
    let case = selected(&root, true);
    fs::remove_file(root.0.join("helper-weights.safetensors")).unwrap();
    let report = case.prepare();
    prepared(&case, &report);
    let canonical = case.disk();
    let anchor = fs::read(&case.anchor).unwrap();
    let mut loaded = case.reader();
    loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
    loaded.anchor_output = Some(root.0.join("capture-retry.anchor"));
    let expected = loaded.expected_campaigns.take();
    assert!(
        recovery::stream_checkpoint::execute(
            &case.config,
            &loaded,
            &capture_request(&report),
            &mut |_| Ok(()),
        )
        .is_err()
    );
    assert_eq!(case.disk(), canonical);
    assert_eq!(fs::read(&case.anchor).unwrap(), anchor);
    loaded.expected_campaigns = expected;
    let captured = recovery::stream_checkpoint::execute(
        &case.config,
        &loaded,
        &capture_request(&report),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(captured.appended_events(), 0);
    assert_eq!(captured.numerical().cumulative_work.admitted_tokens, 2);
    assert_eq!(case.disk(), canonical);
}

#[test]
fn ordinary_preparation_stays_unsampled_and_does_not_admit_legacy_predictors() {
    for cause in ["sampled", "raw", "owned"] {
        let root = Root::new();
        let case = selected(&root, false);
        let original = fs::read(&case.recipe).unwrap();
        if cause == "sampled" {
            fs::write(&case.selection, document(2, 1)).unwrap();
        } else {
            super::super::super::forecast::install(&root, &case.recipe, cause == "owned", 60_000);
        }
        let error = prepare::command(
            &case.args(),
            None,
            || panic!("ineligible preparation must refuse before time or owner creation"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(
            error.contains(if cause == "sampled" {
                "zero sampled draws"
            } else {
                "explicit owned prompt-audit reset recipe"
            }),
            "{cause}: {error}"
        );
        assert!(!case.config.store.exists());
        assert!(!case.anchor.exists());
        fs::write(&case.recipe, original).unwrap();
        fs::write(&case.selection, document(2, 0)).unwrap();
        fs::remove_file(root.0.join("helper-weights.safetensors")).unwrap();
        let report = case.prepare();
        prepared(&case, &report);
        unpredicted(&case.reader());
    }
}

#[test]
fn ordinary_prepared_prompt_cannot_reuse_historical_policy_source_for_new_generation() {
    let root = Root::new();
    let case = selected(&root, false);
    let report = case.prepare();
    prepared(&case, &report);
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    let (result, packet, retained) =
        continue_prepared(&root, &case, ReviewDecision::Approve, &mut |_| {
            ElapsedTick(1001)
        });
    assert!(result.is_err());
    assert!(packet.is_none());
    let state = view(&case);
    assert!(state.stop.is_some());
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 0);
    assert_eq!(state.control.ledger.reserved, 0);
    let mut loaded = case.reader();
    loaded.recovery_anchor = Some(custody::load(&retained).unwrap());
    let (host, _, forecast, _) = recovery::open_selected(&case.config, &loaded).unwrap();
    assert!(forecast.is_none());
    numerical(&host, 2, 0);
}
