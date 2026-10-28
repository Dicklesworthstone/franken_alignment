//! One real mediated plain-stream reset with the original file-source checker.
//! The fixture graph is declared coverage, not evidence of host isolation.
use super::*;
use fa_reference::action::consequence::delivery::persistent::governance::PolicyUpdate;
use fa_reference::action::consequence::delivery::persistent::observed::{
    mediation::FileMediationUpdate,
    guarded::learned_recovery::FileGuardedLearnedRecovery,
};
use fa_reference::action::consequence::oversight::learned_source::LearnedAvailability;

fn document_at(generation: u64) -> String {
    source_document().replacen("\"generation\":1", &format!("\"generation\":{generation}"), 1)
        .replace("\"inventory_generation\":1", &format!("\"inventory_generation\":{generation}"))
        .replace("\"complete_inventory_generation\":1", &format!("\"complete_inventory_generation\":{generation}"))
}

fn recovery(root: &Directory, profile: &FileOversightProfile, config: &FileLearnedConfig,
    expected: &FileMediatedRequirements, anchor: &FileHistoryAnchor) -> FileMediatedLearnedRecovery
{
    let mut run = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        root.store(), profile.clone(), expected, config, anchor).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 2).unwrap();
    }
    assert_eq!(run.progress().status, FileLearnedRecoveryStatus::Ready);
    run
}

