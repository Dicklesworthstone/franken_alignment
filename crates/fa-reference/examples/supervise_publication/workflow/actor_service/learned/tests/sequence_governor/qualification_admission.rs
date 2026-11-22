//! Joint recovery admission preserves every independent guard before live work.
//! Reaching the deliberately blocked asset proves admission only; numerical
//! continuation and fresh human decisions are exercised by the execution tests.
use super::*;
use crate::workflow::actor_service::learned::{anchors, campaigns};

const JOINT: &str = r#"{"id":71,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":500000,"maximum_false_stop_ppm":500000,"max_cases":4,"max_member_outcomes":16}"#;

fn joint_document(original: &str, joint: &str) -> String {
    format!(r#"{{"schema":"fa.learned-joint-publication/1","joint":{joint},"recipe":{original}}}"#)
}

fn recovered_document(original: &str) -> String {
    format!(
        r#"{{"schema":"fa.learned-recovery-policy/2","effective_policy":{{"generation":1,"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":512}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}},"campaigns":{{"cases":16,"input_bytes":1048576,"max_campaigns":4}},"recipe":{original}}}"#
    )
}

struct Selection {
    config: Config,
    paths: Vec<PathBuf>,
    originals: Vec<String>,
    documents: Vec<String>,
    args: Vec<String>,
    anchor: PathBuf,
    canonical: Vec<u8>,
    anchor_bytes: Vec<u8>,
}

impl Selection {
    fn new(root: &Root) -> Self {
        let (config, paths, entries) = selected(root, None, None);
        let originals = paths
            .iter()
            .map(|path| fs::read_to_string(path).unwrap())
            .collect::<Vec<_>>();
        let (governor, _, _) = selections(
            root,
            &entries[0].actor,
            std::process::id().checked_add(10_000).unwrap(),
        );
        let campaign = live_governor::Settings::read(&governor)
            .unwrap()
            .campaign_requirement();
        for (path, original) in paths.iter().zip(&originals) {
            fs::write(path, joint_document(original, JOINT)).unwrap();
        }

        // Ordinary creation freezes both guards in the original first image.
        // This baseline has no releases and makes no qualification claim.
        let mut first = recipe::load(&paths[0], &config, true).unwrap();
        campaigns::admit_selected(&mut first, campaign).unwrap();
        let (host, human, predictor, identity) =
            recovery::create_selected(&config, &first).unwrap();
        assert_eq!(
            host.held_out_joint_policy().unwrap(),
            first.generation.required_held_out_joint()
        );
        host.check_credibility().unwrap();
        let anchor = root.0.join("joint-admission.anchor");
        anchors::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
        drop((host, human, predictor, identity));
        let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
        let anchor_bytes = fs::read(&anchor).unwrap();
        fs::write(
            config.store.join("delivery.pending"),
            b"retained pre-admission staging",
        )
        .unwrap();

        let mut args = inputs(root, &config, &paths[1..], &entries[1..], None);
        let at = args
            .iter()
            .position(|arg| arg == "--campaign-profile")
            .unwrap();
        args.drain(at..at + 2);
        args.extend([
            "--after-message".into(),
            "1".into(),
            "--history-anchor".into(),
            anchor.display().to_string(),
        ]);
        let documents = originals
            .iter()
            .map(|original| recovered_document(&joint_document(original, JOINT)))
            .collect();
        let selected = Self {
            config,
            paths,
            originals,
            documents,
            args,
            anchor,
            canonical,
            anchor_bytes,
        };
        selected.restore();
        selected
    }

    fn restore(&self) {
        for (path, document) in self.paths.iter().zip(&self.documents) {
            fs::write(path, document).unwrap();
        }
    }

    fn unchanged(&self, root: &Root) {
        assert_eq!(
            fs::read(self.config.store.join("delivery.bin")).unwrap(),
            self.canonical
        );
        assert_eq!(
            fs::read(self.config.store.join("delivery.pending")).unwrap(),
            b"retained pre-admission staging"
        );
        assert_eq!(fs::read(&self.anchor).unwrap(), self.anchor_bytes);
        assert!(!root.0.join("final.anchor").exists());
        quiescent(root);
    }
}

fn block_asset(root: &Root) -> (PathBuf, Vec<u8>, String) {
    let path = root.0.join("fit-binding.json");
    let bytes = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    let error = format!("not an admitted regular file: {path:?}");
    (path, bytes, error)
}

#[test]
fn governed_joint_recovery_admits_its_capsule_selection_but_not_creation_or_forecast_rebinding() {
    let root = Root::new();
    let selected = Selection::new(&root);
    let governor = live_governor::Settings::read(&root.0.join("governor.json")).unwrap();
    let campaign = governor.campaign_requirement();
    let (_, _, asset_error) = block_asset(&root);
    let capsule = root.0.join("independent-capsule.json");
    for credibility in [None, Some(capsule.as_path())] {
        let error = resident::command_with_clock(&selected.args, credibility, || {
            panic!("admitted selection must still stop before the clock")
        })
        .unwrap_err();
        assert_eq!(error, asset_error);
        selected.unchanged(&root);
    }

    // Admission of an original recovered guard cannot enable it in a new live
    // governor. This decision precedes every asset, including the blocked one.
    for path in &selected.paths[1..] {
        assert!(recipe::GovernedRecipe::read_recovery(path, campaign).is_ok());
        let error = match recipe::GovernedRecipe::read(path, &selected.config, campaign) {
            Ok(_) => panic!("joint recovery recipe admitted as a new governor"),
            Err(error) => error,
        };
        assert!(
            error.contains("joint credibility requires explicit recovery"),
            "{error}"
        );
    }

    for owned in [false, true] {
        // Put the predictor in the LATER recipe: every recipe must be checked
        // before the first one's numerical/helper assets can be opened.
        fs::write(&selected.paths[2], &selected.originals[2]).unwrap();
        super::super::super::forecast::install(&root, &selected.paths[2], owned, 60_000);
        let predictive = fs::read_to_string(&selected.paths[2]).unwrap();
        fs::write(
            &selected.paths[2],
            recovered_document(&joint_document(&predictive, JOINT)),
        )
        .unwrap();
        let error = resident::command_with_clock(&selected.args, Some(&capsule), || {
            panic!("policy-bound forecast rebinding reached the clock")
        })
        .unwrap_err();
        assert!(
            error.contains("nonpredictive finite message continuation"),
            "{error}"
        );
        assert_ne!(error, asset_error);
        selected.unchanged(&root);
        selected.restore();
    }
    let error = resident::command_with_clock(&selected.args, Some(&capsule), || {
        panic!("restored selection must stop at its original asset")
    })
    .unwrap_err();
    assert_eq!(error, asset_error);
    selected.unchanged(&root);
}

#[test]
fn governed_joint_recovery_keeps_complete_policy_campaign_and_original_joint_pins() {
    let root = Root::new();
    let selected = Selection::new(&root);
    let (asset, bytes, asset_error) = block_asset(&root);
    let capsule = root.0.join("independent-capsule.json");
    for (from, to, expected) in [
        (
            "\"cases\":16",
            "\"cases\":17",
            "exact original campaign limits",
        ),
        (
            "\"input_bytes\":1048576",
            "\"input_bytes\":1048577",
            "exact original campaign limits",
        ),
        (
            "\"max_campaigns\":4",
            "\"max_campaigns\":5",
            "exact original campaign limits",
        ),
        (
            "\"payload_at_most\",\"maximum\":512",
            "\"payload_at_most\",\"maximum\":511",
            "same complete effective policy",
        ),
        (
            "\"effective_policy\":{\"generation\":1",
            "\"effective_policy\":{\"generation\":2",
            "same complete effective policy",
        ),
    ] {
        assert!(selected.documents[2].contains(from));
        fs::write(
            &selected.paths[2],
            selected.documents[2].replacen(from, to, 1),
        )
        .unwrap();
        let error = resident::command_with_clock(&selected.args, Some(&capsule), || {
            panic!("inexact policy/campaign reached the clock")
        })
        .unwrap_err();
        assert!(error.contains(expected), "{from}: {error}");
        assert_ne!(error, asset_error);
        selected.unchanged(&root);
        selected.restore();
    }

    // Joint equality uses the fully admitted original generation. Restore its
    // real asset, then check every policy field before any clock or replay.
    fs::remove_dir(&asset).unwrap();
    fs::write(&asset, bytes).unwrap();
    let alternatives = [
        ("\"id\":71", "\"id\":72"),
        ("\"generation\":1", "\"generation\":2"),
        ("\"minimum_safe_roots\":1", "\"minimum_safe_roots\":2"),
        (
            "\"minimum_violation_roots\":1",
            "\"minimum_violation_roots\":2",
        ),
        (
            "\"maximum_escape_ppm\":500000",
            "\"maximum_escape_ppm\":500001",
        ),
        (
            "\"maximum_false_stop_ppm\":500000",
            "\"maximum_false_stop_ppm\":500001",
        ),
        ("\"max_cases\":4", "\"max_cases\":5"),
        ("\"max_member_outcomes\":16", "\"max_member_outcomes\":17"),
    ];
    for joint in std::iter::once(None).chain(alternatives.into_iter().map(|(from, to)| {
        assert!(JOINT.contains(from));
        Some(JOINT.replacen(from, to, 1))
    })) {
        let original = &selected.originals[2];
        let inner = joint
            .as_deref()
            .map(|policy| joint_document(original, policy))
            .unwrap_or_else(|| original.clone());
        fs::write(&selected.paths[2], recovered_document(&inner)).unwrap();
        let error = resident::command_with_clock(&selected.args, Some(&capsule), || {
            panic!("inexact joint generation reached the clock")
        })
        .unwrap_err();
        assert!(
            error.contains("complete original generation or stream"),
            "{error}"
        );

        // Agreement between later recipes would not replace the original
        // guard. Its genuine recovery cursor also refuses before its Fence,
        // staging cleanup, governor issuance or numerical work.
        let mut wrong = recipe::load(&selected.paths[2], &selected.config, false).unwrap();
        wrong.recovery_anchor = Some(anchors::load(&selected.anchor).unwrap());
        assert!(
            recovery::open_governed_with_checkpoints(&selected.config, &wrong, &mut |_| Ok(()))
                .is_err()
        );
        selected.unchanged(&root);
        selected.restore();
    }

    // The matching guard reaches the SAME cursor's real finalizer. No request
    // predecessor is manufactured: this recovers the untouched baseline only.
    let mut exact = recipe::load(&selected.paths[2], &selected.config, false).unwrap();
    exact.recovery_anchor = Some(anchors::load(&selected.anchor).unwrap());
    let ((host, _human, _predictor, _identity), _governor) =
        recovery::open_governed_with_checkpoints(&selected.config, &exact, &mut |_| Ok(()))
            .unwrap();
    assert_eq!(
        host.held_out_joint_policy().unwrap(),
        exact.generation.required_held_out_joint()
    );
    assert_eq!(host.inspect().executions, 0);
    host.check_credibility().unwrap();
}

#[test]
fn governed_joint_capsule_path_cannot_become_the_governor_listener_before_assets_or_replay() {
    let root = Root::new();
    let selected = Selection::new(&root);
    let (_, _, asset_error) = block_asset(&root);
    let governor_socket = root.0.join("peers/governor.sock");
    let alias = root.0.join("capsule-parent");
    std::os::unix::fs::symlink(root.0.join("peers"), &alias).unwrap();
    for capsule in [governor_socket, alias.join("governor.sock")] {
        assert!(!capsule.exists());
        let error = resident::command_with_clock(&selected.args, Some(&capsule), || {
            panic!("capsule/listener custody collision reached the clock")
        })
        .unwrap_err();
        assert!(error.contains("governor socket conflicts"), "{error}");
        assert_ne!(error, asset_error);
        assert!(!capsule.exists());
        selected.unchanged(&root);
    }
    // The same absent, independently named capsule passes custody admission;
    // absence is not itself a reason to conflate its pathname with a listener.
    let capsule = root.0.join("independent-capsule.json");
    let error = resident::command_with_clock(&selected.args, Some(&capsule), || {
        panic!("noncolliding selection must stop at the admitted asset")
    })
    .unwrap_err();
    assert_eq!(error, asset_error);
    selected.unchanged(&root);
}
