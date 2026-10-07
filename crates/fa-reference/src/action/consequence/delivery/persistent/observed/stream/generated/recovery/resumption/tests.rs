//! Original numerical replay, retained budgets and source-linked message admission.
use super::*;
use crate::{Snapshot, action::ElapsedTick};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    MAX_SAMPLING_ENTRIES, tokenizer::ByteBpe,
};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderBudget, MAX_DECODER_PRODUCTS};
use crate::action::consequence::delivery::persistent::{JournalIo, RecoveryReserve};
use crate::action::consequence::delivery::persistent::observed::{
    FileOversightProfile, decoder::FileDecoderConfig, stream::generated::FileTextMessageRequest,
};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};

#[allow(dead_code)]
mod fixture {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));
    pub(super) fn stopped() -> FileDecoderConfig { configured(3.0, 65, Some(256)) }
}
use fixture::{Directory, bytes, command, request, tokenizer};
fn profile() -> FileOversightProfile {
    let mut p = fixture::host_profile(); p.delivery.initial_payload.clear(); p.delivery.total = 4096;
    p.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap(); p
}
fn stream() -> StreamProfile { StreamProfile::new(9, 1, 4, 64, 256).unwrap() }
fn owner(root: &Directory) -> FileOversight {
    let (mut host, _) = FileOversight::create_generated_text_stream_with_reserve(root.store(), profile(),
        stream(), fixture::stopped(), tokenizer(false), RecoveryReserve::terminal()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); host
}
fn reopen(root: &Directory) -> FileOversight {
    FileOversight::open_generated_text_stream_with_reserve(root.store(), profile(), stream(),
        &fixture::stopped(), &tokenizer(false), RecoveryReserve::terminal()).unwrap().0
}
fn begin(host: &mut FileOversight) -> FileTextGenerationCommand {
    let original = command(host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), original.clone()).unwrap(); original
}
fn finish(host: &mut FileOversight) {
    for _ in 0..8 {
        let progress = host.decoder_text_progress(7).unwrap();
        if progress.is_complete() { return; }
        host.advance_decoder_text(host.revision(), 7, progress.generation_revision()).unwrap();
    }
    panic!("native fixture did not finish within its original horizon");
}
fn resume(host: &mut FileOversight) {
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
}

#[test]
fn every_interrupted_cut_and_completed_unsubmitted_cut_preserve_original_work() {
    let reference = Directory::new(); let mut control = owner(&reference);
    begin(&mut control); finish(&mut control);
    let expected = control.decoder_text_generation(7).unwrap();
    for cut in 0..=4 {
        let root = Directory::new(); let mut host = owner(&root); let original = begin(&mut host);
        for revision in 0..cut { host.advance_decoder_text(host.revision(), 7, revision).unwrap(); }
        drop(host); let mut host = reopen(&root);
        let before = bytes(&host); let numerical = host.decoder_inspection().unwrap();
        assert!(numerical.paused); assert!(!host.clock_ready());
        assert_eq!(host.prepare_decoder_text_publication_recovery(7, original.request()).unwrap(), original);
        assert_eq!(bytes(&host), before); assert_eq!(host.decoder_inspection().unwrap(), numerical);
        resume(&mut host); finish(&mut host);
        let actual = host.decoder_text_generation(7).unwrap();
        assert_eq!(actual.command(), &original);
        assert_eq!(actual.numerical().result().unwrap().work(), expected.numerical().result().unwrap().work());
        assert_eq!(actual.result().unwrap().bytes().unwrap(), b"A");
        assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 4);
        assert_eq!(host.decoder_inspection().unwrap().numerical.position, 4);
        assert_eq!(host.inspect().executions, 0); // Preparation never publishes.
    }
}

