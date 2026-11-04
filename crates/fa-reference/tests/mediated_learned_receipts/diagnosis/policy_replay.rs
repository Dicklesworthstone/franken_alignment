//! Whole retained-policy comparisons from real original learned recovery.
//! Synthetic models are controls, never detector or deployment qualification.
use super::*;
use fa_reference::action::consequence::policy_campaign::{
    PolicyDelta, PolicyReplayReport, ReplayCaseId, ReplayLimits,
};

fn limits() -> ReplayLimits {
    ReplayLimits {
        cases: 16,
        input_bytes: 1_048_576,
    }
}

fn candidate(generation: u64, bytes: usize) -> Policy {
    Policy::new(generation, vec![Predicate::PayloadAtMost(bytes)]).unwrap()
}

fn finish(
    run: Cursor,
    next: Policy,
    allowance: ReplayLimits,
) -> Result<(FileDeliverySnapshot, PolicyReplayReport), JournalError> {
    cursor!(run, run => run.finish_policy_replay(next, allowance))
}

fn replay(
    history: &DeniedHistory,
    next: Policy,
    allowance: ReplayLimits,
) -> Result<(FileDeliverySnapshot, PolicyReplayReport), JournalError> {
    finish(history.begin()?.ready()?, next, allowance)
}

fn history_cursor(history: &History) -> Cursor {
    match history.begin() {
        Recovery::Plain(run) => Cursor::Mediated(run),
        Recovery::Predictive(run) => Cursor::PredictiveMediated(run),
    }
}

