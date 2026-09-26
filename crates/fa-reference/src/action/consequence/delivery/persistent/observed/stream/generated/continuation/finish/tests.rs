//! Original numerical computation and two-key canonical-file publication.
//! Synthetic weights/ballots are fixtures, not detector or isolation evidence.
use super::*;
use crate::action::FrozenAction;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    MAX_SAMPLING_ENTRIES, text::TextGenerationRequest, tokenizer::ByteBpe,
};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use crate::action::consequence::delivery::persistent::{FilePermit, JournalIo, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileHumanReviewer,
    FileOversightProfile, decoder::{FileDecoderConfig, text::FileTextGenerationCommand}};
use crate::action::consequence::delivery::persistent::observed::stream::generated::FileTextMessageRequest;
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeInput, ReviewWindow,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot}};
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::BTreeMap;

#[allow(dead_code)]
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));
    pub(super) fn decoder() -> FileDecoderConfig { configured(3.0, 65, Some(256)) }
}
use fixtures::{Directory, bytes, command, request, tokenizer};
fn profile(cap: usize) -> FileOversightProfile {
    let mut p = fixtures::host_profile();
    p.delivery.initial_payload.clear(); p.delivery.total = 4096;
    p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(cap)]).unwrap();
    p
}
fn stream(full: bool) -> StreamProfile {
    if full { StreamProfile::new(9, 1, 1, 1, 1).unwrap() }
    else { StreamProfile::new(9, 1, 4, 64, 256).unwrap() }
}
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn start(root: &Directory, cap: usize, full: bool) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_generated_text_stream(root.store(),
        profile(cap), stream(full), fixtures::decoder(), tokenizer(false)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
fn generate(host: &mut FileOversight, generation: u64, id: u64) {
    let input = command(host, generation, request(b"ab", 2));
    host.generate_decoder_text(host.revision(), input).unwrap();
    let source = FileTextMessageRequest { request: id, generation,
        generation_revision: host.decoder_generation_progress(generation).unwrap().generation_revision(),
        target: host.inspect().target, policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100) };
    host.submit_decoder_text_message(host.revision(), source, snapshot()).unwrap();
}
fn id(host: &FileOversight, request: u64) -> u64 {
    match host.request_status(request).unwrap().disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        _ => panic!("original admitted request"),
    }
}
struct Keys { action: FrozenAction, inputs: CommitteeInput, automatic: FilePermit, human: FileHumanPermit }
fn review(host: &mut FileOversight, reviewer: &FileHumanReviewer, request: u64) -> Keys {
    let id = id(host, request);
    let action = host.request_action(request).unwrap().clone();
    let captured = EvidenceSnapshot::new(EvidenceIdentity { scope: profile(4096).delivery.scope,
        source: 7, generation: 1 }, snapshot(), BTreeMap::from([("reviewer".into(), b"context".to_vec())])).unwrap();
    let inputs = captured.inputs_for(&action, &profile(4096).committee).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    let round = id + 100;
    host.begin_review(host.revision(), id, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human }
}
fn publish(host: &mut FileOversight, reviewer: &FileHumanReviewer, request: u64, reconcile: bool) {
    let keys = review(host, reviewer, request); let attempt = id(host, request);
    host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
    assert!(matches!(host.publish_checked(host.revision(), attempt, Some(&keys.inputs), snapshot(),
        ElapsedTick(1)).unwrap().outcome, EndpointOutcome::Executed { .. }));
    if reconcile { host.reconcile(host.revision(), attempt).unwrap(); }
}
fn submit_finish(host: &mut FileOversight, after: u64, request: u64) -> FileStreamProposal {
    let intent = host.prepare_decoder_text_finish(after, request, ElapsedTick(100)).unwrap();
    let spec = host.stream_finish_spec(intent.deadline).unwrap();
    assert_eq!(spec.target, Some(intent.target)); assert_eq!(spec.policy_epoch, intent.expected_policy_epoch);
    host.submit_request(host.revision(), request, spec, snapshot()).unwrap();
    intent
}

#[test]
fn native_finish_at_full_capacity_adds_no_tokens_but_still_requires_review_and_charges_full_context() {
    let root = Directory::new(); let (mut host, reviewer) = start(&root, 4096, true);
    generate(&mut host, 7, 91); publish(&mut host, &reviewer, 91, true);
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert!(host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2)).is_err());
    let before = bytes(&host);
    let intent = host.prepare_decoder_text_finish(91, 92, ElapsedTick(100)).unwrap();
    assert_eq!(bytes(&host), before); assert!(intent.message.is_none());
    assert_eq!(submit_finish(&mut host, 91, 92), intent);
    let spec = host.request_action(92).unwrap().spec();
    let frame = ReleaseFrame::decode(&spec.payload).unwrap();
    assert!(frame.is_finish()); assert_eq!(frame.prior_messages(), &["A"]);
    let total = host.inspect().control.ledger.charged + spec.units;
    assert!(!host.stream_snapshot().unwrap().published.finished());
    assert!(host.publish_checked(host.revision(), id(&host, 92), None, snapshot(), ElapsedTick(1)).is_err());
    publish(&mut host, &reviewer, 92, true);
    let state = host.stream_snapshot().unwrap();
    assert!(state.confirmed.finished()); assert_eq!(state.confirmed, state.published);
    assert_eq!(state.confirmed.visible(), b"A"); assert_eq!(state.confirmed.message_count(), 1);
    assert_eq!(state.publication.executions, 2); assert_eq!(state.publication.control.ledger.charged, total);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.decoder_text_finish_request(91, 92).unwrap(), intent);
    assert!(host.prepare_decoder_text_finish(91, 93, ElapsedTick(100)).is_err());
}

