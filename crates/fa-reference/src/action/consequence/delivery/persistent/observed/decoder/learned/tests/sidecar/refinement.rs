//! Original commit/reveal completion, disclosure purchase and durable barriers.
use super::*;
use super::super::super::sidecar::FileLearnedSidecarFinish;
use crate::action::consequence::oversight::sidecar::SidecarRefinementOutcome;

fn plan(host: &mut FileOversight, rounds: usize) -> (crate::action::FrozenAction, FileLearnedSidecar) {
    step(host).unwrap();
    let action = host.propose(host.revision(), 1, action_spec(host), snapshot()).unwrap();
    let actor = host.learned_generation_inspection().unwrap().numerical.actor_revision;
    let evidence = host.machine.broker.learned_decoder_evidence(1).unwrap().unwrap();
    let mut request = request();
    request.priority = evidence.audit().source().groups().take(1).collect();
    assert_eq!(request.priority.len(), 1);
    request.budget.rounds = rounds;
    let handle = host.begin_learned_sidecar_plan(host.revision(), 1, actor, request).unwrap();
    (action, handle)
}
fn completed(host: &mut FileOversight, round: u64, verdict: Option<Verdict>) {
    let now = host.inspect().control.ledger.elapsed.unwrap().0;
    let window = ReviewWindow { commit_by: ElapsedTick(now + 4), reveal_by: ElapsedTick(now + 8) };
    host.begin_review(host.revision(), 1, round, [9; 32], window, snapshot()).unwrap();
    if let Some(verdict) = verdict {
        let digest = commitment(round, "reviewer", &[9; 32], verdict, b"original salt").unwrap();
        host.commit_review(host.revision(), round, "reviewer", digest).unwrap();
        host.open_reveals(host.revision(), round).unwrap();
        host.reveal_review(host.revision(), round, "reviewer", verdict, b"original salt".to_vec()).unwrap();
    } else {
        host.observe_time(host.revision(), window.reveal_by).unwrap();
    }
}

#[test]
fn durable_abstention_buys_one_original_residual_then_requires_a_fresh_allow_and_both_keys() {
    let root = Directory::new(); let config = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, reviewer) = owner(&root, &config);
    let (action, mut handle) = plan(&mut host, 2);
    let original = host.checked_learned_sidecar(&handle).unwrap();
    let coarse = original.round().clone();
    let sequence = host.inspect().control.sequence;
    completed(&mut host, 101, Some(Verdict::Abstain));
    let result = host.finish_learned_sidecar_review(host.revision(), &mut handle, 101, snapshot()).unwrap();
    let FileLearnedSidecarFinish::Refined { group, input_revision, archive } = result else { panic!("original refinement"); };
    assert_eq!(archive.policy.abstained, vec!["reviewer".to_owned()]);
    assert_eq!(archive.inputs.as_ref(), coarse.input());
    assert_eq!(input_revision, 2);
    assert_eq!(handle.input_revision(), 2);
    let refined = host.checked_learned_sidecar(&handle).unwrap().round().clone();
    assert_eq!(refined.selected_groups(), &[group]);
    assert_eq!(refined.work().rounds, 2);
    assert!(refined.work().residual_bytes > 0);
    assert_eq!(host.inspect().control.sequence, sequence);
    assert!(host.authorize(host.revision(), 1, refined.input(), snapshot()).is_err());
    assert_eq!(host.inspect().executions, 0);
    completed(&mut host, 102, Some(Verdict::Allow));
    let result = host.finish_learned_sidecar_review(host.revision(), &mut handle, 102, snapshot()).unwrap();
    let FileLearnedSidecarFinish::Applied { outcome: Some(SidecarRefinementOutcome::Final), receipt, archive } = result else { panic!("original final apply"); };
    assert!(receipt.is_ok());
    assert!(archive.policy.abstained.is_empty() && archive.policy.missing.is_empty());
    assert_eq!(archive.inputs.as_ref(), refined.input());
    assert_eq!(read_machine(&host, &config).retained_learned_sidecar(1, host.revision()).unwrap().packet, refined);
    let automatic = host.authorize(host.revision(), 1, refined.input(), snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, refined.input(), ElapsedTick(10)).unwrap();
    let revision = host.revision(); let human = reviewer.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, refined.input(), snapshot()).unwrap();
    let result = host.publish_checked(host.revision(), 1, Some(refined.input()), snapshot(), ElapsedTick(2)).unwrap();
    assert!(matches!(result.outcome, EndpointOutcome::Executed { .. }));
    assert_eq!(host.inspect().executions, 1);
}

