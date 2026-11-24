//! Recover the original numerical owner and its complete independent accounting.
//! Every history here is produced by real public generation/review/file APIs.
use super::*;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::monitoring::restart::KvRestartBudget, model::MAX_MODEL_KV_VALUES,
};
use fa_reference::action::consequence::delivery::{
    persistent::observed::{
        containment::FileResetRequest,
        decoder::learned::{
            FileLearnedPublicationProfile, FileLearnedRecoveryStatus,
            checkpoint::FileLearnedResetIntent,
        },
        guarded::{
            anchored::FileHistoryAnchor,
            evaluation::andon::{FileAndonRecoveryRequirements, learned::FileAndonLearnedRecovery},
        },
    },
    publication_gate::PublicationLimits,
};
use fa_reference::action::consequence::gate::ReviewBinding;
use fa_reference::witness::refinement::RefinementBudget;
use std::cell::Cell;

fn retained(host: &FileOversight, guards: FileGuardSet) -> FileAndonRecoveryRequirements {
    FileAndonRecoveryRequirements::new(
        requirements(host, guards),
        host.andon_config().unwrap().unwrap().clone(),
        host.andon_ledger()
            .unwrap()
            .windows()
            .values()
            .map(|window| window.budget)
            .collect(),
        host.andon_source().unwrap().revision(),
    )
    .unwrap()
}

fn begin(
    root: &Directory,
    expected: &FileAndonRecoveryRequirements,
    recipe: &FileLearnedConfig,
    anchor: &FileHistoryAnchor,
) -> Result<FileAndonLearnedRecovery, JournalError> {
    FileOversight::begin_open_andon_guarded_anchored_with_learned_generation(
        root.store(),
        profile(),
        expected,
        &protocol(),
        recipe,
        anchor,
    )
}

fn drive(run: &mut FileAndonLearnedRecovery) -> Result<(), JournalError> {
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = run.progress();
        let after = run.advance(before.replayed_events, 1)?;
        assert_eq!(after.replayed_events, before.replayed_events + 1);
        assert_eq!(after.total_events, before.total_events);
    }
    Ok(())
}

fn label(
    host: &mut FileOversight,
    roles: &FileEvaluatedOversightRoles,
    round: u64,
    evidence: u8,
    truth: GroundTruth,
) {
    let ticket = host.evaluation_ticket(round).unwrap();
    let revision = host.revision();
    assert!(
        roles
            .evaluator
            .assess(
                host,
                revision,
                &ticket,
                Assessment {
                    origin: 201,
                    evidence_id: [evidence; 32],
                    truth,
                }
            )
            .unwrap()
    );
}

fn later_window(host: &mut FileOversight) {
    host.register_andon_window(
        host.revision(),
        AndonBudget {
            window: 2,
            start: 100,
            end: 200,
            false_stops_allowed: 2,
            cost_per_stop: 9,
        },
    )
    .unwrap();
}

