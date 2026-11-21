//! Actual independent model measurement before unsampled prompt preparation.
//! These finite model fixtures exercise custody and enforcement, not H19.
use super::super::identity_tests as identity_fixture;
use super::*;
use fa_reference::action::consequence::activation::{
    identity::wire as identity_wire, tensor::kv::decoder::MAX_DECODER_PRODUCTS,
};
use fa_reference::action::consequence::oversight::identity::{IdentityOutcome, IdentityStatus};

fn select(root: &Root, case: &Case) {
    identity_fixture::provision(root, &case.recipe, false);
}

fn fresh_selection(case: &Case) {
    let document = fs::read_to_string(&case.recipe).unwrap();
    fs::write(
        &case.recipe,
        document
            .replacen("\"check\":901", "\"check\":902", 1)
            .replacen(
                "\"measurement_sequence\":1",
                "\"measurement_sequence\":2",
                1,
            ),
    )
    .unwrap();
}

fn capture_request(prepared: &Json) -> stream_checkpoint::Request {
    stream_checkpoint::Request::Capture(
        FileLearnedStreamCheckpointRequest::new(
            number(prepared, "revision") - 1,
            FileLearnedCheckpointInfo {
                checkpoint: 71,
                actor_revision: number(prepared, "actor_revision"),
                authority_epoch: number(prepared, "authority_epoch"),
                control_sequence: number(prepared, "control_sequence"),
                stream: number(prepared, "stream"),
                position: 2,
                sampled_draws: 0,
            },
        )
        .unwrap(),
    )
}

fn substitute_anchor_weight(root: &Root) {
    let path = root.0.join("weights.safetensors");
    let mut bytes = fs::read(&path).unwrap();
    let header = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    let parsed = strict_json::parse(&bytes[8..8 + header], Limits::default()).unwrap();
    let offset = parsed
        .get("model.embed_tokens.weight")
        .unwrap()
        .get("data_offsets")
        .unwrap()
        .as_array()
        .unwrap()[0]
        .as_u64()
        .unwrap() as usize;
    // This stimulus token is outside the actor's prompt and output. Model IDs,
    // manifest, passport, fitted codec and publication parameters stay fixed.
    let at = 8 + header + offset + 122 * 2 * 4;
    assert_eq!(&bytes[at..at + 4], &1.0_f32.to_le_bytes());
    bytes[at..at + 4].copy_from_slice(&(-1.0_f32).to_le_bytes());
    fs::write(path, bytes).unwrap();
}

#[test]
fn measured_preparation_retains_the_exact_unsampled_checkpoint_without_helper_assets() {
    let root = Root::new();
    let case = Case::new(&root, false);
    select(&root, &case);
    fs::remove_file(root.0.join("helper-weights.safetensors")).unwrap();
    let prepared = case.prepare();
    assert_eq!(number(&prepared, "position"), 2);
    assert_eq!(number(&prepared, "sampled_draws"), 0);
    assert_eq!(number(&prepared, "sampling_attempts"), 0);
    assert_eq!(number(&prepared, "checkpoints"), 1);
    assert_eq!(number(&prepared, "restart_attempts"), 0);
    quiet(&case.view().publication);
    assert_eq!(case.view().consistency.consistency.evidence.samples(), 0);
    assert_eq!(case.view().consistency.work, LearnedMonitorWork::default());
    assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);

    let before = case.disk();
    let mut loaded = case.reader();
    loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
    let captured = recovery::stream_checkpoint::execute(
        &case.config,
        &loaded,
        &capture_request(&prepared),
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(captured.appended_events(), 0);
    assert_eq!(captured.numerical().position, 2);
    assert_eq!(captured.numerical().cumulative_work.admitted_tokens, 2);
    assert_eq!(case.disk(), before);

    // The exact original registration yields a fresh observer after recovery,
    // but the preparation match is now historical and cannot authorize work.
    fs::remove_file(root.0.join("observed.manifest")).unwrap();
    let (mut host, _, forecast, identity) = recovery::open_selected(&case.config, &loaded).unwrap();
    assert!(forecast.is_some());
    assert!(identity.is_some());
    assert_eq!(
        host.identity_report(901).unwrap().outcome,
        IdentityOutcome::Matched
    );
    assert!(host.identity_installation(901).unwrap().is_some());
    // Recovery must observe current native time before reporting live identity.
    assert!(!host.clock_ready());
    assert!(host.identity_status().is_err());
    let revision = host.revision();
    host.observe_time(revision, ElapsedTick(1001)).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        *captured.numerical()
    );
    assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
}

