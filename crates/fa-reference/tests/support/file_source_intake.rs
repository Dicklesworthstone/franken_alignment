#![allow(dead_code)]
#[path = "file_evidence_driver.rs"] pub mod evidence;
pub use evidence::{FileRig, SOURCE, document, replace, driver};
use fa_reference::action::consequence::delivery::persistent::observed::source::FileSourcePolicy;
use fa_reference::action::consequence::oversight::evidence_source::{FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
use fa_reference::action::consequence::oversight::policy_state::{StateFreshness, StateLimits, StateSource};

/// No bootstrap refresh, no manual snapshot and no existing actor request.
pub fn cold(age: u64, limits: StateLimits) -> FileRig {
    let mut rig = driver::Rig::new();
    let path = rig.root.0.join("evidence.json");
    replace(&path, &document(1));
    let source = FileEvidenceSource::new(&path, SOURCE, driver::profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_file_source(revision, FileSourcePolicy {
            source: StateSource { scope: driver::profile().delivery.scope, source: SOURCE, generation: 1 },
            limits, freshness: StateFreshness::new(age).unwrap(),
        }).unwrap();
    }
    FileRig { rig, source, path }
}
