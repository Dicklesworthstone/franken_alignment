#![allow(dead_code)]
#[path = "file_oversight.rs"]
pub mod oversight;
pub use oversight::{Directory, snapshot, profile};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::requests::{FileRequestDisposition, actor::{FileActorPort, FileActorSupervisor}};
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::oversight::actor_wire::{Command, encode_command};
use fa_reference::action::{ActionSpec, ElapsedTick};

pub type Port = FileActorPort<FileOversight>;
pub type Supervisor = FileActorSupervisor<FileOversight>;

pub fn proposal(spec: &ActionSpec) -> ActorProposal {
    ActorProposal { target: spec.target.unwrap(), payload: spec.payload.clone(), units: spec.units,
        deadline: spec.deadline, expected_policy_epoch: spec.policy_epoch }
}
pub fn create(root: &Directory) -> (Port, Supervisor, FileHumanReviewer, ActorProposal) {
    let (host, reviewer) = oversight::create(root);
    let proposal = proposal(&oversight::spec(&host, b"published"));
    let (port, supervisor) = host.into_actor_gateway();
    (port, supervisor, reviewer, proposal)
}
pub fn observe(supervisor: &mut Supervisor) {
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
}
pub fn submit_bytes(request: u64, proposal: &ActorProposal) -> Vec<u8> {
    encode_command(&Command::Submit { request, proposal: proposal.clone() }).unwrap()
}
pub fn attempt(host: &FileOversight, request: u64) -> u64 {
    match host.request_status(request).unwrap().disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("request not admitted: {other:?}"),
    }
}
pub fn ready(supervisor: &mut Supervisor, reviewer: &FileHumanReviewer, request: u64) -> oversight::Keys {
    let mut host = supervisor.host_mut().unwrap();
    let id = attempt(&host, request);
    let action = host.request_action(request).unwrap().clone();
    let inputs = oversight::inputs(&action, b"PRIVATE-SOURCE-BYTES");
    oversight::review_existing(&mut host, id, request + 100, &inputs);
    let revision = host.revision();
    let automatic = host.authorize(revision, id, &inputs, snapshot()).unwrap();
    let revision = host.revision();
    let request = host.request_human_approval(revision, request + 1000, id, &inputs, ElapsedTick(31)).unwrap();
    let revision = host.revision();
    let human = reviewer.approve(&mut host, revision, &request).unwrap();
    oversight::Keys { action, inputs, automatic, human, request }
}
