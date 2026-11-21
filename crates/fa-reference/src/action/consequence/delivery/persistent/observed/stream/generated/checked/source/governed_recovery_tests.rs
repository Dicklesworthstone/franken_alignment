//! Original native inference, campaign custody and exact anchored recovery.
//! Synthetic models test enforcement and accounting, not empirical safety.
use super::*;
use crate::action::consequence::delivery::persistent::observed::{
    credential::FileCredentialPolicy,
    guarded::{
        FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements, anchored::FileHistoryAnchor,
        generated_recovery::FileGuardedGeneratedSourceRecovery,
    },
};

fn expected(host: &FileOversight) -> FileRecoveryRequirements {
    let snapshot = host.inspect();
    FileRecoveryRequirements {
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
    }
}

fn begin(
    root: &Directory,
    selected: GeneratedSourceProfile,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> Result<FileGuardedGeneratedSourceRecovery, JournalError> {
    FileOversight::begin_open_guarded_generated_text_stream_from_source_anchored(
        root.store(),
        governed_profile(),
        expected,
        &fixtures::stopped_config(),
        &tokenizer(false),
        selected,
        anchor,
    )
}

fn advance(run: &mut FileGuardedGeneratedSourceRecovery) {
    while run.progress().status == FileGeneratedSourceRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1).unwrap();
        assert_eq!(after.total_events, before.total_events);
        assert_eq!(after.replayed_events, before.replayed_events + 1);
        assert_eq!(after.attempted_events, after.replayed_events);
    }
    assert_eq!(
        run.progress().status,
        FileGeneratedSourceRecoveryStatus::Ready
    );
}

