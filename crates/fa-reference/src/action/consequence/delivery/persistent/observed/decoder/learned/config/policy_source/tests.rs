//! Original journal/capture/learned generator, not a substitute policy ledger.
//! Synthetic weights exercise mechanics, not the effectiveness of a detector.
use super::*;
use crate::action::{ElapsedTick, ResolvedTarget};
use crate::action::consequence::delivery::persistent::{JournalIo, JournalError,
    requests::actor::{FileActorSupervisor, LearnedTextProposal}};
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    FileOversightProfile, FileHumanReviewer, source::FileSourceError};
use crate::action::consequence::oversight::policy_state::{StateSource, StateLimits, StateFreshness};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot,
    FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::Snapshot;
use std::path::Path;

#[path = "../text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn policy() -> FileSourcePolicy {
    FileSourcePolicy { source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap() }
}
fn configured() -> FileLearnedConfig { config().with_required_policy_source(policy()).unwrap() }
fn capture(generation: u64) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 42, generation, scope: profile().delivery.scope },
        snapshot(), ["alpha", "beta"].into_iter().map(|member| (member.to_owned(), Vec::new())).collect()).unwrap()
}
fn publish(path: &Path, value: &EvidenceSnapshot) {
    let staged = path.with_extension("pending");
    std::fs::write(&staged, value.encode()).unwrap(); std::fs::rename(staged, path).unwrap();
}
fn reader(root: &Directory) -> FileEvidenceSource {
    FileEvidenceSource::new(root.store().with_extension("policy"), 42,
        profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap()
}
fn refresh(host: &mut FileOversight, source: &mut FileEvidenceSource, now: u64)
    -> Result<(), FileSourceError>
{
    host.refresh_file_source(host.revision(), source, ElapsedTick(now)).map(|_| ())
}

#[test]
fn required_policy_source_installs_the_original_capture_and_expiring_lease_without_running_inference() {
    let root = Directory::new(); let config = configured(); let (mut host, _) = owner(&root, &config);
    assert!(host.file_source_required() && host.policy_only_file_source_required());
    assert_eq!(config.required_policy_source(), Some(policy()));
    assert!(host.machine.broker.capture_policy_state().is_err());
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let mut source = reader(&root); publish(&root.store().with_extension("policy"), &capture(1));
    refresh(&mut host, &mut source, 1).unwrap();
    let original = host.machine.broker.capture_policy_state().unwrap();
    assert_eq!(original.snapshot(), &snapshot()); assert!(original.lease().is_some());
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture(1).identity()));
    host.observe_time(host.revision(), ElapsedTick(6)).unwrap();
    assert!(host.machine.broker.capture_policy_state().is_err(), "read-start lease expires at its boundary");
    refresh(&mut host, &mut source, 7).unwrap();
    let renewed = host.machine.broker.capture_policy_state().unwrap();
    assert_eq!(renewed.snapshot(), original.snapshot());
    assert_ne!(renewed.frontier(), original.frontier(), "fresh reads are not old closure relabels");
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn complete_recipe_binds_source_scope_identity_limits_and_freshness_before_recovery() {
    let root = Directory::new(); let expected = configured(); let (host, _) = owner(&root, &expected); drop(host);
    let weak = config();
    assert!(matches!(FileOversight::open_with_learned_generation(root.store(), profile(), &weak),
        Err(JournalError::Contract(Error::Binding))));
    for field in 0..5 {
        let mut changed = policy();
        match field {
            0 => changed.source.source += 1,
            1 => changed.source.scope.principal += 1,
            2 => changed.source.generation += 1,
            3 => changed.limits.events -= 1,
            _ => changed.freshness = StateFreshness::new(6).unwrap(),
        }
        let changed = config().with_required_policy_source(changed).unwrap();
        assert_ne!(changed, expected);
        assert!(matches!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &changed),
            Err(JournalError::Contract(Error::Binding))));
    }
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &expected).unwrap();
    assert!(host.policy_only_file_source_required());
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready()); assert!(host.machine.broker.capture_policy_state().is_err());
}

