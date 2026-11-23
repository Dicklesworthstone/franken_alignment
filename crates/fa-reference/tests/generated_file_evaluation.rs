//! Original SafeTensors generation, source-bound congress and independent labels.
//! Synthetic weights exercise real native execution; they do not qualify a model.

#[path = "tower_generated_operator/fixture.rs"]
#[allow(dead_code)]
mod fixture;
#[path = "support/file_oversight_actor.rs"]
mod support;
#[path = "support/file_credibility.rs"]
mod credibility;

use fa_reference::{Error, Snapshot};
use fa_reference::action::{ElapsedTick, FrozenAction};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest,
    tokenizer::TokenizationBudget,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome,
    persistent::{
        JournalError, Reconciliation, RecoveryReserve,
        observed::{
            FileHumanReviewer, FileOversight, FileOversightProfile,
            credibility::{FileIndependentEvaluator, andon::FileAndonConfig},
            decoder::text::FileTextGenerationCommand,
            guarded::{
                FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
                evaluation::{FileEvaluatedOversightRoles, andon::FileAndonRecoveryRequirements},
            },
            source::FileSourcePolicy,
            stream::generated::{
                FileTextMessageRequest,
                checked::source::{
                    FileGeneratedSourceRecoveryStatus, GeneratedSourceProfile,
                    GeneratedSourcePublication, evaluation::FileGeneratedEvaluationConfig,
                },
            },
        },
        requests::{FileRequestDisposition, actor::FileGeneratedTextActorPort},
    },
    stream::StreamProfile,
};
use fa_reference::action::consequence::oversight::{
    CommitteeInput,
    credibility::{Assessment, GroundTruth},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource},
    policy_state::{StateFreshness, StateLimits, StateSource},
};
use fa_reference::round::{Verdict, review::andon::{AndonBudget, LedgerIdentity}};
use std::fs;
use support::{Directory, Supervisor};

const REQUEST: u64 = 701;
const ROUND: u64 = 801;

fn profile() -> FileOversightProfile {
    let mut profile = fixture::profile();
    for member in profile.delivery.congress.members.values_mut() {
        member.weight = 1;
    }
    profile.delivery.congress.continue_minimum = 2;
    profile.delivery.congress.narrow_at = 3;
    profile.delivery.congress.suspend_at = 4;
    profile
}

fn stream() -> StreamProfile {
    StreamProfile::new(9, 1, 4, 64, 256).unwrap()
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
            stream: stream(),
            reserve: Some(RecoveryReserve::terminal()),
        },
    }
}

fn accounting() -> FileAndonConfig {
    FileAndonConfig::new(
        LedgerIdentity { deployment: 101, evaluator: 202 },
        vec![AndonBudget {
            window: 1,
            start: 0,
            end: 20,
            false_stops_allowed: 1,
            cost_per_stop: 7,
        }],
    )
    .unwrap()
}

fn create(root: &Directory) -> (FileOversight, FileEvaluatedOversightRoles) {
    let (mut host, roles) = FileOversight::create_generated_text_stream_from_source_evaluated(
        root.store(),
        profile(),
        fixture::decoder(),
        fixture::tokenizer(),
        selection(),
        FileGeneratedEvaluationConfig::new(credibility::protocol())
            .with_andon(accounting())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(host.credibility_report().unwrap().retained_cases, 0);
    assert!(!host.clock_ready());
    assert_eq!(roles.evaluator.check_owner(&host), Ok(()));
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, roles)
}

fn evidence(root: &Directory) -> FileEvidenceSource {
    let scope = profile().delivery.scope;
    let observed = EvidenceSnapshot::new(
        EvidenceIdentity { scope, source: 7, generation: 1 },
        support::snapshot(),
        support::oversight::MEMBERS.into_iter().map(|member| {
            (member.to_owned(), b"actual source context".to_vec())
        }).collect(),
    )
    .unwrap();
    let path = root.0.join("evidence.json");
    fs::write(&path, observed.encode()).unwrap();
    FileEvidenceSource::new(path, 7, scope, 1_048_576).unwrap()
}

fn image(root: &Directory) -> Vec<u8> {
    fs::read(root.store().join("delivery.bin")).unwrap()
}

