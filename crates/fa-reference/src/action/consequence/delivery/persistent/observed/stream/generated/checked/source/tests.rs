//! Real canonical replacements and original inference; synthetic weights only.
use super::*;
use super::super::{GeneratedPublicationFeed, PublicationLimits, PublicationChangePolicy,
    PublicationFreshnessPolicy};
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::action::consequence::delivery::persistent::JournalIo;
use crate::action::consequence::delivery::persistent::observed::{
    decoder::text::FileTextGenerationCommand, source::FileSourceReplacement,
    credibility::held_out_joint::HeldOutJointBudget,
};
use crate::action::consequence::oversight::{
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateSource, StateLimits, StateFreshness},
};
use crate::witness::refinement::{RefinementBudget, index::routing::RoutingBudget};
use crate::Snapshot;
use std::collections::BTreeMap;

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
fn joint(generation: u64) -> HeldOutJointPolicy {
    HeldOutJointPolicy::new(70, generation, 1, 1, 0, 0, HeldOutJointBudget::default()).unwrap()
}
fn selection(mode: usize) -> GeneratedSourceProfile {
    let stream = StreamProfile::new(9, 1, 4, 64, 256).unwrap();
    let reserve = (mode != 0).then_some(RecoveryReserve::terminal());
    let checked = GeneratedPublicationProfile { stream, reserve,
        limits: PublicationLimits { bindings: 4,
            validation: RefinementBudget { steps: 1000, value_bytes: 10000 } },
        feed: (mode >= 3).then_some(GeneratedPublicationFeed {
            changes: PublicationChangePolicy { source: 71, after: 0,
                lookup: RoutingBudget { steps: 1000, bytes: 10000 } },
            freshness: PublicationFreshnessPolicy { clock_domain: 99, max_age_ticks: 50 },
            snapshot_fallback: mode >= 4,
        }),
    };
    GeneratedSourceProfile {
        source: FileSourcePolicy { source: StateSource {
            scope: profile().delivery.scope, source: 51, generation: 1 },
            limits: StateLimits { events: 64, retained_bytes: 65536 },
            freshness: StateFreshness::new(50).unwrap() },
        publication: match mode {
            0 | 1 => GeneratedSourcePublication::Stream { stream, reserve },
            2..=4 => GeneratedSourcePublication::Checked(checked),
            _ => GeneratedSourcePublication::Joint { publication: checked, policy: joint(1) },
        },
    }
}
fn create(root: &Directory, selected: GeneratedSourceProfile) -> FileOversight {
    FileOversight::create_generated_text_stream_from_source(root.store(), profile(),
        fixtures::stopped_config(), tokenizer(false), selected).unwrap().0
}
fn reopen(root: &Directory, selected: GeneratedSourceProfile)
    -> Result<(FileOversight, FileHumanReviewer), JournalError>
{
    FileOversight::open_generated_text_stream_from_source(root.store(), profile(),
        &fixtures::stopped_config(), &tokenizer(false), selected)
}
fn canonical(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join(storage::CANONICAL)).unwrap()
}
fn evidence(root: &Directory) -> FileEvidenceSource {
    let path = root.store().parent().unwrap().join("evidence.json");
    let scope = profile().delivery.scope;
    let snapshot = EvidenceSnapshot::new(EvidenceIdentity { source: 51, generation: 1, scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() },
        BTreeMap::from([("reviewer".into(), b"actual file context".to_vec())])).unwrap();
    std::fs::write(&path, snapshot.encode()).unwrap();
    FileEvidenceSource::new(path, 51, scope, 1048576).unwrap()
}

#[test]
fn source_bootstrap_installs_every_selected_contract_without_inference_or_capture() {
    for mode in 0..6 {
        let root = Directory::new(); let selected = selection(mode);
        let host = create(&root, selected);
        assert_eq!(selected.check(&host.events), Ok(()));
        assert_eq!(selected.check_machine(&host.machine), Ok(()));
        let source = host.file_source_status().unwrap();
        assert_eq!(source.policy, selected.source);
        assert_eq!(source.producer, None);
        assert_eq!(source.capture.closed, None);
        assert!(!host.clock_ready());
        assert!(host.generated_text_stream_required().unwrap());
        assert!(host.publication_guard_required());
        assert_eq!(host.held_out_joint_policy().unwrap(), selected.publication.joint());
        assert_eq!(host.journal_capacity().unwrap().reserve(), selected.publication.reserve());
        assert_eq!(host.publication_validation_profile().unwrap(),
            selected.publication.witnesses().map(|p| p.limits));
        let numerical = host.decoder_inspection().unwrap().numerical;
        assert_eq!(numerical.position, 0); assert_eq!(numerical.sampled_draws, 0);
        assert_eq!(host.inspect().executions, 0);
        assert!(host.pending_decoder_generation().unwrap().is_none());
        assert_eq!(host.events.iter().filter(|e| matches!(e, Event::Source(SourceEvent::Enable(_)))).count(), 1);
        assert!(matches!(host.events.last(), Some(Event::Source(SourceEvent::Enable(_)))));
    }
}