#[test]
fn prepared_identity_needs_a_fresh_measurement_and_both_publication_keys() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new();
        let case = Case::new(&root, false);
        select(&root, &case);
        case.prepare();
        fresh_selection(&case);
        let (peer_path, peers) = super::super::functional_creation::peers_file(&root, &case.config);
        let output = root.0.join("identity-continued.anchor");
        let args = vec![
            "create-learned-generated".into(),
            case.config_path.display().to_string(),
            case.recipe.display().to_string(),
            peer_path.display().to_string(),
            "--continue-generation".into(),
            "--history-anchor".into(),
            case.anchor.display().to_string(),
            "--retain-history-anchor".into(),
            output.display().to_string(),
        ];
        let mut human = None;
        let result = super::super::super::command_with_clock(&args, None, || {
            if human.is_none() && peers.socket(1).exists() {
                human = Some(super::super::peer_intake::human(peers.clone(), 1, decision));
            }
            ElapsedTick(1001)
        });
        let packet = human
            .expect("identity never supplies the independent human key")
            .join()
            .unwrap();
        assert_eq!(
            result.is_ok(),
            decision == ReviewDecision::Approve,
            "{result:?}"
        );
        assert_eq!(packet.views().len(), 2);
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
        if decision == ReviewDecision::Approve {
            let mut loaded = case.reader();
            loaded.recovery_anchor = Some(custody::load(&output).unwrap());
            let (host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
            assert_eq!(
                host.identity_report(901).unwrap().outcome,
                IdentityOutcome::Matched
            );
            assert_eq!(
                host.identity_report(902).unwrap().outcome,
                IdentityOutcome::Matched
            );
            assert!(host.identity_installation(902).unwrap().is_some());
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.position, 4);
            assert_eq!(numerical.sampled_draws, 2);
            assert_eq!(numerical.cumulative_work.admitted_tokens, 4);
            assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
            assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        }
    }
}

#[test]
fn model_or_manifest_mismatch_and_missing_measurement_never_prepare_a_checkpoint() {
    for variant in 0..4 {
        let root = Root::new();
        let case = Case::new(&root, false);
        select(&root, &case);
        match variant {
            0 => substitute_anchor_weight(&root),
            1 => {
                let path = root.0.join("observed.manifest");
                let mut manifest =
                    identity_wire::decode_manifest(&fs::read(&path).unwrap()).unwrap();
                manifest.weights = [9; 32];
                fs::write(path, identity_wire::encode_manifest(&manifest)).unwrap();
            }
            2 => fs::remove_file(root.0.join("observed.manifest")).unwrap(),
            _ => {
                let text = fs::read_to_string(&case.recipe).unwrap();
                fs::write(
                    &case.recipe,
                    text.replacen(
                        &format!("\"scalar_products\":{MAX_DECODER_PRODUCTS}"),
                        "\"scalar_products\":1",
                        1,
                    ),
                )
                .unwrap();
            }
        }
        let mut output = Vec::new();
        let failure =
            prepare::command(&case.args(), None, || ElapsedTick(1000), &mut output).unwrap_err();
        if variant <= 1 {
            assert!(
                failure.contains("computed learned identity refused"),
                "{failure}"
            );
        }
        assert!(output.is_empty());
        let view = case.view();
        assert!(view.publication.stop.is_some());
        assert_eq!(view.publication.executions, 0);
        assert!(view.publication.payload.is_empty());
        assert_eq!(view.publication.control.ledger.charged, 0);
        assert_eq!(view.publication.control.ledger.reserved, 0);
        assert_eq!(view.consistency.consistency.evidence.samples(), 0);
        assert_eq!(view.consistency.work, LearnedMonitorWork::default());
        assert!(view.pending_request.is_none());
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
        // The terminal result retains its own acknowledged custody; failure
        // cannot claim the selected reusable prompt checkpoint was prepared.
        assert_eq!(
            custody::load(&case.anchor).unwrap().revision(),
            view.publication.revision
        );
    }
}

