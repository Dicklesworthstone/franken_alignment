//! Actual numerical computation and original two-key publication. Synthetic model.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    MAX_SAMPLING_ENTRIES, tokenizer::ByteBpe,
};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use crate::action::consequence::delivery::persistent::{Reconciliation, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversightProfile,
    decoder::FileDecoderConfig, storage};
use crate::action::consequence::delivery::persistent::observed::stream::generated::FileTextMessageRequest;
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{ReviewWindow, evidence_source::{EvidenceIdentity, EvidenceSnapshot}};
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::BTreeMap;

#[allow(dead_code)]
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));
    pub(super) fn stopped() -> FileDecoderConfig { configured(3.0, 65, Some(256)) }
}
use fixtures::{Directory, bytes, command, request, tokenizer};
fn profile() -> FileOversightProfile {
    let mut p = fixtures::host_profile();
    p.delivery.initial_payload.clear(); p.delivery.total = 4096;
    p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap(); p
}
fn stream() -> StreamProfile { StreamProfile::new(9, 1, 4, 64, 256).unwrap() }
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn owner(root: &Directory, selected: StreamProfile) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_generated_text_stream_with_reserve(root.store(), profile(),
        selected, fixtures::stopped(), tokenizer(false), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); (host, reviewer)
}
fn generate(host: &mut FileOversight, id: u64) {
    let input = command(host, id, request(b"ab", 2));
    host.generate_decoder_text(host.revision(), input).unwrap();
}
fn publish(host: &mut FileOversight, reviewer: &FileHumanReviewer, request: u64, generation: u64,
    reconcile: bool) -> u64
{
    let source = FileTextMessageRequest { request, generation,
        generation_revision: host.decoder_generation_progress(generation).unwrap().generation_revision(),
        target: host.inspect().target, policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100) };
    let status = host.submit_decoder_text_message(host.revision(), source, snapshot()).unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else { panic!("admission"); };
    let action = host.request_action(request).unwrap().clone();
    let capture = EvidenceSnapshot::new(EvidenceIdentity { source: 7, generation: 1, scope: profile().delivery.scope },
        snapshot(), BTreeMap::from([("reviewer".to_owned(), b"context".to_vec())])).unwrap();
    let inputs = capture.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone()).unwrap();
    let round = attempt + 100;
    host.begin_review(host.revision(), attempt, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
    host.commit_review(host.revision(), round, "reviewer", commitment(round, "reviewer", &[9; 32],
        Verdict::Allow, b"salt").unwrap()).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
    let offer = host.request_human_approval(host.revision(), round + 1000, attempt, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &offer).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    host.publish_checked(host.revision(), attempt, Some(&inputs), snapshot(), ElapsedTick(1)).unwrap();
    if reconcile { assert!(matches!(host.reconcile(host.revision(), attempt).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { .. }))); }
    attempt
}
fn first(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = owner(root, stream()); generate(&mut host, 7);
    publish(&mut host, &reviewer, 91, 7, true); (host, reviewer)
}

#[test]
fn continuation_retains_both_numerical_context_and_cumulative_reviewed_messages() {
    let root = Directory::new(); let control_root = Directory::new();
    let (mut host, reviewer) = first(&root); let (mut control, _) = first(&control_root);
    let prepared = host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2)).unwrap();
    assert_eq!(prepared.position(), 4);
    assert_eq!(prepared.actor_revision(), host.decoder_inspection().unwrap().numerical.actor_revision);
    let numerical = host.decoder_inspection().unwrap().numerical;
    host.begin_decoder_text_continuation(host.revision(), 91, prepared.clone()).unwrap();
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.pending_decoder_text().unwrap().unwrap(), prepared);
    for revision in 0..4 { host.advance_decoder_text(host.revision(), 8, revision).unwrap(); }
    generate(&mut control, 8);
    assert_eq!(host.decoder_inspection().unwrap().numerical, control.decoder_inspection().unwrap().numerical);
    assert_eq!(host.decoder_text_progress(8).unwrap().bytes().unwrap(), b"A");
    assert_eq!(host.inspect().executions, 1);
    publish(&mut host, &reviewer, 92, 8, true);
    let state = host.stream_snapshot().unwrap();
    assert_eq!(state.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert_eq!(state.confirmed, state.published);
    let action = host.request_action(92).unwrap();
    let frame = ReleaseFrame::decode(&action.spec().payload).unwrap();
    assert_eq!(frame.prior_messages(), &["A"]); assert_eq!(frame.message(), Some("A"));
    let charged = host.request_action(91).unwrap().spec().units + action.spec().units;
    assert_eq!(host.inspect().control.ledger.charged, charged);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
    assert!(host.prepare_decoder_text_continuation(91, 9, request(b"ab", 2)).is_err());
    assert!(host.prepare_decoder_text_continuation(92, 9, request(b"ab", 2)).is_ok());
}

