//! Exercise the service's real bootstrap and existing actor/helper/reviewer loop.
//! Sources, files and sockets are real; model weights and decisions are fixtures.
use super::*;
use fa_reference::action::consequence::oversight::policy_state::StateFreshness;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourceReplacement;

fn selected(root: &Root, config: &Config, mode: usize)
    -> Option<(PublicationProfile, FilePublicationProducer)>
{
    if mode == 0 { return None; }
    let (mut profile, producer) = publication(root, config, false);
    if mode == 2 {
        let inner = profile_json(root, config, false);
        let json = format!(r#"{{"schema":"fa.supervised-joint-publication/1","joint":{{"id":70,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":1,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":4,"max_member_outcomes":16}},"publication":{inner}}}"#);
        profile = PublicationProfile::decode(json.as_bytes()).unwrap();
    }
    Some((profile, producer))
}
fn seed_source(config: &mut Config, native: &Inputs, publication: Option<&PublicationProfile>) {
    let selected = bootstrap::selection(config, native, publication).unwrap();
    let (mut host, _) = bootstrap::prepare(config, native, selected, false).unwrap();
    assert_eq!(host.file_source_status().unwrap().policy, config.source_policy);
    assert!(host.file_source_status().unwrap().capture.closed.is_none());
    assert!(!host.clock_ready());
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(native.generation, n.actor_revision, n.position,
        native.request.clone()).unwrap();
    host.begin_decoder_text(host.revision(), command).unwrap();
    host.advance_decoder_text(host.revision(), native.generation, 0).unwrap();
}
fn image(config: &Config) -> Vec<u8> { std::fs::read(config.store.join("delivery.bin")).unwrap() }

#[test]
fn native_source_service_keeps_human_approval_and_source_free_receipts_in_every_mode() {
    for mode in 0..3 {
        for approve in [true, false] {
            let root = Root::new(); let config = configured(&root);
            let native = inputs(&root, &config); let (actor, reviewer) = profiles(&root, &config);
            let selected = selected(&root, &config, mode);
            let publication = selected.as_ref().map(|(profile, _)| profile);
            let store = config.store.clone(); let profile = config.profile.clone();
            let decoder = native.decoder.clone(); let tokenizer = native.tokenizer.clone();
            let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
            let human = reviewing(reviewer.clone(), store.clone(), profile.clone(),
                if approve { ReviewDecision::Approve } else { ReviewDecision::Reject });
            let mut original = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
            let result = serve_selected(config, &actor, &reviewer, native, (false, publication),
                || ElapsedTick(1000), &mut original);
            assert!(result.is_ok(), "mode={mode} approve={approve}: {result:?}");
            human.join().unwrap(); let first = client.join().unwrap();
            assert_eq!(executed(&first), approve);
            if !approve {
                assert!(matches!(first.result, Ok(Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch, .. })));
            }
            let before = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
            assert_eq!(before.text.bytes().unwrap(), b"A");
            assert_eq!(before.text.generation_revision(), 4);
            assert_eq!(before.publication.executions, u64::from(approve));
            let mut config = configured(&root); let native = inputs(&root, &config);
            config.programs.clear(); std::fs::remove_file(root.0.join("evidence.json")).unwrap();
            if mode != 0 { std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap(); }
            let (sender, receiver) = mpsc::channel(); let client = actor_thread(actor.clone(), receiver);
            let mut recovered = ReferenceOutput { bytes: Vec::new(), sender: Some(sender) };
            let result = serve_selected(config, &actor, &reviewer, native, (true, publication),
                || ElapsedTick(20000), &mut recovered);
            assert!(result.is_ok(), "{result:?}");
            let last = client.join().unwrap(); assert_eq!(executed(&last), approve);
            if !approve {
                assert!(matches!(last.result, Ok(Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch, .. })));
            }
            assert_eq!(recovered.bytes, original.bytes); // exact original expired document
            let after = FileOversight::read_decoder_text_progress(&store, &profile, &decoder, &tokenizer, 7).unwrap();
            assert!(after.numerical.paused);
            assert_eq!(after.numerical.numerical, before.numerical.numerical);
            assert_eq!(after.publication.payload, before.publication.payload);
            assert_eq!(after.publication.executions, before.publication.executions);
            assert_eq!(after.publication.control.ledger.charged, before.publication.control.ledger.charged);
            assert_eq!(after.publication.control.ledger.reserved, 0);
            assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
        }
    }
}

#[test]
fn native_source_service_bad_policy_cannot_leave_a_model_only_bootstrap() {
    for mode in 0..3 {
        let root = Root::new(); let mut config = configured(&root);
        let native = inputs(&root, &config); let (actor, reviewer) = profiles(&root, &config);
        let chosen = selected(&root, &config, mode);
        let publication = chosen.as_ref().map(|(profile, _)| profile);
        config.source_policy.limits.events = 0;
        let store = config.store.clone(); let mut output = Vec::new();
        assert!(serve_selected(config, &actor, &reviewer, native, (false, publication),
            || ElapsedTick(1000), &mut output).is_err());
        assert!(output.is_empty()); assert!(!store.exists());
        assert!(!actor.socket.exists()); assert!(!reviewer.socket(91).exists());
        let config = configured(&root); let native = inputs(&root, &config);
        let selected = bootstrap::selection(&config, &native, publication).unwrap();
        let (host, _) = bootstrap::prepare(&config, &native, selected, false).unwrap();
        assert_eq!(host.file_source_status().unwrap().policy, config.source_policy);
        assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
    }
}

