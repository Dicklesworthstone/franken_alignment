//! Original mediated preparation and reset followed by independently reviewed
//! live publication. All artifacts, source checks and numerical work are real.
use super::*;
use super::super::{anchors as custody, checkpoint_prepare as preparation,
    stream_checkpoint as control, stream_reset as reset};
use fa_reference::action::consequence::delivery::{stream::ReleaseFrame,
    persistent::observed::{driver::topology::checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES},
        reviewer::wire::ReviewDecision}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{io::{self, Cursor, Write}, path::{Path, PathBuf}};

#[path = "mediated_predictive_stream_reset/fixture.rs"]
mod fixture;
use fixture::{Case, Pair, info};

fn checkpoint(path: &Path) -> TopologyCheckpoint {
    TopologyCheckpoint::read_trusted(&mut Cursor::new(fs::read(path).unwrap()),
        MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap()
}
fn report(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn number(value: &Json, key: &str) -> u64 { text(value, key).parse().unwrap() }
fn text<'a>(value: &'a Json, key: &str) -> &'a str { value.get(key).and_then(Json::as_str).unwrap() }
fn control_command(args: &[String]) -> Json {
    let mut output = Vec::new();
    control::command(args, None, || ElapsedTick(1000), &mut output).unwrap();
    report(&output)
}
fn reset_command(args: &[String]) -> Json {
    let mut output = Vec::new();
    reset::command(args, None, || ElapsedTick(1000), &mut output).unwrap();
    report(&output)
}
fn unchanged_source(before: &TopologyCheckpoint, pair: &Pair, available: bool) -> TopologyCheckpoint {
    let after = checkpoint(&pair.source);
    assert_eq!(before.topology_bytes(), after.topology_bytes());
    assert_eq!(before.inventory_bytes(), after.inventory_bytes());
    assert_eq!(before.expected(), after.expected());
    assert_eq!(before.work(), after.work());
    assert_eq!(before.last_operation(), after.last_operation());
    assert_eq!(after.available(), available);
    assert_eq!(after.history_anchor(), &custody::load(&pair.history).unwrap());
    after
}

/// No later phase guesses an input pair from the journal. Every command consumes
/// the previous acknowledged archives and an independently complete request.
fn contained(case: &Case) -> (Pair, PathBuf, Json) {
    let prepared = case.prepare(); let cut = info(&prepared);
    let original = checkpoint(&case.pair("prepared").source);
    assert!(original.available()); assert!(original.work().checks > 0);
    let narrow = case.narrow(cut);
    let narrowed = case.pair("narrowed");
    let command = case.control_args("--narrow-stream-checkpoint", &narrow,
        &case.pair("prepared"), &narrowed, "narrow");
    let result = control_command(&command);
    assert_eq!(text(&result, "outcome"), "narrowed");
    assert_eq!(number(&result, "appended_events"), 1);
    assert_eq!(number(&result, "sampled_draws"), 0);
    let first = number(&result, "first_incident_sequence");
    assert!(first > cut.control_sequence);
    unchanged_source(&original, &narrowed, true);
    let reset = case.reset(cut, first);
    let pending = case.pair("pending");
    let begun = control_command(&case.control_args("--begin-stream-reset", &reset,
        &narrowed, &pending, "begin"));
    assert_eq!(text(&begun, "outcome"), "pending");
    unchanged_source(&original, &pending, true);
    let complete = case.pair("complete");
    let result = reset_command(&case.control_args("--complete-stream-reset", &reset,
        &pending, &complete, "complete"));
    assert_eq!(text(&result, "outcome"), "restored");
    assert_eq!(number(&result, "restart_attempts"), 1);
    assert_eq!(number(&result, "sampled_draws"), 0);
    assert_eq!(number(&result, "incident_count"), 1);
    assert_eq!(result.get("paused").and_then(Json::as_bool), Some(true));
    unchanged_source(&original, &complete, false);
    let state = case.view();
    assert_eq!(state.publication.executions, 0);
    assert!(state.consistency.consistency.pending_attempt.is_none());
    assert_eq!(state.consistency.consistency.evidence.samples(), 0);
    assert_eq!(state.consistency.retained_source_bytes, 0);
    assert!(!state.consistency.consistency.coverage_lost);
    (complete, reset, result)
}

#[test]
fn mediated_owned_operator_prepares_contains_restores_then_requires_new_cut_forecast_and_both_keys() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new(); let case = Case::new(&root);
        let (complete, _, _) = contained(&case);
        let prior = checkpoint(&complete.source);
        let published = case.pair("published");
        let profile = case.profile(&complete, &published, "continue", Some((2, false)));
        let (peer_path, peers) = super::functional_creation::peers_file(&root, &case.config);
        let args = vec!["create-learned-generated".into(), case.config_path.display().to_string(),
            case.recipe.display().to_string(), peer_path.display().to_string(),
            "--continue-generation".into(), "--history-anchor".into(), complete.history.display().to_string(),
            "--retain-history-anchor".into(), published.history.display().to_string(),
            "--topology-profile".into(), profile.display().to_string()];
        let mut human = None;
        let result = super::super::command_with_clock(&args, None, || {
            if human.is_none() && peers.socket(1).exists() {
                human = Some(super::peer_intake::human(peers.clone(), 1, decision));
            }
            ElapsedTick(1001)
        });
        let packet = human.expect("reset does not provide the separate human approval").join().unwrap();
        assert_eq!(result.is_ok(), decision == ReviewDecision::Approve, "{result:?}");
        assert_eq!(packet.views().len(), 2);
        assert_eq!(ReleaseFrame::decode(&packet.action().spec().payload).unwrap().message(), Some("A"));
        let state = case.view();
        assert_eq!(state.consistency.consistency.evidence.samples(), 1);
        assert!(state.consistency.work.probe_coordinates > 0);
        assert_eq!(state.publication.executions, u64::from(decision == ReviewDecision::Approve));
        assert_eq!(state.publication.control.ledger.charged,
            if decision == ReviewDecision::Approve { packet.action().spec().units } else { 0 });
        let source = checkpoint(&published.source);
        assert_eq!(source.history_anchor(), &custody::load(&published.history).unwrap());
        assert!(source.work().checks > prior.work().checks);
        assert!(source.last_operation() > prior.last_operation());
        assert_eq!(source.expected().graph().spec().generation, 2);
    }
}

