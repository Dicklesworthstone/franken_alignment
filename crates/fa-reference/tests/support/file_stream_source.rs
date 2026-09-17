#![allow(dead_code)]
#[path = "file_stream_wire.rs"] pub mod stream;
pub use stream::{Directory, ordinary, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileSupervisedDriver;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::FileStreamActorPort;
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};

pub fn create(root: &Directory) -> (FileStreamActorPort, FileSupervisedDriver, FileHumanReviewer, FileEvidenceSource, EvidenceSnapshot) {
    let p = profile();
    let (mut h, human) = FileOversight::create_stream(root.store(), p.clone(), stream::stream()).unwrap();
    h.enable_file_source(h.revision(), FileSourcePolicy {
        source: StateSource { scope: p.delivery.scope, source: 17, generation: 1 },
        limits: StateLimits::default(), freshness: StateFreshness::new(50).unwrap(),
    }).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: 17, generation: 1, scope: p.delivery.scope },
        snapshot(), ordinary::MEMBERS.into_iter().map(|member| (member.to_owned(), b"actual source context".to_vec())).collect()).unwrap();
    let path = root.0.join("private-evidence.json"); std::fs::write(&path, evidence.encode()).unwrap();
    let source = FileEvidenceSource::new(path, 17, p.delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    let (port, supervisor) = h.into_stream_actor_gateway().unwrap();
    (port, FileSupervisedDriver::new(supervisor), human, source, evidence)
}
pub fn review_dispatch(driver: &mut FileSupervisedDriver, human: &FileHumanReviewer,
    evidence: &EvidenceSnapshot, request: u64, attempt: u64) -> ordinary::Keys
{
    let mut h = driver.supervisor_mut().host_mut().unwrap();
    let action = h.request_action(request).unwrap().clone();
    let inputs = evidence.inputs_for(&action, &profile().committee).unwrap();
    ordinary::review_existing(&mut h, attempt, attempt + 100, &inputs);
    let revision = h.revision();
    let automatic = h.authorize(revision, attempt, &inputs, snapshot()).unwrap();
    let revision = h.revision();
    let request = h.request_human_approval(revision, attempt + 1000, attempt, &inputs, ElapsedTick(30)).unwrap();
    let revision = h.revision(); let human = human.approve(&mut h, revision, &request).unwrap();
    let keys = ordinary::Keys { action, inputs, automatic, human, request };
    ordinary::dispatch(&mut h, &keys); keys
}
