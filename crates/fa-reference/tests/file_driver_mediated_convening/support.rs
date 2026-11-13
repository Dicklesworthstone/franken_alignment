//! Reuse the actual file evidence and socket-congress fixtures with one original
//! mediation observer. Controlled graph declarations and ballots do not attest
//! production process isolation or model reliability.

use super::fixture::{self, FileRig, HELD_ROUND, REQUEST, document, driver, replace};
use fa_reference::action::consequence::Consequence;
use fa_reference::action::consequence::delivery::persistent::observed::driver::{
    FileDriverEvent, FileDriverPhase,
    topology::FileDriverTopology,
};
use fa_reference::action::consequence::delivery::persistent::observed::mediation::source::{
    TopologySourceWork, TOPOLOGY_REFRESH_READ_BYTES,
};
use fa_reference::action::consequence::mediation::MAX_CHECK_EDGE_VISITS;
use fa_reference::action::consequence::oversight::evidence_source::{
    FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
};
use fa_reference::action::ActionState;
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use fa_reference::round::Verdict;
use std::{fs, path::PathBuf};

pub const INVENTORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/authority_topology_inventory.json"
));

pub fn topology_path(file: &FileRig) -> PathBuf {
    file.rig.root.0.join("topology.json")
}

pub fn inventory_path(file: &FileRig) -> PathBuf {
    file.rig.root.0.join("inventory.json")
}

pub fn graph(generation: u64, checks: u64, bypass: bool) -> String {
    let extra = if bypass {
        r#",{"id":3,"from":1,"to":4,"channel":"dispatch","route":"publication","provenance":103}"#
    } else {
        ""
    };
    let read_bytes = checks * TOPOLOGY_REFRESH_READ_BYTES;
    format!(
        r#"{{"schema":"fa.authority-topology/1","generation":{generation},
        "inventory_generation":{generation},"complete_inventory_generation":{generation},
        "scope":{{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5,"purpose":"effect"}},
        "target":{{"adapter":10,"object":11,"contract_version":1,"expected_version":1,"generation":1}},
        "inventory_scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"file-publication",
        "nodes":[{{"id":1,"kind":"actor"}},{{"id":2,"kind":"enforcer"}},{{"id":4,"kind":"sink"}}],
        "edges":[{{"id":1,"from":1,"to":2,"channel":"dispatch","route":"publication","provenance":101}},
        {{"id":2,"from":2,"to":4,"channel":"dispatch","route":"publication","provenance":102}}{extra}],
        "enforcers":[{{"node":2,"adapter":10,"contract_version":1,"generation":1,"provenance":102}}],
        "gates":[2],"check_edge_budget":{MAX_CHECK_EDGE_VISITS},
        "source_limits":{{"checks":{checks},"read_bytes":{read_bytes}}}}}"#
    )
}

fn prepared(checks: u64) -> (FileRig, FileDriverTopology) {
    let mut rig = driver::Rig::new();
    let topology = rig.root.0.join("topology.json");
    let inventory = rig.root.0.join("inventory.json");
    let text = graph(1, checks, false);
    fs::write(&topology, &text).unwrap();
    fs::write(&inventory, INVENTORY).unwrap();
    let expected =
        LoadedAuthorityTopology::from_json_bytes(text.as_bytes(), INVENTORY.as_bytes()).unwrap();
    let source = {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_publication_guard(revision).unwrap();
        let revision = host.revision();
        let observer = host.enable_mediation(revision, expected.graph().clone()).unwrap();
        let mut source = observer
            .bind_topology_source(&host, host.revision(), topology, inventory, expected)
            .unwrap();
        let revision = host.revision();
        let checked = source.refresh(&mut host, revision, 1).unwrap();
        assert!(checked.refusal.is_none());
        assert_eq!(checked.work.checks, 1);
        source
    };
    let path = rig.root.0.join("evidence.json");
    replace(&path, &document(1));
    let source_file = FileEvidenceSource::new(
        &path,
        fixture::evidence::SOURCE,
        driver::profile().delivery.scope,
        MAX_EVIDENCE_FILE_BYTES,
    )
    .unwrap();
    (
        FileRig { rig, source: source_file, path },
        FileDriverTopology::new(source),
    )
}

fn complete_hold(file: &mut FileRig) {
    let _ticket = file.rig.submit(REQUEST);
    file.start(HELD_ROUND);
    let FileDriverEvent::ReviewApplied { receipt, .. } =
        file.finish(Verdict::Abstain).result.unwrap()
    else {
        panic!("original mediated congress did not complete");
    };
    assert_eq!(receipt.policy.control.decision.consequence, Consequence::HoldEffect);
    assert_eq!(file.rig.driver.phase(), FileDriverPhase::Idle);
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().control.ledger.stages[&1], ActionState::Reviewing);
    assert_eq!(host.inspect().executions, 0);
    assert!(host.mediation_snapshot().unwrap().accepted.is_some());
}

pub fn held(checks: u64) -> FileRig {
    let (mut file, selected) = prepared(checks);
    attach(&mut file, selected);
    complete_hold(&mut file);
    assert_eq!(work(&file).checks, 1);
    file
}

pub fn held_without_selection(checks: u64) -> (FileRig, FileDriverTopology) {
    let (mut file, selected) = prepared(checks);
    complete_hold(&mut file);
    (file, selected)
}

pub fn attach(file: &mut FileRig, selected: FileDriverTopology) {
    let revision = file.rig.driver.supervisor().host().unwrap().revision();
    file.rig.driver.require_topology(revision, selected).unwrap();
}

pub fn work(file: &FileRig) -> TopologySourceWork {
    file.rig.driver.topology().unwrap().source().work()
}
