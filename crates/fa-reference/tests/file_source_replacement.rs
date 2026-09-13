//! Public durable-source recovery, using real files and the original two-key host.
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::{Directory, Keys, MEMBERS, ROOT, profile, snapshot, spec, votes, window};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::source::{
    FileSourceError, FileSourcePolicy, FileSourceReplacement,
};
use fa_reference::action::consequence::oversight::evidence_source::{
    EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
};
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::{Error, Snapshot};
use std::fs;
use std::rc::Rc;

fn create(root: &Directory, limits: StateLimits) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.enable_file_source(host.revision(), FileSourcePolicy {
        source: StateSource { scope: profile().delivery.scope, source: 31, generation: 1 },
        limits, freshness: StateFreshness::new(10).unwrap(),
    }).unwrap();
    (host, reviewer)
}
fn evidence(generation: u64, state: Snapshot) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 31, generation, scope: profile().delivery.scope },
        state, MEMBERS.into_iter().map(|member| (member.to_owned(), b"complete context".to_vec())).collect()).unwrap()
}
fn refresh(host: &mut FileOversight, root: &Directory, value: &EvidenceSnapshot, tick: u64)
    -> Result<Rc<EvidenceSnapshot>, FileSourceError>
{
    let temporary = root.0.join("evidence.tmp"); let path = root.0.join("evidence.json");
    fs::write(&temporary, value.encode()).unwrap(); fs::rename(&temporary, &path).unwrap();
    // Deliberately create a new reader every time. The durable HOST must retain
    // rollback/equivocation floors independently of any reader's in-memory state.
    let mut reader = FileEvidenceSource::new(path, 31, profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.refresh_file_source(host.revision(), &mut reader, ElapsedTick(tick))
}
fn replacement(host: &FileOversight, operation: u64, next_generation: u64) -> FileSourceReplacement {
    FileSourceReplacement { operation, next_generation,
        expected_generation: host.file_source_status().unwrap().capture.source.generation,
        expected_authority_epoch: host.inspect().control.ledger.epoch }
}
fn ready(host: &mut FileOversight, reviewer: &FileHumanReviewer, id: u64, value: &EvidenceSnapshot) -> Keys {
    let state = value.snapshot().clone();
    let action = host.propose(host.revision(), id, spec(host, b"recovered publication"), state.clone()).unwrap();
    let inputs = value.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), id, host.input_revision(id).unwrap(), inputs.clone()).unwrap();
    host.begin_review(host.revision(), id, id + 100, ROOT, window(host), state.clone()).unwrap();
    votes(host, id + 100, fa_reference::round::Verdict::Allow);
    host.finish_review(host.revision(), id + 100, Some(&inputs), state.clone()).unwrap().unwrap();
    let automatic = host.authorize(host.revision(), id, &inputs, state).unwrap();
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(now.0 + 30)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human, request }
}
fn dispatch(host: &mut FileOversight, keys: &Keys, value: &EvidenceSnapshot) {
    host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action,
        &keys.inputs, value.snapshot().clone()).unwrap();
}
fn publish(host: &mut FileOversight, id: u64, keys: &Keys, value: &EvidenceSnapshot, tick: u64) {
    let result = host.publish_checked(host.revision(), id, Some(&keys.inputs), value.snapshot().clone(), ElapsedTick(tick)).unwrap();
    assert_eq!(result.basis, PublicationBasis::Revalidated);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"recovered publication");
    assert_eq!(host.reconcile(host.revision(), id).unwrap(), Reconciliation::Resolved(result.outcome));
}

#[test]
fn exhausted_capture_recovers_to_real_two_key_publication_without_reusing_old_approval() {
    let root = Directory::new();
    let limits = StateLimits { events: 1, ..StateLimits::default() };
    let (mut host, reviewer) = create(&root, limits); let value = evidence(1, snapshot());
    refresh(&mut host, &root, &value, 1).unwrap();
    let old = ready(&mut host, &reviewer, 1, &value);
    assert_eq!(refresh(&mut host, &root, &value, 2), Err(FileSourceError::Refused(Error::Limit)));
    assert_eq!(host.inspect().control.ledger.reserved, 16);
    assert_eq!(host.file_source_status().unwrap().capture.fault, Some(Error::Limit));
    let request = replacement(&host, 41, 2);
    let change = host.replace_file_source(host.revision(), request).unwrap();
    assert_eq!(change.cancelled, vec![1]); assert_eq!(change.refunded_units, 16);
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(host.inspect().control.ledger.available, 100);
    let status = host.file_source_status().unwrap();
    assert_eq!(status.policy.limits, limits); assert_eq!(status.policy.freshness.max_age_ticks(), 10);
    assert_eq!(status.capture.closed, None); assert_eq!(status.capture.fault, None);
    assert_eq!(status.capture.retained_events, 0);
    assert!(host.dispatch(host.revision(), &old.automatic, &old.human, &old.action,
        &old.inputs, value.snapshot().clone()).is_err());
    assert_eq!(host.propose(host.revision(), 2, spec(&host, b"blocked"), snapshot()).unwrap_err(),
        JournalError::Contract(Error::Incomplete));
    refresh(&mut host, &root, &value, 3).unwrap();
    let fresh = ready(&mut host, &reviewer, 2, &value);
    dispatch(&mut host, &fresh, &value); publish(&mut host, 2, &fresh, &value, 4);
}