#[test]
fn one_event_replay_preserves_numerics_all_accounting_and_capacity_before_one_fence() {
    let root = Directory::new();
    let recipe = config(0);
    let (mut host, old) = accounted(&root, recipe.clone(), &guards());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    fresh_identity(&mut host, &old.oversight, 1, 1);
    for (attempt, round, truth) in [
        (1, 101, GroundTruth::Benign),
        (2, 102, GroundTruth::Censored),
    ] {
        let (_, input) = staged(&mut host, attempt);
        reviewed(&mut host, attempt, round, &input, Verdict::Hold).unwrap();
        label(
            &mut host,
            &old,
            round,
            u8::try_from(attempt).unwrap(),
            truth,
        );
    }
    later_window(&mut host);
    let old_ticket = host.evaluation_ticket(102).unwrap();
    let expected = retained(&host, guards());
    let anchor = host.history_anchor().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    assert!(numerical.work.admitted_tokens > 0);
    let accounting = host.andon_ledger().unwrap().clone();
    assert_eq!(accounting.windows()[&1].total_cost, 7);
    assert_eq!(
        accounting.windows()[&1].incident.as_ref().unwrap().rounds,
        BTreeSet::from([101])
    );
    let report = host.credibility_report().unwrap();
    assert_eq!(report.retained_cases, 2);
    assert_eq!(report.censored_cases, 1);
    let labels = host.evaluation_history(102).unwrap().to_vec();
    let receipts = host
        .andon_source()
        .unwrap()
        .receipts()
        .iter()
        .map(|receipt| (**receipt).clone())
        .collect::<Vec<_>>();
    let capacity = host.journal_capacity().unwrap();
    let bytes = root.bytes();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged staging").unwrap();
    assert_eq!(
        begin(&root, &expected, &recipe, &anchor)
            .unwrap()
            .finish()
            .err(),
        Some(Error::Incomplete.into())
    );
    assert_eq!(root.bytes(), bytes);
    assert!(pending.exists());
    let mut run = begin(&root, &expected, &recipe, &anchor)
        .unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal())
        .unwrap();
    assert_eq!(run.progress().replayed_events, 0);
    assert_eq!(run.advance(1, 1).err(), Some(Error::Stale.into()));
    assert_eq!(run.advance(0, 0).err(), Some(Error::InvalidInput.into()));
    assert_eq!(
        begin(&root, &expected, &recipe, &anchor).err(),
        Some(JournalError::Busy)
    );
    drive(&mut run).unwrap();
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    assert_eq!(root.bytes(), bytes);
    assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging");
    // Even a fully verified candidate remains a nonmutating cancellation point.
    drop(run);
    assert_eq!(root.bytes(), bytes);
    assert!(pending.exists());
    let mut run = begin(&root, &expected, &recipe, &anchor).unwrap();
    drive(&mut run).unwrap();
    let (mut host, fresh) = run.finish().unwrap();
    assert_eq!(
        host.revision(),
        expected.base().minimum.journal_revision + 1
    );
    assert_eq!(
        host.history_anchor_after(&anchor).unwrap().revision(),
        host.revision()
    );
    assert!(!pending.exists());
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        numerical
    );
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    assert_eq!(host.andon_ledger().unwrap(), &accounting);
    assert_eq!(
        host.andon_source().unwrap().revision(),
        expected.minimum_evaluation_revision()
    );
    assert_eq!(
        host.andon_source()
            .unwrap()
            .receipts()
            .iter()
            .map(|receipt| (**receipt).clone())
            .collect::<Vec<_>>(),
        receipts
    );
    assert_eq!(host.credibility_report().unwrap(), report);
    assert_eq!(host.evaluation_history(102).unwrap(), labels);
    let remaining = host.journal_capacity().unwrap();
    assert_eq!(remaining.limits(), capacity.limits());
    assert_eq!(remaining.reserve(), capacity.reserve());
    assert_eq!(remaining.used().events, capacity.used().events + 1);
    assert!(remaining.used().bytes > capacity.used().bytes);
    assert_eq!(
        remaining.ordinary_remaining().events + 1,
        capacity.ordinary_remaining().events
    );
    assert!(fresh.oversight.identity_observer.is_some());
    assert!(fresh.oversight.policy_governor.is_some());
    fresh.evaluator.check_owner(&host).unwrap();
    assert_eq!(
        old.evaluator.check_owner(&host).err(),
        Some(Error::Binding.into())
    );
    assert!(
        host.resume_learned_generation(
            host.revision(),
            numerical.actor_revision,
            numerical.position
        )
        .is_err()
    );
    let ticket = host.evaluation_ticket(102).unwrap();
    let benign = Assessment {
        origin: 201,
        evidence_id: [9; 32],
        truth: GroundTruth::Benign,
    };
    let revision = host.revision();
    let fenced = root.bytes();
    assert_eq!(
        fresh
            .evaluator
            .assess(&mut host, revision, &old_ticket, benign)
            .err(),
        Some(Error::Binding.into())
    );
    assert!(
        fresh
            .evaluator
            .assess(&mut host, revision, &ticket, benign)
            .is_err()
    );
    assert_eq!(
        root.bytes(),
        fenced,
        "a fresh evaluator cannot supply a clock"
    );
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let revision = host.revision();
    assert!(
        fresh
            .evaluator
            .assess(&mut host, revision, &ticket, benign)
            .unwrap()
    );
    let window = &host.andon_ledger().unwrap().windows()[&1];
    assert_eq!(window.false_stops, 2);
    assert_eq!(window.total_cost, 14);
    assert_eq!(window.incident.as_ref().unwrap().opened_by_round, 101);
    assert_eq!(
        window.incident.as_ref().unwrap().rounds,
        BTreeSet::from([101, 102])
    );
    assert_eq!(
        host.andon_ledger().unwrap().windows()[&2],
        accounting.windows()[&2]
    );
    let labelled = root.bytes();
    let revision = host.revision();
    assert!(
        !fresh
            .evaluator
            .assess(&mut host, revision, &ticket, benign)
            .unwrap()
    );
    assert_eq!(root.bytes(), labelled);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        numerical
    );
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn fresh_two_key_continuation_keeps_unknown_liability_and_rejects_old_roles() {
    let root = Directory::new();
    let recipe = config(0);
    let (mut host, old) = accounted(&root, recipe.clone(), &guards());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    fresh_identity(&mut host, &old.oversight, 1, 1);
    let (old_action, old_input, old_key, old_request) = prepared(&mut host, 1, 101);
    label(&mut host, &old, 101, 1, GroundTruth::Benign);
    let revision = host.revision();
    let old_human = old
        .oversight
        .human
        .approve(&mut host, revision, &old_request)
        .unwrap();
    host.dispatch(
        host.revision(),
        &old_key,
        &old_human,
        &old_action,
        &old_input,
        snapshot(),
    )
    .unwrap();
    let expected = retained(&host, guards());
    let anchor = host.history_anchor().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let report = host.credibility_report().unwrap();
    drop(host);
    let (mut host, fresh) = FileOversight::open_andon_guarded_anchored_with_learned_generation(
        root.store(),
        profile(),
        &expected,
        &protocol(),
        &recipe,
        &anchor,
    )
    .unwrap();
    assert_eq!(
        host.revision(),
        expected.base().minimum.journal_revision + 1
    );
    assert_eq!(host.inspect().control.ledger.charged, 16);
    assert_eq!(
        host.inspect().control.ledger.stages[&1],
        ActionState::Unknown
    );
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.credibility_report().unwrap(), report);
    assert!(!host.clock_ready());
    assert_eq!(host.identity_status().err(), Some(Error::Incomplete.into()));
    assert!(
        host.dispatch(
            host.revision(),
            &old_key,
            &old_human,
            &old_action,
            &old_input,
            snapshot()
        )
        .is_err()
    );
    assert!(
        host.resume_learned_generation(
            host.revision(),
            numerical.actor_revision,
            numerical.position
        )
        .is_err()
    );
    resume(&mut host, 2);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        numerical
    );
    assert_eq!(
        host.reconcile(host.revision(), 1).unwrap(),
        Reconciliation::AwaitingResolution
    );
    assert_eq!(host.inspect().control.ledger.charged, 16);
    let (action, input) = staged(&mut host, 2);
    assert_eq!(
        reviewed(&mut host, 2, 102, &input, Verdict::Allow).err(),
        Some(Error::Incomplete.into())
    );
    assert!(
        host.authorize(host.revision(), 2, &input, snapshot())
            .is_err()
    );
    fresh_identity(&mut host, &fresh.oversight, 2, 2);
    reviewed(&mut host, 2, 103, &input, Verdict::Allow).unwrap();
    let ticket = host.evaluation_ticket(103).unwrap();
    let revision = host.revision();
    assert_eq!(
        old.evaluator
            .assess(
                &mut host,
                revision,
                &ticket,
                Assessment {
                    origin: 201,
                    evidence_id: [2; 32],
                    truth: GroundTruth::Benign,
                }
            )
            .err(),
        Some(Error::Binding.into())
    );
    label(&mut host, &fresh, 103, 2, GroundTruth::Benign);
    let key = host
        .authorize(host.revision(), 2, &input, snapshot())
        .unwrap();
    let request = host
        .request_human_approval(host.revision(), 1002, 2, &input, ElapsedTick(30))
        .unwrap();
    let revision = host.revision();
    let before = root.bytes();
    assert!(
        old.oversight
            .human
            .approve(&mut host, revision, &request)
            .is_err()
    );
    assert_eq!(root.bytes(), before);
    assert!(
        host.publish_checked(host.revision(), 2, Some(&input), snapshot(), ElapsedTick(2))
            .is_err()
    );
    let revision = host.revision();
    let human = fresh
        .oversight
        .human
        .approve(&mut host, revision, &request)
        .unwrap();
    host.dispatch(host.revision(), &key, &human, &action, &input, snapshot())
        .unwrap();
    assert_eq!(
        host.publish_checked(host.revision(), 2, Some(&input), snapshot(), ElapsedTick(2))
            .unwrap()
            .outcome,
        EndpointOutcome::Executed {
            resulting_version: 2
        }
    );
    host.reconcile(host.revision(), 2).unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().payload, b"visible");
    assert_eq!(host.inspect().control.ledger.charged, 32);
    assert_eq!(
        host.inspect().control.ledger.stages[&1],
        ActionState::Unknown
    );
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
}

