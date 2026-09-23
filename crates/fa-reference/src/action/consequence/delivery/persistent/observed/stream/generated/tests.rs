//! Original numerical generation and original message authority, not supplied output.
use super::*;
use crate::action::FrozenAction;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationRequest, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest, tokenizer::ByteBpe,
};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::persistent::{FilePermit, Reconciliation};
use crate::action::consequence::delivery::persistent::observed::{FileHumanPermit, FileHumanReviewer,
    FileOversightProfile, decoder::{FileDecoderConfig, generation::FileGenerationCommand,
        text::FileTextGenerationCommand}};
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::stream::{ReleaseFrame, StreamProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeInput, ReviewWindow,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot}};
use crate::round::{Verdict, commitment};
use std::collections::BTreeMap;

#[allow(dead_code)]
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));

    // Original synthetic model: the generated content changes the next residual
    // so the next actual sampled ID is the control. No canned report or callback.
    pub(super) fn stopped_config(output: u32) -> FileDecoderConfig {
        configured(3.0, output, Some(256))
    }
}
use fixtures::{Directory, bytes, command, request, tokenizer};

fn profile() -> FileOversightProfile {
    let mut p = fixtures::host_profile();
    p.delivery.initial_payload.clear();
    p.delivery.total = 4096;
    p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
    p
}
fn stream() -> StreamProfile { StreamProfile::new(9, 1, 4, 64, 256).unwrap() }
fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }
fn start(root: &Directory, p: FileOversightProfile, c: &FileDecoderConfig)
    -> (FileOversight, FileHumanReviewer)
{
    let (mut host, reviewer) = FileOversight::create_stream(root.store(), p, stream()).unwrap();
    host.enable_decoder(host.revision(), c.clone()).unwrap();
    host.enable_decoder_tokenizer(host.revision(), tokenizer(false)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
fn generate(host: &mut FileOversight, id: u64, r: TextGenerationRequest) {
    let c = command(host, id, r);
    host.generate_decoder_text(host.revision(), c).unwrap();
}
fn submission(host: &FileOversight, request: u64, generation: u64) -> FileTextMessageRequest {
    FileTextMessageRequest { request, generation,
        generation_revision: host.decoder_generation_progress(generation).unwrap().generation_revision(),
        target: host.inspect().target, policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100) }
}
fn attempt(status: FileRequestStatus) -> u64 {
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else { panic!("admitted original request"); };
    attempt
}
fn submit(host: &mut FileOversight, request: u64, generation: u64) -> FileRequestStatus {
    let input = submission(host, request, generation);
    host.submit_decoder_text_message(host.revision(), input, snapshot()).unwrap()
}
fn unchanged_failure(host: &mut FileOversight, input: FileTextMessageRequest) -> JournalError {
    let before = bytes(host); let numerical = host.decoder_inspection().unwrap();
    let state = host.inspect();
    let error = host.submit_decoder_text_message(host.revision(), input, snapshot()).unwrap_err();
    assert_eq!(bytes(host), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
    assert_eq!(host.inspect(), state); assert!(host.storage_failure().is_none());
    error
}

#[test]
fn generated_message_uses_original_ids_bytes_frame_and_proposal_accounting() {
    let root = Directory::new(); let other_root = Directory::new();
    let c = fixtures::stopped_config(65);
    let (mut host, _) = start(&root, profile(), &c);
    let (mut original, _) = start(&other_root, profile(), &c);
    for h in [&mut host, &mut original] { generate(h, 7, request(b"ab", 2)); }
    let result = host.decoder_text_generation(7).unwrap();
    assert_eq!(result.result().unwrap().bytes().unwrap(), b"A");
    assert_eq!(result.result().unwrap().generation().finish(), GenerationFinish::StopToken);
    let spec = original.stream_message_spec("A", ElapsedTick(100)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let revision = host.revision();
    let input = submission(&host, 91, 7);
    let actual = host.submit_decoder_text_message(revision, input.clone(), snapshot()).unwrap();
    let expected = original.submit_request(original.revision(), 91, spec.clone(), snapshot()).unwrap();
    assert_eq!(actual, expected); assert_eq!(host.revision(), revision + 1);
    assert_eq!(host.request_action(91).unwrap().spec(), &spec);
    assert_eq!(host.decoder_text_message_request(91).unwrap(), &input);
    assert_eq!(host.inspect().control, original.inspect().control);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.machine.broker.hosted_replay_bytes().unwrap(), original.machine.broker.hosted_replay_bytes().unwrap());
    let frame = ReleaseFrame::decode(&spec.payload).unwrap();
    assert_eq!(frame.message(), Some("A")); assert_eq!(spec.units, spec.payload.len() as u64);
    assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn generated_message_requires_natural_stop_full_prompt_and_exact_nonempty_utf8() {
    for mode in 0..8 {
        let root = Directory::new();
        let c = match mode {
            2 => fixtures::config(0.5, 65), // actual monitored prompt hold
            5 => fixtures::stopped_config(0xc3), // incomplete UTF-8, not repaired
            6 => fixtures::config(3.0, 256), // valid reviewed stop, empty output
            _ => fixtures::stopped_config(65),
        };
        let (mut host, _) = start(&root, profile(), &c);
        let mut r = request(b"ab", 2);
        if mode == 1 { r.max_new_tokens = 1; } // content token but no reviewed terminal
        if mode == 3 { r.generation.sampling_entries = 259; }
        if mode == 4 { r.generation.scalar_products = 0; }
        if mode == 7 { r.stop_tokens.push(65); } // content stops cannot hide suffixes
        let cmd = command(&host, 7, r);
        if mode == 0 { host.begin_decoder_text(host.revision(), cmd).unwrap(); }
        else { host.generate_decoder_text(host.revision(), cmd).unwrap(); }
        let input = submission(&host, 91, 7);
        unchanged_failure(&mut host, input);
        assert!(matches!(host.decoder_text_message_request(91), Err(JournalError::Contract(Error::Missing))));
        assert!(matches!(host.request_status(91), Err(JournalError::Contract(Error::Missing))));
        assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
    }
    let root = Directory::new(); let (mut host, _) = start(&root, profile(), &fixtures::stopped_config(258));
    generate(&mut host, 7, request(b"ab", 2));
    assert!(matches!(submit(&mut host, 91, 7).disposition, FileRequestDisposition::Admitted { .. }));
    let spec = host.request_action(91).unwrap().spec();
    assert_eq!(ReleaseFrame::decode(&spec.payload).unwrap().message(), Some("é"));
}

#[test]
fn generated_message_cancelled_prefix_and_bare_numerical_results_have_no_message_source() {
    let root = Directory::new(); let (mut host, _) = start(&root, profile(), &fixtures::stopped_config(65));
    let c = command(&host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), c).unwrap();
    for revision in 0..3 { host.advance_decoder_text(host.revision(), 7, revision).unwrap(); }
    let cancelled = host.cancel_decoder_text(host.revision(), 7, 3).unwrap();
    assert_eq!(cancelled.bytes().unwrap(), b"A");
    assert_eq!(cancelled.finish(), Some(Ok(GenerationFinish::Cancelled)));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    let input = submission(&host, 91, 7); unchanged_failure(&mut host, input);
    // Complete a genuine bare-ID request. A matching numerical result cannot
    // retroactively acquire original prompt bytes or text-tokenizer provenance.
    let n = host.decoder_inspection().unwrap().numerical;
    let c = FileGenerationCommand::new(8, n.actor_revision, n.position, GenerationRequest {
        prompt: vec![257], max_new_tokens: 2, stop_tokens: vec![256], budget: request(b"ab", 2).generation,
    }).unwrap();
    host.generate_decoder(host.revision(), c).unwrap();
    let input = submission(&host, 92, 8);
    assert!(matches!(unchanged_failure(&mut host, input), JournalError::Contract(Error::Missing)));
}

#[test]
fn generated_message_rejects_stale_source_target_epoch_and_generation_revision() {
    let root = Directory::new(); let (mut host, _) = start(&root, profile(), &fixtures::stopped_config(65));
    generate(&mut host, 7, request(b"ab", 2));
    let input = submission(&host, 91, 7);
    for which in 0..5 {
        let mut changed = input.clone();
        match which {
            0 => changed.target.expected_version += 1,
            1 => changed.target.object += 1,
            2 => changed.policy_epoch += 1,
            3 => changed.generation_revision += 1,
            _ => changed.generation = 999,
        }
        unchanged_failure(&mut host, changed);
    }
    let n = host.decoder_inspection().unwrap().numerical;
    host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 257,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap().unwrap();
    assert!(matches!(unchanged_failure(&mut host, input), JournalError::Contract(Error::Stale)));
    generate(&mut host, 8, request(b"ab", 2));
    submit(&mut host, 92, 8); // genuine current source succeeds
}

#[test]
fn generated_message_retries_preserve_recorded_refusals_and_do_not_rebase_or_pause_new_work() {
    for denied in [false, true] {
        let root = Directory::new(); let mut p = profile();
        if denied { p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(1)]).unwrap(); }
        let (mut host, _) = start(&root, p, &fixtures::stopped_config(65));
        generate(&mut host, 7, request(b"ab", 2));
        let input = submission(&host, 91, 7);
        let status = host.submit_decoder_text_message(host.revision(), input.clone(), snapshot()).unwrap();
        assert_eq!(matches!(status.disposition, FileRequestDisposition::NotAdmitted(_)), denied);
        if !denied { host.cancel_request(host.revision(), 91).unwrap(); }
        let current = host.request_status(91).unwrap();
        let c = command(&host, 8, request(b"ab", 2));
        host.begin_decoder_text(host.revision(), c).unwrap();
        let before = bytes(&host); let n = host.decoder_inspection().unwrap();
        host.source_interrupted = true;
        assert_eq!(host.submit_decoder_text_message(0, input.clone(), Snapshot::default()).unwrap(), current);
        assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap(), n);
        assert!(host.source_interrupted); assert_eq!(host.pending_decoder_text().unwrap().unwrap().id(), 8);
        for which in 0..5 {
            let mut changed = input.clone();
            match which { 0 => changed.generation = 8, 1 => changed.generation_revision += 1,
                2 => changed.target.generation += 1, 3 => changed.policy_epoch += 1,
                _ => changed.deadline.0 += 1 }
            assert!(matches!(host.submit_decoder_text_message(0, changed, Snapshot::default()),
                Err(JournalError::Contract(Error::Binding))));
        }
        assert_eq!(bytes(&host), before);
    }
}

