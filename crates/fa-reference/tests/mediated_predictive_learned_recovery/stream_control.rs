//! Paired native containment reaches actual stream publication under a new cut.
//! The declared graph and tiny model do not qualify deployment containment.
use super::*;
use fa_reference::action::consequence::delivery::{stream::StreamProfile, persistent::{
    RecoveryReserve, requests::actor::LearnedTextRelease, observed::{
        decoder::learned::{FileLearnedStreamCheckpointRequest, FileLearnedStreamControlHistory,
            FileLearnedStreamControlReceipt, FileLearnedStreamNarrowControl, FileLearnedStreamNarrowRequest,
            checkpoint::FileLearnedCheckpointInfo},
        driver::topology::{FileDriverTopology, checkpoint::{TopologyCheckpoint, MAX_TOPOLOGY_CHECKPOINT_BYTES}},
        guarded::mediated::predictive::{FileMediatedStreamControlRequest, FileMediatedStreamControlHistory,
            FileMediatedStreamControlRecovery},
        mediation::source::{TopologySourceRecovery, TopologyPublication},
    },
}};
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
mod fixture;
use fixture::*;

#[test]
fn paired_owned_prompt_containment_reaches_native_human_publication_and_a_bypass_cannot() {
    let mut common_epoch = None;
    for bypass in [false, true] {
        let h = History::new(true);
        let capture = FileMediatedStreamControlRequest::Capture(h.capture);
        let captured = h.apply(&h.saved, capture.clone()).unwrap();
        assert_eq!(captured.control().appended_events(), 1);
        assert_eq!(captured.control().snapshot().revision, h.saved.history_anchor().revision() + 1);
        assert_eq!(captured.topology_checkpoint().history_anchor(), captured.control().history_anchor());
        unchanged_source(&h.saved, captured.topology_checkpoint());
        let bytes = h.root.bytes();
        let again = h.apply(&h.saved, capture).unwrap();
        assert_eq!(again.control().appended_events(), 0);
        assert_eq!(again.topology_checkpoint(), captured.topology_checkpoint());
        assert_eq!(h.root.bytes(), bytes);
        let request = narrow(captured.control(), h.capture.checkpoint());
        let selection = FileMediatedStreamControlRequest::Narrow(request.clone());
        let narrowed = h.apply(captured.topology_checkpoint(), selection.clone()).unwrap();
        let first = match narrowed.control().receipt() {
            FileLearnedStreamControlReceipt::Narrowed { receipt, .. } => {
                assert_eq!(receipt.incident_count, 0);
                assert_eq!(receipt.sequence, h.capture.checkpoint().control_sequence + 1);
                assert_eq!(receipt.revocation_floor, h.capture.checkpoint().authority_epoch + 1);
                receipt.sequence
            }, _ => panic!("original narrowing receipt"),
        };
        assert_eq!(narrowed.control().numerical(), captured.control().numerical());
        assert_eq!(narrowed.control().recovery_usage(), captured.control().recovery_usage());
        assert_eq!(narrowed.control().snapshot().control.ledger.charged, 0);
        unchanged_source(&h.saved, narrowed.topology_checkpoint());
        let bytes = h.root.bytes();
        let again = h.apply(captured.topology_checkpoint(), selection).unwrap();
        assert_eq!(again.control().appended_events(), 0);
        assert_eq!(again.control().receipt(), narrowed.control().receipt());
        assert_eq!(again.topology_checkpoint(), narrowed.topology_checkpoint());
        assert_eq!(h.root.bytes(), bytes);
        let intent = reset(narrowed.control());
        let selection = FileMediatedStreamControlRequest::BeginReset {
            intent: intent.clone(), first_incident_sequence: first,
        };
        let begun = h.apply(narrowed.topology_checkpoint(), selection.clone()).unwrap();
        assert_eq!(begun.control().appended_events(), 1);
        assert_eq!(begun.control().recovery_usage().restart_attempts, 0);
        unchanged_source(&h.saved, begun.topology_checkpoint());
        let bytes = h.root.bytes();
        let again = h.apply(narrowed.topology_checkpoint(), selection).unwrap();
        assert_eq!(again.control().appended_events(), 0);
        assert_eq!(again.topology_checkpoint(), begun.topology_checkpoint());
        assert_eq!(h.root.bytes(), bytes);
        let (host, old_roles) = h.complete(begun.topology_checkpoint(), &intent, first).unwrap();
        let usage = host.learned_recovery_usage().unwrap();
        let evidence = host.action_consistency_snapshot().unwrap().evidence;
        let numerical = host.learned_generation_inspection().unwrap().numerical;
        let result = host.learned_reset_result(900).unwrap().unwrap();
        assert!(result.control.restored); assert_eq!(result.resumed_stream, Some(22));
        assert_eq!(usage.restart_attempts, 1);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        assert_eq!(numerical.work, captured.control().numerical().work);
        assert_eq!(numerical.cumulative_work, captured.control().numerical().cumulative_work);
        assert_eq!(numerical.sampled_draws, 0);
        assert_eq!(host.current_policy().unwrap(), &h.profile.delivery.policy);
        assert_eq!(result.restored_forecast_audit.as_ref().unwrap().monitoring().end_position(), 2);
        assert!(host.learned_generation_inspection().unwrap().paused);
        assert!(!host.mediation_snapshot().unwrap().available);
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        let completed_revision = host.revision(); drop(host);
        // Simulate lost completion archives. The old exact pair can recover
        // only this reset/Fence suffix, retaining both original audit charges.
        let (mut host, roles) = h.complete(begun.topology_checkpoint(), &intent, first).unwrap();
        assert_eq!(host.revision(), completed_revision + 1);
        assert_eq!(host.learned_recovery_usage().unwrap(), usage);
        assert_eq!(host.action_consistency_snapshot().unwrap().evidence, evidence);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        let completed = begun.topology_checkpoint().after_learned_stream_reset(&host, &intent, first).unwrap();
        unchanged_source(&h.saved, &completed); assert!(!completed.available());
        assert_eq!(completed.history_anchor(), &host.history_anchor().unwrap());
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let n = host.learned_generation_inspection().unwrap().numerical;
        host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
        assert_eq!(Mode::Owned.forecast(&mut host, old_roles.consistency_observer.as_ref().unwrap(), 71).err(),
            Some(Error::Binding.into()));
        let bytes = h.root.bytes();
        assert!(host.advance_learned_generation(host.revision(), n.actor_revision, n.position).is_err());
        assert_eq!(h.root.bytes(), bytes);
        let directory = h.root.store().parent().unwrap().to_owned();
        let next = document(2, bypass);
        std::fs::write(directory.join("topology.json"), &next).unwrap();
        std::fs::write(directory.join("inventory.json"), INVENTORY).unwrap();
        let loaded = LoadedAuthorityTopology::from_json_bytes(next.as_bytes(), INVENTORY).unwrap();
        let revision = host.revision();
        let mut source = roles.topology_observer.resume_topology_source(&mut host, revision, 2,
            TopologySourceRecovery { topology_path: directory.join("topology.json"),
                inventory_path: directory.join("inventory.json"), previous: completed.expected().clone(),
                next: loaded.clone(), work: completed.work(), last_operation: completed.last_operation(),
            }).unwrap();
        assert_eq!(source.work(), completed.work());
        assert!(host.mediation_snapshot().unwrap().accepted.is_none());
        let revision = host.revision(); let refreshed = source.refresh(&mut host, revision, 3).unwrap();
        assert_eq!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))), !bypass);
        assert_eq!(source.work().checks, completed.work().checks + 1);
        let epoch = host.inspect().control.ledger.epoch;
        if let Some(expected) = common_epoch { assert_eq!(epoch, expected); }
        common_epoch = Some(epoch);
        let forecast = Mode::Owned.forecast(&mut host, roles.consistency_observer.as_ref().unwrap(), 71)
            .unwrap().unwrap();
        assert_eq!(forecast.frame.stream, 22);
        let n = host.learned_generation_inspection().unwrap().numerical;
        let pending = FileLearnedStreamNarrowRequest::new(host.revision(), h.capture.checkpoint(),
            FileLearnedStreamNarrowControl { operation: 801, expected_control_sequence: host.inspect().control.sequence,
                expected_actor_revision: n.actor_revision, expected_authority_epoch: epoch,
                binding: ReviewBinding { round: 801, reducer_generation: 1, evidence_root: [7; 32] },
                retained_targets: vec![host.inspect().target],
            }).unwrap();
        let bytes = h.root.bytes();
        assert_eq!(host.narrow_learned_stream_checkpoint(&pending).err(), Some(Error::Incomplete.into()));
        assert_eq!(h.root.bytes(), bytes);
        let (samples, last) = finish_generation(&mut host);
        let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
        assert_eq!(samples, original_samples(&model, support::source(&model)));
        fresh_identity(&mut host, &roles.oversight, 1, 2);
        let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
        let revision = supervisor.host().unwrap().revision();
        supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
        let _ticket = actor.submit(71, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
        {
            let mut host = supervisor.host_mut().unwrap();
            assert_eq!(host.action_consistency_snapshot().unwrap().evidence.samples(), 1);
            assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
            let n = host.learned_generation_inspection().unwrap().numerical;
            let sampled = FileLearnedStreamNarrowRequest::new(host.revision(), h.capture.checkpoint(),
                FileLearnedStreamNarrowControl { operation: 802, expected_control_sequence: host.inspect().control.sequence,
                    expected_actor_revision: n.actor_revision, expected_authority_epoch: epoch,
                    binding: ReviewBinding { round: 802, reducer_generation: 1, evidence_root: [6; 32] },
                    retained_targets: vec![host.inspect().target],
                }).unwrap();
            let bytes = h.root.bytes();
            assert_eq!(host.narrow_learned_stream_checkpoint(&sampled).err(), Some(Error::WrongState.into()));
            assert_eq!(h.root.bytes(), bytes);
            if bypass {
                assert!(matches!(host.request_status(71).unwrap().disposition, FileRequestDisposition::NotAdmitted(_)));
                assert!(host.request_action(71).is_err());
                assert!(host.mediation_snapshot().unwrap().accepted.is_none());
                assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 0);
            } else {
                let (action, input, automatic, request) = prepared(&mut host, 71, 101, &last);
                let revision = host.revision();
                assert!(host.publish_checked(revision, 1, Some(&input), snapshot(), ElapsedTick(2)).is_err());
                let revision = host.revision();
                assert_eq!(old_roles.oversight.human.approve(&mut host, revision, &request).err(), Some(Error::Binding.into()));
                let human = roles.oversight.human.approve(&mut host, revision, &request).unwrap();
                let revision = host.revision();
                host.dispatch(revision, &automatic, &human, &action, &input, snapshot()).unwrap();
                let revision = host.revision();
                let publication = host.publish_checked_from_topology_source(&mut source, TopologyPublication {
                    revision, operation: 4, attempt: 1, current: Some(&input), snapshot: snapshot(),
                }, || ElapsedTick(2)).unwrap();
                assert_eq!(publication.publication.unwrap().unwrap().outcome,
                    EndpointOutcome::Executed { resulting_version: 2 });
                let revision = host.revision(); host.reconcile(revision, 1).unwrap();
                assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), ["OK"]);
                assert_eq!(host.inspect().executions, 1);
                assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
                assert_eq!(host.delivery_mediation(1).unwrap().unwrap().graph(), loaded.graph());
                assert_eq!(source.work().checks, completed.work().checks + 2);
            }
            assert_eq!(host.learned_recovery_usage().unwrap(), usage);
            assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
        }
        drop(supervisor); drop(actor);
        let bytes = h.root.bytes();
        assert!(h.complete(begun.topology_checkpoint(), &intent, first).is_err(),
            "old paired custody cannot skip later source, prediction or publication events");
        assert_eq!(h.root.bytes(), bytes);
    }
}

