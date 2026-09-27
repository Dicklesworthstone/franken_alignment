//! Whole-message release and close using ORIGINAL stream receipts and permits.
//! Synthetic weights/votes are protocol controls, not trained-model evidence.
use super::*;
use fa_reference::action::consequence::delivery::{DispatchEnvelope, stream::StreamProfile};

fn stream_owner(human: bool, message_cap: usize, stream_cap: usize)
    -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>)
{
    let contract = CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(),
        HelperContract::new(InputProfileBinding { profile_id: 1, profile_bytes: Vec::new(),
            tokenizer_epoch: 4, policy_epoch: 0, model_epoch: 3 }, 1, b"approve?".to_vec()).unwrap(),
    )])).unwrap();
    let mut endpoint = PublicationEndpoint::new_stream(target(),
        StreamProfile::new(1, 1, 4, message_cap, stream_cap).unwrap(), 1000, 8).unwrap();
    let mut owner = OversightBroker::new(ControllerConfig {
        scope: scope(), total: 4096, max_attempts: 8, actor: actor(&[]), suspend_at_incident: 3,
        policy: Policy::new(1, vec![Predicate::PayloadAtMost(1024)]).unwrap(),
        congress: CongressPolicy { generation: 1,
            members: BTreeMap::from([("reviewer".to_owned(), MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
            caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1,
            continue_hold_maximum: 0, narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1,
        }, narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
    }, &mut endpoint, contract).unwrap();
    let human = if human { Some(owner.enable_human_review(HumanReviewPolicy {
        reviewer_id: 55, max_validity_ticks: 100, max_requests: 8,
    }).unwrap()) } else { None };
    owner.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
    owner.confirm_fence(endpoint.install_fence(owner.fence_request()).unwrap()).unwrap();
    (owner, endpoint, human)
}
fn configured_stream(human: bool, required_sidecar: bool)
    -> (OversightBroker, PublicationEndpoint, Option<HumanReviewer>)
{
    let (mut owner, endpoint, human) = stream_owner(human, 128, 512);
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    owner.own_learned_text_stream(model.clone(), tokenizer(&model), config(&model),
        LearnedDecoderBindingLimits::default()).unwrap();
    if required_sidecar { owner.enable_learned_sidecar_requirement().unwrap(); }
    (owner, endpoint, human)
}
fn message(owner: &mut OversightBroker, id: u64) -> FrozenAction {
    owner.propose_learned_text_stream_message(id, ElapsedTick(100), Vec::new(), &snapshot()).unwrap().action
}
fn finish(owner: &mut OversightBroker, id: u64) -> FrozenAction {
    owner.propose_learned_text_stream_finish(id, ElapsedTick(100), Vec::new(), &snapshot()).unwrap().action
}
fn dispatch(owner: &mut OversightBroker, id: u64, action: &FrozenAction) -> DispatchEnvelope {
    let input = record_input(owner, id, action);
    approve(owner, id, &input);
    let permit = owner.authorize(id, Some(&input), &snapshot()).unwrap();
    owner.dispatch(&permit, action, Some(&input), &snapshot()).unwrap()
}
fn publish(owner: &mut OversightBroker, endpoint: &mut PublicationEndpoint, id: u64, action: &FrozenAction) {
    let envelope = dispatch(owner, id, action);
    owner.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
}
fn confirmed_message(owner: &mut OversightBroker, endpoint: &mut PublicationEndpoint) -> u64 {
    complete(owner);
    let action = message(owner, 1);
    let units = action.spec().units;
    publish(owner, endpoint, 1, &action);
    units
}

#[test]
fn complete_message_then_separate_sidecar_workers_and_two_keys_close_without_new_inference() {
    let (mut owner, mut endpoint, reviewer) = configured_stream(true, true);
    let reviewer = reviewer.unwrap();
    assert!(owner.learned_text_stream_required());
    assert_eq!(owner.propose_learned_text_stream_message(1, ElapsedTick(100), Vec::new(), &snapshot()).err(), Some(Error::Incomplete));
    complete(&mut owner);
    let numerical = owner.hosted_learned_generation().unwrap();
    let observed = owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).unwrap();
    assert_eq!(observed.bytes(), b"OK");
    assert_eq!(owner.propose_learned_text_stream_finish(1, ElapsedTick(100), Vec::new(), &snapshot()).err(), Some(Error::WrongState));
    let mut total = 0;
    for id in 1..=2 {
        let expected = if id == 1 { owner.stream_message_spec("OK", ElapsedTick(100)).unwrap() }
            else { owner.stream_finish_spec(ElapsedTick(100)).unwrap() };
        let action = if id == 1 { message(&mut owner, id) } else { finish(&mut owner, id) };
        assert_eq!(action.spec(), &expected);
        assert_eq!(action.spec().units, action.spec().payload.len() as u64);
        assert!(action.spec().units > observed.bytes().len() as u64);
        let choice = LearnedSidecarRequest { identity: SidecarIdentity { object_id: 1000 + id, generation: 1, transform_id: 7 },
            priority: Vec::new(), budget: SidecarCongressBudget::default() };
        let sidecar = owner.begin_learned_sidecar(id, owner.actor_revision(), choice).unwrap();
        let input = owner.current_learned_sidecar(&sidecar).unwrap().clone();
        let session = owner.begin_learned_sidecar_review(&sidecar, 100 + id, [7; 32],
            ReviewWindow { commit_by: ElapsedTick(20), reveal_by: ElapsedTick(30) }, &snapshot()).unwrap();
        let (mut round, ports) = HelperRound::new(session, HelperLimits::default()).unwrap();
        let worker = &ports["reviewer"]; let salt = b"stream-reviewer";
        worker.submit_commitment(worker.request().commitment(Verdict::Allow, salt).unwrap()).unwrap();
        round.advance(ElapsedTick(1)).unwrap();
        worker.reveal(Verdict::Allow, salt).unwrap();
        let review = round.finish(ElapsedTick(1)).unwrap();
        owner.apply_review(review, Some(&input), &snapshot()).unwrap();
        let permit = owner.authorize(id, Some(&input), &snapshot()).unwrap();
        assert_eq!(owner.dispatch(&permit, &action, Some(&input), &snapshot()).err(), Some(Error::Incomplete));
        let request = owner.request_human_approval(200 + id, id, Some(&input), ElapsedTick(80)).unwrap();
        let human = reviewer.approve(&request, ElapsedTick(1)).unwrap();
        let envelope = owner.dispatch_with_human(&permit, &human, &action, Some(&input), &snapshot()).unwrap();
        owner.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
        total += action.spec().units;
        assert_eq!(owner.inspect().ledger.charged, total);
        assert_eq!(endpoint.payload(), b"OK");
        assert_eq!(endpoint.stream_view().unwrap().messages().collect::<Vec<_>>(), vec!["OK"]);
        assert_eq!(endpoint.stream_view().unwrap().finished(), id == 2);
        assert_eq!(owner.hosted_learned_generation().unwrap(), numerical);
        assert!(owner.dispatch_with_human(&permit, &human, &action, Some(&input), &snapshot()).is_err());
    }
    assert_eq!(endpoint.execution_count(), 2);
    assert_eq!(owner.propose_learned_text_stream_message(3, ElapsedTick(100), Vec::new(), &snapshot()).err(), Some(Error::WrongState));
    assert_eq!(owner.propose_learned_text_stream_finish(3, ElapsedTick(100), Vec::new(), &snapshot()).err(), Some(Error::WrongState));
}

#[test]
fn generic_proposals_cannot_substitute_content_boundaries_target_epoch_or_frame_charge() {
    let (mut owner, mut endpoint, _) = configured_stream(false, false); complete(&mut owner);
    let valid = owner.stream_message_spec("OK", ElapsedTick(100)).unwrap();
    for case in 0..8 {
        let mut wrong = valid.clone();
        match case {
            0 => wrong.payload = b"OK".to_vec(),
            1 => wrong.payload = owner.stream_message_spec("replacement", ElapsedTick(100)).unwrap().payload,
            2 => wrong.units = 2,
            3 => wrong.target.as_mut().unwrap().expected_version += 1,
            4 => wrong.target.as_mut().unwrap().generation += 1,
            5 => wrong.policy_epoch += 1,
            6 => wrong.payload = owner.stream_finish_spec(ElapsedTick(100)).unwrap().payload,
            7 => wrong.payload = owner.stream_message_spec("O", ElapsedTick(100)).unwrap().payload,
            _ => unreachable!(),
        }
        let state = owner.inspect(); let retained = owner.decoder_binding_usage();
        assert_eq!(owner.propose(1, wrong, &snapshot()).err(), Some(Error::Binding), "case {case}");
        assert_eq!(owner.inspect(), state); assert_eq!(owner.decoder_binding_usage(), retained);
        assert_eq!(owner.input_revision(1), Err(Error::Missing));
    }
    // The generic entry point is not disabled: the identical valid proposal works.
    let action = owner.propose(1, valid.clone(), &snapshot()).unwrap().action;
    assert_eq!(action.spec(), &valid);
    assert_eq!(endpoint.execution_count(), 0);
    publish(&mut owner, &mut endpoint, 1, &action);
    // Same cumulative audience bytes, different message boundaries. This is a
    // valid wire frame in isolation, but not the original confirmed history.
    use fa_reference::action::consequence::delivery::stream::{ReleaseFrame, STREAM_HEADER_BYTES};
    let closing = owner.stream_finish_spec(ElapsedTick(100)).unwrap();
    let mut split = closing.clone();
    split.payload[36..40].copy_from_slice(&2_u32.to_be_bytes());
    split.payload.truncate(STREAM_HEADER_BYTES);
    split.payload.extend_from_slice(&1_u32.to_be_bytes()); split.payload.push(b'O');
    split.payload.extend_from_slice(&1_u32.to_be_bytes()); split.payload.push(b'K');
    split.units = split.payload.len() as u64;
    let frame = ReleaseFrame::decode(&split.payload).unwrap();
    assert_eq!(frame.prior_messages(), &["O", "K"]); assert!(frame.is_finish());
    assert_eq!(frame.prior_messages().concat(), "OK");
    assert_eq!(owner.propose(2, split, &snapshot()).err(), Some(Error::Binding));
    assert_eq!(owner.propose(2, closing.clone(), &snapshot()).unwrap().action.spec(), &closing);
}

#[test]
fn lost_acknowledgment_blocks_both_append_and_close_until_original_receipt_resolves() {
    for executed in [false, true] {
        let (mut owner, mut endpoint, _) = configured_stream(false, false); complete(&mut owner);
        let action = message(&mut owner, 1); let envelope = dispatch(&mut owner, 1, &action);
        if executed { let _lost = endpoint.deliver(&envelope).unwrap(); }
        owner.acknowledgment_lost(1).unwrap();
        assert_eq!(owner.stream_pending(), Some(1));
        assert_eq!(owner.stream_state().unwrap().1.message_count(), 0);
        assert_eq!(owner.inspect().ledger.charged, action.spec().units);
        assert!(owner.cancel(1).is_err());
        for close in [false, true] {
            let proposal = if close { owner.propose_learned_text_stream_finish(2, ElapsedTick(100), Vec::new(), &snapshot()) }
                else { owner.propose_learned_text_stream_message(2, ElapsedTick(100), Vec::new(), &snapshot()) };
            assert_eq!(proposal.err(), Some(Error::Incomplete));
        }
        let receipt = endpoint.seal_unexecuted(&owner.status_query(1).unwrap()).unwrap();
        assert!(owner.accept_receipt(receipt.clone()).unwrap());
        assert!(!owner.accept_receipt(receipt).unwrap());
        assert_eq!(owner.stream_pending(), None);
        assert_eq!(owner.inspect().ledger.charged, if executed { action.spec().units } else { 0 });
        assert_eq!(owner.stream_state().unwrap().1.message_count(), usize::from(executed));
        if executed {
            assert_eq!(owner.propose_learned_text_stream_message(2, ElapsedTick(100), Vec::new(), &snapshot()).err(), Some(Error::WrongState));
            let close = finish(&mut owner, 2); publish(&mut owner, &mut endpoint, 2, &close);
        } else {
            assert_eq!(owner.propose_learned_text_stream_finish(2, ElapsedTick(100), Vec::new(), &snapshot()).err(), Some(Error::WrongState));
            let retried = message(&mut owner, 2); publish(&mut owner, &mut endpoint, 2, &retried);
        }
        assert_eq!(endpoint.payload(), b"OK");
        assert_eq!(endpoint.stream_view().unwrap().message_count(), 1);
    }
}

#[test]
fn duplicate_prepared_message_loses_eligibility_after_receipt_advances_the_audience() {
    let (mut owner, mut endpoint, _) = configured_stream(false, false); complete(&mut owner);
    let first = message(&mut owner, 1); let duplicate = message(&mut owner, 2);
    let input = record_input(&mut owner, 2, &duplicate); approve(&mut owner, 2, &input);
    let reserved = owner.authorize(2, Some(&input), &snapshot()).unwrap();
    publish(&mut owner, &mut endpoint, 1, &first);
    let state = owner.inspect();
    assert!(owner.dispatch(&reserved, &duplicate, Some(&input), &snapshot()).is_err());
    assert_eq!(owner.inspect(), state); assert_eq!(endpoint.execution_count(), 1);
    let duplicate_frame = owner.stream_message_spec("OK", ElapsedTick(100)).unwrap();
    assert_eq!(owner.propose(3, duplicate_frame, &snapshot()).err(), Some(Error::Binding));
    owner.cancel(2).unwrap();
    let close = finish(&mut owner, 3); publish(&mut owner, &mut endpoint, 3, &close);
    assert_eq!(endpoint.payload(), b"OK"); assert_eq!(endpoint.stream_view().unwrap().message_count(), 1);
}

#[test]
fn cancelling_an_unexecuted_append_or_finish_neither_closes_nor_consumes_generated_content() {
    let (mut owner, mut endpoint, _) = configured_stream(false, false); complete(&mut owner);
    let first = message(&mut owner, 1); let input = record_input(&mut owner, 1, &first);
    approve(&mut owner, 1, &input); let _permit = owner.authorize(1, Some(&input), &snapshot()).unwrap();
    owner.cancel(1).unwrap();
    assert_eq!(owner.inspect().ledger.reserved, 0); assert_eq!(owner.inspect().ledger.charged, 0);
    assert_eq!(owner.propose_learned_text_stream_finish(2, ElapsedTick(100), Vec::new(), &snapshot()).err(), Some(Error::WrongState));
    let retried = message(&mut owner, 2); publish(&mut owner, &mut endpoint, 2, &retried);
    let close = finish(&mut owner, 3); let input = record_input(&mut owner, 3, &close);
    approve(&mut owner, 3, &input); let _permit = owner.authorize(3, Some(&input), &snapshot()).unwrap();
    owner.cancel(3).unwrap();
    assert!(!endpoint.stream_view().unwrap().finished());
    let retried_close = finish(&mut owner, 4); publish(&mut owner, &mut endpoint, 4, &retried_close);
    assert!(endpoint.stream_view().unwrap().finished()); assert_eq!(endpoint.execution_count(), 2);
    assert_eq!(owner.inspect().ledger.charged, retried.spec().units + retried_close.spec().units);
}

#[test]
fn source_suspension_blocks_new_finish_but_does_not_rewrite_already_visible_text() {
    let (mut owner, mut endpoint, _) = configured_stream(false, false);
    let charged = confirmed_message(&mut owner, &mut endpoint);
    let close = finish(&mut owner, 2); let input = record_input(&mut owner, 2, &close);
    approve(&mut owner, 2, &input); let permit = owner.authorize(2, Some(&input), &snapshot()).unwrap();
    stop(&mut owner);
    assert!(owner.dispatch(&permit, &close, Some(&input), &snapshot()).is_err());
    assert_eq!(owner.inspect().ledger.reserved, 0);
    assert_eq!(owner.inspect().ledger.charged, charged);
    assert_eq!(endpoint.payload(), b"OK"); assert!(!endpoint.stream_view().unwrap().finished());
    let settled = owner.progress_stop(&mut endpoint).unwrap();
    assert!(settled.progress.drained()); assert_eq!(settled.progress.charged_units, charged);
    assert!(!endpoint.stream_view().unwrap().finished());
}

#[test]
fn pending_finish_reconciles_after_stop_without_reopening_a_text_source_or_retrying_effect() {
    for executed in [false, true] {
        let (mut owner, mut endpoint, _) = configured_stream(false, false);
        let message_charge = confirmed_message(&mut owner, &mut endpoint);
        let close = finish(&mut owner, 2); let envelope = dispatch(&mut owner, 2, &close);
        if executed { let _lost = endpoint.deliver(&envelope).unwrap(); }
        owner.acknowledgment_lost(2).unwrap(); stop(&mut owner);
        assert_eq!(owner.inspect().ledger.charged, message_charge + close.spec().units);
        assert!(owner.hosted_learned_text_message(LearnedEvidenceLimits::default()).is_err());
        let settled = owner.progress_stop(&mut endpoint).unwrap();
        assert!(settled.progress.drained());
        assert_eq!(settled.progress.charged_units, message_charge + if executed { close.spec().units } else { 0 });
        assert_eq!(endpoint.stream_view().unwrap().finished(), executed);
        assert_eq!(endpoint.payload(), b"OK"); assert_eq!(endpoint.stream_view().unwrap().message_count(), 1);
    }
}

#[test]
fn bootstrap_capacity_and_endpoint_mode_refusals_are_atomic_with_a_fitting_control() {
    let model = model(&[b'O' as u32, b'K' as u32, END]);
    let (mut raw, _, _) = empty_owner(false);
    assert_eq!(raw.own_learned_text_stream(model.clone(), tokenizer(&model), config(&model),
        LearnedDecoderBindingLimits::default()), Err(Error::Binding));
    assert!(!raw.decoder_monitoring_required());
    let (mut owner, endpoint, _) = stream_owner(false, 2, 2);
    let before = owner.inspect(); let revision = owner.actor_revision();
    let mut too_large = config(&model); too_large.output.max_bytes = 3;
    assert_eq!(owner.own_learned_text_stream(model.clone(), tokenizer(&model), too_large,
        LearnedDecoderBindingLimits::default()), Err(Error::Limit));
    assert_eq!(owner.inspect(), before); assert_eq!(owner.actor_revision(), revision);
    assert!(!owner.decoder_monitoring_required());
    let mut exact = config(&model); exact.output.max_bytes = 2;
    owner.own_learned_text_stream(model.clone(), tokenizer(&model), exact,
        LearnedDecoderBindingLimits::default()).unwrap(); complete(&mut owner);
    assert!(message(&mut owner, 1).spec().payload.ends_with(b"OK"));
    assert!(owner.own_learned_text_stream(model.clone(), tokenizer(&model), config(&model),
        LearnedDecoderBindingLimits::default()).is_err());
    assert_eq!(endpoint.execution_count(), 0);
}

#[test]
fn multibyte_output_is_one_complete_utf8_message_and_not_individual_token_releases() {
    let (mut owner, mut endpoint, _) = stream_owner(false, 2, 2);
    let model = model(&[0xc3, 0xa9, END]); let mut config = config(&model); config.output.max_bytes = 2;
    owner.own_learned_text_stream(model.clone(), tokenizer(&model), config,
        LearnedDecoderBindingLimits::default()).unwrap();
    for _ in 0..4 {
        assert!(owner.propose_learned_text_stream_message(1, ElapsedTick(100), Vec::new(), &snapshot()).is_err());
        let current = owner.hosted_learned_generation().unwrap();
        owner.advance_hosted_learned(current.actor_revision, current.position).unwrap();
        assert_eq!(endpoint.execution_count(), 0); assert!(endpoint.payload().is_empty());
    }
    let action = message(&mut owner, 1); publish(&mut owner, &mut endpoint, 1, &action);
    assert_eq!(endpoint.stream_view().unwrap().messages().collect::<Vec<_>>(), vec!["é"]);
    let close = finish(&mut owner, 2); publish(&mut owner, &mut endpoint, 2, &close);
    assert!(endpoint.stream_view().unwrap().finished()); assert_eq!(endpoint.payload(), "é".as_bytes());
}

#[test]
fn finish_requires_new_witness_validation_and_an_unexpired_action() {
    let (mut owner, mut endpoint, _) = configured_stream(false, false);
    confirmed_message(&mut owner, &mut endpoint);
    let witness = fa_reference::ReadWitness::EmptyRange { start: 10, end: 20 };
    let close = owner.propose_learned_text_stream_finish(2, ElapsedTick(100), vec![witness.clone()], &snapshot()).unwrap().action;
    assert_eq!(close.spec().required_witnesses, vec![witness]);
    let input = record_input(&mut owner, 2, &close); approve(&mut owner, 2, &input);
    let permit = owner.authorize(2, Some(&input), &snapshot()).unwrap();
    let mut changed = snapshot(); changed.values.insert(15, b"new".to_vec());
    assert!(owner.dispatch(&permit, &close, Some(&input), &changed).is_err());
    assert!(!endpoint.stream_view().unwrap().finished());
    owner.observe_time(ElapsedTick(100)).unwrap();
    assert_eq!(owner.dispatch(&permit, &close, Some(&input), &snapshot()).err(), Some(Error::Stale));
    assert_eq!(endpoint.execution_count(), 1);
}
