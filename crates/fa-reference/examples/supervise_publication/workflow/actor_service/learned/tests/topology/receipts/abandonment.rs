//! Paired source custody for real learned Unknown dispositions. Historical
//! retries may confirm only the native selected one-event successor.
use super::*;
use crate::workflow::actor_service::learned::abandonment as abandon;
use fa_reference::strict_json::{self, Json, Limits};

fn arguments(case: &Case, output: &Path, source: &Path) -> Vec<String> {
    let request = case.root.0.join("abandon.json");
    fs::write(&request, format!(r#"{{"schema_version":"fa.abandon-unknown/1","operation":900,"attempt":1,"expected_control_sequence":{},"expected_authority_epoch":{}}}"#,
        case.before.control.sequence, case.before.control.ledger.epoch)).unwrap();
    vec![
        "create-learned-generated".into(),
        case.config_path.display().to_string(),
        case.recipe.display().to_string(),
        "--abandon-unknown".into(),
        request.display().to_string(),
        "--history-anchor".into(),
        case.root.0.join("history-1.bin").display().to_string(),
        "--retain-history-anchor".into(),
        output.display().to_string(),
        "--topology-profile".into(),
        case.profile.display().to_string(),
        "--retain-topology-checkpoint".into(),
        source.display().to_string(),
    ]
}

fn retained_pair(case: &Case, output: &Path, source: &Path) -> TopologyCheckpoint {
    let saved = source_file(source);
    assert_eq!(saved.history_anchor(), &anchors::load(output).unwrap());
    assert_eq!(saved.history_anchor().revision(), case.before.revision + 1);
    assert_eq!(saved.minimum().journal_revision, case.before.revision + 1);
    assert_eq!(
        saved.minimum().control_sequence,
        case.saved.minimum().control_sequence
    );
    assert_eq!(
        saved.minimum().authority_epoch,
        case.saved.minimum().authority_epoch
    );
    assert_eq!(saved.available(), case.saved.available());
    assert_eq!(saved.work(), case.saved.work());
    assert_eq!(saved.last_operation(), case.saved.last_operation());
    assert_eq!(saved.topology_bytes(), case.saved.topology_bytes());
    assert_eq!(saved.inventory_bytes(), case.saved.inventory_bytes());
    assert_eq!(saved.expected(), case.saved.expected());
    saved
}

fn report(bytes: &[u8], history: bool, source: bool) -> Json {
    let parsed = strict_json::parse(bytes, Limits::default()).unwrap();
    assert_eq!(
        parsed.get("schema").unwrap().as_str(),
        Some("fa.learned-abandon-unknown-result/1")
    );
    let value = parsed.get("knowledge").unwrap().get("value").unwrap();
    assert_eq!(
        value.get("history_anchor_retained").unwrap().as_bool(),
        Some(history)
    );
    assert_eq!(
        value.get("topology_checkpoint_retained").unwrap().as_bool(),
        Some(source)
    );
    assert_eq!(
        value
            .get("effect_outcome")
            .unwrap()
            .get("kind")
            .unwrap()
            .as_str(),
        Some("unknown")
    );
    parsed
}

