//! Original native source recovery with an independently retained history cut.
//! Synthetic weights test contract composition, not detector quality or origin.
use super::*;
use crate::action::consequence::delivery::persistent::observed::guarded::anchored::MAX_HISTORY_ANCHOR_BYTES;
use std::io::Cursor;

fn begin_anchored(
    root: &Directory,
    selected: GeneratedSourceProfile,
    anchor: &FileHistoryAnchor,
) -> Result<FileGeneratedSourceRecovery, JournalError> {
    FileOversight::begin_open_generated_text_stream_from_source_anchored(
        root.store(),
        profile(),
        &fixtures::stopped_config(),
        &tokenizer(false),
        selected,
        anchor,
    )
}

fn portable(anchor: &FileHistoryAnchor) -> FileHistoryAnchor {
    let mut bytes = Vec::new();
    anchor
        .write_to(&mut bytes, MAX_HISTORY_ANCHOR_BYTES)
        .unwrap();
    FileHistoryAnchor::read_trusted(&mut Cursor::new(bytes), MAX_HISTORY_ANCHOR_BYTES).unwrap()
}

#[cfg(target_os = "linux")]
mod authenticated_stop {
    use super::*;
    use crate::action::consequence::delivery::persistent::observed::reviewer::peer::{
        PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
        control::{
            FileRecoveryStop, StopClient, StopClientProgress, StopControlProgress,
            StopRecoveryConnection, StopStatus,
        },
    };
    use std::os::unix::net::UnixStream;

    fn ready_anchored(
        root: &Directory,
        anchor: &FileHistoryAnchor,
    ) -> FileGeneratedSourceRecovery {
        let mut run = begin_anchored(root, selection(1), anchor).unwrap();
        advance(&mut run);
        run
    }

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

    fn offer(
        server: &mut StopRecoveryConnection<UnixStream>,
        client: &mut StopClient<UnixStream>,
    ) {
        for _ in 0..1024 {
            server.step().unwrap();
            if client.step().unwrap() == StopClientProgress::NeedsDecision {
                return;
            }
        }
        panic!("original anchored Stop offer did not complete");
    }

    fn finish(
        server: &mut StopRecoveryConnection<UnixStream>,
        client: &mut StopClient<UnixStream>,
    ) {
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
        panic!("original anchored Stop acknowledgment did not complete");
    }

    #[test]
    fn anchored_peer_stop_requires_explicit_request_and_retains_unknown_liability_on_retry() {
        let root = Directory::new();
        let host = unknown_message(&root);
        let before = host.inspect();
        let anchor = portable(&host.history_anchor().unwrap());
        let numerical = host.decoder_inspection().unwrap().numerical;
        assert!(before.control.ledger.charged > 0);
        drop(host);
        let before_bytes = canonical(&root);
        fs::write(pending(&root), b"anchor is not authenticated Stop intent").unwrap();
        let target = ready_anchored(&root, &anchor).into_recovery_stop(701).unwrap();
        let (mut server, mut client) = connect(target);
        offer(&mut server, &mut client);
        for _ in 0..4 {
            assert_eq!(server.step().unwrap(), StopControlProgress::Blocked);
            assert_eq!(client.step().unwrap(), StopClientProgress::NeedsDecision);
        }
        assert!(server.application().is_none());
        assert_eq!(canonical(&root), before_bytes);
        assert!(pending(&root).exists());
        assert!(matches!(
            begin_anchored(&root, selection(1), &anchor),
            Err(JournalError::Busy)
        ));
        drop(client);
        drop(server);
        assert_eq!(canonical(&root), before_bytes);

        let run = ready_anchored(&root, &anchor);
        assert_eq!(run.machine.broker.hosted_decoder().unwrap(), numerical);
        let (mut server, mut client) = connect(run.into_recovery_stop(701).unwrap());
        offer(&mut server, &mut client);
        client.request_stop().unwrap();
        finish(&mut server, &mut client);
        let application = server.into_application().unwrap();
        assert_eq!(client.receipt(), Some(application.receipt));
        assert_eq!(application.receipt.status, StopStatus::StoppedPending);
        let result = application.result.unwrap();
        assert_eq!(result.appended_events(), 2);
        assert_eq!(result.snapshot().revision, before.revision + 2);
        assert_eq!(
            result.snapshot().control.ledger.charged,
            before.control.ledger.charged
        );
        assert_eq!(
            result.snapshot().control.ledger.stages,
            before.control.ledger.stages
        );
        assert_eq!(result.snapshot().payload, before.payload);
        assert_eq!(result.snapshot().executions, before.executions);
        assert!(!result.progress().drained());
        assert!(!pending(&root).exists());

        let stopped = portable(result.history_anchor());
        let stopped_bytes = canonical(&root);
        // A previously valid pre-Stop image cannot erase the acknowledged Stop
        // once its independently retained successor anchor is required.
        fs::write(root.store().join(storage::CANONICAL), &before_bytes).unwrap();
        fs::write(pending(&root), b"retain acknowledged Stop rollback evidence").unwrap();
        assert_eq!(
            begin_anchored(&root, selection(1), &stopped).err(),
            Some(Error::Stale.into())
        );
        assert_eq!(canonical(&root), before_bytes);
        assert_eq!(
            fs::read(pending(&root)).unwrap(),
            b"retain acknowledged Stop rollback evidence"
        );
        fs::write(root.store().join(storage::CANONICAL), &stopped_bytes).unwrap();
        let retry = ready_anchored(&root, &stopped);
        assert_eq!(retry.machine.broker.hosted_decoder().unwrap(), numerical);
        assert_eq!(retry.stop_request(702), Err(Error::Duplicate.into()));
        let (mut server, mut client) = connect(retry.into_recovery_stop(701).unwrap());
        offer(&mut server, &mut client);
        client.request_stop().unwrap();
        finish(&mut server, &mut client);
        let retried = server.into_application().unwrap().result.unwrap();
        assert_eq!(retried.appended_events(), 0);
        assert_eq!(retried.snapshot(), result.snapshot());
        assert_eq!(retried.history_anchor(), result.history_anchor());
        assert_eq!(canonical(&root), stopped_bytes);
    }
}
