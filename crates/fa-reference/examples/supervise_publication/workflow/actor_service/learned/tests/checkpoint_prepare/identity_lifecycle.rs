//! Identity lifecycle boundaries around fresh prompt-checkpoint preparation.
//! Complements the original matching/mismatch and asset-admission fixtures;
//! fixed stimuli exercise original custody and enforcement, not calibration.
use super::super::{functional_creation, identity_tests, peer_intake};
use super::*;
use crate::workflow::actor_service::learned::identity as check;
use fa_reference::action::consequence::{
    activation::identity::wire as identity_wire,
    delivery::persistent::observed::reviewer::wire::ReviewPacket,
    oversight::identity::{IdentityMismatch, IdentityOutcome},
};

fn next_measurement(case: &Case) {
    let before = case.reader().generation;
    let text = fs::read_to_string(&case.recipe).unwrap();
    assert_eq!(text.matches("\"check\":901").count(), 1);
    assert_eq!(text.matches("\"measurement_sequence\":1").count(), 1);
    fs::write(
        &case.recipe,
        text.replace("\"check\":901", "\"check\":902")
            .replace("\"measurement_sequence\":1", "\"measurement_sequence\":2"),
    )
    .unwrap();
    assert_eq!(case.reader().generation, before);
}

fn continue_prepared(
    root: &Root,
    case: &Case,
    decision: ReviewDecision,
) -> (Result<(), String>, Option<ReviewPacket>, PathBuf) {
    let (peer_path, peers) = functional_creation::peers_file(root, &case.config);
    let retained = root.0.join("continued-identity.anchor");
    let args = vec![
        "create-learned-generated".into(),
        case.config_path.display().to_string(),
        case.recipe.display().to_string(),
        peer_path.display().to_string(),
        "--continue-generation".into(),
        "--history-anchor".into(),
        case.anchor.display().to_string(),
        "--retain-history-anchor".into(),
        retained.display().to_string(),
    ];
    let mut human = None;
    let result = super::super::super::command_with_clock(&args, None, || {
        if human.is_none() && peers.socket(1).exists() {
            human = Some(peer_intake::human(peers.clone(), 1, decision));
        }
        ElapsedTick(1001)
    });
    (result, human.map(|peer| peer.join().unwrap()), retained)
}

fn reopen(case: &Case, anchor: &Path) -> FileOversight {
    let mut loaded = case.reader();
    loaded.recovery_anchor = Some(custody::load(anchor).unwrap());
    let (host, _, _, _) = recovery::open_selected(&case.config, &loaded).unwrap();
    assert!(host.identity_checks_required());
    host
}

fn no_request_or_effect(case: &Case) {
    let view = case.view();
    assert!(view.pending_request.is_none());
    assert_eq!(view.publication.executions, 0);
    assert_eq!(view.publication.control.ledger.charged, 0);
    assert_eq!(view.publication.control.ledger.reserved, 0);
    assert!(view.consistency.consistency.pending_attempt.is_none());
    assert_eq!(view.consistency.consistency.evidence.samples(), 0);
    assert_eq!(view.consistency.work, LearnedMonitorWork::default());
}

#[test]
fn completed_identity_expiring_before_installation_cannot_become_a_prepared_prompt() {
    for expired in [false, true] {
        let root = Root::new();
        let case = Case::new(&root, false);
        identity_tests::provision(&root, &case.recipe, false);
        let text = fs::read_to_string(&case.recipe).unwrap();
        fs::write(
            &case.recipe,
            text.replace("\"timeout_ticks\":10000", "\"timeout_ticks\":1")
                .replace("\"validity_ticks\":60000", "\"validity_ticks\":1"),
        )
        .unwrap();
        let mut calls = 0;
        let mut output = Vec::new();
        let result = prepare::command(
            &case.args(),
            None,
            || {
                calls += 1;
                // Startup/qualification use three clocks; native setup, all five
                // stimulus tokens and commit use the next 21. The later workflow
                // and installation checks must observe the expired tick themselves.
                ElapsedTick(if expired && calls >= 25 { 1001 } else { 1000 })
            },
            &mut output,
        );
        assert_eq!(result.is_err(), expired);
        no_request_or_effect(&case);
        if expired {
            assert!(output.is_empty());
        } else {
            assert_eq!(number(&report(&output), "position"), 2);
        }
        assert_eq!(case.view().publication.stop.is_some(), expired);
        let host = reopen(&case, &case.anchor);
        let historical = host.identity_report(901).unwrap();
        assert_eq!(historical.outcome, IdentityOutcome::Matched);
        assert_eq!(historical.observations.len(), 2);
        assert_eq!(historical.completed_at, Some(ElapsedTick(1000)));
        assert_eq!(historical.valid_until, ElapsedTick(1001));
        assert_eq!(host.identity_installation(901).unwrap().is_none(), expired);
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, if expired { 0 } else { 2 });
        assert_eq!(
            numerical.cumulative_work.admitted_tokens,
            if expired { 0 } else { 2 }
        );
        assert_eq!(numerical.cumulative_work.sampling_attempts, 0);
        assert_eq!(host.learned_stream_checkpoint(71).is_err(), expired);
    }
}