#[test]
fn learned_topology_abandonment_preserves_source_work_and_available_cut_across_all_predictors() {
    for mode in [None, Some(false), Some(true)] {
        for published in [false, true] {
            let case = dispatched_at(mode, published, true);
            assert_eq!(case.before.control.ledger.stages[&1], ActionState::Unknown);
            let input = case.root.0.join("history-1.bin");
            let original_history = fs::read(&input).unwrap();
            let original_source = fs::read(case.root.0.join("source-1.bin")).unwrap();
            let output = case.root.0.join("abandoned.anchor");
            let source = case.root.0.join("abandoned.source");
            let args = arguments(&case, &output, &source);
            let mut bytes = Vec::new();
            abandon::command_with(&args, None, &mut || Duration::ZERO, &mut bytes).unwrap();
            report(&bytes, true, true);
            let state = observed(&case.config, &case.loaded);
            assert_eq!(state.revision, case.before.revision + 1);
            assert_eq!(state.control.sequence, case.before.control.sequence);
            assert_eq!(state.control.ledger.epoch, case.before.control.ledger.epoch);
            assert_eq!(
                state.control.ledger.elapsed,
                case.before.control.ledger.elapsed
            );
            assert_eq!(
                state.control.ledger.charged,
                case.before.control.ledger.charged
            );
            assert_eq!(
                state.control.ledger.stages[&1],
                ActionState::IrrecoverablyUnknown
            );
            assert_eq!(state.payload, case.before.payload);
            assert_eq!(state.executions, case.before.executions);
            let saved = retained_pair(&case, &output, &source);
            assert!(
                saved.available(),
                "abandonment cannot invent a recovery Fence"
            );
            assert_eq!(fs::read(&input).unwrap(), original_history);
            assert_eq!(
                fs::read(case.root.0.join("source-1.bin")).unwrap(),
                original_source
            );
            let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
            // The exact old pair is usable only by the restricted disposition
            // consumer; ordinary receipt recovery keeps its exact-extent law.
            let mut loaded = recipe::load(&case.recipe, &case.config, false).unwrap();
            loaded.recovery_anchor = Some(anchors::load(&input).unwrap());
            custody::admit(
                &case.config,
                &mut loaded,
                Some(&case.profile),
                custody::Mode::Historical,
            )
            .unwrap();
            assert!(query::read(&case.config, &loaded, None, &mut || Duration::ZERO).is_err());
            let next_profile = profile_for(&case, &saved, &source, "abandoned-profile.json");
            loaded.topology = None;
            loaded.recovery_anchor = Some(saved.history_anchor().clone());
            custody::admit(
                &case.config,
                &mut loaded,
                Some(&next_profile),
                custody::Mode::Historical,
            )
            .unwrap();
            assert!(matches!(
                query::read(&case.config, &loaded, None, &mut || Duration::ZERO)
                    .unwrap()
                    .result,
                Ok(Knowledge::Unknown {
                    reason: UnknownReason::OutcomeUnknown
                })
            ));
            assert_eq!(
                fs::read(case.config.store.join("delivery.bin")).unwrap(),
                canonical
            );
            assert_no_live_inputs(&case);
        }
    }
}

#[test]
fn learned_topology_abandonment_retains_both_independent_outputs_after_export_failure_or_late_deadline()
 {
    for (history_fails, source_fails) in [(true, false), (false, true), (true, true)] {
        let case = dispatched_at(Some(true), true, true);
        let output = case.root.0.join("first.anchor");
        let source = case.root.0.join("first.source");
        let args = arguments(&case, &output, &source);
        let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
        let mut raced = false;
        let mut bytes = Vec::new();
        let error = abandon::command_with(
            &args,
            None,
            &mut || {
                if !raced && fs::read(case.config.store.join("delivery.bin")).unwrap() != canonical
                {
                    if history_fails {
                        fs::write(&output, b"history winner").unwrap();
                    }
                    if source_fails {
                        fs::write(&source, b"source winner").unwrap();
                    }
                    raced = true;
                }
                if raced {
                    Duration::from_secs(3600)
                } else {
                    Duration::ZERO
                }
            },
            &mut bytes,
        )
        .unwrap_err();
        assert!(error.contains(&format!(
            "acknowledged at revision {}",
            case.before.revision + 1
        )));
        assert!(error.contains("deadline reached after native acknowledgment"));
        report(&bytes, !history_fails, !source_fails);
        if history_fails {
            assert_eq!(fs::read(&output).unwrap(), b"history winner");
        } else {
            assert_eq!(
                anchors::load(&output).unwrap().revision(),
                case.before.revision + 1
            );
        }
        if source_fails {
            assert_eq!(fs::read(&source).unwrap(), b"source winner");
        } else {
            assert_eq!(
                source_file(&source).history_anchor().revision(),
                case.before.revision + 1
            );
        }
        let abandoned = fs::read(case.config.store.join("delivery.bin")).unwrap();
        // Interrupted export retries the exact original request from the old
        // paired cut. A second disposition, recovery Fence or source read is forbidden.
        let retry_history = case.root.0.join("retry.anchor");
        let retry_source = case.root.0.join("retry.source");
        let retry_args = arguments(&case, &retry_history, &retry_source);
        let mut retry_report = Vec::new();
        abandon::command_with(&retry_args, None, &mut || Duration::ZERO, &mut retry_report)
            .unwrap();
        report(&retry_report, true, true);
        retained_pair(&case, &retry_history, &retry_source);
        assert_eq!(
            fs::read(case.config.store.join("delivery.bin")).unwrap(),
            abandoned
        );
        assert_no_live_inputs(&case);
    }
}

