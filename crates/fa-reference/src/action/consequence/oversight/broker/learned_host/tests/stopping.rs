//! Original stop/fence/reconciliation behavior under real learned-host triggers.
use super::*;
use super::super::LearnedHostStopCause;
use crate::action::ActionState;
use crate::action::consequence::oversight::decoder_host::HostedStopPolicy;

fn stop_policy() -> HostedStopPolicy { HostedStopPolicy::new(7, 1, 900).unwrap() }

#[test]
fn quiet_completion_is_not_a_stop_and_preserves_the_original_computation() {
    let model = model(); let config = config(&model, false, 3);
    let mut control = original(&model, config.clone());
    let (mut owner, endpoint, _) = owner(&[], false); attach(&mut owner, &model, config);
    owner.enable_learned_host_stop(stop_policy()).unwrap();
    assert_eq!(owner.enforce_learned_host_stop(), Ok(None));
    while control.status().is_active() {
        control.advance(control.position()).unwrap(); step(&mut owner).unwrap(); same_actor(&owner, &control);
        assert_eq!(owner.enforce_learned_host_stop(), Ok(None));
    }
    assert_eq!(owner.hosted_learned_generation().unwrap().status, GenerationStatus::Finished(GenerationStop::TokenLimit));
    assert!(owner.learned_host_stop_incident().is_none()); assert!(owner.stop_receipt().is_none());
    assert_eq!(step(&mut owner).err(), Some(Error::WrongState));
    assert!(owner.learned_host_stop_incident().is_none()); assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn alarm_cancels_undispatched_work_but_dispatched_outcomes_need_real_endpoint_settlement() {
    for phase in 0..3 {
        let model = model(); let (mut owner, mut endpoint, _) = owner(&[], false);
        attach(&mut owner, &model, config(&model, true, 1)); owner.enable_learned_host_stop(stop_policy()).unwrap();
        step(&mut owner).unwrap();
        let (action, inputs) = proposal(&mut owner, 1); approve(&mut owner, 1, &inputs);
        let permit = owner.authorize(1, Some(&inputs), &snapshot()).unwrap();
        let message = if phase > 0 { Some(owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap()) } else { None };
        if phase == 2 { endpoint.deliver(message.as_ref().unwrap()).unwrap(); } // lose acknowledgment
        let epoch = owner.inspect().ledger.epoch;
        let actor = owner.delivery.controller().actor().clone(); let revision = owner.actor_revision();
        let held = step(&mut owner).unwrap(); assert!(held.accepted().is_none() && held.sample().is_none());
        let incident = owner.learned_host_stop_incident().unwrap();
        assert_eq!(incident.cause(), LearnedHostStopCause::Monitoring(MonitorOutcome::Alarm));
        assert_eq!(incident.stream(), 21); assert_eq!(incident.evaluation_origin(), 201);
        assert_eq!(incident.observed().work.admitted_tokens, 2);
        assert_eq!(incident.observed().sampled_draws, 0);
        assert_eq!(incident.last_stop_error(), None);
        let receipt = incident.stop_receipt().unwrap().clone();
        assert_eq!(receipt.request().operation, 900); assert!(receipt.revocation_floor() > epoch);
        assert!(owner.inspect().suspended); assert_eq!(owner.actor_revision(), revision);
        assert_eq!(owner.delivery.controller().actor(), &actor);
        assert_eq!(receipt.refunded_units(), if phase == 0 { 16 } else { 0 });
        assert_eq!(owner.inspect().ledger.stages[&1], if phase == 0 { ActionState::Cancelled } else { ActionState::Unknown });
        assert_eq!(owner.inspect().ledger.charged, if phase == 0 { 0 } else { 16 });
        assert!(!owner.stop_progress().unwrap().endpoint_fenced);
        let stopped = owner.inspect(); let numerical = owner.hosted_learned_generation().unwrap();
        assert_eq!(owner.enforce_learned_host_stop(), Ok(Some(receipt.clone())));
        assert_eq!(owner.inspect(), stopped); assert_eq!(step(&mut owner).err(), Some(Error::WrongState));
        assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
        let settled = owner.progress_stop(&mut endpoint).unwrap();
        assert!(settled.progress.drained());
        assert_eq!(settled.progress.charged_units, if phase == 2 { 16 } else { 0 });
        assert_eq!(owner.inspect().ledger.available, if phase == 2 { 84 } else { 100 });
        assert_eq!(endpoint.execution_count(), if phase == 2 { 1 } else { 0 });
        assert_eq!(endpoint.payload(), if phase == 2 { &b"visible"[..] } else { &b"initial"[..] });
        if let Some(message) = message { assert_eq!(endpoint.deliver(&message).err(), Some(Error::Stale)); }
    }
}

#[test]
fn missing_audits_and_exhausted_monitoring_are_not_reported_as_an_alarm() {
    for missing in [false, true] {
        let model = model(); let mut config = config(&model, false, 1);
        if missing { config.telemetry.source_check_values = 0; }
        else { config.telemetry.monitor_probe_coordinates = 0; }
        let (mut owner, endpoint, _) = owner(&[], false); attach(&mut owner, &model, config);
        owner.enable_learned_host_stop(stop_policy()).unwrap();
        let result = step(&mut owner);
        let cause = if missing {
            assert_eq!(result.err(), Some(Error::Limit)); LearnedHostStopCause::Failure(Error::Limit)
        } else {
            let event = result.unwrap(); assert!(event.accepted().is_none());
            assert_eq!(event.status(), GenerationStatus::Held(MonitorOutcome::BudgetExhausted));
            LearnedHostStopCause::Monitoring(MonitorOutcome::BudgetExhausted)
        };
        assert_eq!(owner.learned_host_stop_incident().unwrap().cause(), cause);
        assert!(owner.learned_host_stop_incident().unwrap().stop_receipt().is_some());
        assert!(owner.inspect().suspended); assert_eq!(owner.delivery.controller().actor().next_position(), 0);
        assert_eq!(owner.hosted_learned_generation().unwrap().work.admitted_tokens, 1);
        assert_eq!(step(&mut owner).err(), Some(Error::WrongState)); assert_eq!(endpoint.execution_count(), 0);
    }
}

#[test]
fn automatic_stop_blocks_previously_approved_human_and_automatic_keys_without_publication() {
    let model = model(); let (mut owner, endpoint, reviewer) = owner(&[], true);
    attach(&mut owner, &model, config(&model, true, 1)); owner.enable_learned_host_stop(stop_policy()).unwrap();
    step(&mut owner).unwrap();
    let (action, inputs) = proposal(&mut owner, 1); approve(&mut owner, 1, &inputs);
    let permit = owner.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let request = owner.request_human_approval(8, 1, Some(&inputs), ElapsedTick(80)).unwrap();
    let human = reviewer.unwrap().approve(&request, ElapsedTick(1)).unwrap();
    step(&mut owner).unwrap();
    assert!(owner.dispatch_with_human(&permit, &human, &action, Some(&inputs), &snapshot()).is_err());
    assert_eq!(owner.inspect().ledger.stages[&1], ActionState::Cancelled);
    assert_eq!(owner.inspect().ledger.available, 100); assert_eq!(endpoint.execution_count(), 0);
    // The existing stop invalidates the authority/approval basis. It need not
    // falsely relabel an unused independently issued human key as consumed.
    assert_eq!(owner.human_status(8).unwrap().disposition,
        crate::action::consequence::oversight::human::HumanDisposition::Approved);
}

#[test]
fn fixed_stop_policy_cannot_be_installed_late_or_changed_after_work_starts() {
    let model = model();
    let (mut fresh, _, _) = owner(&[], false);
    assert_eq!(fresh.enable_learned_host_stop(stop_policy()), Err(Error::Incomplete));
    attach(&mut fresh, &model, config(&model, false, 1));
    fresh.enable_learned_host_stop(stop_policy()).unwrap();
    assert_eq!(fresh.enable_learned_host_stop(HostedStopPolicy::new(8, 2, 901).unwrap()), Err(Error::Duplicate));
    let before = fresh.hosted_learned_generation().unwrap();
    assert_eq!(fresh.advance_hosted_learned(before.actor_revision, 1).err(), Some(Error::Stale));
    assert_eq!(fresh.hosted_learned_generation().unwrap(), before); assert!(fresh.learned_host_stop_incident().is_none());
    step(&mut fresh).unwrap();
    assert_eq!(fresh.learned_host_stop_policy(), Some(stop_policy()));
    let (mut late, _, _) = owner(&[], false); attach(&mut late, &model, config(&model, false, 1)); step(&mut late).unwrap();
    assert_eq!(late.enable_learned_host_stop(stop_policy()), Err(Error::WrongState));
    assert!(late.learned_host_stop_policy().is_none()); assert!(!late.inspect().suspended);
    assert!(step(&mut late).unwrap().accepted().is_some());
}

#[test]
fn synchronization_failures_and_caught_unwinds_cannot_resume_before_containment() {
    for unwind in [false, true] {
        let model = model(); let (mut owner, _, _) = owner(&[], false);
        attach(&mut owner, &model, config(&model, false, 1)); owner.enable_learned_host_stop(stop_policy()).unwrap();
        let revision = owner.actor_revision();
        let expected = if unwind {
            assert!(catch_unwind(AssertUnwindSafe(|| {
                let host = owner.learned_host.as_mut().unwrap(); host.fault = Some(Error::Incomplete);
                let _guard = host.run.guard_host_sync(); host.run.advance(0).unwrap();
                panic!("interrupted before actor synchronization");
            })).is_err());
            assert!(!owner.inspect().suspended);
            // This public call services the pending trigger before attempting
            // another token. The already computed first token is not recomputed.
            assert_eq!(owner.advance_hosted_learned(revision, 0).err(), Some(Error::WrongState));
            Error::Incomplete
        } else {
            owner.learned_host.as_mut().unwrap().profile.model_generation += 1;
            assert_eq!(step(&mut owner).err(), Some(Error::Binding)); Error::Binding
        };
        let incident = owner.learned_host_stop_incident().unwrap();
        assert_eq!(incident.cause(), LearnedHostStopCause::Failure(expected));
        assert_eq!(incident.observed().work.admitted_tokens, 1);
        assert_eq!(incident.observed().host_failure, Some(expected));
        assert!(incident.stop_receipt().is_some()); assert!(owner.inspect().suspended);
        assert_eq!(owner.actor_revision(), revision); assert_eq!(owner.delivery.controller().actor().next_position(), 0);
        assert_eq!(owner.hosted_learned_observation().unwrap().availability(), LearnedAvailability::Failed);
    }
}

#[test]
fn endpoint_fencing_failure_keeps_unknown_charges_until_a_real_retry_settles_them() {
    let model = model(); let (mut owner, mut endpoint, _) = owner(&[], false);
    attach(&mut owner, &model, config(&model, true, 1)); owner.enable_learned_host_stop(stop_policy()).unwrap();
    step(&mut owner).unwrap();
    let (action, inputs) = proposal(&mut owner, 1); approve(&mut owner, 1, &inputs);
    let permit = owner.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let message = owner.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    step(&mut owner).unwrap();
    let receipt = owner.learned_host_stop_incident().unwrap().stop_receipt().unwrap().clone();
    let mut wrong_endpoint = PublicationEndpoint::new(target(), b"unrelated".to_vec(), 1000, 8).unwrap();
    assert_eq!(owner.progress_stop(&mut wrong_endpoint).err(), Some(Error::Binding));
    assert_eq!(owner.inspect().ledger.charged, 16); assert_eq!(owner.inspect().ledger.available, 84);
    assert_eq!(owner.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(owner.enforce_learned_host_stop(), Ok(Some(receipt)));
    let settled = owner.progress_stop(&mut endpoint).unwrap(); assert!(settled.progress.drained());
    assert_eq!(owner.inspect().ledger.charged, 0); assert_eq!(owner.inspect().ledger.available, 100);
    assert_eq!(endpoint.execution_count(), 0); assert_eq!(endpoint.deliver(&message).err(), Some(Error::Stale));
}

#[test]
fn prior_manual_suspension_is_not_relabelled_as_a_learned_monitor_incident() {
    let model = model(); let (mut owner, _, _) = owner(&[], false);
    attach(&mut owner, &model, config(&model, false, 1)); owner.enable_learned_host_stop(stop_policy()).unwrap();
    let view = owner.inspect();
    let manual = owner.request_stop(StopRequest { operation: 901, expected_control_sequence: view.sequence,
        expected_authority_epoch: view.ledger.epoch }).unwrap();
    assert_eq!(owner.enforce_learned_host_stop(), Ok(None));
    assert!(owner.learned_host_stop_incident().is_none()); assert_eq!(owner.stop_receipt(), Some(&manual));
    assert_eq!(step(&mut owner).err(), Some(Error::WrongState));
    assert_eq!(owner.hosted_learned_generation().unwrap().work.admitted_tokens, 0);
}