#[test]
fn native_and_workflow_deadlines_during_identity_leave_retained_stopped_originals() {
    for expired_tick in [11_000, 61_000] {
        let root = Root::new();
        let case = Case::new(&root, false);
        identity_tests::provision(&root, &case.recipe, false);
        let mut calls = 0;
        let mut output = Vec::new();
        let result = prepare::command(
            &case.args(),
            None,
            || {
                calls += 1;
                // Includes the three command/qualification ticks before the native
                // check's first post-token clock. No actor prompt token has begun.
                ElapsedTick(if calls >= 10 { expired_tick } else { 1000 })
            },
            &mut output,
        );
        assert!(result.is_err());
        assert!(output.is_empty());
        no_request_or_effect(&case);
        assert!(case.view().publication.stop.is_some());
        let host = reopen(&case, &case.anchor);
        assert_eq!(
            host.identity_report(901).unwrap().outcome,
            IdentityOutcome::Unavailable
        );
        assert!(host.identity_installation(901).unwrap().is_none());
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 0);
        assert_eq!(numerical.cumulative_work.admitted_tokens, 0);
        assert_eq!(numerical.cumulative_work.sampling_attempts, 0);
        assert!(host.learned_stream_checkpoint(71).is_err());
    }
}

#[test]
fn prepared_historical_match_cannot_replace_a_fresh_recovery_measurement() {
    for missing in [false, true] {
        let root = Root::new();
        let case = Case::new(&root, false);
        identity_tests::provision(&root, &case.recipe, false);
        case.prepare();
        next_measurement(&case);
        let path = root.0.join("observed.manifest");
        if missing {
            fs::remove_file(&path).unwrap();
        } else {
            let mut observed = identity_wire::decode_manifest(&fs::read(&path).unwrap()).unwrap();
            observed.weights = [9; 32];
            fs::write(path, identity_wire::encode_manifest(&observed)).unwrap();
        }
        let (result, packet, retained) = continue_prepared(&root, &case, ReviewDecision::Approve);
        assert!(result.is_err());
        assert!(packet.is_none());
        no_request_or_effect(&case);
        assert!(case.view().publication.stop.is_some());
        let host = reopen(&case, &retained);
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
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 0);
    }
}

#[test]
fn required_identity_cannot_proceed_without_its_separately_owned_observer() {
    let root = Root::new();
    let case = Case::new(&root, false);
    identity_tests::provision(&root, &case.recipe, false);
    let loaded = recipe::load_for_preparation(&case.recipe, &case.config).unwrap();
    let (host, reviewer, forecast, observer) =
        recovery::create_selected(&case.config, &loaded).unwrap();
    assert!(forecast.is_some());
    assert!(observer.is_some());
    let before = host.learned_generation_inspection().unwrap();
    let revision = host.revision();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    drop(port);
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut config = Config::read_learned_generation(&case.config_path).unwrap();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    let error = check::prepare(
        &mut driver,
        &mut config,
        &loaded,
        &mut None,
        (&mut control, &reviewer, &deadline()),
        &mut || panic!("absent identity custody reached live work"),
    )
    .unwrap_err();
    assert!(
        error.contains("missing learned identity observer custody"),
        "{error}"
    );
    {
        let host = driver.supervisor().host().unwrap();
        assert!(host.identity_checks_required());
        assert_eq!(host.revision(), revision);
        assert_eq!(host.learned_generation_inspection().unwrap(), before);
        assert!(host.learned_stream_checkpoint(71).is_err());
        assert!(host.request_status(loaded.request).is_err());
    }
    assert_eq!(cleanup(driver, 1000, 1), 0);
    drop(observer);
    drop(forecast);
}