struct Reviewed {
    attempt: u64,
    action: FrozenAction,
    inputs: CommitteeInput,
    snapshot: Snapshot,
}

fn review(
    host: &mut FileOversight,
    source: &mut FileEvidenceSource,
    verdict: Verdict,
) -> Reviewed {
    let captured = host.refresh_file_source(host.revision(), source, ElapsedTick(1)).unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let command = FileTextGenerationCommand::new(
        7,
        numerical.actor_revision,
        numerical.position,
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
        },
    )
    .unwrap();
    host.generate_decoder_text(host.revision(), command).unwrap();
    assert_eq!(host.decoder_text_progress(7).unwrap().bytes().unwrap(), b"A");
    let snapshot = captured.snapshot().clone();
    let request = FileTextMessageRequest {
        request: REQUEST,
        generation: 7,
        generation_revision: host.decoder_text_progress(7).unwrap().generation_revision(),
        target: host.inspect().target,
        policy_epoch: host.inspect().control.ledger.epoch,
        deadline: ElapsedTick(100),
    };
    let admitted = host.submit_decoder_text_message(host.revision(), request, snapshot.clone()).unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } = admitted.disposition else {
        panic!("original generated message must reach native admission");
    };
    let action = host.request_action(REQUEST).unwrap().clone();
    let inputs = captured.inputs_for(&action, &profile().committee).unwrap();
    host.record_inputs(host.revision(), attempt, host.input_revision(attempt).unwrap(), inputs.clone()).unwrap();
    host.begin_review(
        host.revision(), attempt, ROUND, support::oversight::ROOT,
        support::oversight::window(host), snapshot.clone(),
    )
    .unwrap();
    support::oversight::votes(host, ROUND, verdict);
    let receipt = host.finish_review(host.revision(), ROUND, Some(&inputs), snapshot.clone()).unwrap().unwrap();
    assert_eq!(receipt.policy.control.decision.consequence,
        if verdict == Verdict::Allow { Consequence::Continue } else { Consequence::HoldEffect });
    Reviewed { attempt, action, inputs, snapshot }
}

fn assessment(truth: GroundTruth) -> Assessment {
    Assessment { origin: 1, evidence_id: [19; 32], truth }
}

fn assess(host: &mut FileOversight, evaluator: &FileIndependentEvaluator, truth: GroundTruth) -> bool {
    let ticket = evaluator.evaluation_case(host, ROUND).unwrap();
    let revision = host.revision();
    evaluator.assess(host, revision, &ticket, assessment(truth)).unwrap()
}

fn recovery_requirements(host: &FileOversight) -> FileAndonRecoveryRequirements {
    let state = host.inspect();
    FileAndonRecoveryRequirements::new(
        FileRecoveryRequirements {
            guards: FileGuardSet {
                stream: Some(stream()),
                decoder: Some(fixture::decoder()),
                decoder_stop: None,
                source: Some(selection().source),
                identity: None,
                campaigns: None,
                credential: None,
            },
            effective_policy: host.current_policy().unwrap().clone(),
            credential_epoch: None,
            minimum: FileRecoveryFloor {
                journal_revision: state.revision,
                control_sequence: state.control.sequence,
                authority_epoch: state.control.ledger.epoch,
            },
        },
        accounting(),
        accounting().windows().to_vec(),
        host.credibility_report().unwrap().revision,
    )
    .unwrap()
}

