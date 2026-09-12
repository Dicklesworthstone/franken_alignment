//! Synthetic labelled captures for algorithm tests, not detector qualification.
use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame, ProgressiveFrame};
use fa_reference::action::consequence::activation::probe::{LinearProbe, ProbeOutcome};
use fa_reference::action::consequence::activation::probe::training::*;
use std::collections::BTreeMap;

pub fn profile() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 }
}
pub fn origin(id: u64) -> CaseOrigin { CaseOrigin { task: id, lineage: id + 100 } }
pub fn split(id: u64) -> DataSplit {
    match id { 1..=4 => DataSplit::Training, 5..=8 => DataSplit::Calibration, _ => DataSplit::Evaluation }
}
pub fn assignments() -> BTreeMap<CaseOrigin, DataSplit> {
    (1..=12).map(|id| (origin(id), split(id))).collect()
}
pub fn source(id: u64, values: &[f32]) -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: profile(), stream: id, sequence: 1, position: 0 }, values).unwrap()
}
pub fn label(id: u64) -> CaseLabel { if id % 2 == 0 { CaseLabel::Violation } else { CaseLabel::Benign } }
pub fn corpus(heldout_scale: f32, reverse_training: bool) -> SealedCorpus {
    let mut corpus = ProbeCorpus::new(1, 1, profile(), 2, assignments()).unwrap();
    for id in 1..=12 {
        let value = if id % 2 == 0 { 2.0 } else { -2.0 };
        let values = [value * if id <= 4 { 1.0 } else { heldout_scale }, 7.0];
        let mut class = label(id);
        if reverse_training && id <= 4 {
            class = if class == CaseLabel::Benign { CaseLabel::Violation } else { CaseLabel::Benign };
        }
        corpus.capture(origin(id), class, source(id, &values)).unwrap();
    }
    corpus.seal().unwrap()
}
pub fn policy() -> FitPolicy { FitPolicy::new(30, 2, 100, 0.25, 0.01, 0.001).unwrap() }
pub fn fit(corpus: &SealedCorpus) -> FittedProbe {
    let policy = policy();
    let work = corpus.estimate_fit(&policy).unwrap();
    corpus.fit(policy, TrainingBudget { source_coordinate_visits: work.source_coordinate_visits }).unwrap()
}
pub fn outcome(probe: &LinearProbe, values: &[f32]) -> ProbeOutcome {
    let source = source(99, values);
    let bytes = source.encode_initial(23).unwrap();
    let frame = ProgressiveFrame::from_initial(&source.verify_block(&bytes).unwrap()).unwrap();
    probe.evaluate(&frame).unwrap().outcome()
}
