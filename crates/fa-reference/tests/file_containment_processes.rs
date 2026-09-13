//! Actual authority-process death after a durable reset, not a power-loss claim.
#![cfg(unix)]
#[path = "support/file_oversight.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::containment::{FileResetRequest, FileStateUpdate};
use fa_reference::action::consequence::gate::ReviewBinding;
use fa_reference::action::consequence::gate::containment::ActorState;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CHILD: &str = "durable_containment_child";
const PATH: &str = "FA_CONTAINMENT_CHILD_STORE";
const MARKER: &str = "FA_CONTAINMENT_CHILD_MARKER";
const EXECUTED: &str = "FA_CONTAINMENT_CHILD_EXECUTED";
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            if let Err(error) = self.0.kill() { eprintln!("containment child kill: {error}"); }
            if let Err(error) = self.0.wait() { eprintln!("containment child wait: {error}"); }
        }
    }
}

/// The test executable doubles as an explicit fixture process. Normal test
/// discovery does nothing here; the parent opts in through the exact environment.
#[test]
fn durable_containment_child() {
    let Some(path) = std::env::var_os(PATH) else { return; };
    let marker = PathBuf::from(std::env::var_os(MARKER).unwrap());
    let executed = std::env::var(EXECUTED).unwrap() == "yes";
    let (mut host, reviewer) = FileOversight::create(path, profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let saved = host.capture_actor_checkpoint(host.revision(), 77, 0, 0).unwrap();
    let keys = ready(&mut host, &reviewer, 1, b"old effect"); dispatch(&mut host, &keys);
    if executed { host.publish(host.revision(), 1).unwrap(); }
    let actor = host.actor_snapshot().unwrap();
    let update = FileStateUpdate { operation: 1, expected_actor_revision: actor.actor_revision,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        state: ActorState::new(actor.state.profile(), vec![9, 8], vec![3, 2], vec![255, 0], 2).unwrap() };
    host.record_actor_state(host.revision(), update).unwrap();
    let request = FileResetRequest { operation: 1, expected_control_sequence: host.inspect().control.sequence,
        expected_actor_revision: host.actor_snapshot().unwrap().actor_revision,
        expected_authority_epoch: host.inspect().control.ledger.epoch,
        binding: ReviewBinding { round: 10001, evidence_root: ROOT, reducer_generation: 1 },
        retained_targets: vec![host.inspect().target] };
    host.reset_actor(host.revision(), &saved, request.clone()).unwrap();
    let text = format!("{} {} {} {} {}", std::process::id(), request.expected_control_sequence,
        request.expected_actor_revision, request.expected_authority_epoch,
        request.retained_targets[0].expected_version);
    let pending = marker.with_extension("pending"); fs::write(&pending, text).unwrap(); fs::rename(pending, &marker).unwrap();
    // Keep the actual locks/controller/key objects live until killed. There is
    // no clean release or in-process transfer masquerading as process failure.
    std::thread::sleep(Duration::from_secs(30));
    panic!("parent did not terminate the ready authority process");
}

#[test]
fn killed_authority_recovers_actor_memory_and_incident_but_never_replays_an_effect() {
    for executed in [false, true] {
        let root = Directory::new(); let marker = root.0.join("authority.ready");
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD, "--nocapture"])
            .env(PATH, root.store()).env(MARKER, &marker)
            .env(EXECUTED, if executed { "yes" } else { "no" })
            .stdin(Stdio::null()).stdout(Stdio::inherit()).stderr(Stdio::inherit()).spawn().unwrap();
        let mut child = OwnedChild(child); let end = Instant::now() + Duration::from_secs(15);
        let observed = loop {
            match fs::read_to_string(&marker) {
                Ok(text) => break text,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => panic!("readiness failure: {error}"),
            }
            assert!(child.0.try_wait().unwrap().is_none(), "authority exited before reset publication");
            assert!(Instant::now() < end, "authority did not publish its reset marker");
            std::thread::sleep(Duration::from_millis(1));
        };
        let words: Vec<u64> = observed.split_whitespace().map(|word| word.parse().unwrap()).collect();
        assert_eq!(words.len(), 5); assert_eq!(words[0], u64::from(child.0.id()));
        child.0.kill().unwrap(); let status = child.0.wait().unwrap(); assert!(!status.success());
        let (mut host, reviewer) = FileOversight::open(root.store(), profile()).unwrap();
        let actor = host.actor_snapshot().unwrap();
        assert_eq!(actor.state, profile().delivery.actor); assert_eq!(actor.incident_count, 1);
        assert_eq!(actor.actor_revision, words[2] + 1);
        assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Unknown);
        assert_eq!(host.inspect().control.ledger.charged, 16); assert_eq!(host.inspect().control.ledger.reserved, 0);
        assert_eq!(host.inspect().executions, u64::from(executed));
        let saved = host.actor_checkpoint(77).unwrap(); let mut target = profile().delivery.target;
        target.expected_version = words[4];
        let request = FileResetRequest { operation: 1, expected_control_sequence: words[1],
            expected_actor_revision: words[2], expected_authority_epoch: words[3],
            binding: ReviewBinding { round: 10001, evidence_root: ROOT, reducer_generation: 1 }, retained_targets: vec![target] };
        let revision = host.revision(); let receipt = host.reset_actor(0, &saved, request).unwrap();
        assert_eq!(receipt.incident_count, 1); assert_eq!(host.revision(), revision);
        host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
        let outcome = host.reconcile(host.revision(), 1).unwrap();
        assert_eq!(outcome, if executed { Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }) }
            else { Reconciliation::AwaitingResolution });
        assert_eq!(host.inspect().control.ledger.charged, 16);
        assert!(host.publish(host.revision(), 1).is_err());
        if !executed { host.seal_unexecuted(host.revision(), 1).unwrap(); }
        let keys = ready(&mut host, &reviewer, 2, b"fresh effect"); dispatch(&mut host, &keys);
        host.publish(host.revision(), 2).unwrap(); host.reconcile(host.revision(), 2).unwrap();
        assert_eq!(host.inspect().executions, 1 + u64::from(executed));
        let ledger = host.inspect().control.ledger;
        assert_eq!(ledger.available + ledger.reserved + ledger.charged, 100);
        let control = host.inspect().control;
        host.request_stop(host.revision(), StopRequest { operation: 1, expected_control_sequence: control.sequence,
            expected_authority_epoch: control.ledger.epoch }).unwrap();
        assert_eq!(host.actor_snapshot().unwrap().incident_count, 1);
    }
}
