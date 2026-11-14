//! Original numerical recovery with independently selected campaign custody.
//! Fixture weights exercise replay/role/Stop contracts, not policy efficacy or
//! the authentication of an independently operated deployment.
use super::*;
use super::continuation as native;
use super::super::anchors;
use fa_reference::action::consequence::{
    delivery::persistent::{
        FileDeliverySnapshot,
        observed::{
            decoder::learned::FileLearnedRecoveryStatus,
            guarded::FileCampaignRequirement,
        },
    },
    gate::containment::session::policy::{Policy, Predicate},
    oversight::learned_host::HostedLearnedInspection,
    policy_campaign::ReplayLimits,
};
use std::path::PathBuf;

struct Case {
    root: Root,
    config: Config,
    loaded: Loaded,
    before: FileDeliverySnapshot,
    numerical: HostedLearnedInspection,
    anchor: PathBuf,
    anchor_bytes: Vec<u8>,
}

fn requirement() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits { cases: 16, input_bytes: 1_048_576 },
        max_campaigns: 8,
    }
}

fn prepared() -> Case {
    let root = Root::new();
    let mut config = configured(&root);
    let path = native::write(&root);
    evidence(&root, &config, 1, true, false);
    let mut loaded = recipe::load(&path, &config, true).unwrap();
    loaded.expected_campaigns = Some(requirement());
    loaded.creation_campaigns = Some(requirement());
    let ((host, reviewer, observer, _identity), _original_governor) =
        recovery::create_governed(&config, &loaded).unwrap();
    assert!(observer.is_none());
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline(), &mut || ElapsedTick(1000)).unwrap());
    let (before, numerical, original_anchor) = {
        let host = driver.supervisor().host().unwrap();
        (host.inspect(), host.learned_generation_inspection().unwrap().numerical,
            host.history_anchor().unwrap())
    };
    assert_eq!(numerical.sampled_draws, 2);
    assert!(numerical.work.accepted_decoder.scalar_products().unwrap() > 0);
    drop(port);
    assert_eq!(cleanup(driver, 1000, 1), 0);
    let anchor = root.0.join("governed-recovery-before.anchor");
    anchors::export_acknowledged(&original_anchor, &anchor).unwrap();
    let anchor_bytes = fs::read(&anchor).unwrap();
    loaded.creation_campaigns = None;
    loaded.recovery_anchor = Some(anchors::load(&anchor).unwrap());
    loaded.anchor_output = Some(root.0.join("governed-recovery-after.anchor"));
    loaded.floor.journal_revision = before.revision;
    loaded.floor.control_sequence = before.control.sequence;
    loaded.floor.authority_epoch = before.control.ledger.epoch;
    fs::write(config.store.join("delivery.pending"), b"unacknowledged governed staging").unwrap();
    // Recovery must retain the numerical recipe already admitted by its caller;
    // it cannot silently refresh policy evidence to improve its old eligibility.
    fs::remove_file(root.0.join("evidence.json")).unwrap();
    Case { root, config, loaded, before, numerical, anchor, anchor_bytes }
}

fn assert_original_inputs(case: &Case, reads: u64) {
    assert_eq!(fs::read(&case.anchor).unwrap(), case.anchor_bytes);
    assert_eq!(case.config.source.status().read_attempts, reads);
    assert!(!case.root.0.join("evidence.json").exists());
}

#[test]
fn governed_live_recovery_preserves_original_numerics_and_one_fresh_fence() {
    let case = prepared();
    let reads = case.config.source.status().read_attempts;
    let mut control = Control::new(&case.config, case.loaded.request, None).unwrap();
    let mut calls = 0;
    let ((host, _human, prediction, _identity), _fresh_governor) =
        recovery::open_governed_live(&case.config, &case.loaded, &mut control,
            &deadline(), &mut || { calls += 1; ElapsedTick(1000) }).unwrap();
    assert_eq!(calls, case.before.revision as usize + 2);
    assert_eq!(host.revision(), case.before.revision + 1);
    assert_eq!(host.current_policy().unwrap(), &case.loaded.expected_policy);
    assert!(host.policy_campaigns_required());
    let recovered = host.learned_generation_inspection().unwrap();
    assert_eq!(recovered.numerical, case.numerical);
    assert!(recovered.paused);
    assert!(!host.clock_ready());
    assert!(prediction.is_none());
    assert!(host.inspect().stop.is_none());
    assert_eq!(host.inspect().executions, case.before.executions);
    assert_eq!(host.inspect().control.ledger.charged, case.before.control.ledger.charged);
    assert!(!case.config.store.join("delivery.pending").exists());
    assert!(!case.loaded.anchor_output.as_ref().unwrap().exists(),
        "the live caller retains the final owner; recovery must not freeze an early output cut");
    assert_original_inputs(&case, reads);
}

