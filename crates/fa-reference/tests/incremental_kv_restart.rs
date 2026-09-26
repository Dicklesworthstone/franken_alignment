//! Actual original decoder/codec/monitor execution; these fixtures are not a
//! trained-model, production-resource or detector-qualification campaign.
#[path = "support/restart_model.rs"]
mod support;
use support::*;
use fa_reference::Error;
use fa_reference::action::consequence::activation::monitor::MonitorOutcome;
use fa_reference::action::consequence::activation::tensor::kv::{MAX_KV_POSITIONS,
    decoder::{DecoderBudget, MAX_DECODER_PRODUCTS,
        monitoring::{LearnedDecoderStatus, restart::{KvRestartBudget,
            incremental::{IncrementalRestartStatus, IncrementalRestartWork}}}},
};

#[test]
fn one_position_policy_restarts_a_long_prefix_with_exact_audits_and_continuation() {
    let model = model();
    for retention in 0..3 {
        let policy = policy(&model, 0, retention);
        let mut original = model.monitored_session(21, 201, policy.clone()).unwrap();
        let mut events = Vec::new();
        for (position, token) in [0, 1, 2, 0, 2, 1].into_iter().enumerate() {
            let event = original.advance(position as u64, token).unwrap();
            assert!(event.step().is_some());
            events.push(event);
        }
        let saved = original.checkpoint_kv(capture_limit()).unwrap();
        // The old all-at-once contract is deliberately unchanged.
        assert!(matches!(saved.begin_restart(22, KvRestartBudget {
            cache_values: saved.cache_values(), audit: policy.allowance(),
        }), Err(Error::Limit)));
        let mut restart = saved.begin_incremental_restart(22, budget(&policy, 6)).unwrap();
        assert_eq!(restart.source(), original.accepted_cache_image().unwrap().descriptor());
        assert_eq!(restart.position_count(), 6);
        assert!(!restart.is_ready());
        let reserved = restart.reservation();
        assert_eq!(reserved.monitor_probe_coordinates, policy.allowance().monitoring.probe_coordinates as u64 * 6);
        for (position, original_event) in events.iter().enumerate() {
            let audit = restart.advance(position as u64).unwrap();
            assert!(audit.monitoring().complete_quiet());
            assert_eq!(audit.monitoring().planned_rows(), 4);
            assert_eq!(audit.monitoring().first_position(), position as u64);
            assert_eq!(audit.monitoring().end_position(), position as u64 + 1);
            assert_eq!(audit.compression(), original_event.compression());
            assert_eq!(audit.monitoring().source().descriptor(), original_event.audit().source().descriptor());
            assert_eq!(audit.monitoring().work(), original_event.audit().work());
            assert_eq!(restart.next_position(), position as u64 + 1);
            assert_eq!(restart.work().attempted_positions, position + 1);
            assert_eq!(restart.work().reported_positions, position + 1);
            assert_eq!(restart.work().quiet_positions, position + 1);
            assert_eq!(restart.reservation(), reserved);
        }
        assert!(restart.is_ready());
        assert_eq!(restart.advance(6).err(), Some(Error::WrongState));
        let total = restart.work();
        assert_eq!(total.source_values_recaptured, saved.cache_values());
        assert_eq!(total.reported.source_check_values, saved.cache_values() as u64);
        assert!(total.reported.monitor_probe_coordinates <= reserved.monitor_probe_coordinates);
        let (mut resumed, receipt) = restart.finish().unwrap();
        assert_eq!(receipt.work(), total);
        assert_eq!(receipt.reservation(), reserved);
        assert_eq!(receipt.evaluation_origin(), 201);
        assert_eq!(receipt.restoration().source, original.accepted_cache_image().unwrap().descriptor());
        assert_eq!(receipt.restoration().bytes_written, saved.cache_values() * 4);
        assert!(resumed.last_event().is_none());
        same_cache(&original.accepted_cache_image().unwrap(), &resumed.accepted_cache_image().unwrap());
        for (position, token) in [(6, 2), (7, 0)] {
            let a = original.advance(position, token).unwrap();
            let b = resumed.advance(position, token).unwrap();
            assert_eq!(logits(&a.step().unwrap().logits), logits(&b.step().unwrap().logits));
            same_cache(&original.accepted_cache_image().unwrap(), &resumed.accepted_cache_image().unwrap());
        }
        let independent = model.recompute(90, &[0, 1, 2, 0, 2, 1, 2, 0],
            DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
        assert_eq!(logits(independent.logits().unwrap()), logits(resumed.accepted_logits().unwrap()));
        same_cache(&independent.cache_image().unwrap(), &resumed.accepted_cache_image().unwrap());
        let no_history = model.recompute(91, &[0], DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap();
        assert_ne!(logits(no_history.logits().unwrap()), logits(resumed.accepted_logits().unwrap()));
    }
}

#[test]
fn no_partial_or_skipped_prefix_can_release_a_session() {
    let model = model();
    let policy = policy(&model, 0, 1);
    let mut original = model.monitored_session(21, 201, policy.clone()).unwrap();
    for (p, token) in [0, 1, 2].into_iter().enumerate() { original.advance(p as u64, token).unwrap(); }
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    for cut in 0..3 {
        let mut partial = saved.begin_incremental_restart(22, budget(&policy, 3)).unwrap();
        for position in 0..cut { partial.advance(position).unwrap(); }
        let before = partial.work();
        let status = partial.status();
        for wrong in [cut + 1, u64::MAX] {
            assert_eq!(partial.advance(wrong).err(), Some(Error::Stale));
            assert_eq!(partial.work(), before);
            assert_eq!(partial.status(), status);
        }
        if cut > 0 { assert_eq!(partial.advance(cut - 1).err(), Some(Error::Stale)); }
        assert!(!partial.is_ready());
        assert!(matches!(partial.finish(), Err(Error::Incomplete)));
    }
    assert_eq!(original.status(), LearnedDecoderStatus::Active);
    assert_eq!(original.position(), 3);
    let mut complete = saved.begin_incremental_restart(23, budget(&policy, 3)).unwrap();
    for position in 0..3 { complete.advance(position).unwrap(); }
    assert!(complete.finish().is_ok());
}

#[test]
fn source_checks_and_fixed_policy_limits_are_not_bypassed_by_incremental_restore() {
    let model = model();
    let policy = policy(&model, 0, 1);
    let mut original = model.monitored_session(21, 201, policy.clone()).unwrap();
    original.advance(0, 0).unwrap();
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let mut short = budget(&policy, 1);
    short.per_position.preparation.source_check.source_values = saved.cache_values() - 1;
    let mut failed = saved.begin_incremental_restart(22, short).unwrap();
    let reservation = failed.reservation();
    assert_eq!(failed.advance(0).err(), Some(Error::Limit));
    assert_eq!(failed.status(), IncrementalRestartStatus::Failed(Error::Limit));
    assert_eq!(failed.work().attempted_positions, 1);
    assert_eq!(failed.work().reported_positions, 0);
    assert_eq!(failed.work().quiet_positions, 0);
    assert!(failed.last_audit().is_none());
    assert_eq!(failed.reservation(), reservation);
    assert_eq!(failed.advance(0).err(), Some(Error::WrongState));
    assert!(matches!(failed.finish(), Err(Error::Incomplete)));
    short.per_position.preparation.source_check.source_values += 1;
    let mut permitted = saved.begin_incremental_restart(23, short).unwrap();
    assert!(permitted.advance(0).unwrap().monitoring().complete_quiet());
    assert!(permitted.finish().is_ok());
    assert_eq!(original.position(), 1);
}

#[test]
fn a_later_refinement_hold_latches_after_an_earlier_quiet_position() {
    let model = model();
    let policy = policy(&model, 1, 1);
    let mut original = model.monitored_session(21, 201, policy.clone()).unwrap();
    let first = original.advance(0, 1).unwrap();
    let second = original.advance(1, 2).unwrap();
    assert!(first.audit().complete_quiet() && second.audit().complete_quiet());
    assert_eq!(first.audit().work().refinements, 0);
    assert!(second.audit().work().refinements > 0);
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let mut limited = budget(&policy, 2);
    limited.per_position.monitoring.refinements = 0;
    let mut restart = saved.begin_incremental_restart(22, limited).unwrap();
    assert!(restart.advance(0).unwrap().monitoring().complete_quiet());
    let held = restart.advance(1).unwrap();
    assert_eq!(held.monitoring().outcome(), MonitorOutcome::BudgetExhausted);
    assert_eq!(restart.status(), IncrementalRestartStatus::Held(MonitorOutcome::BudgetExhausted));
    assert_eq!(restart.next_position(), 1);
    assert_eq!(restart.work().attempted_positions, 2);
    assert_eq!(restart.work().reported_positions, 2);
    assert_eq!(restart.work().quiet_positions, 1);
    assert_eq!(restart.work().source_values_recaptured, saved.cache_values());
    assert_eq!(restart.advance(1).err(), Some(Error::WrongState));
    assert!(matches!(restart.finish(), Err(Error::Incomplete)));
    let mut permitted = saved.begin_incremental_restart(23, budget(&policy, 2)).unwrap();
    permitted.advance(0).unwrap();
    assert!(permitted.advance(1).unwrap().monitoring().complete_quiet());
    assert!(permitted.finish().is_ok());
    assert_eq!(original.status(), LearnedDecoderStatus::Active);
}

#[test]
fn full_prefix_bounds_and_source_split_are_checked_before_audit_work() {
    let model = model();
    let policy = policy(&model, 0, 1);
    let mut original = model.monitored_session(21, 201, policy.clone()).unwrap();
    original.advance(0, 0).unwrap();
    original.advance(1, 1).unwrap();
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    let exact = budget(&policy, 2);
    assert!(matches!(saved.begin_incremental_restart(22, budget(&policy, 1)), Err(Error::Limit)));
    let mut invalid = exact;
    invalid.positions = MAX_KV_POSITIONS + 1;
    assert!(matches!(saved.begin_incremental_restart(22, invalid), Err(Error::Limit)));
    invalid = exact;
    invalid.cache_values = saved.cache_values() - 1;
    assert!(matches!(saved.begin_incremental_restart(22, invalid), Err(Error::Limit)));
    for stream in [0, 21] {
        assert!(matches!(saved.begin_incremental_restart(stream, exact), Err(Error::InvalidInput)));
    }
    assert!(matches!(saved.begin_incremental_restart(11, exact), Err(Error::Duplicate)));
    let permitted = saved.begin_incremental_restart(22, exact).unwrap();
    assert_eq!(permitted.work(), IncrementalRestartWork::default());
    assert_eq!(permitted.source(), original.accepted_cache_image().unwrap().descriptor());
}

#[test]
fn empty_prefix_is_admission_only_and_old_checkpoint_does_not_clear_a_source_hold() {
    let model = model();
    let policy = policy(&model, 2, 1);
    let mut original = model.monitored_session(21, 201, policy.clone()).unwrap();
    let empty = original.checkpoint_kv(capture_limit()).unwrap();
    let mut restart = empty.begin_incremental_restart(22, budget(&policy, 0)).unwrap();
    assert!(restart.is_ready());
    assert!(restart.last_audit().is_none());
    assert_eq!(restart.work(), IncrementalRestartWork::default());
    assert_eq!(restart.reservation().source_check_values, 0);
    assert_eq!(restart.advance(0).err(), Some(Error::WrongState));
    let (mut admitted, receipt) = restart.finish().unwrap();
    assert_eq!(receipt.restoration().bytes_written, 0);
    assert_eq!(admitted.accepted_logits(), Err(Error::Incomplete));
    original.advance(0, 0).unwrap();
    let saved = original.checkpoint_kv(capture_limit()).unwrap();
    assert!(original.advance(1, 2).unwrap().step().is_none());
    assert!(matches!(original.checkpoint_kv(capture_limit()), Err(Error::WrongState)));
    let mut restart = saved.begin_incremental_restart(23, budget(&policy, 1)).unwrap();
    restart.advance(0).unwrap();
    let (mut resumed, _) = restart.finish().unwrap();
    assert!(resumed.advance(1, 2).unwrap().step().is_none());
    assert_eq!(original.status(), LearnedDecoderStatus::Held(MonitorOutcome::Alarm));
    assert_eq!(resumed.status(), original.status());
    assert_eq!(original.advance(1, 0).err(), Some(Error::WrongState));
    assert!(admitted.advance(0, 0).unwrap().step().is_some());
}
