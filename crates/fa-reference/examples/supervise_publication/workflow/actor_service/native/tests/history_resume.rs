//! Anchored recovery of an actually interrupted native generation.
//! The same original work resumes; independent human rejection still prevents
//! publication. Fixture weights establish workflow behavior, not model quality.
use super::*;
use crate::workflow::actor_service::learned::anchors as archive;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::FileHistoryAnchor;
use std::path::Path;

fn selection(config: &Config, input: &Path, output: &Path) -> anchors::Selection {
    let args = vec![
        "serve-open".into(),
        "config".into(),
        "actor".into(),
        "reviewer".into(),
        "--native-text".into(),
        "recipe".into(),
        "--history-anchor".into(),
        input.display().to_string(),
        "--retain-history-anchor".into(),
        output.display().to_string(),
    ];
    let (_, options) = anchors::take_options(&args).unwrap();
    options.admit(config, true).unwrap().unwrap()
}

fn pending(root: &Root) -> (Config, Inputs, FileHistoryAnchor) {
    let mut config = configured(root);
    let native = inputs(root, &config);
    let selected = bootstrap::selection(&config, &native, None).unwrap();
    let (mut host, _) = bootstrap::prepare(&config, &native, selected, false).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000))
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(
        native.generation,
        numerical.actor_revision,
        numerical.position,
        native.request.clone(),
    )
    .unwrap();
    host.begin_decoder_text(host.revision(), command).unwrap();
    host.advance_decoder_generation(host.revision(), native.generation, 0)
        .unwrap();
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 1);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    config.programs = native_programs_at_epoch(root, 1);
    (config, native, anchor)
}

#[test]
fn native_anchored_pending_resume_preserves_work_and_requires_fresh_human_approval() {
    for approve in [true, false] {
        let root = Root::new();
        let (config, native, anchor) = pending(&root);
        let (actor, reviewer) = profiles(&root, &config);
        let retained = root.0.join("pending.anchor");
        archive::export_acknowledged(&anchor, &retained).unwrap();
        let original_archive = std::fs::read(&retained).unwrap();
        let output_path = root.0.join("completed.anchor");
        let history = selection(&config, &retained, &output_path);
        let selected = bootstrap::selection(&config, &native, None).unwrap();
        let store = config.store.clone();
        let profile = config.profile.clone();
        let decoder = native.decoder.clone();
        let tokenizer = native.tokenizer.clone();
        let before = FileOversight::read_decoder_text_progress(
            &store, &profile, &decoder, &tokenizer, native.generation,
        )
        .unwrap();
        let (sender, receiver) = mpsc::channel();
        let client = actor_thread(actor.clone(), receiver);
        let human = reviewing(
            reviewer.clone(),
            store.clone(),
            profile.clone(),
            if approve {
                ReviewDecision::Approve
            } else {
                ReviewDecision::Reject
            },
        );
        let mut output = ReferenceOutput {
            bytes: Vec::new(),
            sender: Some(sender),
        };
        serve_lifecycle_with_history(
            config,
            (&actor, &reviewer),
            native,
            (true, None, None, finish::Intent::Existing),
            (None, None, Some(&history)),
            || ElapsedTick(1001),
            &mut output,
        )
        .unwrap();
        let packet = human.join().unwrap();
        assert_eq!(packet.action().spec().policy_epoch, 1);
        let frame = ReleaseFrame::decode(&packet.action().spec().payload).unwrap();
        assert_eq!(frame.message(), Some("A"));
        assert!(frame.prior_messages().is_empty());
        assert!(!frame.is_finish());
        let response = client.join().unwrap();
        let after = FileOversight::read_decoder_text_progress(
            &store, &profile, &decoder, &tokenizer, 7,
        )
        .unwrap();
        assert_eq!(before.text.generation_revision(), 1);
        assert_eq!(after.text.command(), before.text.command());
        assert_eq!(after.text.generation_revision(), 4);
        assert_eq!(after.numerical.numerical.position, 4);
        assert_eq!(after.numerical.numerical.sampled_draws, 2);
        assert_eq!(after.publication.executions, u64::from(approve));
        assert_eq!(after.publication.control.ledger.epoch, 1);
        let saved = archive::load(&output_path).unwrap();
        assert_eq!(saved.revision(), after.publication.revision);
        assert_eq!(archive::load(&retained).unwrap(), anchor);
        assert_eq!(std::fs::read(&retained).unwrap(), original_archive);
        if approve {
            assert!(matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::Executed,
                    ..
                })
            ));
            assert_eq!(after.publication.payload, b"A");
            assert_eq!(
                after.publication.control.ledger.charged,
                packet.action().spec().units
            );
        } else {
            assert!(matches!(
                response.result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::CancelledBeforeDispatch,
                    ..
                })
            ));
            assert!(after.publication.payload.is_empty());
            assert_eq!(after.publication.control.ledger.charged, 0);
        }
        // Validate the retained output against the exact original canonical
        // image without replay, another Fence, or replacing the input archive.
        let canonical = std::fs::read(store.join("delivery.bin")).unwrap();
        FileOversight::begin_open_generated_text_stream_from_source_anchored(
            &store, profile, &decoder, &tokenizer, selected, &saved,
        )
        .unwrap()
        .cancel();
        assert_eq!(std::fs::read(store.join("delivery.bin")).unwrap(), canonical);
        assert!(!actor.socket.exists());
        assert!(!reviewer.socket(actor.request).exists());
        assert!(!crate::workflow::control::socket_path(&reviewer, actor.request).exists());
    }
}