#[test]
fn continuation_requires_reconciled_publication_not_completed_or_cancelled_output() {
    let root = Directory::new(); let (mut host, reviewer) = owner(&root, stream());
    generate(&mut host, 7);
    let before = bytes(&host);
    assert!(host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2)).is_err());
    assert_eq!(bytes(&host), before);
    let attempt = publish(&mut host, &reviewer, 91, 7, false);
    let before = bytes(&host);
    assert!(host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2)).is_err());
    assert_eq!(bytes(&host), before);
    host.reconcile(host.revision(), attempt).unwrap();
    assert!(host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2)).is_ok());
    let source = FileTextMessageRequest { request: 92, generation: 7,
        generation_revision: host.decoder_generation_progress(7).unwrap().generation_revision(),
        target: host.inspect().target, policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100) };
    host.submit_decoder_text_message(host.revision(), source, snapshot()).unwrap();
    host.cancel_request(host.revision(), 92).unwrap();
    assert!(host.prepare_decoder_text_continuation(92, 8, request(b"ab", 2)).is_err());
}

#[test]
fn continuation_preparation_survives_pause_but_begin_needs_original_resume_and_is_not_recreation() {
    let root = Directory::new(); let (host, _) = first(&root); drop(host);
    let (mut host, _) = FileOversight::open_generated_text_stream_with_reserve(root.store(), profile(), stream(),
        &fixtures::stopped(), &tokenizer(false), RecoveryReserve::terminal()).unwrap();
    let prepared = host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2)).unwrap();
    let before = bytes(&host);
    assert!(host.begin_decoder_text_continuation(host.revision(), 91, prepared.clone()).is_err());
    assert_eq!(bytes(&host), before); assert!(host.storage_failure().is_none());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    host.begin_decoder_text_continuation(host.revision(), 91, prepared.clone()).unwrap();
    host.advance_decoder_text(host.revision(), 8, 0).unwrap();
    let before = bytes(&host);
    assert!(host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2)).is_err());
    assert!(host.prepare_decoder_text_continuation(91, 9, request(b"ab", 2)).is_err());
    host.begin_decoder_text(0, prepared).unwrap(); // existing exact retry, never another token
    assert_eq!(bytes(&host), before);
    assert_eq!(host.decoder_text_progress(8).unwrap().generation_revision(), 1);
}

#[test]
fn continuation_rechecks_prepared_cut_and_full_remaining_horizon_without_mutation() {
    let root = Directory::new(); let (mut host, _) = first(&root);
    let mut exact = request(b"ab", 11); // two prompt positions + eleven samples > remaining 12
    let before = bytes(&host);
    assert!(matches!(host.prepare_decoder_text_continuation(91, 8, exact.clone()),
        Err(JournalError::Contract(Error::Limit))));
    exact.max_new_tokens = 10;
    assert!(host.prepare_decoder_text_continuation(91, 8, exact).is_ok());
    let mut content_stop = request(b"ab", 2); content_stop.stop_tokens.push(65);
    assert!(host.prepare_decoder_text_continuation(91, 8, content_stop).is_err());
    assert_eq!(bytes(&host), before);
    let prepared = host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 257,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap().unwrap();
    let before = bytes(&host);
    assert!(host.begin_decoder_text_continuation(host.revision(), 91, prepared).is_err());
    assert_eq!(bytes(&host), before); assert!(host.storage_failure().is_none());
}

#[test]
fn continuation_cannot_exceed_the_original_stream_message_or_byte_limits() {
    for selected in [StreamProfile::new(9, 1, 1, 64, 256).unwrap(),
        StreamProfile::new(9, 1, 4, 1, 1).unwrap()] {
        let root = Directory::new(); let (mut host, reviewer) = owner(&root, selected);
        generate(&mut host, 7); publish(&mut host, &reviewer, 91, 7, true);
        let before = bytes(&host);
        let mut next = request(b"ab", 2); next.max_output_bytes = 1;
        assert!(matches!(host.prepare_decoder_text_continuation(91, 8, next),
            Err(JournalError::Contract(Error::Limit))));
        assert_eq!(bytes(&host), before);
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), before);
    }
}
