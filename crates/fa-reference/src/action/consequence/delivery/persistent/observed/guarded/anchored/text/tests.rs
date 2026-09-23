//! Real canonical files, original numerical cursors and original guard custody.
use super::*;
use crate::action::ElapsedTick;
use crate::action::consequence::delivery::persistent::{JournalIo, JournalLimits};
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileCampaignRequirement, FileGuardSet, FileRecoveryFloor,
};
use crate::action::consequence::delivery::persistent::observed::decoder::text::FileTextGenerationCommand;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::action::consequence::policy_campaign::ReplayLimits;

// Reuse the unchanged native weights/tokenizer fixtures at their original path.
// Their weights are synthetic; the original decoder, monitor and journal execute.
#[path = "../../../decoder/text/tests/fixtures.rs"]
mod fixtures;
use fixtures::*;

const BARRIERS: [JournalIo; 5] = [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync,
    JournalIo::Rename, JournalIo::DirectorySync];

fn guards(config: &FileDecoderConfig) -> FileGuardSet {
    FileGuardSet { stream: None, decoder: Some(config.clone()), decoder_stop: None,
        source: None, identity: None, credential: None,
        campaigns: Some(FileCampaignRequirement {
            limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 }, max_campaigns: 8,
        }) }
}
fn requirements(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let inspection = host.inspect();
    FileRecoveryRequirements { guards, effective_policy: host.profile.delivery.policy.clone(),
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: host.revision(),
            control_sequence: inspection.control.sequence, authority_epoch: inspection.control.ledger.epoch } }
}
fn fresh(root: &Directory, config: &FileDecoderConfig, profile: FileOversightProfile)
    -> (FileOversight, FileGuardSet)
{
    let guards = guards(config);
    let (mut host, _) = FileOversight::create_guarded(root.store(), profile, &guards, None).unwrap();
    host.enable_decoder_tokenizer(host.revision(), tokenizer(false)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, guards)
}
fn zero_floor(expected: &mut FileRecoveryRequirements) {
    expected.minimum = FileRecoveryFloor { journal_revision: 0, control_sequence: 0, authority_epoch: 0 };
}
fn reopen(root: &Directory, expected: &FileRecoveryRequirements, anchor: &FileHistoryAnchor)
    -> Result<(FileOversight, FileOversightRoles), JournalError>
{
    FileOversight::open_guarded_text_anchored(root.store(), host_profile(), expected, &tokenizer(false), anchor)
}

#[test]
fn text_anchor_recovers_every_cursor_cut_with_roles_and_without_duplicate_samples() {
    for cut in 0..=4 {
        let root = Directory::new(); let c = config(3.0, 65);
        let (mut host, guards) = fresh(&root, &c, host_profile());
        let intent = command(&host, 7, request(b"ab", 2));
        host.begin_decoder_text(host.revision(), intent).unwrap();
        let mut progress = host.decoder_text_progress(7).unwrap();
        for _ in 0..cut {
            progress = host.advance_decoder_text(host.revision(), 7, progress.generation_revision()).unwrap();
        }
        let expected = requirements(&host, guards); let anchor = host.history_anchor().unwrap();
        let prefix = progress.bytes().unwrap().to_vec(); let revision = host.revision();
        let spent = host.decoder_inspection().unwrap().numerical.sampled_draws;
        drop(host);
        let (mut host, roles) = reopen(&root, &expected, &anchor).unwrap();
        assert_eq!(host.revision(), revision + 1);
        assert!(roles.policy_governor.is_some()); assert!(roles.identity_observer.is_none());
        assert!(host.publication_guard_required()); assert!(host.policy_campaigns_required());
        assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
        progress = host.decoder_text_progress(7).unwrap();
        assert_eq!(progress.bytes().unwrap(), prefix);
        assert_eq!(progress.generation_revision(), cut);
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, spent);
        if !progress.is_complete() {
            let disk = bytes(&host);
            assert!(host.advance_decoder_text(host.revision(), 7, progress.generation_revision()).is_err());
            assert_eq!(bytes(&host), disk);
            let n = host.decoder_inspection().unwrap().numerical;
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            host.resume_decoder(host.revision(), n.actor_revision, n.position).unwrap();
        }
        let mut delivered = prefix;
        while !progress.is_complete() {
            progress = host.advance_decoder_text(host.revision(), 7, progress.generation_revision()).unwrap();
            delivered.extend_from_slice(progress.delta_from(delivered.len()).unwrap().bytes());
        }
        assert_eq!(delivered, b"AA");
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(progress.finish(), Some(Ok(GenerationFinish::TokenLimit)));
        assert_eq!(host.inspect().executions, 0);
        assert!(host.history_anchor_after(&anchor).unwrap().revision() > anchor.revision());
    }
}

