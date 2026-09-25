//! Real canonical-image recovery with the existing synthetic numerical fixture.
//! These tests do not establish deployment isolation or anti-rollback protection.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;

#[allow(dead_code)]
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));
    pub(super) fn stopped_config() -> FileDecoderConfig { configured(3.0, 65, Some(256)) }
}
use fixtures::{Directory, bytes, command, request, tokenizer};

fn profile() -> FileOversightProfile {
    let mut profile = fixtures::host_profile();
    profile.delivery.initial_payload.clear();
    profile
}
fn stream() -> StreamProfile { StreamProfile::new(9, 1, 4, 64, 256).unwrap() }
fn create(root: &Directory, decoder: &FileDecoderConfig, reserved: bool) -> FileOversight {
    let (host, _) = if reserved {
        FileOversight::create_generated_text_stream_with_reserve(root.store(), profile(),
            stream(), decoder.clone(), tokenizer(false), RecoveryReserve::terminal())
    } else {
        FileOversight::create_generated_text_stream(root.store(), profile(),
            stream(), decoder.clone(), tokenizer(false))
    }.unwrap();
    host
}
fn reopen(root: &Directory, decoder: &FileDecoderConfig, reserved: bool)
    -> Result<(FileOversight, FileHumanReviewer), JournalError>
{
    if reserved {
        FileOversight::open_generated_text_stream_with_reserve(root.store(), profile(),
            stream(), decoder, &tokenizer(false), RecoveryReserve::terminal())
    } else {
        FileOversight::open_generated_text_stream(root.store(), profile(), stream(), decoder, &tokenizer(false))
    }
}
fn canonical(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join(storage::CANONICAL)).unwrap()
}
fn finish(host: &mut FileOversight) {
    for _ in 0..8 {
        let progress = host.decoder_generation_progress(7).unwrap();
        if progress.is_complete() {
            assert_eq!(progress.finish(), Some(Ok(GenerationFinish::StopToken)));
            return;
        }
        host.advance_decoder_generation(host.revision(), 7, progress.generation_revision()).unwrap();
    }
    panic!("bounded four-token fixture did not finish");
}

#[test]
fn native_recovery_preserves_acknowledged_progress_and_requires_explicit_resume() {
    for reserved in [false, true] {
        let root = Directory::new(); let control_root = Directory::new();
        let decoder = fixtures::stopped_config();
        let mut host = create(&root, &decoder, reserved);
        let mut control = create(&control_root, &decoder, reserved);
        for owner in [&mut host, &mut control] {
            owner.observe_time(owner.revision(), ElapsedTick(1)).unwrap();
            let intent = command(owner, 7, request(b"ab", 2));
            owner.begin_decoder_text(owner.revision(), intent).unwrap();
            owner.advance_decoder_generation(owner.revision(), 7, 0).unwrap();
        }
        let before = host.decoder_generation_progress(7).unwrap();
        let numerical = host.decoder_inspection().unwrap().numerical;
        let revision = host.revision();
        drop(host);
        let (mut host, _) = reopen(&root, &decoder, reserved).unwrap();
        assert_eq!(host.revision(), revision + 1); // original authority fence only
        assert!(host.generated_text_stream_required().unwrap());
        assert!(host.publication_guard_required());
        assert!(!host.clock_ready());
        let recovered = host.decoder_inspection().unwrap();
        assert!(recovered.paused);
        assert_eq!(recovered.numerical.actor_revision, numerical.actor_revision);
        assert_eq!(recovered.numerical.position, numerical.position);
        let after = host.decoder_generation_progress(7).unwrap();
        assert_eq!(after.generation_revision(), before.generation_revision());
        assert_eq!(after.command(), before.command());
        assert_eq!(after.tokens(), before.tokens());
        assert_eq!(host.pending_decoder_text().unwrap().unwrap().request(), &request(b"ab", 2));
        let frozen = bytes(&host);
        assert!(host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).is_err());
        assert!(host.advance_decoder_generation(host.revision(), 7, after.generation_revision()).is_err());
        assert_eq!(bytes(&host), frozen);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).unwrap();
        finish(&mut host); finish(&mut control);
        let result = host.decoder_text_generation(7).unwrap();
        let expected = control.decoder_text_generation(7).unwrap();
        assert_eq!(result.command(), expected.command());
        assert_eq!(result.result().unwrap().bytes().unwrap(), b"A");
        assert_eq!(result.result().unwrap().bytes().unwrap(), expected.result().unwrap().bytes().unwrap());
        assert_eq!(host.decoder_generation_progress(7).unwrap().generation_revision(),
            control.decoder_generation_progress(7).unwrap().generation_revision());
        assert_eq!(host.decoder_inspection().unwrap().numerical,
            control.decoder_inspection().unwrap().numerical);
        assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
        assert_eq!(host.inspect().executions, 0); // continuation is NOT publication
    }
}

