//! Real numerical progress and governance history through the original journal.
//! Synthetic reviewer/evaluator observations test enforcement, not detector quality.
use super::*;
use super::super::super::{FileOversight, FileOversightProfile, JournalError, JournalIo};
use super::super::super::decoder::text::FileTextGenerationCommand;
use crate::action::{ActionSpec, ElapsedTick, VERSION};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationFinish, MAX_SAMPLING_ENTRIES, text::TextGenerationRequest, tokenizer::ByteBpe,
};
use crate::action::consequence::activation::tensor::kv::decoder::MAX_DECODER_PRODUCTS;
use crate::action::consequence::congress::{CredibilityBinding, CredibilityRequirements};
use crate::action::consequence::congress::credibility::{Campaign, CaseSpec, CredibilityLedger,
    EvaluationLabel, EvaluationScope, HelperGeneration, LabelSource, LabelVerdict, Observation};
use crate::action::consequence::delivery::persistent::credibility::{CredibilityActivation, CredibilityWithdrawalRequest};
use crate::action::consequence::oversight::{ReviewWindow,
    evidence_source::{EvidenceIdentity, EvidenceSnapshot}};
use crate::round::{Verdict, commitment};
use crate::Snapshot;
use std::collections::{BTreeMap, BTreeSet};

#[allow(dead_code)]
mod fixtures {
    include!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/src/action/consequence/delivery/persistent/observed/decoder/text/tests/fixtures.rs"));
    pub(super) fn stopped() -> FileDecoderConfig { configured(3.0, 65, Some(256)) }
}
use fixtures::{Directory, bytes, command, request, tokenizer};

fn snapshot() -> Snapshot { Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::new() } }

