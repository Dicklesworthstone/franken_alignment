//! Separate owners are forcibly killed only after their real journal operation
//! has returned. These are process-death scenarios, not hardware power-cut tests.
#![cfg(unix)]
#[path = "support/file_delivery.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::delivery::persistent::*;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::{ActionState, ElapsedTick};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const CHILD_TEST: &str = "file_delivery_process_child";
const ROOT_ENV: &str = "FA_FILE_DELIVERY_PROCESS_ROOT";
const MODE_ENV: &str = "FA_FILE_DELIVERY_PROCESS_MODE";
struct ChildGuard(Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        match self.0.try_wait() {
            Ok(Some(_)) => {}
            _ => {
                let _ = self.0.kill();
                if let Err(error) = self.0.wait() { eprintln!("file-delivery child reap failed: {error}"); }
            }
        }
    }
}

#[test]
fn file_delivery_process_child() {
    let Some(root) = std::env::var_os(ROOT_ENV) else { return; };
    let root = std::path::PathBuf::from(root);
    let mode = std::env::var(MODE_ENV).unwrap();
    let mut host = FileDelivery::create(root.join("publication"), profile()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let (action, permit) = approved(&mut host, 1, b"child publication");
    match mode.as_str() {
        "authorized" => {}
        "dispatched" => { host.dispatch(host.revision(), &permit, &action, snapshot()).unwrap(); }
        "published" => {
            host.dispatch(host.revision(), &permit, &action, snapshot()).unwrap();
            host.publish(host.revision(), 1).unwrap();
        }
        _ => panic!("invalid child mode"),
    }
    // A readiness marker is not journal evidence; it only tells the parent that
    // the selected real operation returned while this child still owns the lock.
    std::fs::write(root.join("ready"), b"ready").unwrap();
    loop {
        std::hint::black_box(&host);
        std::thread::sleep(Duration::from_secs(60));
    }
}

fn kill_owner(root: &Directory, mode: &str) {
    let child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact").arg(CHILD_TEST).arg("--nocapture")
        .env(ROOT_ENV, &root.0).env(MODE_ENV, mode)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::inherit())
        .spawn().unwrap();
    let mut child = ChildGuard(child);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() { panic!("child exited before kill: {status}"); }
        if root.0.join("ready").is_file() { break; }
        assert!(Instant::now() < deadline, "child did not become ready");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(FileDelivery::open(root.store(), profile()).unwrap_err(), JournalError::Busy);
    child.0.kill().unwrap();
    let status = child.0.wait().unwrap();
    assert!(!status.success(), "the owner must be terminated, not exit through normal cleanup");
}

#[test]
fn abrupt_owner_death_preserves_exact_liabilities_and_releases_the_kernel_lock() {
    for mode in ["authorized", "dispatched", "published"] {
        let root = Directory::new();
        kill_owner(&root, mode);
        let mut recovered = FileDelivery::open(root.store(), profile()).unwrap();
        assert!(!recovered.clock_ready());
        let state = recovered.inspect();
        assert_eq!(state.executions, u64::from(mode == "published"));
        assert_eq!(state.control.ledger.reserved, 0);
        if mode == "authorized" {
            assert_eq!(state.control.ledger.stages[&1], ActionState::Cancelled);
            assert_eq!(state.control.ledger.available, 100);
            assert_eq!(state.control.ledger.charged, 0);
        } else {
            assert_eq!(state.control.ledger.stages[&1], ActionState::Unknown);
            assert_eq!(state.control.ledger.charged, 16);
            assert_eq!(state.control.ledger.available, 84);
        }
        recovered.observe_time(recovered.revision(), ElapsedTick(2)).unwrap();
        let results = recovered.reconcile_pending(recovered.revision()).unwrap();
        match mode {
            "authorized" => assert!(results.is_empty()),
            "dispatched" => {
                assert_eq!(results[&1], Ok(Reconciliation::AwaitingResolution));
                assert_eq!(recovered.inspect().control.ledger.charged, 16);
                recovered.seal_unexecuted(recovered.revision(), 1).unwrap();
                assert_eq!(recovered.inspect().control.ledger.available, 100);
            }
            "published" => {
                assert_eq!(results[&1], Ok(Reconciliation::Resolved(EndpointOutcome::Executed { resulting_version: 2 })));
                assert_eq!(recovered.inspect().control.ledger.charged, 16);
                assert_eq!(recovered.inspect().payload, b"child publication");
            }
            _ => unreachable!(),
        }
        assert!(recovered.publish(recovered.revision(), 1).is_err());
        assert_eq!(recovered.inspect().executions, u64::from(mode == "published"));
    }
}
