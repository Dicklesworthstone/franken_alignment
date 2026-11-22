//! Ordinary stream control preserves exact source custody without inventing a
//! predictor. These actual numerical/effect paths do not qualify the fixture's
//! declared topology as operating-system containment.
use super::*;
use fa_reference::action::consequence::delivery::persistent::{
    governance::PolicyUpdate,
    observed::{
        decoder::learned::{
            FileLearnedStreamCheckpointRequest, FileLearnedStreamControlReceipt,
            FileLearnedStreamNarrowControl, FileLearnedStreamNarrowRequest,
            checkpoint::FileLearnedCheckpointInfo,
        },
        guarded::mediated::predictive::{
            FileMediatedStreamControlHistory, FileMediatedStreamControlRecovery,
            FileMediatedStreamControlRequest,
        },
        mediation::source::TopologySourceRecovery,
    },
};

fn setup(reserved: bool) -> (
    Directory, FileOversightProfile, FileLearnedConfig, FileOversight,
    FileMediatedRoles, FileDriverTopology,
) {
    let root = Directory::new();
    let p = receipt_profile();
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let mut source = support::source(&model);
    source.max_new_tokens = 6;
    let config = FileLearnedConfig::new_text_stream(model.clone(), numerical::tokenizer(&model),
        source, LearnedDecoderBindingLimits::default(), stream_contract()).unwrap()
        .with_required_sidecar().unwrap();
    let (mut host, roles) = start(Lane::Plain, &root, p.clone(), &config, false, reserved);
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let directory = root.store().parent().unwrap();
    let inventory = source_inventory();
    let loaded = LoadedAuthorityTopology::from_json_bytes(
        source_document().as_bytes(), inventory.as_bytes()).unwrap();
    std::fs::write(directory.join("topology.json"), source_document()).unwrap();
    std::fs::write(directory.join("inventory.json"), inventory).unwrap();
    let mut source = roles.topology_observer.bind_topology_source(&host, host.revision(),
        directory.join("topology.json"), directory.join("inventory.json"), loaded).unwrap();
    let revision = host.revision();
    let refreshed = source.refresh(&mut host, revision, 1).unwrap();
    assert!(refreshed.refusal.is_none());
    assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
    assert!(step(&mut host).sample().is_none());
    assert!(step(&mut host).sample().is_none());
    assert!(!host.action_consistency_required());
    assert!(roles.consistency_observer.is_none());
    (root, p, config, host, roles, FileDriverTopology::new(source))
}

fn saved(host: &FileOversight, source: &FileDriverTopology) -> TopologyCheckpoint {
    source.checkpoint(host, source_document().as_bytes(), source_inventory().as_bytes()).unwrap()
}

fn capture(host: &FileOversight) -> FileLearnedStreamCheckpointRequest {
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    let control = host.inspect().control;
    FileLearnedStreamCheckpointRequest::new(host.revision(), FileLearnedCheckpointInfo {
        checkpoint: 71, actor_revision: numerical.actor_revision,
        authority_epoch: control.ledger.epoch, control_sequence: control.sequence,
        stream: 21, position: numerical.position, sampled_draws: numerical.sampled_draws,
    }).unwrap()
}

fn unchanged_source(before: &TopologyCheckpoint, after: &TopologyCheckpoint) {
    assert_eq!(after.expected(), before.expected());
    assert_eq!(after.topology_bytes(), before.topology_bytes());
    assert_eq!(after.inventory_bytes(), before.inventory_bytes());
    assert_eq!(after.work(), before.work());
    assert_eq!(after.last_operation(), before.last_operation());
}

fn begin(root: &Directory, p: &FileOversightProfile, config: &FileLearnedConfig,
    expected: &FileMediatedRequirements, saved: &TopologyCheckpoint,
) -> Result<FileMediatedLearnedRecovery, JournalError> {
    FileOversight::begin_open_mediated_guarded_anchored_with_learned_generation(
        root.store(), p.clone(), expected, config, saved.history_anchor())?
        .require_recovery_reserve(RecoveryReserve::terminal())
}

