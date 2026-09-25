//! Real canonical files and original numerical computation; synthetic model only.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;
use crate::witness::refinement::{RefinementBudget, index::routing::RoutingBudget};

#[allow(dead_code)]
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));
    pub(super) fn stopped_config() -> FileDecoderConfig { configured(3.0, 65, Some(256)) }
}
use fixtures::{Directory, bytes, command, request, tokenizer};

fn profile() -> FileOversightProfile {
    let mut p = fixtures::host_profile(); p.delivery.initial_payload.clear(); p
}
fn selection(feed: bool, fallback: bool) -> GeneratedPublicationProfile {
    GeneratedPublicationProfile {
        stream: StreamProfile::new(9, 1, 4, 64, 256).unwrap(),
        reserve: Some(RecoveryReserve::terminal()),
        limits: PublicationLimits { bindings: 4, validation: RefinementBudget { steps: 1000, value_bytes: 10000 } },
        feed: feed.then_some(GeneratedPublicationFeed {
            changes: PublicationChangePolicy { source: 71, after: 0, lookup: RoutingBudget { steps: 1000, bytes: 10000 } },
            freshness: PublicationFreshnessPolicy { clock_domain: 99, max_age_ticks: 50 },
            snapshot_fallback: fallback,
        }),
    }
}
fn create(root: &Directory, selected: GeneratedPublicationProfile) -> FileOversight {
    FileOversight::create_generated_text_stream_checked(root.store(), profile(),
        fixtures::stopped_config(), tokenizer(false), selected).unwrap().0
}
fn reopen(root: &Directory, selected: GeneratedPublicationProfile)
    -> Result<(FileOversight, FileHumanReviewer), JournalError>
{
    FileOversight::open_generated_text_stream_checked(root.store(), profile(),
        &fixtures::stopped_config(), &tokenizer(false), selected)
}
fn canonical(root: &Directory) -> Vec<u8> { std::fs::read(root.store().join(storage::CANONICAL)).unwrap() }

#[test]
fn checked_native_bootstrap_contains_every_guard_without_computation_or_publication() {
    for (feed, fallback) in [(false, false), (true, false), (true, true)] {
        let root = Directory::new(); let selected = selection(feed, fallback);
        let host = create(&root, selected);
        assert_eq!(selected.check(&host.events), Ok(()));
        assert!(host.generated_text_stream_required().unwrap());
        assert!(host.publication_guard_required());
        assert_eq!(host.publication_validation_profile().unwrap(), Some(selected.limits));
        assert_eq!(host.journal_capacity().unwrap().reserve(), selected.reserve);
        if feed {
            assert_eq!(host.publication_snapshot_fallback_enabled().unwrap(), fallback);
            assert_eq!(host.publication_change_status().unwrap().source, 71);
        }
        assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
    }
}