#[test]
fn generated_publication_and_independent_labeling_share_the_original_owner() {
    let root = Directory::new();
    let (mut host, roles) = create(&root);
    assert_eq!(roles.evaluator.evaluation_case(&host, ROUND).err(),
        Some(JournalError::Contract(Error::Missing)));
    let mut source = evidence(&root);
    let reviewed = review(&mut host, &mut source, Verdict::Allow);
    let ticket = roles.evaluator.evaluation_case(&host, ROUND).unwrap();
    assert_eq!(ticket.case().round, ROUND);
    assert_eq!(ticket.case().attempt, reviewed.attempt);
    assert!(!ticket.case().stopped);
    assert_eq!(host.credibility_report().unwrap().pending_cases, 1);
    // The native evaluator adds no substitute for the existing human effect key.
    let automatic = host.authorize(host.revision(), reviewed.attempt, &reviewed.inputs, reviewed.snapshot.clone()).unwrap();
    assert!(host.publish_checked(host.revision(), reviewed.attempt, Some(&reviewed.inputs),
        reviewed.snapshot.clone(), ElapsedTick(1)).is_err());
    assert_eq!(host.inspect().executions, 0);
    let request = host.request_human_approval(host.revision(), 9001, reviewed.attempt,
        &reviewed.inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = roles.oversight.human.approve(&mut host, revision, &request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &reviewed.action, &reviewed.inputs,
        reviewed.snapshot.clone()).unwrap();
    assert!(matches!(host.publish_checked(host.revision(), reviewed.attempt,
        Some(&reviewed.inputs), reviewed.snapshot.clone(), ElapsedTick(1)).unwrap().outcome,
        EndpointOutcome::Executed { .. }));
    assert!(matches!(host.reconcile(host.revision(), reviewed.attempt).unwrap(),
        Reconciliation::Resolved(EndpointOutcome::Executed { .. })));
    let control = host.inspect().control;
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert!(assess(&mut host, &roles.evaluator, GroundTruth::Benign));
    assert_eq!(host.inspect().control, control);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.credibility_report().unwrap().benign_origins, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    let committed = image(&root);
    assert!(!assess(&mut host, &roles.evaluator, GroundTruth::Benign));
    assert_eq!(image(&root), committed);
    assert_eq!(host.evaluation_history(ROUND).unwrap(), &[assessment(GroundTruth::Benign)]);
}

#[test]
fn generated_empirical_hold_opens_native_calibration_incident_without_new_authority() {
    let root = Directory::new();
    let (mut host, roles) = create(&root);
    let mut source = evidence(&root);
    let reviewed = review(&mut host, &mut source, Verdict::Hold);
    let control = host.inspect().control;
    let numerical = host.decoder_inspection().unwrap().numerical;
    assert!(assess(&mut host, &roles.evaluator, GroundTruth::Benign));
    assert_eq!(host.inspect().control, control);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    let window = &host.andon_ledger().unwrap().windows()[&1];
    assert_eq!(window.false_stops, 1);
    assert_eq!(window.total_cost, 7);
    let incident = window.incident.as_ref().unwrap();
    assert_eq!(incident.opened_by_round, ROUND);
    assert_eq!(incident.rounds.iter().copied().collect::<Vec<_>>(), vec![ROUND]);
    assert_eq!(host.andon_receipt(ROUND).unwrap().confirmation().occurred_at, 1);
    let committed = image(&root);
    assert!(!assess(&mut host, &roles.evaluator, GroundTruth::Benign));
    assert_eq!(image(&root), committed);
    assert!(host.authorize(host.revision(), reviewed.attempt,
        &reviewed.inputs, reviewed.snapshot).is_err());
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
}

#[test]
fn numerically_equal_generated_cases_do_not_transfer_evaluator_or_ticket_custody() {
    let first_root = Directory::new();
    let second_root = Directory::new();
    let (mut first, first_roles) = create(&first_root);
    let (mut second, second_roles) = create(&second_root);
    review(&mut first, &mut evidence(&first_root), Verdict::Hold);
    review(&mut second, &mut evidence(&second_root), Verdict::Hold);
    let first_ticket = first_roles.evaluator.evaluation_case(&first, ROUND).unwrap();
    let second_ticket = second_roles.evaluator.evaluation_case(&second, ROUND).unwrap();
    assert_eq!(first_ticket.case(), second_ticket.case());
    assert_eq!(first.credibility_report().unwrap().revision,
        second.credibility_report().unwrap().revision);
    let before = image(&second_root);
    assert_eq!(first_roles.evaluator.evaluation_case(&second, ROUND).err(),
        Some(JournalError::Contract(Error::Binding)));
    let revision = second.revision();
    assert_eq!(first_roles.evaluator.assess(&mut second, revision, &second_ticket,
        assessment(GroundTruth::Benign)), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(second_roles.evaluator.assess(&mut second, revision, &first_ticket,
        assessment(GroundTruth::Benign)), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(image(&second_root), before);
    assert_eq!(second.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    assert!(assess(&mut second, &second_roles.evaluator, GroundTruth::Benign));
    assert_eq!(second.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(first.andon_ledger().unwrap().windows()[&1].false_stops, 0);
}

#[test]
fn anchored_generated_recovery_preserves_censored_labels_and_requires_fresh_role_and_time() {
    let root = Directory::new();
    let (mut host, old_roles) = create(&root);
    let mut source = evidence(&root);
    review(&mut host, &mut source, Verdict::Hold);
    let old_ticket = old_roles.evaluator.evaluation_case(&host, ROUND).unwrap();
    assert!(assess(&mut host, &old_roles.evaluator, GroundTruth::Censored));
    assert_eq!(host.credibility_report().unwrap().censored_cases, 1);
    assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 0);
    let expected = recovery_requirements(&host);
    let anchor = host.history_anchor().unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let original = image(&root);
    drop(host);
    let mut recovery = FileOversight::begin_open_andon_guarded_generated_text_stream_from_source_anchored(
        root.store(), profile(), &expected, &fixture::decoder(), &fixture::tokenizer(),
        selection(), &credibility::protocol(), &anchor,
    )
    .unwrap();
    for _ in 0..2048 {
        let progress = recovery.progress();
        if progress.status == FileGeneratedSourceRecoveryStatus::Ready {
            break;
        }
        assert_eq!(progress.status, FileGeneratedSourceRecoveryStatus::Replaying);
        let next = recovery.advance(progress.replayed_events, 1).unwrap();
        assert_eq!(next.replayed_events, progress.replayed_events + 1);
        assert_eq!(image(&root), original);
    }
    assert_eq!(recovery.progress().status, FileGeneratedSourceRecoveryStatus::Ready);
    let (mut restored, roles) = recovery.finish().unwrap();
    assert!(!restored.clock_ready());
    assert_eq!(restored.decoder_inspection().unwrap().numerical.position, numerical.position);
    assert_eq!(restored.decoder_inspection().unwrap().numerical.sampled_draws, numerical.sampled_draws);
    assert_eq!(restored.credibility_report().unwrap().censored_cases, 1);
    assert_eq!(restored.evaluation_history(ROUND).unwrap(), &[assessment(GroundTruth::Censored)]);
    assert_eq!(old_roles.evaluator.check_owner(&restored), Err(JournalError::Contract(Error::Binding)));
    let fresh_ticket = roles.evaluator.evaluation_case(&restored, ROUND).unwrap();
    assert_eq!(fresh_ticket.case(), old_ticket.case());
    let revision = restored.revision();
    let fenced = image(&root);
    assert_eq!(roles.evaluator.assess(&mut restored, revision, &old_ticket,
        assessment(GroundTruth::Benign)), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(old_roles.evaluator.assess(&mut restored, revision, &fresh_ticket,
        assessment(GroundTruth::Benign)), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(roles.evaluator.assess(&mut restored, revision, &fresh_ticket,
        assessment(GroundTruth::Benign)), Err(JournalError::Contract(Error::Incomplete)));
    assert_eq!(image(&root), fenced);
    restored.observe_time(restored.revision(), ElapsedTick(40)).unwrap();
    let control = restored.inspect().control;
    assert!(assess(&mut restored, &roles.evaluator, GroundTruth::Benign));
    assert_eq!(restored.inspect().control, control);
    assert_eq!(restored.inspect().executions, 0);
    assert_eq!(restored.evaluation_history(ROUND).unwrap(),
        &[assessment(GroundTruth::Censored), assessment(GroundTruth::Benign)]);
    let confirmation = restored.andon_receipt(ROUND).unwrap().confirmation();
    assert_eq!(confirmation.occurred_at, 1);
    assert_eq!(confirmation.confirmed_at, 40);
    assert_eq!(restored.andon_ledger().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(restored.andon_ledger().unwrap().windows()[&1].total_cost, 7);
    let confirmed = image(&root);
    assert!(!assess(&mut restored, &roles.evaluator, GroundTruth::Benign));
    assert_eq!(image(&root), confirmed);
}
