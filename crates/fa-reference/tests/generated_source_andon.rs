//! Native sampled text, independent labels and two-key publication share the
//! original journal. Synthetic weights and ballots exercise custody and law,
//! not detector accuracy or institutional evaluator independence.
#![forbid(unsafe_code)]

#[path = "tower_generated_operator/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/file_oversight_actor.rs"]
mod support;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
    tokenizer::TokenizationBudget,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        Reconciliation, RecoveryReserve,
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            credibility::{FileIndependentEvaluator, andon::FileAndonConfig},
            decoder::text::FileTextGenerationCommand,
            source::FileSourcePolicy,
            stream::generated::{
                FileTextMessageRequest,
                checked::source::{
                    GeneratedSourceProfile, GeneratedSourcePublication,
                    evaluation::FileGeneratedEvaluationConfig,
                },
            },
        },
        requests::{FileRequestDisposition, actor::FileGeneratedTextActorPort},
    },
    stream::StreamProfile,
};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::{
    CommitteeInput,
    credibility::{Assessment, EvaluationProtocol, Fraction, GroundTruth},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use fa_reference::round::{
    Verdict,
    review::andon::{AndonBudget, LedgerIdentity},
};
use std::{fs, rc::Rc};
use support::{Directory, Supervisor};

const REQUEST: u64 = 701;
const HELD_ROUND: u64 = 801;

fn profile() -> FileOversightProfile {
    let mut profile = fixture::profile();
    profile.delivery.policy = Policy::new(1, vec![Predicate::PayloadAtMost(4096)]).unwrap();
    profile
}

fn selection() -> GeneratedSourceProfile {
    GeneratedSourceProfile {
        source: FileSourcePolicy {
            source: StateSource {
                scope: profile().delivery.scope,
                source: 7,
                generation: 1,
            },
            limits: StateLimits::default(),
            freshness: StateFreshness::new(100).unwrap(),
        },
        publication: GeneratedSourcePublication::Stream {
            stream: StreamProfile::new(9, 1, 4, 64, 256).unwrap(),
            reserve: Some(RecoveryReserve::terminal()),
        },
    }
}

fn protocol() -> EvaluationProtocol {
    EvaluationProtocol {
        domain: 1,
        stratum: 2,
        period: 3,
        minimum_violation_origins: 1,
        minimum_benign_origins: 1,
        precision_floor: Fraction {
            numerator: 1,
            denominator: 2,
        },
        recall_floor: Fraction {
            numerator: 1,
            denominator: 2,
        },
        false_positive_ceiling: Fraction {
            numerator: 1,
            denominator: 2,
        },
        false_stop_budget: 10,
    }
}

fn config() -> FileAndonConfig {
    FileAndonConfig::new(
        LedgerIdentity {
            deployment: 101,
            evaluator: 202,
        },
        vec![AndonBudget {
            window: 1,
            start: 0,
            end: 100,
            false_stops_allowed: 2,
            cost_per_stop: 7,
        }],
    )
    .unwrap()
}

fn create(root: &Directory) -> (FileOversight, FileHumanReviewer, FileIndependentEvaluator) {
    let evaluation = FileGeneratedEvaluationConfig::new(protocol())
        .with_andon(config())
        .unwrap();
    let (host, roles) = FileOversight::create_generated_text_stream_from_source_evaluated(
        root.store(),
        profile(),
        fixture::decoder(),
        fixture::tokenizer(),
        selection(),
        evaluation,
    )
    .unwrap();
    (host, roles.oversight.human, roles.evaluator)
}

fn bytes(root: &Directory) -> Vec<u8> {
    fs::read(root.store().join("delivery.bin")).unwrap()
}

fn evidence(root: &Directory) -> FileEvidenceSource {
    let scope = profile().delivery.scope;
    let observed = EvidenceSnapshot::new(
        EvidenceIdentity {
            scope,
            source: 7,
            generation: 1,
        },
        support::snapshot(),
        support::oversight::MEMBERS
            .into_iter()
            .map(|member| {
                (
                    member.to_owned(),
                    b"actual independent file context".to_vec(),
                )
            })
            .collect(),
    )
    .unwrap();
    let path = root.0.join("evidence.json");
    fs::write(&path, observed.encode()).unwrap();
    FileEvidenceSource::new(path, 7, scope, 1_048_576).unwrap()
}

fn request() -> TextGenerationRequest {
    TextGenerationRequest {
        prompt: b"ab".to_vec(),
        prefix_controls: vec![256],
        max_new_tokens: 2,
        stop_tokens: vec![256],
        tokenization: TokenizationBudget::default(),
        generation: GenerationBudget {
            scalar_products: MAX_DECODER_PRODUCTS,
            sampling_entries: MAX_SAMPLING_ENTRIES,
        },
        max_output_bytes: 4,
    }
}

fn generate(host: &mut FileOversight, source: &mut FileEvidenceSource) -> Rc<EvidenceSnapshot> {
    let captured = host
        .refresh_file_source(host.revision(), source, ElapsedTick(1))
        .unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command =
        FileTextGenerationCommand::new(7, numerical.actor_revision, numerical.position, request())
            .unwrap();
    host.generate_decoder_text(host.revision(), command)
        .unwrap();
    assert_eq!(
        host.decoder_text_progress(7).unwrap().bytes().unwrap(),
        b"A"
    );
    captured
}

fn submit(
    host: &mut FileOversight,
    captured: &EvidenceSnapshot,
    id: u64,
    generation: u64,
) -> (u64, CommitteeInput) {
    let input = FileTextMessageRequest {
        request: id,
        generation,
        generation_revision: host
            .decoder_text_progress(generation)
            .unwrap()
            .generation_revision(),
        target: host.inspect().target,
        policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100),
    };
    let status = host
        .submit_decoder_text_message(host.revision(), input, captured.snapshot().clone())
        .unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
        panic!("the actual generated message must be admitted");
    };
    let action = host.request_action(id).unwrap();
    let inputs = captured.inputs_for(action, &profile().committee).unwrap();
    (attempt, inputs)
}

