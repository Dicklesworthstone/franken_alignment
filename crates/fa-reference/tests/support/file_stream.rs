#![allow(dead_code)]
#[path = "file_oversight.rs"] mod oversight;
pub use oversight::{Directory, Keys, snapshot, inputs, review_existing};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile, FileHumanReviewer};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};

pub fn profile() -> FileOversightProfile {
    let mut profile = oversight::profile();
    profile.delivery.initial_payload.clear();
    profile.delivery.total = 4096;
    profile.delivery.policy = Policy::new(1, vec![
        Predicate::ExactValue { key: 7, value: b"ok".to_vec() },
        Predicate::PayloadAtMost(1024), Predicate::All(vec![0, 1]),
    ]).unwrap();
    profile
}
pub fn stream() -> StreamProfile { StreamProfile::new(91, 3, 4, 16, 64).unwrap() }
pub fn create(root: &Directory) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create_stream(root.store(), profile(), stream()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
pub fn ready(host: &mut FileOversight, reviewer: &FileHumanReviewer, id: u64, message: Option<&str>) -> Keys {
    let now = host.inspect().control.ledger.elapsed.unwrap().0;
    let spec = match message {
        Some(message) => host.stream_message_spec(message, ElapsedTick(now + 100)),
        None => host.stream_finish_spec(ElapsedTick(now + 100)),
    }.unwrap();
    let action = host.propose(host.revision(), id, spec, snapshot()).unwrap();
    let inputs = inputs(&action, b"complete source view");
    review_existing(host, id, id + 100, &inputs);
    let automatic = host.authorize(host.revision(), id, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), id + 1000, id, &inputs, ElapsedTick(now + 30)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human, request }
}
pub fn dispatch(host: &mut FileOversight, keys: &Keys) { oversight::dispatch(host, keys); }
pub fn publish(host: &mut FileOversight, keys: &Keys) {
    let now = host.inspect().control.ledger.elapsed.unwrap();
    let publication = host.publish_checked(host.revision(), keys.automatic.attempt(), Some(&keys.inputs), snapshot(), now).unwrap();
    assert!(matches!(publication.outcome, fa_reference::action::consequence::delivery::EndpointOutcome::Executed { .. }));
}
