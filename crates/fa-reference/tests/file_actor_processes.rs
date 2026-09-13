//! Real process termination between durable admission, reservation and outcome.
#![cfg(unix)]
#[path = "support/file_delivery.rs"]
mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::*;
use fa_reference::action::consequence::delivery::persistent::requests::*;
use fa_reference::action::consequence::oversight::actor::{ActorOutcome, ActorProposal, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::actor_wire::{ActorWire, Command, encode_command};
use fa_reference::round::Verdict;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::time::{Duration, Instant};

fn proposal() -> ActorProposal {
    ActorProposal { target: profile().target, payload: b"payload".to_vec(), units: 16,
        deadline: ElapsedTick(100), expected_policy_epoch: 0 }
}
fn command() -> Vec<u8> { encode_command(&Command::Submit { request: 700, proposal: proposal() }).unwrap() }
fn id(status: FileRequestStatus) -> u64 {
    match status.disposition {
        FileRequestDisposition::Admitted { attempt, .. } => attempt,
        other => panic!("unexpected request: {other:?}"),
    }
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        match self.0.try_wait() {
            Ok(Some(_)) => {},
            _ => {
                if let Err(error) = self.0.kill() { eprintln!("request child kill: {error}"); }
                if let Err(error) = self.0.wait() { eprintln!("request child reap: {error}"); }
            }
        }
    }
}

/// Entry point used only by the owned subprocesses of the parent scenario.
#[test]
fn durable_actor_child() {
    let Some(path) = std::env::var_os("FA_ACTOR_RECOVERY_ROOT") else { return; };
    let root = PathBuf::from(path);
    let mode = std::env::var("FA_ACTOR_RECOVERY_STAGE").unwrap();
    let mut host = FileDelivery::create(root.join("publication"), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (port, mut supervisor) = host.into_actor_gateway();
    let revision = supervisor.host().unwrap().revision();
    supervisor.set_snapshot(revision, Some(snapshot())).unwrap();
    let mut wire = ActorWire::new(port);
    // The caller need not have received this response before the process dies.
    let response = wire.exchange(&command()); assert!(response.result.is_ok());
    let mut guard = supervisor.host_mut().unwrap(); let host = &mut *guard;
    let attempt = id(host.request_status(700).unwrap());
    match mode.as_str() {
        "submitted" => {},
        "reserved" | "dispatched" | "published" => {
            host.review(host.revision(), review(attempt, 100, Verdict::Allow)).unwrap();
            let key = host.authorize(host.revision(), attempt, snapshot()).unwrap();
            if mode != "reserved" {
                let action = host.request_action(700).unwrap().clone();
                host.dispatch(host.revision(), &key, &action, snapshot()).unwrap();
            }
            if mode == "published" { host.publish(host.revision(), attempt).unwrap(); }
        }
        _ => panic!("unknown fixture stage"),
    }
    let staging = root.join("ready.pending");
    fs::write(&staging, std::process::id().to_string()).unwrap();
    fs::rename(staging, root.join("ready")).unwrap();
    std::thread::sleep(Duration::from_secs(30));
    panic!("parent did not terminate the owned child");
}

#[test]
fn kill_reopen_and_exact_wire_retry_preserve_all_four_effect_stages() {
    for mode in ["submitted", "reserved", "dispatched", "published"] {
        let root = Directory::new();
        let child = ProcessCommand::new(std::env::current_exe().unwrap())
            .args(["--exact", "durable_actor_child", "--nocapture"])
            .env("FA_ACTOR_RECOVERY_ROOT", &root.0).env("FA_ACTOR_RECOVERY_STAGE", mode)
            .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit()).spawn().unwrap();
        let mut child = OwnedChild(child);
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match fs::read_to_string(root.0.join("ready")) {
                Ok(text) => { assert_eq!(text.parse::<u32>().unwrap(), child.0.id()); break; }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
                Err(error) => panic!("readiness failure: {error}"),
            }
            assert!(child.0.try_wait().unwrap().is_none(), "worker exited before durable stage");
            assert!(Instant::now() < deadline, "child readiness deadline");
            std::thread::sleep(Duration::from_millis(1));
        }
        child.0.kill().unwrap(); assert!(!child.0.wait().unwrap().success());
        let (port, mut supervisor) = FileDelivery::open(root.store(), profile()).unwrap().into_actor_gateway();
        let mut wire = ActorWire::new(port);
        let revision = supervisor.host().unwrap().revision();
        let result = wire.exchange(&command()).result.unwrap();
        assert_eq!(supervisor.host().unwrap().revision(), revision);
        assert_eq!(supervisor.host().unwrap().retained_requests(), 1);
        let expected = match mode {
            "submitted" | "reserved" => {
                assert!(matches!(result, Knowledge::Known { value: ActorOutcome::CancelledBeforeDispatch, .. }));
                ActorOutcome::CancelledBeforeDispatch
            }
            _ => {
                assert!(matches!(result, Knowledge::Unknown { reason: UnknownReason::OutcomeUnknown }));
                if mode == "published" { ActorOutcome::Executed } else { ActorOutcome::ConfirmedNotExecuted }
            }
        };
        {
            let mut guard = supervisor.host_mut().unwrap(); let host = &mut *guard;
            let attempt = id(host.request_status(700).unwrap());
            host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
            if mode == "published" { host.reconcile(host.revision(), attempt).unwrap(); }
            if mode == "dispatched" { host.seal_unexecuted(host.revision(), attempt).unwrap(); }
            assert_eq!(host.inspect().executions, u64::from(mode == "published"));
            assert_eq!(host.inspect().control.ledger.charged, if mode == "published" { 16 } else { 0 });
            assert_eq!(host.inspect().control.ledger.reserved, 0);
            assert!(host.publish(host.revision(), attempt).is_err());
        }
        let poll = encode_command(&Command::Poll { request: 700 }).unwrap();
        assert!(matches!(wire.exchange(&poll).result, Ok(Knowledge::Known { value, .. }) if value == expected));
        let revision = supervisor.host().unwrap().revision();
        wire.exchange(&command());
        assert_eq!(supervisor.host().unwrap().revision(), revision);
    }
}
