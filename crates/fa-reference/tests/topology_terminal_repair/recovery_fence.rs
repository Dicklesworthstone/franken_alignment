//! The credential-free Fence cut is usable by every original learned lane.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::driver::topology::checkpoint::TopologyCheckpointResume;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::mediated::FileMediatedRoles;

fn anchored(root: &Directory, lane: Lane, saved: &TopologyCheckpoint) -> Original {
    let config = configuration(lane);
    let expected = expected(lane, saved);
    if lane == Lane::Plain {
        Original::Plain(
            FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
                root.store(),
                profile(),
                &expected,
                &config,
                saved.history_anchor(),
            )
            .unwrap()
            .require_recovery_reserve(RecoveryReserve::terminal())
            .unwrap(),
        )
    } else {
        let expected = FilePredictiveMediatedRequirements {
            oversight: expected.oversight,
            topology: expected.topology,
            prediction: predictor(&config),
            evaluation: expected.evaluation,
        };
        Original::Predictive(
            FileOversight::begin_open_predictive_mediated_guarded_anchored_with_learned_generation(
                root.store(),
                profile(),
                &expected,
                &config,
                saved.history_anchor(),
            )
            .unwrap()
            .require_recovery_reserve(RecoveryReserve::terminal())
            .unwrap(),
        )
    }
}

fn finish(mut run: Original) -> (FileOversight, FileMediatedRoles) {
    run.drive().unwrap();
    match run {
        Original::Plain(run) => run.finish().unwrap(),
        Original::Predictive(run) => run.finish().unwrap(),
    }
}

#[test]
fn learned_recovery_fence_checkpoint_supports_corrected_restart_in_all_native_lanes() {
    for lane in LANES {
        let root = Directory::new();
        let driver = seed(&root, lane, ElapsedTick(1), true);
        let saved = imported(&archive(&capture(&driver)));
        let numerical = driver
            .supervisor()
            .host()
            .unwrap()
            .learned_generation_inspection()
            .unwrap()
            .numerical;
        let charged = driver
            .supervisor()
            .host()
            .unwrap()
            .inspect()
            .control
            .ledger
            .charged;
        drop(driver);
        let (host, roles) = finish(anchored(&root, lane, &saved));
        assert!(host.credential_status().unwrap().is_none());
        let before = root.bytes();
        let fenced = roles.topology_observer
            .recovered_topology_checkpoint(&host, &saved).unwrap();
        same_source(&saved, &fenced);
        assert!(!fenced.available());
        assert_eq!(fenced.history_anchor(), &host.history_anchor().unwrap());
        assert_eq!(
            fenced.history_anchor().revision(),
            saved.history_anchor().revision() + 1
        );
        let state = host.learned_generation_inspection().unwrap();
        assert!(state.paused);
        assert_eq!(state.numerical, numerical);
        assert_eq!(host.inspect().control.ledger.charged, charged);
        assert_eq!(root.bytes(), before);

        let (port, mut driver) = host.into_supervised_driver();
        drop(port);
        // Startup requested an unchanged graph. The failed bind neither owns
        // a live source nor erases the already acknowledged recovery Fence.
        let result = driver.resume_topology_checkpoint(
            revision(&driver),
            saved.last_operation() + 1,
            roles.topology_observer,
            &fenced,
            TopologyCheckpointResume {
                topology_path: root.0.join("topology.json"),
                inventory_path: root.0.join("inventory.json"),
                next: saved.expected().clone(),
            },
        );
        assert!(result.is_err());
        assert!(driver.topology().is_none());
        assert_eq!(root.bytes(), before);
        drop(driver);

        // The old checkpoint cannot claim that the fenced topology is still
        // available. The original guarded replay refuses it without writing.
        let mut stale = anchored(&root, lane, &saved);
        assert!(stale.drive().is_err(), "{lane:?}");
        drop(stale);
        assert_eq!(root.bytes(), before);

        let fenced = imported(&archive(&fenced));
        let (mut host, roles) = finish(anchored(&root, lane, &fenced));
        assert_eq!(
            roles.topology_observer.recovered_topology_checkpoint(&host, &saved),
            Err(JournalError::Contract(Error::Stale))
        );
        let resumed = roles.topology_observer
            .recovered_topology_checkpoint(&host, &fenced).unwrap();
        same_source(&saved, &resumed);
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical
        );
        assert_eq!(host.inspect().control.ledger.charged, charged);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        fs::write(root.0.join("topology.json"), document(2)).unwrap();
        let next =
            LoadedAuthorityTopology::from_json_bytes(document(2).as_bytes(), INVENTORY).unwrap();
        let (port, mut driver) = host.into_supervised_driver();
        drop(port);
        let report = driver
            .resume_topology_checkpoint(
                revision(&driver),
                resumed.last_operation() + 1,
                roles.topology_observer,
                &resumed,
                TopologyCheckpointResume {
                    topology_path: root.0.join("topology.json"),
                    inventory_path: root.0.join("inventory.json"),
                    next,
                },
            )
            .unwrap();
        assert!(report.refusal.is_none());
        assert!(matches!(report.checked, Some(Ok(CutCheck::Verified(_)))));
        let source = driver.topology().unwrap().source();
        assert_eq!(source.work().checks, saved.work().checks + 1);
        assert_eq!(source.last_operation(), saved.last_operation() + 2);
        let host = driver.supervisor().host().unwrap();
        assert_eq!(
            host.learned_generation_inspection().unwrap().numerical,
            numerical
        );
        assert_eq!(host.inspect().control.ledger.charged, charged);
        assert_eq!(host.inspect().executions, 0);
    }
}
