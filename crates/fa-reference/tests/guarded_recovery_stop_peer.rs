//! Plain guarded recovery uses the original peer-authenticated, no-clock Stop.
//! Synthetic ballots exercise liability/authority invariants, not helper quality.
#![cfg(target_os = "linux")]
#![forbid(unsafe_code)]

#[path = "support/file_guarded.rs"]
mod support;

use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::{
    JournalError, Reconciliation, RecoveryReserve,
    observed::{
        FileOversight,
        guarded::{
            FileGuardSet, FileRecoveryRequirements,
            anchored::FileHistoryAnchor,
            recovery::{FileGuardedRecovery, FileGuardedRecoveryStatus},
        },
        reviewer::peer::{
            PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
            control::{
                FileRecoveryStop, StopClient, StopClientProgress, StopControlProgress,
                StopRecoveryConnection, StopStatus,
            },
        },
    },
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::{ActionState, ElapsedTick};
use std::{collections::BTreeMap, fs, os::unix::net::UnixStream, path::PathBuf};
use support::{Directory, ordinary};

const OPERATION: u64 = 902;

fn guards() -> FileGuardSet {
    FileGuardSet {
        identity: None,
        ..support::guards()
    }
}

fn seed(root: &Directory, liabilities: bool) -> FileOversight {
    let (mut host, roles) = FileOversight::create_guarded_reserved(
        root.store(),
        ordinary::profile(),
        &guards(),
        None,
        RecoveryReserve::terminal(),
    )
    .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    if liabilities {
        let sent = ordinary::ready(&mut host, &roles.human, 1, b"unknown");
        ordinary::dispatch(&mut host, &sent);
        assert_eq!(
            host.reconcile(host.revision(), 1).unwrap(),
            Reconciliation::AwaitingResolution
        );
        let _reserved = ordinary::ready(&mut host, &roles.human, 2, b"reserved");
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert_eq!(host.inspect().control.ledger.reserved, 16);
    }
    host
}

fn begin(
    root: &Directory,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> Result<FileGuardedRecovery, JournalError> {
    FileOversight::begin_open_guarded_anchored(root.store(), ordinary::profile(), expected, anchor)?
        .require_recovery_reserve(RecoveryReserve::terminal())
}

fn advance(run: &mut FileGuardedRecovery) -> Result<(), JournalError> {
    while run.progress().status == FileGuardedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1)?;
        assert_eq!(after.replayed_events, before.replayed_events + 1);
    }
    assert_eq!(run.progress().status, FileGuardedRecoveryStatus::Ready);
    Ok(())
}