#[test]
fn generated_message_cannot_relabel_an_ordinary_request_even_when_payloads_match() {
    let root = Directory::new(); let (mut host, _) = start(&root, profile(), &fixtures::stopped_config(65));
    generate(&mut host, 7, request(b"ab", 2));
    let input = submission(&host, 91, 7);
    let spec = host.stream_message_spec("A", input.deadline).unwrap();
    host.submit_request(host.revision(), 91, spec, snapshot()).unwrap();
    assert!(matches!(unchanged_failure(&mut host, input), JournalError::Contract(Error::Duplicate)));
    assert!(matches!(host.decoder_text_message_request(91), Err(JournalError::Contract(Error::Missing))));
}

struct Keys { action: FrozenAction, inputs: CommitteeInput, automatic: FilePermit, human: FileHumanPermit }
fn review(host: &mut FileOversight, reviewer: &FileHumanReviewer, request: u64) -> Keys {
    let id = attempt(host.request_status(request).unwrap());
    let action = host.request_action(request).unwrap().clone();
    let captured = EvidenceSnapshot::new(EvidenceIdentity { scope: profile().delivery.scope, source: 7, generation: 1 },
        snapshot(), BTreeMap::from([("reviewer".to_owned(), b"context".to_vec())])).unwrap();
    let inputs = captured.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    let round = id + 100;
    host.begin_review(host.revision(), id, round, [9; 32], ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
    }, snapshot()).unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
    assert!(host.publish_checked(host.revision(), id, Some(&inputs), snapshot(), ElapsedTick(1)).is_err());
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human }
}