#[test]
fn text_anchor_rejects_valid_rollback_even_when_numeric_floors_are_zero() {
    let root = Directory::new(); let c = config(3.0, 65);
    let (mut host, guards) = fresh(&root, &c, host_profile());
    let old = bytes(&host);
    let intent = command(&host, 7, request(b"ab", 2));
    host.generate_decoder_text(host.revision(), intent).unwrap();
    let latest = bytes(&host); let anchor = host.history_anchor().unwrap();
    let mut expected = requirements(&host, guards); zero_floor(&mut expected);
    let path = root.store().join(storage::CANONICAL); drop(host);
    std::fs::write(&path, &old).unwrap();
    // Causal control: the OLD profile/model-only opener accepts this valid cut.
    let (weaker, _) = FileOversight::open_with_text_decoder(root.store(), host_profile(), &c, &tokenizer(false)).unwrap();
    assert!(weaker.decoder_text_generation(7).is_err()); drop(weaker);
    std::fs::write(&path, &old).unwrap();
    assert!(matches!(reopen(&root, &expected, &anchor), Err(JournalError::Contract(Error::Stale))));
    assert_eq!(std::fs::read(&path).unwrap(), old);
    std::fs::write(&path, &latest).unwrap();
    let (host, _) = reopen(&root, &expected, &anchor).unwrap();
    assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"AA");
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
}

#[test]
fn text_anchor_rejects_an_equal_counter_fork_accepted_by_guard_only_recovery() {
    let root = Directory::new(); let c = config(3.0, 65);
    let (host, guards) = fresh(&root, &c, host_profile());
    let original = bytes(&host); let anchor = host.history_anchor().unwrap();
    let mut expected = requirements(&host, guards); zero_floor(&mut expected);
    let mut events = host.events.clone();
    assert!(matches!(events.last(), Some(Event::Core(BaseEvent::Time(ElapsedTick(1))))));
    *events.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(2)));
    let fork = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    let original_state = host.inspect();
    let alternative = Machine::replay(&host.profile, &events).unwrap().snapshot(events.len());
    assert_eq!(alternative.revision, original_state.revision);
    assert_eq!(alternative.control.sequence, original_state.control.sequence);
    assert_eq!(alternative.control.ledger.epoch, original_state.control.ledger.epoch);
    let path = root.store().join(storage::CANONICAL); drop(host);
    std::fs::write(&path, &fork).unwrap();
    let (weak, _) = FileOversight::open_guarded(root.store(), host_profile(), &expected).unwrap(); drop(weak);
    std::fs::write(&path, &fork).unwrap();
    assert!(matches!(reopen(&root, &expected, &anchor), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(&path).unwrap(), fork);
    std::fs::write(&path, &original).unwrap();
    assert!(reopen(&root, &expected, &anchor).is_ok());
}

