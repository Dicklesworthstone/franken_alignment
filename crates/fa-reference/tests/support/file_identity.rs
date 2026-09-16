#![allow(dead_code)]
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::identity::{IdentityAnchor, ModelManifest, ModelPassport};
use fa_reference::action::consequence::oversight::identity::{IdentityOutcome, IdentityPolicy, IdentityStatus};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::identity::{FileIdentityChallenge, FileIdentityObserver};

pub fn manifest() -> ModelManifest {
    ModelManifest { tenant: 1, model: 9, model_generation: 1, host_generation: 1,
        tokenizer_generation: 1, weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
        architecture: [4; 32], numeric_profile: [5; 32] }
}
pub fn capture_profile() -> CaptureProfile {
    CaptureProfile { tenant: 1, model: 9, model_generation: 1, tap: 4, layout_generation: 1 }
}
pub fn passport() -> ModelPassport {
    ModelPassport::new(51, 1, manifest(), vec![IdentityAnchor::new(10, capture_profile(), 5,
        vec![7], &[[-1.0, 1.0], [0.0, 2.0]]).unwrap()]).unwrap()
}
pub fn policy() -> IdentityPolicy {
    IdentityPolicy { observer_id: 99, timeout_ticks: 10, validity_ticks: 20, max_checks: 8 }
}
pub fn frame(sequence: u64, values: &[f32]) -> SourceFrame {
    SourceFrame::capture(FrameIdentity { profile: capture_profile(), stream: 5, sequence, position: 0 }, values).unwrap()
}
pub fn enable(host: &mut FileOversight) -> FileIdentityObserver {
    host.enable_identity_checks(host.revision(), passport(), policy()).unwrap()
}
pub fn begin(host: &mut FileOversight, check: u64) -> FileIdentityChallenge {
    let control = host.inspect().control;
    let actor = host.actor_snapshot().unwrap().actor_revision;
    host.begin_identity_check(host.revision(), check, control.sequence, actor).unwrap().unwrap()
}
pub fn measure(host: &mut FileOversight, observer: &FileIdentityObserver,
    challenge: &FileIdentityChallenge, sequence: u64)
{
    let now = host.inspect().control.ledger.elapsed.unwrap(); let revision = host.revision();
    let report = observer.observe_manifest(host, revision, challenge, manifest(), now).unwrap();
    assert_eq!(report.measurement.unwrap().outcome, IdentityOutcome::Collecting);
    assert!(report.containment.is_none());
    let revision = host.revision();
    let report = observer.observe_anchor(host, revision, challenge, 10, &frame(sequence, &[0.0, 1.0]), now).unwrap();
    assert_eq!(report.measurement.unwrap().outcome, IdentityOutcome::Matched);
    assert!(report.containment.is_none());
}
pub fn install(host: &mut FileOversight, challenge: &FileIdentityChallenge) {
    let control = host.inspect().control;
    let receipt = host.apply_identity_check(host.revision(), challenge, control.sequence, control.ledger.epoch).unwrap();
    assert_eq!(receipt.report.outcome, IdentityOutcome::Matched);
    assert!(receipt.cancelled.is_empty()); assert_eq!(receipt.refunded_units, 0);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Matching {
        check: challenge.id(), valid_until: challenge.evidence().valid_until(),
    });
}
pub fn matched(host: &mut FileOversight, observer: &FileIdentityObserver, check: u64, sequence: u64) -> FileIdentityChallenge {
    let challenge = begin(host, check); measure(host, observer, &challenge, sequence); install(host, &challenge); challenge
}
pub fn now(host: &FileOversight) -> ElapsedTick { host.inspect().control.ledger.elapsed.unwrap() }
