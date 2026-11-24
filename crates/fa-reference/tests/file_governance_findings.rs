//! Live findings must come from acknowledged native incidents under one finite
//! independent observation grant. Fixture labels are not detector qualification.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/file_credibility.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileIndependentEvaluator;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorSupervisor, FileGovernanceFindingsResult, FileGovernanceFindingsSnapshot,
    FileSupervisorGovernanceLedgerPort,
};
use fa_reference::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::governance_assurance::RiskTheaterFamily;
use fa_reference::observation_algebra::{
    Knowledge as QueryKnowledge, UnknownReason as QueryUnknownReason,
};
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};
use fixture::Directory;
use std::collections::BTreeSet;

fn evaluated(root: &Directory, budget: u64) -> (FileOversight, FileIndependentEvaluator) {
    let (mut host, _) = FileOversight::create(root.store(), fixture::profile()).unwrap();
    let mut protocol = fixture::protocol();
    protocol.false_stop_budget = budget;
    let evaluator = host.enable_credibility(host.revision(), protocol).unwrap();
    let config = FileAndonConfig::new(
        LedgerIdentity {
            deployment: 101,
            evaluator: 202,
        },
        vec![AndonBudget {
            window: 1,
            start: 0,
            end: 100,
            false_stops_allowed: budget,
            cost_per_stop: u64::MAX,
        }],
    )
    .unwrap();
    host.enable_andon(host.revision(), &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, evaluator)
}

fn known(result: FileGovernanceFindingsResult) -> FileGovernanceFindingsSnapshot {
    match result.unwrap() {
        Knowledge::Known { value, basis } => {
            assert_eq!(value.basis(), basis);
            assert_eq!(value.risk_theater().len(), RiskTheaterFamily::ALL.len());
            for family in RiskTheaterFamily::ALL {
                assert_eq!(
                    value.risk_theater().get(&family),
                    Some(&QueryKnowledge::Unknown {
                        reason: QueryUnknownReason::LowerLayerUnverified,
                    })
                );
            }
            value
        }
        other => panic!("expected original native findings: {other:?}"),
    }
}

fn unknown(result: FileGovernanceFindingsResult) {
    assert_eq!(
        result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::ControllerUnavailable,
        })
    );
}

fn read_unchanged(
    supervisor: &FileActorSupervisor<FileOversight>,
    root: &Directory,
    port: &FileSupervisorGovernanceLedgerPort,
) -> FileGovernanceFindingsSnapshot {
    let before = supervisor.host().unwrap().inspect();
    let image = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let findings = known(port.findings());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(
        std::fs::read(root.store().join("delivery.bin")).unwrap(),
        image
    );
    assert_eq!(findings.basis().journal_revision, before.revision);
    assert_eq!(findings.basis().control_sequence, before.control.sequence);
    assert_eq!(
        findings.basis().authority_epoch,
        before.control.ledger.epoch
    );
    assert_eq!(findings.basis().recorded_at, before.control.ledger.elapsed);
    assert_eq!(findings.basis().scope, fixture::profile().delivery.scope);
    findings
}