#[test]
fn mediated_pending_stream_reset_preserves_full_custody_and_requires_a_new_native_source_cut() {
    let root = Directory::new(); let p = receipt_profile();
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let mut text = support::source(&model);
    // The abandoned three draws stay spent after the rewind. This same real
    // fixture needs room for the original post-reset continuation as well.
    text.max_new_tokens = 6; let audit = text.policy.allowance();
    let config = FileLearnedConfig::new_text_stream(model.clone(), numerical::tokenizer(&model),
        text, LearnedDecoderBindingLimits::default(), stream_contract()).unwrap()
        .with_required_sidecar().unwrap();
    let (mut host, old_roles) = start(Lane::Plain, &root, p.clone(), &config, false, false);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    assert!(step(&mut host).sample().is_none());
    let directory = root.store().parent().unwrap().to_owned();
    let topology_path = directory.join("topology.json"); let inventory_path = directory.join("inventory.json");
    let inventory = source_inventory(); let first_document = document_at(1);
    let first_topology = LoadedAuthorityTopology::from_json_bytes(first_document.as_bytes(), inventory.as_bytes()).unwrap();
    assert_eq!(first_topology.graph(), &graph(1, false));
    std::fs::write(&topology_path, &first_document).unwrap(); std::fs::write(&inventory_path, &inventory).unwrap();
    let mut source = old_roles.topology_observer.bind_topology_source(&host, host.revision(),
        topology_path.clone(), inventory_path.clone(), first_topology).unwrap();
    let revision = host.revision(); let refreshed = source.refresh(&mut host, revision, 1).unwrap();
    assert!(refreshed.refusal.is_none());
    assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
    let original = host.learned_generation_inspection().unwrap().numerical;
    let saved = host.capture_learned_stream_checkpoint(host.revision(), 1,
        original.actor_revision, host.inspect().control.ledger.epoch).unwrap();
    assert!(step(&mut host).sample().is_none());
    // The real graph/inventory successor advances its revocation floor, but
    // does NOT advance the original containment control sequence.
    let second_document = document_at(2);
    let second_topology = LoadedAuthorityTopology::from_json_bytes(second_document.as_bytes(), inventory.as_bytes()).unwrap();
    std::fs::write(&topology_path, &second_document).unwrap();
    let revision = host.revision(); source.rebind(&mut host, revision, 2, second_topology.clone()).unwrap().unwrap();
    assert_eq!(host.inspect().control.sequence, saved.info().control_sequence);
    let revision = host.revision(); let refreshed = source.refresh(&mut host, revision, 3).unwrap();
    assert!(refreshed.refusal.is_none());
    assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
    assert_eq!(source.work().checks, 2);
    let (samples, _) = finish_generation(&mut host); assert_eq!(samples.len(), 3);
    fresh_identity(&mut host, &old_roles.oversight, 1, 1);
    host.propose_learned_text_stream_message(host.revision(), 1, ElapsedTick(100),
        Vec::new(), snapshot()).unwrap();
    let control = host.inspect().control;
    let update = PolicyUpdate::new(800, control.sequence, control.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(4096)]).unwrap()).unwrap();
    let campaign = host.request_policy_campaign(host.revision(), &update).unwrap();
    assert!(!campaign.report().cases().is_empty() && !campaign.report().requires_shadow());
    let revision = host.revision(); let promotion = old_roles.oversight.policy_governor.as_ref().unwrap()
        .approve(&mut host, revision, &campaign, false).unwrap();
    host.promote_policy_campaign(host.revision(), &promotion).unwrap();
    // Original full-corpus policy promotion supplies the strict later control
    // frontier. It remains a supervisor declaration, not incident attestation.
    let first_incident = host.inspect().control.sequence;
    assert!(first_incident > saved.info().control_sequence);
    let spent = host.learned_generation_inspection().unwrap().numerical;
    let current = host.inspect();
    let intent = FileLearnedResetIntent::for_recovery(saved.id(), FileResetRequest {
        operation: 900, expected_control_sequence: current.control.sequence,
        expected_actor_revision: host.learned_generation_inspection().unwrap().numerical.actor_revision,
        expected_authority_epoch: current.control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [8; 32] },
        retained_targets: vec![current.target],
    }, KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES, audit }).unwrap();
    host.begin_learned_stream_reset(host.revision(), &saved, intent.control().clone(),
        intent.budget(), first_incident).unwrap();
    let mut expected = requirements_for(&host, &p, &config);
    expected.oversight.effective_policy = host.current_policy().unwrap().clone();
    let anchor = host.history_anchor().unwrap();
    assert_eq!(expected.topology.current, *second_topology.graph()); assert!(expected.topology.available);
    let cut = host.revision(); let original_bytes = root.bytes(); drop(host);

    let mut wrong = expected.clone(); wrong.topology.current = wrong.topology.initial.clone();
    assert_eq!(FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        root.store(), p.clone(), &wrong, &config, &anchor).err(), Some(Error::Binding.into()));
    // The complete base guard wrapper cannot strip the installed mediation.
    let weaker: Result<FileGuardedLearnedRecovery, JournalError> =
        FileOversight::begin_open_guarded_with_learned_generation(root.store(), p.clone(), &expected.oversight, &config);
    assert_eq!(weaker.err(), Some(Error::Binding.into()));
    for field in 0..3 {
        let mut wrong = expected.clone();
        let error = match field {
            0 => { wrong.topology.available = false; Error::Binding }
            1 => { wrong.oversight.minimum.authority_epoch += 1; Error::Stale }
            _ => { wrong.oversight.guards.identity = None; Error::Binding }
        };
        let mut run = FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
            root.store(), p.clone(), &wrong, &config, &anchor).unwrap();
        assert_eq!(run.advance(0, run.progress().total_events).err(), Some(error.into()));
        assert_eq!(run.finish_pending_stream_reset(&intent, first_incident).err(), Some(error.into()));
        assert_eq!(root.bytes(), original_bytes);
    }
    assert_eq!(recovery(&root, &p, &config, &expected, &anchor)
        .finish_pending_stream_reset(&intent, first_incident + 1).err(), Some(Error::Binding.into()));
    assert_eq!(root.bytes(), original_bytes);
    let (mut host, roles) = recovery(&root, &p, &config, &expected, &anchor)
        .finish_pending_stream_reset(&intent, first_incident).unwrap();
    assert_eq!(host.revision(), cut + 2);
    assert!(host.pending_learned_reset().unwrap().is_none());
    assert!(host.learned_reset_result(900).unwrap().unwrap().control.restored);
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    let restored = host.learned_generation_inspection().unwrap().numerical;
    assert_eq!(restored.position, original.position); assert_eq!(restored.work, original.work);
    assert_eq!(restored.sampled_draws, original.sampled_draws);
    assert_eq!(restored.cumulative_work, spent.cumulative_work);
    assert_eq!(restored.availability, LearnedAvailability::Empty);
    assert_eq!(host.current_policy().unwrap(), &expected.oversight.effective_policy);
    let mediation = host.mediation_snapshot().unwrap();
    assert_eq!(mediation.graph, expected.topology.current);
    assert!(!mediation.available && mediation.accepted.is_none());
    assert!(roles.oversight.identity_observer.is_some() && roles.oversight.policy_governor.is_some());
    assert!(roles.evaluator.is_some() && roles.consistency_observer.is_none());
    assert!(host.learned_generation_inspection().unwrap().paused); assert!(!host.clock_ready());
    assert_eq!(host.inspect().executions, 0); assert_eq!(host.inspect().control.ledger.charged, 0);
    let bytes = root.bytes(); let revision = host.revision(); let source_work = source.work();
    assert_eq!(source.refresh(&mut host, revision, 4).err(), Some(Error::Binding.into()));
    assert_eq!(source.work(), source_work); assert_eq!(root.bytes(), bytes);
    assert_eq!(std::fs::read(&topology_path).unwrap(), second_document.as_bytes());
    assert_eq!(std::fs::read(&inventory_path).unwrap(), inventory.as_bytes());

    // Returned custody is real, but the Fence has withdrawn the old cut. Renew
    // coverage only through a newer declared graph and actual original reads.
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let third_document = document_at(3);
    let third_topology = LoadedAuthorityTopology::from_json_bytes(third_document.as_bytes(), inventory.as_bytes()).unwrap();
    std::fs::write(&topology_path, &third_document).unwrap();
    let update = FileMediationUpdate { operation: 901, expected_generation: 2,
        expected_authority_epoch: host.inspect().control.ledger.epoch, next: Some(third_topology.graph().clone()) };
    let revision = host.revision(); roles.topology_observer.update(&mut host, revision, &update).unwrap().unwrap();
    let mut fresh = roles.topology_observer.bind_topology_source(&host, host.revision(),
        topology_path, inventory_path, third_topology).unwrap();
    let revision = host.revision(); let refreshed = fresh.refresh(&mut host, revision, 1).unwrap();
    assert!(refreshed.refusal.is_none());
    assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
    assert!(host.mediation_snapshot().unwrap().accepted.is_some());
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    assert!(step(&mut host).sample().is_none());
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.position, original.position + 1);
    assert_eq!(host.inspect().executions, 0);
}