fn review(host: &mut FileOversight, attempt: u64, round: u64, inputs: &CommitteeInput, hold: bool) {
    host.record_inputs(
        host.revision(),
        attempt,
        host.input_revision(attempt).unwrap(),
        inputs.clone(),
    )
    .unwrap();
    host.begin_review(
        host.revision(),
        attempt,
        round,
        support::oversight::ROOT,
        support::oversight::window(host),
        support::snapshot(),
    )
    .unwrap();
    let votes = [
        ("alpha", if hold { Verdict::Hold } else { Verdict::Allow }),
        ("beta", Verdict::Allow),
    ];
    for (member, vote) in votes {
        support::oversight::commit(host, round, member, vote);
    }
    host.open_reveals(host.revision(), round).unwrap();
    for (member, vote) in votes {
        host.reveal_review(
            host.revision(),
            round,
            member,
            vote,
            support::oversight::salt(member),
        )
        .unwrap();
    }
    host.finish_review(host.revision(), round, Some(inputs), support::snapshot())
        .unwrap()
        .unwrap();
}

fn publish(
    host: &mut FileOversight,
    human: &FileHumanReviewer,
    request: u64,
    attempt: u64,
    inputs: &CommitteeInput,
) {
    let action = host.request_action(request).unwrap().clone();
    let automatic = host
        .authorize(host.revision(), attempt, inputs, support::snapshot())
        .unwrap();
    // The automatic key and evaluator labels never substitute for the human.
    assert!(
        host.publish_checked(
            host.revision(),
            attempt,
            Some(inputs),
            support::snapshot(),
            ElapsedTick(1)
        )
        .is_err()
    );
    let approval = host
        .request_human_approval(
            host.revision(),
            request + 1000,
            attempt,
            inputs,
            ElapsedTick(31),
        )
        .unwrap();
    let revision = host.revision();
    let human = human.approve(host, revision, &approval).unwrap();
    host.dispatch(
        host.revision(),
        &automatic,
        &human,
        &action,
        inputs,
        support::snapshot(),
    )
    .unwrap();
    assert!(matches!(
        host.publish_checked(
            host.revision(),
            attempt,
            Some(inputs),
            support::snapshot(),
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
}

#[test]
fn original_native_evaluator_accounts_a_real_hold_then_two_messages_and_finish_publish_lawfully() {
    let root = Directory::new();
    let (mut host, human, evaluator) = create(&root);
    assert_eq!(evaluator.check_owner(&host), Ok(()));
    assert_eq!(host.credibility_protocol().unwrap(), Some(&protocol()));
    assert_eq!(host.andon_config().unwrap(), Some(&config()));
    assert_eq!(
        host.journal_capacity().unwrap().reserve(),
        Some(RecoveryReserve::terminal())
    );
    let mut source = evidence(&root);
    let captured = generate(&mut host, &mut source);
    let (attempt, inputs) = submit(&mut host, &captured, REQUEST, 7);
    review(&mut host, attempt, HELD_ROUND, &inputs, true);
    let numerical = host.decoder_inspection().unwrap().numerical;
    let control = host.inspect().control;
    let ticket = evaluator.evaluation_case(&host, HELD_ROUND).unwrap();
    assert!(ticket.case().stopped);
    assert_eq!(host.credibility_report().unwrap().pending_cases, 1);
    assert!(
        host.authorize(host.revision(), attempt, &inputs, support::snapshot())
            .is_err()
    );
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    let label = Assessment {
        origin: 17,
        evidence_id: [17; 32],
        truth: GroundTruth::Benign,
    };
    let revision = host.revision();
    assert!(
        evaluator
            .assess(&mut host, revision, &ticket, label)
            .unwrap()
    );
    assert_eq!(host.evaluation_history(HELD_ROUND).unwrap(), &[label]);
    assert_eq!(
        host.andon_receipt(HELD_ROUND).unwrap().case(),
        ticket.case()
    );
    assert_eq!(
        host.andon_receipt(HELD_ROUND).unwrap().binding().identity(),
        config().identity()
    );
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].total_cost, 7);
    assert_eq!(host.inspect().control, control);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    let before = bytes(&root);
    assert!(!evaluator.assess(&mut host, 0, &ticket, label).unwrap());
    assert_eq!(bytes(&root), before);
    // Independent truth records the false hold; a NEW complete congress and
    // human key are still required to release those exact generated bytes.
    review(&mut host, attempt, 802, &inputs, false);
    publish(&mut host, &human, REQUEST, attempt, &inputs);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    let captured = host
        .refresh_file_source(host.revision(), &mut source, ElapsedTick(1))
        .unwrap();
    let continuation = host
        .prepare_decoder_text_continuation(REQUEST, 8, request())
        .unwrap();
    host.begin_decoder_text_continuation(host.revision(), REQUEST, continuation)
        .unwrap();
    while host.decoder_text_progress(8).unwrap().finish().is_none() {
        let step = host.decoder_text_progress(8).unwrap().generation_revision();
        host.advance_decoder_text(host.revision(), 8, step).unwrap();
    }
    let (attempt, inputs) = submit(&mut host, &captured, 702, 8);
    review(&mut host, attempt, 803, &inputs, false);
    publish(&mut host, &human, 702, attempt, &inputs);
    assert_eq!(
        host.stream_snapshot()
            .unwrap()
            .confirmed
            .messages()
            .collect::<Vec<_>>(),
        ["A", "A"]
    );
    let numerical = host.decoder_inspection().unwrap().numerical;
    let intent = host
        .prepare_decoder_text_finish(702, 703, ElapsedTick(100))
        .unwrap();
    let spec = host.stream_finish_spec(intent.deadline).unwrap();
    assert_eq!(spec.target, Some(intent.target));
    let status = host
        .submit_request(host.revision(), 703, spec, support::snapshot())
        .unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = status.disposition else {
        panic!("explicit finish must be separately admitted");
    };
    let inputs = captured
        .inputs_for(host.request_action(703).unwrap(), &profile().committee)
        .unwrap();
    review(&mut host, attempt, 804, &inputs, false);
    publish(&mut host, &human, 703, attempt, &inputs);
    let stream = host.stream_snapshot().unwrap();
    assert_eq!(stream.confirmed, stream.published);
    assert!(stream.confirmed.finished());
    assert_eq!(stream.confirmed.messages().collect::<Vec<_>>(), ["A", "A"]);
    assert_eq!(host.inspect().executions, 3);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    for round in 802..=804 {
        let ticket = evaluator.evaluation_case(&host, round).unwrap();
        assert!(!ticket.case().stopped);
        let revision = host.revision();
        evaluator
            .assess(
                &mut host,
                revision,
                &ticket,
                Assessment {
                    origin: round,
                    evidence_id: [18; 32],
                    truth: GroundTruth::Benign,
                },
            )
            .unwrap();
    }
    assert_eq!(host.credibility_report().unwrap().pending_cases, 0);
    assert_eq!(host.andon_source().unwrap().receipts().len(), 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].total_cost, 7);
}

