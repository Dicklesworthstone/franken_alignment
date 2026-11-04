//! Resident functional/campaign creation and exactly one selected first cut.
//! Actual numerical work, native votes and authenticated human decisions remain
//! the publication path; these fixtures do not qualify model or restart quality.
use super::*;
use super::super::mediated_topology;
use super::super::super::{anchors, checkpoint_on_generation as checkpoint};
use fa_reference::action::consequence::{
    delivery::persistent::{governance::PolicyUpdate, observed::{
        decoder::learned::sidecar::FileLearnedSidecarFinish,
        guarded::FileCampaignRequirement,
    }},
    gate::containment::{RestartGrade, session::policy::{Policy, Predicate}},
    policy_campaign::ReplayLimits,
};

fn requirement() -> FileCampaignRequirement {
    FileCampaignRequirement { limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 },
        max_campaigns: 8 }
}

fn checkpoint_document(id: u64, position: u64, draws: u64) -> String {
    format!(r#"{{"schema":"fa.learned-generation-checkpoint/1","checkpoint":"{id}","expected_position":"{position}","expected_sampled_draws":"{draws}"}}"#)
}

struct Case {
    config: Config,
    paths: Vec<PathBuf>,
    entries: Vec<resident::Entry>,
    args: Vec<String>,
    anchor: PathBuf,
    campaign: PathBuf,
    checkpoint: PathBuf,
}
impl Case {
    fn new(root: &Root, mode: Option<bool>) -> Self {
        let (config, paths, entries) = selected(root, mode, None);
        for (index, entry) in entries.iter().enumerate() {
            let inputs = mediated_topology::command_inputs(root, &config, &paths[index], &entry.actor);
            fs::rename(&inputs[5], root.0.join(format!("sequence-actor-{}.json", entry.loaded.request))).unwrap();
        }
        let config_path = root.0.join("command-host.json");
        let raw = fs::read_to_string(&config_path).unwrap();
        assert!(raw.contains("\"grade\": \"audit_only\""));
        fs::write(&config_path, raw.replace("\"grade\": \"audit_only\"",
            "\"grade\": \"functional_restart\"")).unwrap();
        let config = Config::read_learned_generation(&config_path).unwrap();
        let anchor = root.0.join("resident-final.anchor");
        let campaign = root.0.join("resident-campaign.json");
        fs::write(&campaign, r#"{"schema":"fa.policy-campaign-profile/1","cases":16,"input_bytes":1048576,"max_campaigns":8}"#).unwrap();
        let checkpoint = root.0.join("resident-checkpoint.json");
        fs::write(&checkpoint, checkpoint_document(71, 2, 0)).unwrap();
        let args = vec!["serve-learned-generated".into(), config_path.display().to_string(),
            root.0.join("resident-sequence.json").display().to_string(),
            root.0.join("command-reviewer.json").display().to_string(),
            "--retain-history-anchor".into(), anchor.display().to_string()];
        let case = Self { config, paths, entries, args, anchor, campaign, checkpoint };
        case.write_sequence(root, 0);
        case
    }

    fn write_sequence(&self, root: &Root, first: usize) {
        let entries = self.entries.iter().enumerate().skip(first).map(|(index, entry)| {
            format!(r#"{{"recipe":{},"actor_profile":{},"release":"{}"}}"#,
                quote(self.paths[index].to_str().unwrap()),
                quote(root.0.join(format!("sequence-actor-{}.json", entry.loaded.request)).to_str().unwrap()),
                if matches!(entry.release, LearnedTextRelease::Finish) { "finish" } else { "message" })
        }).collect::<Vec<_>>().join(",");
        fs::write(&self.args[2], format!(r#"{{"schema":"fa.learned-publication-sequence/1","entries":[{entries}]}}"#)).unwrap();
    }

    fn selected_args(&self, capture: bool) -> Vec<String> {
        let mut args = self.args.clone();
        args.extend(["--campaign-profile".into(), self.campaign.display().to_string()]);
        if capture {
            args.extend(["--checkpoint-before-generation".into(), self.checkpoint.display().to_string()]);
        }
        args
    }

    fn reader(&self, campaigns: bool) -> Reader {
        let config = Config::read_learned_recovery(Path::new(&self.args[1])).unwrap();
        let mut loaded = recipe::load(&self.paths[0], &config, false).unwrap();
        if campaigns { loaded.expected_campaigns = Some(requirement()); }
        loaded.recovery_anchor = Some(anchors::load(&self.anchor).unwrap());
        Reader { config, loaded }
    }
}

#[test]
fn resident_functional_campaign_and_prompt_checkpoint_retain_each_native_and_human_gate() {
    for decision in [ReviewDecision::Approve, ReviewDecision::Reject] {
        let root = Root::new(); let case = Case::new(&root, None);
        let mut peers = Peers::new(&root, &case.config, &case.entries);
        let result = resident::command_with_clock(&case.selected_args(true), None, || {
            peers.poll([Some(Actor::Observe); 3],
                [Some(ReviewDecision::Approve), Some(decision), Some(ReviewDecision::Approve)]);
            ElapsedTick(1000)
        });
        assert_eq!(result.is_ok(), decision == ReviewDecision::Approve, "{result:?}");
        assert_eq!(peers.seen_actor, [true, true, decision == ReviewDecision::Approve]);
        assert_eq!(peers.seen_human, peers.seen_actor);
        let (replies, packets) = peers.join();
        assert!(confirmed(replies[0].as_ref().unwrap()));
        assert_eq!(confirmed(replies[1].as_ref().unwrap()), decision == ReviewDecision::Approve);
        let mut sessions = BTreeSet::new();
        for packet in packets.iter().flatten() {
            assert!(sessions.insert(packet.binding().session));
            assert_eq!(packet.views().len(), 2);
        }
        let mut reader = case.reader(true);
        let state = reader.snapshot();
        let successful = if decision == ReviewDecision::Approve { 3 } else { 1 };
        assert_eq!(state.executions, successful);
        assert_eq!(state.payload, if successful == 3 { b"AA".as_slice() } else { b"A".as_slice() });
        let charged: u64 = packets.iter().take(successful as usize).flatten()
            .map(|packet| packet.action().spec().units).sum();
        assert_eq!(state.control.ledger.charged, charged);
        assert_eq!(state.control.ledger.reserved, 0);
        assert_eq!(state.control.ledger.epoch, 0, "resident entries cannot create recovery fences");
        assert!(state.stop.is_none());
        assert_eq!(reader.loaded.recovery_anchor.as_ref().unwrap().revision(), state.revision);

        let original = fs::read(case.config.store.join("delivery.bin")).unwrap();
        reader.loaded.expected_campaigns = None;
        assert!(recovery::open_selected(&reader.config, &reader.loaded).is_err());
        reader.loaded.expected_campaigns = Some(FileCampaignRequirement { max_campaigns: 9, ..requirement() });
        assert!(recovery::open_selected(&reader.config, &reader.loaded).is_err());
        assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), original);
        reader.loaded.expected_campaigns = Some(requirement());
        let (mut host, _, observer, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
        assert!(observer.is_none() && host.policy_campaigns_required());
        let saved = host.learned_stream_checkpoint(71).unwrap();
        assert_eq!(saved.info().position, 2); assert_eq!(saved.info().sampled_draws, 0);
        assert_eq!(saved.info().authority_epoch, 0);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 6); assert_eq!(numerical.sampled_draws, 4);
        assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
        assert_eq!(host.actor_snapshot().unwrap().state.profile().grade, RestartGrade::FunctionalRestart);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
        assert_eq!(host.stream_snapshot().unwrap().confirmed.finished(), successful == 3);
        for request in 1..=2 {
            let FileLearnedSidecarFinish::Applied { receipt, archive, .. } =
                host.learned_sidecar_outcome(request).unwrap().result else { panic!("missing native review"); };
            assert!(receipt.is_ok());
            assert_eq!(archive.commit_times.len(), 2); assert_eq!(archive.reveal_times.len(), 2);
            assert_eq!(archive.inputs.action(), packets[request as usize - 1].as_ref().unwrap().action());
        }
        let before = host.inspect();
        let update = PolicyUpdate::new(901, before.control.sequence, before.control.ledger.epoch,
            Policy::new(2, vec![Predicate::PayloadAtMost(512)]).unwrap()).unwrap();
        assert_eq!(host.replace_policy(host.revision(), &update), Err(JournalError::Contract(Error::Incomplete)));
        assert_eq!(host.inspect(), before);
        drop(host); quiescent(&root);
    }
}

#[test]
fn resident_functional_campaigns_keep_both_predictor_lifetimes_across_all_releases() {
    for owned in [false, true] {
        let root = Root::new(); let case = Case::new(&root, Some(owned));
        let mut peers = Peers::new(&root, &case.config, &case.entries);
        resident::command_with_clock(&case.selected_args(false), None, || {
            peers.poll([Some(Actor::Observe); 3], [Some(ReviewDecision::Approve); 3]);
            ElapsedTick(1000)
        }).unwrap();
        let (replies, packets) = peers.join();
        assert!(replies.iter().all(|response| confirmed(response.as_ref().unwrap())));
        let reader = case.reader(true);
        let before = reader.snapshot();
        assert_eq!(before.executions, 3); assert_eq!(before.payload, b"AA");
        assert_eq!(before.control.ledger.epoch, 0);
        assert_eq!(before.control.ledger.charged,
            packets.iter().flatten().map(|packet| packet.action().spec().units).sum::<u64>());
        let (host, _, observer, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
        assert!(observer.is_some() && host.policy_campaigns_required());
        let predictor = host.action_consistency_snapshot().unwrap();
        assert_eq!(predictor.evidence.samples(), 3);
        assert_eq!(predictor.pending_attempt, None);
        assert!(!predictor.coverage_lost && !predictor.evidence.crossed());
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 6); assert_eq!(numerical.sampled_draws, 4);
        assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
        if owned {
            let original = host.learned_action_consistency_snapshot().unwrap();
            assert!(original.work.probe_coordinates > 0 && original.retained_source_bytes > 0);
            assert!(!original.has_unreported_work);
        }
        drop(host); quiescent(&root);
    }
}

#[test]
fn a_wrong_resident_checkpoint_cut_cannot_sample_or_open_a_successor() {
    for (position, draws) in [(1, 0), (3, 0), (2, 1)] {
        let root = Root::new(); let case = Case::new(&root, None);
        fs::write(&case.checkpoint, checkpoint_document(71, position, draws)).unwrap();
        assert!(resident::command_with_clock(&case.selected_args(true), None,
            || ElapsedTick(1000)).is_err());
        let reader = case.reader(true);
        let before = reader.snapshot();
        assert_eq!(before.executions, 0); assert!(before.payload.is_empty());
        assert_eq!(before.control.ledger.charged, 0); assert_eq!(before.control.ledger.reserved, 0);
        assert!(before.stop.is_some());
        let (host, _, _, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 2); assert_eq!(numerical.sampled_draws, 0);
        assert_eq!(numerical.cumulative_work.sampling_attempts, 0);
        assert!(host.learned_stream_checkpoint(71).is_err());
        assert!(host.request_status(2).is_err());
        drop(host); quiescent(&root);
    }
}

#[test]
fn resident_creation_and_first_only_selections_refuse_before_time_or_store() {
    for mismatch in 0..6 {
        let root = Root::new(); let mut case = Case::new(&root, None);
        let mut continuing = false;
        match mismatch {
            0 => for entry in &mut case.entries { entry.loaded.expected_campaigns = Some(requirement()); },
            1 => case.entries[1].loaded.expected_campaigns = Some(requirement()),
            2 => case.entries[0].loaded.creation_campaigns = Some(requirement()),
            3 => {
                for entry in &mut case.entries { entry.loaded.expected_campaigns = Some(requirement()); }
                case.entries[0].loaded.creation_campaigns = Some(requirement());
                case.entries[1].loaded.creation_campaigns = Some(requirement());
            }
            4 => case.entries[1].loaded.checkpoint_on_generation = Some(
                checkpoint::Selection::decode(checkpoint_document(72, 4, 2).as_bytes()).unwrap()),
            5 => {
                for entry in &mut case.entries { entry.loaded.expected_campaigns = Some(requirement()); }
                case.entries[0].loaded.creation_campaigns = Some(requirement());
                continuing = true;
            }
            _ => unreachable!(),
        }
        let reviewer = fixture::peers(&root, &case.config);
        let store = case.config.store.clone();
        assert!(resident::run(case.config, case.entries, &reviewer, continuing, None, None,
            || panic!("invalid resident guard or later selection must precede time")).is_err());
        assert!(!store.exists()); quiescent(&root);
    }
}

#[test]
fn resident_command_grammar_and_restart_grades_cannot_smuggle_creation_into_recovery() {
    let root = Root::new(); let case = Case::new(&root, None);
    for tail in [vec!["--continue-generation"], vec!["--after-message", "1"],
        vec!["--campaign-profile", "/duplicate.json"], vec!["--actor-profile", "/actor.json"],
        vec!["--precheck-profile", "/precheck.json"], vec!["--finish-after-message", "1"],
        vec!["--checkpoint-before-generation", "/duplicate.json"]] {
        let mut args = case.selected_args(true); args.extend(tail.into_iter().map(str::to_owned));
        assert!(resident::command_with_clock(&args, None,
            || panic!("incompatible resident options must precede time")).is_err());
        assert!(!case.config.store.exists()); quiescent(&root);
    }
    let original = fs::read_to_string(&case.args[1]).unwrap();
    for grade in ["audit_only", "bit_exact", "functional", "unknown"] {
        fs::write(&case.args[1], original.replace("functional_restart", grade)).unwrap();
        assert!(resident::command_with_clock(&case.selected_args(true), None,
            || panic!("unsupported checkpoint grade must precede time")).is_err());
        assert!(!case.config.store.exists()); quiescent(&root);
    }
}

#[test]
fn recovered_resident_sequence_captures_only_its_first_new_boundary_without_refunding_draws() {
    let root = Root::new(); let case = Case::new(&root, None);
    let mut config = Config::read_learned_generation(Path::new(&case.args[1])).unwrap();
    let loaded = recipe::load(&case.paths[0], &config, true).unwrap();
    let (host, reviewer, _, _) = recovery::create_selected(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor); let mut wire = ActorWire::new(port);
    let limit = deadline(); let mut control = Control::new(&config, 1, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &limit, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &limit, &mut || ElapsedTick(1000)).unwrap();
    let (mut driver, history) = native::review_current(&mut config, &loaded, &reviewer, driver, &limit);
    let human = native::approve(&mut driver, &mut config, &reviewer, 1);
    native::publish(&mut driver, &mut config, &human);
    assert!(executed(&mut wire, 1));
    let before = driver.supervisor().host().unwrap().learned_generation_inspection().unwrap().numerical;
    assert_eq!(before.position, 4); assert_eq!(before.sampled_draws, 2);
    let input = root.0.join("resident-first.anchor");
    anchors::export_acknowledged(&driver.supervisor().host().unwrap().history_anchor().unwrap(), &input).unwrap();
    drop(history); drop(wire); assert_eq!(cleanup(driver, 1000, 1), 0);

    case.write_sequence(&root, 1);
    fs::write(&case.checkpoint, checkpoint_document(72, 4, 2)).unwrap();
    let mut args = case.args.clone();
    args.extend(["--after-message".into(), "1".into(), "--history-anchor".into(), input.display().to_string(),
        "--checkpoint-before-generation".into(), case.checkpoint.display().to_string()]);
    let mut peers = Peers::new(&root, &case.config, &case.entries);
    resident::command_with_clock(&args, None, || {
        peers.poll([None, Some(Actor::Observe), Some(Actor::Observe)],
            [None, Some(ReviewDecision::Approve), Some(ReviewDecision::Approve)]);
        ElapsedTick(1000)
    }).unwrap();
    assert_eq!(peers.seen_actor, [false, true, true]);
    assert_eq!(peers.seen_human, [false, true, true]);
    let (replies, packets) = peers.join();
    assert!(replies[0].is_none() && packets[0].is_none());
    assert!(replies[1..].iter().all(|reply| confirmed(reply.as_ref().unwrap())));
    let reader = case.reader(false);
    let published = reader.snapshot();
    assert_eq!(published.executions, 3); assert_eq!(published.payload, b"AA");
    assert_eq!(published.control.ledger.epoch, 1, "one startup recovery fence only");
    let (host, _, _, _) = recovery::open_selected(&reader.config, &reader.loaded).unwrap();
    let saved = host.learned_stream_checkpoint(72).unwrap();
    assert_eq!(saved.info().position, 4); assert_eq!(saved.info().sampled_draws, 2);
    assert_eq!(saved.info().authority_epoch, 1);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(numerical.position, 6); assert_eq!(numerical.sampled_draws, 4);
    assert_eq!(numerical.cumulative_work.sampling_attempts, 4);
    assert!(host.stream_snapshot().unwrap().confirmed.finished());
    assert!(host.learned_stream_checkpoint(71).is_err());
    drop(host); quiescent(&root);
}