#[test]
fn offline_controls_retain_exact_source_work_and_retries_need_no_live_documents() {
    let root = Root::new(); let case = Case::new(&root);
    let prepared = case.prepare(); let cut = info(&prepared);
    let original = checkpoint(&case.pair("prepared").source);
    let capture = case.root.join("capture-retry.json");
    fs::write(&capture, format!(r#"{{"schema":"fa.learned-stream-checkpoint/1","expected_journal_revision":"{}",
        "checkpoint":"{}","expected_actor_revision":"{}","expected_authority_epoch":"{}","expected_control_sequence":"{}",
        "expected_stream":"{}","expected_position":"{}","expected_sampled_draws":"{}"}}"#,
        number(&prepared, "revision") - 1, cut.checkpoint, cut.actor_revision, cut.authority_epoch,
        cut.control_sequence, cut.stream, cut.position, cut.sampled_draws)).unwrap();
    let canonical = case.disk();
    let result = control_command(&case.control_args("--capture-stream-checkpoint", &capture,
        &case.pair("prepared"), &case.pair("capture-retry"), "capture"));
    assert_eq!(text(&result, "outcome"), "already_captured");
    assert_eq!(number(&result, "appended_events"), 0);
    assert_eq!(case.disk(), canonical);
    unchanged_source(&original, &case.pair("capture-retry"), true);
    let narrow = case.narrow(cut);
    for name in ["live-1.json", "inventory-1.json", "evidence.json", "native-roster.json"] {
        fs::remove_file(case.root.join(name)).unwrap();
    }
    let narrowed = case.pair("narrowed");
    let result = control_command(&case.control_args("--narrow-stream-checkpoint", &narrow,
        &case.pair("prepared"), &narrowed, "narrow"));
    let canonical = case.disk();
    let retry = control_command(&case.control_args("--narrow-stream-checkpoint", &narrow,
        &case.pair("prepared"), &case.pair("narrow-retry"), "narrow-retry"));
    assert_eq!(text(&retry, "outcome"), "already_narrowed");
    assert_eq!(number(&retry, "appended_events"), 0);
    assert_eq!(case.disk(), canonical);
    unchanged_source(&original, &case.pair("narrow-retry"), true);
    let reset = case.reset(cut, number(&result, "first_incident_sequence"));
    let pending = case.pair("pending");
    control_command(&case.control_args("--begin-stream-reset", &reset, &narrowed, &pending, "begin"));
    let canonical = case.disk();
    let retry = control_command(&case.control_args("--begin-stream-reset", &reset,
        &narrowed, &case.pair("begin-retry"), "begin-retry"));
    assert_eq!(text(&retry, "outcome"), "already_pending");
    assert_eq!(case.disk(), canonical);
    let complete = case.pair("complete");
    let first = reset_command(&case.control_args("--complete-stream-reset", &reset, &pending, &complete, "complete"));
    let retried = case.pair("complete-retry");
    let second = reset_command(&case.control_args("--complete-stream-reset", &reset, &pending, &retried, "complete-retry"));
    for key in ["restart_attempts", "reserved_cache_values", "values_restored", "incident_count", "sampled_draws"] {
        assert_eq!(number(&first, key), number(&second, key));
    }
    assert_eq!(number(&second, "revision"), number(&first, "revision") + 1, "only the ordinary recovery Fence repeats");
    unchanged_source(&original, &retried, false);
    assert!(!case.root.join("missing-next.json").exists());
    assert_eq!(case.view().consistency.consistency.evidence.samples(), 0);
}

