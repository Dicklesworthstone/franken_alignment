//! Original canonical cuts, source pins, guarded reopen and receipt reconciliation.
use super::*;
use crate::action::ActionState;
use crate::action::consequence::delivery::persistent::{JournalIo, codec::shared::{Reader, Writer}, storage};
use crate::action::consequence::delivery::persistent::observed::{journal, decoder::DecoderEvent,
    guarded::{FileGuardSet, FileRecoveryRequirements, FileRecoveryFloor}};

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];
fn requirements(c: &FileDecoderConfig) -> FileRecoveryRequirements {
    FileRecoveryRequirements {
        guards: FileGuardSet { stream: Some(stream()), decoder: Some(c.clone()), decoder_stop: None,
            source: None, identity: None, campaigns: None, credential: None },
        effective_policy: profile().delivery.policy, credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 },
    }
}
fn read(root: &Directory, c: &FileDecoderConfig) -> Result<super::super::FileTextMessageSnapshot, JournalError> {
    FileOversight::read_decoder_text_message(root.store(), &profile(), c, &tokenizer(false), stream(), 91)
}
fn failure(error: JournalError, barrier: JournalIo) {
    let JournalError::Io(failure) = error else { panic!("expected original Store failure"); };
    assert_eq!(failure.operation, barrier);
    assert_eq!(failure.replacement_may_be_visible, matches!(barrier, JournalIo::Rename | JournalIo::DirectorySync));
}

#[test]
fn generated_message_one_image_read_retains_origin_after_new_inference_and_guarded_recovery() {
    let root = Directory::new(); let c = fixtures::stopped_config(65);
    let (mut host, _) = start(&root, profile(), &c);
    generate(&mut host, 7, request(b"ab", 2));
    submit(&mut host, 91, 7);
    let original = host.decoder_text_message_snapshot(91).unwrap();
    assert_eq!(original.source.generation, 7); assert_eq!(original.generation.bytes().unwrap(), b"A");
    let before = bytes(&host); let state = host.inspect();
    let readback = read(&root, &c).unwrap();
    assert_eq!(readback.source, original.source); assert_eq!(readback.status, original.status);
    assert_eq!(readback.stream.publication, state); assert_eq!(bytes(&host), before);
    assert_eq!(readback.generation.generation().work(), original.generation.generation().work());
    host.cancel_request(host.revision(), 91).unwrap();
    generate(&mut host, 8, request(b"ab", 2));
    let newer_position = host.decoder_inspection().unwrap().numerical.position;
    let anchor = host.history_anchor().unwrap(); drop(host);
    let (mut recovered, _roles) = FileOversight::open_guarded_text_anchored(root.store(), profile(),
        &requirements(&c), &tokenizer(false), &anchor).unwrap();
    assert!(!recovered.clock_ready()); assert!(recovered.decoder_inspection().unwrap().paused);
    assert_eq!(recovered.decoder_inspection().unwrap().numerical.position, newer_position);
    let historical = recovered.decoder_text_message_snapshot(91).unwrap();
    assert_eq!(historical.source, original.source);
    assert_eq!(historical.generation.bytes().unwrap(), b"A");
    assert!(matches!(historical.status.disposition,
        FileRequestDisposition::Admitted { stage: ActionState::Cancelled, .. }));
    assert!(historical.generation.generation().end_position() < newer_position);
    let before = bytes(&recovered);
    recovered.submit_decoder_text_message(0, original.source, Snapshot::default()).unwrap();
    assert_eq!(bytes(&recovered), before); assert!(!recovered.clock_ready());
    assert_eq!(read(&root, &c).unwrap().status, historical.status);
}

