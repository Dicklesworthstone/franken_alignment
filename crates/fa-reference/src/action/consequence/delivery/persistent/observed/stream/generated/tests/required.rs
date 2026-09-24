//! The same original numerical/authority path, now with mandatory source linkage.
use super::*;
use crate::action::consequence::delivery::persistent::observed::journal;
use crate::action::consequence::oversight::actor::{ActorProposal, ActorError};

fn required(root: &Directory, c: &FileDecoderConfig) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_generated_text_stream(root.store(), profile(),
        stream(), c.clone(), tokenizer(false)).unwrap();
    assert!(host.generated_text_stream_required().unwrap());
    assert_eq!(host.revision(), 3);
    assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 0);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}

#[test]
fn generated_message_required_blocks_raw_bytes_but_preserves_native_and_legacy_admission() {
    let root = Directory::new(); let legacy_root = Directory::new();
    let c = fixtures::stopped_config(65);
    let (mut host, _) = required(&root, &c);
    let (mut legacy, _) = start(&legacy_root, profile(), &c);
    assert!(!legacy.generated_text_stream_required().unwrap());
    for owner in [&mut host, &mut legacy] { generate(owner, 7, request(b"ab", 2)); }
    let spec = host.stream_message_spec("A", ElapsedTick(100)).unwrap();
    let before = bytes(&host); let numerical = host.decoder_inspection().unwrap();
    assert!(matches!(host.submit_request(host.revision(), 91, spec.clone(), snapshot()),
        Err(JournalError::Contract(Error::Binding))));
    assert!(matches!(host.propose(host.revision(), 1, spec.clone(), snapshot()),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
    assert!(host.storage_failure().is_none()); assert_eq!(host.machine.requests.len(), 0);
    assert!(matches!(host.request_status(91), Err(JournalError::Contract(Error::Missing))));
    let expected = legacy.submit_request(legacy.revision(), 91, spec.clone(), snapshot()).unwrap();
    let actual = submit(&mut host, 91, 7);
    assert_eq!(actual, expected); assert_eq!(host.request_action(91).unwrap().spec(), &spec);
    assert_eq!(host.inspect().control, legacy.inspect().control);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical.numerical);
    // An exact ordinary retry reads the ALREADY source-linked record. It creates
    // no new message and cannot replace its provenance or adopt another output.
    let before = bytes(&host);
    assert_eq!(host.submit_request(0, 91, spec, Snapshot::default()).unwrap(), actual);
    assert_eq!(bytes(&host), before);
    assert_eq!(host.decoder_text_message_request(91).unwrap().generation, 7);
}

#[test]
fn generated_message_required_is_enforced_at_the_existing_actor_gateway() {
    let root = Directory::new(); let (mut host, _) = required(&root, &fixtures::stopped_config(65));
    generate(&mut host, 7, request(b"ab", 2));
    let spec = host.stream_message_spec("A", ElapsedTick(100)).unwrap();
    let proposal = ActorProposal { target: spec.target.unwrap(), payload: spec.payload,
        expected_policy_epoch: spec.policy_epoch, deadline: spec.deadline, units: spec.units };
    let before = bytes(&host); let rev = host.revision();
    let (port, mut supervisor) = host.into_actor_gateway();
    supervisor.set_snapshot(rev, Some(snapshot())).unwrap();
    assert_eq!(port.submit(91, &proposal).unwrap_err(), ActorError::IdempotencyConflict);
    let h = supervisor.host().unwrap();
    assert_eq!(bytes(&h), before); assert!(h.storage_failure().is_none());
    assert_eq!(h.inspect().executions, 0); assert_eq!(h.machine.requests.len(), 0);
    drop(h);
    let mut h = supervisor.host_mut().unwrap();
    assert!(matches!(submit(&mut h, 91, 7).disposition, FileRequestDisposition::Admitted { .. }));
}

#[test]
fn generated_message_required_reaches_original_two_key_publication_without_extra_inference() {
    let root = Directory::new(); let (mut host, reviewer) = required(&root, &fixtures::stopped_config(65));
    generate(&mut host, 7, request(b"ab", 2));
    let id = attempt(submit(&mut host, 91, 7));
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert!(host.publish(host.revision(), id).is_err());
    let keys = review(&mut host, &reviewer, 91);
    host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
    let cut = host.stream_snapshot().unwrap();
    assert_eq!(cut.published.message_count(), 0); assert_eq!(cut.confirmed.message_count(), 0);
    host.publish_checked(host.revision(), id, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    let cut = host.stream_snapshot().unwrap();
    assert_eq!(cut.published.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(cut.confirmed.message_count(), 0); assert_eq!(cut.pending, Some(id));
    host.reconcile(host.revision(), id).unwrap();
    let cut = host.stream_snapshot().unwrap();
    assert_eq!(cut.confirmed, cut.published); assert_eq!(cut.pending, None);
    assert_eq!(cut.publication.executions, 1);
    assert_eq!(cut.publication.control.ledger.charged, keys.action.spec().units);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.decoder_text_message_snapshot(91).unwrap().generation.bytes().unwrap(), b"A");
    // Finishing is an ordinary, separately reviewed NON-message effect. Its
    // original builder includes the confirmed history; it grants no next message.
    let finish = host.stream_finish_spec(ElapsedTick(100)).unwrap();
    let frame = ReleaseFrame::decode(&finish.payload).unwrap(); assert_eq!(frame.message(), None);
    let status = host.submit_request(host.revision(), 92, finish.clone(), snapshot()).unwrap();
    assert!(matches!(status.disposition, FileRequestDisposition::Admitted { .. }));
    assert_eq!(host.request_action(92).unwrap().spec(), &finish);
    assert!(matches!(host.decoder_text_message_request(92), Err(JournalError::Contract(Error::Missing))));
    assert_eq!(host.inspect().executions, 1); // neither proposal nor completion marker is publication
}

#[test]
fn generated_message_required_replay_refuses_removing_only_the_native_source_link() {
    let root = Directory::new(); let (mut host, _) = required(&root, &fixtures::stopped_config(65));
    generate(&mut host, 7, request(b"ab", 2)); submit(&mut host, 91, 7);
    let spec = host.request_action(91).unwrap().spec().clone();
    let mut forged = host.events.clone();
    let last = forged.last_mut().unwrap(); assert!(matches!(last, Event::TextMessage(..)));
    *last = Event::Core(BaseEvent::SubmitRequest(91, spec, snapshot()));
    assert!(matches!(Machine::replay(&profile(), &forged), Err(Error::Binding)));
    // This is not merely malformed text or a bad numerical witness: the same
    // raw submission is valid under the explicitly legacy caller-text profile.
    forged[0] = Event::StreamBootstrap(stream());
    let legacy = Machine::replay(&profile(), &forged).unwrap();
    assert_eq!(legacy.requests.status(91).unwrap(), host.request_status(91).unwrap());
    assert_eq!(legacy.broker.hosted_replay_bytes().unwrap(), host.machine.broker.hosted_replay_bytes().unwrap());
    let original = Machine::replay(&profile(), &host.events).unwrap();
    assert!(original.generated_text_only);
    assert_eq!(original.snapshot(host.events.len()), host.inspect());
}

#[test]
fn generated_message_required_bootstrap_cannot_retrofit_disable_or_reset_the_stream() {
    for first in [Event::Core(BaseEvent::Time(ElapsedTick(1))), Event::StreamBootstrap(stream()),
        Event::GeneratedStreamBootstrap(stream()), Event::Core(BaseEvent::Fence)] {
        let mut machine = Machine::new(&profile()).unwrap(); machine.apply(&first).unwrap();
        let before = machine.snapshot(1); let required = machine.generated_text_only;
        for next in [Event::GeneratedStreamBootstrap(stream()), Event::StreamBootstrap(stream())] {
            assert!(matches!(machine.apply(&next), Err(Error::WrongState)));
            assert_eq!(machine.snapshot(1), before); assert_eq!(machine.generated_text_only, required);
        }
    }
    let root = Directory::new(); let c = fixtures::stopped_config(65);
    let mut p = profile(); p.delivery.limits.events = 2;
    assert!(matches!(FileOversight::create_generated_text_stream(root.store(), p,
        stream(), c.clone(), tokenizer(false)), Err(JournalError::Contract(Error::Limit))));
    assert!(!root.store().exists());
    let mut p = profile(); p.delivery.limits.events = 3;
    let (h, _) = FileOversight::create_generated_text_stream(root.store(), p,
        stream(), c, tokenizer(false)).unwrap();
    assert_eq!(h.revision(), 3); assert!(h.generated_text_stream_required().unwrap());
    assert_eq!(h.decoder_inspection().unwrap().numerical.position, 0);
}

#[test]
fn generated_message_required_cancellation_and_unfinished_output_cannot_be_replaced_by_raw_text() {
    let root = Directory::new(); let (mut host, _) = required(&root, &fixtures::stopped_config(65));
    let cmd = command(&host, 7, request(b"ab", 2)); host.begin_decoder_text(host.revision(), cmd).unwrap();
    let partial = host.advance_decoder_text_batch(host.revision(), 7, 0, 3).unwrap();
    assert_eq!(partial.bytes().unwrap(), b"A"); assert!(!partial.is_complete());
    let source = submission(&host, 91, 7); unchanged_failure(&mut host, source);
    host.cancel_decoder_text(host.revision(), 7, 3).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    let source = submission(&host, 91, 7); unchanged_failure(&mut host, source);
    let raw = host.stream_message_spec("A", ElapsedTick(100)).unwrap();
    let before = bytes(&host);
    assert!(matches!(host.submit_request(host.revision(), 91, raw, snapshot()),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(bytes(&host), before); assert!(host.storage_failure().is_none());
    generate(&mut host, 8, request(b"ab", 2));
    assert!(matches!(submit(&mut host, 92, 8).disposition, FileRequestDisposition::Admitted { .. }));
    assert_eq!(host.decoder_text_progress(7).unwrap().finish(), Some(Ok(GenerationFinish::Cancelled)));
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn generated_message_required_bootstrap_has_independent_tag_and_keeps_legacy_bytes() {
    let root = Directory::new(); let (host, _) = required(&root, &fixtures::stopped_config(65));
    let encoded = journal::encode(&profile(), host.store.identity(), &host.events).unwrap();
    let decoded = journal::decode(&profile(), host.store.identity(), &encoded).unwrap();
    assert!(matches!(decoded.first(), Some(Event::GeneratedStreamBootstrap(p)) if *p == stream()));
    let mut r = Reader::new(&encoded);
    assert_eq!(r.take(8).unwrap(), b"FAOVR\0\0\x01");
    r.bootstrap(&profile().delivery, host.store.identity()).unwrap();
    r.blob(2 * 1024 * 1024).unwrap(); assert_eq!(r.count(4096).unwrap(), host.events.len());
    let record = r.blob(encoded.len()).unwrap();
    let mut expected = vec![32];
    expected.extend_from_slice(&9_u64.to_be_bytes()); expected.extend_from_slice(&1_u64.to_be_bytes());
    for n in [4_u32, 64, 256] { expected.extend_from_slice(&n.to_be_bytes()); }
    assert_eq!(record, expected);
    let mut legacy = host.events.clone(); legacy[0] = Event::StreamBootstrap(stream());
    let legacy = journal::encode(&profile(), host.store.identity(), &legacy).unwrap();
    // Only the first-event discriminator differs; shared profile/decoder bytes
    // are not regenerated or versioned to conceal a different numerical profile.
    let changed: Vec<_> = encoded.iter().zip(&legacy).filter(|(a, b)| a != b).collect();
    assert_eq!(changed.len(), 1); assert_eq!((*changed[0].0, *changed[0].1), (32, 24));
    assert_eq!(encoded.len(), legacy.len());
    assert!(matches!(Machine::replay(&profile(), &decoded), Ok(m) if m.generated_text_only));
}

#[test]
fn generated_message_required_raw_refusal_keeps_forecast_and_native_admission_consumes_it_once() {
    use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame,
        consistency::{BinaryForecast, ErrorBudget, ForecastRegistration}};
    use crate::action::consequence::delivery::persistent::observed::consistency::{FileConsistencyConfig, FileConsistencyParameters};
    let root = Directory::new(); let (mut host, _) = required(&root, &fixtures::stopped_config(65));
    let capture = CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 };
    let pair = BinaryForecast::new(16384, 49152).unwrap();
    let config = FileConsistencyConfig::new(FileConsistencyParameters { probe_id: 1, probe_generation: 1,
        profile: capture, weights: vec![1.0], bias: 0.0, threshold: 0.0,
        forecast: ForecastRegistration { domain: 19, generation: 1, policy_generation: 1,
            event_prefix: b"risk".to_vec(), negative: pair, at_threshold: pair, positive: pair },
        alpha: ErrorBudget::new(1, 4).unwrap(), stream: 17, max_predictions: 16, max_prediction_age_ticks: 10,
    }).unwrap();
    let observer = host.enable_action_consistency(host.revision(), config).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 257,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap().unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let frame = SourceFrame::capture(FrameIdentity { profile: capture, stream: 17, sequence: 1,
        position: n.position - 1 }, &[-1.0]).unwrap();
    let revision = host.revision();
    observer.forecast_action(&mut host, revision, 1, n.actor_revision, &frame).unwrap().unwrap();
    generate(&mut host, 7, request(b"ab", 2));
    let forecast = host.action_consistency_snapshot().unwrap();
    assert_eq!(forecast.pending_attempt, Some(1)); assert_eq!(forecast.evidence.samples(), 0);
    let spec = host.stream_message_spec("A", ElapsedTick(100)).unwrap();
    let before = bytes(&host);
    assert!(matches!(host.submit_request(host.revision(), 91, spec, snapshot()),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(host.action_consistency_snapshot().unwrap(), forecast);
    assert!(host.storage_failure().is_none()); assert_eq!(bytes(&host), before);
    let status = submit(&mut host, 91, 7); assert_eq!(attempt(status), 1);
    let after = host.action_consistency_snapshot().unwrap();
    assert_eq!(after.pending_attempt, None); assert_eq!(after.evidence.samples(), 1);
    assert!(!after.coverage_lost);
    let before = bytes(&host); submit(&mut host, 91, 7);
    assert_eq!(bytes(&host), before); assert_eq!(host.action_consistency_snapshot().unwrap(), after);
}