fn ready(
    root: &Directory,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> FileGuardedRecovery {
    let mut run = begin(root, expected, anchor).unwrap();
    advance(&mut run).unwrap();
    run
}

fn files(root: &Directory) -> BTreeMap<PathBuf, Vec<u8>> {
    fs::read_dir(root.store())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

fn connect(
    target: FileRecoveryStop,
) -> (StopRecoveryConnection<UnixStream>, StopClient<UnixStream>) {
    let expectation = target.expectation();
    let operation = target.request().operation;
    let (server, client) = UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    client.set_nonblocking(true).unwrap();
    let policy = |stream: &UnixStream| {
        let peer = PeerCredentials::observe(stream).unwrap();
        PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap()
    };
    let server_policy = policy(&server);
    let client_policy = policy(&client);
    (
        VerifiedReviewerSocket::verify(server, server_policy)
            .unwrap()
            .into_recovery_stop_connection(target, [39; 32])
            .unwrap(),
        VerifiedReviewerSocket::verify(client, client_policy)
            .unwrap()
            .into_stop_client(expectation, operation)
            .unwrap(),
    )
}

fn offer(server: &mut StopRecoveryConnection<UnixStream>, client: &mut StopClient<UnixStream>) {
    for _ in 0..1024 {
        server.step().unwrap();
        if client.step().unwrap() == StopClientProgress::NeedsDecision {
            return;
        }
    }
    panic!("original Stop offer did not complete");
}

fn finish(server: &mut StopRecoveryConnection<UnixStream>, client: &mut StopClient<UnixStream>) {
    let mut applied = 0;
    for _ in 0..1024 {
        if server.step().unwrap() == StopControlProgress::Applied {
            applied += 1;
        }
        if client.step().unwrap() == StopClientProgress::Complete {
            assert_eq!(applied, 1);
            return;
        }
    }
    panic!("original Stop acknowledgment did not complete");
}

#[test]
fn authenticated_stop_preserves_unknown_charge_and_cancels_only_undispatched_reservations() {
    let root = Directory::new();
    let host = seed(&root, true);
    let expected = support::requirements(&host, guards());
    let anchor = host.history_anchor().unwrap();
    let before = host.inspect();
    drop(host);
    fs::write(
        root.store().join("delivery.pending"),
        b"keep before explicit Stop",
    )
    .unwrap();
    let unchanged = files(&root);
    let target = ready(&root, &expected, &anchor)
        .into_recovery_stop(OPERATION)
        .unwrap();
    assert_eq!(
        target.request().expected_control_sequence,
        before.control.sequence
    );
    assert_eq!(
        target.request().expected_authority_epoch,
        before.control.ledger.epoch
    );
    assert_eq!(
        target.expectation().reviewer,
        ordinary::profile().human.reviewer_id
    );
    assert_eq!(
        target.expectation().scope,
        ordinary::profile().delivery.scope
    );
    assert_eq!(
        target.expectation().clock_domain,
        ordinary::profile().delivery.clock_domain
    );
    let (mut server, mut client) = connect(target);
    offer(&mut server, &mut client);
    for _ in 0..4 {
        assert_eq!(server.step().unwrap(), StopControlProgress::Blocked);
        assert_eq!(client.step().unwrap(), StopClientProgress::NeedsDecision);
    }
    assert!(server.application().is_none());
    assert_eq!(files(&root), unchanged);
    assert!(matches!(
        begin(&root, &expected, &anchor),
        Err(JournalError::Busy)
    ));
    client.request_stop().unwrap();
    finish(&mut server, &mut client);
    let application = server.into_application().unwrap();
    assert_eq!(client.receipt(), Some(application.receipt));
    assert_eq!(application.receipt.status, StopStatus::StoppedPending);
    assert!(!client.outcome_unknown());
    let stopped = application.result.unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(stopped.snapshot().revision, before.revision + 2);
    assert_eq!(
        stopped.snapshot().control.ledger.elapsed,
        before.control.ledger.elapsed
    );
    assert_eq!(
        stopped.snapshot().control.ledger.stages[&1],
        ActionState::Unknown
    );
    assert_eq!(
        stopped.snapshot().control.ledger.stages[&2],
        ActionState::Cancelled
    );
    assert_eq!(stopped.progress().charged_units, 16);
    assert_eq!(stopped.progress().reserved_units, 0);
    assert!(!stopped.progress().drained());
    assert_eq!(stopped.snapshot().payload, before.payload);
    assert_eq!(stopped.snapshot().executions, 0);
    assert!(stopped.snapshot().control.suspended);
    assert!(!root.store().join("delivery.pending").exists());
    // The same operation selects the retained predecessor and performs no new
    // clock observation or terminal write. Another operation cannot take over.
    let canonical = files(&root);
    let retry = ready(&root, &expected, stopped.history_anchor());
    assert_eq!(
        retry.into_recovery_stop(OPERATION + 1).err(),
        Some(Error::Duplicate.into())
    );
    let (mut server, mut client) = connect(
        ready(&root, &expected, stopped.history_anchor())
            .into_recovery_stop(OPERATION)
            .unwrap(),
    );
    offer(&mut server, &mut client);
    client.request_stop().unwrap();
    finish(&mut server, &mut client);
    let retried = server.into_application().unwrap().result.unwrap();
    assert_eq!(retried.appended_events(), 0);
    assert_eq!(retried.snapshot(), stopped.snapshot());
    assert_eq!(files(&root), canonical);
}

#[test]
fn incomplete_and_mismatched_guarded_cursors_never_issue_stop_or_clean_staging() {
    let root = Directory::new();
    let host = seed(&root, false);
    let expected = support::requirements(&host, guards());
    let anchor = host.history_anchor().unwrap();
    let count = host.revision() as usize;
    drop(host);
    fs::write(root.store().join("delivery.pending"), b"preserve").unwrap();
    let unchanged = files(&root);
    for cut in 0..count {
        let mut run = begin(&root, &expected, &anchor).unwrap();
        for predecessor in 0..cut {
            run.advance(predecessor, 1).unwrap();
        }
        assert_eq!(
            run.into_recovery_stop(OPERATION).err(),
            Some(Error::Incomplete.into())
        );
        assert_eq!(files(&root), unchanged);
    }
    for mismatch in 0..7 {
        let mut wrong = expected.clone();
        match mismatch {
            0 => wrong.guards.campaigns = None,
            1 => wrong.guards.campaigns.as_mut().unwrap().max_campaigns += 1,
            2 => wrong.guards.campaigns.as_mut().unwrap().limits.cases += 1,
            3 => wrong.minimum.journal_revision += 1,
            4 => wrong.minimum.control_sequence += 1,
            5 => wrong.minimum.authority_epoch += 1,
            _ => {
                wrong.effective_policy = Policy::new(1, vec![Predicate::PayloadAtMost(1)]).unwrap()
            }
        }
        assert!(
            begin(&root, &wrong, &anchor)
                .and_then(|mut run| {
                    advance(&mut run)?;
                    run.into_recovery_stop(OPERATION)
                })
                .is_err(),
            "independent mismatch {mismatch}"
        );
        assert_eq!(files(&root), unchanged);
    }
    assert_eq!(
        ready(&root, &expected, &anchor).into_recovery_stop(0).err(),
        Some(Error::InvalidInput.into())
    );
    let target = ready(&root, &expected, &anchor)
        .into_recovery_stop(OPERATION)
        .unwrap();
    let (mut server, mut client) = connect(target);
    offer(&mut server, &mut client);
    drop(client);
    drop(server);
    assert_eq!(files(&root), unchanged);
    drop(begin(&root, &expected, &anchor).unwrap());
    assert_eq!(files(&root), unchanged);
}

#[test]
fn missing_or_different_recovery_reserve_cannot_be_replaced_by_the_stop_handoff() {
    let root = Directory::new();
    let (host, _) =
        FileOversight::create_guarded(root.store(), ordinary::profile(), &guards(), None).unwrap();
    let expected = support::requirements(&host, guards());
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let unchanged = files(&root);
    let mut run = FileOversight::begin_open_guarded_anchored(
        root.store(),
        ordinary::profile(),
        &expected,
        &anchor,
    )
    .unwrap();
    advance(&mut run).unwrap();
    assert_eq!(
        run.into_recovery_stop(OPERATION).err(),
        Some(Error::Incomplete.into())
    );
    assert_eq!(files(&root), unchanged);

    let root = Directory::new();
    let host = seed(&root, false);
    let expected = support::requirements(&host, guards());
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let unchanged = files(&root);
    let mut wrong = RecoveryReserve::terminal();
    wrong.events += 1;
    assert!(
        FileOversight::begin_open_guarded_anchored(
            root.store(),
            ordinary::profile(),
            &expected,
            &anchor
        )
        .unwrap()
        .require_recovery_reserve(wrong)
        .is_err()
    );
    assert_eq!(files(&root), unchanged);
    drop(
        ready(&root, &expected, &anchor)
            .into_recovery_stop(OPERATION)
            .unwrap(),
    );
    assert_eq!(files(&root), unchanged);
}
