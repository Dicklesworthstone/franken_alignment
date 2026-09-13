#![allow(dead_code)]
#[path = "file_evidence_driver.rs"] pub mod evidence;
pub use evidence::{FileRig, SOURCE, document, replace, driver};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::FileHumanPermit;
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileDriverEvent;
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::{FileEvidenceReport, FileReviewLaunch};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};
use fa_reference::round::Verdict;
use std::os::unix::net::UnixStream;

pub fn configured(generation: u64, age: u64, limits: StateLimits) -> FileRig {
    let mut rig = driver::Rig::new();
    let path = rig.root.0.join("evidence.json"); replace(&path, &document(generation));
    let mut source = FileEvidenceSource::new(&path, SOURCE, driver::profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_file_source(revision, FileSourcePolicy {
            source: StateSource { scope: driver::profile().delivery.scope, source: SOURCE, generation: 1 },
            limits, freshness: StateFreshness::new(age).unwrap(),
        }).unwrap();
        let revision = host.revision(); host.refresh_file_source(revision, &mut source, ElapsedTick(1)).unwrap();
    }
    let proposal = rig.proposal();
    let revision = rig.driver.supervisor().host().unwrap().revision();
    rig.driver.supervisor_mut().set_snapshot(revision, Some(document(generation).snapshot().clone())).unwrap();
    let _ticket = rig.port.submit(1, &proposal).unwrap();
    FileRig { rig, source, path }
}

pub fn launch(file: &mut FileRig, round: u64) -> FileReviewLaunch<UnixStream> {
    let host = file.rig.driver.supervisor().host().unwrap();
    let epoch = host.request_action(1).unwrap().spec().policy_epoch;
    let (workers, clients) = driver::sockets(epoch);
    file.rig.clients = clients;
    FileReviewLaunch { request: 1, round, window: driver::helper::window(&host),
        expected_input_revision: host.input_revision(1).unwrap(), workers, limits: HelperLimits::default() }
}

pub fn finish(file: &mut FileRig, now: u64, verdict: Verdict) -> FileEvidenceReport<FileDriverEvent> {
    for _ in 0..128 {
        let report = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(now), None);
        if matches!(report.result, Ok(FileDriverEvent::Workers { .. })) {
            assert!(report.observations.is_empty()); assert!(report.source_updates.is_empty());
            driver::worker_steps(&mut file.rig.clients, verdict);
        } else { return report; }
    }
    panic!("original worker review exceeded the bounded fixture pass count");
}

pub fn reviewed(file: &mut FileRig) {
    let launch = launch(file, 101);
    file.rig.driver.start_file_review(&mut file.source, launch, || ElapsedTick(1)).unwrap();
    let report = finish(file, 1, Verdict::Allow);
    assert!(matches!(report.result, Ok(FileDriverEvent::ReviewApplied { .. })));
    assert_eq!(report.source_updates.len(), 1); assert!(report.source_updates[0].is_ok());
}

pub fn human(file: &mut FileRig, now: u64) -> FileHumanPermit {
    let report = file.rig.driver.request_human_approval_from_file(&mut file.source, 1001,
        ElapsedTick(now + 30), || ElapsedTick(now));
    assert_eq!(report.source_updates.len(), 1); assert!(report.source_updates[0].is_ok());
    let request = report.result.unwrap();
    let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision(); file.rig.reviewer.approve(&mut host, revision, &request).unwrap()
}

pub fn capture_count(file: &FileRig) -> usize {
    file.rig.driver.supervisor().host().unwrap().file_source_status().unwrap().capture.retained_events
}
