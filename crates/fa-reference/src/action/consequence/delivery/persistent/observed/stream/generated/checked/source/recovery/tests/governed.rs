//! The original native Store and authenticated Stop, with governance selected.
use super::*;
use crate::action::consequence::delivery::persistent::observed::guarded::{
    FileCampaignRequirement, FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
    generated_recovery::FileGuardedGeneratedSourceRecovery,
};
use crate::action::consequence::policy_campaign::ReplayLimits;

fn campaigns() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits {
            cases: 16,
            input_bytes: 1_048_576,
        },
        max_campaigns: 4,
    }
}

fn seeded(root: &Directory) -> (FileHistoryAnchor, FileRecoveryRequirements) {
    let (mut host, _, _) = FileOversight::create_generated_text_stream_from_source_governed(
        root.store(),
        profile(),
        fixtures::stopped_config(),
        tokenizer(false),
        selection(1),
        campaigns(),
    )
    .unwrap();
    let mut evidence = source(root);
    host.refresh_file_source(host.revision(), &mut evidence, ElapsedTick(1))
        .unwrap();
    let command = command(&host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), command).unwrap();
    host.advance_decoder_text(host.revision(), 7, 0).unwrap();
    let snapshot = host.inspect();
    let expected = FileRecoveryRequirements {
        guards: FileGuardSet {
            stream: Some(host.stream_snapshot().unwrap().confirmed.profile()),
            decoder: Some(fixtures::stopped_config()),
            decoder_stop: None,
            source: Some(host.file_source_status().unwrap().policy),
            identity: None,
            campaigns: Some(campaigns()),
            credential: None,
        },
        effective_policy: host.current_policy().unwrap().clone(),
        credential_epoch: None,
        minimum: FileRecoveryFloor {
            journal_revision: snapshot.revision,
            control_sequence: snapshot.control.sequence,
            authority_epoch: snapshot.control.ledger.epoch,
        },
    };
    (host.history_anchor().unwrap(), expected)
}

fn ready(
    root: &Directory,
    anchor: &FileHistoryAnchor,
    expected: &FileRecoveryRequirements,
) -> FileGuardedGeneratedSourceRecovery {
    let mut run = FileOversight::begin_open_guarded_generated_text_stream_from_source_anchored(
        root.store(),
        profile(),
        expected,
        &fixtures::stopped_config(),
        &tokenizer(false),
        selection(1),
        anchor,
    )
    .unwrap();
    while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    assert_eq!(
        run.progress().status,
        FileGeneratedSourceRecoveryStatus::Ready
    );
    run
}

#[test]
fn governed_native_recovery_storage_failures_return_no_fresh_role_or_stop_acknowledgment() {
    for terminal in [false, true] {
        for barrier in [
            JournalIo::Stage,
            JournalIo::Write,
            JournalIo::FileSync,
            JournalIo::Rename,
            JournalIo::DirectorySync,
        ] {
            let root = Directory::new();
            let (anchor, expected) = seeded(&root);
            let before = canonical(&root);
            let run = ready(&root, &anchor, &expected);
            let revision = run.progress().total_events;
            let request = run.stop_request(701).unwrap();
            run.fail_storage_once(barrier);
            let error = if terminal {
                run.finish_admission_stop(request).err()
            } else {
                run.finish().err()
            };
            assert!(
                matches!(error, Some(JournalError::Io(failure)) if failure.operation == barrier)
            );
            if barrier != JournalIo::DirectorySync {
                assert_eq!(canonical(&root), before);
            }
            let retried = ready(&root, &anchor, &expected);
            if terminal {
                let stopped = retried.finish_admission_stop(request).unwrap();
                assert_eq!(stopped.snapshot().revision, revision as u64 + 2);
                assert_eq!(
                    stopped.appended_events(),
                    if barrier == JournalIo::DirectorySync {
                        0
                    } else {
                        2
                    }
                );
            } else {
                let (host, roles) = retried.finish().unwrap();
                assert!(roles.policy_governor.is_some());
                assert_eq!(
                    host.revision(),
                    revision as u64
                        + if barrier == JournalIo::DirectorySync {
                            2
                        } else {
                            1
                        }
                );
                assert!(host.policy_campaigns_required());
                assert_eq!(
                    host.decoder_text_progress(7).unwrap().generation_revision(),
                    1
                );
            }
        }
    }
}

#[cfg(target_os = "linux")]
mod transport {
    use super::*;
    use crate::action::consequence::delivery::persistent::observed::reviewer::peer::{
        PeerCredentials, PeerPolicy, VerifiedReviewerSocket,
        control::{StopClientProgress, StopControlProgress},
    };
    use std::os::unix::net::UnixStream;

    #[test]
    fn governed_ready_stop_requires_an_independent_authenticated_request_and_returns_only_history()
    {
        let root = Directory::new();
        let (anchor, expected) = seeded(&root);
        let before = canonical(&root);
        fs::write(pending(&root), b"offering Stop is not Stop authority").unwrap();
        let target = ready(&root, &anchor, &expected)
            .into_recovery_stop(701)
            .unwrap();
        let expectation = target.expectation();
        let request = target.request();
        let (server, client) = UnixStream::pair().unwrap();
        server.set_nonblocking(true).unwrap();
        client.set_nonblocking(true).unwrap();
        let policy = |socket: &UnixStream| {
            let peer = PeerCredentials::observe(socket).unwrap();
            PeerPolicy::new(peer.uid(), peer.gid(), Some(peer.pid())).unwrap()
        };
        let server_policy = policy(&server);
        let client_policy = policy(&client);
        let mut server = VerifiedReviewerSocket::verify(server, server_policy)
            .unwrap()
            .into_recovery_stop_connection(target, [47; 32])
            .unwrap();
        let mut client = VerifiedReviewerSocket::verify(client, client_policy)
            .unwrap()
            .into_stop_client(expectation, request.operation)
            .unwrap();
        for _ in 0..1024 {
            server.step().unwrap();
            if client.step().unwrap() == StopClientProgress::NeedsDecision {
                break;
            }
        }
        assert_eq!(client.step().unwrap(), StopClientProgress::NeedsDecision);
        for _ in 0..4 {
            assert_eq!(server.step().unwrap(), StopControlProgress::Blocked);
        }
        assert!(server.application().is_none());
        assert_eq!(canonical(&root), before);
        assert!(pending(&root).exists());
        client.request_stop().unwrap();
        let mut applied = 0;
        let mut complete = false;
        for _ in 0..1024 {
            if server.step().unwrap() == StopControlProgress::Applied {
                applied += 1;
            }
            if client.step().unwrap() == StopClientProgress::Complete {
                complete = true;
                break;
            }
        }
        assert!(complete);
        assert_eq!(applied, 1);
        let application = server.into_application().unwrap();
        assert_eq!(client.receipt(), Some(application.receipt));
        let stopped = application.result.unwrap();
        assert_eq!(stopped.appended_events(), 2);
        assert_eq!(stopped.snapshot().revision, anchor.revision() + 2);
        assert!(stopped.snapshot().stop.is_some());
        let bytes = canonical(&root);
        let repeated = ready(&root, stopped.history_anchor(), &expected)
            .finish_admission_stop(request)
            .unwrap();
        assert_eq!(repeated.appended_events(), 0);
        assert_eq!(canonical(&root), bytes);
    }
}
