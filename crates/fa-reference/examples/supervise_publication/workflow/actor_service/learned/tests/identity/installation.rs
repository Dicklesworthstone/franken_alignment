//! Computation completion and identity installation are separate native cuts.
use super::*;
use fa_reference::{Error, action::consequence::delivery::persistent::JournalError};

fn one_tick_identity(root: &Root) -> Case {
    let config = configured(root);
    let path = native::write(root);
    provision(root, &path, false);
    let input = fs::read_to_string(&path).unwrap();
    assert_eq!(input.matches("\"timeout_ticks\":10000").count(), 1);
    assert_eq!(input.matches("\"validity_ticks\":60000").count(), 1);
    fs::write(&path, input.replace("\"timeout_ticks\":10000", "\"timeout_ticks\":1")
        .replace("\"validity_ticks\":60000", "\"validity_ticks\":1")).unwrap();
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let registration = loaded.generation.required_computed_identity_registration().unwrap();
    assert_eq!(registration.policy.timeout_ticks, 1);
    assert_eq!(registration.policy.validity_ticks, 1);
    let (host, reviewer, forecast, identity) = recovery::create_selected(&config, &loaded).unwrap();
    assert!(forecast.is_none());
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    Case { config, path, loaded, reviewer, driver: FileSupervisedDriver::new(supervisor),
        wire: ActorWire::new(port), prediction: None,
        identity: Some(check::Session::new(identity.unwrap())), histories: Vec::new() }
}

#[test]
fn installation_observes_current_native_validity_after_acknowledged_work_before_two_key_publication() {
    for expired in [false, true] {
        let root = Root::new();
        let mut case = one_tick_identity(&root);
        let before = case.numerical();
        let deadline = deadline();
        let mut control = Control::new(&case.config, case.loaded.request, None).unwrap();
        let mut calls = 0;
        let result = check::prepare(&mut case.driver, &mut case.config, &case.loaded,
            &mut case.identity, (&mut control, &case.reviewer, &deadline), &mut || {
                calls += 1;
                // Calls through 21 cover setup, all five tokens and canonical
                // commit at 1000. The first later workflow check sees 1001,
                // still within its deadline. Installation must observe that
                // current tick instead of reusing the sealed completion time.
                ElapsedTick(if expired && calls >= 22 { 1001 } else { 1000 })
            });
        assert_eq!(calls, 23, "installation must sample its own current time");
        let now = ElapsedTick(if expired { 1001 } else { 1000 });
        assert!(deadline.check(now).is_ok(), "only native identity validity expires");
        {
            let history = case.identity.as_ref().unwrap().history();
            assert_eq!(history.len(), 1);
            let run = &history[0];
            assert_eq!(run.status(), ComputedIdentityStatus::Committed);
            assert_eq!(run.work().planned_tokens, 5);
            assert_eq!(run.work().entered_tokens, 5);
            assert_eq!(run.work().completed_tokens, 5);
            assert_eq!(run.work().measured_anchors, 2);
            assert!(run.work().completed_scalar_products > 0);
            assert!(!run.interrupted());
            let completed = run.result().unwrap();
            assert_eq!(completed.work, run.work());
            assert_eq!(completed.started_at, ElapsedTick(1000));
            assert_eq!(completed.completed_at, ElapsedTick(1000));
            let report = completed.observation.measurement.as_ref().unwrap();
            assert_eq!(report.outcome, IdentityOutcome::Matched);
            assert_eq!(report.completed_at, Some(ElapsedTick(1000)));
            assert_eq!(report.deadline, ElapsedTick(1001));
            assert_eq!(report.valid_until, ElapsedTick(1001));

            let host = case.driver.supervisor().host().unwrap();
            assert_eq!(host.identity_report(901).unwrap(), *report,
                "installation expiry must preserve the acknowledged historical match");
            assert_eq!(host.inspect().control.ledger.elapsed, Some(now));
            let current = host.learned_generation_inspection().unwrap();
            assert_eq!(current.numerical, before.numerical);
            assert_eq!(current.pending, before.pending);
            assert_eq!(current.paused, before.paused);
            assert_eq!(current.numerical.sampled_draws, 0);
            assert_eq!(current.numerical.position, 0);
            assert_eq!(host.inspect().executions, 0);
            assert_eq!(host.inspect().control.ledger.charged, 0);
            assert_eq!(host.inspect().control.ledger.reserved, 0);
            assert!(host.request_status(case.loaded.request).is_err());
            if expired {
                assert_eq!(result, Err(debug(JournalError::Contract(Error::Stale))));
                assert!(host.identity_installation(901).unwrap().is_none());
                assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
            } else {
                assert_eq!(result, Ok(true));
                assert!(host.identity_installation(901).unwrap().is_some());
                assert_eq!(host.identity_status().unwrap(), IdentityStatus::Matching {
                    check: 901, valid_until: ElapsedTick(1001) });
            }
        }
        if expired {
            case.close();
        } else {
            // The valid control still requires real generation, original
            // congress and the separate human key before one visible effect.
            let case = case.publish();
            let host = case.driver.supervisor().host().unwrap();
            assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["A"]);
            assert_eq!(host.inspect().executions, 1);
            assert_eq!(host.inspect().control.ledger.charged,
                host.request_action(case.loaded.request).unwrap().spec().units);
            assert_eq!(host.inspect().control.ledger.reserved, 0);
            assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, 2);
            drop(host);
            case.close();
        }
    }
}