#[test]
fn native_recovery_rejects_each_bootstrap_substitution_without_changing_the_image() {
    let root = Directory::new(); let decoder = fixtures::stopped_config();
    let host = create(&root, &decoder, false);
    let before = bytes(&host); drop(host);
    for which in 0..4 {
        let result = match which {
            0 => FileOversight::open_generated_text_stream(root.store(), profile(),
                StreamProfile::new(9, 2, 4, 64, 256).unwrap(), &decoder, &tokenizer(false)),
            1 => FileOversight::open_generated_text_stream(root.store(), profile(), stream(),
                &fixtures::config(3.0, 66), &tokenizer(false)),
            2 => FileOversight::open_generated_text_stream(root.store(), profile(), stream(),
                &decoder, &tokenizer(true)),
            _ => reopen(&root, &decoder, true),
        };
        assert!(matches!(result, Err(JournalError::Contract(Error::Binding))));
        assert_eq!(canonical(&root), before);
    }
    // The failed opens release their writer locks; the exact original still opens.
    let (host, _) = reopen(&root, &decoder, false).unwrap();
    assert!(host.generated_text_stream_required().unwrap());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn native_recovery_cannot_drop_an_existing_reserve_or_relabel_a_caller_text_stream() {
    let decoder = fixtures::stopped_config();
    let reserved = Directory::new();
    let host = create(&reserved, &decoder, true);
    let before = bytes(&host); drop(host);
    assert!(matches!(reopen(&reserved, &decoder, false), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&reserved), before);
    let (host, _) = reopen(&reserved, &decoder, true).unwrap();
    assert_eq!(host.journal_capacity().unwrap().reserve(), Some(RecoveryReserve::terminal()));
    drop(host);

    let ordinary = Directory::new();
    let (mut host, _) = FileOversight::create_stream(ordinary.store(), profile(), stream()).unwrap();
    host.enable_decoder(host.revision(), decoder.clone()).unwrap();
    host.enable_decoder_tokenizer(host.revision(), tokenizer(false)).unwrap();
    let before = bytes(&host); drop(host);
    assert!(matches!(reopen(&ordinary, &decoder, false), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&ordinary), before);
    // It remains a valid ordinary stream, not a malformed negative fixture.
    let (host, _) = FileOversight::open_stream(ordinary.store(), profile(), stream()).unwrap();
    assert!(!host.generated_text_stream_required().unwrap());
}

#[test]
fn native_recovery_does_not_reroll_a_held_generation() {
    let root = Directory::new(); let decoder = fixtures::config(0.5, 65);
    let mut host = create(&root, &decoder, true);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let intent = command(&host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), intent).unwrap();
    host.advance_decoder_generation(host.revision(), 7, 0).unwrap();
    let before = host.decoder_generation_progress(7).unwrap();
    assert!(before.is_complete());
    assert_ne!(before.finish(), Some(Ok(GenerationFinish::StopToken)));
    drop(host);
    let (mut host, _) = reopen(&root, &decoder, true).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let frozen = bytes(&host);
    assert!(host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).is_err());
    let after = host.advance_decoder_generation(0, 7, 0).unwrap(); // receipt-only retry
    assert_eq!(after.generation_revision(), before.generation_revision());
    assert_eq!(after.finish(), before.finish());
    assert_eq!(after.tokens(), before.tokens());
    assert_eq!(bytes(&host), frozen);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn native_bootstrap_preflight_rejects_duplicate_reserves_and_changed_stream_limits() {
    let mut events = vec![Event::GeneratedStreamBootstrap(stream()),
        Event::Core(BaseEvent::ReserveRecovery(RecoveryReserve::terminal()))];
    assert_eq!(check_generated_contract(&events, stream(), Some(&RecoveryReserve::terminal())), Ok(()));
    let narrower = StreamProfile::new(9, 1, 3, 64, 256).unwrap();
    assert_eq!(check_generated_contract(&events, narrower, Some(&RecoveryReserve::terminal())), Err(Error::Binding));
    events.push(Event::Core(BaseEvent::ReserveRecovery(RecoveryReserve::terminal())));
    assert_eq!(check_generated_contract(&events, stream(), Some(&RecoveryReserve::terminal())), Err(Error::Binding));
}