#[test]
fn checked_native_recovery_rejects_each_contract_change_before_touching_the_image() {
    let root = Directory::new(); let selected = selection(true, true);
    let host = create(&root, selected); let before = bytes(&host); drop(host);
    for which in 0..10 {
        let mut changed = selected;
        match which {
            0 => changed.limits.validation.steps -= 1,
            1 => changed.limits.bindings -= 1,
            2 => changed.feed.as_mut().unwrap().changes.source += 1,
            3 => changed.feed.as_mut().unwrap().changes.after += 1,
            4 => changed.feed.as_mut().unwrap().changes.lookup.bytes -= 1,
            5 => changed.feed.as_mut().unwrap().freshness.max_age_ticks += 1,
            6 => changed.feed.as_mut().unwrap().snapshot_fallback = false,
            7 => changed.feed = None,
            8 => changed.reserve = None,
            _ => changed.stream = StreamProfile::new(9, 2, 4, 64, 256).unwrap(),
        }
        assert!(matches!(reopen(&root, changed), Err(JournalError::Contract(Error::Binding))), "case {which}");
        assert_eq!(canonical(&root), before, "case {which}");
    }
    let (host, _) = reopen(&root, selected).unwrap();
    assert_eq!(selected.check(&host.events), Ok(()));
    assert!(host.decoder_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn checked_native_recovery_continues_original_progress_without_bypassing_witnesses() {
    let root = Directory::new(); let control_root = Directory::new();
    let selected = selection(false, false);
    let mut host = create(&root, selected); let mut control = create(&control_root, selected);
    for owner in [&mut host, &mut control] {
        owner.observe_time(owner.revision(), ElapsedTick(1)).unwrap();
        let intent = command(owner, 7, request(b"ab", 2));
        owner.begin_decoder_text(owner.revision(), intent).unwrap();
        owner.advance_decoder_text(owner.revision(), 7, 0).unwrap();
    }
    let before = host.decoder_text_progress(7).unwrap(); drop(host);
    let (mut host, _) = reopen(&root, selected).unwrap();
    let frozen = bytes(&host);
    assert!(host.advance_decoder_text(host.revision(), 7, 1).is_err());
    assert_eq!(bytes(&host), frozen);
    assert_eq!(host.decoder_text_progress(7).unwrap().command(), before.command());
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
    for owner in [&mut host, &mut control] {
        for revision in 1..4 { owner.advance_decoder_text(owner.revision(), 7, revision).unwrap(); }
        let result = owner.decoder_text_progress(7).unwrap();
        assert_eq!(result.finish(), Some(Ok(GenerationFinish::StopToken)));
        assert_eq!(result.bytes().unwrap(), b"A");
        assert_eq!(result.generation_revision(), 4);
        assert_eq!(owner.publication_validation_profile().unwrap(), Some(selected.limits));
        assert!(owner.publication_guard_required());
        assert_eq!(owner.inspect().executions, 0);
    }
    assert_eq!(host.decoder_inspection().unwrap().numerical, control.decoder_inspection().unwrap().numerical);
}

#[test]
fn checked_native_open_cannot_upgrade_an_unchecked_native_or_ordinary_stream() {
    for native in [false, true] {
        let root = Directory::new(); let selected = selection(false, false);
        let (host, _) = if native {
            FileOversight::create_generated_text_stream_with_reserve(root.store(), profile(), selected.stream,
                fixtures::stopped_config(), tokenizer(false), RecoveryReserve::terminal())
        } else {
            FileOversight::create_stream(root.store(), profile(), selected.stream)
        }.unwrap();
        let before = bytes(&host); drop(host);
        assert!(matches!(reopen(&root, selected), Err(JournalError::Contract(Error::Binding))));
        assert_eq!(canonical(&root), before);
        let (host, _) = if native {
            FileOversight::open_generated_text_stream_with_reserve(root.store(), profile(), selected.stream,
                &fixtures::stopped_config(), &tokenizer(false), RecoveryReserve::terminal())
        } else { FileOversight::open_stream(root.store(), profile(), selected.stream) }.unwrap();
        assert_eq!(host.generated_text_stream_required().unwrap(), native);
        assert_eq!(host.publication_validation_profile().unwrap(), None);
    }
}

#[test]
fn checked_native_invalid_limits_and_foreign_clock_fail_before_store_creation() {
    for which in 0..3 {
        let root = Directory::new(); let mut selected = selection(true, false);
        match which {
            0 => selected.limits.bindings = 0,
            1 => selected.feed.as_mut().unwrap().freshness.clock_domain += 1,
            _ => selected.feed.as_mut().unwrap().changes.source = 0,
        }
        assert!(FileOversight::create_generated_text_stream_checked(root.store(), profile(),
            fixtures::stopped_config(), tokenizer(false), selected).is_err());
        assert!(!root.store().exists());
    }
}

#[test]
fn checked_native_bootstrap_rejects_duplicates_and_unselected_routing() {
    let selected = selection(true, true);
    let mut events = vec![Event::GeneratedStreamBootstrap(selected.stream),
        Event::Core(BaseEvent::ReserveRecovery(RecoveryReserve::terminal()))];
    events.extend(selected.witnesses().into_iter().map(Event::PublicationWitness));
    assert_eq!(selected.check(&events), Ok(()));
    for extra in [WitnessEvent::Enable(selected.limits), WitnessEvent::SubtreeRouting,
        WitnessEvent::Freshness(FreshnessEvent::SnapshotFallback)] {
        events.push(Event::PublicationWitness(extra));
        assert_eq!(selected.check(&events), Err(Error::Binding));
        events.pop();
    }
    events.push(Event::Core(BaseEvent::ReserveRecovery(RecoveryReserve::terminal())));
    assert_eq!(selected.check(&events), Err(Error::Binding));
}
