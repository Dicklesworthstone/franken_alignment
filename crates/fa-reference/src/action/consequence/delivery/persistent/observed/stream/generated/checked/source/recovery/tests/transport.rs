//! The original Linux peer consumer receives the SAME numerical reconstruction.
use super::*;
use crate::action::consequence::delivery::persistent::observed::reviewer::peer::{
    PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
    control::{
        FileRecoveryStop, StopClient, StopClientProgress, StopControlProgress,
        StopRecoveryConnection, StopStatus,
    },
};
use std::os::unix::net::UnixStream;

const OPERATION: u64 = 701;

fn connect(
    target: FileRecoveryStop,
) -> (StopRecoveryConnection<UnixStream>, StopClient<UnixStream>) {
    let expectation = target.expectation();
    let operation = target.request().operation;
    let (server, client) = UnixStream::pair().unwrap();
    server.set_nonblocking(true).unwrap();
    client.set_nonblocking(true).unwrap();
    let policy = |socket: &UnixStream| {
        let peer = PeerCredentials::observe(socket).unwrap();
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
fn authenticated_native_recovery_stop_needs_explicit_request_and_preserves_unknown_liability() {
    let root = Directory::new();
    let host = unknown_message(&root);
    let before = host.inspect();
    let numerical = host.decoder_inspection().unwrap().numerical;
    drop(host);
    fs::write(pending(&root), b"preserve until explicit peer Stop").unwrap();
    let unchanged = canonical(&root);
    let target = ready(&root, selection(1))
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
    assert_eq!(target.expectation().reviewer, profile().human.reviewer_id);
    assert_eq!(target.expectation().scope, profile().delivery.scope);
    assert_eq!(
        target.expectation().clock_domain,
        profile().delivery.clock_domain
    );
    let (mut server, mut client) = connect(target);
    offer(&mut server, &mut client);
    for _ in 0..4 {
        assert_eq!(server.step().unwrap(), StopControlProgress::Blocked);
        assert_eq!(client.step().unwrap(), StopClientProgress::NeedsDecision);
    }
    assert!(server.application().is_none());
    assert_eq!(canonical(&root), unchanged);
    assert!(pending(&root).exists());
    assert!(matches!(
        begin(&root, selection(1)),
        Err(JournalError::Busy)
    ));
    // Disconnection, silence, and even an offered operation are not Stop intent.
    drop(client);
    drop(server);
    assert_eq!(canonical(&root), unchanged);
    assert!(pending(&root).exists());

    let run = ready(&root, selection(1));
    assert_eq!(run.machine.broker.hosted_decoder().unwrap(), numerical);
    let (mut server, mut client) = connect(run.into_recovery_stop(OPERATION).unwrap());
    offer(&mut server, &mut client);
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
        stopped.snapshot().control.ledger.charged,
        before.control.ledger.charged
    );
    assert_eq!(
        stopped.snapshot().control.ledger.stages,
        before.control.ledger.stages
    );
    assert_eq!(stopped.snapshot().payload, before.payload);
    assert_eq!(stopped.snapshot().executions, 0);
    assert!(!stopped.progress().drained());
    assert!(!pending(&root).exists());
    let canonical_stop = canonical(&root);
    let replayed = ready(&root, selection(1));
    assert_eq!(replayed.machine.broker.hosted_decoder().unwrap(), numerical);
    assert!(replayed.machine.decoder_paused());
    let (mut server, mut client) = connect(replayed.into_recovery_stop(OPERATION).unwrap());
    offer(&mut server, &mut client);
    client.request_stop().unwrap();
    finish(&mut server, &mut client);
    let retry = server.into_application().unwrap().result.unwrap();
    assert_eq!(retry.appended_events(), 0);
    assert_eq!(retry.snapshot(), stopped.snapshot());
    assert_eq!(retry.history_anchor(), stopped.history_anchor());
    assert_eq!(canonical(&root), canonical_stop);
}

#[test]
fn incomplete_or_unreserved_native_reconstruction_cannot_create_a_stop_target() {
    for mode in [0, 1] {
        let root = Directory::new();
        let selected = selection(mode);
        let host = seed_pending(&root, selected);
        let count = host.events.len();
        drop(host);
        fs::write(pending(&root), b"original staging").unwrap();
        let unchanged = canonical(&root);
        for cut in 0..count {
            let mut run = begin(&root, selected).unwrap();
            if cut != 0 {
                run.advance(0, cut).unwrap();
            }
            assert_eq!(
                run.into_recovery_stop(OPERATION).err(),
                Some(Error::Incomplete.into())
            );
            assert_eq!(canonical(&root), unchanged);
            assert!(pending(&root).exists());
        }
        let target = ready(&root, selected).into_recovery_stop(OPERATION);
        if mode == 0 {
            assert_eq!(target.err(), Some(Error::Incomplete.into()));
        } else {
            drop(target.unwrap());
        }
        assert_eq!(canonical(&root), unchanged);
        assert!(pending(&root).exists());
        // Declining the terminal path still permits the original checked opener.
        let (host, _) = ready(&root, selected).finish().unwrap();
        assert_eq!(
            host.decoder_text_progress(7).unwrap().generation_revision(),
            1
        );
    }
}
