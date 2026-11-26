//! Exact joint accounting on the original learned evaluation owner.
//! These fixtures defend custody and refusal; they do not qualify a committee.
use super::*;

#[test]
fn jointly_accounted_continuation_preserves_the_original_sample_and_separate_approval() {
    continued_profile(true);
}

#[test]
fn jointly_accounted_message_and_finish_keep_fresh_human_keys_and_original_labels() {
    message_then_finish_profile(ReviewDecision::Approve, true);
    message_then_finish_profile(ReviewDecision::Reject, true);
}

#[test]
fn jointly_accounted_recovery_stop_wins_at_each_original_replay_boundary() {
    stopped_profile(true);
}

#[test]
fn learned_recovery_requires_exact_joint_presence_thresholds_and_budget_before_replay() {
    for mismatch in 0..8 {
        let root = Root::new();
        // Exercise both marginal-over-joint and joint-over-marginal requests.
        let originally_joint = mismatch != 7;
        let original = prepared_profile(&root, false, originally_joint);
        let accounting = accounting_for(&original, &root, 0, "joint-mismatch");
        let exact = fs::read_to_string(&accounting).unwrap();
        let changed = match mismatch {
            0 => {
                let mut marginal = exact.clone();
                let start = marginal.find("  \"joint\": {").unwrap();
                let end = marginal.find("  \"evaluation\": {").unwrap();
                marginal.replace_range(start..end, "");
                marginal.replace("fa.native-andon-operator/2", "fa.native-andon-operator/1")
            }
            1 => exact.replace("\"id\": 71", "\"id\": 72"),
            2 => exact.replace("\"generation\": 1", "\"generation\": 2"),
            3 => exact.replace(
                "\"maximum_escape_rate\": {\"numerator\": 0, \"denominator\": 1}",
                "\"maximum_escape_rate\": {\"numerator\": 1, \"denominator\": 1}",
            ),
            4 => exact.replace(
                "\"maximum_benign_stop_rate\": {\"numerator\": 1, \"denominator\": 2}",
                "\"maximum_benign_stop_rate\": {\"numerator\": 1, \"denominator\": 3}",
            ),
            5 => exact.replace("\"cases\": 512", "\"cases\": 511"),
            6 => exact.replace("\"member_outcomes\": 2048", "\"member_outcomes\": 2047"),
            7 => super::super::with_joint_policy(&exact),
            _ => unreachable!(),
        };
        assert_ne!(changed, exact);
        fs::write(&accounting, changed).unwrap();
        let mut loaded = recipe::load(&root.0.join("recipe.json"), &original.config, true).unwrap();
        let (selection, reviewer) = live_selection(&root, &original, &mut loaded, &accounting);
        fs::write(
            original.config.store.join("delivery.pending"),
            b"unacknowledged staging",
        )
        .unwrap();
        let before = files(&original.config.store);
        let old_anchor = fs::read(&original.anchor).unwrap();
        let reads = original.config.source.status().read_attempts;
        let mut control = Control::new(&original.config, loaded.request, Some(&reviewer)).unwrap();
        let result = selection.open(
            &original.config,
            &loaded,
            &mut control,
            &deadline(),
            &mut || ElapsedTick(1000),
        );
        assert!(result.is_err(), "accepted joint mismatch {mismatch}");
        // A rejected declaration cannot replay/fence, discard pending bytes,
        // refresh inputs, issue evaluator roles or overwrite either anchor.
        assert_eq!(files(&original.config.store), before);
        assert_eq!(fs::read(&original.anchor).unwrap(), old_anchor);
        assert_eq!(original.config.source.status().read_attempts, reads);
        assert!(control.recovery_application().is_none());
        assert!(!selection.peer.socket_path().exists());
        assert!(!loaded.anchor_output.as_ref().unwrap().exists());

        // Only the independent declaration changes for this positive control.
        // The same original image and pending file now reach their sole Fence.
        fs::write(&accounting, exact).unwrap();
        let (exact_selection, _) = live_selection(&root, &original, &mut loaded, &accounting);
        let ((host, human, observer, identity), evaluator) = exact_selection
            .open(
                &original.config,
                &loaded,
                &mut control,
                &deadline(),
                &mut || ElapsedTick(1000),
            )
            .unwrap();
        let settings = andon_settings::Settings::read(&accounting).unwrap();
        assert_eq!(host.joint_credibility_policy().unwrap(), settings.joint);
        assert_eq!(settings.joint.is_some(), originally_joint);
        assert_eq!(host.revision(), original.before.revision + 1);
        assert_eq!(host.inspect().executions, original.before.executions);
        assert_eq!(
            host.learned_generation_inspection()
                .unwrap()
                .numerical
                .sampled_draws,
            1
        );
        assert_eq!(original.config.source.status().read_attempts, reads);
        assert_eq!(fs::read(&original.anchor).unwrap(), old_anchor);
        assert!(!original.config.store.join("delivery.pending").exists());
        assert!(observer.is_none() && identity.is_none());
        drop((evaluator, human, host));
    }
}
