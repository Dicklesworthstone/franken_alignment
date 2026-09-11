//! Public-API fixtures: real sockets, deterministic fixture verdicts, no model claim.
use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
use fa_reference::action::consequence::delivery::PublicationEndpoint;
use fa_reference::action::consequence::gate::TargetCeiling;
use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
use fa_reference::action::consequence::oversight::{CommitteeContract, CommitteeInput, HelperContract, ReviewWindow, action_frame};
use fa_reference::action::consequence::oversight::actor::{ActorPort, ActorProposal, ActorTicket, IntakeLimits};
use fa_reference::action::consequence::oversight::helper_client::{ClientProgress, HelperClient};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::consequence::oversight::human::{HumanPermit, HumanReviewer, HumanReviewPolicy};
use fa_reference::action::consequence::oversight::supervised::{DriverEvent, ReviewLaunch, SupervisedDriver};
use fa_reference::action::{ElapsedTick, Purpose, ResolvedTarget, Scope};
use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
use fa_reference::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use fa_reference::reducer::Caps;
use fa_reference::round::Verdict;
use fa_reference::Snapshot;
use std::collections::BTreeMap;
use std::os::unix::net::UnixStream;

pub fn target() -> ResolvedTarget {
    ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }
}
pub fn snapshot() -> Snapshot {
    Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, vec![9])]) }
}
pub fn proposal() -> ActorProposal {
    ActorProposal { target: target(), payload: b"publish".to_vec(), units: 16,
        deadline: ElapsedTick(100), expected_policy_epoch: 0 }
}

pub struct Rig {
    pub port: ActorPort,
    pub driver: SupervisedDriver,
    pub inputs: Option<CommitteeInput>,
    pub clients: BTreeMap<String, HelperClient<UnixStream>>,
}
impl Rig {
    pub fn new(two_key: bool) -> (Self, Option<HumanReviewer>) {
        Self::with_endpoint(PublicationEndpoint::new(target(), b"old".to_vec(), 200, 16).unwrap(), two_key)
    }
    pub fn with_endpoint(endpoint: PublicationEndpoint, two_key: bool) -> (Self, Option<HumanReviewer>) {
        let contracts = CommitteeContract::new(["alice", "bob"].into_iter().map(|member| {
            (member.to_owned(), HelperContract::new(InputProfileBinding {
                profile_id: 1, profile_bytes: member.as_bytes().to_vec(), model_epoch: 1,
                tokenizer_epoch: 1, policy_epoch: 0,
            }, 9, b"Check the frozen effect".to_vec()).unwrap())
        }).collect()).unwrap();
        let config = ControllerConfig {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 100, max_attempts: 16,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: vec![9] }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([
                ("alice".to_owned(), MemberPolicy { cohort: "a".to_owned(), weight: 1 }),
                ("bob".to_owned(), MemberPolicy { cohort: "b".to_owned(), weight: 1 }),
            ]), caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 2,
                continue_hold_maximum: 0, narrow_at: 3, suspend_at: 4,
                minimum_members: 2, minimum_cohorts: 2 },
            narrowed_targets: TargetCeiling::new(&[target()]).unwrap(),
        };
        let (port, mut driver) = SupervisedDriver::new(config, endpoint, contracts, IntakeLimits::default()).unwrap();
        let reviewer = two_key.then(|| driver.supervisor_mut().broker_mut().enable_human_review(
            HumanReviewPolicy { reviewer_id: 90, max_validity_ticks: 50, max_requests: 16 },
        ).unwrap());
        driver.observe_time(ElapsedTick(1)).unwrap();
        driver.confirm_dispatcher_fence().unwrap();
        (Self { port, driver, inputs: None, clients: BTreeMap::new() }, reviewer)
    }
    pub fn accept(&mut self, request: u64) -> ActorTicket {
        let ticket = self.port.submit(request, &proposal()).unwrap();
        let intake = self.driver.accept_next(&snapshot()).unwrap().unwrap();
        assert_eq!(intake.request, request);
        assert!(intake.result.unwrap().is_some());
        ticket
    }
    pub fn launch(&mut self, request: u64, round: u64) -> ReviewLaunch {
        let supervisor = self.driver.supervisor();
        let action = supervisor.action(request).unwrap();
        let contracts = supervisor.broker().contracts();
        let mut views = BTreeMap::new();
        let mut streams = BTreeMap::new();
        self.clients.clear();
        for (member, helper) in contracts.members() {
            let mut bytes = action_frame(action);
            let boundary = bytes.len();
            bytes.extend_from_slice(helper.question());
            let actual = ActualHelperInput::new(bytes.clone(), helper.profile_at(action.spec().policy_epoch), vec![
                SubmittedPart { span: ByteSpan { start: 0, end: boundary }, kind: PartKind::Other },
                SubmittedPart { span: ByteSpan { start: boundary, end: bytes.len() }, kind: PartKind::Question },
            ], vec![]).unwrap();
            views.insert(member.clone(), EvidenceViewManifest::new(actual, AuthorizationProjection {
                projection_id: 9, policy_epoch: action.spec().policy_epoch, projected_originals: vec![],
            }, vec![]).unwrap());
            let (server, client) = UnixStream::pair().unwrap();
            streams.insert(member.clone(), server);
            self.clients.insert(member.clone(), HelperClient::from_unix(client, helper.profile_at(action.spec().policy_epoch)).unwrap());
        }
        let inputs = CommitteeInput::capture(action, contracts, views).unwrap();
        self.inputs = Some(inputs.clone());
        ReviewLaunch { request, round, evidence_root: [1; 32], window: ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(10),
        }, expected_input_revision: supervisor.broker().input_revision(supervisor.attempt(request).unwrap()).unwrap(),
            inputs, streams, limits: HelperLimits::default() }
    }
    pub fn start(&mut self, request: u64, round: u64) {
        let launch = self.launch(request, round);
        self.driver.start_review(launch, &snapshot()).unwrap();
    }
    pub fn tick(&mut self, now: u64, verdicts: [Verdict; 2], human: Option<&HumanPermit>) -> DriverEvent {
        for (index, (member, client)) in self.clients.iter_mut().enumerate() {
            if client.step().unwrap() == ClientProgress::NeedsInference {
                assert_eq!(client.input().unwrap().actual_input(), self.inputs.as_ref().unwrap().views()[member].actual_input());
                client.respond(verdicts[index], member.as_bytes()).unwrap();
            }
        }
        self.driver.step(ElapsedTick(now), self.inputs.as_ref(), &snapshot(), human).unwrap()
    }
    pub fn finish_review(&mut self, verdicts: [Verdict; 2]) -> DriverEvent {
        for _ in 0..128 {
            match self.tick(1, verdicts, None) {
                DriverEvent::Workers { .. } => {}
                result @ DriverEvent::ReviewApplied { .. } => return result,
                other => panic!("unexpected review event: {other:?}"),
            }
        }
        panic!("socket fixture did not complete its bounded review");
    }
}