#[test]
fn source_bootstrap_invalid_configuration_refuses_before_creating_the_store() {
    for which in 0..5 {
        let root = Directory::new(); let selected = selection(5);
        let mut bad = selected; let mut p = profile();
        match which {
            0 => bad.source.source.scope.principal += 1,
            1 => bad.source.source.source = 0,
            2 => bad.source.source.generation = 0,
            3 => bad.source.limits.events = 0,
            _ => p.delivery.limits.events = 1,
        }
        assert!(FileOversight::create_generated_text_stream_from_source(root.store(), p,
            fixtures::stopped_config(), tokenizer(false), bad).is_err());
        assert!(!root.store().exists());
        let host = create(&root, selected); // same destination, valid near-control
        assert!(host.file_source_required());
    }
}

#[test]
fn source_bootstrap_each_replacement_failure_leaves_no_model_only_canonical_image() {
    for mode in [0, 2, 5] {
        for stage in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
            JournalIo::Rename, JournalIo::DirectorySync] {
            let root = Directory::new(); let selected = selection(mode);
            let prepared = Prepared::new(profile(), fixtures::stopped_config(), &tokenizer(false), selected).unwrap();
            let store = storage::Store::create(&root.store()).unwrap();
            let expected = journal::encode(&prepared.profile, store.identity(), &prepared.events).unwrap();
            store.fail_once(stage);
            assert!(matches!(prepared.publish(store), Err(JournalError::Io(f)) if f.operation == stage));
            let path = root.store().join(storage::CANONICAL);
            if stage == JournalIo::DirectorySync {
                assert_eq!(canonical(&root), expected);
                let (host, _) = reopen(&root, selected).unwrap();
                assert!(host.file_source_required());
                assert_eq!(selected.check(&host.events), Ok(()));
                assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
                assert_eq!(host.inspect().executions, 0);
            } else { assert!(!path.exists(), "{stage:?}"); }
        }
    }
}

#[test]
fn source_recovery_rejects_every_source_substitution_before_cleanup_or_fence() {
    let root = Directory::new(); let selected = selection(5);
    let host = create(&root, selected); let before = bytes(&host); drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged staging sentinel").unwrap();
    for which in 0..6 {
        let mut bad = selected;
        match which {
            0 => bad.source.source.source += 1,
            1 => bad.source.source.generation += 1,
            2 => bad.source.source.scope.principal += 1,
            3 => bad.source.limits.events -= 1,
            4 => bad.source.limits.retained_bytes -= 1,
            _ => bad.source.freshness = StateFreshness::new(51).unwrap(),
        }
        assert!(matches!(reopen(&root, bad), Err(JournalError::Contract(Error::Binding))), "{which}");
        assert_eq!(canonical(&root), before);
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging sentinel");
    }
    let (host, _) = reopen(&root, selected).unwrap();
    assert!(!pending.exists());
    assert!(host.decoder_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    assert_eq!(host.file_source_status().unwrap().capture.closed, None);
    assert_eq!(host.file_source_status().unwrap().policy, selected.source);
}

#[test]
fn source_recovery_preserves_pending_progress_and_still_needs_actual_evidence() {
    for mode in [0, 2, 5] {
        let root = Directory::new(); let other = Directory::new();
        let selected = selection(mode);
        let mut host = create(&root, selected); let mut control = create(&other, selected);
        let mut file = evidence(&root); let mut control_file = evidence(&other);
        for (owner, source) in [(&mut host, &mut file), (&mut control, &mut control_file)] {
            owner.refresh_file_source(owner.revision(), source, ElapsedTick(1)).unwrap();
            let cmd = command(owner, 7, request(b"ab", 2));
            owner.begin_decoder_text(owner.revision(), cmd).unwrap();
            owner.advance_decoder_text(owner.revision(), 7, 0).unwrap();
        }
        let before = host.decoder_text_progress(7).unwrap(); drop(host);
        let (mut host, _) = reopen(&root, selected).unwrap();
        assert_eq!(host.decoder_text_progress(7).unwrap().command(), before.command());
        assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 1);
        let frozen = bytes(&host);
        assert!(host.advance_decoder_text(host.revision(), 7, 1).is_err());
        assert_eq!(bytes(&host), frozen);
        // Only a new actual file observation restores source eligibility.
        host.refresh_file_source(host.revision(), &mut file, ElapsedTick(2)).unwrap();
        let n = host.decoder_inspection().unwrap().numerical;
        host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
        for owner in [&mut host, &mut control] {
            for revision in 1..4 { owner.advance_decoder_text(owner.revision(), 7, revision).unwrap(); }
            let text = owner.decoder_text_progress(7).unwrap();
            assert_eq!(text.bytes().unwrap(), b"A");
            assert_eq!(text.finish(), Some(Ok(GenerationFinish::StopToken)));
            assert_eq!(owner.inspect().executions, 0);
        }
        assert_eq!(host.decoder_inspection().unwrap().numerical, control.decoder_inspection().unwrap().numerical);
    }
}

