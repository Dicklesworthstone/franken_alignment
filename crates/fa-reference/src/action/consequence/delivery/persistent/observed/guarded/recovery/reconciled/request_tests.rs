//! Receipt selection shares the existing native fixture, not a new request reducer.
use super::*;
use super::super::FileGuardedRecoveryStatus as Status;
use crate::action::consequence::delivery::persistent::RecoveryReserve;
use crate::action::consequence::delivery::persistent::observed::FileOversightProfile;
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements};
use std::fs;
#[path = "fixture.rs"]
mod fixture;
use fixture::*;

fn ready(run: &mut FileGuardedRecovery, quantum: usize) {
    for _ in 0..=run.progress().total_events {
        let done = run.progress().replayed_events;
        if run.advance(done, quantum).unwrap().status == Status::Ready { return; }
    }
    panic!("original bounded replay did not complete");
}
fn begin(root: &Directory, expected: &FileRecoveryRequirements) -> FileGuardedRecovery {
    FileOversight::begin_open_guarded(root.store(), profile(), expected).unwrap()
}

#[test]
fn receipt_preflight_requires_a_real_request_before_clock_or_recovery_writes() {
    let root = Directory::new(); let mut prepared = Ready::new(&root);
    // Use the ORIGINAL recorded actor admission, not an invented request row.
    let spec = prepared.action.spec().clone();
    let revision = prepared.host.revision();
    prepared.host.transact(revision, Event::Core(BaseEvent::SubmitRequest(77, spec, snapshot()))).unwrap();
    assert!(prepared.host.request_status(77).is_ok());
    let expected = requirements(&prepared.host, guards()); drop(prepared);
    fs::write(root.store().join("delivery.pending"), b"preserve on request refusal").unwrap();
    let original = root.files();
    assert_eq!(begin(&root, &expected).require_existing_request(77).err(), Some(Error::Incomplete.into()));
    for (request, error) in [(0, Error::InvalidInput), (78, Error::Missing)] {
        let mut run = begin(&root, &expected); ready(&mut run, 1);
        assert_eq!(run.require_existing_request(request).err(), Some(error.into()));
        assert_eq!(root.files(), original);
    }
    let mut run = begin(&root, &expected); ready(&mut run, 1);
    let recovered = run.require_existing_request(77).unwrap().finish_reconciled(|| ElapsedTick(3)).unwrap();
    assert!(recovered.owner.request_status(77).is_ok());
    assert_eq!(recovered.owner.inspect().executions, 0);
    assert!(!root.store().join("delivery.pending").exists());
}