fn ready(
    root: &Directory,
    selected: GeneratedSourceProfile,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> FileGuardedGeneratedSourceRecovery {
    let mut run = begin(root, selected, expected, anchor).unwrap();
    advance(&mut run);
    run
}

fn refusal(
    root: &Directory,
    selected: GeneratedSourceProfile,
    expected: &FileRecoveryRequirements,
    anchor: &FileHistoryAnchor,
) -> JournalError {
    let mut run = match begin(root, selected, expected, anchor) {
        Ok(run) => run,
        Err(error) => return error,
    };
    // Native configuration mismatches can refuse before replay. Complete guard
    // inventory and policy are checked at Ready by the original guarded owner.
    // Neither path may reach a live or terminal finalizer after a mismatch.
    let error = run
        .advance(
            0,
            crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS,
        )
        .expect_err("mismatched original requirements must never reach Ready");
    assert!(matches!(
        run.progress().status,
        FileGeneratedSourceRecoveryStatus::Failed(_)
    ));
    assert_eq!(run.stop_request(799).err(), Some(error.clone()));
    assert_eq!(run.finish().err(), Some(error.clone()));
    error
}

fn pending(root: &Directory) -> std::path::PathBuf {
    root.store().join("delivery.pending")
}

#[test]
fn governed_recovery_preserves_every_source_profile_and_actual_pending_generation() {
    for mode in 0..6 {
        let root = Directory::new();
        let selected = selection(mode);
        let (mut host, _, _) = create_governed(&root, selected);
        let mut source = evidence(&root);
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
            .unwrap();
        let cmd = command(&host, 7, request(b"ab", 2));
        host.begin_decoder_text(host.revision(), cmd).unwrap();
        host.advance_decoder_text(host.revision(), 7, 0).unwrap();
        let anchor = host.history_anchor().unwrap();
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let expected = expected(&host);
        let numerical = host.decoder_inspection().unwrap().numerical;
        let progress = host.decoder_text_progress(7).unwrap();
        let before = host.inspect();
        let fenced = journal::encode_appended(
            &governed_profile(),
            host.store.identity(),
            &host.events,
            &Event::Core(BaseEvent::Fence),
        )
        .unwrap();
        drop(host);
        let original = canonical(&root);
        std::fs::write(pending(&root), b"retain until original Fence").unwrap();
        let mut run = begin(&root, selected, &expected, &anchor).unwrap();
        assert_eq!(run.progress().attempted_events, 0);
        assert!(matches!(
            begin(&root, selected, &expected, &anchor),
            Err(JournalError::Busy)
        ));
        advance(&mut run);
        assert_eq!(canonical(&root), original);
        assert!(pending(&root).exists());
        let (mut host, roles) = run.finish().unwrap();
        assert!(roles.policy_governor.is_some());
        assert_eq!(canonical(&root), fenced);
        assert!(!pending(&root).exists());
        assert_eq!(host.revision(), before.revision + 1);
        assert!(host.policy_campaigns_required());
        assert!(host.decoder_inspection().unwrap().paused);
        assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
        assert_eq!(
            host.decoder_text_progress(7).unwrap().command(),
            progress.command()
        );
        assert_eq!(host.current_policy().unwrap(), &expected.effective_policy);
        assert_eq!(
            host.journal_capacity().unwrap().reserve(),
            selected.publication.reserve()
        );
        assert_eq!(
            host.held_out_joint_policy().unwrap(),
            selected.publication.joint()
        );
        assert_eq!(
            host.publication_validation_profile().unwrap(),
            selected.publication.witnesses().map(|p| p.limits)
        );
        assert_eq!(host.file_source_status().unwrap().capture.closed, None);
        assert!(!host.clock_ready());
        let before = bytes(&host);
        assert!(host.advance_decoder_text(host.revision(), 7, 1).is_err());
        assert_eq!(bytes(&host), before);
        host.refresh_file_source(host.revision(), &mut source, ElapsedTick(3))
            .unwrap();
        host.resume_decoder(
            host.revision(),
            numerical.actor_revision,
            numerical.position,
        )
        .unwrap();
        while host.decoder_text_progress(7).unwrap().finish().is_none() {
            let revision = host.decoder_text_progress(7).unwrap().generation_revision();
            host.advance_decoder_text(host.revision(), 7, revision)
                .unwrap();
        }
        assert_eq!(
            host.decoder_text_progress(7).unwrap().bytes().unwrap(),
            b"A"
        );
        assert_eq!(host.inspect().executions, 0);
        host.history_anchor_after(&anchor).unwrap();
    }
}

#[test]
fn governed_recovery_issues_fresh_governor_for_original_corpus_and_next_publication() {
    let root = Directory::new();
    let (mut host, human, old_governor) = create_governed(&root, selection(1));
    let mut source = evidence(&root);
    let captured = generate(&mut host, &mut source, 7);
    release(&mut host, &human, &captured, 91, 7, true);
    let review = campaign(&mut host, 701);
    let revision = host.revision();
    let old_key = old_governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let charged = host.inspect().control.ledger.charged;
    let stream = host.stream_snapshot().unwrap();
    let expected = expected(&host);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let (mut host, roles) = ready(&root, selection(1), &expected, &anchor)
        .finish()
        .unwrap();
    let human = roles.human;
    let governor = roles.policy_governor.unwrap();
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().control.ledger.charged, charged);
    assert_eq!(host.stream_snapshot().unwrap().confirmed, stream.confirmed);
    assert_eq!(
        host.policy_campaign(701).unwrap().observed_disposition(),
        CampaignDisposition::Revoked
    );
    let before = bytes(&host);
    let revision = host.revision();
    assert_eq!(
        old_governor
            .approve(&mut host, revision, &review, false)
            .err(),
        Some(Error::Binding.into())
    );
    assert_eq!(
        host.promote_policy_campaign(revision, &old_key),
        Err(Error::Binding.into())
    );
    assert_eq!(bytes(&host), before);

    let new_review = campaign(&mut host, 702);
    assert_eq!(new_review.report().cases().len(), 2);
    let revision = host.revision();
    let key = governor
        .approve(&mut host, revision, &new_review, false)
        .unwrap();
    host.promote_policy_campaign(host.revision(), &key).unwrap();
    assert_eq!(host.current_policy().unwrap().generation(), 2);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().control.ledger.charged, charged);
    let capture = host
        .refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    host.resume_decoder(
        host.revision(),
        numerical.actor_revision,
        numerical.position,
    )
    .unwrap();
    let command = host
        .prepare_decoder_text_continuation(91, 8, request(b"ab", 2))
        .unwrap();
    host.begin_decoder_text_continuation(host.revision(), 91, command)
        .unwrap();
    while host.decoder_text_progress(8).unwrap().finish().is_none() {
        let revision = host.decoder_text_progress(8).unwrap().generation_revision();
        host.advance_decoder_text(host.revision(), 8, revision)
            .unwrap();
    }
    release(&mut host, &human, &capture, 92, 8, true);
    assert_eq!(host.inspect().executions, 2);
    assert_eq!(
        host.stream_snapshot()
            .unwrap()
            .confirmed
            .messages()
            .collect::<Vec<_>>(),
        ["A", "A"]
    );
    assert!(host.inspect().control.ledger.charged > charged);
    assert!(host.decoder_inspection().unwrap().numerical.sampled_draws > numerical.sampled_draws);
    host.history_anchor_after(&anchor).unwrap();
}