#[test]
fn missing_votes_and_exhausted_round_budget_never_purchase_a_residual() {
    for (rounds, verdict) in [(2, None), (1, Some(Verdict::Abstain))] {
        let root = Directory::new(); let config = config(false, 1).with_required_sidecar().unwrap();
        let (mut host, _) = owner(&root, &config);
        let (_, mut handle) = plan(&mut host, rounds);
        let before = host.retained_learned_sidecar(1).unwrap().packet;
        completed(&mut host, 101, verdict);
        let result = host.finish_learned_sidecar_review(host.revision(), &mut handle, 101, snapshot()).unwrap();
        let FileLearnedSidecarFinish::Applied { outcome, receipt, archive } = result else { panic!("unfunded/missing round applied originally"); };
        if verdict.is_none() {
            assert!(matches!(outcome, Some(SidecarRefinementOutcome::Missing { .. })));
            assert_eq!(archive.policy.missing, vec!["reviewer".to_owned()]);
        } else {
            assert!(matches!(outcome, Some(SidecarRefinementOutcome::BudgetExhausted { .. })));
            assert_eq!(archive.policy.abstained, vec!["reviewer".to_owned()]);
        }
        assert!(receipt.is_ok());
        assert_eq!(host.retained_learned_sidecar(1).unwrap().packet, before);
        assert_eq!(handle.input_revision(), 1);
        assert!(host.authorize(host.revision(), 1, before.input(), snapshot()).is_err());
        assert!(!host.machine.sessions.contains_key(&101));
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn a_frozen_schedule_can_finish_without_buying_an_unused_refinement() {
    let root = Directory::new(); let config = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, _) = owner(&root, &config);
    let (_, mut handle) = plan(&mut host, 2);
    let before = host.retained_learned_sidecar(1).unwrap().packet;
    completed(&mut host, 101, Some(Verdict::Abstain));
    let result = host.finish_learned_sidecar_review_inner(host.revision(), &mut handle, 101, false, snapshot()).unwrap();
    let FileLearnedSidecarFinish::Applied { outcome: None, receipt, archive } = result else { panic!("original apply without purchase"); };
    assert!(receipt.is_ok());
    assert_eq!(archive.policy.abstained, vec!["reviewer".to_owned()]);
    assert_eq!(host.retained_learned_sidecar(1).unwrap().packet, before);
    assert_eq!(handle.input_revision(), 1);
    assert!(host.authorize(host.revision(), 1, before.input(), snapshot()).is_err());
}

#[test]
fn original_apply_refusal_consumes_the_round_and_retains_its_complete_archive() {
    let root = Directory::new(); let config = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, _) = owner(&root, &config);
    let (_, mut handle) = plan(&mut host, 2);
    completed(&mut host, 101, Some(Verdict::Allow));
    let mut changed = snapshot(); changed.values.insert(7, b"changed".to_vec());
    let before = host.revision();
    let result = host.finish_learned_sidecar_review(before, &mut handle, 101, changed).unwrap();
    let FileLearnedSidecarFinish::Applied { receipt: Err(_), archive, .. } = result else { panic!("committed original refusal"); };
    assert_eq!(host.revision(), before + 1);
    assert_eq!(archive.policy.transcript.commits.len(), 1);
    assert_eq!(archive.policy.transcript.reveals.len(), 1);
    assert!(archive.policy.missing.is_empty());
    assert!(!host.machine.sessions.contains_key(&101));
    let before = host.revision();
    assert_eq!(host.finish_learned_sidecar_review(before, &mut handle, 101, snapshot()).err(), Some(JournalError::Contract(Error::Missing)));
    assert_eq!(host.revision(), before);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn corrupted_completion_expectation_cannot_become_a_refinement_or_apply_on_replay() {
    let root = Directory::new(); let config = config(false, 1).with_required_sidecar().unwrap();
    let (mut host, _) = owner(&root, &config);
    let (_, mut handle) = plan(&mut host, 2);
    completed(&mut host, 101, Some(Verdict::Abstain));
    host.finish_learned_sidecar_review(host.revision(), &mut handle, 101, snapshot()).unwrap();
    let mut events = host.events.clone();
    let Event::Decoder(DecoderEvent::Learned(LearnedEvent::Sidecar(SidecarEvent::Finish { expected, .. })))
        = events.last_mut().unwrap() else { panic!("original finish event"); };
    let mut bytes = expected.to_vec(); let end = bytes.len() - 1; bytes[end] ^= 1;
    *expected = bytes.into();
    assert_eq!(Machine::replay(&host.profile, &events).err(), Some(Error::Binding));
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn storage_failure_never_updates_a_live_handle_or_exposes_an_unacknowledged_residual() {
    for barrier in [JournalIo::Stage, JournalIo::Write, JournalIo::FileSync, JournalIo::Rename, JournalIo::DirectorySync] {
        let root = Directory::new(); let config = config(false, 1).with_required_sidecar().unwrap();
        let (mut host, _) = owner(&root, &config);
        let (_, mut handle) = plan(&mut host, 2);
        completed(&mut host, 101, Some(Verdict::Abstain));
        let before = host.revision();
        host.store.fail_once(barrier);
        let error = host.finish_learned_sidecar_review(before, &mut handle, 101, snapshot()).unwrap_err();
        let JournalError::Io(failure) = error else { panic!("original storage barrier"); };
        assert_eq!(failure.operation, barrier);
        assert_eq!(host.revision(), before);
        assert_eq!(handle.input_revision(), 1);
        assert_eq!(host.input_revision(1).unwrap(), 1);
        assert_eq!(host.current_learned_sidecar(&handle).err(), Some(JournalError::Unavailable));
        drop(host);
        let (host, _) = FileOversight::open_with_learned_generation(root.store(), profile(), &config).unwrap();
        let packet = host.retained_learned_sidecar(1).unwrap().packet;
        assert_eq!(packet.work().rounds, if barrier == JournalIo::DirectorySync { 2 } else { 1 });
        assert_eq!(packet.selected_groups().len(), usize::from(barrier == JournalIo::DirectorySync));
        assert_eq!(host.inspect().executions, 0);
    }
}
