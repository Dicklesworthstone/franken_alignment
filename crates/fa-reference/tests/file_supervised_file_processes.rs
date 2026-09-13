//! Executable workers consume actual file-captured inputs on the durable path.
#![cfg(unix)]
#[path = "support/file_evidence_driver.rs"] mod fixture;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::FileSourceReviewError;
use fa_reference::action::consequence::delivery::persistent::observed::helpers::processes::HelperRoundAdmission;
use fa_reference::action::consequence::delivery::persistent::observed::publication::PublicationBasis;
use fa_reference::action::consequence::oversight::helper_client::{ClientPhase, HelperClient};
use fa_reference::action::consequence::oversight::helper_processes::HelperProgram;
use fa_reference::round::Verdict;
use fa_reference::Error;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::time::{Duration, Instant};

const MEMBER: &str = "FA_FILE_SOURCE_HELPER_MEMBER";
const CHILD: &str = "file_evidence_child";

/// Synthetic fixture, not a trained model. The original client decodes the
/// inherited socket input and creates the actual bound commitment and reveal.
#[test]
fn file_evidence_child() {
    let Ok(member) = std::env::var(MEMBER) else { return; };
    let profile = driver::profile().committee.members()[&member].profile_at(0);
    let mut client = HelperClient::from_process_stdin(profile).unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        client.step().unwrap();
        if client.phase() == ClientPhase::NeedsInference {
            let input = client.input().unwrap();
            assert_eq!(input.member(), member);
            let (own, other) = if member == "alpha" { (ALPHA, BETA) } else { (BETA, ALPHA) };
            let bytes = input.actual_input().submitted_bytes();
            assert!(bytes.windows(own.len()).any(|part| part == own));
            assert!(!bytes.windows(other.len()).any(|part| part == other));
            assert!(!bytes.windows(PRIVATE.len()).any(|part| part == PRIVATE));
            client.respond(Verdict::Allow, &driver::helper::oversight::salt(&member)).unwrap();
        }
        if client.phase() == ClientPhase::ReplySent { return; }
        assert!(Instant::now() < deadline, "child fixture exceeded its bound");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn programs(file: &FileRig) -> BTreeMap<String, HelperProgram> {
    driver::MEMBERS.into_iter().map(|member| {
        let program = HelperProgram::new(std::env::current_exe().unwrap(), file.rig.root.0.clone(),
            vec![OsString::from("--exact"), OsString::from(CHILD), OsString::from("--nocapture")],
            BTreeMap::from([(OsString::from(MEMBER), OsString::from(member))])).unwrap();
        (member.to_owned(), program)
    }).collect()
}
fn finish(file: &mut FileRig) {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
        match report.result.unwrap() {
            FileDriverEvent::Workers { .. } => assert!(report.observations.is_empty()),
            FileDriverEvent::ReviewApplied { .. } => {
                assert_eq!(report.observations, vec![Ok(document(1).identity())]);
                assert_eq!(file.rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
                return;
            }
            event => panic!("unexpected process review result: {event:?}"),
        }
        assert!(Instant::now() < deadline, "parent fixture exceeded its bound");
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn reap(file: &mut FileRig) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !file.rig.driver.helpers_reaped() {
        file.rig.driver.reap_helpers();
        assert!(Instant::now() < deadline, "direct children were not reaped");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(file.rig.driver.helper_processes().values().all(|status| status.exit.is_some()));
}

#[test]
fn file_backed_processes_reach_original_human_keys_and_guarded_publication() {
    let mut file = FileRig::new();
    let launch = launch(&file.rig, 101, programs(&file));
    let identity = file.rig.driver.start_file_process_review(&mut file.source, launch, || ElapsedTick(1)).unwrap();
    assert_eq!(identity, document(1).identity());
    assert_eq!(file.rig.driver.helper_processes().len(), 2);
    finish(&mut file);
    let key = file.human();
    file.dispatch(&key);
    let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(matches!(report.result, Ok(FileDriverEvent::PublicationChecked { publication, source_failure: None, .. })
        if publication.basis == PublicationBasis::Revalidated && matches!(publication.outcome, EndpointOutcome::Executed { resulting_version: 2 })));
    std::fs::remove_file(&file.path).unwrap();
    let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(2), None);
    assert!(report.observations.is_empty());
    assert!(matches!(report.result, Ok(FileDriverEvent::Reconciled { outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }), .. })));
    assert_eq!(file.source.status().read_attempts, 6);
    reap(&mut file);
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().payload, b"publication");
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn partial_process_failure_keeps_capture_identity_and_every_started_child() {
    let mut file = FileRig::new();
    let not_executable = file.rig.root.0.join("not-executable");
    std::fs::write(&not_executable, b"not a program").unwrap();
    std::fs::set_permissions(&not_executable, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut programs = programs(&file);
    programs.insert("beta".to_owned(), HelperProgram::new(not_executable, file.rig.root.0.clone(), Vec::new(), BTreeMap::new()).unwrap());
    let launch = launch(&file.rig, 101, programs);
    let error = file.rig.driver.start_file_process_review(&mut file.source, launch, || ElapsedTick(1)).unwrap_err();
    match error {
        FileSourceReviewError::Processes { observation, error } => {
            assert_eq!(observation, document(1).identity());
            assert_eq!(error.admission, HelperRoundAdmission::Committed);
        }
        error => panic!("unexpected setup result: {error:?}"),
    }
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    assert_eq!(file.rig.driver.helper_processes().len(), 1);
    assert!(file.rig.driver.helper_processes()["alpha"].stop_requested);
    assert_eq!(file.source.status().read_attempts, 1);
    reap(&mut file);
    let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    assert_eq!(host.commit_review(revision, 101, "alpha", 0).unwrap_err(), JournalError::Contract(Error::WrongState));
    let input = document(1).inputs_for(host.request_action(1).unwrap(), &driver::profile().committee).unwrap();
    let revision = host.revision();
    assert!(host.authorize(revision, 1, &input, driver::snapshot()).is_err());
    assert_eq!(host.inspect().control.ledger.available, 100);
    assert_eq!(host.inspect().executions, 0);
}
