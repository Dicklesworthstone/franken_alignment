#![allow(dead_code)]
#[path = "file_delivery.rs"] mod delivery;
pub use delivery::{Directory, snapshot};
use fa_reference::action::consequence::delivery::persistent::FilePermit;
use fa_reference::action::consequence::delivery::persistent::observed::*;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ObservedReceipt, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::human::HumanReviewPolicy;
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, VERSION};
use fa_reference::evidence_view::{AuthorizationProjection, EvidencePartView, EvidenceViewManifest, OriginalIdentity, RedactionMetadata, WindowMetadata};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::round::{Verdict, commitment};
use std::collections::BTreeMap;

pub const ROOT: [u8; 32] = [9; 32];
pub const MEMBERS: [&str; 2] = ["alpha", "beta"];

pub fn profile() -> FileOversightProfile {
    let delivery = delivery::profile();
    let committee = CommitteeContract::new(MEMBERS.into_iter().map(|member| {
        (member.to_owned(), HelperContract::new(InputProfileBinding {
            profile_id: 1, profile_bytes: b"exact-view-file-v1".to_vec(), tokenizer_epoch: 2,
            policy_epoch: 0, model_epoch: 3,
        }, 7, b"Approve this exact publication?".to_vec()).unwrap())
    }).collect()).unwrap();
    FileOversightProfile { delivery, committee,
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 16 } }
}
pub fn create(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
pub fn spec(host: &FileOversight, payload: &[u8]) -> ActionSpec {
    let state = host.inspect();
    let now = state.control.ledger.elapsed.unwrap_or(ElapsedTick(0));
    ActionSpec { version: VERSION, scope: profile().delivery.scope, target: Some(state.target),
        payload: payload.to_vec(), required_witnesses: Vec::new(), policy_epoch: state.control.ledger.epoch,
        deadline: ElapsedTick(now.0 + 100), units: 16 }
}
pub fn inputs(action: &FrozenAction, evidence: &[u8]) -> CommitteeInput {
    let contracts = profile().committee;
    let mut views = BTreeMap::new();
    for (member, helper) in contracts.members() {
        let mut bytes = action_frame(action);
        let question = bytes.len();
        bytes.extend_from_slice(helper.question());
        let source = bytes.len();
        bytes.extend_from_slice(evidence);
        let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, helper.profile_at(action.spec().policy_epoch), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: question } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: question, end: source } },
            SubmittedPart { kind: PartKind::Evidence { source_id: 7, transform_id: 1 }, span: ByteSpan { start: source, end } },
        ], Vec::new()).unwrap();
        let original = OriginalIdentity { tenant_id: action.spec().scope.tenant, object_id: 7, generation: 1 };
        let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
            projection_id: helper.projection_id(), policy_epoch: action.spec().policy_epoch, projected_originals: vec![original],
        }, vec![EvidencePartView { input_part_index: 2, original, transform_id: 1,
            redaction: RedactionMetadata::None, window: WindowMetadata {
                original_byte_len: evidence.len() as u64, window_start: 0, window_len: evidence.len() as u64, truncated: false,
            } }]).unwrap();
        views.insert(member.clone(), view);
    }
    CommitteeInput::capture(action, &contracts, views).unwrap()
}
pub fn window(host: &FileOversight) -> ReviewWindow {
    let now = host.inspect().control.ledger.elapsed.unwrap().0;
    ReviewWindow { commit_by: ElapsedTick(now + 5), reveal_by: ElapsedTick(now + 10) }
}
pub fn salt(member: &str) -> Vec<u8> { format!("original-{member}").into_bytes() }
pub fn commit(host: &mut FileOversight, round: u64, member: &str, verdict: Verdict) {
    let digest = commitment(round, member, &ROOT, verdict, &salt(member)).unwrap();
    host.commit_review(host.revision(), round, member, digest).unwrap();
}
pub fn votes(host: &mut FileOversight, round: u64, verdict: Verdict) {
    for member in MEMBERS { commit(host, round, member, verdict); }
    host.open_reveals(host.revision(), round).unwrap();
    for member in MEMBERS { host.reveal_review(host.revision(), round, member, verdict, salt(member)).unwrap(); }
}
pub fn review_existing(host: &mut FileOversight, id: u64, round: u64, inputs: &CommitteeInput) -> ObservedReceipt {
    host.record_inputs(host.revision(), id, host.input_revision(id).unwrap(), inputs.clone()).unwrap();
    host.begin_review(host.revision(), id, round, ROOT, window(host), snapshot()).unwrap();
    votes(host, round, Verdict::Allow);
    host.finish_review(host.revision(), round, Some(inputs), snapshot()).unwrap().unwrap()
}
pub fn reviewed(host: &mut FileOversight, id: u64, payload: &[u8]) -> (FrozenAction, CommitteeInput) {
    let action = host.propose(host.revision(), id, spec(host, payload), snapshot()).unwrap();
    let inputs = inputs(&action, b"complete source view");
    review_existing(host, id, id + 100, &inputs);
    (action, inputs)
}
#[derive(Debug)]
pub struct Keys {
    pub action: FrozenAction,
    pub inputs: CommitteeInput,
    pub automatic: FilePermit,
    pub human: FileHumanPermit,
    pub request: FileHumanRequest,
}
pub fn ready(host: &mut FileOversight, reviewer: &FileHumanReviewer, id: u64, payload: &[u8]) -> Keys {
    let (action, inputs) = reviewed(host, id, payload);
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(now.0 + 30)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human, request }
}
pub fn dispatch(host: &mut FileOversight, keys: &Keys) {
    host.dispatch(host.revision(), &keys.automatic, &keys.human, &keys.action, &keys.inputs, snapshot()).unwrap();
}