#[test]
fn rebooted_reader_cannot_erase_the_original_journals_producer_or_equal_version_byte_floor() {
    let root = Directory::new(); let config = configured(); let (mut host, _) = owner(&root, &config);
    let path = root.store().with_extension("policy"); let mut source = reader(&root);
    publish(&path, &capture(7)); refresh(&mut host, &mut source, 1).unwrap(); drop(host); drop(source);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture(7).identity()));
    assert!(host.machine.broker.capture_policy_state().is_err());
    publish(&path, &capture(6));
    assert_eq!(refresh(&mut host, &mut reader(&root), 2), Err(FileSourceError::Refused(Error::Stale)));
    let mut changed = snapshot(); changed.values.insert(7, b"substitute".to_vec());
    let changed = EvidenceSnapshot::new(capture(7).identity(), changed, capture(7).contexts().clone()).unwrap();
    publish(&path, &changed);
    assert_eq!(refresh(&mut host, &mut reader(&root), 2), Err(FileSourceError::Refused(Error::Binding)));
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture(7).identity()));
    publish(&path, &capture(8)); refresh(&mut host, &mut reader(&root), 3).unwrap();
    assert!(host.machine.broker.capture_policy_state().is_ok());
    // Refresh is not numerical resume, actor approval or an external effect.
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn refused_newer_policy_remains_a_durable_floor_instead_of_falling_back_to_older_complete_data() {
    for defect in 0..3 {
        let root = Directory::new(); let config = configured(); let (mut host, _) = owner(&root, &config);
        let path = root.store().with_extension("policy");
        publish(&path, &capture(1)); refresh(&mut host, &mut reader(&root), 1).unwrap();
        let mut state = snapshot(); let mut contexts = capture(2).contexts().clone();
        match defect {
            0 => state.complete = false,
            1 => { contexts.insert("alpha".to_owned(), b"replace learned helper evidence".to_vec()); }
            _ => { contexts.remove("beta"); }
        }
        let bad = EvidenceSnapshot::new(capture(2).identity(), state, contexts).unwrap();
        publish(&path, &bad);
        assert!(matches!(refresh(&mut host, &mut reader(&root), 2), Err(FileSourceError::Refused(_))));
        assert_eq!(host.file_source_status().unwrap().producer, Some(capture(2).identity()));
        assert!(host.machine.broker.capture_policy_state().is_err()); drop(host);
        let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        publish(&path, &capture(1));
        assert_eq!(refresh(&mut host, &mut reader(&root), 3), Err(FileSourceError::Refused(Error::Stale)));
        publish(&path, &capture(3)); refresh(&mut host, &mut reader(&root), 3).unwrap();
        assert_eq!(host.file_source_status().unwrap().producer, Some(capture(3).identity()));
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn semantic_epoch_rollback_is_rejected_even_when_the_producer_generation_increases() {
    let root = Directory::new(); let config = configured(); let (mut host, _) = owner(&root, &config);
    let path = root.store().with_extension("policy");
    let mut newer = snapshot(); newer.semantic_epoch = 2;
    let newer = EvidenceSnapshot::new(capture(1).identity(), newer, capture(1).contexts().clone()).unwrap();
    publish(&path, &newer); refresh(&mut host, &mut reader(&root), 1).unwrap(); drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    publish(&path, &capture(2));
    assert_eq!(refresh(&mut host, &mut reader(&root), 2), Err(FileSourceError::Refused(Error::Stale)));
    assert_eq!(host.file_source_status().unwrap().semantic_epoch, Some(2));
    assert!(host.machine.broker.capture_policy_state().is_err());
}

#[test]
fn full_context_source_keeps_its_original_contract_and_cannot_be_relabelled_as_policy_only() {
    let root = Directory::new(); let (mut host, _) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_file_source(host.revision(), policy()).unwrap();
    assert!(host.file_source_required()); assert!(!host.policy_only_file_source_required());
    let mut contexts = capture(1).contexts().clone(); contexts.insert("alpha".to_owned(), b"original context".to_vec());
    let full = EvidenceSnapshot::new(capture(1).identity(), snapshot(), contexts).unwrap();
    publish(&root.store().with_extension("policy"), &full);
    refresh(&mut host, &mut reader(&root), 1).unwrap();
    assert_eq!(host.machine.broker.capture_policy_state().unwrap().snapshot(), &snapshot());
    assert!(host.enable_learned_generation(host.revision(), configured()).is_err());
    assert!(!host.policy_only_file_source_required());
}

#[test]
fn policy_source_bootstrap_requires_original_sidecar_provenance_and_original_capacity_bounds() {
    let (model, tokenizer, source) = recipe();
    let unguarded = FileLearnedConfig::new_text(model, tokenizer, source,
        crate::action::consequence::oversight::decoder_monitoring::LearnedDecoderBindingLimits::default()).unwrap();
    assert!(matches!(unguarded.with_required_policy_source(policy()), Err(Error::Binding)));
    assert!(matches!(configured().with_required_policy_source(policy()), Err(Error::Duplicate)));
    let mut zero = policy(); zero.limits.events = 0;
    assert!(matches!(config().with_required_policy_source(zero), Err(Error::InvalidInput)));
    let mut over = policy(); over.limits.events += 1;
    assert!(matches!(config().with_required_policy_source(over), Err(Error::Limit)));
    let root = Directory::new(); let mut foreign = policy(); foreign.source.scope.principal += 1;
    let foreign = config().with_required_policy_source(foreign).unwrap();
    assert!(matches!(FileOversight::create_with_learned_text(root.store(), profile(), foreign),
        Err(JournalError::Contract(Error::Binding))));
}

#[test]
fn source_storage_faults_acknowledge_no_capture_and_recovery_never_revives_a_live_lease() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = configured(); let (mut host, _) = owner(&root, &config);
        let path = root.store().with_extension("policy"); publish(&path, &capture(1));
        host.store.fail_once(barrier);
        assert!(matches!(refresh(&mut host, &mut reader(&root), 1), Err(FileSourceError::Journal(_))));
        assert!(host.storage_failure().is_some()); drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert!(host.policy_only_file_source_required());
        assert!(host.machine.broker.capture_policy_state().is_err());
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    }
}