fn ready(root: &Directory, p: &FileOversightProfile, config: &FileLearnedConfig,
    expected: &FileMediatedRequirements, saved: &TopologyCheckpoint,
    request: FileMediatedStreamControlRequest,
) -> Result<FileMediatedStreamControlRecovery, JournalError> {
    let mut run = begin(root, p, config, expected, saved)?
        .require_stream_control_checkpoint(saved, request)?;
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1)?;
    }
    Ok(run)
}

fn apply(root: &Directory, p: &FileOversightProfile, config: &FileLearnedConfig,
    expected: &FileMediatedRequirements, saved: &TopologyCheckpoint,
    request: FileMediatedStreamControlRequest,
) -> Result<FileMediatedStreamControlHistory, JournalError> {
    ready(root, p, config, expected, saved, request)?.finish_stream_control()
}

#[test]
fn ordinary_mediated_capture_retains_source_work_and_exact_lost_export_retries() {
    let (root, p, config, host, _roles, source) = setup(true);
    let expected = requirements_for(&host, &p, &config);
    let original = saved(&host, &source);
    let request = capture(&host);
    let numerical = host.learned_generation_inspection().unwrap().numerical;
    drop(source); drop(host);
    // The source custodian's retained pair, rather than fresh file reads, is
    // the only source of offline custody and already-spent work.
    std::fs::remove_file(root.store().parent().unwrap().join("topology.json")).unwrap();
    std::fs::remove_file(root.store().parent().unwrap().join("inventory.json")).unwrap();
    let selection = FileMediatedStreamControlRequest::Capture(request);
    let result = apply(&root, &p, &config, &expected, &original, selection.clone()).unwrap();
    assert_eq!(result.control().appended_events(), 1);
    assert_eq!(result.control().snapshot().revision, request.revision() + 1);
    assert_eq!(result.control().numerical(), &numerical);
    assert_eq!(result.control().recovery_usage().restart_attempts, 0);
    assert_eq!(result.control().snapshot().executions, 0);
    assert_eq!(result.control().snapshot().control.ledger.charged, 0);
    assert_eq!(result.control().history_anchor(), result.topology_checkpoint().history_anchor());
    unchanged_source(&original, result.topology_checkpoint());
    let bytes = root.bytes();
    // Both the exact old pair across its single selected event and the new
    // acknowledged pair identify the same already-captured checkpoint.
    for pair in [&original, result.topology_checkpoint()] {
        let again = apply(&root, &p, &config, &expected, pair, selection.clone()).unwrap();
        assert_eq!(again.control().appended_events(), 0);
        assert_eq!(again.control().receipt(), result.control().receipt());
        assert_eq!(again.topology_checkpoint(), result.topology_checkpoint());
        assert_eq!(root.bytes(), bytes);
    }
    let mut changed = request.checkpoint(); changed.checkpoint += 1;
    let wrong = FileMediatedStreamControlRequest::Capture(
        FileLearnedStreamCheckpointRequest::new(request.revision(), changed).unwrap());
    for pair in [&original, result.topology_checkpoint()] {
        assert!(apply(&root, &p, &config, &expected, pair, wrong.clone()).is_err());
        assert_eq!(root.bytes(), bytes);
    }
}

