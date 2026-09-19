#![allow(dead_code)]
#[path = "file_oversight.rs"] mod oversight;
pub use oversight::*;
use fa_reference::action::consequence::delivery::publication_gate::PublicationLimits;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::publication::capture::{
    FileCaptureIdentity, FilePublicationCapture, PublicationInputFile,
};
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{FilePublicationInputs, FileWitnessInput};
use fa_reference::action::consequence::oversight::CommitteeInput;
use fa_reference::action::{ElapsedTick, FrozenAction};
use fa_reference::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use fa_reference::witness::{AdapterDomainInput, DomainClosure, DomainProjection, QueryRole, SnapshotEntry, WitnessRequest};
use fa_reference::witness::refinement::RefinementBudget;

pub const SOURCE: u64 = 91;
pub fn limits() -> PublicationLimits {
    PublicationLimits { bindings: 8, validation: RefinementBudget { steps: 10_000, value_bytes: 1_048_576 } }
}
pub fn source_host(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_with_publication_validation(root.store(), profile(), limits()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
pub fn observations(inputs: &CommitteeInput, revision: u64, keys: &[u64]) -> FilePublicationInputs {
    let key = ProjectionKey { source: 40, branch: 4, projection: 7, source_epoch: 1 };
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 4).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    frontiers.record_close(marker).unwrap();
    let structured = FileWitnessInput::new(revision, revision, 20,
        AdapterDomainInput::new(DomainProjection::new(40, 1, key), DomainClosure::Closed(marker)),
        keys.iter().map(|key| SnapshotEntry::new(*key, 1, b"original".to_vec()).unwrap()).collect(), &frontiers).unwrap();
    FilePublicationInputs::new(Some(structured), Some(inputs.views()["alpha"].actual_input().clone()))
}
pub fn packet(attempt: u64, action: &FrozenAction, inputs: &CommitteeInput,
    generation: u64, keys: &[u64]) -> FilePublicationCapture
{
    FilePublicationCapture::new(attempt, FileCaptureIdentity { source: SOURCE, generation },
        action, observations(inputs, generation, keys)).unwrap()
}
pub fn requests() -> Vec<WitnessRequest> {
    vec![WitnessRequest::ExactValue { key: 0, role: QueryRole::Subject },
        WitnessRequest::AbsentKey { key: 1 }, WitnessRequest::EmptyRange { start: 6, end: 9 },
        WitnessRequest::RangeMembers { start: 2, end: 6 }]
}
pub fn source(root: &Directory) -> PublicationInputFile {
    PublicationInputFile::new(root.0.join("witness-input.bin"), SOURCE).unwrap()
}
pub fn replace_source(root: &Directory, capture: &FilePublicationCapture) {
    let pending = root.0.join("witness-input.pending");
    std::fs::write(&pending, capture.to_bytes().unwrap()).unwrap();
    std::fs::rename(pending, root.0.join("witness-input.bin")).unwrap();
}
pub fn refresh(host: &mut FileOversight, root: &Directory, attempt: u64) {
    host.refresh_publication_from_file(host.revision(), attempt, &source(root)).unwrap().unwrap();
}
pub fn source_keys(host: &mut FileOversight, reviewer: &FileHumanReviewer, root: &Directory, id: u64) -> Keys {
    let (action, inputs) = reviewed(host, id, b"visible");
    let original = packet(id, &action, &inputs, 1, &[0, 2, 4]);
    replace_source(root, &original);
    host.bind_publication_file_source(host.revision(), id, original, requests()).unwrap();
    refresh(host, root, id);
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(now.0 + 30)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human, request }
}
