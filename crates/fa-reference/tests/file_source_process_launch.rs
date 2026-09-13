//! Original executable helper protocol after a durable source launch capture.
#![cfg(unix)]
#[path = "support/file_driver_source.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileDriverEvent;
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::{FileReviewLaunch, FileSourceReviewError};
use fa_reference::action::consequence::delivery::persistent::observed::helpers::processes::{FileProcessFailure, HelperRoundAdmission};
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourceError;
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::{HelperProgram, ProcessFailure, ProcessStage};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::consequence::oversight::policy_state::StateLimits;
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

const CHILD: &str = "durable_source_worker_child";
const MEMBER_ENV: &str = "FA_SOURCE_DRIVER_WORKER";

#[test]
fn durable_source_worker_child() {
    let Ok(member) = std::env::var(MEMBER_ENV) else { return; };
    let contracts = driver::profile().committee;
    let mut client = HelperClient::from_process_stdin(contracts.members()[&member].profile_at(0)).unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while Instant::now() < deadline {
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let expected = if member == "alpha" { evidence::ALPHA } else { evidence::BETA };
            let forbidden = if member == "alpha" { evidence::BETA } else { evidence::ALPHA };
            let bytes = input.actual_input().submitted_bytes();
            assert!(bytes.windows(expected.len()).any(|part| part == expected));
            assert!(!bytes.windows(forbidden.len()).any(|part| part == forbidden));
            assert!(!bytes.windows(evidence::PRIVATE.len()).any(|part| part == evidence::PRIVATE));
            client.respond(Verdict::Allow, b"synthetic-executable-helper").unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("worker fixture exceeded its bound");
}

fn programs(file: &FileRig) -> BTreeMap<String, HelperProgram> {
    driver::MEMBERS.into_iter().map(|member| {
        (member.to_owned(), HelperProgram::new(std::env::current_exe().unwrap(), file.rig.root.0.clone(),
            vec![OsString::from("--exact"), OsString::from(CHILD), OsString::from("--nocapture")],
            BTreeMap::from([(OsString::from(MEMBER_ENV), OsString::from(member))])).unwrap())
    }).collect()
}
fn process_launch(file: &FileRig, workers: BTreeMap<String, HelperProgram>) -> FileReviewLaunch<HelperProgram> {
    let host = file.rig.driver.supervisor().host().unwrap();
    FileReviewLaunch { request: 1, round: 101, window: driver::helper::window(&host),
        expected_input_revision: host.input_revision(1).unwrap(), workers, limits: HelperLimits::default() }
}
fn reap(file: &mut FileRig) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !file.rig.driver.helpers_reaped() {
        file.rig.driver.reap_helpers();
        assert!(Instant::now() < deadline, "original child owner did not finish cleanup");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn executable_workers_receive_durable_fresh_inputs_and_reach_two_key_publication() {
    let mut file = configured(1, 3, StateLimits::default());
    replace(&file.path, &document(2));
    let launch = process_launch(&file, programs(&file)); let baseline = capture_count(&file);
    let identity = file.rig.driver.start_file_process_review(&mut file.source, launch, || ElapsedTick(4)).unwrap();
    assert_eq!(identity, document(2).identity()); assert_eq!(capture_count(&file), baseline + 1);
    let children = file.rig.driver.helper_processes();
    assert_eq!(children.len(), 2); assert_ne!(children["alpha"].pid, children["beta"].pid);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(4), None);
        if matches!(report.result, Ok(FileDriverEvent::Workers { .. })) {
            assert!(report.source_updates.is_empty());
            assert!(Instant::now() < deadline, "process review did not finish");
            std::thread::sleep(Duration::from_millis(1));
        } else {
            assert_eq!(report.source_updates, vec![Ok(document(2).identity())]);
            assert!(matches!(report.result, Ok(FileDriverEvent::ReviewApplied { .. })));
            break;
        }
    }
    let key = human(&mut file, 4);
    let dispatched = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(6), Some(&key));
    assert!(matches!(dispatched.result, Ok(FileDriverEvent::Dispatched { .. })));
    let published = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(8), None);
    assert!(matches!(published.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
        if publication.basis == PublicationBasis::Revalidated));
    assert_eq!(capture_count(&file), baseline + 6);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 1);
    reap(&mut file);
}

#[test]
fn source_failure_precedes_spawning_and_partial_spawn_keeps_its_admitted_source_and_child_owner() {
    let mut missing = configured(1, 10, StateLimits::default());
    let launch = process_launch(&missing, programs(&missing));
    fs::remove_file(&missing.path).unwrap();
    let result = missing.rig.driver.start_file_process_review(&mut missing.source, launch, || ElapsedTick(1));
    assert!(matches!(result, Err(FileSourceReviewError::DurableSource(FileSourceError::Read { .. }))));
    assert!(missing.rig.driver.helper_processes().is_empty()); assert!(missing.rig.driver.helpers_reaped());

    let mut file = configured(1, 10, StateLimits::default()); replace(&file.path, &document(2));
    let mut workers = programs(&file);
    let invalid = file.rig.root.0.join("not-executable");
    fs::write(&invalid, b"not an executable image").unwrap();
    fs::set_permissions(&invalid, fs::Permissions::from_mode(0o600)).unwrap();
    workers.insert("beta".to_owned(), HelperProgram::new(invalid, file.rig.root.0.clone(), Vec::new(), BTreeMap::new()).unwrap());
    let launch = process_launch(&file, workers);
    let result = file.rig.driver.start_file_process_review(&mut file.source, launch, || ElapsedTick(1));
    match result {
        Err(FileSourceReviewError::Processes { observation, error }) => {
            assert_eq!(observation, document(2).identity());
            assert_eq!(error.admission, HelperRoundAdmission::Committed);
            assert!(matches!(error.failure, FileProcessFailure::Launch { member: Some(member),
                failure: ProcessFailure::Io { stage: ProcessStage::Spawn, .. } } if member == "beta"));
        }
        other => panic!("unexpected partial launch result: {other:?}"),
    }
    assert_eq!(file.rig.driver.helper_processes().len(), 1);
    assert!(file.rig.driver.helper_processes()["alpha"].stop_requested);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().file_source_status().unwrap().producer.unwrap().generation, 2);
    assert_eq!(file.rig.driver.supervisor().host().unwrap().inspect().executions, 0);
    reap(&mut file);
}