#[test]
fn native_source_service_mismatch_preserves_staging_and_saved_generation_before_fencing() {
    for mode in 0..3 {
        let root = Root::new(); let mut config = configured(&root); let native = inputs(&root, &config);
        let chosen = selected(&root, &config, mode); let publication = chosen.as_ref().map(|(p, _)| p);
        seed_source(&mut config, &native, publication);
        let before = image(&config); let pending = config.store.join("delivery.pending");
        std::fs::write(&pending, b"retain rejected recovery staging").unwrap();
        for which in 0..5 {
            let mut bad = configured(&root); let native = inputs(&root, &bad);
            let (actor, reviewer) = profiles(&root, &bad);
            match which {
                0 => bad.source_policy.source.source += 1,
                1 => bad.source_policy.source.generation += 1,
                2 => bad.source_policy.limits.events -= 1,
                3 => bad.source_policy.limits.retained_bytes -= 1,
                _ => bad.source_policy.freshness = StateFreshness::new(
                    bad.source_policy.freshness.max_age_ticks() + 1).unwrap(),
            }
            let mut output = Vec::new();
            assert!(serve_selected(bad, &actor, &reviewer, native, (true, publication),
                || ElapsedTick(1001), &mut output).is_err());
            assert!(output.is_empty()); assert_eq!(image(&config), before);
            assert_eq!(std::fs::read(&pending).unwrap(), b"retain rejected recovery staging");
            assert!(!actor.socket.exists()); assert!(!reviewer.socket(91).exists());
        }
        let selected = bootstrap::selection(&config, &native, publication).unwrap();
        let (host, _) = bootstrap::prepare(&config, &native, selected, true).unwrap();
        assert!(!pending.exists()); assert!(host.decoder_inspection().unwrap().paused);
        assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 1);
        assert_eq!(host.decoder_inspection().unwrap().numerical.position, 1);
    }
}

#[test]
fn native_source_service_omitted_joint_selection_is_read_only_not_a_recovery_fence() {
    let root = Root::new(); let mut config = configured(&root); let native = inputs(&root, &config);
    let (joint, _producer) = selected(&root, &config, 2).unwrap();
    let ordinary = PublicationProfile::decode(profile_json(&root, &config, false).as_bytes()).unwrap();
    seed_source(&mut config, &native, Some(&joint));
    let before = image(&config); let pending = config.store.join("delivery.pending");
    std::fs::write(&pending, b"unverified original staging").unwrap();
    for selected in [None, Some(&ordinary)] {
        let config = configured(&root); let native = inputs(&root, &config);
        let (actor, reviewer) = profiles(&root, &config); let mut output = Vec::new();
        assert!(serve_selected(config, &actor, &reviewer, native, (true, selected),
            || ElapsedTick(1001), &mut output).is_err());
        assert!(output.is_empty()); assert_eq!(image(&configured(&root)), before);
        assert_eq!(std::fs::read(&pending).unwrap(), b"unverified original staging");
        assert!(!actor.socket.exists());
    }
    let selected = bootstrap::selection(&config, &native, Some(&joint)).unwrap();
    let (host, _) = bootstrap::prepare(&config, &native, selected, true).unwrap();
    assert!(host.held_out_joint_policy().unwrap().is_some());
    assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 1);
}

#[test]
fn native_source_service_opens_legacy_pending_work_and_validated_source_rotations() {
    for rotate in [false, true] {
        let root = Root::new(); let mut config = configured(&root); let native = inputs(&root, &config);
        let (publication, _producer) = publication(&root, &config, false);
        seed(&mut config, &native, &publication, 1); // original two-write bootstrap
        if rotate {
            let selected = bootstrap::selection(&config, &native, Some(&publication)).unwrap();
            let (mut host, _) = bootstrap::prepare(&config, &native, selected, true).unwrap();
            host.replace_file_source(host.revision(), FileSourceReplacement { operation: 70,
                expected_generation: config.source_policy.source.generation,
                expected_authority_epoch: host.inspect().control.ledger.epoch,
                next_generation: config.source_policy.source.generation + 1 }).unwrap();
            config.source_policy.source.generation += 1;
        }
        std::fs::remove_file(root.0.join("evidence.json")).unwrap();
        std::fs::remove_file(root.0.join("producer/delivery.bin")).unwrap();
        config.programs.clear();
        let selected = bootstrap::selection(&config, &native, Some(&publication)).unwrap();
        let (host, _) = bootstrap::prepare(&config, &native, selected, true).unwrap();
        assert_eq!(host.file_source_status().unwrap().policy, config.source_policy);
        assert!(host.file_source_status().unwrap().capture.closed.is_none());
        assert!(host.decoder_inspection().unwrap().paused);
        assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 1);
        assert_eq!(host.decoder_inspection().unwrap().numerical.position, 1);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn native_source_service_missing_live_capture_still_cannot_emit_a_reference() {
    let root = Root::new(); let config = configured(&root); let native = inputs(&root, &config);
    let (actor, reviewer) = profiles(&root, &config);
    std::fs::remove_file(root.0.join("evidence.json")).unwrap();
    let mut output = Vec::new();
    assert!(serve(config, &actor, &reviewer, native, || ElapsedTick(1000), &mut output).is_err());
    assert!(output.is_empty()); assert!(!reviewer.socket(91).exists()); assert!(!actor.socket.exists());
    let config = configured(&root); let native = inputs(&root, &config);
    let selected = bootstrap::selection(&config, &native, None).unwrap();
    let (host, _) = bootstrap::prepare(&config, &native, selected, true).unwrap();
    assert_eq!(host.file_source_status().unwrap().policy, config.source_policy);
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
    assert_eq!(host.inspect().executions, 0); assert!(host.inspect().stop.is_some());
}
