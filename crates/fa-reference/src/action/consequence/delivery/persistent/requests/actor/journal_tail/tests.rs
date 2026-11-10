//! Native custody and noninterference over original durable and numerical work.
//! Public Tower tests independently cover real publication and wire correlation.

use super::*;
use crate::action::ElapsedTick;

#[path = "../inspection/tests/fixture.rs"]
mod fixture;
use fixture::*;
#[path = "../review_explanation/tests/refinement/model.rs"]
mod learned_model;

fn known(result: FileJournalTailResult) -> FileJournalTailPage {
    match result.unwrap() {
        Knowledge::Known { value, basis } => {
            assert_eq!(value.basis(), basis);
            value
        }
        other => panic!("expected original journal metadata: {other:?}"),
    }
}

#[test]
fn metadata_pages_preserve_original_snapshot_private_inputs_and_canonical_bytes() {
    let root = Root::new();
    let (mut host, reviewer) = create(&root);
    dispatch(&mut host, &reviewer, 1);
    let (_, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let files = root.files();
    let (port, grant) = supervisor.journal_tail_port(8, 3).unwrap();
    assert_eq!(grant.events_per_page(), 3);
    let mut page = known(port.tail(0));
    let basis = page.basis();
    let mut seen = Vec::new();
    loop {
        assert_eq!(page.basis(), basis);
        assert_eq!(page.after_revision(), 0);
        assert_eq!(page.offset(), seen.len());
        assert_eq!(page.events_per_page(), 3);
        seen.extend_from_slice(page.events());
        assert_eq!(page.through_revision(), seen.len() as u64);
        for secret in [
            "secret-payload",
            "private-question",
            "private-profile",
            "initial-secret",
            "salt",
        ] {
            assert!(!format!("{page:?}").contains(secret));
        }
        if page.enumeration_complete() {
            break;
        }
        page = known(port.clone().continue_tail(page.continuation().unwrap()));
    }
    assert_eq!(seen.len(), before.revision as usize);
    assert_eq!(
        seen.iter()
            .map(FileJournalTailEvent::revision)
            .collect::<Vec<_>>(),
        (1..=before.revision).collect::<Vec<_>>()
    );
    assert!(
        seen.iter()
            .any(|event| event.kind() == FileJournalEventKind::Commit)
    );
    assert!(
        seen.iter()
            .any(|event| event.kind() == FileJournalEventKind::Reveal)
    );
    assert!(
        seen.iter()
            .any(|event| event.kind() == FileJournalEventKind::Dispatch)
    );
    assert!(grant.grant.capture.borrow().is_none());
    assert_eq!(supervisor.owner.borrow().snapshot, Some(snapshot()));
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(root.files(), files);
}

#[test]
fn storage_failure_retires_the_retained_capture_without_reading_an_older_disk_image() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 3);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.journal_tail_port(4, 1).unwrap();
    let first = known(port.tail(0));
    let cursor = first.continuation().unwrap();
    std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
        assert!(host.storage_failure().is_some());
    }
    let files = root.files();
    assert_eq!(port.continue_tail(cursor), Ok(super::unknown()));
    assert!(grant.grant.capture.borrow().is_none());
    assert!(cursor.page.upgrade().is_none());
    assert_eq!(grant.remaining_queries(), 3);
    assert_eq!(port.tail(0), Ok(super::unknown()));
    assert_eq!(grant.remaining_queries(), 3);
    assert_eq!(root.files(), files);
}