#[test]
fn actual_false_stop_incidents_preserve_full_support_cost_and_recovered_history() {
    let root = Directory::new();
    let (host, evaluator) = evaluated(&root, 2);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.governance_ledger_port(4).unwrap();
    let initial = read_unchanged(&supervisor, &root, &port);
    assert!(!initial.credibility().unwrap().calibration_incident_open());
    assert!(initial.andon().unwrap().incidents().is_empty());
    assert_eq!(initial.andon().unwrap().inspected_windows(), 1);

    for id in [1, 2] {
        {
            let mut host = supervisor.host_mut().unwrap();
            fixture::review(&mut host, id, Verdict::Hold);
            // One repeated origin is not two independent population samples,
            // but these are TWO actual stopped rounds in the native budget.
            assert!(fixture::label(
                &mut host,
                &evaluator,
                id,
                7,
                GroundTruth::Benign
            ));
        }
        let findings = read_unchanged(&supervisor, &root, &port);
        let credibility = findings.credibility().unwrap();
        assert_eq!(credibility.lifetime_false_stops(), id);
        assert_eq!(credibility.protocol().false_stop_budget, 2);
        assert_eq!(credibility.calibration_incident_open(), id == 2);
        assert_eq!(
            findings.andon().unwrap().incidents().len(),
            usize::from(id == 2)
        );
        if id == 2 {
            let window = &findings.andon().unwrap().incidents()[&1];
            assert_eq!(window.false_stops, 2);
            assert_eq!(window.total_cost, 2 * u128::from(u64::MAX));
            let incident = window.incident.as_ref().unwrap();
            assert_eq!(incident.opened_by_round, 102);
            assert_eq!(incident.rounds, BTreeSet::from([101, 102]));
            let original = supervisor.host().unwrap();
            assert_eq!(window, &original.andon_ledger().unwrap().windows()[&1]);
        }
    }
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(!fixture::label(
            &mut host,
            &evaluator,
            2,
            7,
            GroundTruth::Benign
        ));
        assert_eq!(host.revision(), revision);
        let revision = host.revision();
        host.register_andon_window(
            revision,
            AndonBudget {
                window: 2,
                start: 100,
                end: 200,
                false_stops_allowed: 4,
                cost_per_stop: 3,
            },
        )
        .unwrap();
    }
    let retained = read_unchanged(&supervisor, &root, &port);
    assert_eq!(retained.andon().unwrap().inspected_windows(), 2);
    assert_eq!(retained.andon().unwrap().incidents().len(), 1);
    assert_eq!(initial.credibility().unwrap().lifetime_false_stops(), 0);
    assert!(initial.andon().unwrap().incidents().is_empty());
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    assert_eq!(grant.remaining_queries(), 0);

    drop(supervisor);
    unknown(port.findings());
    let (host, _) = FileOversight::open(root.store(), fixture::profile()).unwrap();
    let (_, reopened) = host.into_actor_gateway();
    let (fresh, _fresh_grant) = reopened.governance_ledger_port(1).unwrap();
    let recovered = read_unchanged(&reopened, &root, &fresh);
    assert_eq!(recovered.andon(), retained.andon());
    assert_eq!(recovered.credibility().unwrap().lifetime_false_stops(), 2);
    assert!(recovered.credibility().unwrap().calibration_incident_open());
    unknown(port.findings());
    for private in [
        "actual frozen helper context",
        "Approve this exact publication?",
        "exact-view-file-v1",
    ] {
        assert!(!format!("{recovered:?}").contains(private));
    }
}

#[test]
fn pending_censored_true_violations_and_allowed_benign_reviews_are_not_false_stop_incidents() {
    let root = Directory::new();
    let (host, evaluator) = evaluated(&root, 1);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.governance_ledger_port(5).unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        fixture::review(&mut host, 1, Verdict::Hold);
    }
    for truth in [
        None,
        Some(GroundTruth::Censored),
        Some(GroundTruth::Violation),
    ] {
        if let Some(truth) = truth {
            let mut host = supervisor.host_mut().unwrap();
            assert!(fixture::label(&mut host, &evaluator, 1, 1, truth));
        }
        let findings = read_unchanged(&supervisor, &root, &port);
        assert_eq!(findings.credibility().unwrap().lifetime_false_stops(), 0);
        assert!(!findings.credibility().unwrap().calibration_incident_open());
        assert!(findings.andon().unwrap().incidents().is_empty());
    }
    {
        let mut host = supervisor.host_mut().unwrap();
        fixture::review(&mut host, 2, Verdict::Allow);
        assert!(fixture::label(
            &mut host,
            &evaluator,
            2,
            2,
            GroundTruth::Benign
        ));
    }
    let allowed = read_unchanged(&supervisor, &root, &port);
    assert_eq!(allowed.credibility().unwrap().lifetime_false_stops(), 0);
    assert!(allowed.andon().unwrap().incidents().is_empty());
    {
        let mut host = supervisor.host_mut().unwrap();
        fixture::review(&mut host, 3, Verdict::Hold);
        assert!(fixture::label(
            &mut host,
            &evaluator,
            3,
            3,
            GroundTruth::Benign
        ));
    }
    let actual = read_unchanged(&supervisor, &root, &port);
    assert!(actual.credibility().unwrap().calibration_incident_open());
    assert_eq!(
        actual.andon().unwrap().incidents()[&1]
            .incident
            .as_ref()
            .unwrap()
            .rounds,
        BTreeSet::from([103])
    );
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn ledger_and_findings_share_one_nonrenewable_disclosure_allowance() {
    let root = Directory::new();
    let (host, _) = evaluated(&root, 1);
    let (_, supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.governance_ledger_port(2).unwrap();
    let cloned = port.clone();
    assert!(matches!(port.ledger(), Ok(Knowledge::Known { .. })));
    known(cloned.findings());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.findings(), Err(ActorError::Capacity));
    assert_eq!(cloned.clone().ledger(), Err(ActorError::Capacity));
    let (revocable, custody) = supervisor.governance_ledger_port(2).unwrap();
    known(revocable.findings());
    custody.revoke();
    assert_eq!(
        revocable.findings(),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_governance_ledger",
        })
    );
    assert_eq!(
        revocable.clone().findings(),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_governance_ledger",
        })
    );
}