#[test]
fn foreign_and_recovered_evaluators_cannot_discover_or_label_the_original_native_case() {
    let root = Directory::new();
    let other = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    let (_, _, foreign) = create(&other);
    let mut source = evidence(&root);
    let captured = generate(&mut host, &mut source);
    let (attempt, inputs) = submit(&mut host, &captured, REQUEST, 7);
    review(&mut host, attempt, HELD_ROUND, &inputs, true);
    let ticket = evaluator.evaluation_case(&host, HELD_ROUND).unwrap();
    let label = Assessment {
        origin: 17,
        evidence_id: [17; 32],
        truth: GroundTruth::Benign,
    };
    let before = bytes(&root);
    assert_eq!(foreign.check_owner(&host), Err(Error::Binding.into()));
    assert_eq!(
        foreign.evaluation_case(&host, HELD_ROUND).err(),
        Some(Error::Binding.into())
    );
    let revision = host.revision();
    assert_eq!(
        foreign.assess(&mut host, revision, &ticket, label),
        Err(Error::Binding.into())
    );
    assert_eq!(bytes(&root), before);
    let numerical = host.decoder_inspection().unwrap().numerical;
    let anchor = host.history_anchor().unwrap();
    drop(host);
    // Existing recovery keeps the contracts and history but returns no new
    // evaluator. Reopening never restores the original process-local role.
    let (mut host, _) = FileOversight::open_generated_text_stream_from_source_anchored(
        root.store(),
        profile(),
        &fixture::decoder(),
        &fixture::tokenizer(),
        selection(),
        &anchor,
    )
    .unwrap();
    let before = bytes(&root);
    assert_eq!(evaluator.check_owner(&host), Err(Error::Binding.into()));
    assert_eq!(
        evaluator.evaluation_case(&host, HELD_ROUND).err(),
        Some(Error::Binding.into())
    );
    let revision = host.revision();
    let fresh_ticket = host.evaluation_ticket(HELD_ROUND).unwrap();
    assert_eq!(
        evaluator.assess(&mut host, revision, &fresh_ticket, label),
        Err(Error::Binding.into())
    );
    assert_eq!(
        evaluator.assess(&mut host, revision, &ticket, label),
        Err(Error::Binding.into())
    );
    assert_eq!(bytes(&root), before);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.andon_config().unwrap(), Some(&config()));
    assert_eq!(host.credibility_report().unwrap().pending_cases, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    assert!(!host.clock_ready());
    assert!(host.decoder_inspection().unwrap().paused);
}