#[test]
fn source_recovery_cannot_add_or_drop_a_publication_or_joint_requirement() {
    for mode in [1, 2, 5] {
        let root = Directory::new(); let selected = selection(mode);
        let host = create(&root, selected); let before = bytes(&host); drop(host);
        for wrong_mode in [1, 2, 5] {
            if mode == wrong_mode { continue; }
            assert!(matches!(reopen(&root, selection(wrong_mode)), Err(JournalError::Contract(Error::Binding))));
            assert_eq!(canonical(&root), before);
        }
        let (host, _) = reopen(&root, selected).unwrap();
        assert_eq!(selected.check(&host.events), Ok(()));
    }
}

#[test]
fn source_recovery_accepts_compatible_legacy_creation_but_never_installs_missing_source() {
    for installed in [false, true] {
        let root = Directory::new(); let selected = selection(1);
        let (mut host, _) = FileOversight::create_generated_text_stream_with_reserve(root.store(),
            profile(), selected.publication.stream(), fixtures::stopped_config(), tokenizer(false),
            RecoveryReserve::terminal()).unwrap();
        if installed { host.enable_file_source(host.revision(), selected.source).unwrap(); }
        let before = bytes(&host); drop(host);
        if installed {
            let (host, _) = reopen(&root, selected).unwrap();
            assert_eq!(host.file_source_status().unwrap().policy, selected.source);
        } else {
            assert!(matches!(reopen(&root, selected), Err(JournalError::Contract(Error::Binding))));
            assert_eq!(canonical(&root), before);
        }
    }
}

#[test]
fn source_recovery_retains_governed_replacement_without_resetting_its_generation() {
    let root = Directory::new(); let mut selected = selection(1);
    let mut host = create(&root, selected);
    let mut file = evidence(&root);
    host.refresh_file_source(host.revision(), &mut file, ElapsedTick(1)).unwrap();
    let request = FileSourceReplacement { operation: 70, expected_generation: 1,
        expected_authority_epoch: host.inspect().control.ledger.epoch, next_generation: 2 };
    let receipt = host.replace_file_source(host.revision(), request).unwrap();
    let before = bytes(&host); drop(host);
    assert!(matches!(reopen(&root, selected), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&root), before);
    selected.source.source.generation = 2;
    let (mut host, _) = reopen(&root, selected).unwrap();
    assert_eq!(host.file_source_status().unwrap().policy, selected.source);
    let before = bytes(&host);
    assert_eq!(host.replace_file_source(0, request).unwrap(), receipt);
    assert_eq!(bytes(&host), before);
    assert_eq!(host.file_source_status().unwrap().capture.closed, None);
}

#[test]
fn source_preflight_does_not_validate_forged_replacement_or_duplicate_bootstraps() {
    let selected = selection(5);
    let events = selected.events(fixtures::stopped_config(), &tokenizer(false)).unwrap();
    for extra in [Event::Source(SourceEvent::Enable(selected.source)),
        Event::Credibility(CredibilityEvent::EnableHeldOutJoint(joint(1)))] {
        let mut bad = events.clone(); bad.push(extra);
        assert_eq!(selected.check(&bad), Err(Error::Binding));
    }
    let mut malformed = events;
    malformed.push(Event::Source(SourceEvent::Replace(FileSourceReplacement {
        operation: 70, expected_generation: 99, expected_authority_epoch: 0, next_generation: 2,
    })));
    let mut replacement = selected; replacement.source.source.generation = 2;
    assert_eq!(replacement.check(&malformed), Ok(())); // projection is not validation
    assert!(Machine::replay(&profile(), &malformed).is_err());
}