#[test]
fn governed_recovery_checks_complete_effective_policy_without_reinterpreting_bootstrap() {
    let root = Directory::new();
    let (mut host, human, governor) = create_governed(&root, selection(1));
    let mut source = evidence(&root);
    let capture = generate(&mut host, &mut source, 7);
    release(&mut host, &human, &capture, 91, 7, true);
    let review = campaign(&mut host, 701);
    let revision = host.revision();
    let key = governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    host.promote_policy_campaign(host.revision(), &key).unwrap();
    let current = expected(&host);
    let anchor = host.history_anchor().unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    drop(host);
    let before = canonical(&root);
    std::fs::write(pending(&root), b"wrong current policy cannot clean staging").unwrap();
    for policy in [
        governed_profile().delivery.policy,
        Policy::new(2, vec![Predicate::PayloadAtMost(2047)]).unwrap(),
        Policy::new(3, vec![Predicate::PayloadAtMost(2048)]).unwrap(),
    ] {
        let mut wrong = current.clone();
        wrong.effective_policy = policy;
        for terminal in [false, true] {
            let mut run = begin(&root, selection(1), &wrong, &anchor).unwrap();
            assert_eq!(
                run.advance(
                    0,
                    crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS
                ),
                Err(Error::Binding.into())
            );
            assert_eq!(
                run.progress().status,
                FileGeneratedSourceRecoveryStatus::Failed(Error::Binding)
            );
            assert_eq!(run.stop_request(799), Err(Error::Binding.into()));
            let error = if terminal {
                run.finish_admission_stop(crate::action::consequence::delivery::StopRequest {
                    operation: 799,
                    expected_control_sequence: 0,
                    expected_authority_epoch: 0,
                })
                .err()
            } else {
                run.finish().err()
            };
            assert_eq!(error, Some(Error::Binding.into()));
            assert_eq!(canonical(&root), before);
            assert!(pending(&root).exists());
        }
    }
    // Changing the bootstrap policy to the CURRENT one would rewrite the meaning
    // of the original corpus. The original framing rejects this before replay.
    let mut wrong_profile = governed_profile();
    wrong_profile.delivery.policy = current.effective_policy.clone();
    assert!(
        FileOversight::begin_open_guarded_generated_text_stream_from_source_anchored(
            root.store(),
            wrong_profile,
            &current,
            &fixtures::stopped_config(),
            &tokenizer(false),
            selection(1),
            &anchor
        )
        .is_err()
    );
    assert_eq!(canonical(&root), before);
    let (host, roles) = ready(&root, selection(1), &current, &anchor)
        .finish()
        .unwrap();
    assert!(roles.policy_governor.is_some());
    assert_eq!(host.current_policy().unwrap(), &current.effective_policy);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
}

#[test]
fn governed_recovery_rejects_missing_changed_or_unselected_guards_before_finalization() {
    let root = Directory::new();
    let (host, _, _) = create_governed(&root, selection(5));
    let correct = expected(&host);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    let before = canonical(&root);
    std::fs::write(pending(&root), b"retain all original guard evidence").unwrap();
    for changed in 0..8 {
        let mut wrong = correct.clone();
        let mut selected = selection(5);
        match changed {
            0 => wrong.guards.campaigns.as_mut().unwrap().max_campaigns += 1,
            1 => wrong.guards.campaigns.as_mut().unwrap().limits.cases += 1,
            2 => wrong.guards.campaigns.as_mut().unwrap().limits.input_bytes -= 1,
            3 => wrong.guards.campaigns.as_mut().unwrap().max_campaigns = 0,
            4 => selected.publication = selection(1).publication,
            5 => selected.source.source.generation += 1,
            6 => {
                let GeneratedSourcePublication::Joint { publication, .. } =
                    &mut selected.publication
                else {
                    unreachable!()
                };
                publication.reserve = None;
            }
            _ => {
                let GeneratedSourcePublication::Joint { publication, .. } =
                    &mut selected.publication
                else {
                    unreachable!()
                };
                publication.feed.as_mut().unwrap().snapshot_fallback = false;
            }
        }
        let error = refusal(&root, selected, &wrong, &anchor);
        assert_eq!(
            error,
            if changed == 3 {
                Error::InvalidInput
            } else {
                Error::Binding
            }
            .into(),
            "{changed}"
        );
        assert_eq!(canonical(&root), before);
        assert!(pending(&root).exists());
    }
    ready(&root, selection(5), &correct, &anchor).cancel();
    assert_eq!(canonical(&root), before);

    let ungoverned = Directory::new();
    let (host, _) = FileOversight::create_generated_text_stream_from_source(
        ungoverned.store(),
        governed_profile(),
        fixtures::stopped_config(),
        tokenizer(false),
        selection(1),
    )
    .unwrap();
    let ungoverned_anchor = host.history_anchor().unwrap();
    let ungoverned_expected = expected(&host);
    drop(host);
    let before = canonical(&ungoverned);
    assert_eq!(
        refusal(
            &ungoverned,
            selection(1),
            &ungoverned_expected,
            &ungoverned_anchor
        ),
        Error::Binding.into()
    );
    assert_eq!(canonical(&ungoverned), before);
    // The same original ungoverned history still has its explicitly ungoverned
    // recovery profile. No guard is installed just to satisfy this selection.
    assert!(
        FileOversight::open_generated_text_stream_from_source_anchored(
            ungoverned.store(),
            governed_profile(),
            &fixtures::stopped_config(),
            &tokenizer(false),
            selection(1),
            &ungoverned_anchor
        )
        .is_ok()
    );
}