#[test]
fn conflicting_borrow_withdraws_old_progress_but_preserves_a_new_explicit_capture() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 2);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.journal_tail_port(4, 1).unwrap();
    let first = known(port.tail(0));
    let cursor = first.continuation().unwrap();
    {
        let _borrow = supervisor.host_mut().unwrap();
        assert_eq!(port.continue_tail(cursor), Ok(super::unknown()));
    }
    assert_eq!(grant.remaining_queries(), 3);
    assert!(grant.grant.capture.borrow().is_none());
    assert_eq!(
        port.continue_tail(cursor),
        Err(ActorError::IdempotencyConflict)
    );
    let fresh = known(port.tail(0));
    assert_eq!(fresh.basis(), first.basis());
    assert_ne!(fresh.continuation().unwrap().id(), cursor.id());
    let middle = known(port.continue_tail(fresh.continuation().unwrap()));
    let final_page = known(port.continue_tail(middle.continuation().unwrap()));
    assert!(final_page.enumeration_complete());
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn equal_scope_replacement_cannot_adopt_original_owner_grants_or_cursors() {
    let first_root = Root::new();
    let (mut first_host, _first_reviewer) = create(&first_root);
    populate(&mut first_host, 2);
    let (_, mut supervisor) = first_host.into_actor_gateway();
    let (port, grant) = supervisor.journal_tail_port(4, 1).unwrap();
    let first = known(port.tail(0));
    let second_root = Root::new();
    let (mut replacement, _second_reviewer) = create(&second_root);
    populate(&mut replacement, 2);
    assert_eq!(replacement.inspect(), supervisor.host().unwrap().inspect());
    *supervisor.host_mut().unwrap() = replacement;
    assert_eq!(
        port.continue_tail(first.continuation().unwrap()),
        Ok(super::unknown())
    );
    assert_eq!(port.tail(0), Ok(super::unknown()));
    assert_eq!(grant.remaining_queries(), 3);
    assert!(grant.grant.capture.borrow().is_none());
    let (new_port, _new_grant) = supervisor.journal_tail_port(1, 64).unwrap();
    let new_page = known(new_port.tail(0));
    assert!(new_page.enumeration_complete());
    assert_eq!(new_page.basis(), first.basis());
}

#[test]
fn actual_learned_metadata_keeps_numerical_state_and_work_unchanged_without_replay() {
    use crate::action::consequence::delivery::persistent::observed::{
        guarded::investigation::proposal_replay_event_attempts,
        replay::review_replay_event_attempts,
    };
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    host.enable_learned_generation(host.revision(), learned_model::config(false, 1))
        .unwrap();
    for _ in 0..2 {
        let state = host.learned_generation_inspection().unwrap().numerical;
        host.advance_learned_generation(host.revision(), state.actor_revision, state.position)
            .unwrap()
            .unwrap();
    }
    host.clear_learned_replay_cache();
    let numerical = host.learned_generation_inspection().unwrap();
    assert_eq!(numerical.numerical.position, 2);
    let (_, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let files = root.files();
    let proposal_attempts = proposal_replay_event_attempts();
    let review_attempts = review_replay_event_attempts();
    let (port, grant) = supervisor.journal_tail_port(8, 2).unwrap();
    let mut page = known(port.tail(0));
    let mut events = Vec::new();
    loop {
        events.extend_from_slice(page.events());
        if page.enumeration_complete() {
            break;
        }
        page = known(port.continue_tail(page.continuation().unwrap()));
    }
    assert!(
        events
            .iter()
            .any(|event| event.kind() == FileJournalEventKind::LearnedEnabled)
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind() == FileJournalEventKind::LearnedStepBegin)
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind() == FileJournalEventKind::LearnedStep)
            .count(),
        2
    );
    assert!(events.iter().all(|event| event.attempt().is_none()
        && event.request().is_none()
        && event.round().is_none()));
    assert_eq!(
        supervisor
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap(),
        numerical
    );
    assert_eq!(proposal_replay_event_attempts(), proposal_attempts);
    assert_eq!(review_replay_event_attempts(), review_attempts);
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(supervisor.owner.borrow().snapshot, Some(snapshot()));
    assert!(grant.grant.capture.borrow().is_none());
    assert_eq!(root.files(), files);
}

#[test]
fn forged_cursor_ids_do_not_replace_valid_capture_or_consume_its_allowance() {
    let root = Root::new();
    let (mut host, _reviewer) = create(&root);
    populate(&mut host, 2);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.journal_tail_port(4, 1).unwrap();
    let first = known(port.tail(0));
    let cursor = first.continuation().unwrap();
    let mut forged = cursor.clone();
    forged.id = cursor.id().checked_add(1).unwrap();
    assert_eq!(
        port.continue_tail(&forged),
        Err(ActorError::IdempotencyConflict)
    );
    forged.id = cursor.id();
    forged.page = Weak::new();
    assert_eq!(
        port.continue_tail(&forged),
        Err(ActorError::IdempotencyConflict)
    );
    assert_eq!(grant.remaining_queries(), 3);
    let second = known(port.continue_tail(cursor));
    assert_eq!(second.offset(), 1);
    assert_eq!(second.events()[0].revision(), 2);
    assert_eq!(grant.remaining_queries(), 2);
}