#[test]
fn denied_corpus_is_complete_in_all_six_guarded_modes_and_releases_the_writer() {
    for mediated in [false, true] {
        for lane in LANES {
            let history = DeniedHistory::new(lane, mediated, Admission::Denied);
            let before = history.files();
            let result = replay(&history, candidate(2, 4096), limits()).unwrap();
            let (snapshot, report) = &result;
            assert_eq!(snapshot, &history.journal);
            assert_eq!(snapshot.executions, 0);
            assert_eq!(snapshot.control.ledger.charged, 0);
            assert_eq!(snapshot.control.ledger.reserved, 0);
            assert_eq!(report.previous_policy(), &history.profile.delivery.policy);
            assert_eq!(report.candidate_policy(), &candidate(2, 4096));
            assert_eq!(report.cases().len(), 1);
            assert_eq!(
                report.newly_reviewable(),
                vec![ReplayCaseId::Proposal(history.attempt.unwrap())]
            );
            let case = &report.cases()[0];
            assert_eq!(case.original().result(), Truth::Violated);
            assert_eq!(case.candidate().unwrap().result(), Truth::Satisfied);
            assert_eq!(case.original_consequence(), None);
            assert!(case.missing_nodes().is_empty());
            assert!(!report.requires_shadow());
            assert_eq!(history.files(), before);
            // The report holds no writer, owner or role. Repeat the exact
            // anchored replay while the first immutable result remains alive.
            assert_eq!(replay(&history, candidate(2, 4096), limits()).unwrap(), result);
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn real_message_finish_and_review_cases_cannot_be_selectively_dropped() {
    for lane in LANES {
        let history = History::new(lane, Cut::FinishPending, false);
        let before = history.files();
        let result = finish(
            history_cursor(&history).ready().unwrap(),
            candidate(2, 4096),
            limits(),
        )
        .unwrap();
        let (snapshot, report) = &result;
        assert_eq!(snapshot.revision, history.anchor.revision());
        assert_eq!(snapshot.executions, 1);
        assert_eq!(report.cases().len(), 3);
        assert_eq!(
            report
                .cases()
                .iter()
                .filter(|case| matches!(case.id(), ReplayCaseId::Proposal(_)))
                .count(),
            2
        );
        assert_eq!(
            report
                .cases()
                .iter()
                .filter(|case| matches!(case.id(), ReplayCaseId::Review { .. }))
                .count(),
            1
        );
        assert!(report.cases().iter().any(|case| {
            case.id() == ReplayCaseId::Proposal(history.attempt)
        }));
        assert!(report.cases().iter().any(|case| {
            matches!(case.id(), ReplayCaseId::Review { attempt, round: 101, .. }
                if attempt == history.attempt)
                && case.original_consequence().is_some()
        }));
        assert!(report.cases().iter().all(|case| case.delta() == PolicyDelta::Unchanged));
        assert_eq!(
            finish(
                history_cursor(&history).ready().unwrap(),
                candidate(2, 4096),
                ReplayLimits { cases: 2, ..limits() },
            )
            .unwrap_err(),
            Error::Limit.into()
        );
        // A proposal-specific experiment cannot turn the corpus into a
        // favorable single-case campaign: the finalizer uses the same broker.
        let selected = history_cursor(&history).select(72).unwrap().ready().unwrap();
        assert_eq!(finish(selected, candidate(2, 4096), limits()).unwrap(), result);
        assert_eq!(history.files(), before);
    }
}

#[test]
fn unknown_publications_stay_charged_when_candidate_policy_would_block_them() {
    for lane in LANES {
        let history = History::new(lane, Cut::Sent, false);
        let before = history.files();
        assert!(matches!(history.original, Knowledge::Unknown { .. }));
        let (snapshot, report) = finish(
            history_cursor(&history).ready().unwrap(),
            candidate(2, 1),
            limits(),
        )
        .unwrap();
        assert_eq!(snapshot.revision, history.anchor.revision());
        assert_eq!(snapshot.executions, 0);
        assert!(history.charge > 0);
        assert_eq!(snapshot.control.ledger.charged, history.charge);
        assert_eq!(report.cases().len(), 2);
        assert_eq!(report.newly_blocked().len(), 2);
        assert!(report.newly_reviewable().is_empty());
        assert_eq!(observe(&history.ready().port(71, None).unwrap()), history.original);
        assert_eq!(history.files(), before);
    }
}

#[test]
fn unseen_observations_stay_shadow_required_even_below_a_satisfied_boolean() {
    for mediated in [false, true] {
        for lane in LANES {
            let history = DeniedHistory::new(lane, mediated, Admission::Denied);
            let before = history.files();
            let next = Policy::new(
                2,
                vec![
                    Predicate::PayloadAtMost(4096),
                    Predicate::Absent { key: 999 },
                    Predicate::Any(vec![0, 1]),
                ],
            )
            .unwrap();
            let (snapshot, report) = replay(&history, next, limits()).unwrap();
            assert_eq!(snapshot, history.journal);
            assert!(report.requires_shadow());
            assert!(report.newly_reviewable().is_empty());
            assert_eq!(report.cases()[0].delta(), PolicyDelta::RequiresShadow);
            assert_eq!(report.cases()[0].missing_nodes(), &[1]);
            assert!(report.cases()[0].candidate().is_none());
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn incomplete_stale_empty_and_over_budget_comparisons_have_no_partial_result() {
    for mediated in [false, true] {
        for lane in LANES {
            let history = DeniedHistory::new(lane, mediated, Admission::Denied);
            let before = history.files();
            assert_eq!(
                finish(history.begin().unwrap(), candidate(2, 4096), limits()).unwrap_err(),
                Error::Incomplete.into()
            );
            assert_eq!(
                replay(&history, candidate(1, 4096), limits()).unwrap_err(),
                Error::Stale.into()
            );
            for (allowance, expected) in [
                (ReplayLimits { cases: 0, ..limits() }, Error::InvalidInput),
                (ReplayLimits { input_bytes: 0, ..limits() }, Error::InvalidInput),
                (ReplayLimits { input_bytes: 1, ..limits() }, Error::Limit),
            ] {
                assert_eq!(
                    replay(&history, candidate(2, 4096), allowance).unwrap_err(),
                    expected.into()
                );
                assert_eq!(history.files(), before);
            }
            let report = replay(&history, candidate(2, 4096), limits()).unwrap().1;
            let case = &report.cases()[0];
            let retained_bytes = case.action().spec().payload.len();
            assert!(retained_bytes > 2, "charge the encoded frame, not only decoded OK");
            assert!(case.action().spec().required_witnesses.is_empty());
            assert!(case.original().witnesses().is_empty());
            assert!(case.candidate().unwrap().witnesses().is_empty());
            assert_eq!(report.input_bytes(), retained_bytes);
            let exact = ReplayLimits { cases: 1, input_bytes: retained_bytes };
            assert_eq!(
                replay(&history, candidate(2, 4096), exact)
                    .unwrap()
                    .1
                    .input_bytes(),
                retained_bytes
            );
            assert_eq!(
                replay(
                    &history,
                    candidate(2, 4096),
                    ReplayLimits { input_bytes: retained_bytes - 1, ..exact },
                )
                .unwrap_err(),
                Error::Limit.into()
            );
            assert_eq!(history.files(), before);
            let refused = DeniedHistory::new(lane, mediated, Admission::NotAdmitted);
            let before = refused.files();
            assert_eq!(
                replay(&refused, candidate(2, 4096), limits()).unwrap_err(),
                Error::Incomplete.into()
            );
            assert_eq!(refused.files(), before);
        }
    }
}

#[test]
fn prior_policy_records_are_not_relabelled_as_the_effective_policy_corpus() {
    for lane in LANES {
        let history =
            DeniedHistory::build(lane, true, Admission::Reviewing, false, true, false);
        let before = history.files();
        assert_eq!(history.expected.oversight.effective_policy.generation(), 2);
        assert_eq!(
            history.investigate(REQUEST).unwrap().origin().proposal().policy.generation(),
            1
        );
        assert_eq!(
            replay(&history, candidate(2, 4096), limits()).unwrap_err(),
            Error::Stale.into()
        );
        // A real independently approved promotion exists, but no new proposal
        // under policy 2 exists. Do not silently replay policy 1 as policy 2.
        assert_eq!(
            replay(&history, candidate(3, 4096), limits()).unwrap_err(),
            Error::Incomplete.into()
        );
        assert_eq!(history.files(), before);
    }
}

fn rejects_guard(history: &DeniedHistory, expected: &Contract) {
    let before = history.files();
    let result = history
        .begin_with(expected, &history.config, &history.anchor)
        .and_then(Cursor::ready)
        .and_then(|run| finish(run, candidate(2, 4096), limits()));
    assert!(result.is_err());
    assert_eq!(history.files(), before);
}

#[test]
fn each_original_floor_and_guard_still_qualifies_the_report() {
    for mediated in [false, true] {
        for lane in LANES {
            let history =
                DeniedHistory::build(lane, mediated, Admission::Denied, true, false, false);
            let before = history.files();
            replay(&history, candidate(2, 4096), limits()).unwrap();
            for field in 0..10 {
                let mut wrong = history.expected.clone();
                match field {
                    0 => wrong.oversight.minimum.journal_revision += 1,
                    1 => wrong.oversight.minimum.control_sequence += 1,
                    2 => wrong.oversight.minimum.authority_epoch += 1,
                    3 => wrong.oversight.credential_epoch = None,
                    4 => {
                        wrong.oversight.credential_epoch = Some(FileCredentialEpoch {
                            generation: 1,
                            revoked: false,
                        })
                    }
                    5 => wrong.oversight.guards.campaigns = None,
                    6 => wrong.oversight.guards.identity = None,
                    7 => wrong.oversight.guards.source = None,
                    8 => wrong.oversight.guards.stream = None,
                    _ => {
                        wrong.oversight.effective_policy = candidate(2, 1);
                    }
                }
                rejects_guard(&history, &wrong);
            }
            if history.expected.evaluation.is_some() {
                let mut wrong = history.expected.clone();
                wrong.evaluation = None;
                rejects_guard(&history, &wrong);
            }
            if mediated {
                for field in 0..3 {
                    let mut wrong = history.expected.clone();
                    match field {
                        0 => wrong.topology.available = !wrong.topology.available,
                        1 => wrong.topology.initial = graph(2, false),
                        _ => wrong.topology.current = graph(2, false),
                    }
                    rejects_guard(&history, &wrong);
                }
            }
            if lane != Lane::Plain {
                let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
                let mode = if lane == Lane::Raw { Mode::Raw } else { Mode::Owned };
                let other = recipe(&model, source(&model), mode, false);
                let mut wrong = history.expected.clone();
                wrong.prediction = Some(prediction(&other));
                rejects_guard(&history, &wrong);
            }
            replay(&history, candidate(2, 4096), limits()).unwrap();
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn foreign_anchors_failed_suffix_and_changed_canonical_bytes_never_release_a_report() {
    for mediated in [false, true] {
        for lane in LANES {
            let history = DeniedHistory::new(lane, mediated, Admission::Denied);
            let before = history.files();
            let foreign = DeniedHistory::new(lane, mediated, Admission::Denied);
            assert!(
                history
                    .begin_with(&history.expected, &history.config, &foreign.anchor)
                    .is_err()
            );
            let path = history.root.store().join("delivery.bin");
            let canonical = history.root.bytes();
            assert_eq!(&canonical[canonical.len() - 8..], &2_u64.to_le_bytes());
            let mut invalid = canonical.clone();
            let end = invalid.len();
            invalid[end - 8..].copy_from_slice(&0_u64.to_le_bytes());
            std::fs::write(&path, &invalid).unwrap();
            let mut failed = history.begin().unwrap();
            while failed.progress().status == FileLearnedRecoveryStatus::Replaying {
                if failed.advance().is_err() {
                    break;
                }
            }
            assert_eq!(
                failed.progress().status,
                FileLearnedRecoveryStatus::Failed(Error::Stale)
            );
            assert_eq!(
                finish(failed, candidate(2, 4096), limits()).unwrap_err(),
                Error::Stale.into()
            );
            assert_eq!(history.root.bytes(), invalid);
            std::fs::write(&path, &canonical).unwrap();
            let run = history.begin().unwrap().ready().unwrap();
            let mut changed = canonical.clone();
            *changed.last_mut().unwrap() ^= 1;
            std::fs::write(&path, &changed).unwrap();
            assert_eq!(
                finish(run, candidate(2, 4096), limits()).unwrap_err(),
                Error::Binding.into()
            );
            assert_eq!(history.root.bytes(), changed);
            std::fs::write(&path, &canonical).unwrap();
            replay(&history, candidate(2, 4096), limits()).unwrap();
            assert_eq!(history.files(), before);
        }
    }
}

#[test]
fn full_ordinary_capacity_and_unusable_staging_are_observable_without_cleanup() {
    let history = DeniedHistory::build(Lane::Plain, true, Admission::Denied, false, false, true);
    let before = history.files();
    let result = replay(&history, candidate(2, 4096), limits()).unwrap();
    assert_eq!(result.0, history.journal);
    assert_eq!(history.files(), before);
    let staging = history.root.store().join("delivery.pending");
    std::fs::remove_file(&staging).unwrap();
    std::fs::create_dir(&staging).unwrap();
    let before = history.files();
    assert_eq!(replay(&history, candidate(2, 4096), limits()).unwrap(), result);
    assert!(staging.is_dir());
    assert_eq!(history.files(), before);
}
