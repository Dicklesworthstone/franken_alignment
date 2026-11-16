//! Exact independently retained history is additional admission, never another
//! replay machine, fresh observation, effect approval or refund authority.
use super::*;

fn begin_anchored(
    root: &Directory,
    selected: GeneratedSourceProfile,
    anchor: &FileHistoryAnchor,
) -> Result<FileGeneratedSourceRecovery, JournalError> {
    FileOversight::begin_open_generated_text_stream_from_source_anchored(
        root.store(),
        profile(),
        &fixtures::stopped_config(),
        &tokenizer(false),
        selected,
        anchor,
    )
}

fn ready_anchored(
    root: &Directory,
    selected: GeneratedSourceProfile,
    anchor: &FileHistoryAnchor,
) -> FileGeneratedSourceRecovery {
    let mut run = begin_anchored(root, selected, anchor).unwrap();
    advance(&mut run);
    run
}

#[test]
fn exact_native_prefix_and_genuine_successors_resume_original_work_under_every_source_profile() {
    for mode in 0..6 {
        for successor in [false, true] {
            let root = Directory::new();
            let selected = selection(mode);
            let mut host = seed_pending(&root, selected);
            let anchor = host.history_anchor().unwrap();
            if successor {
                host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
                host.advance_decoder_text(host.revision(), 7, 1).unwrap();
            }
            let before = host.inspect();
            let numerical = host.decoder_inspection().unwrap().numerical;
            let original = host.decoder_text_progress(7).unwrap();
            let next_step = original.generation_revision();
            let expected_bytes = journal::encode_appended(
                &profile(),
                host.store.identity(),
                &host.events,
                &Event::Core(BaseEvent::Fence),
            )
            .unwrap();
            drop(host);
            let original_bytes = canonical(&root);
            fs::write(pending(&root), b"retain until original fenced finish").unwrap();

            let mut run = begin_anchored(&root, selected, &anchor).unwrap();
            assert_eq!(run.progress().attempted_events, 0);
            assert_eq!(run.machine.decoder_contract(), None);
            assert!(matches!(
                begin_anchored(&root, selected, &anchor),
                Err(JournalError::Busy)
            ));
            advance(&mut run);
            assert_eq!(canonical(&root), original_bytes);
            assert!(pending(&root).exists());
            let (mut host, _) = run.finish().unwrap();

            assert_eq!(canonical(&root), expected_bytes);
            assert!(!pending(&root).exists());
            assert_eq!(host.revision(), before.revision + 1);
            assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
            assert!(host.decoder_inspection().unwrap().paused);
            assert!(!host.clock_ready());
            assert_eq!(host.file_source_status().unwrap().capture.closed, None);
            assert_eq!(
                host.decoder_text_progress(7).unwrap().command(),
                original.command()
            );
            assert_eq!(
                host.decoder_text_progress(7).unwrap().generation_revision(),
                next_step
            );
            assert!(
                host.advance_decoder_text(host.revision(), 7, next_step)
                    .is_err()
            );

            let mut evidence = source(&root);
            host.refresh_file_source(host.revision(), &mut evidence, ElapsedTick(3))
                .unwrap();
            host.resume_decoder(
                host.revision(),
                numerical.actor_revision,
                numerical.position,
            )
            .unwrap();
            for step in next_step..4 {
                host.advance_decoder_text(host.revision(), 7, step).unwrap();
            }
            let complete = host.decoder_text_progress(7).unwrap();
            assert_eq!(complete.bytes().unwrap(), b"A");
            assert_eq!(complete.finish(), Some(Ok(GenerationFinish::StopToken)));
            assert_eq!(host.inspect().executions, 0);
            host.history_anchor_after(&anchor).unwrap();
        }
    }
}