#[test]
fn ordinary_mediated_capture_keeps_every_guard_and_canonical_boundary() {
    let (root, p, config, host, _roles, source) = setup(true);
    let expected = requirements_for(&host, &p, &config);
    let original = saved(&host, &source);
    let request = capture(&host);
    drop(source); drop(host);
    let selection = FileMediatedStreamControlRequest::Capture(request);
    let bytes = root.bytes();
    let staging = root.store().join("delivery.pending");
    std::fs::write(&staging, b"preserve unrelated staging").unwrap();
    for field in 0..7 {
        let mut wrong = expected.clone();
        match field {
            0 => wrong.oversight.guards.identity = None,
            1 => wrong.oversight.guards.stream = None,
            2 => wrong.oversight.guards.campaigns = None,
            3 => wrong.evaluation = None,
            4 => wrong.topology.available = !wrong.topology.available,
            5 => wrong.oversight.minimum.authority_epoch += 1,
            _ => wrong.oversight.effective_policy = Policy::new(2, vec![]).unwrap(),
        }
        assert!(apply(&root, &p, &config, &wrong, &original, selection.clone()).is_err(), "field {field}");
        assert_eq!(root.bytes(), bytes);
        assert_eq!(std::fs::read(&staging).unwrap(), b"preserve unrelated staging");
    }
    let run = begin(&root, &p, &config, &expected, &original).unwrap()
        .require_stream_control_checkpoint(&original, selection.clone()).unwrap();
    assert_eq!(run.finish_stream_control().err(), Some(Error::Incomplete.into()));
    let mut run = begin(&root, &p, &config, &expected, &original).unwrap();
    run.advance(0, 1).unwrap();
    assert_eq!(run.require_stream_control_checkpoint(&original, selection.clone()).err(),
        Some(Error::WrongState.into()));
    let run = ready(&root, &p, &config, &expected, &original, selection.clone()).unwrap();
    let mut changed = bytes.clone(); let last = changed.len() - 1; changed[last] ^= 1;
    std::fs::write(root.store().join("delivery.bin"), &changed).unwrap();
    assert_eq!(run.finish_stream_control().err(), Some(Error::Binding.into()));
    assert_eq!(root.bytes(), changed);
    assert_eq!(std::fs::read(&staging).unwrap(), b"preserve unrelated staging");
    std::fs::write(root.store().join("delivery.bin"), &bytes).unwrap();
    let result = apply(&root, &p, &config, &expected, &original, selection).unwrap();
    assert_eq!(result.control().appended_events(), 1);
    assert_eq!(result.control().numerical().sampled_draws, 0);
}

#[test]
fn ordinary_mediated_control_preserves_terminal_capacity_and_exact_narrow_checkpoint() {
    for (reserved, remaining) in [(false, None), (true, None), (true, Some(0)), (true, Some(1))] {
        let (root, p, config, mut host, _roles, source) = setup(reserved);
        if let Some(remaining) = remaining {
            while host.journal_capacity().unwrap().ordinary_remaining().events > remaining {
                host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
            }
        }
        let expected = requirements_for(&host, &p, &config);
        let original = saved(&host, &source);
        let capture = capture(&host);
        let control = host.inspect().control;
        let mut wrong_checkpoint = capture.checkpoint();
        wrong_checkpoint.position += 1;
        let narrow = FileLearnedStreamNarrowRequest::new(host.revision(), wrong_checkpoint,
            FileLearnedStreamNarrowControl {
                operation: 800, expected_control_sequence: control.sequence,
                expected_actor_revision: capture.checkpoint().actor_revision,
                expected_authority_epoch: control.ledger.epoch,
                binding: ReviewBinding { round: 800, reducer_generation: 1, evidence_root: [8; 32] },
                retained_targets: vec![host.inspect().target],
            }).unwrap();
        let bytes = root.bytes(); drop(source); drop(host);
        assert!(apply(&root, &p, &config, &expected, &original,
            FileMediatedStreamControlRequest::Narrow(narrow)).is_err());
        assert_eq!(root.bytes(), bytes);
        let result = apply(&root, &p, &config, &expected, &original,
            FileMediatedStreamControlRequest::Capture(capture));
        if reserved && remaining != Some(0) {
            assert_eq!(result.unwrap().control().appended_events(), 1);
        } else {
            assert!(result.is_err()); assert_eq!(root.bytes(), bytes);
        }
    }
}