#[test]
fn exact_pair_guards_reserve_and_event_zero_admission_are_required_before_control() {
    let h = History::new(true); let foreign = History::new(true);
    let request = FileMediatedStreamControlRequest::Capture(h.capture);
    let bytes = h.root.bytes();
    assert!(h.begin(&h.expected, &foreign.saved).is_err());
    let mut late = h.begin(&h.expected, &h.saved).unwrap(); late.advance(0, 1).unwrap();
    assert_eq!(late.require_stream_control_checkpoint(&h.saved, request.clone()).err(), Some(Error::WrongState.into()));
    assert!(h.begin(&h.expected, &h.saved).unwrap()
        .require_stream_control_checkpoint(&h.saved, request.clone()).unwrap().finish_stream_control().is_err());
    for field in 0..7 {
        let mut expected = h.expected.clone();
        match field {
            0 => expected.oversight.guards.identity = None,
            1 => expected.oversight.effective_policy = Policy::new(2, vec![Predicate::PayloadAtMost(4096)]).unwrap(),
            2 => expected.topology.available = false,
            3 => expected.oversight.minimum.authority_epoch += 1,
            4 => expected.oversight.minimum.journal_revision += 1,
            5 => expected.prediction = changed_owned(&h.config),
            _ => expected.evaluation = Some(protocol()),
        }
        let result = h.begin(&expected, &h.saved).and_then(|run|
            run.require_stream_control_checkpoint(&h.saved, request.clone())).and_then(|mut run| {
                while run.progress().status == FileLearnedRecoveryStatus::Replaying {
                    run.advance(run.progress().replayed_events, 1)?;
                }
                run.finish_stream_control()
            });
        assert!(result.is_err()); assert_eq!(h.root.bytes(), bytes);
    }
    let plain = fa_reference::action::consequence::delivery::persistent::observed::guarded::predictive::owned::FileOwnedPredictiveRequirements {
        oversight: h.expected.oversight.clone(), prediction: h.config.required_owned_pre_output_forecast().unwrap().clone(),
        evaluation: None,
    };
    assert!(FileOversight::begin_open_predictive_guarded_anchored_with_owned_learned_consistency(
        h.root.store(), h.profile.clone(), &plain, &h.config, h.saved.history_anchor()).is_err());
    let unreserved = History::new(false); let before = unreserved.root.bytes();
    assert!(unreserved.apply(&unreserved.saved, FileMediatedStreamControlRequest::Capture(unreserved.capture)).is_err());
    assert_eq!(unreserved.root.bytes(), before);
    assert_eq!(h.apply(&h.saved, request).unwrap().control().appended_events(), 1);
}

