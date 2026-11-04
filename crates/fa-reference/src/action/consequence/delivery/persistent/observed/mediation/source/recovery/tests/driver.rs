//! The actual driver owns the resumed custodian before any fresh source check.
use super::*;
use crate::action::consequence::delivery::persistent::observed::driver::FileDriverPhase;
use crate::action::consequence::delivery::persistent::observed::driver::topology::DriverTopologyOutcome;
use crate::action::consequence::delivery::persistent::observed::mediation::FileMediationUpdate;

#[test]
fn driver_reinstatement_checks_actual_files_and_retains_the_same_spent_source() {
    let Recovered { mut host, observer, source, root } = recovered();
    let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
    fs::write(root.topology(), text(2)).unwrap();
    let revision = host.revision(); let (_port, mut driver) = host.into_supervised_driver();
    assert_verified(driver.resume_topology(revision, 2, observer, source).unwrap());
    assert_eq!(driver.phase(), FileDriverPhase::Idle);
    let selected = driver.topology().unwrap();
    assert_eq!(selected.source().work().checks, 2); assert_eq!(selected.source().last_operation(), 3);
    let observation = selected.last_observation().unwrap();
    assert_eq!(observation.request, None); assert_eq!(observation.attempt, None);
    assert!(matches!(&observation.outcome, DriverTopologyOutcome::Finished(Ok(report)) if report.refusal.is_none()));
    assert!(driver.supervisor().host().unwrap().mediation_snapshot().unwrap().accepted.is_some());
    // The later check is not a newly admitted source or a renewed allowance.
    fs::rename(root.inventory(), root.0.join("retained-inventory")).unwrap();
    let revision = driver.supervisor().host().unwrap().revision();
    let report = driver.refresh_topology(revision).unwrap();
    assert!(report.refusal.is_some()); assert_eq!(driver.topology().unwrap().source().work().checks, 3);
    assert!(!driver.supervisor().host().unwrap().mediation_snapshot().unwrap().available);
    assert_eq!(driver.supervisor().host().unwrap().inspect().executions, 0);
}

#[test]
fn initial_refusal_keeps_custody_diagnostics_and_withdrawal_in_the_driver() {
    let Recovered { mut host, observer, source, root } = recovered();
    let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
    let revision = host.revision(); let (_port, mut driver) = host.into_supervised_driver();
    // Generation 1 remains on disk; independent expectations require generation 2.
    let report = driver.resume_topology(revision, 2, observer, source).unwrap();
    assert_eq!(report.refusal, Some(TopologySourceRefusal::ChangedInput));
    let selected = driver.topology().unwrap();
    assert_eq!(selected.source().work().checks, 2); assert_eq!(selected.source().last_operation(), 3);
    assert!(matches!(&selected.last_observation().unwrap().outcome,
        DriverTopologyOutcome::Finished(Ok(retained)) if retained == &report));
    let host = driver.supervisor().host().unwrap();
    assert!(!host.mediation_snapshot().unwrap().available); assert!(host.mediation_snapshot().unwrap().accepted.is_none());
    assert_eq!(host.inspect().executions, 0); assert!(!root.bytes().is_empty());
}

#[test]
fn driver_requires_room_for_initial_and_final_checks_before_any_rebind() {
    for exhausted_operations in [false, true] {
        let Recovered { host, observer, mut source, root } = recovered();
        let operation = if exhausted_operations { u64::MAX - 1 } else { 8 };
        if !exhausted_operations {
            source.work = TopologySourceWork { checks: 7, reserved_read_bytes: 7 * TOPOLOGY_REFRESH_READ_BYTES };
            source.last_operation = 7;
        }
        let before = root.bytes(); let revision = host.revision();
        let (_port, mut driver) = host.into_supervised_driver();
        assert!(driver.resume_topology(revision, operation, observer, source).is_err());
        assert!(driver.topology().is_none()); assert_eq!(root.bytes(), before);
        assert_eq!(driver.supervisor().host().unwrap().revision(), revision);
        assert!(!driver.supervisor().host().unwrap().mediation_snapshot().unwrap().available);
    }
}

#[test]
fn recorded_operation_floor_dominates_an_older_independent_counter() {
    for operation in [2, 901] {
        let Recovered { mut host, observer, source, root } = recovered();
        let update = FileMediationUpdate { operation: 900, expected_generation: 1,
            expected_authority_epoch: host.inspect().control.ledger.epoch, next: None };
        let revision = host.revision(); observer.update(&mut host, revision, &update).unwrap();
        let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
        fs::write(root.topology(), text(2)).unwrap();
        let before = root.bytes(); let revision = host.revision();
        let (_port, mut driver) = host.into_supervised_driver();
        let result = driver.resume_topology(revision, operation, observer, source);
        if operation == 2 {
            assert!(matches!(result, Err(JournalError::Contract(Error::Stale))));
            assert_eq!(root.bytes(), before); assert!(driver.topology().is_none());
        } else {
            assert_verified(result.unwrap());
            assert_eq!(driver.topology().unwrap().source().last_operation(), 902);
            assert_eq!(driver.topology().unwrap().source().work().checks, 2);
        }
    }
}

#[test]
fn retained_unpersisted_operation_floor_survives_automatic_refresh_allocation() {
    let Recovered { mut host, observer, mut source, root } = recovered();
    source.last_operation = 40;
    let revision = host.revision(); host.observe_time(revision, ElapsedTick(2)).unwrap();
    fs::write(root.topology(), text(2)).unwrap();
    let revision = host.revision(); let (_port, mut driver) = host.into_supervised_driver();
    assert_verified(driver.resume_topology(revision, 41, observer, source).unwrap());
    assert_eq!(driver.topology().unwrap().source().last_operation(), 42);
    assert_eq!(driver.topology().unwrap().source().work().checks, 2);
}