#[test]
fn governed_deadline_at_every_event_and_ready_preserves_canonical_and_staged_history() {
    let case = prepared();
    let original = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let staged = fs::read(case.config.store.join("delivery.pending")).unwrap();
    let reads = case.config.source.status().read_attempts;
    let total = case.before.revision as usize;
    let mut control = Control::new(&case.config, case.loaded.request, None).unwrap();
    for expire_at in 0..=total + 1 {
        let limit = deadline();
        let mut calls = 0;
        let result = recovery::open_governed_live(&case.config, &case.loaded,
            &mut control, &limit, &mut || {
                let at = calls;
                calls += 1;
                if at == expire_at { limit.logical } else { ElapsedTick(1000) }
            });
        assert_eq!(result.err().as_deref(), Some("one-shot workflow deadline reached"));
        assert_eq!(calls, expire_at + 1);
        assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), original);
        assert_eq!(fs::read(case.config.store.join("delivery.pending")).unwrap(), staged);
        assert_original_inputs(&case, reads);
    }
    let mut progress = Vec::new();
    let ((host, _, _, _), _governor) = recovery::open_governed_with_checkpoints(
        &case.config, &case.loaded, &mut |view| { progress.push(view); Ok(()) }).unwrap();
    assert_eq!(progress[0], None);
    assert_eq!(progress.len(), total + 2);
    let ready = progress.last().unwrap().unwrap();
    assert_eq!(ready.status, FileLearnedRecoveryStatus::Ready);
    assert_eq!(ready.replayed_events, total);
    assert_eq!(host.revision(), case.before.revision + 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, case.numerical);
}

#[test]
fn governed_selection_refuses_missing_custody_or_creation_before_any_replay_checkpoint() {
    let mut case = prepared();
    let original = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let staged = fs::read(case.config.store.join("delivery.pending")).unwrap();
    let anchor = case.loaded.recovery_anchor.take().unwrap();
    let refuse = |loaded: &Loaded| {
        assert!(recovery::open_governed_with_checkpoints(&case.config, loaded,
            &mut |_| panic!("unsupported selection must precede replay and time")).is_err());
        assert_eq!(fs::read(case.config.store.join("delivery.bin")).unwrap(), original);
        assert_eq!(fs::read(case.config.store.join("delivery.pending")).unwrap(), staged);
    };
    refuse(&case.loaded);
    case.loaded.recovery_anchor = Some(anchor);
    case.loaded.expected_campaigns = None;
    refuse(&case.loaded);
    case.loaded.expected_campaigns = Some(requirement());
    case.loaded.creation_campaigns = Some(requirement());
    refuse(&case.loaded);
    case.loaded.creation_campaigns = None;
    assert!(recovery::open_governed_with_checkpoints(&case.config, &case.loaded,
        &mut |_| Ok(())).is_ok());
}

#[test]
fn governed_recovery_checks_full_policy_campaign_source_and_floor_before_cleanup() {
    let mut case = prepared();
    let original = fs::read(case.config.store.join("delivery.bin")).unwrap();
    let staged = fs::read(case.config.store.join("delivery.pending")).unwrap();
    let refuse = |config: &Config, loaded: &Loaded| {
        let mut ready = false;
        assert!(recovery::open_governed_with_checkpoints(config, loaded, &mut |view| {
            ready |= view.is_some_and(|progress| progress.status == FileLearnedRecoveryStatus::Ready);
            Ok(())
        }).is_err());
        assert!(!ready, "a mismatched complete guard cannot reach Ready");
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), original);
        assert_eq!(fs::read(config.store.join("delivery.pending")).unwrap(), staged);
    };
    let policy = case.loaded.expected_policy.clone();
    case.loaded.expected_policy = Policy::new(policy.generation(),
        vec![Predicate::PayloadAtMost(511)]).unwrap();
    refuse(&case.config, &case.loaded);
    case.loaded.expected_policy = policy;
    for changed in [
        FileCampaignRequirement { limits: ReplayLimits { cases: 17, ..requirement().limits }, ..requirement() },
        FileCampaignRequirement { limits: ReplayLimits { input_bytes: 1_048_577, ..requirement().limits }, ..requirement() },
        FileCampaignRequirement { max_campaigns: 9, ..requirement() },
    ] {
        case.loaded.expected_campaigns = Some(changed);
        refuse(&case.config, &case.loaded);
    }
    case.loaded.expected_campaigns = Some(requirement());
    let floor = case.loaded.floor;
    case.loaded.floor.journal_revision = u64::MAX;
    refuse(&case.config, &case.loaded);
    case.loaded.floor = floor;
    case.loaded.floor.control_sequence = u64::MAX;
    refuse(&case.config, &case.loaded);
    case.loaded.floor = floor;
    case.loaded.floor.authority_epoch = u64::MAX;
    refuse(&case.config, &case.loaded);
    case.loaded.floor = floor;
    let events = case.config.source_policy.limits.events;
    case.config.source_policy.limits.events += 1;
    refuse(&case.config, &case.loaded);
    case.config.source_policy.limits.events = events;
    let ((host, _, _, _), _governor) = recovery::open_governed_with_checkpoints(
        &case.config, &case.loaded, &mut |_| Ok(())).unwrap();
    assert_eq!(host.revision(), case.before.revision + 1);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, case.numerical);
}

#[cfg(target_os = "linux")]
#[path = "governed_tests/stop.rs"]
mod authenticated_stop;