#[test]
fn learned_topology_abandonment_refuses_conflicting_retry_and_unrelated_successor_before_cleanup() {
    let case = dispatched_at(Some(false), false, true);
    let output = case.root.0.join("abandoned.anchor");
    let source = case.root.0.join("abandoned.source");
    let args = arguments(&case, &output, &source);
    abandon::command_with(&args, None, &mut || Duration::ZERO, &mut Vec::new()).unwrap();
    let canonical = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let pending = case.config.store.join("delivery.pending");
    fs::write(&pending, b"keep conflicting retry staging").unwrap();
    let mut retry_args = arguments(
        &case,
        &case.root.0.join("retry.anchor"),
        &case.root.0.join("retry.source"),
    );
    let request = Path::new(&retry_args[4]);
    let original = fs::read_to_string(request).unwrap();
    fs::write(
        request,
        original.replace("\"operation\":900", "\"operation\":901"),
    )
    .unwrap();
    assert!(
        abandon::command_with(&retry_args, None, &mut || Duration::ZERO, &mut Vec::new()).is_err()
    );
    assert_eq!(
        fs::read(&pending).unwrap(),
        b"keep conflicting retry staging"
    );
    assert_eq!(
        fs::read(case.config.store.join("delivery.bin")).unwrap(),
        canonical
    );
    fs::write(request, original).unwrap();
    // Advance through an independently retained CURRENT pair using the original
    // live recovery Fence; the stale pre-abandonment pair cannot cover it.
    let saved = retained_pair(&case, &output, &source);
    let profile = profile_for(&case, &saved, &source, "current-profile.json");
    let mut loaded = recipe::load(&case.recipe, &case.config, false).unwrap();
    loaded.recovery_anchor = Some(saved.history_anchor().clone());
    custody::admit(
        &case.config,
        &mut loaded,
        Some(&profile),
        custody::Mode::Historical,
    )
    .unwrap();
    let recovered = recovery::open_selected(&case.config, &loaded).unwrap();
    drop(recovered);
    let advanced = fs::read(case.config.store.join("delivery.bin")).unwrap();
    assert_ne!(advanced, canonical);
    fs::write(&pending, b"keep unrelated successor staging").unwrap();
    retry_args[8] = case.root.0.join("unrelated.anchor").display().to_string();
    retry_args[12] = case.root.0.join("unrelated.source").display().to_string();
    assert!(
        abandon::command_with(&retry_args, None, &mut || Duration::ZERO, &mut Vec::new()).is_err()
    );
    assert_eq!(
        fs::read(&pending).unwrap(),
        b"keep unrelated successor staging"
    );
    assert_eq!(
        fs::read(case.config.store.join("delivery.bin")).unwrap(),
        advanced
    );
    assert!(!Path::new(&retry_args[8]).exists());
    assert!(!Path::new(&retry_args[12]).exists());
}