#[test]
fn generated_message_admission_barriers_never_split_source_from_original_request() {
    for barrier in BARRIERS {
        let root = Directory::new(); let c = fixtures::stopped_config(65);
        let (mut host, _) = start(&root, profile(), &c);
        generate(&mut host, 7, request(b"ab", 2));
        let source = submission(&host, 91, 7);
        let n = host.decoder_inspection().unwrap().numerical;
        let rev = host.revision(); let anchor = host.history_anchor().unwrap();
        host.store.fail_once(barrier);
        let error = host.submit_decoder_text_message(rev, source.clone(), snapshot()).unwrap_err();
        failure(error, barrier);
        assert!(matches!(host.decoder_text_message_snapshot(91), Err(JournalError::Unavailable)));
        assert!(matches!(host.submit_decoder_text_message(0, source.clone(), snapshot()), Err(JournalError::Unavailable)));
        let visible = barrier == JournalIo::DirectorySync;
        let disk = read(&root, &c);
        if visible {
            let disk = disk.unwrap();
            assert_eq!(disk.source, source); assert_eq!(disk.stream.publication.revision, rev + 1);
            assert_eq!(disk.generation.bytes().unwrap(), b"A");
            assert!(matches!(disk.status.disposition, FileRequestDisposition::Admitted { .. }));
            assert_eq!(disk.stream.published.message_count(), 0);
        } else { assert!(matches!(disk, Err(JournalError::Contract(Error::Missing)))); }
        drop(host);
        let (mut h, _) = FileOversight::open_guarded_text_anchored(root.store(), profile(),
            &requirements(&c), &tokenizer(false), &anchor).unwrap();
        assert_eq!(h.decoder_inspection().unwrap().numerical, n);
        if visible {
            let before = bytes(&h);
            let status = h.submit_decoder_text_message(0, source, snapshot()).unwrap();
            assert!(matches!(status.disposition, FileRequestDisposition::Admitted { stage: ActionState::Cancelled, .. }));
            assert_eq!(bytes(&h), before);
        } else {
            assert!(matches!(h.decoder_text_message_request(91), Err(JournalError::Contract(Error::Missing))));
            assert!(matches!(h.submit_decoder_text_message(h.revision(), source, snapshot()),
                Err(JournalError::Contract(Error::Incomplete))));
        }
        assert_eq!(h.stream_snapshot().unwrap().published.message_count(), 0);
        assert_eq!(h.inspect().executions, 0);
    }
}

