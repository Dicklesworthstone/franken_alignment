//! Catch-up retains ORIGINAL durable source versions, leases and replacements.
use super::Task;
use super::super::{FileLearnedConfig, FileOversight, FileOversightProfile, FileHumanReviewer,
    FileLearnedStepPreparationStatus, Error};
use crate::action::{ActionSpec, ElapsedTick, ResolvedTarget, VERSION};
use crate::action::consequence::delivery::persistent::requests::actor::{FileActorSupervisor, LearnedTextProposal};
use crate::action::consequence::delivery::persistent::observed::source::{FileSourcePolicy, FileSourceError, FileSourceReplacement};
use crate::action::consequence::oversight::policy_state::{StateSource, StateLimits, StateFreshness};
use crate::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use crate::action::consequence::oversight::learned_source::LearnedEvidenceLimits;
use crate::Snapshot;

#[path = "../../config/text/intake/tests/fixture.rs"]
mod fixture;
use fixture::*;

fn policy() -> FileSourcePolicy {
    FileSourcePolicy { source: StateSource { scope: profile().delivery.scope, source: 42, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(5).unwrap() }
}
fn configured() -> FileLearnedConfig { config().with_required_policy_source(policy()).unwrap() }
fn early(root: &Directory, config: &FileLearnedConfig) -> FileOversight {
    let (mut host, _) = FileOversight::create_with_learned_text(root.store(), profile(), config.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); step(&mut host); host
}
fn capture(generation: u64, complete: bool) -> EvidenceSnapshot {
    let mut state = snapshot(); state.complete = complete;
    EvidenceSnapshot::new(EvidenceIdentity { source: 42, generation, scope: profile().delivery.scope },
        state, ["alpha", "beta"].into_iter().map(|member| (member.to_owned(), Vec::new())).collect()).unwrap()
}
fn refresh(host: &mut FileOversight, root: &Directory, value: &EvidenceSnapshot, tick: u64)
    -> Result<(), FileSourceError>
{
    let path = root.store().with_extension("policy"); let stage = path.with_extension("pending");
    std::fs::write(&stage, value.encode()).unwrap(); std::fs::rename(stage, &path).unwrap();
    // A new concrete reader cannot supply a missing version floor. The existing
    // journal and original leased capture remain responsible for admissibility.
    let mut source = FileEvidenceSource::new(path, 42, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(tick)).map(|_| ())
}
fn bytes(host: &FileOversight) -> Vec<u8> {
    host.store.read(host.profile.delivery.limits.bytes).unwrap()
}

#[test]
fn live_policy_refreshes_between_both_stages_match_original_synchronous_state_and_work() {
    let root = Directory::new(); let other = Directory::new(); let config = configured();
    let mut host = early(&root, &config); let mut control = early(&other, &config);
    refresh(&mut host, &root, &capture(1, true), 1).unwrap();
    refresh(&mut control, &other, &capture(1, true), 1).unwrap();
    for completion in [false, true] {
        let mut task = Task::new(&mut host, completion); task.ready(&host);
        let n = control.learned_generation_inspection().unwrap().numerical;
        let tick = if completion { 3 } else { 2 };
        refresh(&mut host, &root, &capture(2, true), tick).unwrap();
        refresh(&mut control, &other, &capture(2, true), tick).unwrap();
        let cursor = task.progress().replayed_events; let old_bytes = bytes(&host);
        let source_state = host.file_source_status().unwrap();
        let progress = task.catch_up(&host, host.revision(), cursor, 1).unwrap();
        assert_eq!(progress.replayed_events, cursor + 1);
        assert_eq!(progress.status, FileLearnedStepPreparationStatus::Ready);
        assert_eq!(bytes(&host), old_bytes);
        let actual = task.finish(&mut host).unwrap();
        if completion {
            let expected = control.complete_learned_step(control.revision(), n.actor_revision, n.position).unwrap().unwrap();
            let actual = actual.unwrap(); assert_eq!(actual.status(), expected.status()); assert_eq!(actual.sample(), expected.sample());
        } else {
            assert!(actual.is_none()); control.begin_learned_step(control.revision(), n.actor_revision, n.position).unwrap();
        }
        assert_eq!(host.file_source_status().unwrap(), source_state);
        assert_eq!(host.file_source_status(), control.file_source_status());
        assert_eq!(host.learned_generation_inspection().unwrap().numerical, control.learned_generation_inspection().unwrap().numerical);
        assert_eq!(host.machine.broker.retained_actor_state().cache(), control.machine.broker.retained_actor_state().cache());
        assert_eq!(host.machine.broker.retained_actor_state().sampler(), control.machine.broker.retained_actor_state().sampler());
        assert_eq!(host.inspect().executions, 0);
    }
    drop(host);
    let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture(2, true).identity()));
    assert!(host.machine.broker.capture_policy_state().is_err());
    assert!(host.learned_generation_inspection().unwrap().paused);
}