// Establish genuine control-sequence history with two original reviewed and
// cancelled proposals. Never edit the sequence or pretend elapsed ticks are
// campaign positions. Neither proposal is authorized, dispatched or published.
fn seeded(root: &Directory) -> FileOversight {
    let p = fixtures::host_profile();
    let mut host = fixtures::owner(root, &fixtures::stopped());
    let intent = command(&host, 6, request(b"ab", 2));
    host.generate_decoder_text(host.revision(), intent).unwrap();
    for id in [10, 11] {
        let action = host.propose(host.revision(), id, ActionSpec {
            version: VERSION, scope: p.delivery.scope, target: Some(host.inspect().target),
            payload: b"reviewed".to_vec(), required_witnesses: Vec::new(),
            policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100), units: 8,
        }, snapshot()).unwrap();
        let source = EvidenceSnapshot::new(EvidenceIdentity { scope: p.delivery.scope,
            source: 7, generation: id }, snapshot(),
            BTreeMap::from([("reviewer".into(), b"original review context".to_vec())])).unwrap();
        let inputs = source.inputs_for(&action, &p.committee).unwrap();
        host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
        let round = id + 100; let root = source.reference_root();
        host.begin_review(host.revision(), id, round, root,
            ReviewWindow { commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, snapshot()).unwrap();
        host.commit_review(host.revision(), round, "reviewer",
            commitment(round, "reviewer", &root, Verdict::Allow, b"salt").unwrap()).unwrap();
        host.open_reveals(host.revision(), round).unwrap();
        host.reveal_review(host.revision(), round, "reviewer", Verdict::Allow, b"salt".to_vec()).unwrap();
        host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
        host.cancel(host.revision(), id).unwrap();
    }
    assert!(host.inspect().control.sequence >= 2);
    assert_eq!(host.inspect().executions, 0);
    host
}
fn activation(host: &FileOversight, operation: u64, generation: u64) -> CredibilityActivation {
    let p = fixtures::host_profile();
    let frontier = host.inspect().control.sequence;
    assert!(frontier >= 2);
    let helpers = BTreeMap::from([("reviewer".to_owned(), HelperGeneration {
        generation: p.committee.members()["reviewer"].profile_at(0).model_epoch,
        cohort: p.delivery.congress.members["reviewer"].cohort.clone(),
    })]);
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: operation, model_generation: 1, evaluator_generation: 1,
            held_out_manifest: [42; 32] }, label_owner: "independent-evaluator".into(),
        helpers: helpers.clone(), strata: BTreeSet::from(["publication".into()]),
        cases: (1..=2).map(|id| CaseSpec { id, stratum: "publication".into(),
            evidence_root: [id as u8; 32], dispatch_sequence: frontier }).collect(),
    }).unwrap();
    for id in 1..=2 {
        let observation = if id == 1 { Observation::Clear } else { Observation::Hold { first_sequence: frontier - 1 } };
        ledger.record_observations(id, BTreeMap::from([("reviewer".into(), observation)])).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "independent-evaluator".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [id as u8; 32], recorded_sequence: frontier,
            verdict: if id == 1 { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    let evidence = ledger.seal(frontier).unwrap();
    CredibilityActivation { operation, expected_control_sequence: frontier,
        expected_epoch: host.inspect().control.ledger.epoch, scope: p.delivery.scope,
        policy_generation: p.delivery.policy.generation(), actor_profile: p.delivery.actor.profile(),
        binding: CredibilityBinding { scope: evidence.scope().clone(), label_owner: evidence.label_owner().into(),
            helpers, strata: evidence.strata().clone(), reducer_generation: generation },
        stratum: "publication".into(), requirements: CredibilityRequirements { minimum_safe_cases: 1,
            minimum_violation_cases: 1, minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 1, lead_bonus_weight: 0, lead_saturation_sequences: 0,
            maximum_evidence_age: 100, maximum_member_share_ppm: 1_000_000, maximum_cohort_share_ppm: 1_000_000 },
        snapshot: evidence }
}
fn qualify(host: &mut FileOversight, operation: u64, generation: u64) -> CredibilityActivation {
    let request = activation(host, operation, generation);
    host.activate_credibility(host.revision(), request.clone(), &fixtures::host_profile().committee).unwrap();
    host.check_credibility().unwrap();
    request
}
fn pending(host: &mut FileOversight) {
    let intent = command(host, 7, request(b"ab", 2));
    host.begin_decoder_text(host.revision(), intent).unwrap();
    host.advance_decoder_text(host.revision(), 7, 0).unwrap();
}
fn withdrawal(host: &FileOversight, operation: u64) -> CredibilityWithdrawalRequest {
    CredibilityWithdrawalRequest { operation, expected_control_sequence: host.inspect().control.sequence,
        expected_epoch: host.inspect().control.ledger.epoch }
}

#[test]
fn native_qualification_recovery_preserves_pause_cursor_and_work_until_explicit_resume() {
    let root = Directory::new(); let control_root = Directory::new();
    let mut host = seeded(&root); let mut control = seeded(&control_root);
    let old = qualify(&mut host, 20, 2); qualify(&mut control, 20, 2);
    pending(&mut host); pending(&mut control);
    let numerical = host.decoder_inspection().unwrap().numerical;
    let intent = host.pending_decoder_text().unwrap().unwrap();
    drop(host);
    let (mut host, _) = FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
        &fixtures::stopped(), &tokenizer(false)).unwrap();
    assert!(host.check_credibility().is_err());
    let frozen = bytes(&host);
    host.activate_credibility(0, old, &fixtures::host_profile().committee).unwrap();
    assert_eq!(bytes(&host), frozen);
    assert!(host.check_credibility().is_err()); // historical receipt is not reactivation
    qualify(&mut host, 21, 3);
    assert!(!host.clock_ready());
    assert!(host.decoder_inspection().unwrap().paused);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.pending_decoder_text().unwrap(), Some(intent));
    assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 1);
    let frozen = bytes(&host);
    assert!(host.advance_decoder_text(host.revision(), 7, 1).is_err());
    assert!(host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).is_err());
    assert_eq!(bytes(&host), frozen);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    host.resume_decoder(host.revision(), numerical.actor_revision, numerical.position).unwrap();
    for owner in [&mut host, &mut control] {
        for revision in 1..4 { owner.advance_decoder_text(owner.revision(), 7, revision).unwrap(); }
    }
    let actual = host.decoder_text_generation(7).unwrap();
    let expected = control.decoder_text_generation(7).unwrap();
    assert_eq!(actual.result().unwrap().bytes().unwrap(), b"A");
    assert_eq!(actual.result().unwrap().generation().finish(), GenerationFinish::StopToken);
    assert_eq!(actual.result().unwrap().generation().work(), expected.result().unwrap().generation().work());
    assert_eq!(host.decoder_inspection().unwrap().numerical, control.decoder_inspection().unwrap().numerical);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn native_qualification_and_withdrawal_can_fence_authority_without_replacing_pending_inference() {
    let root = Directory::new(); let mut host = seeded(&root);
    qualify(&mut host, 20, 2); pending(&mut host);
    let intent = host.pending_decoder_text().unwrap();
    let numerical = host.decoder_inspection().unwrap().numerical;
    let epoch = host.inspect().control.ledger.epoch;
    qualify(&mut host, 21, 3); // a live, pending owner, not only a recovered one
    assert_eq!(host.inspect().control.ledger.epoch, epoch + 1);
    let lost = withdrawal(&host, 22);
    host.withdraw_credibility(host.revision(), lost.clone()).unwrap();
    assert!(host.check_credibility().is_err());
    assert!(host.storage_failure().is_none()); // accepted loss is not a storage fault
    assert_eq!(host.pending_decoder_text().unwrap(), intent);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 1);
    qualify(&mut host, 23, 4);
    let frozen = bytes(&host);
    host.withdraw_credibility(0, lost).unwrap(); // old loss cannot withdraw the new activation
    assert_eq!(bytes(&host), frozen); host.check_credibility().unwrap();
    assert_eq!(host.pending_decoder_text().unwrap(), intent);
    assert_eq!(host.decoder_inspection().unwrap().numerical, numerical);
    assert_eq!(host.inspect().executions, 0);
}