#[test]
fn generated_message_read_pins_stream_tokenizer_and_model_before_invalid_suffix_replay() {
    let root = Directory::new(); let c = fixtures::stopped_config(65);
    let (mut host, _) = start(&root, profile(), &c);
    generate(&mut host, 7, request(b"ab", 2)); submit(&mut host, 91, 7);
    let canonical = bytes(&host); let mut events = host.events.clone();
    events.push(Event::Decoder(DecoderEvent::CancelGeneration { id: 999, revision: 0 }));
    let malformed = journal::encode(&profile(), host.store.identity(), &events).unwrap();
    let path = root.store().join(storage::CANONICAL);
    std::fs::write(&path, &malformed).unwrap();
    // Right structure and source prefix do not excuse a semantically bad tail.
    assert!(read(&root, &c).is_err());
    assert!(matches!(FileOversight::read_decoder_text_message(root.store(), &profile(), &c,
        &tokenizer(true), stream(), 91), Err(JournalError::Contract(Error::Binding))));
    assert!(matches!(FileOversight::read_decoder_text_message(root.store(), &profile(), &fixtures::stopped_config(258),
        &tokenizer(false), stream(), 91), Err(JournalError::Contract(Error::Binding))));
    let other = StreamProfile::new(9, 2, 4, 64, 256).unwrap();
    assert!(matches!(FileOversight::read_decoder_text_message(root.store(), &profile(), &c,
        &tokenizer(false), other, 91), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(&path).unwrap(), malformed);
    std::fs::write(&path, &canonical).unwrap();
    assert_eq!(read(&root, &c).unwrap().generation.bytes().unwrap(), b"A");
    // Forgery at the SOURCE record also fails the original semantic replay.
    for field in 0..4 {
        let mut events = host.events.clone();
        let Event::TextMessage(source, _) = events.last_mut().unwrap() else { panic!("source event"); };
        match field { 0 => source.generation = 999, 1 => source.generation_revision = 1,
            2 => source.target.object += 1, _ => source.policy_epoch += 1 }
        let bad = journal::encode(&profile(), host.store.identity(), &events).unwrap();
        std::fs::write(&path, &bad).unwrap(); assert!(read(&root, &c).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bad);
    }
    std::fs::write(&path, canonical).unwrap();
    assert!(read(&root, &c).is_ok());
}

#[test]
fn generated_message_tag31_has_independent_bytes_and_closed_bounds() {
    let input = FileTextMessageRequest { request: 1, generation: 2, generation_revision: 3,
        target: ResolvedTarget { adapter: 4, object: 5, contract_version: 6, expected_version: 7, generation: 8 },
        policy_epoch: 9, deadline: ElapsedTick(10) };
    let expected = (1_u64..=10).flat_map(u64::to_be_bytes).collect::<Vec<_>>();
    let mut writer = Writer::new(80);
    write_request(&mut writer, &input).unwrap(); assert_eq!(writer.finish(), expected);
    let mut reader = Reader::new(&expected);
    assert_eq!(read_request(&mut reader).unwrap(), input); reader.end().unwrap();
    for end in 0..expected.len() { assert!(read_request(&mut Reader::new(&expected[..end])).is_err()); }
    for (offset, value) in [(0, 0), (8, 0), (16, MAX_GENERATION_TOKENS as u64 + 1)] {
        let mut bad = expected.clone(); bad[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
        assert!(read_request(&mut Reader::new(&bad)).is_err());
    }
    assert!(write_request(&mut Writer::new(79), &input).is_err());
    let root = Directory::new(); let c = fixtures::stopped_config(65);
    let (host, _) = start(&root, profile(), &c);
    let event = Event::TextMessage(Box::new(input), snapshot());
    let encoded = journal::encode(&profile(), host.store.identity(), &[event]).unwrap();
    // Hand-built body: tag 31, ten big-endian integers, then the original empty
    // complete semantic snapshot (epoch 1, complete 1, map count 0).
    let mut body = vec![31]; body.extend_from_slice(&expected);
    body.extend_from_slice(&1_u64.to_be_bytes()); body.push(1); body.extend_from_slice(&0_u32.to_be_bytes());
    let mut framed = (body.len() as u32).to_be_bytes().to_vec(); framed.extend(body);
    assert!(encoded.ends_with(&framed));
    let parsed = journal::decode(&profile(), host.store.identity(), &encoded).unwrap();
    assert_eq!(journal::encode(&profile(), host.store.identity(), &parsed).unwrap(), encoded);
    assert!(Machine::replay(&profile(), &parsed).is_err()); // syntax is not authority
}

#[test]
fn generated_message_capacity_and_source_interruption_do_not_create_partial_admissions() {
    for maximum in [6, 7] {
        let root = Directory::new(); let c = fixtures::stopped_config(65); let mut p = profile();
        p.delivery.limits.events = maximum;
        let (mut host, _) = start(&root, p, &c);
        generate(&mut host, 7, request(b"ab", 2)); assert_eq!(host.revision(), 6);
        let input = submission(&host, 91, 7);
        if maximum == 6 {
            assert!(matches!(unchanged_failure(&mut host, input), JournalError::Contract(Error::Limit)));
        } else {
            let before = bytes(&host); host.source_interrupted = true;
            assert!(matches!(host.submit_decoder_text_message(host.revision(), input.clone(), snapshot()),
                Err(JournalError::Contract(Error::Incomplete))));
            assert_eq!(bytes(&host), before); assert!(host.source_interrupted);
            // Restore only the private test seam, not a public repair operation.
            host.source_interrupted = false;
            host.submit_decoder_text_message(host.revision(), input.clone(), snapshot()).unwrap();
            assert_eq!(host.revision(), 7);
            host.source_interrupted = true;
            let before = bytes(&host);
            host.submit_decoder_text_message(0, input, Snapshot::default()).unwrap();
            assert_eq!(bytes(&host), before); assert!(host.source_interrupted);
            assert!(host.decoder_text_message_snapshot(91).is_ok());
        }
    }
}

#[test]
fn generated_message_missing_or_lost_forecast_matches_original_refusal() {
    use crate::action::consequence::activation::{CaptureProfile,
        consistency::{BinaryForecast, ErrorBudget, ForecastRegistration}};
    use crate::action::consequence::delivery::persistent::observed::consistency::{FileConsistencyConfig, FileConsistencyParameters};
    let pair = BinaryForecast::new(16384, 49152).unwrap();
    let config = FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 1, probe_generation: 1,
        profile: CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 },
        weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 19, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 16, max_prediction_age_ticks: 10,
    }).unwrap();
    for lost in [false, true] {
        let a = Directory::new(); let b = Directory::new(); let c = fixtures::stopped_config(65);
        let (mut bound, _) = start(&a, profile(), &c); let (mut raw, _) = start(&b, profile(), &c);
        for h in [&mut bound, &mut raw] {
            let observer = h.enable_action_consistency(h.revision(), config.clone()).unwrap();
            generate(h, 7, request(b"ab", 2));
            if lost { let revision = h.revision(); observer.unavailable(h, revision).unwrap(); }
        }
        let source = submission(&bound, 91, 7);
        let actual = bound.submit_decoder_text_message(bound.revision(), source, snapshot()).unwrap();
        let spec = raw.stream_message_spec("A", ElapsedTick(100)).unwrap();
        let expected = raw.submit_request(raw.revision(), 91, spec, snapshot()).unwrap();
        assert_eq!(actual, expected); assert!(matches!(actual.disposition, FileRequestDisposition::NotAdmitted(_)));
        let state = bound.action_consistency_snapshot().unwrap();
        assert_eq!(state, raw.action_consistency_snapshot().unwrap());
        assert_eq!(state.coverage_lost, lost); assert_eq!(state.evidence.samples(), 0);
        assert_eq!(bound.inspect().control, raw.inspect().control);
        assert_eq!(bound.stream_snapshot().unwrap().published.message_count(), 0);
        let disk = read(&a, &c).unwrap(); assert_eq!(disk.status, actual);
        assert_eq!(disk.generation.bytes().unwrap(), b"A"); // provenance is not permission
    }
}