#[test]
fn governed_recovery_cannot_silently_accept_another_original_guard_family() {
    use crate::action::consequence::oversight::{
        credibility::{EvaluationProtocol, Fraction},
        decoder_host::HostedStopPolicy,
    };
    for which in 0..3 {
        let root = Directory::new();
        let (mut host, _, _) = create_governed(&root, selection(1));
        match which {
            0 => {
                host.transact(
                    host.revision(),
                    Event::CredentialGuard(FileCredentialPolicy {
                        family: "file".into(),
                        route: "publication".into(),
                        credential: "independently-selected".into(),
                        profile_generation: 1,
                    }),
                )
                .unwrap();
            }
            1 => {
                host.transact(
                    host.revision(),
                    Event::Decoder(DecoderEvent::StopPolicy(
                        HostedStopPolicy::new(91, 1, 9001).unwrap(),
                    )),
                )
                .unwrap();
            }
            _ => {
                host.enable_credibility(
                    host.revision(),
                    EvaluationProtocol {
                        domain: 1,
                        stratum: 1,
                        period: 1,
                        minimum_violation_origins: 1,
                        minimum_benign_origins: 1,
                        precision_floor: Fraction {
                            numerator: 1,
                            denominator: 1,
                        },
                        recall_floor: Fraction {
                            numerator: 1,
                            denominator: 1,
                        },
                        false_positive_ceiling: Fraction {
                            numerator: 1,
                            denominator: 1,
                        },
                        false_stop_budget: 1,
                    },
                )
                .unwrap();
            }
        }
        let expected = expected(&host);
        let anchor = host.history_anchor().unwrap();
        drop(host);
        let before = canonical(&root);
        std::fs::write(pending(&root), b"an original extra guard is not optional").unwrap();
        assert_eq!(
            refusal(&root, selection(1), &expected, &anchor),
            Error::Binding.into(),
            "{which}"
        );
        assert_eq!(canonical(&root), before);
        assert!(pending(&root).exists());
        // This is an otherwise valid original native history, not a malformed
        // file. The compatible legacy cursor still replays it but issues no
        // governor; it is deliberately cancelled before any live finalizer.
        let mut legacy = FileOversight::begin_open_generated_text_stream_from_source_anchored(
            root.store(),
            governed_profile(),
            &fixtures::stopped_config(),
            &tokenizer(false),
            selection(1),
            &anchor,
        )
        .unwrap();
        let complete = legacy
            .advance(
                0,
                crate::action::consequence::delivery::persistent::MAX_JOURNAL_EVENTS,
            )
            .unwrap();
        assert_eq!(complete.status, FileGeneratedSourceRecoveryStatus::Ready);
        legacy.cancel();
        assert_eq!(canonical(&root), before);
    }
    let root = Directory::new();
    let (host, _, _) = create_governed(&root, selection(1));
    let expected = expected(&host);
    let anchor = host.history_anchor().unwrap();
    drop(host);
    assert!(
        ready(&root, selection(1), &expected, &anchor)
            .finish()
            .is_ok()
    );
}