#[test]
fn unavailable_original_owner_never_becomes_cached_or_clean_findings() {
    for fail_storage in [false, true] {
        let root = Directory::new();
        let (host, evaluator) = evaluated(&root, 1);
        let (_, mut supervisor) = host.into_actor_gateway();
        {
            let mut host = supervisor.host_mut().unwrap();
            fixture::review(&mut host, 1, Verdict::Hold);
            assert!(fixture::label(
                &mut host,
                &evaluator,
                1,
                1,
                GroundTruth::Benign
            ));
        }
        let (port, grant) = supervisor.governance_ledger_port(2).unwrap();
        assert!(
            known(port.findings())
                .credibility()
                .unwrap()
                .calibration_incident_open()
        );
        {
            let mut host = supervisor.host_mut().unwrap();
            unknown(port.findings());
            assert_eq!(grant.remaining_queries(), 1);
            if fail_storage {
                std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
                let revision = host.revision();
                assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
            }
        }
        if fail_storage {
            unknown(port.findings());
            assert_eq!(grant.remaining_queries(), 1);
        } else {
            assert!(
                known(port.findings())
                    .credibility()
                    .unwrap()
                    .calibration_incident_open()
            );
        }
        let remaining = grant.remaining_queries();
        drop(supervisor);
        unknown(port.findings());
        assert_eq!(grant.remaining_queries(), remaining);
    }
}

#[test]
fn missing_native_sources_remain_distinct_from_installed_empty_incident_queries() {
    let root = Directory::new();
    let (host, _) = FileOversight::create(root.store(), fixture::profile()).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.governance_ledger_port(2).unwrap();
    let absent = read_unchanged(&supervisor, &root, &port);
    assert!(absent.credibility().is_none());
    assert!(absent.andon().is_none());
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.enable_credibility(revision, fixture::protocol())
            .unwrap();
    }
    let configured = read_unchanged(&supervisor, &root, &port);
    assert_eq!(configured.credibility().unwrap().lifetime_false_stops(), 0);
    assert!(
        !configured
            .credibility()
            .unwrap()
            .calibration_incident_open()
    );
    assert!(configured.andon().is_none());
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn foreign_evaluator_cannot_open_native_findings_but_original_evaluator_can() {
    let root = Directory::new();
    let other = Directory::new();
    let (mut host, evaluator) = evaluated(&root, 1);
    let (_foreign_host, foreign_evaluator) = evaluated(&other, 1);
    fixture::review(&mut host, 1, Verdict::Hold);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.governance_ledger_port(3).unwrap();
    let before = read_unchanged(&supervisor, &root, &port);
    {
        let mut host = supervisor.host_mut().unwrap();
        let ticket = host.evaluation_ticket(101).unwrap();
        let revision = host.revision();
        assert!(matches!(
            foreign_evaluator.assess(
                &mut host,
                revision,
                &ticket,
                fixture::assessment(1, GroundTruth::Benign),
            ),
            Err(JournalError::Contract(Error::Binding))
        ));
    }
    assert_eq!(read_unchanged(&supervisor, &root, &port), before);
    {
        let mut host = supervisor.host_mut().unwrap();
        assert!(fixture::label(
            &mut host,
            &evaluator,
            1,
            1,
            GroundTruth::Benign
        ));
    }
    let after = read_unchanged(&supervisor, &root, &port);
    assert!(after.credibility().unwrap().calibration_incident_open());
    assert_eq!(
        after.andon().unwrap().incidents()[&1]
            .incident
            .as_ref()
            .unwrap()
            .rounds,
        BTreeSet::from([101])
    );
    assert!(before.andon().unwrap().incidents().is_empty());
}
