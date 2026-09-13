//! Verify that automatic containment requests real direct-child cleanup before
//! a subsequent clock failure. This is not descendant or hostile-host isolation.
#![cfg(unix)]
#![forbid(unsafe_code)]
#[path = "support/supervised_driver.rs"]
#[allow(dead_code)]
mod control;
#[path = "support/hosted_decoder.rs"]
#[allow(dead_code)]
mod numerical;
use control::{Rig, snapshot};
use fa_reference::action::consequence::activation::monitor::decoder::MonitoredStep;
use fa_reference::action::consequence::oversight::decoder_host::HostedStopPolicy;
use fa_reference::action::consequence::oversight::decoder_monitoring::DecoderBindingLimits;
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::action::consequence::oversight::supervised::{DriverEvent, DriverPhase, ProcessReviewLaunch, ReviewLaunch};
use fa_reference::action::ElapsedTick;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("fa-automatic-child-{}-{stamp}", std::process::id()));
        fs::DirBuilder::new().mode(0o700).create(&path).unwrap(); Self(path.canonicalize().unwrap())
    }
    fn ready(&self, member: &str) -> PathBuf { self.0.join(format!("{member}.ready")) }
}
impl Drop for Directory {
    fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("automatic-child fixture cleanup: {error}"); } }
}

/// Subprocess entry, not an independent containment scenario. The readiness file
/// is written from the actual child before the parent is allowed to trigger stop.
#[test]
fn automatic_stop_worker() {
    let Some(ready) = std::env::var_os("FA_AUTOMATIC_CHILD_READY") else { return; };
    let ready = PathBuf::from(ready); let pending = ready.with_extension("pending");
    fs::write(&pending, std::process::id().to_string()).unwrap(); fs::rename(pending, ready).unwrap();
    std::thread::sleep(Duration::from_secs(30));
    panic!("parent did not terminate its registered child");
}

#[test]
fn quiet_work_preserves_live_children_but_a_trip_stops_them_before_failed_clock_io() {
    let directory = Directory::new(); let (mut rig, _) = Rig::new(false);
    let broker = rig.driver.supervisor_mut().broker_mut();
    broker.own_sampled_decoder(numerical::alarm(), DecoderBindingLimits::default()).unwrap();
    broker.enable_hosted_stop(HostedStopPolicy::new(1, 1, 900).unwrap()).unwrap();
    let state = broker.hosted_decoder().unwrap();
    assert!(matches!(rig.driver.advance_hosted_forced(state.actor_revision, 0, 0, numerical::compute(),
        || ElapsedTick(1)).inference.unwrap(), MonitoredStep::Released(_)));
    rig.accept(1);
    let ReviewLaunch { request, round, evidence_root, window, expected_input_revision, inputs, streams, limits } = rig.launch(1, 11);
    drop(streams); rig.clients.clear();
    let programs = ["alice", "bob"].into_iter().map(|member| {
        (member.to_owned(), HelperProgram::new(std::env::current_exe().unwrap(), directory.0.clone(),
            vec!["--exact".into(), "automatic_stop_worker".into(), "--nocapture".into()],
            BTreeMap::from([("FA_AUTOMATIC_CHILD_READY".into(), directory.ready(member).into_os_string())])).unwrap())
    }).collect();
    rig.driver.start_process_review(ProcessReviewLaunch { request, round, evidence_root, window,
        expected_input_revision, inputs, programs, limits }, &snapshot()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let originals = loop {
        let states = rig.driver.reap_helpers(); assert_eq!(states.len(), 2);
        assert!(states.values().all(|state| !state.stop_requested && state.exit.is_none()));
        let ready = states.iter().all(|(member, state)| match fs::read_to_string(directory.ready(member)) {
            Ok(pid) => { assert_eq!(pid.parse::<u32>().unwrap(), state.pid); true }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => panic!("readiness error: {error}"),
        });
        if ready { break states; }
        assert!(Instant::now() < deadline, "registered children did not become ready");
        std::thread::sleep(Duration::from_millis(1));
    };
    let state = rig.driver.supervisor().broker().hosted_decoder().unwrap();
    let quiet = rig.driver.advance_hosted_forced(state.actor_revision, state.position, 0,
        numerical::compute(), || ElapsedTick(1));
    assert!(matches!(quiet.inference.unwrap(), MonitoredStep::Released(_)));
    quiet.synchronization.unwrap(); assert!(quiet.containment.is_none());
    assert!(rig.driver.helper_processes().values().all(|state| !state.stop_requested && state.exit.is_none()));
    let state = rig.driver.supervisor().broker().hosted_decoder().unwrap();
    let mut calls = 0;
    let stopped = rig.driver.advance_hosted_forced(state.actor_revision, state.position, 1,
        numerical::compute(), || { calls += 1; ElapsedTick(if calls == 1 { 1 } else { 0 }) });
    assert!(matches!(stopped.inference.unwrap(), MonitoredStep::Held(_))); stopped.synchronization.unwrap();
    assert_eq!(stopped.containment, Some(Err(Error::Stale)));
    assert_eq!(rig.driver.phase(), DriverPhase::Idle);
    assert!(rig.driver.helper_processes().values().all(|state| state.stop_requested));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !rig.driver.helpers_reaped() {
        rig.driver.reap_helpers();
        assert!(Instant::now() < deadline, "registered children were not reaped");
        std::thread::sleep(Duration::from_millis(1));
    }
    for (member, state) in rig.driver.helper_processes() {
        assert_eq!(state.pid, originals[&member].pid); assert!(state.stop_requested && state.exit.is_some());
    }
    assert!(!rig.driver.stop_progress().unwrap().effects.endpoint_fenced);
    assert!(!rig.driver.stop_progress().unwrap().quiesced());
    let DriverEvent::HostedStop { sweep } = rig.driver.step(ElapsedTick(1), None, &snapshot(), None).unwrap()
        else { panic!("original stop not serviced after clock recovery"); };
    assert!(sweep.progress.drained()); assert!(rig.driver.stop_progress().unwrap().quiesced());
    assert_eq!(rig.driver.endpoint().execution_count(), 0);
    assert_eq!(rig.driver.supervisor().broker().inspect().ledger.available, 100);
}
