//! Original owner integration shared by the two operator recipe tests.
//! These tests do not exercise socket identity, transport, or process isolation.
use crate::config::Config;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, text::TextGenerationRequest,
};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversight, decoder::text::FileTextGenerationCommand,
    stream::generated::FileTextMessageRequest,
};
use fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use fa_reference::action::consequence::delivery::stream::ReleaseFrame;
use fa_reference::action::consequence::oversight::ReviewWindow;
use fa_reference::round::{Verdict, commitment};
use fa_reference::{Error, Snapshot};
use std::fs;

pub(in crate::workflow::actor_service) fn begin(
    host: &mut FileOversight, config: &mut Config, generation: u64, text: TextGenerationRequest,
) {
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(generation,
        numerical.actor_revision, numerical.position, text).unwrap();
    host.begin_decoder_text(host.revision(), command).unwrap();
    for revision in 0..3 {
        host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
        host.advance_decoder_text(host.revision(), generation, revision).unwrap();
    }
    let progress = host.decoder_text_progress(generation).unwrap();
    assert!(!progress.is_complete());
    assert_eq!(progress.generation_revision(), 3);
    assert_eq!(progress.bytes().unwrap(), b"A");
    assert_eq!(progress.numerical().partial().unwrap().work().attempted_samples, 1);
    assert_eq!(host.inspect().executions, 0);
    assert!(host.inspect().payload.is_empty());
}

pub(in crate::workflow::actor_service) fn complete(
    host: &mut FileOversight, config: &mut Config,
    reviewers: (&FileHumanReviewer, &FileHumanReviewer),
    ids: (u64, u64), approve: bool,
) -> FileTextMessageRequest {
    let (generation, request_id) = ids;
    let recovered = host.decoder_inspection().unwrap();
    assert!(recovered.paused);
    assert_eq!(recovered.numerical.position, 3);
    let disk = fs::read(config.store.join("delivery.bin")).unwrap();
    assert!(host.advance_decoder_text(host.revision(), generation, 3).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
    let captured = host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    host.resume_decoder(host.revision(), recovered.numerical.actor_revision,
        recovered.numerical.position).unwrap();
    let finished = host.advance_decoder_text(host.revision(), generation, 3).unwrap();
    assert_eq!(finished.finish(), Some(Ok(GenerationFinish::StopToken)));
    assert_eq!(finished.generation_revision(), 4);
    assert_eq!(finished.bytes().unwrap(), b"A");
    let report = finished.numerical().receipt().unwrap().result().unwrap();
    assert_eq!(report.work().attempted_samples, 2);
    assert_eq!(report.reviewed_prompt_tokens(), 2);
    assert_eq!(report.requested_prompt_tokens(), 2);

    let request = FileTextMessageRequest { request: request_id, generation,
        generation_revision: 4, target: host.inspect().target,
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(20_000) };
    let snapshot = captured.snapshot().clone();
    let status = host.submit_decoder_text_message(host.revision(), request.clone(), snapshot.clone()).unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
        panic!("original generated message must be admitted: {status:?}");
    };
    let action = host.request_action(request_id).unwrap().clone();
    assert_eq!(ReleaseFrame::decode(&action.spec().payload).unwrap().message(), Some("A"));
    let inputs = captured.inputs_for(&action, &config.profile.committee).unwrap();
    host.record_inputs(host.revision(), attempt, host.input_revision(attempt).unwrap(), inputs.clone()).unwrap();
    let round = 101;
    let root = captured.reference_root();
    host.begin_review(host.revision(), attempt, round, root, ReviewWindow {
        commit_by: ElapsedTick(1005), reveal_by: ElapsedTick(1010),
    }, snapshot.clone()).unwrap();
    for member in ["alpha", "beta"] {
        let salt = format!("original-{member}");
        let digest = commitment(round, member, &root, Verdict::Allow, salt.as_bytes()).unwrap();
        host.commit_review(host.revision(), round, member, digest).unwrap();
    }
    host.open_reveals(host.revision(), round).unwrap();
    for member in ["alpha", "beta"] {
        host.reveal_review(host.revision(), round, member, Verdict::Allow,
            format!("original-{member}").into_bytes()).unwrap();
    }
    host.finish_review(host.revision(), round, Some(&inputs), snapshot.clone()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot.clone()).unwrap();
    let disk = fs::read(config.store.join("delivery.bin")).unwrap();
    assert!(host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot.clone(), ElapsedTick(1000)).is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
    assert_eq!(host.inspect().executions, 0);
    assert!(host.inspect().payload.is_empty());
    let offer = host.request_human_approval(host.revision(), 1001, attempt, &inputs, ElapsedTick(1030)).unwrap();
    let revision = host.revision();
    assert!(matches!(reviewers.1.approve(host, revision, &offer),
        Err(fa_reference::action::consequence::delivery::persistent::JournalError::Contract(Error::Binding))));
    assert_eq!(host.revision(), revision); // pre-recovery reviewer is never a new key
    if approve {
        let revision = host.revision();
        let human = reviewers.0.approve(host, revision, &offer).unwrap();
        host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot.clone()).unwrap();
        assert_eq!(host.inspect().executions, 0);
        let publication = host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot, ElapsedTick(1000)).unwrap();
        assert!(matches!(publication.outcome, EndpointOutcome::Executed { .. }));
        host.reconcile(host.revision(), attempt).unwrap();
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.inspect().control.ledger.charged, action.spec().payload.len() as u64);
        // The source action carries the complete ReleaseFrame. The stream
        // endpoint materializes its message, while charging the whole frame.
        assert_eq!(host.inspect().payload, b"A");
    } else {
        let revision = host.revision();
        reviewers.0.reject(host, revision, &offer).unwrap();
        host.cancel_request(host.revision(), request_id).unwrap();
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.inspect().control.ledger.charged, 0);
        assert!(host.inspect().payload.is_empty());
    }
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    request
}

pub(in crate::workflow::actor_service) fn receipt(
    host: &mut FileOversight, config: &Config, request: &FileTextMessageRequest, approve: bool,
) {
    let revision = host.revision();
    let before = host.decoder_inspection().unwrap();
    let disk = fs::read(config.store.join("delivery.bin")).unwrap();
    let status = host.request_status(request.request).unwrap();
    assert_eq!(host.submit_decoder_text_message(0, request.clone(), Snapshot::default()).unwrap(), status);
    assert_eq!(host.revision(), revision);
    assert_eq!(host.decoder_inspection().unwrap(), before);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), disk);
    assert!(before.paused);
    assert_eq!(before.numerical.position, 4);
    assert_eq!(host.decoder_text_progress(request.generation).unwrap().bytes().unwrap(), b"A");
    assert_eq!(host.inspect().executions, u64::from(approve));
    if approve {
        let action = host.request_action(request.request).unwrap();
        assert_eq!(ReleaseFrame::decode(&action.spec().payload).unwrap().message(), Some("A"));
        assert_eq!(host.inspect().payload, b"A");
        assert_eq!(host.inspect().control.ledger.charged, action.spec().payload.len() as u64);
    }
    else { assert!(host.inspect().payload.is_empty()); }
}