#[test]
fn text_anchor_pins_tokenizer_and_model_before_replaying_an_invalid_suffix() {
    let root = Directory::new(); let c = config(3.0, 65);
    let (host, guards) = fresh(&root, &c, host_profile());
    let original = bytes(&host); let anchor = host.history_anchor().unwrap();
    let expected = requirements(&host, guards);
    let mut events = host.events.clone(); events.push(Event::Core(BaseEvent::Cancel(u64::MAX)));
    let invalid = journal::encode(&host.profile, host.store.identity(), &events).unwrap();
    let path = root.store().join(storage::CANONICAL); drop(host);
    std::fs::write(&path, &invalid).unwrap();
    assert!(matches!(FileOversight::open_guarded_text_anchored(root.store(), host_profile(),
        &expected, &tokenizer(true), &anchor), Err(JournalError::Contract(Error::Binding))));
    let mut changed = expected.clone(); changed.guards.decoder = Some(config(4.0, 65));
    assert!(matches!(reopen(&root, &changed, &anchor), Err(JournalError::Contract(Error::Binding))));
    // With correct pins, the original reducer reaches and rejects the bad event.
    assert!(matches!(reopen(&root, &expected, &anchor), Err(JournalError::Contract(Error::Missing))));
    assert_eq!(std::fs::read(&path).unwrap(), invalid);
    std::fs::write(&path, original).unwrap(); assert!(reopen(&root, &expected, &anchor).is_ok());
}

#[test]
fn text_anchor_does_not_replace_policy_guard_credential_or_floor_checks() {
    let root = Directory::new(); let c = config(3.0, 65);
    let (host, guards) = fresh(&root, &c, host_profile());
    let original = bytes(&host); let anchor = host.history_anchor().unwrap();
    let expected = requirements(&host, guards); drop(host);
    for mode in 0..5 {
        let mut wrong = expected.clone();
        match mode {
            0 => wrong.guards.campaigns = None,
            1 => wrong.effective_policy = crate::action::consequence::gate::containment::session::policy::Policy::new(
                2, vec![crate::action::consequence::gate::containment::session::policy::Predicate::PayloadAtMost(127)]).unwrap(),
            2 => wrong.minimum.journal_revision += 1,
            3 => wrong.credential_epoch = Some(super::super::super::FileCredentialEpoch { generation: 1, revoked: true }),
            _ => wrong.guards.decoder = None,
        }
        assert!(reopen(&root, &wrong, &anchor).is_err());
        assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), original);
    }
    assert!(reopen(&root, &expected, &anchor).is_ok());
}

#[test]
fn text_anchor_cannot_be_transplanted_to_an_equivalent_foreign_owner() {
    let a = Directory::new(); let b = Directory::new(); let c = config(3.0, 65);
    let (first, _) = fresh(&a, &c, host_profile());
    let foreign = first.history_anchor().unwrap(); drop(first);
    let (second, guards) = fresh(&b, &c, host_profile());
    let own = second.history_anchor().unwrap(); let original = bytes(&second);
    let expected = requirements(&second, guards); drop(second);
    assert!(matches!(reopen(&b, &expected, &foreign), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(b.store().join(storage::CANONICAL)).unwrap(), original);
    assert!(reopen(&b, &expected, &own).is_ok());
}

#[test]
fn text_anchor_requires_the_actual_publication_guard_and_tokenizer_installation() {
    let c = config(3.0, 65); let root = Directory::new();
    // Original fixture deliberately has no publication guard or campaign gate.
    let host = owner(&root, &c); let anchor = host.history_anchor().unwrap();
    let mut g = guards(&c); g.campaigns = None;
    let expected = requirements(&host, g); let original = bytes(&host); drop(host);
    assert!(matches!(reopen(&root, &expected, &anchor), Err(JournalError::Contract(Error::Incomplete))));
    assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), original);
    let root = Directory::new(); let g = guards(&c);
    let (host, _) = FileOversight::create_guarded(root.store(), host_profile(), &g, None).unwrap();
    let expected = requirements(&host, g); let anchor = host.history_anchor().unwrap(); drop(host);
    assert!(matches!(reopen(&root, &expected, &anchor), Err(JournalError::Contract(Error::Binding))));
}

