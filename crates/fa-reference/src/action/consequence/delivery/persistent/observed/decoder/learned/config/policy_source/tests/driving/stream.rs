//! Both generated-stream effects cross the original leased source and two keys.
use super::*;
use crate::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits;

fn configured_stream() -> FileLearnedConfig {
    let (model, tokenizer, source) = recipe();
    FileLearnedConfig::new_text_stream(model, tokenizer, source, LearnedDecoderBindingLimits::default(),
        StreamProfile::new(61, 1, 8, 32, 128).unwrap()).unwrap().with_required_sidecar().unwrap()
        .with_required_policy_source(policy()).unwrap()
}
fn profile_stream() -> FileOversightProfile {
    let mut profile = profile(); profile.delivery.initial_payload.clear(); profile.delivery.total = 1000; profile
}
fn document_stream(request: u64, release: LearnedTextRelease) -> Vec<u8> {
    encode_command(&Command::Submit { request,
        proposal: FileLearnedTextStreamActorPort::encode_release(request, release, ElapsedTick(100)).unwrap() }).unwrap()
}

#[test]
fn durable_generated_append_and_confirmed_finish_each_require_native_review_and_fresh_human_key() {
    let root = Directory::new(); let config = configured_stream();
    let (mut host, reviewer) = FileOversight::create_with_learned_text_stream(root.store(), profile_stream(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); for _ in 0..3 { step(&mut host); }
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let (port, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut wire = ActorWire::new(port); let mut source = reader(&root);
    publish(&root.store().with_extension("policy"), &capture(1));
    let report = supervisor.exchange_learned_text_stream_actor_from_policy_file(&mut wire,
        &document_stream(71, LearnedTextRelease::Message), &mut source, || ElapsedTick(1)).unwrap();
    assert_eq!(report.response.result, Ok(Knowledge::Pending { request: 71 }));
    assert_eq!(report.intake.unwrap().source_updates, vec![Ok(capture(1).identity())]);
    let message_cost = supervisor.host().unwrap().request_action(71).unwrap().spec().units;
    let mut run = start(FileSupervisedDriver::new(supervisor), 71, 101, b"allow");
    complete(&mut run, &mut source, 1);
    let human = approve(&mut run, &reviewer, &mut source, 1001, 1);
    assert!(matches!(tick(&mut run, &mut source, Some(&human), 1), FileDriverEvent::Dispatched { .. }));
    for published in [false, true] {
        if published { assert!(matches!(tick(&mut run, &mut source, None, 1), FileDriverEvent::PublicationChecked { .. })); }
        let reads = source.status().read_attempts;
        let report = run.supervisor_mut().exchange_learned_text_stream_actor_from_policy_file(&mut wire,
            &document_stream(72, LearnedTextRelease::Finish), &mut source, || panic!("unconfirmed append cannot acquire policy")).unwrap();
        assert_eq!(report.response.result, Err(WireError::Unavailable)); assert!(report.intake.is_none());
        assert_eq!(source.status().read_attempts, reads);
    }
    assert!(matches!(tick(&mut run, &mut source, None, 1), FileDriverEvent::Reconciled { .. }));
    let handoff = run.into_handoff().unwrap(); let mut driver = handoff.driver;
    let report = driver.supervisor_mut().exchange_learned_text_stream_actor_from_policy_file(&mut wire,
        &document_stream(72, LearnedTextRelease::Finish), &mut source, || ElapsedTick(1)).unwrap();
    assert_eq!(report.response.result, Ok(Knowledge::Pending { request: 72 }));
    assert_eq!(report.intake.unwrap().source_updates, vec![Ok(capture(1).identity())]);
    let finish_cost = {
        let host = driver.supervisor().host().unwrap(); let spec = host.request_action(72).unwrap().spec();
        let frame = ReleaseFrame::decode(&spec.payload).unwrap();
        assert!(frame.is_finish()); assert_eq!(frame.prior_messages(), &["aa"]);
        assert_eq!(spec.units, spec.payload.len() as u64); spec.units
    };
    let mut run = start(driver, 72, 201, b"allow"); complete(&mut run, &mut source, 1);
    assert!(matches!(tick(&mut run, &mut source, None, 1), FileDriverEvent::AwaitingHuman { request: 72 }));
    assert!(run.step_from_policy_file(&mut source, || ElapsedTick(1), Some(&human)).result.is_err());
    assert_eq!(run.supervisor().host().unwrap().inspect().executions, 1);
    let finish_human = approve(&mut run, &reviewer, &mut source, 1002, 1);
    assert!(matches!(tick(&mut run, &mut source, Some(&finish_human), 1), FileDriverEvent::Dispatched { .. }));
    assert!(matches!(tick(&mut run, &mut source, None, 1), FileDriverEvent::PublicationChecked { .. }));
    assert!(matches!(tick(&mut run, &mut source, None, 1), FileDriverEvent::Reconciled { .. }));
    let host = run.supervisor().host().unwrap(); let view = host.stream_snapshot().unwrap();
    assert_eq!(view.confirmed.visible(), b"aa"); assert_eq!(view.published.visible(), b"aa");
    assert!(view.confirmed.finished() && view.published.finished());
    assert_eq!(view.publication.executions, 2); assert_eq!(view.publication.control.ledger.charged, message_cost + finish_cost);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert!(host.policy_only_file_source_required()); drop(host);
    let reads = source.status().read_attempts;
    for (request, release) in [(71, LearnedTextRelease::Message), (72, LearnedTextRelease::Finish)] {
        let report = run.supervisor_mut().exchange_learned_text_stream_actor_from_policy_file(&mut wire,
            &document_stream(request, release), &mut source, || panic!("historical retry is not a lease renewal")).unwrap();
        assert!(report.intake.is_none());
        assert!(matches!(report.response.result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. })));
    }
    assert_eq!(source.status().read_attempts, reads);
    assert_eq!(handoff.review.status(), NativeReviewStatus::Finished);
}