#[test]
fn wrong_accounting_window_protocol_and_recipe_refuse_before_replay_or_cleanup() {
    let root = Directory::new();
    let recipe = config(0);
    let (mut host, _) = accounted(&root, recipe.clone(), &guards());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    later_window(&mut host);
    let expected = retained(&host, guards());
    let anchor = host.history_anchor().unwrap();
    let bytes = root.bytes();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged staging").unwrap();
    for field in 0..7 {
        let mut identity = expected.config().identity();
        let mut initial = expected.config().windows().to_vec();
        let mut windows = expected.windows().to_vec();
        let mut evaluation = protocol();
        let mut selected = recipe.clone();
        match field {
            0 => identity.deployment += 1,
            1 => identity.evaluator += 1,
            2 => {
                initial[0].cost_per_stop += 1;
                windows[0] = initial[0];
            }
            3 => {
                windows.pop();
            }
            4 => windows[1].false_stops_allowed += 1,
            5 => evaluation.period += 1,
            _ => selected = config(1),
        }
        let changed = FileAndonRecoveryRequirements::new(
            expected.base().clone(),
            FileAndonConfig::new(identity, initial).unwrap(),
            windows,
            expected.minimum_evaluation_revision(),
        )
        .unwrap();
        assert_eq!(
            FileOversight::begin_open_andon_guarded_anchored_with_learned_generation(
                root.store(),
                profile(),
                &changed,
                &evaluation,
                &selected,
                &anchor,
            )
            .err(),
            Some(Error::Binding.into()),
            "field {field}"
        );
        assert_eq!(root.bytes(), bytes, "field {field}");
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging");
    }
    let mut reserve = RecoveryReserve::terminal();
    reserve.events += 1;
    assert_eq!(
        begin(&root, &expected, &recipe, &anchor)
            .unwrap()
            .require_recovery_reserve(reserve)
            .err(),
        Some(Error::Binding.into())
    );
    assert_eq!(root.bytes(), bytes);
    assert!(pending.exists());
    let mut run = begin(&root, &expected, &recipe, &anchor).unwrap();
    drive(&mut run).unwrap();
    let (host, _) = run.finish().unwrap();
    assert_eq!(
        host.revision(),
        expected.base().minimum.journal_revision + 1
    );
    assert_eq!(host.andon_ledger().unwrap().windows().len(), 2);
}