#[test]
fn text_anchor_fence_capacity_is_exact_and_failure_cannot_publish_roles() {
    let baseline = Directory::new(); let c = config(3.0, 65);
    let (host, _) = fresh(&baseline, &c, host_profile()); let count = host.revision() as usize; drop(host);
    for extra in [0, 1] {
        let root = Directory::new(); let mut profile = host_profile();
        profile.delivery.limits = JournalLimits { events: count + extra, ..profile.delivery.limits };
        let (host, guards) = fresh(&root, &c, profile.clone());
        let expected = requirements(&host, guards); let anchor = host.history_anchor().unwrap();
        let original = bytes(&host); drop(host);
        let result = FileOversight::open_guarded_text_anchored(root.store(), profile, &expected, &tokenizer(false), &anchor);
        if extra == 0 {
            assert!(matches!(result, Err(JournalError::Contract(Error::Limit))));
            assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), original);
        } else { assert_eq!(result.unwrap().0.revision(), (count + 1) as u64); }
    }
}

#[test]
fn text_anchor_all_storage_barriers_return_no_candidate_owner_and_recover_actual_cut() {
    for barrier in BARRIERS {
        let root = Directory::new(); let c = config(3.0, 65);
        let (mut host, guards) = fresh(&root, &c, host_profile());
        let intent = command(&host, 7, request(b"ab", 2));
        host.generate_decoder_text(host.revision(), intent).unwrap();
        let expected = requirements(&host, guards); let anchor = host.history_anchor().unwrap();
        let revision = host.revision(); let original = bytes(&host); drop(host);
        let store = storage::Store::open(&root.store()).unwrap(); store.fail_once(barrier);
        let canonical = tokenizer_bytes(&expected, &tokenizer(false)).unwrap();
        let error = FileOversight::open_guarded_text_store(store, host_profile(), &expected, &canonical, &anchor).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("selected native replacement barrier"); };
        assert_eq!(failure.operation, barrier);
        let disk = FileOversight::read_publication(root.store(), &host_profile()).unwrap();
        assert_eq!(disk.revision, revision + u64::from(barrier == JournalIo::DirectorySync));
        if barrier != JournalIo::DirectorySync {
            assert_eq!(std::fs::read(root.store().join(storage::CANONICAL)).unwrap(), original);
        }
        let (host, roles) = reopen(&root, &expected, &anchor).unwrap();
        assert_eq!(host.revision(), disk.revision + 1); assert!(roles.policy_governor.is_some());
        assert!(!host.clock_ready()); assert!(host.decoder_inspection().unwrap().paused);
        assert_eq!(host.decoder_text_generation(7).unwrap().result().unwrap().bytes().unwrap(), b"AA");
        assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 2);
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn text_anchor_preserves_split_utf8_and_cannot_replenish_an_exhausted_request() {
    let root = Directory::new(); let c = split_utf8_config();
    let (mut host, guards) = fresh(&root, &c, host_profile());
    let mut input = request(b"ab", 3); input.generation.sampling_entries = 259;
    let intent = command(&host, 7, input);
    let before = host.generate_decoder_text(host.revision(), intent).unwrap();
    assert_eq!(before.result().unwrap().bytes().unwrap(), &[0xc3]);
    assert!(before.result().unwrap().utf8().is_err());
    assert_eq!(before.result().unwrap().generation().finish(), GenerationFinish::BudgetExhausted);
    let expected = requirements(&host, guards); let anchor = host.history_anchor().unwrap(); drop(host);
    let (mut host, _) = reopen(&root, &expected, &anchor).unwrap();
    let restored = host.decoder_text_generation(7).unwrap();
    assert_eq!(restored.result().unwrap().bytes().unwrap(), &[0xc3]);
    assert_eq!(restored.result().unwrap().generation().work(), before.result().unwrap().generation().work());
    assert_eq!(host.decoder_inspection().unwrap().numerical.sampled_draws, 1);
    let original = bytes(&host);
    let mut wider = restored.command().request().clone(); wider.generation.sampling_entries = MAX_SAMPLING_ENTRIES;
    let replacement = FileTextGenerationCommand::new(7, restored.command().actor_revision(),
        restored.command().position(), wider).unwrap();
    assert!(matches!(host.generate_decoder_text(host.revision(), replacement), Err(JournalError::Contract(Error::Binding))));
    assert_eq!(bytes(&host), original);
}
