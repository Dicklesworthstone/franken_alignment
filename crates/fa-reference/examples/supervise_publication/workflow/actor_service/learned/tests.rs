//! Real external intake, original numerical/sidecar owners and durable effects.
//! No listener is needed by the direct owner tests. Transport remains separately
//! exercised by its real peer test; unavailable sockets are not skipped.
use super::*;
#[path = "tests/models.rs"] mod models;
#[path = "tests/fixture.rs"] mod fixture;
#[path = "tests/direct.rs"] mod direct;
#[path = "tests/admission.rs"] mod admission;
#[path = "tests/transport.rs"] mod transport;
use fixture::{Root, configured, evidence};
use fa_reference::action::consequence::delivery::persistent::observed::{
    helpers::learned::native::{NativeReviewStatus, NativeMemberRecord, sequence::FileNativeSidecarSequence},
};
use fa_reference::action::consequence::oversight::sidecar::receiver::native::SidecarDecisionBasis;
use std::fs;

type Wire = ActorWire<FileLearnedTextStreamActorPort>;
// Independent wire-size bound, without relying on any successful BPE merges:
// one current K/V position, two 192-byte descriptors, and two rank-1 groups
// produce a 554-byte learned image, a 660-byte checked base, and a 684-byte
// sidecar. The action frame is 187 bytes and the complete question is 38.
// Every input byte is retained; byte BPE can only reduce this token count.
const NATIVE_INPUT_BYTES: usize = 909;
// One intake, two probes, two output tokens, commitment, and reveal. Both
// members advance in each original poll, sharing one policy-file observation.
const NATIVE_POLL_OVERHEAD: usize = 7;
fn deadline() -> Deadline {
    Deadline { logical: ElapsedTick(61_000), started: Instant::now(), wall: Duration::from_secs(60) }
}
fn prepared(root: &Root, spelling: &[u8]) -> (Config, Loaded, FileHumanReviewer, FileSupervisedDriver, Wire, Deadline) {
    let mut config = configured(root);
    let path = fixture::write(root, spelling);
    evidence(root, &config, 1, true, false);
    let loaded = recipe::load(&path, &config, true).unwrap();
    let (host, reviewer) = recovery::create(&config, &loaded).unwrap();
    let (port, supervisor) = host.into_learned_text_stream_actor_gateway().unwrap();
    let mut driver = FileSupervisedDriver::new(supervisor);
    let mut wire = ActorWire::new(port);
    let deadline = deadline();
    let mut control = Control::new(&config, loaded.request, None).unwrap();
    assert!(generate(&mut driver, &reviewer, &mut config, &mut control,
        &deadline, &mut || ElapsedTick(1000)).unwrap());
    submit(&mut driver, &mut wire, &mut config, &loaded, &deadline, &mut || ElapsedTick(1000)).unwrap();
    let before = fs::read(config.store.join("delivery.bin")).unwrap();
    assert!(driver.request_learned_human_approval_from_policy_file(&mut config.source,
        700, ElapsedTick(50_000), || ElapsedTick(1000)).result.is_err());
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), before);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
    (config, loaded, reviewer, driver, wire, deadline)
}
fn native_records(review: &FileNativeSidecarSequence) -> impl Iterator<Item = &NativeMemberRecord> {
    review.records().values().flat_map(|round| round.members.values().filter_map(Option::as_ref))
}
fn assert_native(review: &FileNativeSidecarSequence) {
    assert_eq!(review.status(), NativeReviewStatus::Finished);
    assert_eq!(native_records(review).count(), 2);
    for view in review.input().views().values() {
        assert_eq!(view.actual_input().submitted_bytes().len(), NATIVE_INPUT_BYTES);
    }
    let prompt = native_records(review).map(|record|
        record.progress.native.requested_prompt_tokens).max().unwrap();
    assert!(prompt <= NATIVE_INPUT_BYTES);
    assert_eq!(review.polls(), prompt + NATIVE_POLL_OVERHEAD);
    assert!(review.polls() <= NATIVE_INPUT_BYTES + NATIVE_POLL_OVERHEAD);
    for record in native_records(review) {
        assert_eq!(record.progress.basis, Some(SidecarDecisionBasis::NativeModel));
        assert_eq!(record.progress.completed_probes, record.progress.declared_probes);
        assert_eq!(record.progress.completed_probes, 2);
        assert!(record.progress.native.requested_prompt_tokens > 0);
        assert_eq!(record.progress.native.reviewed_prompt_tokens, record.progress.native.requested_prompt_tokens);
        assert_eq!(record.progress.native.work.sampled_draws, 2);
        assert_eq!(record.progress.native.work.decoder.tokens,
            record.progress.native.requested_prompt_tokens as u64 + 2);
        assert!(record.commitment_queued && record.reveal_queued);
        assert!(!record.interrupted && record.failure.is_none());
    }
}
fn response(wire: &mut Wire, request: u64) -> fa_reference::action::consequence::oversight::actor_wire::WireResponse {
    wire.exchange(&encode_command(&Command::Poll { request }).unwrap())
}
fn executed(wire: &mut Wire, request: u64) -> bool {
    matches!(response(wire, request).result, Ok(Knowledge::Known { value: ActorOutcome::Executed, .. }))
}
