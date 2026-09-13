#![allow(dead_code)]
#[path = "file_helper.rs"] pub mod helper;
pub use helper::{Directory, Clients, ROOT, MEMBERS, profile, snapshot, sockets, worker_steps};
use fa_reference::action::consequence::delivery::persistent::observed::{FileOversight, FileHumanPermit, FileHumanReviewer};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileSupervisedDriver, FileDriverLaunch, FileDriverEvent};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{FileActorPort, FileActorTicket};
use fa_reference::action::consequence::oversight::{CommitteeInput, ObservedReceipt};
use fa_reference::action::consequence::oversight::actor::ActorProposal;
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::action::ElapsedTick;
use fa_reference::round::Verdict;
use fa_reference::Error;

pub struct Rig {
    pub root: Directory,
    pub port: FileActorPort<FileOversight>,
    pub driver: FileSupervisedDriver,
    pub reviewer: FileHumanReviewer,
    pub clients: Clients,
    pub inputs: Option<CommitteeInput>,
}
impl Rig {
    pub fn new() -> Self {
        let root = Directory::new();
        let (host, reviewer) = helper::create(&root);
        let (port, driver) = host.into_supervised_driver();
        Self { root, port, driver, reviewer, clients: Clients::new(), inputs: None }
    }
    pub fn proposal(&self) -> ActorProposal {
        let host = self.driver.supervisor().host().unwrap();
        let spec = helper::spec(&host, b"publication");
        ActorProposal { target: spec.target.unwrap(), payload: spec.payload, units: spec.units,
            deadline: spec.deadline, expected_policy_epoch: spec.policy_epoch }
    }
    pub fn submit(&mut self, request: u64) -> FileActorTicket<FileOversight> {
        let proposal = self.proposal();
        let revision = self.driver.supervisor().host().unwrap().revision();
        self.driver.supervisor_mut().set_snapshot(revision, Some(snapshot())).unwrap();
        self.port.submit(request, &proposal).unwrap()
    }
    pub fn launch(&mut self, request: u64, round: u64) -> FileDriverLaunch {
        let host = self.driver.supervisor().host().unwrap();
        let action = host.request_action(request).unwrap();
        let inputs = helper::inputs(action, b"complete evidence used by the real socket workers");
        let attempt = match host.request_status(request).unwrap().disposition {
            fa_reference::action::consequence::delivery::persistent::requests::FileRequestDisposition::Admitted { attempt, .. } => attempt,
            _ => panic!("fixture request refused"),
        };
        let (workers, clients) = sockets(action.spec().policy_epoch);
        self.clients = clients;
        self.inputs = Some(inputs.clone());
        FileDriverLaunch { request, round, evidence_root: ROOT, window: helper::window(&host),
            expected_input_revision: host.input_revision(attempt).unwrap(), inputs,
            workers, limits: HelperLimits::default() }
    }
    pub fn start(&mut self, request: u64, round: u64) {
        let launch = self.launch(request, round);
        let now = self.driver.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
        self.driver.start_review(launch, snapshot(), || now).unwrap();
    }
    pub fn review_event(&mut self, verdict: Verdict, unavailable: bool) -> FileDriverEvent {
        let input = self.inputs.clone();
        let now = self.driver.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
        for _ in 0..128 {
            let event = self.driver.step_with_evidence(|| now, |_, _| {
                if unavailable { Err(Error::Incomplete) }
                else { Ok(DriverEvidence { snapshot: snapshot(), inputs: input.clone() }) }
            }, None).unwrap();
            if matches!(event, FileDriverEvent::Workers { .. }) { worker_steps(&mut self.clients, verdict); }
            else { return event; }
        }
        panic!("bounded original socket review did not finish");
    }
    pub fn reviewed(&mut self, request: u64) -> ObservedReceipt {
        self.start(request, request + 100);
        match self.review_event(Verdict::Allow, false) {
            FileDriverEvent::ReviewApplied { receipt, .. } => *receipt,
            event => panic!("unexpected fixture review: {event:?}"),
        }
    }
    pub fn human(&mut self, key: u64, expires: u64) -> FileHumanPermit {
        let input = self.inputs.as_ref().unwrap();
        let now = self.driver.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
        let request = self.driver.request_human_approval(key, input, ElapsedTick(expires), now).unwrap();
        let mut host = self.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        self.reviewer.approve(&mut host, revision, &request).unwrap()
    }
    pub fn step(&mut self, human: Option<&FileHumanPermit>) -> FileDriverEvent {
        let input = self.inputs.clone();
        let now = self.driver.supervisor().host().unwrap().inspect().control.ledger.elapsed.unwrap();
        self.driver.step_with_evidence(|| now,
            |_, _| Ok(DriverEvidence { snapshot: snapshot(), inputs: input.clone() }), human).unwrap()
    }
}