#[test]
fn ordinary_offline_reset_intent_reaches_original_completion_fresh_source_and_two_key_publication() {
    let (root, p, config, mut host, old_roles, source) = setup(true);
    let original = host.learned_generation_inspection().unwrap().numerical;
    let checkpoint = host.capture_learned_stream_checkpoint(host.revision(), 71,
        original.actor_revision, host.inspect().control.ledger.epoch).unwrap();
    let (samples, _) = finish_generation(&mut host);
    assert_eq!(samples.len(), 3);
    fresh_identity(&mut host, &old_roles.oversight, 1, 1);
    host.propose_learned_text_stream_message(host.revision(), 71, ElapsedTick(100),
        Vec::new(), snapshot()).unwrap();
    let state = host.inspect();
    let update = PolicyUpdate::new(800, state.control.sequence, state.control.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(4096)]).unwrap()).unwrap();
    let campaign = host.request_policy_campaign(host.revision(), &update).unwrap();
    assert!(!campaign.report().cases().is_empty() && !campaign.report().requires_shadow());
    let revision = host.revision();
    let approved = old_roles.oversight.policy_governor.as_ref().unwrap()
        .approve(&mut host, revision, &campaign, false).unwrap();
    host.promote_policy_campaign(host.revision(), &approved).unwrap();
    let first = host.inspect().control.sequence;
    assert!(first > checkpoint.info().control_sequence);
    let spent = host.learned_generation_inspection().unwrap().numerical;
    let current = host.inspect();
    let model = numerical::model(&[u32::from(b'O'), u32::from(b'K'), numerical::END]);
    let intent = FileLearnedResetIntent::for_recovery(71, FileResetRequest {
        operation: 900, expected_control_sequence: first,
        expected_actor_revision: spent.actor_revision,
        expected_authority_epoch: current.control.ledger.epoch,
        binding: ReviewBinding { round: 900, reducer_generation: 1, evidence_root: [9; 32] },
        retained_targets: vec![current.target],
    }, KvRestartBudget { cache_values: MAX_MODEL_KV_VALUES,
        audit: support::source(&model).policy.allowance() }).unwrap();
    let original_pair = saved(&host, &source);
    let mut expected = requirements_for(&host, &p, &config);
    expected.oversight.effective_policy = host.current_policy().unwrap().clone();
    let directory = root.store().parent().unwrap().to_owned();
    drop(source); drop(host);
    std::fs::remove_file(directory.join("topology.json")).unwrap();
    std::fs::remove_file(directory.join("inventory.json")).unwrap();
    let selection = FileMediatedStreamControlRequest::BeginReset {
        intent: intent.clone(), first_incident_sequence: first,
    };
    let begun = apply(&root, &p, &config, &expected, &original_pair, selection.clone()).unwrap();
    assert_eq!(begun.control().appended_events(), 1);
    assert_eq!(begun.control().numerical(), &spent);
    assert_eq!(begun.control().recovery_usage().restart_attempts, 0);
    assert!(matches!(begun.control().receipt(), FileLearnedStreamControlReceipt::ResetBegun {
        already_completed: false, .. }));
    unchanged_source(&original_pair, begun.topology_checkpoint());
    let bytes = root.bytes();
    let retry = apply(&root, &p, &config, &expected, &original_pair, selection).unwrap();
    assert_eq!(retry.control().appended_events(), 0);
    assert_eq!(retry.topology_checkpoint(), begun.topology_checkpoint());
    assert_eq!(root.bytes(), bytes);
    assert!(apply(&root, &p, &config, &expected, &original_pair,
        FileMediatedStreamControlRequest::BeginReset {
            intent: intent.clone(), first_incident_sequence: first + 1,
        }).is_err());
    assert_eq!(root.bytes(), bytes);
    let mut run = begin(&root, &p, &config, &expected, begun.topology_checkpoint()).unwrap()
        .require_stream_reset_checkpoint(begun.topology_checkpoint(), &intent, first).unwrap();
    while run.progress().status == FileLearnedRecoveryStatus::Replaying {
        run.advance(run.progress().replayed_events, 1).unwrap();
    }
    let (mut host, roles) = run.finish_pending_stream_reset().unwrap();
    assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    assert_eq!(host.learned_recovery_usage().unwrap().restart_attempts, 1);
    let restored = host.learned_generation_inspection().unwrap();
    assert!(restored.paused);
    assert_eq!(restored.numerical.position, original.position);
    assert_eq!(restored.numerical.sampled_draws, 0);
    assert_eq!(restored.numerical.cumulative_work, spent.cumulative_work);
    assert!(roles.consistency_observer.is_none());
    assert!(!host.mediation_snapshot().unwrap().available);
    assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    let completed = begun.topology_checkpoint()
        .after_learned_stream_reset(&host, &intent, first).unwrap();
    unchanged_source(&original_pair, &completed);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let next_document = source_document().replacen("\"generation\":1", "\"generation\":2", 1)
        .replace("\"inventory_generation\":1", "\"inventory_generation\":2")
        .replace("\"complete_inventory_generation\":1", "\"complete_inventory_generation\":2");
    let inventory = source_inventory();
    std::fs::write(directory.join("topology.json"), &next_document).unwrap();
    std::fs::write(directory.join("inventory.json"), &inventory).unwrap();
    let next = LoadedAuthorityTopology::from_json_bytes(
        next_document.as_bytes(), inventory.as_bytes()).unwrap();
    let revision = host.revision();
    let mut fresh = roles.topology_observer.resume_topology_source(&mut host, revision, 2,
        TopologySourceRecovery { topology_path: directory.join("topology.json"),
            inventory_path: directory.join("inventory.json"), previous: completed.expected().clone(),
            next, work: completed.work(), last_operation: completed.last_operation() }).unwrap();
    assert_eq!(fresh.work(), completed.work());
    let revision = host.revision();
    let refreshed = fresh.refresh(&mut host, revision, 3).unwrap();
    assert!(matches!(refreshed.checked, Some(Ok(CutCheck::Verified(_)))));
    assert_eq!(fresh.work().checks, completed.work().checks + 1);
    let n = host.learned_generation_inspection().unwrap().numerical;
    host.resume_learned_generation(host.revision(), n.actor_revision, n.position).unwrap();
    let (samples, last) = finish_generation(&mut host);
    assert_eq!(samples.len(), 3);
    fresh_identity(&mut host, &roles.oversight, 2, 2);
    let (actor, mut supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let ticket = actor.submit(72, LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let FileRequestDisposition::Admitted { attempt, .. } = host.request_status(72).unwrap().disposition
            else { panic!("fresh original stream request was not admitted"); };
        let (action, input, automatic, human_request) = support::prepared(&mut host, 72, 102, &last);
        let revision = host.revision();
        assert!(host.publish_checked(revision, attempt, Some(&input), snapshot(), ElapsedTick(3)).is_err());
        let revision = host.revision();
        let human = roles.oversight.human.approve(&mut host, revision, &human_request).unwrap();
        let revision = host.revision();
        host.dispatch(revision, &automatic, &human, &action, &input, snapshot()).unwrap();
        let revision = host.revision();
        let receipt = host.publish_checked(revision, attempt, Some(&input), snapshot(), ElapsedTick(3)).unwrap();
        assert_eq!(receipt.outcome, EndpointOutcome::Executed { resulting_version: 2 });
        let revision = host.revision(); host.reconcile(revision, attempt).unwrap();
        assert_eq!(host.inspect().control.ledger.charged, action.spec().units);
        assert_eq!(host.inspect().executions, 1);
        assert_eq!(host.learned_generation_inspection().unwrap().numerical.cumulative_work.sampling_attempts, 6);
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    }
    assert!(matches!(actor.poll(&ticket), Knowledge::Known { value: ActorOutcome::Executed, .. }));
}
