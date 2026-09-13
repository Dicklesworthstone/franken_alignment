#![allow(dead_code)]
#[path = "file_driver.rs"] pub mod driver;
use driver::Rig;
use fa_reference::action::consequence::delivery::persistent::observed::FileHumanPermit;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{FileDriverEvent, FileDriverPhase};
use fa_reference::action::consequence::delivery::persistent::observed::driver::evidence::{FileEvidenceReport, FileReviewLaunch};
use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::helper_workers::HelperLimits;
use fa_reference::action::ElapsedTick;
use fa_reference::round::Verdict;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SOURCE: u64 = 51;
pub const ALPHA: &[u8] = b"ALPHA-INDEPENDENT-FILE-CONTEXT";
pub const BETA: &[u8] = b"BETA-INDEPENDENT-FILE-CONTEXT";
pub const PRIVATE: &[u8] = b"PRIVATE-POLICY-VALUE-NOT-A-HELPER-CONTEXT";

pub fn document(generation: u64) -> EvidenceSnapshot {
    let mut snapshot = driver::snapshot();
    snapshot.values.insert(99, PRIVATE.to_vec());
    EvidenceSnapshot::new(EvidenceIdentity { source: SOURCE, generation, scope: driver::profile().delivery.scope },
        snapshot, BTreeMap::from([("alpha".to_owned(), ALPHA.to_vec()), ("beta".to_owned(), BETA.to_vec())])).unwrap()
}
pub fn replace(path: &Path, document: &EvidenceSnapshot) {
    let pending = path.with_extension("next");
    std::fs::write(&pending, document.encode()).unwrap();
    std::fs::rename(pending, path).unwrap();
}
pub fn launch<W>(rig: &Rig, round: u64, workers: BTreeMap<String, W>) -> FileReviewLaunch<W> {
    let host = rig.driver.supervisor().host().unwrap();
    FileReviewLaunch { request: 1, round, window: driver::helper::window(&host),
        expected_input_revision: host.input_revision(1).unwrap(), workers, limits: HelperLimits::default() }
}

pub struct FileRig {
    pub rig: Rig,
    pub source: FileEvidenceSource,
    pub path: PathBuf,
}
impl FileRig {
    pub fn new() -> Self {
        let mut rig = Rig::new();
        {
            let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
            let revision = host.revision();
            host.enable_publication_guard(revision).unwrap();
        }
        let path = rig.root.0.join("evidence.json");
        replace(&path, &document(1));
        let source = FileEvidenceSource::new(&path, SOURCE, driver::profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
        let _ticket = rig.submit(1);
        Self { rig, source, path }
    }
    pub fn start(&mut self, round: u64) {
        let epoch = self.rig.driver.supervisor().host().unwrap().request_action(1).unwrap().spec().policy_epoch;
        let (workers, clients) = driver::sockets(epoch);
        self.rig.clients = clients;
        let launch = launch(&self.rig, round, workers);
        let observed = self.rig.driver.start_file_review(&mut self.source, launch, || ElapsedTick(1)).unwrap();
        assert_eq!(observed, document(1).identity());
    }
    pub fn finish(&mut self, verdict: Verdict) -> FileEvidenceReport<FileDriverEvent> {
        for _ in 0..128 {
            let report = self.rig.driver.step_from_file(&mut self.source, || ElapsedTick(1), None);
            if matches!(&report.result, Ok(FileDriverEvent::Workers { .. })) {
                assert!(report.observations.is_empty());
                driver::worker_steps(&mut self.rig.clients, verdict);
            } else { return report; }
        }
        panic!("file-backed review did not finish within the fixed fixture bound");
    }
    pub fn reviewed(&mut self) {
        self.start(101);
        let report = self.finish(Verdict::Allow);
        assert_eq!(report.observations, vec![Ok(document(1).identity())]);
        assert!(matches!(report.result, Ok(FileDriverEvent::ReviewApplied { .. })));
        assert_eq!(self.rig.driver.phase(), FileDriverPhase::AwaitingDispatch { request: 1 });
    }
    pub fn human(&mut self) -> FileHumanPermit {
        let report = self.rig.driver.request_human_approval_from_file(&mut self.source, 1001, ElapsedTick(31), || ElapsedTick(1));
        assert_eq!(report.observations, vec![Ok(document(1).identity())]);
        let request = report.result.unwrap();
        let mut host = self.rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        self.rig.reviewer.approve(&mut host, revision, &request).unwrap()
    }
    pub fn dispatch(&mut self, key: &FileHumanPermit) {
        let report = self.rig.driver.step_from_file(&mut self.source, || ElapsedTick(1), Some(key));
        assert_eq!(report.observations, vec![Ok(document(1).identity()), Ok(document(1).identity())]);
        assert!(matches!(report.result, Ok(FileDriverEvent::Dispatched { request: 1, attempt: 1 })));
    }
}
