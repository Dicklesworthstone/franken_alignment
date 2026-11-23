//! Isolate reply grace's original control barrier from numerical work. The
//! complete generated workflow is covered by the parent composition tests.
use super::*;
use fa_reference::round::Verdict;

#[path = "../../../../../../../../tests/support/file_credibility.rs"]
mod credibility;

/// This narrowly scoped control test prepares an actual original native case,
/// then seals its exact valid assessment through the separately authenticated
/// evaluator process before entering reply grace. The failed arm changes only
/// an original Stop checkpoint's deadline; it must not accept queued new work.
#[test]
fn queued_assessment_requires_a_successful_original_stop_checkpoint() {
    for failed in [false, true] {
        let root = Root::new();
        let mut config = fixture::configured(&root);
        config.profile = credibility::profile();
        config.profile.delivery.clock_domain = crate::config::CLOCK_DOMAIN;
        let (actor, reviewer_profile) = profiles(&root, &config);
        let mut peer = EvaluatorProcess::start(&root, &actor, 2);
        let (mut host, reviewer) =
            FileOversight::create(&config.store, config.profile.clone()).unwrap();
        let evaluator = host
            .enable_credibility(host.revision(), credibility::protocol())
            .unwrap();
        host.enable_andon(host.revision(), &settings().config)
            .unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        credibility::review(&mut host, 1, Verdict::Hold);
        let ticket = host.evaluation_ticket(101).unwrap();
        let case = ticket.case();
        let root_hex = case
            .evidence_root
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let assessment = format!(
            r#"{{"schema_version":"fa.native-assessment/1","round":{},"attempt":{},"policy_generation":{},"congress_generation":{},"case_evidence_root_hex":"{root_hex}","expected_evaluation_revision":{},"origin":1,"evidence_id_hex":"{}","truth":"benign"}}"#,
            case.round,
            case.attempt,
            case.policy_generation,
            case.congress_generation,
            host.credibility_report().unwrap().revision,
            "13".repeat(32),
        );
        let encoded = assessment
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let scope = actor.scope;
        let request = format!(
            r#"{{"schema_version":"fa.native-evaluator-request/1","audience":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{},"deployment":101,"evaluator":202}},"assessment_hex":"{encoded}"}}"#,
            scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        );
        let (_, mut driver) = host.into_supervised_driver();
        let start = crate::workflow::clock();
        let deadline = Deadline {
            logical: ElapsedTick(start.0 + 20_000),
            started: Instant::now(),
            wall: Duration::from_secs(20),
        };
        let service =
            evaluator::Service::start(&peer.selected, &actor, evaluator, &driver, &deadline)
                .unwrap();
        let mut control = Control::new(&config, actor.request, Some(&reviewer_profile)).unwrap();
        control.install_evaluator(Some(service));
        peer.queue(request.as_bytes());
        let before = fs::read(config.store.join("delivery.bin")).unwrap();
        let original = driver
            .supervisor()
            .host()
            .unwrap()
            .credibility_report()
            .unwrap();
        // The authenticated Stop connection exists, but no Stop decision was
        // sent. Its original deadline fails before a native Stop or label.
        let stop = failed.then(|| {
            UnixStream::connect(crate::workflow::control::socket_path(
                &reviewer_profile,
                actor.request,
            ))
            .unwrap()
        });
        let mut clocks = 0;
        let result = reply_grace_selected(
            &mut driver,
            None,
            (100, 1),
            (&mut control, &reviewer, &deadline),
            true,
            &mut || {
                clocks += 1;
                if failed && clocks == 2 {
                    deadline.logical
                } else {
                    start
                }
            },
        );
        let (accepted, response) = peer.completion();
        let host = driver.supervisor().host().unwrap();
        if failed {
            assert!(
                result.is_err(),
                "the original Stop deadline failure must survive reply grace"
            );
            assert!(
                clocks >= 2,
                "the queued original Stop peer must reach its deadline checkpoint"
            );
            assert!(
                !accepted && response.is_empty(),
                "a label queued before a failed Stop check is still unacknowledged"
            );
            assert_eq!(host.credibility_report().unwrap(), original);
            assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
            assert!(
                host.inspect().stop.is_none(),
                "this isolates a failed control check before native application"
            );
        } else {
            assert!(result.is_ok(), "{result:?}");
            assert!(accepted);
            assert_eq!(
                field(&parse(&response), &["status"]).as_str(),
                Some("applied")
            );
            assert_eq!(host.credibility_report().unwrap().pending_cases, 0);
            assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
            assert_eq!(
                host.inspect().executions,
                0,
                "an evaluator label is never a publication key"
            );
        }
        drop(host);
        drop(stop);
        control.take_evaluator().unwrap().retire();
        peer.finish();
        assert_eq!(
            cleanup(driver, config.timing.cleanup_ms, config.timing.poll_ms),
            0
        );
    }
}
