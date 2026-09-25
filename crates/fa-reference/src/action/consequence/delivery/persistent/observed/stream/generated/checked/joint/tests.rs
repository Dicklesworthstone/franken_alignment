//! Real numerical/file operations with synthetic model weights, not live proof.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::action::consequence::delivery::persistent::{RecoveryReserve,
    observed::{decoder::text::FileTextGenerationCommand, stream::generated::checked::GeneratedPublicationFeed}};
use crate::action::consequence::delivery::persistent::observed::credibility::held_out_joint::HeldOutJointBudget;
use crate::action::consequence::delivery::publication_gate::{PublicationLimits,
    changes::{PublicationChangePolicy, freshness::PublicationFreshnessPolicy}};
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::witness::refinement::{RefinementBudget, index::routing::RoutingBudget};

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
fn selected(feed: u8) -> GeneratedPublicationProfile {
    GeneratedPublicationProfile {
        stream: StreamProfile::new(9, 1, 4, 64, 256).unwrap(),
        reserve: Some(RecoveryReserve::terminal()),
        limits: PublicationLimits { bindings: 8,
            validation: RefinementBudget { steps: 256, value_bytes: 65_536 } },
        feed: (feed != 0).then_some(GeneratedPublicationFeed {
            changes: PublicationChangePolicy { source: 7, after: 0, lookup: RoutingBudget::default() },
            freshness: PublicationFreshnessPolicy { clock_domain: 99, max_age_ticks: 20 },
            snapshot_fallback: feed == 2,
        }),
    }
}
fn joint(generation: u64) -> HeldOutJointPolicy {
    HeldOutJointPolicy::new(19, generation, 1, 1, 0, 0,
        HeldOutJointBudget { cases: 8, member_outcomes: 32 }).unwrap()
}
fn create(root: &Directory, selection: GeneratedPublicationProfile) -> FileOversight {
    FileOversight::create_generated_text_stream_with_joint_publication(root.store(), profile(),
        fixtures::stopped_config(), tokenizer(false), selection, joint(1)).unwrap().0
}
fn reopen(root: &Directory, selection: GeneratedPublicationProfile, policy: HeldOutJointPolicy)
    -> Result<(FileOversight, FileHumanReviewer), JournalError>
{
    FileOversight::open_generated_text_stream_with_joint_publication(root.store(), profile(),
        &fixtures::stopped_config(), &tokenizer(false), selection, policy)
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
    panic!("bounded native fixture did not finish");
}

