//! Kill only an owned child, then reopen without any surviving authority object.
#![cfg(unix)]
#[path = "support/file_oversight_actor.rs"]
mod support;
use support::*;
use fa_reference::action::{ElapsedTick, ActionState};
use fa_reference::action::consequence::delivery::{EndpointOutcome, NonExecutionReason};
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, Command, encode_command};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command as Process};
use std::time::{Duration, Instant};

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            if let Err(error) = self.0.kill() { eprintln!("owned child kill: {error}"); }
            if let Err(error) = self.0.wait() { eprintln!("owned child wait: {error}"); }
        }
    }
}
fn original() -> ActorProposal {
    ActorProposal { target: profile().delivery.target, payload: b"published".to_vec(),
        units: 16, deadline: ElapsedTick(101), expected_policy_epoch: 0 }
}

/// Subprocess entry point, not an independent crash-recovery scenario.
#[test]
fn file_oversight_actor_child() {
    let Some(path) = std::env::var_os("FA_OVERSIGHT_CHILD_STORE") else { return; };
    let stage = std::env::var("FA_OVERSIGHT_CHILD_STAGE").unwrap().parse::<u8>().unwrap();
    assert!(stage <= 3);
    let ready_path = PathBuf::from(std::env::var_os("FA_OVERSIGHT_CHILD_READY").unwrap());
    let (mut host, reviewer) = FileOversight::create(PathBuf::from(path), profile()).unwrap();
    let revision = host.revision(); host.observe_time(revision, ElapsedTick(1)).unwrap();
    let (port, mut supervisor) = host.into_actor_gateway(); observe(&mut supervisor);
    let mut wire = ActorWire::new(port);
    assert!(matches!(wire.exchange(&submit_bytes(99, &original())).result, Ok(Knowledge::Pending { .. })));
    let keys = if stage > 0 { Some(ready(&mut supervisor, &reviewer, 99)) } else { None };
    if stage >= 2 {
        let mut host = supervisor.host_mut().unwrap(); let keys = keys.as_ref().unwrap();
        oversight::dispatch(&mut host, keys);
        if stage == 3 { let revision = host.revision(); host.publish(revision, keys.automatic.attempt()).unwrap(); }
    }
    let pending = ready_path.with_extension("pending");
    fs::write(&pending, format!("{} {stage}", std::process::id())).unwrap();
    fs::rename(pending, ready_path).unwrap();
    std::thread::sleep(Duration::from_secs(30));
    panic!("parent did not terminate its ready child");
}

#[test]
fn process_death_at_four_two_key_lifecycle_stages_never_resurrects_or_duplicates_an_effect() {
    for stage in 0..=3 {
        let root = Directory::new(); let marker = root.0.join("child.ready");
        let mut child = OwnedChild(Process::new(std::env::current_exe().unwrap())
            .args(["--exact", "file_oversight_actor_child", "--nocapture"])
            .env("FA_OVERSIGHT_CHILD_STORE", root.store())
            .env("FA_OVERSIGHT_CHILD_STAGE", stage.to_string())
            .env("FA_OVERSIGHT_CHILD_READY", &marker).spawn().unwrap());
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match fs::read_to_string(&marker) {
                Ok(value) => { assert_eq!(value, format!("{} {stage}", child.0.id())); break; }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
                Err(error) => panic!("readiness failed: {error}"),
            }
            assert!(child.0.try_wait().unwrap().is_none(), "child exited before its durable boundary");
            assert!(Instant::now() < deadline, "child failed to reach durable stage {stage}");
            std::thread::sleep(Duration::from_millis(1));
        }
        child.0.kill().unwrap(); assert!(!child.0.wait().unwrap().success());
        let (host, reviewer) = FileOversight::open(root.store(), profile()).unwrap(); drop(reviewer);
        let (port, mut supervisor) = host.into_actor_gateway(); let mut wire = ActorWire::new(port);
        assert!(matches!(wire.exchange(&encode_command(&Command::Poll { request: 99 }).unwrap()).result,
            Ok(Knowledge::Withheld { .. })));
        let revision = supervisor.host().unwrap().revision();
        let retried = wire.exchange(&submit_bytes(99, &original()));
        if stage < 2 {
            assert!(matches!(retried.result, Ok(Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. })));
        } else { assert!(matches!(retried.result, Ok(Knowledge::Unknown { .. }))); }
        assert_eq!(supervisor.host().unwrap().revision(), revision);
        assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
        assert!(!supervisor.host().unwrap().clock_ready());
        {
            let mut host = supervisor.host_mut().unwrap(); let id = attempt(&host, 99);
            let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
            if stage >= 2 {
                let revision = host.revision(); assert!(host.publish(revision, id).is_err());
                let revision = host.revision(); let result = host.reconcile(revision, id).unwrap();
                if stage == 3 {
                    assert_eq!(result, Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 }));
                } else {
                    assert_eq!(result, Reconciliation::AwaitingResolution);
                    assert_eq!(host.inspect().control.ledger.charged, 16);
                    let revision = host.revision();
                    assert_eq!(host.seal_unexecuted(revision, id).unwrap(),
                        Reconciliation::Resolved(EndpointOutcome::NotExecuted { reason: NonExecutionReason::Sealed }));
                }
            }
            assert_eq!(host.inspect().executions, u64::from(stage == 3));
            assert_eq!(host.inspect().control.ledger.available, if stage == 3 { 84 } else { 100 });
            assert_eq!(host.inspect().control.ledger.stages[&id],
                if stage == 3 { ActionState::Confirmed } else if stage == 2 { ActionState::ConfirmedNotExecuted } else { ActionState::Cancelled });
        }
    }
}