#[test]
fn changed_request_or_unrelated_suffix_cannot_repair_a_lost_source_export() {
    let h = History::new(true);
    let capture = FileMediatedStreamControlRequest::Capture(h.capture);
    let captured = h.apply(&h.saved, capture.clone()).unwrap();
    let bytes = h.root.bytes();
    let mut wrong = h.capture.checkpoint(); wrong.position += 1;
    let changed = FileLearnedStreamCheckpointRequest::new(h.capture.revision(), wrong).unwrap();
    assert!(h.apply(&h.saved, FileMediatedStreamControlRequest::Capture(changed)).is_err());
    assert_eq!(h.root.bytes(), bytes);
    let request = narrow(captured.control(), h.capture.checkpoint());
    let narrowed = h.apply(captured.topology_checkpoint(), FileMediatedStreamControlRequest::Narrow(request.clone())).unwrap();
    let bytes = h.root.bytes();
    let mut wrong = request.control().clone(); wrong.binding.evidence_root[0] ^= 1;
    let changed = FileLearnedStreamNarrowRequest::new(request.revision(), request.checkpoint(), wrong).unwrap();
    assert!(h.apply(captured.topology_checkpoint(), FileMediatedStreamControlRequest::Narrow(changed)).is_err());
    assert!(h.apply(&h.saved, capture).is_err(), "the unrelated Narrow suffix cannot explain Capture export");
    assert_eq!(h.root.bytes(), bytes);
    let intent = reset(narrowed.control());
    let first = narrowed.control().snapshot().control.sequence;
    let begun = h.apply(narrowed.topology_checkpoint(), FileMediatedStreamControlRequest::BeginReset {
        intent: intent.clone(), first_incident_sequence: first,
    }).unwrap();
    let bytes = h.root.bytes();
    assert!(h.apply(captured.topology_checkpoint(), FileMediatedStreamControlRequest::Narrow(request)).is_err());
    assert!(h.complete(begun.topology_checkpoint(), &intent, first + 1).is_err());
    assert_eq!(h.root.bytes(), bytes);
    let (host, _) = h.complete(begun.topology_checkpoint(), &intent, first).unwrap();
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
}