#[test]
fn joint_checked_native_recovery_matches_uninterrupted_generation_in_every_feed_mode() {
    for mode in 0..3 {
        let root = Directory::new(); let control_root = Directory::new();
        let selection = selected(mode);
        let mut host = create(&root, selection);
        let mut control = create(&control_root, selection);
        assert!(host.generated_text_stream_required().unwrap());
        assert!(host.publication_guard_required());
        assert_eq!(host.publication_validation_profile().unwrap(), Some(selection.limits));
        assert_eq!(host.held_out_joint_policy().unwrap(), Some(joint(1)));
        host.check_generated_text_joint_publication(selection, joint(1)).unwrap();
        for owner in [&mut host, &mut control] {
            owner.observe_time(owner.revision(), ElapsedTick(1)).unwrap();
            let intent = command(owner, 7, request(b"ab", 2));
            owner.begin_decoder_text(owner.revision(), intent).unwrap();
            owner.advance_decoder_generation(owner.revision(), 7, 0).unwrap();
        }
        let progress = host.decoder_generation_progress(7).unwrap();
        let numerical = host.decoder_inspection().unwrap().numerical;
        let revision = host.revision(); drop(host);
        let (mut host, _) = reopen(&root, selection, joint(1)).unwrap();
        assert_eq!(host.revision(), revision + 1);
        host.check_generated_text_joint_publication(selection, joint(1)).unwrap();
        assert!(!host.clock_ready());
        assert!(host.decoder_inspection().unwrap().paused);
        let recovered = host.decoder_generation_progress(7).unwrap();
        assert_eq!(recovered.command(), progress.command());
        assert_eq!(recovered.tokens(), progress.tokens());
        assert_eq!(recovered.generation_revision(), progress.generation_revision());
        let before = bytes(&host);
        assert!(host.advance_decoder_generation(host.revision(), 7, recovered.generation_revision()).is_err());
        assert_eq!(bytes(&host), before);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).unwrap();
        finish(&mut host); finish(&mut control);
        let actual = host.decoder_text_generation(7).unwrap();
        let expected = control.decoder_text_generation(7).unwrap();
        assert_eq!(actual.result().unwrap().bytes().unwrap(), b"A");
        assert_eq!(actual.result().unwrap().bytes().unwrap(), expected.result().unwrap().bytes().unwrap());
        assert_eq!(host.decoder_inspection().unwrap().numerical, control.decoder_inspection().unwrap().numerical);
        assert_eq!(host.stream_snapshot().unwrap().published.message_count(), 0);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn joint_and_witness_substitutions_refuse_before_any_recovery_write() {
    let root = Directory::new();
    let selection = selected(2);
    let host = create(&root, selection);
    let before = bytes(&host); drop(host);
    assert!(matches!(reopen(&root, selection, joint(2)), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&root), before);
    let mut changed = selection; changed.limits.validation.steps += 1;
    assert!(matches!(reopen(&root, changed, joint(1)), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&root), before);
    let mut changed = selection; changed.feed = None;
    assert!(matches!(reopen(&root, changed, joint(1)), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&root), before);
    let mut changed = selection; changed.reserve = None;
    assert!(matches!(reopen(&root, changed, joint(1)), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&root), before);
    let (host, _) = reopen(&root, selection, joint(1)).unwrap();
    let before = bytes(&host);
    host.check_generated_text_joint_publication(selection, joint(1)).unwrap();
    assert!(host.check_generated_text_joint_publication(selection, joint(2)).is_err());
    assert_eq!(bytes(&host), before);
}

#[test]
fn recovery_does_not_retrofit_a_missing_joint_policy() {
    let root = Directory::new(); let selection = selected(1);
    let (host, _) = FileOversight::create_generated_text_stream_checked(root.store(), profile(),
        fixtures::stopped_config(), tokenizer(false), selection).unwrap();
    let before = bytes(&host); drop(host);
    assert!(matches!(reopen(&root, selection, joint(1)), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(canonical(&root), before);
    // The negative image is a valid original checked-native owner, not corruption.
    let (host, _) = FileOversight::open_generated_text_stream_checked(root.store(), profile(),
        &fixtures::stopped_config(), &tokenizer(false), selection).unwrap();
    assert_eq!(host.held_out_joint_policy().unwrap(), None);
    assert_eq!(host.publication_validation_profile().unwrap(), Some(selection.limits));
}

#[test]
fn invalid_combined_configuration_cannot_create_a_partially_guarded_store() {
    for bad_count in [false, true] {
        let root = Directory::new(); let mut p = profile(); let mut selection = selected(0);
        if bad_count { p.delivery.limits.events = 5; } else { selection.limits.bindings = 0; }
        assert!(FileOversight::create_generated_text_stream_with_joint_publication(root.store(), p,
            fixtures::stopped_config(), tokenizer(false), selection, joint(1)).is_err());
        assert!(!root.store().exists());
        let host = create(&root, selected(0));
        host.check_generated_text_joint_publication(selected(0), joint(1)).unwrap();
    }
}

#[test]
fn joint_preflight_requires_one_exact_policy_and_preserves_other_history() {
    let original = Event::Credibility(CredibilityEvent::EnableHeldOutJoint(joint(1)));
    let mut events = vec![original.clone()];
    assert_eq!(check_joint(&events, joint(1)), Ok(()));
    events.push(Event::Core(BaseEvent::Time(ElapsedTick(1))));
    assert_eq!(check_joint(&events, joint(1)), Ok(()));
    assert_eq!(check_joint(&events, joint(2)), Err(Error::Binding));
    events.push(original);
    assert_eq!(check_joint(&events, joint(1)), Err(Error::Binding));
    assert_eq!(check_joint(&[], joint(1)), Err(Error::Binding));
}