#[test]
fn all_input_and_budget_changes_refuse_without_replacing_the_pending_intent() {
    let root = Directory::new(); let mut host = owner(&root); let original = begin(&mut host);
    host.advance_decoder_text(host.revision(), 7, 0).unwrap();
    let before = bytes(&host);
    for field in 0..7 {
        let mut changed = original.request().clone();
        match field {
            0 => changed.prompt.push(b'!'), 1 => changed.max_new_tokens += 1,
            2 => changed.max_output_bytes += 1, 3 => changed.generation.scalar_products -= 1,
            4 => changed.generation.sampling_entries -= 1, 5 => changed.stop_tokens.clear(),
            _ => changed.tokenization.pair_lookups -= 1,
        }
        assert_eq!(host.prepare_decoder_text_publication_recovery(7, &changed), Err(Error::Binding.into()));
        assert_eq!(bytes(&host), before);
    }
    assert_eq!(host.prepare_decoder_text_publication_recovery(8, original.request()), Err(Error::Missing.into()));
    assert_eq!(host.pending_decoder_text().unwrap(), Some(original));
}

#[test]
fn terminal_non_stop_results_cannot_be_rerolled_as_publication_recovery() {
    for cancel in [false, true] {
        let root = Directory::new(); let mut host = owner(&root);
        let original = command(&host, 7, request(b"ab", if cancel { 2 } else { 1 }));
        host.begin_decoder_text(host.revision(), original.clone()).unwrap();
        if cancel { host.cancel_decoder_text(host.revision(), 7, 0).unwrap(); }
        else { finish(&mut host); }
        let before = bytes(&host);
        assert!(host.prepare_decoder_text_publication_recovery(7, original.request()).is_err());
        assert_eq!(bytes(&host), before); assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn a_submitted_then_cancelled_generation_cannot_be_repackaged_under_another_request() {
    let root = Directory::new(); let mut host = owner(&root); let original = begin(&mut host); finish(&mut host);
    let source = FileTextMessageRequest { request: 91, generation: 7, generation_revision: 4,
        target: host.inspect().target, policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100) };
    host.submit_decoder_text_message(host.revision(), source, Snapshot { semantic_epoch: 1, complete: true,
        ..Snapshot::default() }).unwrap();
    host.cancel_request(host.revision(), 91).unwrap();
    let before = bytes(&host);
    assert_eq!(host.prepare_decoder_text_publication_recovery(7, original.request()), Err(Error::Duplicate.into()));
    assert_eq!(bytes(&host), before); assert_eq!(host.inspect().executions, 0);
}

#[test]
fn an_old_completed_result_cannot_replace_current_numerical_work() {
    for pending in [false, true] {
        let root = Directory::new(); let mut host = owner(&root); let original = begin(&mut host); finish(&mut host);
        if pending {
            let next = command(&host, 8, request(b"ab", 2));
            host.begin_decoder_text(host.revision(), next).unwrap();
        } else {
            let n = host.decoder_inspection().unwrap().numerical;
            host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 257,
                DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap().unwrap();
        }
        let before = bytes(&host);
        assert_eq!(host.prepare_decoder_text_publication_recovery(7, original.request()),
            Err(if pending { Error::Incomplete } else { Error::Stale }.into()));
        assert_eq!(bytes(&host), before);
    }
}

#[test]
fn ambiguous_last_token_recovers_the_actual_old_or_complete_cut_without_extra_draws() {
    for stage in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut host = owner(&root); let original = begin(&mut host);
        for revision in 0..3 { host.advance_decoder_text(host.revision(), 7, revision).unwrap(); }
        host.store.fail_once(stage);
        assert!(matches!(host.advance_decoder_text(host.revision(), 7, 3), Err(JournalError::Io(_))));
        assert_eq!(host.prepare_decoder_text_publication_recovery(7, original.request()), Err(JournalError::Unavailable));
        drop(host); let mut host = reopen(&root);
        let revision = host.decoder_text_progress(7).unwrap().generation_revision();
        assert_eq!(revision, if stage == JournalIo::DirectorySync { 4 } else { 3 });
        host.prepare_decoder_text_publication_recovery(7, original.request()).unwrap();
        resume(&mut host); finish(&mut host);
        let result = host.decoder_text_generation(7).unwrap();
        assert_eq!(result.numerical().result().unwrap().work().attempted_samples, 2);
        assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 4);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn preparation_never_upgrades_a_caller_text_deployment() {
    let root = Directory::new(); let mut host = fixture::owner(&root, &fixture::stopped());
    let original = begin(&mut host); let before = bytes(&host);
    assert_eq!(host.prepare_decoder_text_publication_recovery(7, original.request()), Err(Error::Binding.into()));
    assert_eq!(bytes(&host), before);
}