#[test]
fn native_accounting_cannot_be_installed_after_review_or_reset_after_labels() {
    let root = Directory::new();
    let (mut host, _) = FileOversight::create_generated_text_stream_from_source(
        root.store(),
        profile(),
        fixture::decoder(),
        fixture::tokenizer(),
        selection(),
    )
    .unwrap();
    let evaluator = host
        .enable_credibility(host.revision(), protocol())
        .unwrap();
    let mut source = evidence(&root);
    let captured = generate(&mut host, &mut source);
    let (attempt, inputs) = submit(&mut host, &captured, REQUEST, 7);
    review(&mut host, attempt, HELD_ROUND, &inputs, true);
    let before = bytes(&root);
    assert_eq!(
        host.enable_andon(host.revision(), &config()).err(),
        Some(Error::WrongState.into())
    );
    assert_eq!(bytes(&root), before);
    assert!(host.andon_config().unwrap().is_none());
    assert!(
        evaluator
            .evaluation_case(&host, HELD_ROUND)
            .unwrap()
            .case()
            .stopped
    );

    let root = Directory::new();
    let (mut host, _, evaluator) = create(&root);
    let mut source = evidence(&root);
    let captured = generate(&mut host, &mut source);
    let (attempt, inputs) = submit(&mut host, &captured, REQUEST, 7);
    review(&mut host, attempt, HELD_ROUND, &inputs, true);
    let ticket = evaluator.evaluation_case(&host, HELD_ROUND).unwrap();
    let revision = host.revision();
    evaluator
        .assess(
            &mut host,
            revision,
            &ticket,
            Assessment {
                origin: 17,
                evidence_id: [17; 32],
                truth: GroundTruth::Benign,
            },
        )
        .unwrap();
    let binding = host.andon_binding().unwrap().clone();
    let ledger = host.andon_ledger().unwrap().clone();
    let before = bytes(&root);
    assert_eq!(host.enable_andon(0, &config()).unwrap(), binding);
    let changed = FileAndonConfig::new(
        config().identity(),
        vec![AndonBudget {
            false_stops_allowed: 100,
            ..config().windows()[0]
        }],
    )
    .unwrap();
    assert_eq!(
        host.enable_andon(host.revision(), &changed).err(),
        Some(Error::Binding.into())
    );
    assert_eq!(
        host.enable_credibility(host.revision(), protocol()).err(),
        Some(Error::Duplicate.into())
    );
    assert_eq!(bytes(&root), before);
    assert_eq!(host.andon_ledger().unwrap(), &ledger);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
}
