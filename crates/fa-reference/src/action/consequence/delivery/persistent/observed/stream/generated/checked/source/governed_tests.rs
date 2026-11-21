//! The original source-bound native owner, campaign reducer and two-key effect.
//! Synthetic weights exercise contracts; they do not qualify detector accuracy.
use super::*;
use crate::action::ActionState;
use crate::action::consequence::delivery::EndpointOutcome;
use crate::action::consequence::delivery::persistent::observed::{
    governance::campaigns::FilePolicyCampaignReview, stream::generated::FileTextMessageRequest,
};
use crate::action::consequence::delivery::persistent::requests::FileRequestDisposition;
use crate::action::consequence::delivery::persistent::{Reconciliation, governance::PolicyUpdate};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{ReviewWindow, policy_governance::CampaignDisposition};
use crate::action::consequence::policy_campaign::ReplayLimits;
use crate::round::{Verdict, commitment};

fn campaigns() -> FileCampaignRequirement {
    FileCampaignRequirement {
        limits: ReplayLimits {
            cases: 16,
            input_bytes: 1_048_576,
        },
        max_campaigns: 4,
    }
}

fn governed_profile() -> FileOversightProfile {
    let mut profile = profile();
    profile.delivery.total = 4096;
    profile.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
    profile
}

fn create_governed(
    root: &Directory,
    selected: GeneratedSourceProfile,
) -> (FileOversight, FileHumanReviewer, FilePolicyGovernor) {
    FileOversight::create_generated_text_stream_from_source_governed(
        root.store(),
        governed_profile(),
        fixtures::stopped_config(),
        tokenizer(false),
        selected,
        campaigns(),
    )
    .unwrap()
}

fn campaign(host: &mut FileOversight, id: u64) -> FilePolicyCampaignReview {
    let control = host.inspect().control;
    let update = PolicyUpdate::new(
        id,
        control.sequence,
        control.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(2048)]).unwrap(),
    )
    .unwrap();
    host.request_policy_campaign(host.revision(), &update)
        .unwrap()
}

fn generate(
    host: &mut FileOversight,
    source: &mut FileEvidenceSource,
    id: u64,
) -> std::rc::Rc<EvidenceSnapshot> {
    let captured = host
        .refresh_file_source(host.revision(), source, ElapsedTick(1))
        .unwrap();
    let cmd = command(host, id, request(b"ab", 2));
    host.generate_decoder_text(host.revision(), cmd).unwrap();
    assert_eq!(
        host.decoder_text_progress(id).unwrap().bytes().unwrap(),
        b"A"
    );
    captured
}

