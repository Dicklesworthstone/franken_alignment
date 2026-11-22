//! Original joint baseline creation with independently authenticated policy
//! governance. A joint guard never fabricates held-out qualification or an
//! effect key; the existing numerical, source and human gates remain live.
use super::*;
use crate::workflow::actor_service::learned::campaigns;

const JOINT: &str = r#"{"id":71,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":4,"max_member_outcomes":16}"#;

fn envelope(original: &str, joint: &str) -> String {
    format!(r#"{{"schema":"fa.learned-joint-publication/1","joint":{joint},"recipe":{original}}}"#)
}

pub(super) fn install(config: &Config, paths: &[PathBuf], entries: &mut [resident::Entry]) {
    assert_eq!(paths.len(), entries.len());
    for (path, entry) in paths.iter().zip(entries) {
        let original = fs::read_to_string(path).unwrap();
        fs::write(path, envelope(&original, JOINT)).unwrap();
        entry.loaded = recipe::load(path, config, true).unwrap();
        assert!(entry.loaded.generation.required_held_out_joint().is_some());
    }
}

#[test]
fn governed_joint_baseline_publishes_learned_messages_and_finish_across_independent_promotion() {
    governed_sequence_selected(ReviewDecision::Approve, true);
}

#[test]
fn governed_joint_baseline_keeps_the_fresh_human_key_after_policy_promotion() {
    governed_sequence_selected(ReviewDecision::Reject, true);
}

#[test]
fn governed_joint_baseline_retains_mediated_source_custody_through_messages_and_finish() {
    mediated::governed_joint_creation(ReviewDecision::Approve, false);
}

#[test]
fn governed_joint_baseline_cannot_replace_a_mediated_human_rejection() {
    mediated::governed_joint_creation(ReviewDecision::Reject, false);
}

#[test]
fn governed_joint_baseline_still_requires_the_final_current_topology_source() {
    mediated::governed_joint_creation(ReviewDecision::Approve, true);
}

#[test]
fn governed_joint_creation_compares_every_original_joint_field_before_assets() {
    let root = Root::new();
    let (config, paths, mut entries) = selected(&root, None, None);
    let originals = paths
        .iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>();
    install(&config, &paths, &mut entries);
    selections(
        &root,
        &entries[0].actor,
        std::process::id().checked_add(10_000).unwrap(),
    );
    let args = inputs(&root, &config, &paths, &entries, None);
    // A permitted complete selection reaches this actual original asset. Each
    // changed joint selection must instead refuse before opening that asset.
    let asset = root.0.join("fit-binding.json");
    fs::remove_file(&asset).unwrap();
    fs::create_dir(&asset).unwrap();
    let asset_error = format!("not an admitted regular file: {asset:?}");
    let failure = || {
        resident::command_with_clock(&args, None, || panic!("joint admission reached the clock"))
            .unwrap_err()
    };
    assert_eq!(failure(), asset_error);
    for (from, to) in [
        ("\"id\":71", "\"id\":72"),
        ("\"generation\":1", "\"generation\":2"),
        ("\"minimum_safe_roots\":1", "\"minimum_safe_roots\":2"),
        (
            "\"minimum_violation_roots\":1",
            "\"minimum_violation_roots\":2",
        ),
        ("\"maximum_escape_ppm\":0", "\"maximum_escape_ppm\":1"),
        (
            "\"maximum_false_stop_ppm\":0",
            "\"maximum_false_stop_ppm\":1",
        ),
        ("\"max_cases\":4", "\"max_cases\":5"),
        ("\"max_member_outcomes\":16", "\"max_member_outcomes\":17"),
    ] {
        assert!(JOINT.contains(from));
        fs::write(
            &paths[2],
            envelope(&originals[2], &JOINT.replacen(from, to, 1)),
        )
        .unwrap();
        let error = failure();
        assert!(
            error.contains("joint credibility policy differs"),
            "{from}: {error}"
        );
        assert_ne!(error, asset_error);
        fs::write(&paths[2], envelope(&originals[2], JOINT)).unwrap();
        assert_eq!(failure(), asset_error);
    }
    // Neither an omitted later guard nor a guard introduced after an unguarded
    // first entry can change the original owner selected by the whole sequence.
    for index in [0, 2] {
        fs::write(&paths[index], &originals[index]).unwrap();
        assert!(failure().contains("joint credibility policy differs"));
        fs::write(&paths[index], envelope(&originals[index], JOINT)).unwrap();
        assert_eq!(failure(), asset_error);
    }
    assert!(!config.store.exists());
    assert!(!root.0.join("final.anchor").exists());
    quiescent(&root);
}

#[test]
fn governed_joint_creation_keeps_direct_and_command_activation_and_predictor_boundaries() {
    for cause in [
        "activation",
        "missing_campaign",
        "raw",
        "owned",
        "unequal_generation",
    ] {
        let root = Root::new();
        let mode = match cause {
            "raw" => Some(false),
            "owned" => Some(true),
            _ => None,
        };
        let (config, paths, mut entries) = selected(&root, mode, None);
        let original_last = fs::read_to_string(&paths[2]).unwrap();
        install(&config, &paths, &mut entries);
        let (governor_path, operator_path, _) = selections(
            &root,
            &entries[0].actor,
            std::process::id().checked_add(10_000).unwrap(),
        );
        let governor = live_governor::Settings::read(&governor_path).unwrap();
        let operator =
            learned_operator::Selection::read_governed(&operator_path, GENERATION).unwrap();
        let args = inputs(&root, &config, &paths, &entries, None);
        let capsule = root.0.join("not-a-creation-activation.bin");
        if cause == "activation" {
            let error = resident::command_with_clock(&args, Some(&capsule), || {
                panic!("creation capsule reached assets or time")
            })
            .unwrap_err();
            assert!(
                error.contains("credibility activation requires explicit live recovery"),
                "{error}"
            );
        }
        if cause == "unequal_generation" {
            fs::write(&paths[2], original_last).unwrap();
            entries[2].loaded = recipe::load(&paths[2], &config, true).unwrap();
        }
        if cause != "missing_campaign" {
            let expected = governor.campaign_requirement();
            campaigns::admit_selected(&mut entries[0].loaded, expected).unwrap();
            for entry in &mut entries[1..] {
                entry.loaded.expected_campaigns = Some(expected);
            }
        }
        let peers = fixture::peers(&root, &config);
        let store = config.store.clone();
        let error = match resident::run_with_services(
            config,
            entries,
            &peers,
            false,
            None,
            (cause == "activation").then_some(capsule.as_path()),
            Some(&operator),
            Some(&governor),
            || panic!("unsupported direct joint selection reached the clock"),
        ) {
            Ok(_) => panic!("direct joint creation admitted {cause}"),
            Err(error) => error,
        };
        let expected = match cause {
            "activation" => "credibility activation requires explicit live recovery",
            "missing_campaign" => "campaign limits must match exactly",
            "unequal_generation" => "exact original finite generation and stream",
            _ => "nonpredictive campaign owner",
        };
        assert!(error.contains(expected), "{cause}: {error}");
        assert!(!store.exists());
        assert!(!root.0.join("final.anchor").exists());
        quiescent(&root);
    }
}

#[test]
fn governed_joint_creation_uses_the_original_atomic_constructor_and_unqualified_baseline() {
    let root = Root::new();
    let (config, paths, mut entries) = selected(&root, None, None);
    install(&config, &paths, &mut entries);
    let (governor_path, _, _) = selections(
        &root,
        &entries[0].actor,
        std::process::id().checked_add(10_000).unwrap(),
    );
    let governor = live_governor::Settings::read(&governor_path).unwrap();
    let original_joint = entries[0].loaded.generation.required_held_out_joint();
    // Missing first-image campaign custody cannot create a partially guarded
    // owner even though the independently selected joint contract is valid.
    assert!(recovery::create_governed(&config, &entries[0].loaded).is_err());
    assert!(!config.store.exists());
    campaigns::admit_selected(&mut entries[0].loaded, governor.campaign_requirement()).unwrap();
    let ((host, _human, predictor, identity), _governor) =
        recovery::create_governed(&config, &entries[0].loaded).unwrap();
    assert!(predictor.is_none() && identity.is_none());
    assert!(host.policy_campaigns_required());
    assert_eq!(host.held_out_joint_policy().unwrap(), original_joint);
    host.check_credibility().unwrap();
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!((numerical.position, numerical.sampled_draws), (0, 0));
}