#[test]
fn native_qualification_exception_retains_governance_predecessors_and_numerical_exclusion() {
    let root = Directory::new(); let mut host = seeded(&root);
    qualify(&mut host, 20, 2); pending(&mut host);
    let valid = activation(&host, 21, 3);
    let frozen = bytes(&host); let numerical = host.decoder_inspection().unwrap();
    for which in 0..3 {
        let mut invalid = valid.clone();
        match which { 0 => invalid.expected_epoch += 1, 1 => invalid.scope.tenant += 1,
            _ => invalid.binding.reducer_generation = 2 }
        assert!(host.activate_credibility(host.revision(), invalid, &fixtures::host_profile().committee).is_err());
        assert_eq!(bytes(&host), frozen); assert!(host.storage_failure().is_none());
    }
    assert!(host.enable_decoder(host.revision(), fixtures::stopped()).is_err());
    let n = &numerical.numerical;
    assert!(host.advance_decoder_forced(host.revision(), n.actor_revision, n.position, 257,
        DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).is_err());
    assert_eq!(bytes(&host), frozen); assert_eq!(host.decoder_inspection().unwrap(), numerical);
    host.activate_credibility(host.revision(), valid, &fixtures::host_profile().committee).unwrap();
    host.check_credibility().unwrap();
}

#[test]
fn native_pending_withdrawal_storage_failure_closes_owner_and_recovery_keeps_qualification_stale() {
    for stage in [JournalIo::Stage, JournalIo::DirectorySync] {
        let root = Directory::new(); let mut host = seeded(&root);
        qualify(&mut host, 20, 2); pending(&mut host);
        let n = host.decoder_inspection().unwrap().numerical;
        let lost = withdrawal(&host, 21);
        host.store.fail_once(stage);
        assert!(matches!(host.withdraw_credibility(host.revision(), lost), Err(JournalError::Io(_))));
        assert_eq!(host.check_credibility(), Err(JournalError::Unavailable));
        drop(host);
        let (host, _) = FileOversight::open_with_text_decoder(root.store(), fixtures::host_profile(),
            &fixtures::stopped(), &tokenizer(false)).unwrap();
        assert!(host.check_credibility().is_err()); assert!(host.decoder_inspection().unwrap().paused);
        assert_eq!(host.decoder_inspection().unwrap().numerical, n);
        assert_eq!(host.decoder_text_progress(7).unwrap().generation_revision(), 1);
        assert_eq!(host.inspect().executions, 0);
    }
}