#[test]
fn poisoned_generation_and_semantic_floors_survive_replacement_and_new_readers() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root, StateLimits::default());
    let mut state = snapshot(); state.semantic_epoch = 7;
    refresh(&mut host, &root, &evidence(5, state.clone()), 10).unwrap();
    state.semantic_epoch = 9; state.values.insert(99, vec![0; 8193]);
    assert_eq!(refresh(&mut host, &root, &evidence(6, state.clone()), 11), Err(FileSourceError::Refused(Error::Limit)));
    let request = replacement(&host, 51, 2); host.replace_file_source(host.revision(), request).unwrap();
    state.values.remove(&99);
    assert_eq!(refresh(&mut host, &root, &evidence(5, state.clone()), 12), Err(FileSourceError::Refused(Error::Stale)));
    assert_eq!(refresh(&mut host, &root, &evidence(6, state.clone()), 12), Err(FileSourceError::Refused(Error::Binding)));
    state.semantic_epoch = 8;
    assert_eq!(refresh(&mut host, &root, &evidence(7, state.clone()), 12), Err(FileSourceError::Refused(Error::Stale)));
    state.semantic_epoch = 9; let value = evidence(7, state);
    refresh(&mut host, &root, &value, 12).unwrap();
    let keys = ready(&mut host, &reviewer, 1, &value);
    dispatch(&mut host, &keys, &value); publish(&mut host, 1, &keys, &value, 13);
    host.observe_time(host.revision(), ElapsedTick(22)).unwrap();
    assert_eq!(host.propose(host.revision(), 2, spec(&host, b"expired"), value.snapshot().clone()).unwrap_err(),
        JournalError::Contract(Error::Stale));
}

#[test]
fn exact_retries_survive_reopen_without_cancelling_newer_work_or_replacing_again() {
    let root = Directory::new(); let (mut host, reviewer) = create(&root, StateLimits::default());
    let value = evidence(1, snapshot()); refresh(&mut host, &root, &value, 1).unwrap();
    let request = replacement(&host, 61, 3);
    let change = host.replace_file_source(host.revision(), request).unwrap();
    refresh(&mut host, &root, &value, 2).unwrap();
    let keys = ready(&mut host, &reviewer, 2, &value); let before = host.inspect();
    assert_eq!(host.replace_file_source(0, request).unwrap(), change);
    assert_eq!(host.inspect(), before);
    let conflict = FileSourceReplacement { next_generation: 4, ..request };
    assert_eq!(host.replace_file_source(host.revision(), conflict), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(host.inspect(), before);
    dispatch(&mut host, &keys, &value); publish(&mut host, 2, &keys, &value, 3);
    drop(host);
    let (mut host, _) = FileOversight::open(root.store(), profile()).unwrap();
    assert_eq!(host.file_source_replacement(61).unwrap(), change);
    let before = host.inspect();
    assert_eq!(host.replace_file_source(0, request).unwrap(), change);
    assert_eq!(host.inspect(), before);
    assert_eq!(host.file_source_status().unwrap().capture.source.generation, 3);
    assert_eq!(host.file_source_status().unwrap().capture.closed, None);
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn lifetime_generation_cap_survives_reopen_and_cannot_be_reset_by_skipping_numbers() {
    let root = Directory::new(); let (mut host, _) = create(&root, StateLimits::default());
    for operation in 1..=15 {
        let request = replacement(&host, operation, 1 + operation * 10);
        host.replace_file_source(host.revision(), request).unwrap();
        if operation == 7 { drop(host); host = FileOversight::open(root.store(), profile()).unwrap().0; }
    }
    let before = host.inspect(); let status = host.file_source_status();
    let request = replacement(&host, 16, 161);
    assert_eq!(host.replace_file_source(host.revision(), request), Err(JournalError::Contract(Error::Limit)));
    assert_eq!(host.inspect(), before); assert_eq!(host.file_source_status(), status);
    assert_eq!(host.file_source_replacement(16), Err(JournalError::Contract(Error::Missing)));
    assert_eq!(host.inspect().executions, 0);
}