#[test]
fn paired_control_rejects_wrong_cut_changed_initial_graph_and_omitted_topology_without_mutation() {
    let root = Root::new(); let case = Case::new(&root);
    let prepared = case.prepare(); let cut = info(&prepared);
    let narrow = case.narrow(cut); let prepared_pair = case.pair("prepared");
    let narrowed = case.pair("narrowed");
    control_command(&case.control_args("--narrow-stream-checkpoint", &narrow,
        &prepared_pair, &narrowed, "narrow"));
    let canonical = case.disk();
    for mode in ["wrong-pair", "changed-graph", "omitted", "same-output"] {
        let output = case.pair(mode);
        let mut args = case.control_args("--narrow-stream-checkpoint", &narrow,
            &prepared_pair, &output, mode);
        let initial = fs::read(case.root.join("initial.json")).unwrap();
        match mode {
            "wrong-pair" => args[6] = narrowed.history.display().to_string(),
            "changed-graph" => fs::write(case.root.join("initial.json"), fixture::graph(1, true)).unwrap(),
            "omitted" => args.truncate(9),
            "same-output" => {
                let p = Path::new(&args[10]);
                let text = fs::read_to_string(p).unwrap().replace(
                    output.source.to_str().unwrap(), output.history.to_str().unwrap());
                fs::write(p, text).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(control::command(&args, None, || ElapsedTick(1000), &mut Vec::new()).is_err(), "{mode}");
        assert_eq!(case.disk(), canonical, "{mode}");
        assert!(!output.history.exists()); assert!(!output.source.exists());
        fs::write(case.root.join("initial.json"), initial).unwrap();
    }
}

#[test]
fn bypass_after_reset_cannot_reach_forecast_or_either_publication_key() {
    let root = Root::new(); let case = Case::new(&root);
    let (complete, _, _) = contained(&case);
    let after = case.pair("bypass");
    let topology = case.profile(&complete, &after, "bypass", Some((2, true)));
    let (peer_path, peers) = super::functional_creation::peers_file(&root, &case.config);
    let args = vec!["create-learned-generated".into(), case.config_path.display().to_string(),
        case.recipe.display().to_string(), peer_path.display().to_string(),
        "--continue-generation".into(), "--history-anchor".into(), complete.history.display().to_string(),
        "--retain-history-anchor".into(), after.history.display().to_string(),
        "--topology-profile".into(), topology.display().to_string()];
    assert!(super::super::command_with_clock(&args, None, || {
        assert!(!peers.socket(1).exists(), "bypass cannot request human approval");
        ElapsedTick(1001)
    }).is_err());
    let state = case.view();
    assert_eq!(state.publication.executions, 0);
    assert_eq!(state.consistency.consistency.evidence.samples(), 0);
    assert!(state.consistency.consistency.pending_attempt.is_none());
    assert_eq!(state.consistency.retained_source_bytes, 0);
}

#[test]
fn paired_archive_and_report_failures_preserve_acknowledgment_and_exact_command_retry() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::Error::other("lost paired control report")) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    let root = Root::new(); let case = Case::new(&root);
    let prepared = case.prepare(); let narrow = case.narrow(info(&prepared));
    let original = checkpoint(&case.pair("prepared").source);
    let output = case.pair("failed-export");
    let args = case.control_args("--narrow-stream-checkpoint", &narrow,
        &case.pair("prepared"), &output, "failed-export");
    let mut blocked = false;
    let before = case.disk();
    let error = control::command(&args, None, || {
        // Admission has already checked both paths when the native finalizer
        // reaches its acknowledged later revision. Occupy only the source path.
        if !blocked && case.disk() != before {
            fs::write(&output.source, b"keep competing file").unwrap(); blocked = true;
        }
        ElapsedTick(1000)
    }, &mut Broken).unwrap_err();
    assert!(blocked);
    assert!(error.contains("acknowledged"));
    assert!(error.contains("lost paired control report"));
    assert_eq!(fs::read(&output.source).unwrap(), b"keep competing file");
    assert!(output.history.exists(), "one failed archive cannot suppress the other");
    let canonical = case.disk();
    let repaired = case.pair("repaired");
    let retry = case.control_args("--narrow-stream-checkpoint", &narrow,
        &case.pair("prepared"), &repaired, "repaired");
    // Exercise the public command and actor-service routing with the complete
    // original argv. Offline replay acquires no fresh source or effect time.
    crate::command_with_output(retry, &mut Vec::new()).unwrap();
    assert_eq!(case.disk(), canonical);
    unchanged_source(&original, &repaired, true);
}