#[test]
fn generated_message_reaches_publication_only_through_original_congress_and_two_keys() {
    let root = Directory::new(); let (mut host, reviewer) = start(&root, profile(), &fixtures::stopped_config(65));
    generate(&mut host, 7, request(b"ab", 2));
    let id = attempt(submit(&mut host, 91, 7));
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert!(host.publish(host.revision(), id).is_err());
    assert!(host.publish_checked(host.revision(), id, None, snapshot(), ElapsedTick(1)).is_err());
    let keys = review(&mut host, &reviewer, 91);
    assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
    host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
    assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
    let published = host.publish_checked(host.revision(), id, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    assert!(matches!(published.outcome, EndpointOutcome::Executed { .. }));
    let view = host.stream_snapshot().unwrap();
    assert_eq!(view.published.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(view.confirmed.message_count(), 0); assert_eq!(view.pending, Some(id));
    assert!(matches!(host.reconcile(host.revision(), id).unwrap(), Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
    let view = host.stream_snapshot().unwrap(); assert_eq!(view.confirmed, view.published);
    assert_eq!(view.pending, None); assert_eq!(view.publication.executions, 1);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    let source = host.decoder_text_message_request(91).unwrap().clone();
    let before = bytes(&host);
    host.submit_decoder_text_message(0, source, Snapshot::default()).unwrap();
    assert_eq!(bytes(&host), before); assert_eq!(host.inspect().executions, 1);
    assert!(host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).is_err());
}

#[test]
fn generated_message_incremental_completion_and_current_cumulative_context_are_preserved() {
    let root = Directory::new(); let (mut host, reviewer) = start(&root, profile(), &fixtures::stopped_config(65));
    let c = command(&host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), c).unwrap();
    for revision in 0..4 {
        let input = submission(&host, 91, 7); unchanged_failure(&mut host, input);
        host.advance_decoder_text(host.revision(), 7, revision).unwrap();
    }
    assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 4);
    let id = attempt(submit(&mut host, 91, 7));
    let keys = review(&mut host, &reviewer, 91);
    host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
    host.publish_checked(host.revision(), id, Some(&keys.inputs), snapshot(), ElapsedTick(1)).unwrap();
    host.reconcile(host.revision(), id).unwrap();
    generate(&mut host, 8, request(b"ab", 2));
    let expected = host.stream_message_spec("A", ElapsedTick(100)).unwrap();
    submit(&mut host, 92, 8);
    assert_eq!(host.request_action(92).unwrap().spec(), &expected);
    assert!(expected.payload.len() > host.request_action(91).unwrap().spec().payload.len());
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
    assert_eq!(host.inspect().executions, 1); // second proposal did not disclose its bytes
}

mod recovery;
