#![allow(dead_code)]
#[path = "file_oversight.rs"] pub mod ordinary;
pub use ordinary::{Directory, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{FileActorPort, FileActorSupervisor};
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::oversight::actor_process::{ActorProcess, ActorProgram};
use fa_reference::action::consequence::oversight::actor_transport::DriveBudget;
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, ChannelLimits};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::time::{Duration, Instant};

pub type Port = FileActorPort<FileOversight>;
pub type Process = ActorProcess<Port>;
pub fn proposal() -> ActorProposal {
    ActorProposal { target: profile().delivery.target, payload: b"actor request".to_vec(),
        expected_policy_epoch: 0, deadline: ElapsedTick(100), units: 16 }
}
pub fn program(mode: &str) -> ActorProgram {
    ActorProgram::new(std::env::current_exe().unwrap(), std::env::current_dir().unwrap(),
        vec!["--exact".into(), "actor_child_fixture".into(), "--nocapture".into()],
        BTreeMap::from([(OsString::from("FA_ACTOR_CHILD"), OsString::from(mode))])).unwrap()
}
pub fn setup(root: &Directory, mode: &str)
    -> (Port, FileActorSupervisor<FileOversight>, FileHumanReviewer, Process)
{
    let (mut host, human) = ordinary::create(root);
    host.enable_publication_guard(host.revision()).unwrap();
    let (port, mut supervisor) = host.into_actor_gateway();
    let rev = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(rev, Some(snapshot())).unwrap();
    let process = Process::launch(&program(mode), ActorWire::new(port.clone()), ChannelLimits::default()).unwrap();
    (port, supervisor, human, process)
}
pub fn submitted(process: &mut Process, supervisor: &FileActorSupervisor<FileOversight>) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        assert!(Instant::now() < until, "actor did not submit: {:?}", process.status());
        process.drive(DriveBudget::default()).unwrap();
        if supervisor.host().unwrap().request_status(9000).is_ok() { return; }
        std::thread::sleep(Duration::from_millis(1));
    }
}
pub fn reap<P: fa_reference::action::consequence::oversight::actor_wire::ActorRequestPort>(actor: &mut ActorProcess<P>) {
    let until = Instant::now() + Duration::from_secs(20);
    while actor.poll().child.exit.is_none() {
        assert!(Instant::now() < until, "actor not reaped: {:?}", actor.status());
        std::thread::sleep(Duration::from_millis(1));
    }
}
pub fn approve(host: &mut FileOversight, human: &FileHumanReviewer) -> ordinary::Keys {
    let action = host.request_action(9000).unwrap().clone();
    let input = ordinary::inputs(&action, b"whole input review of a real process request");
    ordinary::review_existing(host, 1, 101, &input);
    let automatic = host.authorize(host.revision(), 1, &input, snapshot()).unwrap();
    let request = host.request_human_approval(host.revision(), 1001, 1, &input, ElapsedTick(30)).unwrap();
    let rev = host.revision(); let key = human.approve(host, rev, &request).unwrap();
    ordinary::Keys { action, inputs: input, automatic, human: key, request }
}
