#![allow(dead_code)]
#[path = "file_oversight.rs"] mod ordinary;
pub use ordinary::{Directory, snapshot, inputs, review_existing, dispatch, Keys};
use fa_reference::action::{ElapsedTick, ActionSpec};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::*;

pub fn profile(domain: u64) -> FileOversightProfile {
    let mut profile = ordinary::profile();
    profile.delivery.scope.branch = domain;
    profile.delivery.clock_domain = 100 + domain;
    profile
}
pub fn create(root: &Directory, domain: u64) -> (FileOversight, FileHumanReviewer) {
    let (mut host, reviewer) = FileOversight::create(root.store(), profile(domain)).unwrap();
    host.enable_publication_guard(host.revision()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, reviewer)
}
pub fn spec(host: &FileOversight, domain: u64, bytes: &[u8]) -> ActionSpec {
    let mut spec = ordinary::spec(host, bytes);
    spec.scope = profile(domain).delivery.scope;
    spec
}
pub fn ready(host: &mut FileOversight, reviewer: &FileHumanReviewer, domain: u64,
    attempt: u64, bytes: &[u8]) -> Keys
{
    let action = host.propose(host.revision(), attempt, spec(host, domain, bytes), snapshot()).unwrap();
    ready_existing(host, reviewer, attempt, action)
}
pub fn ready_existing(host: &mut FileOversight, reviewer: &FileHumanReviewer, attempt: u64,
    action: fa_reference::action::FrozenAction) -> Keys
{
    let inputs = inputs(&action, b"unaltered full helper views");
    review_existing(host, attempt, attempt + 100, &inputs);
    let automatic = host.authorize(host.revision(), attempt, &inputs, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), attempt + 1000, attempt,
        &inputs, ElapsedTick(30)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(host, revision, &request).unwrap();
    Keys { action, inputs, automatic, human, request }
}
pub fn plan(domains: Vec<FileShutdownDomain>) -> FileShutdownPlan {
    FileShutdownPlan::new(900, domains, 64, MAX_SHUTDOWN_HEAD_BYTES).unwrap()
}