#[test]
fn mediated_preparation_requires_distinct_v2_outputs_and_the_explicit_owned_recipe_before_create() {
    for mode in ["legacy-output", "same-output", "recovery-profile", "raw", "legacy-predictor"] {
        let root = Root::new(); let case = Case::new(&root);
        let original = fs::read_to_string(&case.creation).unwrap();
        match mode {
            "legacy-output" => {
                let value = original.replace("fa.mediated-create/2", "fa.mediated-create/1")
                    .replace(&format!(",\"retain_checkpoint\":\"{}\"", case.pair("prepared").source.display()), "");
                fs::write(&case.creation, value).unwrap();
            }
            "same-output" => fs::write(&case.creation, original.replace(
                case.pair("prepared").source.to_str().unwrap(),
                case.pair("prepared").history.to_str().unwrap())).unwrap(),
            "recovery-profile" => fs::write(&case.creation, original.replace(
                "fa.mediated-create/2", "fa.mediated-submit/2")).unwrap(),
            "raw" | "legacy-predictor" => {
                // The fixture installer accepts an original /4 recipe. Rebuild
                // that independent recipe before selecting the valid refused
                // predictor; never ask it to append a second forecast to /5.
                let path = super::continuation::write(&root);
                assert_eq!(path, case.recipe);
                super::forecast::install(&root, &path, mode == "legacy-predictor", 60_000);
                recipe::load(&path, &case.config, false).unwrap();
            }
            _ => unreachable!(),
        }
        assert!(preparation::command(&case.preparation_args(), None,
            || panic!("unsupported selection must refuse before startup"), &mut Vec::new()).is_err(), "{mode}");
        assert!(!case.config.store.exists(), "{mode}");
        assert!(!case.pair("prepared").history.exists());
        assert!(!case.pair("prepared").source.exists());
    }
}

