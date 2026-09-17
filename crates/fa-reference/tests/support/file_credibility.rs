#![allow(dead_code)]
#[path = "file_oversight.rs"] pub mod ordinary;
pub use ordinary::{Directory, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::oversight::{CommitteeInput, ObservedReceipt};
use fa_reference::action::consequence::oversight::credibility::{Assessment, EvaluationProtocol, Fraction, GroundTruth};
use fa_reference::action::consequence::delivery::persistent::observed::{FileHumanReviewer, FileOversight, FileOversightProfile};
use fa_reference::action::consequence::delivery::persistent::observed::credibility::{FileCredibilityUpdate, FileIndependentEvaluator};
use fa_reference::round::Verdict;

pub fn profile() -> FileOversightProfile {
    let mut p = ordinary::profile();
    for entry in p.delivery.congress.members.values_mut() { entry.weight = 1; }
    p.delivery.congress.continue_minimum = 2;
    p.delivery.congress.narrow_at = 3;
    p.delivery.congress.suspend_at = 4;
    p
}
pub fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 31, stratum: 2, period: 3,
        minimum_violation_origins: 1, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
pub fn create(root: &Directory) -> (FileOversight, FileHumanReviewer, FileIndependentEvaluator) {
    let (mut host, human) = FileOversight::create(root.store(), profile()).unwrap();
    let evaluator = host.enable_credibility(host.revision(), protocol()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, human, evaluator)
}
pub fn begin(host: &mut FileOversight, id: u64) -> CommitteeInput {
    let action = host.propose(host.revision(), id, ordinary::spec(host, b"evaluate"), snapshot()).unwrap();
    let inputs = ordinary::inputs(&action, b"actual frozen helper context");
    host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
    host.begin_review(host.revision(), id, id + 100, ordinary::ROOT, ordinary::window(host), snapshot()).unwrap();
    inputs
}
pub fn review(host: &mut FileOversight, id: u64, verdict: Verdict) -> ObservedReceipt {
    let inputs = begin(host, id);
    ordinary::votes(host, id + 100, verdict);
    host.finish_review(host.revision(), id + 100, Some(&inputs), snapshot()).unwrap().unwrap()
}
pub fn assessment(origin: u64, truth: GroundTruth) -> Assessment {
    Assessment { origin, evidence_id: [19; 32], truth }
}
pub fn label(host: &mut FileOversight, evaluator: &FileIndependentEvaluator,
    id: u64, origin: u64, truth: GroundTruth) -> bool
{
    let ticket = host.evaluation_ticket(id + 100).unwrap(); let revision = host.revision();
    evaluator.assess(host, revision, &ticket, assessment(origin, truth)).unwrap()
}
pub fn qualified(host: &mut FileOversight, evaluator: &FileIndependentEvaluator) {
    review(host, 1, Verdict::Hold); label(host, evaluator, 1, 1, GroundTruth::Violation);
    review(host, 2, Verdict::Allow); label(host, evaluator, 2, 2, GroundTruth::Benign);
    assert!(host.credibility_report().unwrap().qualified());
}
pub fn update(host: &FileOversight, operation: u64) -> FileCredibilityUpdate {
    let control = host.inspect().control;
    FileCredibilityUpdate { operation, expected_control_sequence: control.sequence,
        expected_authority_epoch: control.ledger.epoch,
        expected_evaluation_revision: host.credibility_report().unwrap().revision }
}