#[test]
fn checked_publication_contract_cannot_be_omitted_or_changed_during_recovery() {
    let root = Directory::new();
    let limits = PublicationLimits {
        bindings: 4,
        validation: RefinementBudget {
            steps: 32,
            value_bytes: 128,
        },
    };
    let recipe = config(0)
        .with_required_publication_validation(FileLearnedPublicationProfile { limits, feed: None })
        .unwrap();
    let (mut host, _) = accounted(&root, recipe.clone(), &guards());
    assert_eq!(host.publication_validation_profile().unwrap(), Some(limits));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    let absent = retained(&host, guards());
    let expected = absent.clone().require_publication(limits).unwrap();
    let mut changed = limits;
    changed.validation.steps += 1;
    let wrong = absent.clone().require_publication(changed).unwrap();
    let anchor = host.history_anchor().unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let bytes = root.bytes();
    drop(host);
    for contract in [&absent, &wrong] {
        assert_eq!(
            begin(&root, contract, &recipe, &anchor).err(),
            Some(Error::Binding.into())
        );
        assert_eq!(root.bytes(), bytes);
    }
    assert_eq!(
        begin(&root, &expected, &config(0), &anchor).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(root.bytes(), bytes);
    let mut run = begin(&root, &expected, &recipe, &anchor).unwrap();
    drive(&mut run).unwrap();
    let (mut host, fresh) = run.finish().unwrap();
    fresh.evaluator.check_owner(&host).unwrap();
    assert_eq!(host.publication_validation_profile().unwrap(), Some(limits));
    assert_eq!(
        host.learned_generation_inspection().unwrap().numerical,
        numerical
    );
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    assert!(
        host.propose(host.revision(), 1, spec(&host), snapshot())
            .is_err()
    );
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn full_guard_and_future_floor_failures_latch_before_any_role_or_terminal_clock() {
    let root = Directory::new();
    let recipe = config(0);
    let (mut host, _) = accounted(&root, recipe.clone(), &guards());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    let expected = retained(&host, guards());
    let anchor = host.history_anchor().unwrap();
    let bytes = root.bytes();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged staging").unwrap();
    let mut correct = begin(&root, &expected, &recipe, &anchor).unwrap();
    drive(&mut correct).unwrap();
    let stop = correct.stop_request(990).unwrap();
    drop(correct);
    for field in 0..8 {
        let mut base = expected.base().clone();
        let mut evaluation_floor = expected.minimum_evaluation_revision();
        match field {
            0 => base.guards.identity = None,
            1 => base.guards.campaigns = None,
            2 => base.guards.identity.as_mut().unwrap().policy.validity_ticks += 1,
            3 => base.minimum.journal_revision += 1,
            4 => base.minimum.control_sequence += 1,
            5 => base.minimum.authority_epoch += 1,
            6 => {
                base.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap()
            }
            _ => evaluation_floor += 1,
        }
        let changed = FileAndonRecoveryRequirements::new(
            base,
            expected.config().clone(),
            expected.windows().to_vec(),
            evaluation_floor,
        )
        .unwrap();
        let mut run = begin(&root, &changed, &recipe, &anchor).unwrap();
        assert!(drive(&mut run).is_err(), "field {field}");
        let FileLearnedRecoveryStatus::Failed(cause) = run.progress().status else {
            panic!("a complete-contract mismatch must never expose Ready: field {field}");
        };
        assert_eq!(
            cause,
            if matches!(field, 3..=5 | 7) {
                Error::Stale
            } else {
                Error::Binding
            },
            "field {field}"
        );
        assert_eq!(run.progress().replayed_events, run.progress().total_events);
        assert_eq!(
            run.advance(run.progress().replayed_events, 1).err(),
            Some(cause.into())
        );
        assert_eq!(run.stop_request(990).err(), Some(cause.into()));
        if field % 2 == 0 {
            assert_eq!(run.finish().err(), Some(cause.into()));
        } else {
            let clock_calls = Cell::new(0);
            assert_eq!(
                run.try_finish_stopped_with_anchor(stop, || {
                    clock_calls.set(clock_calls.get() + 1);
                    Ok(ElapsedTick(2))
                })
                .err(),
                Some(cause.into())
            );
            assert_eq!(clock_calls.get(), 0);
        }
        assert_eq!(root.bytes(), bytes, "field {field}");
        assert_eq!(std::fs::read(&pending).unwrap(), b"unacknowledged staging");
    }
    let (host, _) = FileOversight::open_andon_guarded_anchored_with_learned_generation(
        root.store(),
        profile(),
        &expected,
        &protocol(),
        &recipe,
        &anchor,
    )
    .unwrap();
    assert_eq!(
        host.revision(),
        expected.base().minimum.journal_revision + 1
    );
    assert!(!pending.exists());
}

#[test]
fn valid_equal_counter_forks_still_require_the_independently_retained_prefix() {
    let root = Directory::new();
    let recipe = config(0);
    let (host, _) = accounted(&root, recipe.clone(), &guards());
    let initial = retained(&host, guards());
    let initial_anchor = host.history_anchor().unwrap();
    let bootstrap = root.bytes();
    drop(host);
    let (mut left, _) = FileOversight::open_andon_guarded_anchored_with_learned_generation(
        root.store(),
        profile(),
        &initial,
        &protocol(),
        &recipe,
        &initial_anchor,
    )
    .unwrap();
    resume(&mut left, 1);
    step(&mut left).unwrap();
    let anchor = left.history_anchor().unwrap();
    let expected = retained(&left, guards());
    let left_view = left.inspect();
    let left_bytes = root.bytes();
    drop(left);
    // Restore a real acknowledged image to produce a second lawful history at
    // the same original path. No event, witness or numerical result is forged.
    std::fs::write(root.store().join("delivery.bin"), bootstrap).unwrap();
    let (mut right, _) = FileOversight::open_andon_guarded_anchored_with_learned_generation(
        root.store(),
        profile(),
        &initial,
        &protocol(),
        &recipe,
        &initial_anchor,
    )
    .unwrap();
    resume(&mut right, 2);
    step(&mut right).unwrap();
    let right_view = right.inspect();
    let right_anchor = right.history_anchor().unwrap();
    let right_bytes = root.bytes();
    drop(right);
    assert_eq!(right_view.revision, left_view.revision);
    assert_eq!(right_view.control.sequence, left_view.control.sequence);
    assert_eq!(
        right_view.control.ledger.epoch,
        left_view.control.ledger.epoch
    );
    assert_ne!(
        right_view.control.ledger.elapsed,
        left_view.control.ledger.elapsed
    );
    assert_ne!(right_bytes, left_bytes);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged fork staging").unwrap();
    // Full original replay with the matching independent anchor accepts the
    // other fork. Equal counters cannot make the retained left anchor accept it.
    let mut valid = begin(&root, &expected, &recipe, &right_anchor).unwrap();
    drive(&mut valid).unwrap();
    assert_eq!(valid.progress().status, FileLearnedRecoveryStatus::Ready);
    drop(valid);
    assert_eq!(
        begin(&root, &expected, &recipe, &anchor).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(root.bytes(), right_bytes);
    assert_eq!(
        std::fs::read(&pending).unwrap(),
        b"unacknowledged fork staging"
    );
    std::fs::write(root.store().join("delivery.bin"), left_bytes).unwrap();
    let (host, _) = FileOversight::open_andon_guarded_anchored_with_learned_generation(
        root.store(),
        profile(),
        &expected,
        &protocol(),
        &recipe,
        &anchor,
    )
    .unwrap();
    assert_eq!(host.revision(), left_view.revision + 1);
    assert!(!pending.exists());
}

#[test]
fn exact_pending_reset_preserves_accounting_and_issues_fresh_roles_after_completion() {
    let root = Directory::new();
    let recipe = config(0);
    let (mut host, old) = accounted(&root, recipe.clone(), &guards());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let saved = host
        .capture_learned_checkpoint(
            host.revision(),
            1,
            numerical.actor_revision,
            host.inspect().control.ledger.epoch,
        )
        .unwrap();
    step(&mut host).unwrap();
    fresh_identity(&mut host, &old.oversight, 1, 1);
    let (_, input) = staged(&mut host, 1);
    reviewed(&mut host, 1, 101, &input, Verdict::Hold).unwrap();
    label(&mut host, &old, 101, 1, GroundTruth::Benign);
    later_window(&mut host);
    let control = host.inspect().control;
    let intent = FileLearnedResetIntent::for_recovery(
        saved.id(),
        FileResetRequest {
            operation: 900,
            expected_control_sequence: control.sequence,
            expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
            expected_authority_epoch: control.ledger.epoch,
            binding: ReviewBinding {
                round: 900,
                reducer_generation: 1,
                evidence_root: [8; 32],
            },
            retained_targets: vec![host.inspect().target],
        },
        KvRestartBudget {
            cache_values: MAX_MODEL_KV_VALUES,
            audit: source(0).policy.allowance(),
        },
    )
    .unwrap();
    host.begin_learned_reset(
        host.revision(),
        &saved,
        intent.control().clone(),
        intent.budget(),
    )
    .unwrap();
    let expected = retained(&host, guards());
    let anchor = host.history_anchor().unwrap();
    let report = host.credibility_report().unwrap();
    let ledger = host.andon_ledger().unwrap().clone();
    let bytes = root.bytes();
    drop(host);
    let pending = root.store().join("delivery.pending");
    std::fs::write(&pending, b"unacknowledged reset staging").unwrap();
    let mut wrong_control = intent.control().clone();
    wrong_control.operation += 1;
    let wrong =
        FileLearnedResetIntent::for_recovery(saved.id(), wrong_control, intent.budget()).unwrap();
    let mut run = begin(&root, &expected, &recipe, &anchor).unwrap();
    drive(&mut run).unwrap();
    assert!(run.finish_pending_reset(&wrong).is_err());
    assert_eq!(root.bytes(), bytes);
    assert_eq!(
        std::fs::read(&pending).unwrap(),
        b"unacknowledged reset staging"
    );
    let mut run = begin(&root, &expected, &recipe, &anchor).unwrap();
    drive(&mut run).unwrap();
    let (host, fresh) = run.finish_pending_reset(&intent).unwrap();
    assert_eq!(
        host.revision(),
        expected.base().minimum.journal_revision + 2
    );
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert!(
        host.learned_reset_result(900)
            .unwrap()
            .unwrap()
            .control
            .restored
    );
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    assert_eq!(host.andon_ledger().unwrap(), &ledger);
    assert_eq!(
        host.andon_source().unwrap().revision(),
        expected.minimum_evaluation_revision()
    );
    assert_eq!(host.credibility_report().unwrap(), report);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(!host.clock_ready());
    assert!(host.check_learned_checkpoint(&saved).is_err());
    assert!(!pending.exists());
    fresh.evaluator.check_owner(&host).unwrap();
    assert_eq!(
        old.evaluator.check_owner(&host).err(),
        Some(Error::Binding.into())
    );
    assert!(fresh.oversight.identity_observer.is_some());
    assert!(fresh.oversight.policy_governor.is_some());
}

#[test]
fn clock_free_admission_stop_preserves_unknown_cost_and_exact_retries() {
    let root = Directory::new();
    let recipe = config(0);
    let (mut host, old) = accounted(&root, recipe.clone(), &guards());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    step(&mut host).unwrap();
    fresh_identity(&mut host, &old.oversight, 1, 1);
    let (action, input, key, request) = prepared(&mut host, 1, 101);
    label(&mut host, &old, 101, 1, GroundTruth::Benign);
    let revision = host.revision();
    let human = old
        .oversight
        .human
        .approve(&mut host, revision, &request)
        .unwrap();
    host.dispatch(host.revision(), &key, &human, &action, &input, snapshot())
        .unwrap();
    let expected = retained(&host, guards());
    let anchor = host.history_anchor().unwrap();
    let bytes = root.bytes();
    drop(host);
    let mut run = begin(&root, &expected, &recipe, &anchor)
        .unwrap()
        .require_recovery_reserve(RecoveryReserve::terminal())
        .unwrap();
    assert_eq!(run.stop_request(990).err(), Some(Error::Incomplete.into()));
    drive(&mut run).unwrap();
    let request = run.stop_request(990).unwrap();
    assert_eq!(root.bytes(), bytes);
    let stopped = run.finish_admission_stop(request).unwrap();
    assert_eq!(stopped.appended_events(), 2);
    assert_eq!(
        stopped.snapshot().revision,
        expected.base().minimum.journal_revision + 2
    );
    assert_eq!(
        stopped.history_anchor().revision(),
        stopped.snapshot().revision
    );
    assert_eq!(stopped.snapshot().control.ledger.charged, 16);
    assert_eq!(
        stopped.snapshot().control.ledger.stages[&1],
        ActionState::Unknown
    );
    assert_eq!(stopped.snapshot().executions, 0);
    assert_eq!(
        stopped.snapshot().control.ledger.elapsed,
        Some(ElapsedTick(1))
    );
    let stopped_bytes = root.bytes();
    let mut retry = begin(&root, &expected, &recipe, stopped.history_anchor()).unwrap();
    drive(&mut retry).unwrap();
    let request = retry.stop_request(990).unwrap();
    let confirmed = retry.finish_admission_stop(request).unwrap();
    assert_eq!(confirmed.appended_events(), 0);
    assert_eq!(confirmed.snapshot(), stopped.snapshot());
    assert_eq!(confirmed.history_anchor(), stopped.history_anchor());
    assert_eq!(root.bytes(), stopped_bytes);
}
