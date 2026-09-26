// Real model/source files from the existing native fixture; these extra helpers
// provide synthetic independent labels and ORIGINAL reviewed control history.
include!(concat!(env!("CARGO_MANIFEST_DIR"),
    "/examples/supervise_publication/workflow/actor_service/native/tests/fixture.rs"));
use crate::config::CLOCK_DOMAIN;
use fa_reference::action::consequence::congress::{CredibilityBinding, CredibilityRequirements};
use fa_reference::action::consequence::congress::credibility::{Campaign, CaseSpec, CredibilityLedger,
    EvaluationLabel, EvaluationScope, HelperGeneration, LabelSource, LabelVerdict, Observation};
use fa_reference::action::consequence::delivery::persistent::credibility::CredibilityActivation;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput, producer::{FilePublicationProducer, PublicationProducerProfile},
};
use fa_reference::action::consequence::oversight::ReviewWindow;
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};
use fa_reference::round::{Verdict, commitment};
use std::collections::BTreeSet;

pub(super) fn publication_json(root: &Root, config: &Config, generation: u64) -> String {
    let s = config.profile.delivery.scope;
    let witness = format!(r#"{{"schema":"fa.supervised-witnesses/3","source":91,"producer":{{"path":"{}/producer/delivery.bin","scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}}}},"feed":{{"source":41,"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{{"kind":"exact_value","key":0,"role":"subject"}},{{"kind":"absent_key","key":1}}]}}"#,
        root.0.display(), s.tenant, s.principal, s.run, s.branch, s.authority);
    format!(r#"{{"schema":"fa.supervised-joint-publication/1","joint":{{"id":9,"generation":{generation},"minimum_safe_roots":2,"minimum_violation_roots":2,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":8,"max_member_outcomes":32}},"publication":{witness}}}"#)
}
pub(super) fn publication(root: &Root, config: &Config) -> (PublicationProfile, FilePublicationProducer) {
    let scope = config.profile.delivery.scope;
    let key = ProjectionKey { source: 40, branch: scope.branch, projection: 7, source_epoch: 1 };
    let close = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap(); frontiers.record_close(close).unwrap();
    let witness = FileWitnessInput::new(1, 1, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(close)),
        vec![SnapshotEntry::new(0, 1, b"original".to_vec()).unwrap()], &frontiers).unwrap();
    let (producer, _) = FilePublicationProducer::create(root.0.join("producer"),
        PublicationProducerProfile { source: 91, scope, feed: 41, clock_domain: CLOCK_DOMAIN, after: 0 },
        FilePublicationInputs::new(Some(witness), None), ElapsedTick(1000)).unwrap();
    (PublicationProfile::decode(publication_json(root, config, 1).as_bytes()).unwrap(), producer)
}
fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }
}
pub(super) fn activation(config: &Config, sequence: u64, epoch: u64,
    operation: u64, generation: u64, pairs: u64) -> CredibilityActivation
{
    assert!(sequence >= 2);
    let helpers = config.profile.delivery.congress.members.iter().map(|(name, member)| (name.clone(),
        HelperGeneration { generation: config.profile.committee.members()[name].profile_at(0).model_epoch,
            cohort: member.cohort.clone() })).collect::<BTreeMap<_, _>>();
    let mut ledger = CredibilityLedger::new(Campaign {
        scope: EvaluationScope { campaign: operation, model_generation: 1, evaluator_generation: 1,
            held_out_manifest: [42; 32] }, label_owner: "independent-evaluator".into(), helpers: helpers.clone(),
        strata: BTreeSet::from(["publication".into()]),
        cases: (1..=pairs * 2).map(|id| CaseSpec { id, stratum: "publication".into(),
            evidence_root: [id as u8; 32], dispatch_sequence: sequence }).collect(),
    }).unwrap();
    for id in 1..=pairs * 2 {
        let safe = id % 2 == 1;
        let observation = if safe { Observation::Clear } else { Observation::Hold { first_sequence: sequence - 1 } };
        ledger.record_observations(id, helpers.keys().map(|name| (name.clone(), observation)).collect()).unwrap();
        ledger.record_label(id, EvaluationLabel { owner: "independent-evaluator".into(), evaluator_generation: 1,
            source: LabelSource::IndependentEvaluation, evidence_root: [id as u8; 32], recorded_sequence: sequence,
            verdict: if safe { LabelVerdict::Safe } else { LabelVerdict::Violation } }).unwrap();
    }
    let snapshot = ledger.seal(sequence).unwrap();
    CredibilityActivation { operation, expected_control_sequence: sequence, expected_epoch: epoch,
        scope: config.profile.delivery.scope, policy_generation: config.profile.delivery.policy.generation(),
        actor_profile: config.profile.delivery.actor.profile(),
        binding: CredibilityBinding { scope: snapshot.scope().clone(), label_owner: snapshot.label_owner().into(),
            helpers, strata: snapshot.strata().clone(), reducer_generation: generation },
        stratum: "publication".into(), requirements: CredibilityRequirements { minimum_safe_cases: 1,
            minimum_violation_cases: 1, minimum_precision_ppm: 1_000_000, minimum_timely_recall_ppm: 1_000_000,
            maximum_false_positive_ppm: 0, base_weight: 5, lead_bonus_weight: 0, lead_saturation_sequences: 0,
            maximum_evidence_age: 100, maximum_member_share_ppm: 500_000, maximum_cohort_share_ppm: 500_000 }, snapshot }
}

