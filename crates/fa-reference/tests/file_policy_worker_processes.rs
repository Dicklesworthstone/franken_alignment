//! Real direct-child ownership; synthetic responses are not detector evidence.
#![cfg(unix)]
#[path = "support/file_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::action::consequence::delivery::persistent::{JournalError, governance::PolicyUpdate};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverLaunch};
use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use fa_reference::action::consequence::oversight::helper_client::{HelperClient, ClientPhase};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::action::consequence::oversight::supervised::DriverEvidence;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[test]
fn policy_update_worker_entry() {
    let Some(member) = std::env::var_os("FA_POLICY_MEMBER") else { return; };
    let member = member.to_str().unwrap();
    let ready = PathBuf::from(std::env::var_os("FA_POLICY_READY").unwrap());
    let release = PathBuf::from(std::env::var_os("FA_POLICY_RELEASE").unwrap());
    let mut worker = HelperClient::from_process_stdin(profile().committee.members()[member].profile_at(0)).unwrap();
    let mut announced = false;
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        worker.step().unwrap();
        if worker.phase() == ClientPhase::NeedsInference {
            if !announced {
                let input = worker.input().unwrap();
                assert_eq!(input.member(), member);
                assert_eq!(input.round(), 101);
                let evidence = b"complete evidence used by the real socket workers";
                assert!(input.actual_input().submitted_bytes().windows(evidence.len()).any(|value| value == evidence));
                let pending = ready.with_extension("pending");
                std::fs::write(&pending, std::process::id().to_string()).unwrap();
                std::fs::rename(pending, &ready).unwrap();
                announced = true;
            }
            if release.try_exists().unwrap() { worker.respond(Verdict::Allow, member.as_bytes()).unwrap(); }
        }
        if worker.phase() == ClientPhase::ReplySent { return; }
        assert!(Instant::now() < end, "fixture worker exceeded its bound");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn only_a_committed_policy_update_retires_the_original_live_helper_cohort() {
    for committed in [false, true] {
        let mut rig = Rig::new(); rig.submit(1);
        let launch = rig.launch(1, 101);
        let FileDriverLaunch { request, round, evidence_root, window, expected_input_revision, inputs, workers, limits } = launch;
        drop(workers); rig.clients.clear();
        let release = rig.root.0.join("release-worker-verdicts");
        let ready: BTreeMap<_, _> = MEMBERS.into_iter().map(|name| (name.to_owned(), rig.root.0.join(format!("{name}.ready")))).collect();
        let programs = MEMBERS.into_iter().map(|name| {
            let environment: BTreeMap<OsString, OsString> = BTreeMap::from([
                ("FA_POLICY_MEMBER".into(), name.into()),
                ("FA_POLICY_READY".into(), ready[name].clone().into_os_string()),
                ("FA_POLICY_RELEASE".into(), release.clone().into_os_string()),
            ]);
            (name.to_owned(), HelperProgram::new(std::env::current_exe().unwrap(), rig.root.0.clone(),
                vec!["--exact".into(), "policy_update_worker_entry".into(), "--nocapture".into()], environment).unwrap())
        }).collect();
        rig.driver.start_process_review(FileDriverLaunch { request, round, evidence_root, window,
            expected_input_revision, inputs, workers: programs, limits }, snapshot(), || ElapsedTick(1)).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(matches!(rig.step(None), FileDriverEvent::Workers { .. }));
            if ready.values().all(|path| path.try_exists().unwrap()) { break; }
            assert!(Instant::now() < end, "live workers failed to receive their original input");
            std::thread::sleep(Duration::from_millis(1));
        }
        let pids = rig.driver.helper_processes();
        assert_eq!(pids.len(), 2);
        for (member, state) in &pids {
            assert_eq!(std::fs::read_to_string(&ready[member]).unwrap().parse::<u32>().unwrap(), state.pid);
            assert!(!state.stop_requested && state.exit.is_none());
        }
        let control = rig.driver.supervisor().host().unwrap().inspect().control;
        let revision = rig.driver.supervisor().host().unwrap().revision();
        let update = PolicyUpdate::new(1, control.sequence + u64::from(!committed), control.ledger.epoch,
            Policy::new(2, vec![Predicate::PayloadAtMost(128)]).unwrap()).unwrap();
        let result = rig.driver.replace_policy(revision, &update);
        if committed {
            assert_eq!(result.unwrap().change().cancelled, vec![1]);
            assert!(rig.driver.helper_processes().values().all(|state| state.stop_requested));
            assert!(matches!(rig.driver.step_with_evidence(|| panic!("cancelled job sampled time"),
                |_, _| panic!("cancelled job reacquired input"), None).unwrap(),
                FileDriverEvent::Stopped { stage: ActionState::Cancelled, .. }));
        } else {
            assert_eq!(result, Err(JournalError::Contract(Error::Stale)));
            assert!(rig.driver.helper_processes().values().all(|state| !state.stop_requested && state.exit.is_none()));
            std::fs::write(&release, b"release synthetic verdicts").unwrap();
            let end = Instant::now() + Duration::from_secs(10);
            loop {
                let input = rig.inputs.clone();
                match rig.driver.step_with_evidence(|| ElapsedTick(1), |_, _|
                    Ok(DriverEvidence { snapshot: snapshot(), inputs: input.clone() }), None).unwrap() {
                    FileDriverEvent::Workers { .. } => {}
                    FileDriverEvent::ReviewApplied { .. } => break,
                    event => panic!("unexpected positive control: {event:?}"),
                }
                assert!(Instant::now() < end);
                std::thread::sleep(Duration::from_millis(1));
            }
            let human = rig.human(1001, 31);
            assert!(matches!(rig.step(Some(&human)), FileDriverEvent::Dispatched { .. }));
            assert!(matches!(rig.step(None), FileDriverEvent::Published { .. }));
        }
        let end = Instant::now() + Duration::from_secs(10);
        while !rig.driver.helpers_reaped() {
            rig.driver.reap_helpers();
            assert!(Instant::now() < end, "original children were not reaped");
            std::thread::sleep(Duration::from_millis(1));
        }
        for (member, state) in rig.driver.helper_processes() {
            assert_eq!(state.pid, pids[&member].pid);
            assert!(state.exit.is_some());
        }
        assert_eq!(rig.driver.supervisor().host().unwrap().inspect().executions, u64::from(!committed));
    }
}