#[test]
fn mediated_live_checkpoint_selection_requires_paired_outputs_before_creating_an_owner() {
    let root = Root::new(); let case = Case::new(&root);
    let selected = super::super::checkpoint_on_generation::Selection::read(&case.selection).unwrap();
    let mut loaded = recipe::load(&case.recipe, &case.config, false).unwrap();
    let (_, options) = custody::take_options(&case.preparation_args()).unwrap();
    options.admit(&case.config, &mut loaded, false).unwrap();
    super::super::topology::admit(&case.config, &mut loaded, Some(&case.creation),
        super::super::topology::Mode::Create).unwrap();
    selected.check(&case.config, &loaded).unwrap();
    let anchor = loaded.anchor_output.take();
    assert!(selected.check(&case.config, &loaded).unwrap_err().contains("history anchor"));
    loaded.anchor_output = anchor;
    let original = fs::read_to_string(&case.creation).unwrap();
    fs::write(&case.creation, original.replace("fa.mediated-create/2", "fa.mediated-create/1")
        .replace(&format!(",\"retain_checkpoint\":\"{}\"", case.pair("prepared").source.display()), "")).unwrap();
    loaded.topology = None;
    super::super::topology::admit(&case.config, &mut loaded, Some(&case.creation),
        super::super::topology::Mode::Create).unwrap();
    assert!(selected.check(&case.config, &loaded).unwrap_err().contains("topology checkpoint"));
    loaded.topology = None; loaded.anchor_output = None;
    selected.check(&case.config, &loaded).unwrap();
    assert!(!case.config.store.exists());
}

#[test]
fn preparation_releases_owner_before_paired_exports_and_preserves_the_other_archive_on_a_race() {
    for lose_source in [false, true] {
        let root = Root::new(); let case = Case::new(&root);
        let generation = recipe::load(&case.recipe, &case.config, false).unwrap().generation;
        let pair = case.pair("prepared");
        let mut raced = false; let mut output = Vec::new();
        let error = preparation::command(&case.preparation_args(), None, || {
            if !raced && case.config.store.exists() {
                match FileOversight::begin_open_with_learned_generation(&case.config.store,
                    case.config.profile.clone(), &generation) {
                    Ok(run) => {
                        drop(run);
                        fs::write(if lose_source { &pair.source } else { &pair.history },
                            b"independent output winner").unwrap();
                        raced = true;
                    }
                    Err(JournalError::Busy) => {},
                    Err(error) => panic!("unexpected original writer state: {error:?}"),
                }
            }
            if raced { ElapsedTick(u64::MAX) } else { ElapsedTick(1000) }
        }, &mut output).unwrap_err();
        assert!(raced, "only the dropped maintenance owner makes this race possible");
        assert!(error.contains("acknowledged") && error.contains("deadline reached after acknowledgment"));
        let value = report(&output);
        assert_eq!(value.get("history_anchor_retained").and_then(Json::as_bool), Some(lose_source));
        assert_eq!(value.get("topology_checkpoint_retained").and_then(Json::as_bool), Some(!lose_source));
        assert_eq!(number(&value, "sampled_draws"), 0);
        assert_eq!(number(&value, "sampling_attempts"), 0);
        assert_eq!(number(&value, "checkpoints"), 1);
        let state = case.view();
        assert!(state.publication.stop.is_none());
        assert_eq!(state.publication.executions, 0);
        assert_eq!(state.publication.control.ledger.epoch, 0);
        assert_eq!(state.consistency.consistency.evidence.samples(), 0);
        if lose_source {
            assert_eq!(custody::load(&pair.history).unwrap().revision(), number(&value, "revision"));
            assert_eq!(fs::read(&pair.source).unwrap(), b"independent output winner");
        } else {
            assert_eq!(checkpoint(&pair.source).history_anchor().revision(), number(&value, "revision"));
            assert_eq!(fs::read(&pair.history).unwrap(), b"independent output winner");
        }
    }
}
