//! Text production after cooperative reconstruction uses the original source.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::decoder::learned::FileLearnedRecoveryStatus;

fn reopen_in_quanta(root: &Directory, config: &FileLearnedConfig) -> (FileOversight, FileHumanReviewer) {
    let mut recovery = FileOversight::begin_open_with_learned_generation(root.store(), profile(), config).unwrap();
    while recovery.progress().status == FileLearnedRecoveryStatus::Replaying {
        let before = recovery.progress();
        let after = recovery.advance(before.replayed_events, 1).unwrap();
        assert_eq!(after.replayed_events, before.replayed_events + 1);
    }
    assert_eq!(recovery.progress().status, FileLearnedRecoveryStatus::Ready);
    recovery.finish().unwrap()
}

#[test]
fn cooperative_recovery_continues_original_text_and_requires_both_publication_keys() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let source = config(&model);
    let recipe = recipe(&model, source.clone());
    let mut original = model.observed_learned_text_generation(tokenizer(&model), source).unwrap();
    original.run_to_stop().unwrap();
    let expected = original.text_message(LearnedEvidenceLimits::default()).unwrap();
    let root = Directory::new();
    let (mut host, _) = owner(&root, &recipe);
    step(&mut host);
    step(&mut host);
    let before = host.learned_generation_inspection().unwrap().numerical;
    drop(host);
    let (mut host, reviewer) = reopen_in_quanta(&root, &recipe);
    assert_eq!(host.learned_generation_inspection().unwrap().numerical, before);
    assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
    resume(&mut host);
    finish(&mut host);
    let actual = host.learned_text_message(LearnedEvidenceLimits::default()).unwrap();
    assert_eq!(actual.bytes(), expected.bytes());
    assert_eq!(actual.work(), expected.work());
    assert_eq!(actual.telemetry_work(), expected.telemetry_work());
    assert_eq!(actual.evidence().tokens(), expected.evidence().tokens());
    let changed = action_spec(&host, b"substituted output");
    assert_eq!(host.propose(host.revision(), 1, changed, snapshot()).err(), Some(JournalError::Contract(Error::Binding)));
    let (action, inputs, automatic, request) = prepared(&mut host);
    assert_eq!(host.inspect().executions, 0);
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    assert!(host.publish(host.revision(), 1).is_err());
    assert_eq!(host.publish_checked(host.revision(), 1, Some(&inputs), snapshot(), ElapsedTick(3)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 1).unwrap();
    assert_eq!(host.inspect().payload, b"OK");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn cooperative_recovery_cannot_turn_a_pending_final_stop_into_completed_text() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let recipe = recipe(&model, config(&model));
    let root = Directory::new();
    let (mut host, _) = owner(&root, &recipe);
    for _ in 0..3 { step(&mut host); }
    let before = host.learned_generation_inspection().unwrap().numerical;
    host.begin_learned_step(host.revision(), before.actor_revision, before.position).unwrap();
    drop(host);
    let (mut host, _) = reopen_in_quanta(&root, &recipe);
    resume(&mut host);
    let recovered = host.learned_generation_inspection().unwrap();
    assert_eq!(recovered.numerical, before);
    assert_eq!(recovered.pending, Some(LearnedStepIntent {
        actor_revision: before.actor_revision, position: before.position,
    }));
    assert!(host.learned_text_message(LearnedEvidenceLimits::default()).is_err());
    let target = destination(&host);
    assert_eq!(host.propose_learned_text(host.revision(), 1, target, snapshot()).err(),
        Some(JournalError::Contract(Error::Incomplete)));
    assert_eq!(host.inspect().executions, 0);
    host.complete_learned_step(host.revision(), before.actor_revision, before.position).unwrap().unwrap();
    assert_eq!(host.learned_text_message(LearnedEvidenceLimits::default()).unwrap().bytes(), b"OK");
    assert_eq!(host.learned_generation_inspection().unwrap().numerical.sampled_draws, before.sampled_draws + 1);
}
