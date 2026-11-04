//! Exact native terminal confirmation after both reset and Stop exports are lost.
//! The surrounding fixture performs actual numerical work and real source reads.
use super::*;
use fa_reference::Error;
use fa_reference::action::consequence::delivery::{
    StopRequest,
    persistent::{
        JournalError,
        observed::{
            guarded::mediated::{
                learned::FileMediatedLearnedRecovery,
                stream_reset::FileStoppedStreamResetRecovery,
            },
            shutdown::recovery::FileAdmissionStoppedHistory,
        },
    },
};

fn expected(case: &SourceCase, saved: &TopologyCheckpoint) -> FileMediatedRequirements {
    let history = topology_custody::history(&case.case.loaded).unwrap().unwrap();
    let mut topology = history.requirement.clone();
    topology.current = saved.expected().graph().clone();
    topology.available = saved.available();
    FileMediatedRequirements {
        oversight: recovery::requirements(&case.case.config, &case.case.loaded),
        topology, prediction: None, evaluation: None,
    }
}
fn raw(case: &SourceCase, saved: &TopologyCheckpoint, expected: &FileMediatedRequirements)
    -> Result<FileMediatedLearnedRecovery, JournalError>
{
    FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        &case.case.config.store, case.case.config.profile.clone(), expected,
        &case.case.loaded.generation, saved.history_anchor())
}
fn begin_with(case: &SourceCase, saved: &TopologyCheckpoint,
    request: &reset::Request, operation: u64, expected: &FileMediatedRequirements)
    -> Result<FileStoppedStreamResetRecovery, JournalError>
{
    raw(case, saved, expected)?.require_stopped_stream_reset_checkpoint(
        saved, &request.intent, request.first_incident_sequence, operation)
}
fn begin(case: &SourceCase, saved: &TopologyCheckpoint) -> FileStoppedStreamResetRecovery {
    begin_with(case, saved, &case.case.request, 900, &expected(case, saved)).unwrap()
}
fn drive(run: &mut FileStoppedStreamResetRecovery) -> Result<(), JournalError> {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let done = run.progress().replayed_events;
        let next = run.advance(done, 1)?;
        assert_eq!(next.replayed_events, done + 1);
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    Ok(())
}
fn ready(case: &SourceCase, saved: &TopologyCheckpoint) -> FileStoppedStreamResetRecovery {
    let mut run = begin(case, saved);
    drive(&mut run).unwrap();
    run
}

/// Shared with the command regressions: original source/history exports stay at
/// BeginStreamReset while native Stop900 and its Fence are really acknowledged.
/// If completed is true, the original numerical reset and Fence precede Stop.
/// No successor archive is written and no live source evidence is synthesized.
pub(super) fn stopped_case(root: &Root, completed: bool) -> SourceCase {
    let case = prepared(root, false);
    let mut requirements = expected(&case, &case.saved);
    if completed {
        let (host, _roles) = native_ready(&case).finish_pending_stream_reset().unwrap();
        assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
        assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
        assert!(!host.mediation_snapshot().unwrap().available);
        requirements.topology.available = false;
        drop(host);
    }
    // This creates the fixture's first Stop through the existing complete
    // mediated cursor. The NEW repair capability below cannot create that Stop.
    let mut run = raw(&case, &case.saved, &requirements).unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal()).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let request = run.stop_request(900).unwrap();
    let stopped = run.finish_admission_stop(request).unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(stopped.history_anchor().revision(),
        case.case.before.revision + if completed { 4 } else { 2 });
    assert_eq!(stopped.snapshot().stop.as_ref().unwrap().request(), request);
    drop(stopped);
    case
}

fn successor(case: &SourceCase, saved: &TopologyCheckpoint, stopped: &FileAdmissionStoppedHistory)
    -> TopologyCheckpoint
{
    saved.after_stream_reset_admission_stop(&case.case.config.profile,
        &fs::canonicalize(&case.case.config.store).unwrap(), stopped,
        &case.case.request.intent, case.case.request.first_incident_sequence).unwrap()
}
fn staging(case: &SourceCase) -> PathBuf {
    let path = case.case.config.store.join("delivery.pending");
    fs::write(&path, b"confirm existing stopped history before cleanup").unwrap();
    path
}