/// Dispatch through the ORIGINAL full-input review and independent human key.
/// Omitting the final publication deliberately creates a real Unknown liability.
fn release(
    host: &mut FileOversight,
    reviewer: &FileHumanReviewer,
    captured: &EvidenceSnapshot,
    request_id: u64,
    generation: u64,
    publish: bool,
) -> u64 {
    let snapshot = captured.snapshot().clone();
    let input = FileTextMessageRequest {
        request: request_id,
        generation,
        generation_revision: host
            .decoder_generation_progress(generation)
            .unwrap()
            .generation_revision(),
        target: host.inspect().target,
        policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100),
    };
    let status = host
        .submit_decoder_text_message(host.revision(), input, snapshot.clone())
        .unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
        panic!("the actual generated message must be admitted");
    };
    let action = host.request_action(request_id).unwrap().clone();
    let inputs = captured
        .inputs_for(&action, &governed_profile().committee)
        .unwrap();
    host.record_inputs(host.revision(), attempt, 0, inputs.clone())
        .unwrap();
    let round = request_id + 100;
    host.begin_review(
        host.revision(),
        attempt,
        round,
        [9; 32],
        ReviewWindow {
            commit_by: ElapsedTick(5),
            reveal_by: ElapsedTick(8),
        },
        snapshot.clone(),
    )
    .unwrap();
    let digest = commitment(round, "reviewer", &[9; 32], Verdict::Allow, b"salt").unwrap();
    host.commit_review(host.revision(), round, "reviewer", digest)
        .unwrap();
    host.open_reveals(host.revision(), round).unwrap();
    host.reveal_review(
        host.revision(),
        round,
        "reviewer",
        Verdict::Allow,
        b"salt".to_vec(),
    )
    .unwrap();
    host.finish_review(host.revision(), round, Some(&inputs), snapshot.clone())
        .unwrap()
        .unwrap();
    let automatic = host
        .authorize(host.revision(), attempt, &inputs, snapshot.clone())
        .unwrap();
    assert!(
        host.publish_checked(
            host.revision(),
            attempt,
            Some(&inputs),
            snapshot.clone(),
            ElapsedTick(1)
        )
        .is_err()
    );
    let human_request = host
        .request_human_approval(
            host.revision(),
            request_id + 1000,
            attempt,
            &inputs,
            ElapsedTick(10),
        )
        .unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &human_request).unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        &inputs,
        snapshot.clone(),
    )
    .unwrap();
    if publish {
        assert!(matches!(
            host.publish_checked(
                host.revision(),
                attempt,
                Some(&inputs),
                snapshot,
                ElapsedTick(1)
            )
            .unwrap()
            .outcome,
            EndpointOutcome::Executed { .. }
        ));
        assert!(matches!(
            host.reconcile(host.revision(), attempt).unwrap(),
            Reconciliation::Resolved(EndpointOutcome::Executed { .. })
        ));
    } else {
        assert_eq!(
            host.reconcile(host.revision(), attempt).unwrap(),
            Reconciliation::AwaitingResolution
        );
        assert_eq!(
            host.inspect().control.ledger.stages[&attempt],
            ActionState::Unknown
        );
    }
    attempt
}