#[test]
fn native_finish_requires_confirmed_receipt_and_cannot_skip_a_new_pending_intent() {
    let root = Directory::new(); let (mut host, reviewer) = start(&root, 4096, false);
    generate(&mut host, 7, 91);
    assert!(host.prepare_decoder_text_finish(91, 92, ElapsedTick(100)).is_err());
    publish(&mut host, &reviewer, 91, false);
    let before = bytes(&host);
    assert!(host.prepare_decoder_text_finish(91, 92, ElapsedTick(100)).is_err());
    assert_eq!(bytes(&host), before);
    assert!(matches!(host.reconcile(host.revision(), id(&host, 91)).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
    assert!(host.prepare_decoder_text_finish(91, 92, ElapsedTick(100)).is_ok());
    let pending = command(&host, 8, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), pending.clone()).unwrap();
    let before = bytes(&host);
    assert!(host.prepare_decoder_text_finish(91, 92, ElapsedTick(100)).is_err());
    assert_eq!(host.pending_decoder_text().unwrap(), Some(pending)); assert_eq!(bytes(&host), before);
}

#[test]
fn native_finish_retains_refused_and_cancelled_intents_without_rebasing_their_deadlines() {
    for denied in [false, true] {
        let root = Directory::new(); let cap = if denied { 50 } else { 4096 };
        let (mut host, reviewer) = start(&root, cap, false);
        generate(&mut host, 7, 91); publish(&mut host, &reviewer, 91, true);
        let original = submit_finish(&mut host, 91, 92);
        assert_eq!(matches!(host.request_status(92).unwrap().disposition,
            FileRequestDisposition::NotAdmitted(_)), denied);
        if !denied { host.cancel_request(host.revision(), 92).unwrap(); }
        let spec = host.machine.requests.original_spec(92).unwrap().clone();
        let status = host.request_status(92).unwrap(); drop(host);
        let (mut host, _) = FileOversight::open_generated_text_stream(root.store(), profile(cap),
            stream(false), &fixtures::decoder(), &tokenizer(false)).unwrap();
        host.observe_time(host.revision(), ElapsedTick(200)).unwrap();
        let before = bytes(&host); let numerical = host.decoder_inspection().unwrap();
        assert_eq!(host.decoder_text_finish_request(91, 92).unwrap(), original);
        assert_eq!(host.submit_request(0, 92, spec, Snapshot::default()).unwrap(), status);
        assert_eq!(host.decoder_inspection().unwrap(), numerical); assert!(numerical.paused);
        assert_eq!(bytes(&host), before); assert!(!host.stream_snapshot().unwrap().published.finished());
        assert_eq!(host.inspect().executions, 1);
    }
}

#[test]
fn native_finish_source_is_the_exact_latest_message_not_equal_text_from_another_request() {
    let root = Directory::new(); let (mut host, reviewer) = start(&root, 4096, false);
    generate(&mut host, 7, 91); publish(&mut host, &reviewer, 91, true);
    generate(&mut host, 8, 92); publish(&mut host, &reviewer, 92, true);
    assert!(host.prepare_decoder_text_finish(91, 93, ElapsedTick(100)).is_err());
    let original = submit_finish(&mut host, 92, 93);
    assert_eq!(host.decoder_text_finish_request(92, 93).unwrap(), original);
    assert!(host.decoder_text_finish_request(91, 93).is_err());
    assert!(host.decoder_text_finish_request(91, 92).is_err()); // a message is not a finish
    assert!(host.prepare_decoder_text_finish(92, 93, ElapsedTick(101)).is_err());
}

#[test]
fn native_finish_preparation_after_recovery_is_read_only_and_superseding_inference_invalidates_it() {
    let root = Directory::new(); let (mut host, reviewer) = start(&root, 4096, false);
    generate(&mut host, 7, 91); publish(&mut host, &reviewer, 91, true); drop(host);
    let (mut host, _) = FileOversight::open_generated_text_stream(root.store(), profile(4096),
        stream(false), &fixtures::decoder(), &tokenizer(false)).unwrap();
    let before = bytes(&host); let n = host.decoder_inspection().unwrap().numerical;
    assert!(host.prepare_decoder_text_finish(91, 92, ElapsedTick(100)).is_ok());
    assert!(host.decoder_inspection().unwrap().paused); assert_eq!(bytes(&host), before);
    assert!(host.stream_finish_spec(ElapsedTick(100)).is_err());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    assert!(host.prepare_decoder_text_finish(91, 92, ElapsedTick(100)).is_ok());
    host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 257,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap().unwrap();
    let before = bytes(&host);
    assert!(host.prepare_decoder_text_finish(91, 92, ElapsedTick(100)).is_err());
    assert_eq!(bytes(&host), before);
}

#[test]
fn native_finish_faulted_owner_cannot_report_an_old_intent_as_current() {
    let root = Directory::new(); let (mut host, reviewer) = start(&root, 4096, false);
    generate(&mut host, 7, 91); publish(&mut host, &reviewer, 91, true);
    let original = submit_finish(&mut host, 91, 92);
    assert_eq!(host.decoder_text_finish_request(91, 92).unwrap(), original);
    host.store.fail_once(JournalIo::DirectorySync);
    assert!(host.observe_time(host.revision(), ElapsedTick(2)).is_err());
    assert_eq!(host.decoder_text_finish_request(91, 92), Err(JournalError::Unavailable));
    assert_eq!(host.prepare_decoder_text_finish(91, 93, ElapsedTick(100)), Err(JournalError::Unavailable));
}
