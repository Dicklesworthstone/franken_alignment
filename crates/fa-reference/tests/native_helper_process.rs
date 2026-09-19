//! Actual binary and existing launcher, not an in-process verdict callback.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/native_worker_assets.rs"]
mod assets;
use assets::{Fixture, SALT, expected, frame, input};
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract};
use fa_reference::action::consequence::oversight::helper_processes::{
    HelperChildren, HelperProgram, ProcessExit, launch_helpers,
};
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn executable() -> PathBuf { PathBuf::from(env!("CARGO_BIN_EXE_fa-native-helper")) }
struct Children(HelperChildren);
impl Children {
    fn wait(&mut self) -> ProcessExit {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let statuses = self.0.reap();
            if self.0.all_reaped() { return statuses["native"].exit.unwrap(); }
            assert!(Instant::now() < deadline, "real native child did not exit within test bound");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for Children {
    fn drop(&mut self) {
        self.0.request_stop_all();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !self.0.all_reaped() && Instant::now() < deadline {
            self.0.reap(); std::thread::sleep(Duration::from_millis(1));
        }
    }
}
fn contracts() -> CommitteeContract {
    CommitteeContract::new(BTreeMap::from([("native".to_owned(),
        HelperContract::new(expected(), 7, b"?".to_vec()).unwrap())])).unwrap()
}
fn launch(fixture: &Fixture) -> (UnixStream, Children) {
    assert_eq!(fixture.policy.input_profile, expected());
    let program = HelperProgram::new(executable(), fixture.root.clone(),
        vec![fixture.path.clone().into_os_string()], BTreeMap::new()).unwrap();
    let (mut sockets, children) = launch_helpers(&contracts(),
        &BTreeMap::from([("native".to_owned(), program)])).unwrap();
    let socket = sockets.remove("native").unwrap();
    assert!(sockets.is_empty());
    (socket, Children(children))
}
fn blocking(socket: &UnixStream) {
    socket.set_nonblocking(false).unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    socket.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
}
fn ending(socket: &mut UnixStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    match socket.read_to_end(&mut bytes) {
        Ok(_) => {}
        // A child refusing startup may close with the request still unread.
        Err(e) if e.kind() == io::ErrorKind::ConnectionReset => {}
        Err(e) => panic!("native socket read: {e}"),
    }
    bytes
}

#[test]
fn real_checkpoint_child_answers_input_dependently_and_obeys_reveal_request() {
    let fixture = Fixture::new(false);
    for (prompt, verdict) in [(b"?".as_slice(), Verdict::Allow), (b"!", Verdict::Deny)] {
        let (mut socket, mut children) = launch(&fixture); blocking(&socket);
        socket.write_all(&frame(prompt)).unwrap();
        let mut commitment = [0; 9]; socket.read_exact(&mut commitment).unwrap();
        let original = input(prompt);
        assert_eq!(commitment, original.commitment_frame(verdict, SALT).unwrap());
        socket.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
        let mut byte = [0];
        assert!(matches!(socket.read(&mut byte).unwrap_err().kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut));
        blocking(&socket); socket.write_all(b"R").unwrap();
        assert_eq!(ending(&mut socket), original.reveal_frame(verdict, SALT).unwrap());
        assert!(children.wait().success);
    }
}

#[test]
fn held_checkpoint_child_emits_no_commitment_and_exits_without_a_default_vote() {
    let fixture = Fixture::new(true);
    let (mut socket, mut children) = launch(&fixture); blocking(&socket);
    socket.write_all(&frame(b"?")).unwrap();
    assert!(ending(&mut socket).is_empty());
    assert!(!children.wait().success);
}

#[test]
fn invalid_manifest_salt_or_asset_binding_fails_closed_in_the_actual_binary() {
    for mode in 0..4 {
        let mut fixture = Fixture::new(false);
        match mode {
            0 => { fixture.manifest = fixture.manifest.replace("\"stream\":12", "\"stream\":12,\"unknown\":true"); fixture.save(); }
            1 => std::fs::write(fixture.root.join("salt.bin"), b"short").unwrap(),
            2 => { fixture.manifest = fixture.manifest.replace("\"tokenizer_generation\":4", "\"tokenizer_generation\":5"); fixture.save(); }
            _ => std::fs::write(fixture.root.join("monitor.json"), b"{}").unwrap(),
        }
        let (mut socket, mut children) = launch(&fixture); blocking(&socket);
        // Startup must exit without consuming or emitting a protocol frame.
        assert!(ending(&mut socket).is_empty()); assert!(!children.wait().success);
    }
}

#[test]
fn missing_socket_or_extra_arguments_never_fall_back_to_stdio_text_inference() {
    for extra in [false, true] {
        let fixture = Fixture::new(false);
        let mut command = Command::new(executable());
        command.arg(&fixture.path).stdin(Stdio::null()).env_clear();
        if extra { command.arg("allow"); }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty()); assert!(!output.stderr.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains(std::str::from_utf8(SALT).unwrap()));
    }
}

#[test]
fn exhausted_process_steps_cannot_compute_or_send_a_replacement_answer() {
    let mut fixture = Fixture::new(false);
    fixture.manifest = fixture.manifest.replace("\"steps\":10000", "\"steps\":2"); fixture.save();
    let (mut socket, mut children) = launch(&fixture); blocking(&socket);
    socket.write_all(&frame(b"?")).unwrap();
    assert!(ending(&mut socket).is_empty()); assert!(!children.wait().success);
}

#[test]
fn actual_spawned_helper_feeds_original_congress_and_missing_native_work_cannot_authorize() {
    use fa_reference::action::{ActionSpec, ActionState, ElapsedTick, Purpose, ResolvedTarget, Scope, VERSION};
    use fa_reference::action::consequence::congress::{CongressPolicy, MemberPolicy};
    use fa_reference::action::consequence::delivery::PublicationEndpoint;
    use fa_reference::action::consequence::gate::TargetCeiling;
    use fa_reference::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
    use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    use fa_reference::action::consequence::gate::containment::session::policy::controller::ControllerConfig;
    use fa_reference::action::consequence::oversight::{CommitteeInput, OversightBroker, ReviewWindow, action_frame};
    use fa_reference::action::consequence::oversight::helper_workers::{HelperLimits, HelperRound};
    use fa_reference::action::consequence::oversight::helper_workers::io::HelperConnection;
    use fa_reference::evidence_view::{AuthorizationProjection, EvidenceViewManifest};
    use fa_reference::full_input::{ActualHelperInput, ByteSpan, PartKind, SubmittedPart};
    use fa_reference::reducer::Caps;
    use fa_reference::Snapshot;

    for alarm in [false, true] {
        let fixture = Fixture::new(alarm);
        let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
        let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
        let contracts = contracts();
        let config = ControllerConfig { scope, total: 100, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("native".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: TargetCeiling::new(&[target]).unwrap(),
        };
        let mut endpoint = PublicationEndpoint::new(target, b"initial".to_vec(), 1000, 8).unwrap();
        let mut broker = OversightBroker::new(config, &mut endpoint, contracts.clone()).unwrap();
        let fence = endpoint.install_fence(broker.fence_request()).unwrap(); broker.confirm_fence(fence).unwrap();
        broker.observe_time(ElapsedTick(1)).unwrap(); endpoint.observe_time(ElapsedTick(1)).unwrap();
        let snapshot = Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) };
        let action = broker.propose(1, ActionSpec { version: VERSION, scope, target: Some(target),
            payload: b"publish".to_vec(), required_witnesses: Vec::new(), policy_epoch: 0,
            deadline: ElapsedTick(100), units: 16 }, &snapshot).unwrap().action;
        let mut bytes = action_frame(&action); let split = bytes.len(); bytes.push(b'?'); let end = bytes.len();
        let actual = ActualHelperInput::new(bytes, expected(), vec![
            SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: split } },
            SubmittedPart { kind: PartKind::Question, span: ByteSpan { start: split, end } },
        ], Vec::new()).unwrap();
        let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
            projection_id: 7, policy_epoch: 0, projected_originals: Vec::new(),
        }, Vec::new()).unwrap();
        let inputs = CommitteeInput::capture(&action, &contracts, BTreeMap::from([("native".to_owned(), view)])).unwrap();
        broker.record_inputs(1, 0, inputs.clone()).unwrap();
        let session = broker.begin_review(1, 9, [7; 32], ReviewWindow {
            commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8),
        }, &snapshot).unwrap();
        let (mut round, mut ports) = HelperRound::new(session, HelperLimits::default()).unwrap();
        let (socket, mut children) = launch(&fixture);
        let mut connection = HelperConnection::new(ports.remove("native").unwrap(), socket).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let io = connection.step(); round.advance(ElapsedTick(1)).unwrap();
            if round.statuses()["native"].revealed || io.is_err() { break; }
            assert!(Instant::now() < end, "spawned native helper failed to finish");
            std::thread::sleep(Duration::from_millis(1));
        }
        if !alarm {
            assert!(round.statuses()["native"].revealed);
            let review = round.finish(ElapsedTick(1)).unwrap();
            broker.apply_review(review, Some(&inputs), &snapshot).unwrap();
            assert!(broker.authorize(1, Some(&inputs), &snapshot).is_ok());
            assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Authorized);
        } else {
            assert!(!round.statuses()["native"].committed);
            let review = round.finish(ElapsedTick(8)).unwrap(); broker.observe_time(ElapsedTick(8)).unwrap();
            broker.apply_review(review, Some(&inputs), &snapshot).unwrap();
            assert!(broker.authorize(1, Some(&inputs), &snapshot).is_err());
            assert_eq!(round.statuses().len(), 1); assert!(!round.statuses()["native"].revealed);
        }
        assert_eq!(children.wait().success, !alarm);
        assert_eq!(endpoint.execution_count(), 0); // judgment is not external publication
    }
}