#[test]
fn governed_native_first_image_retains_all_source_publication_and_campaign_contracts() {
    for mode in 0..6 {
        let root = Directory::new();
        let selected = selection(mode);
        let (host, _, _) = create_governed(&root, selected);
        assert_eq!(selected.check(&host.events), Ok(()));
        assert_eq!(selected.check_machine(&host.machine), Ok(()));
        let expected = campaigns();
        assert!(
            matches!(host.events.last(), Some(Event::Campaign(CampaignEvent::Enable(limits, maximum)))
            if *limits == expected.limits && *maximum == expected.max_campaigns)
        );
        assert_eq!(
            host.events
                .iter()
                .filter(|event| matches!(event, Event::Campaign(CampaignEvent::Enable(..))))
                .count(),
            1
        );
        assert!(host.policy_campaigns_required());
        assert!(host.file_source_required());
        assert!(host.generated_text_stream_required().unwrap());
        assert_eq!(
            host.held_out_joint_policy().unwrap(),
            selected.publication.joint()
        );
        assert_eq!(
            host.journal_capacity().unwrap().reserve(),
            selected.publication.reserve()
        );
        assert_eq!(
            host.publication_validation_profile().unwrap(),
            selected.publication.witnesses().map(|p| p.limits)
        );
        assert_eq!(host.file_source_status().unwrap().capture.closed, None);
        assert!(!host.clock_ready());
        assert_eq!(host.decoder_inspection().unwrap().numerical.position, 0);
        assert_eq!(
            host.decoder_inspection().unwrap().numerical.sampled_draws,
            0
        );
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn invalid_governed_native_configuration_creates_no_partial_store() {
    for which in 0..7 {
        let root = Directory::new();
        let mut required = campaigns();
        let mut selected = selection(5);
        let mut profile = governed_profile();
        match which {
            0 => required.limits.cases = 0,
            1 => required.limits.input_bytes = 0,
            2 => required.max_campaigns = 0,
            3 => required.max_campaigns = usize::MAX,
            4 => selected.source.source.generation = 0,
            5 => selected.source.source.scope.principal += 1,
            _ => {
                profile.delivery.limits.events = selected
                    .events(fixtures::stopped_config(), &tokenizer(false))
                    .unwrap()
                    .len()
            }
        }
        assert!(
            FileOversight::create_generated_text_stream_from_source_governed(
                root.store(),
                profile,
                fixtures::stopped_config(),
                tokenizer(false),
                selected,
                required,
            )
            .is_err(),
            "case {which}"
        );
        assert!(!root.store().exists(), "case {which}");
        let (host, _, _) = create_governed(&root, selection(5));
        assert!(host.policy_campaigns_required());
        assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn governed_native_bootstrap_storage_failures_return_no_roles_or_partial_contracts() {
    for mode in [1, 2, 5] {
        for stage in [
            JournalIo::Stage,
            JournalIo::Write,
            JournalIo::FileSync,
            JournalIo::Rename,
            JournalIo::DirectorySync,
        ] {
            let root = Directory::new();
            let selected = selection(mode);
            let prepared = Prepared::with_campaigns(
                governed_profile(),
                fixtures::stopped_config(),
                &tokenizer(false),
                selected,
                Some(campaigns()),
            )
            .unwrap();
            let store = storage::Store::create(&root.store()).unwrap();
            let expected =
                journal::encode(&prepared.profile, store.identity(), &prepared.events).unwrap();
            store.fail_once(stage);
            assert!(
                matches!(prepared.publish_governed(store), Err(JournalError::Io(failure)) if failure.operation == stage)
            );
            if stage == JournalIo::DirectorySync {
                assert_eq!(canonical(&root), expected);
                let store = storage::Store::open(&root.store()).unwrap();
                let events =
                    journal::decode(&governed_profile(), store.identity(), &expected).unwrap();
                let machine = Machine::replay(&governed_profile(), &events).unwrap();
                assert_eq!(selected.check(&events), Ok(()));
                assert_eq!(selected.check_machine(&machine), Ok(()));
                assert!(machine.broker.policy_campaigns_required());
            } else {
                assert!(!root.store().join(storage::CANONICAL).exists());
            }
        }
    }
}

#[test]
fn native_governor_promotes_real_retained_corpus_and_continues_original_publication() {
    let root = Directory::new();
    let (mut host, reviewer, governor) = create_governed(&root, selection(1));
    let mut source = evidence(&root);
    let captured = generate(&mut host, &mut source, 7);
    release(&mut host, &reviewer, &captured, 91, 7, true);
    let numerical = host.decoder_inspection().unwrap().numerical;
    let ledger = host.inspect().control.ledger;
    let stream = host.stream_snapshot().unwrap();
    let control = host.inspect().control;
    let direct = PolicyUpdate::new(
        701,
        control.sequence,
        control.ledger.epoch,
        Policy::new(2, vec![Predicate::PayloadAtMost(2048)]).unwrap(),
    )
    .unwrap();
    let before = bytes(&host);
    assert_eq!(
        host.replace_policy(host.revision(), &direct),
        Err(Error::Incomplete.into())
    );
    assert_eq!(bytes(&host), before);
    assert_eq!(host.current_policy().unwrap().generation(), 1);
    // The identical candidate and original retained corpus succeed only after
    // the independently returned governor approves the complete campaign.
    let review = campaign(&mut host, 701);
    assert_eq!(
        review.report().cases().len(),
        2,
        "original proposal and completed congress"
    );
    assert!(!review.report().requires_shadow());
    let revision = host.revision();
    let permit = governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    let receipt = host
        .promote_policy_campaign(host.revision(), &permit)
        .unwrap();
    assert_eq!(host.current_policy().unwrap().generation(), 2);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.stream_snapshot().unwrap().confirmed, stream.confirmed);
    assert_eq!(host.inspect().control.ledger.charged, ledger.charged);
    assert_eq!(host.inspect().control.ledger.available, ledger.available);
    let before = bytes(&host);
    assert_eq!(host.promote_policy_campaign(0, &permit).unwrap(), receipt);
    assert_eq!(bytes(&host), before);
    let captured = host
        .refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    let cmd = host
        .prepare_decoder_text_continuation(91, 8, request(b"ab", 2))
        .unwrap();
    host.begin_decoder_text_continuation(host.revision(), 91, cmd)
        .unwrap();
    while host.decoder_text_progress(8).unwrap().finish().is_none() {
        let step = host.decoder_text_progress(8).unwrap().generation_revision();
        host.advance_decoder_text(host.revision(), 8, step).unwrap();
    }
    assert_eq!(
        host.decoder_text_progress(8).unwrap().bytes().unwrap(),
        b"A"
    );
    release(&mut host, &reviewer, &captured, 92, 8, true);
    assert_eq!(
        host.stream_snapshot()
            .unwrap()
            .confirmed
            .messages()
            .collect::<Vec<_>>(),
        ["A", "A"]
    );
    assert_eq!(host.inspect().executions, 2);
    assert!(host.inspect().control.ledger.charged > ledger.charged);
    assert!(host.decoder_inspection().unwrap().numerical.position > numerical.position);
}

#[test]
fn native_governance_preserves_unknown_liability_and_spent_numerical_work() {
    let root = Directory::new();
    let (mut host, reviewer, governor) = create_governed(&root, selection(1));
    let mut source = evidence(&root);
    let captured = generate(&mut host, &mut source, 7);
    let attempt = release(&mut host, &reviewer, &captured, 91, 7, false);
    let before = host.inspect();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let review = campaign(&mut host, 701);
    let revision = host.revision();
    let permit = governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    let receipt = host
        .promote_policy_campaign(host.revision(), &permit)
        .unwrap();
    assert!(receipt.change().cancelled.is_empty());
    assert_eq!(
        host.inspect().control.ledger.charged,
        before.control.ledger.charged
    );
    assert_eq!(
        host.inspect().control.ledger.available,
        before.control.ledger.available
    );
    assert!(before.control.ledger.charged > 0);
    assert_eq!(
        host.inspect().control.ledger.stages[&attempt],
        ActionState::Unknown
    );
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0);
    assert!(
        host.prepare_decoder_text_continuation(91, 8, request(b"ab", 2))
            .is_err()
    );
}

#[test]
fn native_governor_and_approval_remain_bound_to_the_original_acknowledged_owner() {
    let root = Directory::new();
    let other = Directory::new();
    let (mut host, reviewer, governor) = create_governed(&root, selection(1));
    let (_, _, foreign) = create_governed(&other, selection(1));
    let mut source = evidence(&root);
    let captured = generate(&mut host, &mut source, 7);
    release(&mut host, &reviewer, &captured, 91, 7, true);
    let review = campaign(&mut host, 701);
    let before = bytes(&host);
    let revision = host.revision();
    assert_eq!(
        foreign.approve(&mut host, revision, &review, false).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(bytes(&host), before);
    let permit = governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let charged = host.inspect().control.ledger.charged;
    drop(host);
    // Existing source recovery preserves the guard but deliberately issues no
    // governor. Its Fence revokes the old campaign and both old opaque roles.
    let (mut host, _) = FileOversight::open_generated_text_stream_from_source(
        root.store(),
        governed_profile(),
        &fixtures::stopped_config(),
        &tokenizer(false),
        selection(1),
    )
    .unwrap();
    assert!(host.policy_campaigns_required());
    assert_eq!(
        host.policy_campaign(701).unwrap().observed_disposition(),
        CampaignDisposition::Revoked
    );
    let before = bytes(&host);
    let revision = host.revision();
    assert_eq!(
        governor.approve(&mut host, revision, &review, false).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(
        host.promote_policy_campaign(revision, &permit),
        Err(Error::Binding.into())
    );
    assert_eq!(bytes(&host), before);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().control.ledger.charged, charged);
    assert_eq!(host.current_policy().unwrap().generation(), 1);
}