// Baseline joint policy governs later promotions; it is not itself evidence of
// qualification. Seed two real reviewed/cancelled finish proposals, then activate
// original held-out evidence and start the separate generation to be recovered.
pub(super) fn seed(config: &mut Config, native: &Inputs, publication: &PublicationProfile,
    steps: u64) -> (CredibilityActivation, CredibilityActivation)
{
    let selected = publication.native_selection(&config.profile, native.stream, Some(RecoveryReserve::terminal())).unwrap();
    let (mut host, _) = selected.create(&config.store, config.profile.clone(), native.decoder.clone(), native.tokenizer.clone()).unwrap();
    host.enable_file_source(host.revision(), config.source_policy).unwrap();
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let previous = FileTextGenerationCommand::new(6, n.actor_revision, n.position, native.request.clone()).unwrap();
    host.generate_decoder_text(host.revision(), previous).unwrap();
    for id in [10, 11] {
        let spec = host.stream_finish_spec(ElapsedTick(10000)).unwrap();
        let action = host.propose(host.revision(), id, spec, snapshot()).unwrap();
        publication.prepare_with_clock(&mut host, id, || ElapsedTick(1000)).unwrap();
        let source = EvidenceSnapshot::new(EvidenceIdentity { scope: config.profile.delivery.scope,
            source: 51, generation: 1 }, snapshot(), ["alpha", "beta"].into_iter()
            .map(|m| (m.to_owned(), format!("review this fixture: {m}").into_bytes())).collect()).unwrap();
        let inputs = source.inputs_for(&action, &config.profile.committee).unwrap();
        host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
        let round = id + 100; let root = source.reference_root();
        host.begin_review(host.revision(), id, round, root,
            ReviewWindow { commit_by: ElapsedTick(1005), reveal_by: ElapsedTick(1008) }, snapshot()).unwrap();
        for name in ["alpha", "beta"] {
            host.commit_review(host.revision(), round, name, commitment(round, name, &root, Verdict::Allow, b"salt").unwrap()).unwrap();
        }
        host.open_reveals(host.revision(), round).unwrap();
        for name in ["alpha", "beta"] {
            host.reveal_review(host.revision(), round, name, Verdict::Allow, b"salt".to_vec()).unwrap();
        }
        host.finish_review(host.revision(), round, Some(&inputs), snapshot()).unwrap().unwrap();
        host.cancel(host.revision(), id).unwrap();
    }
    let old = activation(config, host.inspect().control.sequence, host.inspect().control.ledger.epoch, 20, 2, 2);
    host.activate_credibility(host.revision(), old.clone(), &config.profile.committee).unwrap();
    // Qualification fences prior source eligibility too. Establish a genuinely
    // new acquisition at this epoch before freezing/advancing the next intent.
    host.refresh_file_source(host.revision(), &mut config.source, ElapsedTick(1000)).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1000)).unwrap();
    let n = host.decoder_inspection().unwrap().numerical;
    let intent = FileTextGenerationCommand::new(native.generation, n.actor_revision, n.position, native.request.clone()).unwrap();
    host.begin_decoder_text(host.revision(), intent).unwrap();
    for revision in 0..steps { host.advance_decoder_text(host.revision(), native.generation, revision).unwrap(); }
    let next = activation(config, host.inspect().control.sequence,
        host.inspect().control.ledger.epoch + 1, 21, 3, 2); // exact original recovery fence
    assert_eq!(host.inspect().executions, 0);
    (old, next)
}
pub(super) fn programs(config: &mut Config, root: &Root, epoch: u64) {
    config.programs = ["alpha", "beta"].into_iter().map(|member| (member.to_owned(),
        HelperProgram::new(std::env::current_exe().unwrap(), root.0.clone(),
            vec!["--exact".into(), "workflow::actor_service::native::qualification::tests::synthetic_helper".into(), "--nocapture".into()],
            BTreeMap::from([(OsString::from("FA_NATIVE_QUAL_MEMBER"), OsString::from(member)),
                (OsString::from("FA_NATIVE_QUAL_EPOCH"), OsString::from(epoch.to_string()))])).unwrap())).collect();
}
