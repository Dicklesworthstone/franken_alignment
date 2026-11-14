//! Actual file capture and socket helpers under one original topology source.
//! The controlled graph and ballots test the authority path, not deployment
//! inventory completeness or helper-model quality.
use super::fixture::{self, FileRig, driver};
use fa_reference::action::{ElapsedTick, ResolvedTarget};
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::Reconciliation;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanPermit,
    driver::{FileDriverEvent, topology::FileDriverTopology},
    mediation::source::{TopologySourceWork, TOPOLOGY_REFRESH_READ_BYTES},
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileNarrowReceipt, FileSupervisorNarrowGrant, MAX_NARROW_REPLAY_EVENTS,
};
use fa_reference::action::consequence::oversight::actor::Knowledge;
use fa_reference::action::consequence::oversight::evidence_source::{
    FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES,
};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::perimeter_inventory::topology::LoadedAuthorityTopology;
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{ReadArguments, TowerAccess, TowerRequest, Verb, encode_request};
use fa_reference::typestate_cx::PurposeTag;
use std::{fs, path::PathBuf};

pub const REQUEST: u64 = 1;
pub const INVENTORY: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology_inventory.json"
));

pub fn graph(checks: u64, bypass: bool) -> String {
    let text = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/authority_topology.json"
    ))
    .replace("\"checks\": 4096", &format!("\"checks\": {checks}"))
    .replace("\"read_bytes\": 16777216", &format!(
        "\"read_bytes\": {}", checks * TOPOLOGY_REFRESH_READ_BYTES
    ));
    if bypass {
        let changed = text.replace("\"provenance\": 102}",
            "\"provenance\": 102}, {\"id\": 3, \"from\": 1, \"to\": 3, \"channel\": \"dispatch\", \"route\": \"publication\", \"provenance\": 103}");
        assert_ne!(changed, text);
        changed
    } else {
        text
    }
}

pub fn topology_path(file: &FileRig) -> PathBuf {
    file.rig.root.0.join("topology.json")
}

pub fn prepared(checks: u64, attached: bool) -> (FileRig, Option<FileDriverTopology>) {
    let mut rig = driver::Rig::new();
    let topology = rig.root.0.join("topology.json");
    let inventory = rig.root.0.join("inventory.json");
    let text = graph(checks, false);
    fs::write(&topology, &text).unwrap();
    fs::write(&inventory, INVENTORY).unwrap();
    let expected = LoadedAuthorityTopology::from_json_bytes(text.as_bytes(), INVENTORY.as_bytes())
        .unwrap();
    let source = {
        let mut host = rig.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.enable_publication_guard(revision).unwrap();
        let revision = host.revision();
        let observer = host.enable_mediation(revision, expected.graph().clone()).unwrap();
        let mut source = observer.bind_topology_source(
            &host, host.revision(), topology, inventory, expected,
        ).unwrap();
        let revision = host.revision();
        let qualified = source.refresh(&mut host, revision, 1).unwrap();
        assert!(qualified.refusal.is_none());
        assert_eq!(qualified.work.checks, 1);
        source
    };
    let selected = FileDriverTopology::new(source);
    let detached = if attached {
        let revision = rig.driver.supervisor().host().unwrap().revision();
        rig.driver.require_topology(revision, selected).unwrap();
        None
    } else {
        Some(selected)
    };
    let path = rig.root.0.join("evidence.json");
    fixture::replace(&path, &fixture::document(1));
    let source = FileEvidenceSource::new(
        &path, fixture::SOURCE, driver::profile().delivery.scope, MAX_EVIDENCE_FILE_BYTES,
    ).unwrap();
    rig.submit(REQUEST);
    let mut file = FileRig { rig, source, path };
    file.reviewed();
    (file, detached)
}

pub fn image(file: &FileRig) -> Vec<u8> {
    fs::read(file.rig.root.store().join("delivery.bin")).unwrap()
}

pub fn work(file: &FileRig) -> TopologySourceWork {
    file.rig.driver.topology().unwrap().source().work()
}

pub fn target(file: &FileRig) -> ResolvedTarget {
    file.rig.driver.supervisor().host().unwrap().request_action(REQUEST)
        .unwrap().spec().target.unwrap()
}

pub fn human(file: &mut FileRig, key: u64) -> FileHumanPermit {
    let request = file.rig.driver.request_human_approval_from_file(
        &mut file.source, key, ElapsedTick(31), || ElapsedTick(1),
    ).result.unwrap();
    let mut host = file.rig.driver.supervisor_mut().host_mut().unwrap();
    let revision = host.revision();
    file.rig.reviewer.approve(&mut host, revision, &request).unwrap()
}

pub fn publish_and_reconcile(file: &mut FileRig) -> EndpointOutcome {
    let event = file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None)
        .result.unwrap();
    let FileDriverEvent::PublicationChecked { publication, source_failure, .. } = event else {
        panic!("original publication gate was bypassed: {event:?}");
    };
    assert_eq!(source_failure, None);
    let outcome = publication.outcome;
    assert!(matches!(
        file.rig.driver.step_from_file(&mut file.source, || ElapsedTick(1), None).result,
        Ok(FileDriverEvent::Reconciled {
            outcome: Reconciliation::Resolved(ref observed), ..
        }) if observed == &outcome
    ));
    outcome
}

pub fn access() -> TowerAccess {
    TowerAccess::new(CallerScope::full_tenant_access("1"), true, vec![PurposeTag::Permit], 0)
}

fn mount() -> Address {
    Address::new("1", Noun::Run, "3", Some(41)).unwrap()
}

pub fn operator(file: &FileRig) -> (PublicationTower, FileSupervisorNarrowGrant) {
    let (port, grant) = file.rig.driver.supervisor()
        .narrow_port(8, 2, MAX_NARROW_REPLAY_EVENTS).unwrap();
    let mut tower = PublicationTower::new_control_operator(
        mount(), file.rig.port.clone(), PublicationLimits::default(),
    ).unwrap();
    tower.set_narrow(Some(port)).unwrap();
    (tower, grant)
}

pub fn command(tower: &mut PublicationTower, operation: u64, retained_targets: Vec<ResolvedTarget>)
    -> TowerRequest
{
    let mut query = TowerRequest::new(format!("mediated-narrow-plan-{operation}"), mount(), Verb::Next);
    query.arguments = ReadArguments::NarrowingNext { operation, retained_targets };
    let response = tower.exchange(&access(), encode_request(&query).unwrap().as_bytes());
    response.to_json().unwrap();
    let PublicationResult::NarrowingNext(linked) = response.result else {
        panic!("original narrowing plan unavailable");
    };
    let Ok(Knowledge::Known { value, basis }) = linked.native() else {
        panic!("original owner did not acknowledge its narrowing plan");
    };
    assert_eq!(value.basis(), *basis);
    assert!(value.can_narrow(), "{:?}", value.block_reason());
    linked.action().unwrap().unwrap()
}

pub fn apply(tower: &mut PublicationTower, command: &TowerRequest) -> PublicationResponse {
    let response = tower.exchange(&access(), encode_request(command).unwrap().as_bytes());
    response.to_json().unwrap();
    response
}

pub fn receipt(response: &PublicationResponse) -> &FileNarrowReceipt {
    let PublicationResult::Narrowing(linked) = &response.result else {
        panic!("original narrowing receipt missing: {response:?}");
    };
    let Ok(Knowledge::Known { value, basis }) = linked.native() else {
        panic!("original narrowing was refused: {response:?}");
    };
    assert_eq!(value.after(), *basis);
    value
}