#[test]
fn governed_recovery_never_renews_campaign_capacity_or_unknown_stream_liability() {
    let root = Directory::new();
    let (mut host, human, governor) = create_governed(&root, selection(1));
    let mut source = evidence(&root);
    let capture = generate(&mut host, &mut source, 7);
    let attempt = release(&mut host, &human, &capture, 91, 7, false);
    for id in 701..705 {
        let control = host.inspect().control;
        // Same-corpus repeats of an identical candidate are deduplicated even
        // after rejection. Distinct candidates spend four real campaign slots.
        let update = PolicyUpdate::new(
            id,
            control.sequence,
            control.ledger.epoch,
            Policy::new(
                2,
                vec![Predicate::PayloadAtMost(2048 - (id - 701) as usize)],
            )
            .unwrap(),
        )
        .unwrap();
        let review = host
            .request_policy_campaign(host.revision(), &update)
            .unwrap();
        let revision = host.revision();
        governor.reject(&mut host, revision, &review).unwrap();
    }
    let expected = expected(&host);
    let anchor = host.history_anchor().unwrap();
    let original = host.inspect();
    let numerical = host.decoder_inspection().unwrap().numerical;
    drop(host);
    let (mut host, _) = ready(&root, selection(1), &expected, &anchor)
        .finish()
        .unwrap();
    let control = host.inspect().control;
    let update = PolicyUpdate::new(
        705,
        control.sequence,
        control.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(2048)]).unwrap(),
    )
    .unwrap();
    let before = bytes(&host);
    assert_eq!(
        host.request_policy_campaign(host.revision(), &update).err(),
        Some(Error::Limit.into())
    );
    assert_eq!(bytes(&host), before);
    assert_eq!(
        host.inspect().control.ledger.charged,
        original.control.ledger.charged
    );
    assert_eq!(
        host.inspect().control.ledger.available,
        original.control.ledger.available
    );
    assert_eq!(
        host.inspect().control.ledger.stages[&attempt],
        ActionState::Unknown
    );
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert!(
        host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2))
            .is_err()
    );
    let latest = host.history_anchor_after(&anchor).unwrap();
    drop(host);
    let run = ready(&root, selection(1), &expected, &latest);
    let request = run.stop_request(801).unwrap();
    let stopped = run.finish_admission_stop(request).unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(
        stopped.snapshot().control.ledger.charged,
        original.control.ledger.charged
    );
    assert_eq!(
        stopped.snapshot().control.ledger.stages[&attempt],
        ActionState::Unknown
    );
    let before = canonical(&root);
    let repeated = ready(&root, selection(1), &expected, stopped.history_anchor())
        .finish_admission_stop(request)
        .unwrap();
    assert_eq!(repeated.appended_events(), 0);
    assert_eq!(repeated.snapshot(), stopped.snapshot());
    assert_eq!(canonical(&root), before);
}

#[test]
fn governed_recovery_retains_anchor_cancellation_and_canonical_finalization_boundaries() {
    let root = Directory::new();
    let (mut host, _, _) = create_governed(&root, selection(1));
    let old = canonical(&root);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let expected = expected(&host);
    let anchor = host.history_anchor().unwrap();
    let events = host.events.len();
    let mut fork = host.events.clone();
    *fork.last_mut().unwrap() = Event::Core(BaseEvent::Time(ElapsedTick(3)));
    Machine::replay(&governed_profile(), &fork).unwrap();
    let fork = journal::encode(&governed_profile(), host.store.identity(), &fork).unwrap();
    let original = canonical(&root);
    drop(host);
    std::fs::write(pending(&root), b"independent retained history").unwrap();
    for (bytes, error) in [(&old, Error::Stale), (&fork, Error::Binding)] {
        std::fs::write(root.store().join(storage::CANONICAL), bytes).unwrap();
        assert_eq!(
            begin(&root, selection(1), &expected, &anchor).err(),
            Some(error.into())
        );
        assert_eq!(&canonical(&root), bytes);
        assert!(pending(&root).exists());
    }
    std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    for cut in [0, events / 2, events] {
        let mut run = begin(&root, selection(1), &expected, &anchor).unwrap();
        if cut > 0 {
            run.advance(0, cut).unwrap();
        }
        assert_eq!(
            run.cancel().status,
            FileGeneratedSourceRecoveryStatus::Cancelled
        );
        assert_eq!(canonical(&root), original);
        assert!(pending(&root).exists());
    }
    for terminal in [false, true] {
        let run = ready(&root, selection(1), &expected, &anchor);
        let request = run.stop_request(901).unwrap();
        std::fs::write(root.store().join(storage::CANONICAL), &fork).unwrap();
        let error = if terminal {
            run.finish_admission_stop(request).err()
        } else {
            run.finish().err()
        };
        assert_eq!(error, Some(Error::Binding.into()));
        assert_eq!(canonical(&root), fork);
        assert!(pending(&root).exists());
        std::fs::write(root.store().join(storage::CANONICAL), &original).unwrap();
    }
    assert!(
        ready(&root, selection(1), &expected, &anchor)
            .finish()
            .is_ok()
    );
}
