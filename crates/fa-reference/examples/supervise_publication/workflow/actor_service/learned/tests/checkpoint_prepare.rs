//! Actual fresh prompt preparation, independent retained custody and subsequent
//! native/human publication. Finite fixtures establish control, not calibration.
use super::*;
use super::super::{anchors as custody, campaigns, checkpoint_prepare as prepare,
    checkpoint_on_generation::Selection, stream_checkpoint};
use fa_reference::action::consequence::activation::monitor::learned::LearnedMonitorWork;
use fa_reference::action::consequence::delivery::persistent::{FileDeliverySnapshot, observed::{
    consistency::learned::FileLearnedPublicationSnapshot,
    decoder::learned::{FileLearnedStreamCheckpointRequest, checkpoint::FileLearnedCheckpointInfo},
    reviewer::wire::ReviewDecision,
}};
use fa_reference::strict_json::{self, Json, Limits};
use std::{io::{self, Write}, path::{Path, PathBuf}};

struct Case {
    config: Config,
    config_path: PathBuf,
    recipe: PathBuf,
    selection: PathBuf,
    anchor: PathBuf,
    campaign: Option<PathBuf>,
}
impl Case {
    fn new(root: &Root, campaign: bool) -> Self {
        let config_path = super::functional_creation::config_file(root, "functional_restart");
        let config = Config::read_learned_generation(&config_path).unwrap();
        let recipe = super::continuation::write(root);
        super::forecast::install_restart(root, &recipe, 60_000);
        evidence(root, &config, 1, true, false);
        let selection = root.0.join("prepare-checkpoint.json");
        fs::write(&selection, document(2, 0)).unwrap();
        let campaign = campaign.then(|| {
            let path = root.0.join("prepare-campaign.json");
            fs::write(&path, r#"{"schema":"fa.policy-campaign-profile/1","cases":16,"input_bytes":1048576,"max_campaigns":8}"#).unwrap();
            path
        });
        Self { config, config_path, recipe, selection, anchor: root.0.join("prepared.anchor"), campaign }
    }
    fn args(&self) -> Vec<String> {
        let mut args = vec!["create-learned-generated".into(), self.config_path.display().to_string(),
            self.recipe.display().to_string(), "--prepare-stream-checkpoint".into(),
            self.selection.display().to_string(), "--retain-history-anchor".into(),
            self.anchor.display().to_string()];
        if let Some(campaign) = &self.campaign {
            args.extend(["--campaign-profile".into(), campaign.display().to_string()]);
        }
        args
    }
    fn reader(&self) -> Loaded {
        let mut loaded = recipe::load(&self.recipe, &self.config, false).unwrap();
        if let Some(campaign) = &self.campaign {
            loaded.expected_campaigns = Some(campaigns::decode(&fs::read(campaign).unwrap()).unwrap());
        }
        loaded
    }
    fn view(&self) -> FileLearnedPublicationSnapshot {
        let loaded = self.reader();
        FileOversight::read_publication_with_owned_learned_consistency(&self.config.store,
            &self.config.profile, &loaded.generation,
            loaded.generation.required_owned_pre_output_forecast().unwrap()).unwrap()
    }
    fn disk(&self) -> Vec<u8> { fs::read(self.config.store.join("delivery.bin")).unwrap() }
    fn prepare(&self) -> Json {
        let mut output = Vec::new();
        prepare::command(&self.args(), None, || ElapsedTick(1000), &mut output).unwrap();
        report(&output)
    }
}

fn document(position: u64, draws: u64) -> String {
    format!(r#"{{"schema":"fa.learned-generation-checkpoint/1","checkpoint":"71","expected_position":"{position}","expected_sampled_draws":"{draws}"}}"#)
}
fn report(bytes: &[u8]) -> Json {
    let value = strict_json::parse(bytes, Limits::default()).unwrap();
    assert_eq!(value.get("schema").and_then(Json::as_str), Some("fa.learned-stream-preparation-result/1"));
    assert_eq!(value.get("outcome").and_then(Json::as_str), Some("prepared"));
    assert_eq!(value.get("effect_authority").and_then(Json::as_str), Some("not_issued"));
    value
}
fn number(value: &Json, name: &str) -> u64 {
    value.get(name).unwrap().as_str().unwrap().parse().unwrap()
}
fn quiet(state: &FileDeliverySnapshot) {
    assert!(state.stop.is_none());
    assert_eq!(state.executions, 0);
    assert_eq!(state.control.ledger.charged, 0);
    assert_eq!(state.control.ledger.reserved, 0);
    assert_eq!(state.control.ledger.epoch, 0);
    assert_eq!(state.control.sequence, 0);
}

#[test]
fn actual_prepare_retains_an_unfenced_unsampled_prompt_and_original_campaign_inventory() {
    for campaign in [false, true] {
        let root = Root::new(); let case = Case::new(&root, campaign);
        // Prompt work has no dependency on later native helper execution assets.
        fs::remove_file(root.0.join("helper-weights.safetensors")).unwrap();
        let prepared = case.prepare();
        assert_eq!(number(&prepared, "position"), 2);
        assert_eq!(number(&prepared, "sampled_draws"), 0);
        assert_eq!(number(&prepared, "sampling_attempts"), 0);
        assert_eq!(number(&prepared, "checkpoints"), 1);
        assert_eq!(number(&prepared, "restart_attempts"), 0);
        assert_eq!(prepared.get("history_anchor_retained").and_then(Json::as_bool), Some(true));
        let original = case.disk(); let view = case.view();
        quiet(&view.publication);
        assert_eq!(view.publication.revision, number(&prepared, "revision"));
        assert!(view.pending_request.is_none());
        assert!(view.consistency.consistency.pending_attempt.is_none());
        assert_eq!(view.consistency.consistency.evidence.samples(), 0);
        assert!(!view.consistency.consistency.coverage_lost);
        assert_eq!(view.consistency.work, LearnedMonitorWork::default());
        assert_eq!(view.consistency.retained_source_bytes, 0);
        assert!(!view.consistency.has_unreported_work);
        let anchor = custody::load(&case.anchor).unwrap();
        assert_eq!(anchor.revision(), view.publication.revision);
        let mut loaded = case.reader();
        loaded.recovery_anchor = Some(anchor);
        loaded.anchor_output = Some(root.0.join("capture-retry.anchor"));
        let info = FileLearnedCheckpointInfo { checkpoint: 71,
            actor_revision: number(&prepared, "actor_revision"),
            authority_epoch: number(&prepared, "authority_epoch"),
            control_sequence: number(&prepared, "control_sequence"),
            stream: number(&prepared, "stream"), position: 2, sampled_draws: 0 };
        let request = stream_checkpoint::Request::Capture(FileLearnedStreamCheckpointRequest::new(
            view.publication.revision - 1, info).unwrap());
        if campaign {
            let expected = loaded.expected_campaigns.take();
            assert!(recovery::stream_checkpoint::execute(&case.config, &loaded, &request, &mut |_| Ok(())).is_err());
            assert_eq!(case.disk(), original);
            loaded.expected_campaigns = expected;
        }
        // A complete original guarded replay can reacquire the same capture
        // without an ordinary recovery Fence or any extra numerical operation.
        let captured = recovery::stream_checkpoint::execute(&case.config, &loaded,
            &request, &mut |_| Ok(())).unwrap();
        assert_eq!(captured.appended_events(), 0);
        assert_eq!(captured.numerical().position, 2);
        assert_eq!(captured.numerical().cumulative_work.sampling_attempts, 0);
        assert!(captured.numerical().cumulative_work.admitted_tokens > 0);
        assert_eq!(case.disk(), original);
        assert_eq!(fs::read_dir(root.0.join("peers")).unwrap().count(), 0);
    }
}

#[test]
fn prepared_prompt_continues_through_fresh_forecast_native_review_and_separate_human_key() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new(); let case = Case::new(&root, false);
        case.prepare();
        let (peer_path, peers) = super::functional_creation::peers_file(&root, &case.config);
        let anchor = root.0.join("continued.anchor");
        let args = vec!["create-learned-generated".into(), case.config_path.display().to_string(),
            case.recipe.display().to_string(), peer_path.display().to_string(),
            "--continue-generation".into(), "--history-anchor".into(), case.anchor.display().to_string(),
            "--retain-history-anchor".into(), anchor.display().to_string()];
        let mut human = None;
        let result = super::super::command_with_clock(&args, None, || {
            if human.is_none() && peers.socket(1).exists() {
                human = Some(super::peer_intake::human(peers.clone(), 1, decision));
            }
            ElapsedTick(1001)
        });
        let packet = human.expect("prepared prompt still needs independent human approval").join().unwrap();
        assert_eq!(result.is_ok(), decision == ReviewDecision::Approve);
        assert_eq!(packet.views().len(), 2);
        let view = case.view();
        assert_eq!(view.publication.executions, u64::from(decision == ReviewDecision::Approve));
        assert_eq!(view.publication.control.ledger.charged,
            if decision == ReviewDecision::Approve { packet.action().spec().units } else { 0 });
        assert_eq!(view.consistency.consistency.evidence.samples(), 1);
        assert!(view.consistency.work.probe_coordinates > 0);
        if decision == ReviewDecision::Approve {
            let mut loaded = case.reader(); loaded.recovery_anchor = Some(custody::load(&anchor).unwrap());
            let (host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
            let numerical = host.learned_generation_inspection().unwrap().numerical;
            assert_eq!(numerical.sampled_draws, 2);
            assert_eq!(numerical.cumulative_work.sampling_attempts, 2);
            assert_eq!(host.learned_recovery_usage().unwrap().checkpoints, 1);
            assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
            assert_eq!(host.learned_stream_checkpoint(71).unwrap().info().authority_epoch, 0);
        }
    }
}

#[test]
fn preparation_owns_its_closed_fresh_grammar_and_rejects_ineligible_recipes_before_creation() {
    let root = Root::new(); let case = Case::new(&root, false);
    let base = case.args();
    for extra in [
        vec!["--continue-generation"], vec!["--history-anchor", "/missing/input.anchor"],
        vec!["--actor-profile", "/missing/actor.json"], vec!["--precheck-profile", "/missing/precheck.json"],
        vec!["--topology-profile", "/missing/topology.json"], vec!["--finish-after-message", "1"],
        vec!["--checkpoint-before-generation", "/missing/checkpoint.json"],
        vec!["--prepare-stream-checkpoint", "/missing/checkpoint.json"],
        vec!["--retain-history-anchor", "/missing/duplicate.anchor"], vec!["--campaign-profile"],
    ] {
        let mut args = base.clone(); args.extend(extra.into_iter().map(str::to_owned));
        assert!(prepare::command(&args, None, || panic!("invalid grammar read time"), &mut Vec::new()).is_err());
        assert!(!case.config.store.exists());
    }
    assert!(prepare::command(&base[..5], None, || panic!("missing custody read time"), &mut Vec::new()).is_err());
    assert!(prepare::command(&base, Some(Path::new("/missing/activation")),
        || panic!("activation read time"), &mut Vec::new()).is_err());
    let original_config = fs::read_to_string(&case.config_path).unwrap();
    for grade in ["audit_only", "bit_exact", "functional", "unknown"] {
        fs::write(&case.config_path, original_config.replace("functional_restart", grade)).unwrap();
        assert!(prepare::command(&base, None, || panic!("ineligible grade read time"), &mut Vec::new()).is_err());
        assert!(!case.config.store.exists());
    }
    fs::write(&case.config_path, original_config).unwrap();
    fs::write(&case.selection, document(2, 1)).unwrap();
    assert!(prepare::command(&base, None, || panic!("sampled selection read time"), &mut Vec::new()).is_err());
    fs::write(&case.selection, document(2, 0)).unwrap();
    let forecast = root.0.join("forecast.json"); let original = fs::read_to_string(&forecast).unwrap();
    fs::write(&forecast, original.replace("owned_prompt_audit", "disabled")).unwrap();
    assert!(prepare::command(&base, None, || panic!("legacy predictor read time"), &mut Vec::new()).is_err());
    fs::write(&forecast, original).unwrap();
    assert!(!case.config.store.exists());
    case.prepare();
    let before = case.disk(); let mut again = case.args();
    again[6] = root.0.join("cannot-recreate.anchor").display().to_string();
    let error = prepare::command(&again, None, || ElapsedTick(1000), &mut Vec::new()).unwrap_err();
    assert!(error.contains("AlreadyExists"), "{error}");
    assert_eq!(case.disk(), before);
    assert!(!Path::new(&again[6]).exists());
}

#[test]
fn preparation_cannot_omit_stronger_identity_or_recovery_requirements() {
    let root = Root::new(); let case = Case::new(&root, false);
    let selected = Selection::read(&case.selection).unwrap();
    for change in 0..5 {
        let mut loaded = case.reader();
        loaded.anchor_output = Some(case.anchor.clone());
        match change {
            0 => loaded.generation = loaded.generation.clone().with_required_computed_identity().unwrap(),
            1 => loaded.floor.journal_revision = 1,
            2 => loaded.floor.control_sequence = 1,
            3 => loaded.floor.authority_epoch = 1,
            _ => loaded.expected_campaigns = Some(campaigns::decode(
                br#"{"schema":"fa.policy-campaign-profile/1","cases":16,"input_bytes":1048576,"max_campaigns":8}"#).unwrap()),
        }
        assert!(prepare::check_selection(&case.config, &loaded, selected).is_err(), "{change}");
        assert!(!case.config.store.exists());
    }
}

#[test]
fn wrong_prompt_boundary_or_missing_policy_source_stops_without_a_forecast_or_draw() {
    for position in [1, 2, 3] {
        let root = Root::new(); let case = Case::new(&root, false);
        fs::write(&case.selection, document(position, 0)).unwrap();
        if position == 2 { fs::remove_file(root.0.join("evidence.json")).unwrap(); }
        let mut output = Vec::new();
        assert!(prepare::command(&case.args(), None, || ElapsedTick(1000), &mut output).is_err());
        assert!(output.is_empty());
        let before = case.disk(); let view = case.view();
        assert!(view.publication.stop.is_some());
        assert_eq!(view.publication.executions, 0);
        assert_eq!(view.publication.control.ledger.charged, 0);
        assert_eq!(view.consistency.consistency.evidence.samples(), 0);
        assert_eq!(view.consistency.work, LearnedMonitorWork::default());
        assert!(view.pending_request.is_none());
        assert_eq!(case.disk(), before);
        let mut loaded = case.reader();
        loaded.recovery_anchor = Some(custody::load(&case.anchor).unwrap());
        let (host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, if position == 2 { 0 } else { 2 });
        assert_eq!(numerical.sampled_draws, 0);
        assert_eq!(numerical.cumulative_work.sampling_attempts, 0);
        assert!(host.learned_stream_checkpoint(71).is_err());
    }
}

#[test]
fn acknowledged_preparation_survives_lost_export_report_and_deadline_without_stop_or_reprefill() {
    struct Sink { fail: bool, bytes: Vec<u8> }
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail { return Err(io::Error::other("lost prepared report")); }
            self.bytes.extend_from_slice(bytes); Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    for (lose_export, lose_report) in [(false, true), (true, false), (true, true)] {
        let root = Root::new(); let case = Case::new(&root, false);
        let generation = case.reader().generation;
        let mut raced = false;
        let mut output = Sink { fail: lose_report, bytes: Vec::new() };
        let error = prepare::command(&case.args(), None, || {
            // The first successful exclusive read after creation is after the
            // maintenance owner dropped. No clock callback can mutate its live
            // prompt or claim an unacknowledged capture as complete.
            if lose_export && !raced && case.config.store.exists() {
                match FileOversight::begin_open_with_learned_generation(&case.config.store,
                    case.config.profile.clone(), &generation) {
                    Ok(run) => {
                        drop(run);
                        fs::write(&case.anchor, b"independent output winner").unwrap();
                        raced = true;
                    }
                    Err(JournalError::Busy) => {},
                    Err(error) => panic!("unexpected original writer state: {error:?}"),
                }
            }
            if raced { ElapsedTick(u64::MAX) } else { ElapsedTick(1000) }
        }, &mut output).unwrap_err();
        assert!(error.contains("acknowledged"), "{error}");
        if lose_report { assert!(error.contains("lost prepared report"), "{error}"); }
        if lose_export {
            assert!(raced);
            assert!(error.contains("retention failed") && error.contains("deadline reached after acknowledgment"));
            assert_eq!(fs::read(&case.anchor).unwrap(), b"independent output winner");
        } else { assert_eq!(custody::load(&case.anchor).unwrap().revision(), case.view().publication.revision); }
        if !lose_report {
            let value = report(&output.bytes);
            assert_eq!(value.get("history_anchor_retained").and_then(Json::as_bool), Some(false));
            assert_eq!(value.get("deadline_reached").and_then(Json::as_bool), Some(true));
            assert_eq!(number(&value, "position"), 2);
            assert_eq!(number(&value, "sampling_attempts"), 0);
        }
        let view = case.view(); quiet(&view.publication);
        assert_eq!(view.consistency.work, LearnedMonitorWork::default());
        let canonical = case.disk();
        let mut args = case.args(); args[6] = root.0.join("cannot-reprepare.anchor").display().to_string();
        assert!(prepare::command(&args, None, || ElapsedTick(1000), &mut Vec::new())
            .unwrap_err().contains("AlreadyExists"));
        assert_eq!(case.disk(), canonical);
        assert!(!Path::new(&args[6]).exists());
    }
}