#[test]
fn equal_counter_valid_native_fork_refuses_before_replay_or_staging_cleanup() {
    let root = Directory::new();
    let selected = selection(1);
    let mut host = seed_pending(&root, selected);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let anchor = host.history_anchor().unwrap();
    let original = canonical(&root);
    let before = host.inspect();
    let mut fork = host.events.clone();
    *fork.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    // A well-formed, semantically valid fork with the same revision, authority
    // sequence and numerical work defeats a mere counter/recipe comparison.
    let fork_machine = Machine::replay(&profile(), &fork).unwrap();
    assert_eq!(fork_machine.snapshot(fork.len()).revision, before.revision);
    assert_eq!(
        fork_machine.snapshot(fork.len()).control.sequence,
        before.control.sequence
    );
    selected.check(&fork).unwrap();
    selected.check_machine(&fork_machine).unwrap();
    let fork_bytes = journal::encode(&profile(), host.store.identity(), &fork).unwrap();
    assert_ne!(fork_bytes, original);
    drop(host);
    fs::write(root.store().join(storage::CANONICAL), &fork_bytes).unwrap();
    fs::write(pending(&root), b"fork must not clean staging").unwrap();

    // The pre-existing unanchored contract accepts this valid alternate history.
    // Only the independently retained prefix adds the missing refusal.
    ready(&root, selected).cancel();
    assert_eq!(
        begin_anchored(&root, selected, &anchor).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(canonical(&root), fork_bytes);
    assert_eq!(
        fs::read(pending(&root)).unwrap(),
        b"fork must not clean staging"
    );

    fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    assert!(ready_anchored(&root, selected, &anchor).finish().is_ok());
}

#[test]
fn shorter_valid_native_history_cannot_satisfy_a_newer_independent_anchor() {
    let root = Directory::new();
    let selected = selection(1);
    let mut host = seed_pending(&root, selected);
    let old = canonical(&root);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let anchor = host.history_anchor().unwrap();
    let current = canonical(&root);
    drop(host);
    fs::write(root.store().join(storage::CANONICAL), &old).unwrap();
    fs::write(pending(&root), b"retain rollback evidence").unwrap();

    ready(&root, selected).cancel();
    assert_eq!(
        begin_anchored(&root, selected, &anchor).err(),
        Some(Error::Stale.into())
    );
    assert_eq!(canonical(&root), old);
    assert_eq!(
        fs::read(pending(&root)).unwrap(),
        b"retain rollback evidence"
    );

    fs::write(root.store().join(storage::CANONICAL), current).unwrap();
    assert!(ready_anchored(&root, selected, &anchor).finish().is_ok());
}

#[test]
fn same_native_recipe_and_event_count_at_another_store_do_not_match_an_anchor() {
    let first = Directory::new();
    let second = Directory::new();
    let first_host = seed_pending(&first, selection(1));
    let foreign = first_host.history_anchor().unwrap();
    let second_host = seed_pending(&second, selection(1));
    let own = second_host.history_anchor().unwrap();
    assert_eq!(own.revision(), foreign.revision());
    let before = canonical(&second);
    drop(second_host);
    fs::write(pending(&second), b"retain other store staging").unwrap();

    assert_eq!(
        begin_anchored(&second, selection(1), &foreign).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(canonical(&second), before);
    assert_eq!(
        fs::read(pending(&second)).unwrap(),
        b"retain other store staging"
    );
    let revision = own.revision();
    let (host, _) = FileOversight::open_generated_text_stream_from_source_anchored(
        second.store(),
        profile(),
        &fixtures::stopped_config(),
        &tokenizer(false),
        selection(1),
        &own,
    )
    .unwrap();
    assert_eq!(host.revision(), revision + 1);
    assert!(host.decoder_inspection().unwrap().paused);
}

#[test]
fn matching_native_anchor_does_not_approve_invalid_unanchored_suffix() {
    let root = Directory::new();
    let selected = selection(1);
    let host = seed_pending(&root, selected);
    let anchor = host.history_anchor().unwrap();
    let original = canonical(&root);
    let mut events = host.events.clone();
    events.push(Event::Core(BaseEvent::Time(ElapsedTick(0))));
    anchor
        .check(&profile(), host.store.identity(), &events)
        .unwrap();
    let bytes = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    drop(host);
    fs::write(root.store().join(storage::CANONICAL), &bytes).unwrap();
    fs::write(pending(&root), b"retain invalid suffix").unwrap();

    let mut run = begin_anchored(&root, selected, &anchor).unwrap();
    assert_eq!(run.progress().attempted_events, 0);
    assert_eq!(run.advance(0, MAX_JOURNAL_EVENTS), Err(Error::Stale.into()));
    assert_eq!(
        run.progress().status,
        FileGeneratedSourceRecoveryStatus::Failed(Error::Stale)
    );
    assert_eq!(run.finish().err(), Some(Error::Stale.into()));
    assert_eq!(canonical(&root), bytes);
    assert_eq!(fs::read(pending(&root)).unwrap(), b"retain invalid suffix");

    fs::write(root.store().join(storage::CANONICAL), original).unwrap();
    assert!(ready_anchored(&root, selected, &anchor).finish().is_ok());
}

#[test]
fn matching_native_anchor_cannot_replace_original_model_tokenizer_source_or_publication_selection() {
    let root = Directory::new();
    let selected = selection(5);
    let host = seed_pending(&root, selected);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let before = canonical(&root);
    fs::write(pending(&root), b"retain selected contracts").unwrap();
    for changed in 0..5 {
        let decoder = if changed == 0 {
            fixtures::config(0.5, 65)
        } else {
            fixtures::stopped_config()
        };
        let tokenizer = tokenizer(changed == 1);
        let mut wrong = selected;
        match changed {
            2 => wrong.source.source.generation += 1,
            3 => wrong.publication = selection(1).publication,
            4 => {
                let GeneratedSourcePublication::Joint { publication, .. } =
                    &mut wrong.publication
                else {
                    unreachable!()
                };
                publication.feed.as_mut().unwrap().snapshot_fallback = false;
            }
            _ => {}
        }
        assert_eq!(
            FileOversight::begin_open_generated_text_stream_from_source_anchored(
                root.store(),
                profile(),
                &decoder,
                &tokenizer,
                wrong,
                &anchor,
            )
            .err(),
            Some(Error::Binding.into()),
            "{changed}"
        );
        assert_eq!(canonical(&root), before);
        assert_eq!(
            fs::read(pending(&root)).unwrap(),
            b"retain selected contracts"
        );
    }
    assert!(ready_anchored(&root, selected, &anchor).finish().is_ok());
}

#[test]
fn anchored_native_stop_keeps_original_unknown_liability_and_exact_retry_receipt() {
    let root = Directory::new();
    let host = unknown_message(&root);
    let anchor = host.history_anchor().unwrap();
    let before = host.inspect();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let stream = host.stream_snapshot().unwrap();
    assert!(before.control.ledger.charged > 0);
    drop(host);

    let run = ready_anchored(&root, selection(1), &anchor);
    assert_eq!(run.machine.broker.hosted_decoder().unwrap(), numerical);
    let request = run.stop_request(701).unwrap();
    let result = run.finish_admission_stop(request).unwrap();
    assert_eq!(result.appended_events(), 2);
    assert_eq!(result.snapshot().revision, before.revision + 2);
    assert_eq!(
        result.snapshot().control.ledger.charged,
        before.control.ledger.charged
    );
    assert_eq!(
        result.snapshot().control.ledger.stages,
        before.control.ledger.stages
    );
    assert_eq!(result.snapshot().payload, before.payload);
    assert_eq!(result.snapshot().executions, before.executions);
    assert!(!result.progress().drained());
    let stopped = canonical(&root);

    let run = ready_anchored(&root, selection(1), &anchor);
    assert_eq!(run.machine.broker.hosted_decoder().unwrap(), numerical);
    assert_eq!(
        run.machine.broker.stream_state().unwrap().1,
        &stream.confirmed
    );
    assert_eq!(run.machine.broker.stream_pending(), stream.pending);
    assert_eq!(run.machine.endpoint.stream_view(), Some(&stream.published));
    assert_eq!(run.stop_request(702), Err(Error::Duplicate.into()));
    assert_eq!(run.stop_request(701).unwrap(), request);
    let retried = run.finish_admission_stop(request).unwrap();
    assert_eq!(retried.appended_events(), 0);
    assert_eq!(retried.snapshot(), result.snapshot());
    assert_eq!(retried.history_anchor(), result.history_anchor());
    assert_eq!(canonical(&root), stopped);
}

#[test]
fn anchored_native_cursor_cancellation_and_ready_image_swap_preserve_original_files() {
    let root = Directory::new();
    let host = seed_pending(&root, selection(1));
    let anchor = host.history_anchor().unwrap();
    let events = host.events.len();
    drop(host);
    let before = canonical(&root);
    fs::write(pending(&root), b"never clean cancelled or swapped history").unwrap();
    for cut in [0, events / 2, events] {
        let mut run = begin_anchored(&root, selection(1), &anchor).unwrap();
        if cut != 0 {
            run.advance(0, cut).unwrap();
        }
        assert_eq!(
            run.cancel().status,
            FileGeneratedSourceRecoveryStatus::Cancelled
        );
        assert_eq!(canonical(&root), before);
        assert!(pending(&root).exists());
    }

    for stop in [false, true] {
        let run = ready_anchored(&root, selection(1), &anchor);
        let request = run.stop_request(701).unwrap();
        let mut changed = before.clone();
        changed.push(0);
        fs::write(root.store().join(storage::CANONICAL), &changed).unwrap();
        let result = if stop {
            run.finish_admission_stop(request).err()
        } else {
            run.finish().err()
        };
        assert_eq!(result, Some(Error::Binding.into()));
        assert_eq!(canonical(&root), changed);
        assert_eq!(
            fs::read(pending(&root)).unwrap(),
            b"never clean cancelled or swapped history"
        );
        fs::write(root.store().join(storage::CANONICAL), &before).unwrap();
    }
    assert!(
        ready_anchored(&root, selection(1), &anchor)
            .finish()
            .is_ok()
    );
}