#[test]
fn original_storage_revalidation_withholds_the_pair_and_exact_retry_recovers() {
    let h = History::new(true); let request = FileMediatedStreamControlRequest::Capture(h.capture);
    let canonical = h.root.bytes();
    let run = h.ready(&h.saved, request.clone()).unwrap();
    std::fs::write(h.root.store().join("delivery.bin"), b"changed canonical bytes").unwrap();
    assert_eq!(run.finish_stream_control().err(), Some(Error::Binding.into()));
    std::fs::write(h.root.store().join("delivery.bin"), &canonical).unwrap();
    let run = h.ready(&h.saved, request.clone()).unwrap();
    std::fs::create_dir(h.root.store().join("delivery.pending")).unwrap();
    assert_eq!(run.finish_stream_control().err(), Some(JournalError::InvalidFile));
    assert_eq!(h.root.bytes(), canonical);
    std::fs::remove_dir(h.root.store().join("delivery.pending")).unwrap();
    let result = h.apply(&h.saved, request.clone()).unwrap();
    assert_eq!(result.control().appended_events(), 1); unchanged_source(&h.saved, result.topology_checkpoint());
    let bytes = h.root.bytes(); let again = h.apply(&h.saved, request).unwrap();
    assert_eq!(again.control().appended_events(), 0);
    assert_eq!(again.topology_checkpoint(), result.topology_checkpoint()); assert_eq!(h.root.bytes(), bytes);
    let mut archive = Vec::new(); result.topology_checkpoint().write_to(&mut archive, MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap();
    assert_eq!(TopologyCheckpoint::read_trusted(&mut &archive[..], MAX_TOPOLOGY_CHECKPOINT_BYTES).unwrap(),
        *result.topology_checkpoint());
    // Transport framing rejects inconsistent expenditure, not forged coherent
    // independent custody. The latter remains the trusted caller's obligation.
    archive[32] ^= 1;
    assert!(TopologyCheckpoint::read_trusted(&mut &archive[..], MAX_TOPOLOGY_CHECKPOINT_BYTES).is_err());
}