#[test]
fn native_compound_stop_confirmation_repairs_old_and_new_pairs_without_another_event() {
    for completed in [false, true] {
        let root = Root::new();
        let case = stopped_case(&root, completed);
        let original_source = fs::read(root.0.join("source-1.bin")).unwrap();
        let canonical = disk(&case.case);
        remove_live_inputs(&root);
        let path = staging(&case);
        drop(ready(&case, &case.saved));
        assert_eq!(disk(&case.case), canonical);
        assert!(path.is_file(), "Ready/drop must not confirm or clean staging");
        let stopped = ready(&case, &case.saved).finish_admission_stop().unwrap();
        assert_eq!(stopped.appended_events(), 0);
        assert_eq!(stopped.history_anchor().revision(),
            case.case.before.revision + if completed { 4 } else { 2 });
        assert_eq!(stopped.snapshot().executions, 0);
        assert_eq!(stopped.snapshot().control.ledger.charged,
            case.case.before.control.ledger.charged);
        assert_eq!(stopped.snapshot().control.ledger.elapsed,
            case.case.before.control.ledger.elapsed);
        assert_eq!(disk(&case.case), canonical);
        assert!(!path.exists(), "native exact confirmation legitimately cleans staging");
        let next = successor(&case, &case.saved, &stopped);
        unchanged_source(&case.saved, &next);
        assert_eq!(next.history_anchor(), stopped.history_anchor());
        let mut bytes = Vec::new();
        next.write_to(&mut bytes, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
        let imported = TopologyCheckpoint::read_trusted(
            &mut Cursor::new(bytes), MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
        assert_eq!(imported, next);
        let retry = ready(&case, &imported).finish_admission_stop().unwrap();
        assert_eq!(retry.appended_events(), 0);
        assert_eq!(retry.history_anchor(), stopped.history_anchor());
        assert_eq!(retry.snapshot(), stopped.snapshot());
        assert_eq!(successor(&case, &imported, &retry), next);
        assert_eq!(disk(&case.case), canonical);
        assert_eq!(fs::read(root.0.join("source-1.bin")).unwrap(), original_source);
        let record = FileOversight::read_learned_reset_result(&case.case.config.store,
            &case.case.config.profile, &case.case.loaded.generation, 900).unwrap();
        match (completed, record.record()) {
            (false, FileLearnedResetRecord::Pending(pending)) => {
                assert!(pending.interrupted);
                assert_eq!(pending.intent, case.case.request.intent);
            }
            (true, FileLearnedResetRecord::Completed { intent, result: Ok(receipt) }) => {
                assert_eq!(intent, &case.case.request.intent);
                assert!(receipt.control.restored);
                assert_eq!(receipt.control.incident_count, 1);
            }
            other => panic!("terminal confirmation changed reset semantics: {other:?}"),
        }
    }
}

#[test]
fn stopped_reset_repair_cannot_create_a_missing_stop_or_change_its_bound_request() {
    for completed in [false, true] {
        let root = Root::new();
        let case = prepared(&root, false);
        if completed {
            let (host, _roles) = native_ready(&case).finish_pending_stream_reset().unwrap();
            drop(host);
        }
        let canonical = disk(&case.case);
        let path = staging(&case);
        assert_eq!(begin_with(&case, &case.saved, &case.case.request, 900,
            &expected(&case, &case.saved)).err(), Some(Error::Missing.into()));
        assert_eq!(disk(&case.case), canonical);
        assert!(path.is_file());
    }
    let root = Root::new();
    let case = stopped_case(&root, true);
    let canonical = disk(&case.case);
    let path = staging(&case);
    for operation in [0, 901] {
        assert!(begin_with(&case, &case.saved, &case.case.request, operation,
            &expected(&case, &case.saved)).is_err());
    }
    for field in 0..3 {
        let mut request = case.case.request.clone();
        match field {
            0 => request.first_incident_sequence += 1,
            1 => {
                let mut control = request.intent.control().clone();
                control.expected_control_sequence += 1;
                request.intent = FileLearnedResetIntent::for_recovery(
                    request.intent.checkpoint(), control, request.intent.budget()).unwrap();
            }
            _ => {
                let mut budget = request.intent.budget();
                budget.cache_values -= 1;
                request.intent = FileLearnedResetIntent::for_recovery(
                    request.intent.checkpoint(), request.intent.control().clone(), budget).unwrap();
            }
        }
        assert!(begin_with(&case, &case.saved, &request, 900,
            &expected(&case, &case.saved)).is_err());
    }
    assert!(begin_with(&case, &case.before_intent, &case.case.request, 900,
        &expected(&case, &case.before_intent)).is_err());
    assert_eq!(disk(&case.case), canonical);
    assert!(path.is_file());
}

#[test]
fn terminal_reset_repair_preserves_independent_floors_policy_and_original_replay_phase() {
    use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    let root = Root::new();
    let case = stopped_case(&root, true);
    let canonical = disk(&case.case);
    let path = staging(&case);
    for field in 0..5 {
        let mut wrong = expected(&case, &case.saved);
        match field {
            0 => wrong.oversight.minimum.journal_revision = u64::MAX,
            1 => wrong.oversight.minimum.control_sequence = u64::MAX,
            2 => wrong.oversight.minimum.authority_epoch = u64::MAX,
            3 => wrong.oversight.effective_policy =
                Policy::new(999, vec![Predicate::PayloadAtMost(1)]).unwrap(),
            _ => wrong.topology.available = !case.saved.available(),
        }
        if let Ok(mut run) = begin_with(&case, &case.saved, &case.case.request, 900, &wrong) {
            assert!(drive(&mut run).is_err(), "independent requirement {field}");
            assert!(run.finish_admission_stop().is_err());
        }
        assert_eq!(disk(&case.case), canonical);
        assert!(path.is_file());
    }
    assert!(begin(&case, &case.saved).finish_admission_stop().is_err());
    let mut partial = begin(&case, &case.saved);
    partial.advance(0, 1).unwrap();
    assert!(partial.finish_admission_stop().is_err());
    let mut original = raw(&case, &case.saved, &expected(&case, &case.saved)).unwrap();
    original.advance(0, 1).unwrap();
    assert_eq!(original.require_stopped_stream_reset_checkpoint(&case.saved,
        &case.case.request.intent, case.case.request.first_incident_sequence, 900).err(),
        Some(Error::WrongState.into()));
    assert_eq!(disk(&case.case), canonical);
    assert!(path.is_file());
}

#[test]
fn exact_terminal_reset_confirmation_rereads_canonical_bytes_and_requires_cleanup() {
    let root = Root::new();
    let case = stopped_case(&root, true);
    let canonical = disk(&case.case);
    let path = staging(&case);
    let run = ready(&case, &case.saved);
    let mut changed = canonical.clone();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    fs::write(case.case.config.store.join("delivery.bin"), &changed).unwrap();
    assert_eq!(run.finish_admission_stop().err(), Some(Error::Binding.into()));
    assert_eq!(disk(&case.case), changed);
    assert!(path.is_file());
    fs::write(case.case.config.store.join("delivery.bin"), &canonical).unwrap();
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert_eq!(ready(&case, &case.saved).finish_admission_stop().err(),
        Some(JournalError::InvalidFile));
    assert_eq!(disk(&case.case), canonical);
    assert!(path.is_dir());
    fs::remove_dir(&path).unwrap();
    staging(&case);
    let stopped = ready(&case, &case.saved).finish_admission_stop().unwrap();
    assert_eq!(stopped.appended_events(), 0);
    assert_eq!(disk(&case.case), canonical);
    assert!(!path.exists());
}

#[test]
fn an_unrelated_time_event_after_native_stop_cannot_repair_older_source_custody() {
    let root = Root::new();
    let case = prepared(&root, false);
    let (mut host, _roles) = native_ready(&case).finish_pending_stream_reset().unwrap();
    let state = host.inspect();
    host.request_stop(host.revision(), StopRequest { operation: 900,
        expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch }).unwrap();
    host.fence(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1001)).unwrap();
    drop(host);
    let canonical = disk(&case.case);
    let path = staging(&case);
    assert_eq!(begin_with(&case, &case.saved, &case.case.request, 900,
        &expected(&case, &case.saved)).err(), Some(Error::Stale.into()));
    assert_eq!(disk(&case.case), canonical);
    assert!(path.is_file());
}