#[test]
fn generated_message_published_unconfirmed_cut_survives_reopen_without_resending() {
    for barrier in BARRIERS {
        let root = Directory::new(); let c = fixtures::stopped_config(65);
        let (mut host, reviewer) = start(&root, profile(), &c);
        generate(&mut host, 7, request(b"ab", 2)); let id = attempt(submit(&mut host, 91, 7));
        let keys = review(&mut host, &reviewer, 91);
        host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
        let anchor = host.history_anchor().unwrap(); let charged = host.inspect().control.ledger.charged;
        let source = host.decoder_text_message_request(91).unwrap().clone();
        host.store.fail_once(barrier);
        failure(host.publish_checked(host.revision(), id, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap_err(), barrier);
        let disk = read(&root, &c).unwrap(); let visible = barrier == JournalIo::DirectorySync;
        assert_eq!(disk.source, source); assert_eq!(disk.generation.bytes().unwrap(), b"A");
        assert_eq!(disk.stream.published.message_count(), usize::from(visible));
        assert_eq!(disk.stream.confirmed.message_count(), 0); assert_eq!(disk.stream.pending, Some(id));
        drop(host);
        let (mut host, _) = FileOversight::open_guarded_text_anchored(root.store(), profile(),
            &requirements(&c), &tokenizer(false), &anchor).unwrap();
        let before = bytes(&host);
        host.submit_decoder_text_message(0, source, Snapshot::default()).unwrap();
        assert_eq!(bytes(&host), before); // never a fresh dispatch or publication
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        if visible {
            host.reconcile(host.revision(), id).unwrap();
            assert_eq!(host.inspect().control.ledger.charged, charged);
        } else {
            assert!(host.publish_checked(host.revision(), id, Some(&keys.inputs), snapshot(), ElapsedTick(2)).is_err());
            host.seal_unexecuted(host.revision(), id).unwrap();
            assert_eq!(host.inspect().control.ledger.charged, 0);
        }
        let final_cut = read(&root, &c).unwrap();
        assert_eq!(final_cut.stream.published, final_cut.stream.confirmed);
        assert_eq!(final_cut.stream.published.message_count(), usize::from(visible));
        assert_eq!(final_cut.stream.publication.executions, u64::from(visible));
        assert_eq!(final_cut.stream.pending, None);
    }
}

#[test]
fn generated_message_later_inference_invalidates_dispatch_and_first_publication() {
    for dispatched in [false, true] {
        let root = Directory::new(); let c = fixtures::stopped_config(65);
        let (mut host, reviewer) = start(&root, profile(), &c);
        generate(&mut host, 7, request(b"ab", 2)); let id = attempt(submit(&mut host, 91, 7));
        let keys = review(&mut host, &reviewer, 91);
        if dispatched {
            host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
        }
        let numerical = host.decoder_inspection().unwrap().numerical;
        host.advance_decoder_forced(host.revision(), numerical.actor_revision, numerical.position, 257,
            DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap().unwrap();
        if dispatched {
            let result = host.publish_checked(host.revision(), id, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
            assert!(!matches!(result.outcome, EndpointOutcome::Executed { .. }));
            host.reconcile(host.revision(), id).unwrap();
            assert_eq!(host.inspect().control.ledger.charged, 0);
        } else {
            assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
        }
        let historical = host.decoder_text_message_snapshot(91).unwrap();
        assert_eq!(historical.generation.bytes().unwrap(), b"A");
        assert_eq!(historical.stream.published.message_count(), 0);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn generated_message_batched_completion_keeps_token_revision_and_original_receipt() {
    let root = Directory::new(); let c = fixtures::stopped_config(65);
    let (mut host, _) = start(&root, profile(), &c);
    let cmd = command(&host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), cmd).unwrap();
    let partial = host.advance_decoder_text_batch(host.revision(), 7, 0, 3).unwrap();
    assert_eq!(partial.bytes().unwrap(), b"A"); assert!(!partial.is_complete());
    let input = submission(&host, 91, 7); unchanged_failure(&mut host, input);
    let complete = host.advance_decoder_text_batch(host.revision(), 7, 3, 4).unwrap();
    assert_eq!(complete.generation_revision(), 4);
    assert_eq!(complete.finish(), Some(Ok(GenerationFinish::StopToken)));
    let numerical = host.decoder_inspection().unwrap().numerical;
    let before = host.revision(); submit(&mut host, 91, 7);
    let linked = read(&root, &c).unwrap();
    assert_eq!(host.revision(), before + 1);
    assert_eq!(linked.source.generation_revision, 4);
    assert_eq!(linked.generation.bytes().unwrap(), b"A");
    assert_eq!(linked.generation.generation().work(),
        complete.numerical().receipt().unwrap().result().unwrap().work());
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(linked.stream.published.message_count(), 0);
}
