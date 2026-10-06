//! Real file observations, original producer floors, and original learned output.
use crate::action::consequence::delivery::persistent::observed::{FileOversight,
    FileOversightProfile, FileHumanReviewer, machine::Machine, journal,
    decoder::learned::{FileLearnedConfig, FileLearnedReplayContinuation, bind_history}};
use crate::Error;
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, VERSION};
use crate::action::consequence::delivery::persistent::{JournalError, JournalIo,
    requests::actor::{FileActorSupervisor, LearnedTextProposal}};
use crate::action::consequence::delivery::persistent::observed::source::{FileSourcePolicy, FileSourceError};
use crate::action::consequence::oversight::policy_state::{StateSource, StateLimits, StateFreshness};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot,
    FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::Snapshot;
#[path = "../../../config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn prefix(host: &FileOversight) -> Option<usize> {
    host.learned_replay.as_ref().map(FileLearnedReplayContinuation::verified_events)
}
fn current_predecessor(host: &FileOversight) {
    assert_eq!(prefix(host), Some(host.events.len() - 1));
}
fn original(host: &FileOversight, config: &FileLearnedConfig) {
    let bytes = host.store.read(host.profile.delivery.limits.bytes).unwrap();
    let mut events = journal::decode(&host.profile, host.store.identity(), &bytes).unwrap();
    bind_history(&mut events, config).unwrap();
    let cold = Machine::replay(&host.profile, &events).unwrap();
    assert_eq!(cold.snapshot(events.len()), host.inspect());
    assert_eq!(cold.broker.hosted_learned_generation().unwrap(),
        host.learned_generation_inspection().unwrap().numerical);
    assert_eq!(cold.file_source_status(), host.machine.file_source_status());
}

fn configured() -> FileLearnedConfig {
    config().with_required_policy_source(FileSourcePolicy {
        source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap(),
    }).unwrap()
}
fn capture(generation: u64, complete: bool) -> EvidenceSnapshot {
    let mut snapshot = snapshot(); snapshot.complete = complete;
    EvidenceSnapshot::new(EvidenceIdentity { source: 42, generation, scope: profile().delivery.scope },
        snapshot, ["alpha", "beta"].into_iter().map(|name| (name.to_owned(), Vec::new())).collect()).unwrap()
}
fn refresh(host: &mut FileOversight, root: &Directory, generation: u64, complete: bool, now: u64)
    -> Result<(), FileSourceError>
{
    let path = root.store().with_extension("policy"); let staged = path.with_extension("pending");
    std::fs::write(&staged, capture(generation, complete).encode()).unwrap(); std::fs::rename(staged, &path).unwrap();
    let mut source = FileEvidenceSource::new(path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(now)).map(|_| ())
}

#[test]
fn repeated_policy_refreshes_retain_only_the_latest_predecessor_without_running_new_inference() {
    let root = Directory::new(); let config = configured(); let (mut host, _) = owner(&root, &config);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    for tick in [1, 2, 6, 9] {
        refresh(&mut host, &root, 7, true, tick).unwrap();
        current_predecessor(&host); original(&host, &config);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, numerical);
        assert!(host.machine.broker.capture_policy_state().is_ok());
    }
    host.observe_time(host.revision(), ElapsedTick(14)).unwrap();
    assert!(host.machine.broker.capture_policy_state().is_err(), "an earlier cached lease must not extend freshness");
    refresh(&mut host, &root, 7, true, 15).unwrap();
    assert!(host.machine.broker.capture_policy_state().is_ok());
    current_predecessor(&host); original(&host, &config);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn committed_refusal_is_cached_as_history_not_replaced_by_an_older_permitting_source() {
    let root = Directory::new(); let config = configured(); let (mut host, _) = owner(&root, &config);
    refresh(&mut host, &root, 1, true, 1).unwrap();
    assert_eq!(refresh(&mut host, &root, 2, false, 2), Err(FileSourceError::Refused(Error::Incomplete)));
    // Inner refusal was acknowledged. Its predecessor must replay the refusal
    // before any later transaction, not restore the previous complete source.
    current_predecessor(&host); original(&host, &config);
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture(2, false).identity()));
    assert!(host.machine.broker.capture_policy_state().is_err());
    let spec = ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"aa".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 2 };
    assert!(host.propose(host.revision(), 1, spec.clone(), snapshot()).is_err());
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture(2, false).identity()));
    assert_eq!(refresh(&mut host, &root, 1, true, 3), Err(FileSourceError::Refused(Error::Stale)));
    assert_eq!(host.inspect().executions, 0);
    refresh(&mut host, &root, 3, true, 4).unwrap();
    host.propose(host.revision(), 1, spec, snapshot()).unwrap();
    // The failed proposal consumed the optimization, not the original source
    // history. Preserve the existing rule that cold controls do not reseed it.
    assert_eq!(prefix(&host), None); original(&host, &config);
    assert_eq!(host.inspect().executions, 0, "admission is still neither publication key");
    drop(host);
    let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(prefix(&recovered), None);
    assert!(recovered.learned_generation_inspection().unwrap().paused);
    assert_eq!(refresh(&mut recovered, &root, 2, true, 5), Err(FileSourceError::Refused(Error::Stale)));
    assert_eq!(prefix(&recovered), None);
    assert!(recovered.machine.broker.capture_policy_state().is_err());
}

#[test]
fn failed_durable_policy_write_keeps_source_interruption_and_cannot_return_an_older_cache() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = configured(); let (mut host, _) = owner(&root, &config);
        refresh(&mut host, &root, 1, true, 1).unwrap(); current_predecessor(&host);
        let before = host.inspect(); host.store.fail_once(barrier);
        assert!(matches!(refresh(&mut host, &root, 2, false, 2), Err(FileSourceError::Journal(JournalError::Io(f)))
            if f.operation == barrier));
        assert!(host.source_interrupted); assert!(host.storage_failure().is_some());
        assert_eq!(prefix(&host), None); assert_eq!(host.inspect(), before);
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        assert_eq!(prefix(&host), None); assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(host.machine.broker.capture_policy_state().is_err());
        assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.available, 100);
    }
}