#[test]
fn source_replacement_tail_keeps_new_generation_epoch_receipt_and_producer_floor() {
    let root = Directory::new(); let config = configured(); let mut host = early(&root, &config);
    refresh(&mut host, &root, &capture(7, true), 1).unwrap();
    let mut task = Task::new(&mut host, true); task.ready(&host);
    let epoch = host.inspect().control.ledger.epoch; let cursor = task.progress().replayed_events;
    let request = FileSourceReplacement { operation: 991, expected_generation: 1,
        expected_authority_epoch: epoch, next_generation: 2 };
    let replaced = host.replace_file_source(host.revision(), request).unwrap();
    refresh(&mut host, &root, &capture(8, true), 2).unwrap();
    let current = host.file_source_status().unwrap(); let current_epoch = host.inspect().control.ledger.epoch;
    assert!(current_epoch > epoch); assert_eq!(current.policy.source.generation, 2);
    let first = task.catch_up(&host, host.revision(), cursor, 1).unwrap();
    assert_eq!(first.status, FileLearnedStepPreparationStatus::Replaying);
    let second = task.catch_up(&host, host.revision(), first.replayed_events, 1).unwrap();
    assert_eq!(second.status, FileLearnedStepPreparationStatus::Ready);
    task.finish(&mut host).unwrap();
    assert_eq!(host.file_source_status().unwrap(), current);
    assert_eq!(host.inspect().control.ledger.epoch, current_epoch);
    assert_eq!(host.file_source_replacement(991).unwrap(), replaced);
    assert_eq!(host.inspect().executions, 0); drop(host);
    let (mut host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
    assert_eq!(host.file_source_status().unwrap().policy.source.generation, 2);
    assert_eq!(host.file_source_status().unwrap().producer, Some(capture(8, true).identity()));
    assert_eq!(host.file_source_replacement(991).unwrap(), replaced);
    assert_eq!(refresh(&mut host, &root, &capture(7, true), 3), Err(FileSourceError::Refused(Error::Stale)));
    assert!(host.learned_generation_inspection().unwrap().paused);
}

#[test]
fn committed_policy_refusal_stays_nonpermitting_after_allowed_numerical_completion() {
    for completion in [false, true] {
        let root = Directory::new(); let config = configured(); let mut host = early(&root, &config);
        refresh(&mut host, &root, &capture(1, true), 1).unwrap();
        let mut task = Task::new(&mut host, completion); task.ready(&host);
        let cursor = task.progress().replayed_events;
        assert_eq!(refresh(&mut host, &root, &capture(2, false), 2), Err(FileSourceError::Refused(Error::Incomplete)));
        let refused = host.file_source_status().unwrap();
        task.catch_up(&host, host.revision(), cursor, 1).unwrap(); task.finish(&mut host).unwrap();
        assert_eq!(host.file_source_status().unwrap(), refused);
        // Numerical work is not permission: finish the real generated message,
        // then prove that even its exact bytes and a good copied snapshot refuse.
        while host.learned_generation_inspection().unwrap().numerical.status.is_active() { step(&mut host); }
        assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"aa");
        let control_root = Directory::new(); let (reference, _) = owner(&control_root, &config);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical,
            reference.learned_generation_inspection().unwrap().numerical);
        assert!(host.machine.broker.capture_policy_state().is_err());
        let spec = ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(host.inspect().target),
            payload: b"aa".to_vec(), required_witnesses: Vec::new(), policy_epoch: host.inspect().control.ledger.epoch,
            deadline: ElapsedTick(100), units: 2 };
        assert!(host.propose(host.revision(), 1, spec.clone(), snapshot()).is_err());
        assert_eq!(host.file_source_status().unwrap().producer, Some(capture(2, false).identity()));
        refresh(&mut host, &root, &capture(3, true), 3).unwrap();
        host.propose(host.revision(), 1, spec, snapshot()).unwrap();
        assert_eq!(host.inspect().executions, 0, "fresh intake still grants neither publication key");
        let (port, mut supervisor) = host.into_learned_text_actor_gateway().unwrap();
        observe(&mut supervisor, snapshot());
        let ticket = port.submit(71, proposal()).unwrap();
        assert!(matches!(port.poll(&ticket),
            crate::action::consequence::oversight::actor::Knowledge::Pending { request: 71 }));
        assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    }
}
