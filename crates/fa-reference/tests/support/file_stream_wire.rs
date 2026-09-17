#![allow(dead_code)]
#[path = "file_oversight.rs"] pub mod ordinary;
pub use ordinary::{Directory, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileOversightProfile, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::stream::{FileStreamProposal, actor_wire::{FileStreamActorPort, encode_stream_proposal}};
use fa_reference::action::consequence::delivery::persistent::requests::actor::FileActorSupervisor;
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};

pub fn stream() -> StreamProfile { StreamProfile::new(7, 1, 4, 1024, 4096).unwrap() }
pub fn profile() -> FileOversightProfile {
    let mut p = ordinary::profile(); p.delivery.total = 4096; p.delivery.initial_payload.clear(); p
}
pub fn create(root: &Directory) -> (FileStreamActorPort, FileActorSupervisor<FileOversight>, FileHumanReviewer) {
    let (mut h, human) = FileOversight::create_stream(root.store(), profile(), stream()).unwrap();
    h.observe_time(h.revision(), ElapsedTick(1)).unwrap();
    let (actor, supervisor) = h.into_stream_actor_gateway().unwrap();
    (actor, supervisor, human)
}
pub fn command(h: &FileOversight, request: u64, message: Option<&str>) -> Command {
    let state = h.inspect();
    let intent = FileStreamProposal { target: state.target, expected_policy_epoch: state.control.ledger.epoch,
        deadline: ElapsedTick(state.control.ledger.elapsed.unwrap().0 + 100), message: message.map(str::to_owned) };
    Command::Submit { request, proposal: encode_stream_proposal(stream(), &intent).unwrap() }
}
pub fn frame(command: &Command) -> Vec<u8> {
    let mut bytes = encode_command(command).unwrap(); bytes.push(b'\n'); bytes
}
pub fn admit(supervisor: &mut FileActorSupervisor<FileOversight>) {
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
}
pub fn approve(h: &mut FileOversight, human: &FileHumanReviewer, request: u64, attempt: u64) -> ordinary::Keys {
    let action = h.request_action(request).unwrap().clone();
    let inputs = ordinary::inputs(&action, b"unaltered full stream evidence");
    ordinary::review_existing(h, attempt, attempt + 100, &inputs);
    let revision = h.revision();
    let automatic = h.authorize(revision, attempt, &inputs, snapshot()).unwrap();
    let revision = h.revision();
    let request = h.request_human_approval(revision, attempt + 1000, attempt, &inputs,
        ElapsedTick(h.inspect().control.ledger.elapsed.unwrap().0 + 30)).unwrap();
    let revision = h.revision(); let human = human.approve(h, revision, &request).unwrap();
    ordinary::Keys { action, inputs, automatic, human, request }
}
