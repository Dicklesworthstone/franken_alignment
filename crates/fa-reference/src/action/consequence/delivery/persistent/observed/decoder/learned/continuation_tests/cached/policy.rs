//! Actual durable policy observations cannot be erased by the private cache.
use super::super::{FileOversight, FileOversightProfile, FileHumanReviewer, FileLearnedConfig, Error};
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, VERSION};
use crate::action::consequence::delivery::persistent::requests::actor::{FileActorSupervisor, LearnedTextProposal};
use crate::action::consequence::delivery::persistent::observed::source::{FileSourcePolicy, FileSourceReplacement, FileSourceError};
use crate::action::consequence::oversight::policy_state::{StateSource, StateLimits, StateFreshness};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::Snapshot;
#[path = "../../config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn configured() -> FileLearnedConfig {
    config().with_required_policy_source(FileSourcePolicy {
        source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap(),
    }).unwrap()
}
fn early(root: &Directory, config: &FileLearnedConfig) -> FileOversight {
    let (mut host, _) = FileOversight::create_with_learned_text(root.store(), profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host);
    assert!(super::prefix(&host).is_some()); host
}
fn capture(generation: u64, complete: bool) -> EvidenceSnapshot {
    let mut snapshot = snapshot(); snapshot.complete = complete;
    EvidenceSnapshot::new(EvidenceIdentity { source: 42, generation, scope: profile().delivery.scope },
        snapshot, ["alpha", "beta"].into_iter().map(|name| (name.to_owned(), Vec::new())).collect()).unwrap()
}
fn refresh(host: &mut FileOversight, root: &Directory, generation: u64, complete: bool, tick: u64)
    -> Result<(), FileSourceError>
{
    let path = root.store().with_extension("policy"); let staged = path.with_extension("pending");
    std::fs::write(&staged, capture(generation, complete).encode()).unwrap(); std::fs::rename(staged, &path).unwrap();
    // Reopening the real reader forces the ORIGINAL journal to supply its floor.
    let mut reader = FileEvidenceSource::new(path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(tick)).map(|_| ())
}

#[test]
fn cached_completion_retains_replaced_source_generation_epoch_and_durable_producer_floor() {
    let root = Directory::new(); let config = configured(); let mut host = early(&root, &config);
    refresh(&mut host, &root, 7, true, 1).unwrap();
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), n.actor_revision, n.position).unwrap();
    let prefix = super::prefix(&host).unwrap(); let epoch = host.inspect().control.ledger.epoch;
    let replacement = host.replace_file_source(host.revision(), FileSourceReplacement {
        operation: 991, expected_generation: 1, expected_authority_epoch: epoch, next_generation: 2,
    }).unwrap();
    refresh(&mut host, &root, 8, true, 2).unwrap();
    assert_eq!(super::prefix(&host), Some(prefix));
    let source = host.file_source_status().unwrap(); let floor = host.inspect().control.ledger.epoch;
    assert!(floor > epoch); assert_eq!(source.policy.source.generation, 2);
    host.complete_learned_step(host.revision(), n.actor_revision, n.position).unwrap().unwrap();
    assert_eq!(host.file_source_status().unwrap(), source); assert_eq!(host.inspect().control.ledger.epoch, floor);
    assert_eq!(host.file_source_replacement(991).unwrap(), replacement); assert_eq!(host.inspect().executions, 0);
    drop(host);
    let (mut recovered, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(super::prefix(&recovered), None);
    assert_eq!(recovered.file_source_status().unwrap().policy.source.generation, 2);
    assert_eq!(recovered.file_source_status().unwrap().producer, Some(capture(8, true).identity()));
    assert_eq!(recovered.file_source_replacement(991).unwrap(), replacement);
    assert_eq!(refresh(&mut recovered, &root, 7, true, 3), Err(FileSourceError::Refused(Error::Stale)));
    assert!(recovered.learned_generation_inspection().unwrap().paused);
    assert!(recovered.machine.broker.capture_policy_state().is_err());
}

#[test]
fn cached_generation_cannot_replace_refused_policy_with_its_older_complete_capture() {
    let root = Directory::new(); let config = configured(); let mut host = early(&root, &config);
    refresh(&mut host, &root, 1, true, 1).unwrap();
    assert_eq!(refresh(&mut host, &root, 2, false, 2), Err(FileSourceError::Refused(Error::Incomplete)));
    let refused = host.file_source_status().unwrap();
    while host.learned_generation_inspection().unwrap().numerical.status.is_active() {
        step(&mut host); assert_eq!(host.file_source_status().unwrap(), refused);
    }
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
    let other = Directory::new();
    let (mut control, _) = FileOversight::create_with_learned_text(other.store(), profile(), config.clone()).unwrap();
    control.observe_time(control.revision(), ElapsedTick(1)).unwrap();
    while control.learned_generation_inspection().unwrap().numerical.status.is_active() {
        super::super::cold(&mut control).unwrap();
    }
    assert_eq!(host.learned_generation_inspection().unwrap().numerical,
        control.learned_generation_inspection().unwrap().numerical);
    let spec = ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
        payload: b"aa".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100), units: 2 };
    assert!(host.propose(host.revision(), 1, spec.clone(), snapshot()).is_err());
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture(2, false).identity()));
    refresh(&mut host, &root, 3, true, 3).unwrap();
    host.propose(host.revision(), 1, spec, snapshot()).unwrap();
    assert_eq!(host.inspect().executions, 0);
    let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
    observe(&mut supervisor, snapshot()); let ticket = port.submit(71, proposal()).unwrap();
    assert!(matches!(port.poll(&ticket), crate::action::consequence::oversight::actor::Knowledge::Pending { request: 71 }));
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
}