#[test]
fn original_identity_cannot_be_omitted_or_changed_for_preparation_or_recovery() {
    let root = Root::new();
    let case = Case::new(&root, false);
    let bare = fs::read_to_string(&case.recipe).unwrap();
    select(&root, &case);
    let wrapped = fs::read_to_string(&case.recipe).unwrap();
    let selection = Selection::read(&case.selection).unwrap();
    let mut missing_measurement = case.reader();
    missing_measurement.anchor_output = Some(case.anchor.clone());
    missing_measurement.identity = None;
    assert!(prepare::check_selection(&case.config, &missing_measurement, selection).is_err());
    let mut unregistered = case.reader();
    fs::write(&case.recipe, &bare).unwrap();
    unregistered.generation = case.reader().generation;
    unregistered.anchor_output = Some(case.anchor.clone());
    assert!(prepare::check_selection(&case.config, &unregistered, selection).is_err());
    assert!(!case.config.store.exists());
    fs::write(&case.recipe, &wrapped).unwrap();
    let prepared = case.prepare();
    let before = case.disk();
    let retained = fs::read(&case.anchor).unwrap();
    for document in [
        bare,
        wrapped.replacen("\"observer_id\":99", "\"observer_id\":100", 1),
        wrapped.replacen("\"max_checks\":8", "\"max_checks\":9", 1),
    ] {
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
        assert_eq!(case.disk(), before);
        assert_eq!(fs::read(&case.anchor).unwrap(), retained);
    }
    fs::write(&case.recipe, wrapped).unwrap();
    let mut loaded = case.reader();
    loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
    assert_eq!(
        recovery::stream_checkpoint::execute(
            &case.config,
            &loaded,
            &capture_request(&prepared),
            &mut |_| Ok(())
        )
        .unwrap()
        .appended_events(),
        0
    );
    assert_eq!(case.disk(), before);
}

#[test]
fn preparation_charges_its_original_passport_to_the_shared_asset_allowance() {
    let root = Root::new();
    let case = Case::new(&root, false);
    select(&root, &case);
    let names = [
        "identity.passport",
        "fit-binding.json",
        "learned-monitor.json",
        "forecast.json",
        "model.json",
        "tokenizer.json",
        "sampling.json",
        "prompt.txt",
        "weights.safetensors",
        "fit.fakv",
    ];
    let total: u64 = names
        .into_iter()
        .map(|name| fs::metadata(root.0.join(name)).unwrap().len())
        .sum();
    let original = fs::read_to_string(&case.recipe).unwrap();
    fs::remove_file(root.0.join("helper-weights.safetensors")).unwrap();
    fs::write(
        &case.recipe,
        original.replace(
            "\"asset_bytes\":4194304",
            &format!("\"asset_bytes\":{}", total - 1),
        ),
    )
    .unwrap();
    assert!(
        prepare::command(
            &case.args(),
            None,
            || panic!("exhausted original asset allowance reached creation"),
            &mut Vec::new()
        )
        .is_err()
    );
    assert!(!case.config.store.exists());
    assert!(!case.anchor.exists());
    fs::write(
        &case.recipe,
        original.replace(
            "\"asset_bytes\":4194304",
            &format!("\"asset_bytes\":{total}"),
        ),
    )
    .unwrap();
    let loaded = recipe::load_for_preparation(&case.recipe, &case.config).unwrap();
    assert!(loaded.identity.is_some());
    assert!(loaded.native.is_none());
    let prepared = case.prepare();
    assert_eq!(number(&prepared, "position"), 2);
    assert_eq!(number(&prepared, "sampled_draws"), 0);
}
