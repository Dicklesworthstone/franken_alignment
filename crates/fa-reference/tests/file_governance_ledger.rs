//! Original acknowledged credibility and Andon accounting through finite weak
//! disclosure custody. The fixtures perform real native reviews and independent
//! assessments; these observations do not establish detector quality.
#![cfg(unix)]
#![forbid(unsafe_code)]

#[path = "support/file_credibility.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::FileIndependentEvaluator;
use fa_reference::action::consequence::delivery::persistent::observed::credibility::andon::FileAndonConfig;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileHumanReviewer, FileOversight,
};
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    FileActorSupervisor, FileGovernanceLedgerResult, FileGovernanceLedgerSnapshot,
    FileSupervisorGovernanceLedgerPort, MAX_GOVERNANCE_LEDGER_QUERIES,
};
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::oversight::actor::{ActorError, Knowledge, UnknownReason};
use fa_reference::action::consequence::oversight::credibility::{
    GroundTruth, QualificationFailure,
};
use fa_reference::round::Verdict;
use fa_reference::round::review::andon::{AndonBudget, LedgerIdentity};
use fixture::{Directory, ordinary};
use std::collections::BTreeSet;

fn andon() -> FileAndonConfig {
    FileAndonConfig::new(
        LedgerIdentity {
            deployment: 101,
            evaluator: 202,
        },
        vec![AndonBudget {
            window: 1,
            start: 0,
            end: 100,
            false_stops_allowed: 2,
            cost_per_stop: u64::MAX,
        }],
    )
    .unwrap()
}

fn evaluated(
    root: &Directory,
    with_andon: bool,
) -> (FileOversight, FileHumanReviewer, FileIndependentEvaluator) {
    let (mut host, human) = FileOversight::create(root.store(), fixture::profile()).unwrap();
    let mut protocol = fixture::protocol();
    protocol.false_stop_budget = 2;
    let evaluator = host.enable_credibility(host.revision(), protocol).unwrap();
    if with_andon {
        host.enable_andon(host.revision(), &andon()).unwrap();
    }
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    (host, human, evaluator)
}

fn image(root: &Directory) -> Vec<u8> {
    std::fs::read(root.store().join("delivery.bin")).unwrap()
}

fn known(result: FileGovernanceLedgerResult) -> FileGovernanceLedgerSnapshot {
    match result.unwrap() {
        Knowledge::Known { value, basis } => {
            assert_eq!(value.basis(), basis);
            value
        }
        other => panic!("expected original acknowledged governance snapshot: {other:?}"),
    }
}

fn unknown(result: FileGovernanceLedgerResult) {
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
) -> FileGovernanceLedgerSnapshot {
    let before = supervisor.host().unwrap().inspect();
    let bytes = image(root);
    let value = known(port.ledger());
    assert_eq!(supervisor.host().unwrap().inspect(), before);
    assert_eq!(image(root), bytes);
    assert_eq!(value.basis().journal_revision, before.revision);
    assert_eq!(value.basis().control_sequence, before.control.sequence);
    assert_eq!(value.basis().authority_epoch, before.control.ledger.epoch);
    assert_eq!(value.basis().recorded_at, before.control.ledger.elapsed);
    assert_eq!(value.basis().scope, fixture::profile().delivery.scope);
    value
}

#[test]
fn actual_independent_labels_remain_visible_with_original_denominators_and_no_publication_power() {
    let root = Directory::new();
    let (host, human, evaluator) = evaluated(&root, false);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.governance_ledger_port(7).unwrap();
    let initial = read_unchanged(&supervisor, &root, &port);
    assert!(initial.andon().is_none());
    assert_eq!(initial.credibility().unwrap().retained_cases(), 0);

    {
        let mut host = supervisor.host_mut().unwrap();
        fixture::review(&mut host, 1, Verdict::Hold);
    }
    let pending = read_unchanged(&supervisor, &root, &port);
    assert_eq!(pending.credibility().unwrap().pending_cases(), 1);
    assert!(
        pending
            .credibility()
            .unwrap()
            .failures()
            .contains(&QualificationFailure::PendingLabels)
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        assert!(fixture::label(
            &mut host,
            &evaluator,
            1,
            7,
            GroundTruth::Censored
        ));
    }
    let censored = read_unchanged(&supervisor, &root, &port);
    let report = censored.credibility().unwrap();
    assert_eq!(report.pending_cases(), 0);
    assert_eq!(report.censored_cases(), 1);
    assert_eq!(report.violation_origins(), 0);
    assert_eq!(report.benign_origins(), 0);
    assert!(
        report
            .failures()
            .contains(&QualificationFailure::CensoredLabels)
    );

    {
        let mut host = supervisor.host_mut().unwrap();
        assert!(fixture::label(
            &mut host,
            &evaluator,
            1,
            7,
            GroundTruth::Violation
        ));
    }
    let resolved = read_unchanged(&supervisor, &root, &port);
    assert_eq!(resolved.credibility().unwrap().censored_cases(), 0);
    assert_eq!(resolved.credibility().unwrap().violation_origins(), 1);
    assert_eq!(
        resolved.credibility().unwrap().members()["alpha"].true_positives,
        1
    );

    // Repeating the same independent origin does not multiply its evidence.
    {
        let mut host = supervisor.host_mut().unwrap();
        fixture::review(&mut host, 2, Verdict::Hold);
        assert!(fixture::label(
            &mut host,
            &evaluator,
            2,
            7,
            GroundTruth::Violation
        ));
    }
    let repeated = read_unchanged(&supervisor, &root, &port);
    assert_eq!(repeated.credibility().unwrap().retained_cases(), 2);
    assert_eq!(repeated.credibility().unwrap().violation_origins(), 1);
    assert_eq!(
        repeated.credibility().unwrap().members()["alpha"].true_positives,
        1
    );
    {
        let mut host = supervisor.host_mut().unwrap();
        fixture::review(&mut host, 3, Verdict::Allow);
        assert!(fixture::label(
            &mut host,
            &evaluator,
            3,
            8,
            GroundTruth::Benign
        ));
    }
    let qualified = read_unchanged(&supervisor, &root, &port);
    let native = supervisor.host().unwrap().credibility_report().unwrap();
    let report = qualified.credibility().unwrap();
    assert_eq!(report.protocol(), &native.protocol);
    assert_eq!(report.revision(), native.revision);
    assert_eq!(report.policy_generation(), native.policy_generation);
    assert_eq!(report.members(), &native.members);
    assert_eq!(report.failures(), native.failures.as_slice());
    assert!(report.failures().is_empty());
    assert_eq!((report.scoped_cases(), report.benign_origins()), (3, 1));
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);

    // A later actual effect still needs original helper AND independent human
    // keys; the read produced neither. Use the unchanged two-key native path.
    {
        let mut host = supervisor.host_mut().unwrap();
        let keys = ordinary::ready(&mut host, &human, 4, b"fresh effect");
        ordinary::dispatch(&mut host, &keys);
        let revision = host.revision();
        let outcome = host
            .publish_checked(
                revision,
                4,
                Some(&keys.inputs),
                fixture::snapshot(),
                ElapsedTick(1),
            )
            .unwrap()
            .outcome;
        assert_eq!(
            outcome,
            EndpointOutcome::Executed {
                resulting_version: 2
            }
        );
        let revision = host.revision();
        assert_eq!(
            host.reconcile(revision, 4).unwrap(),
            Reconciliation::Resolved(outcome)
        );
    }
    let after = read_unchanged(&supervisor, &root, &port);
    assert_eq!(after.credibility().unwrap().pending_cases(), 1);
    assert_eq!(supervisor.host().unwrap().inspect().executions, 1);
    assert!(after.basis().journal_revision > initial.basis().journal_revision);
    assert_eq!(initial.credibility().unwrap().retained_cases(), 0);
    assert_eq!(pending.credibility().unwrap().pending_cases(), 1);
    assert_eq!(grant.remaining_queries(), 0);
    for private in [
        "Approve this exact publication?",
        "exact-view-file-v1",
        "actual frozen helper context",
        "fresh effect",
    ] {
        assert!(!format!("{after:?}").contains(private));
    }
}

#[test]
fn real_false_holds_preserve_full_width_cost_and_incident_without_changing_authority() {
    let root = Directory::new();
    let (host, _, evaluator) = evaluated(&root, true);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.governance_ledger_port(4).unwrap();
    let initial = read_unchanged(&supervisor, &root, &port);
    assert_eq!(initial.andon().unwrap().identity(), andon().identity());
    assert_eq!(initial.andon().unwrap().windows()[&1].false_stops, 0);

    for id in [1, 2] {
        {
            let mut host = supervisor.host_mut().unwrap();
            fixture::review(&mut host, id, Verdict::Hold);
            assert!(fixture::label(
                &mut host,
                &evaluator,
                id,
                id,
                GroundTruth::Benign
            ));
        }
        let captured = read_unchanged(&supervisor, &root, &port);
        let window = &captured.andon().unwrap().windows()[&1];
        assert_eq!(window.false_stops, id);
        assert_eq!(window.total_cost, u128::from(id) * u128::from(u64::MAX));
        assert_eq!(captured.credibility().unwrap().lifetime_false_stops(), id);
        assert_eq!(
            captured.credibility().unwrap().members()["alpha"].false_positives,
            id
        );
        assert_eq!(window.incident.is_some(), id == 2);
        if id == 2 {
            let incident = window.incident.as_ref().unwrap();
            assert_eq!(incident.opened_by_round, 102);
            assert_eq!(incident.rounds, BTreeSet::from([101, 102]));
            assert!(captured.credibility().unwrap().calibration_incident_open());
            assert!(
                captured
                    .credibility()
                    .unwrap()
                    .failures()
                    .contains(&QualificationFailure::FalseStopBudgetExhausted)
            );
        }
        assert_eq!(
            supervisor.host().unwrap().andon_ledger().unwrap().windows(),
            captured.andon().unwrap().windows()
        );
    }
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        assert!(!fixture::label(
            &mut host,
            &evaluator,
            2,
            2,
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
    assert_eq!(retained.andon().unwrap().windows().len(), 2);
    assert_eq!(
        retained.andon().unwrap().windows()[&1].total_cost,
        2 * u128::from(u64::MAX)
    );
    assert_eq!(retained.andon().unwrap().windows()[&2].false_stops, 0);
    assert_eq!(initial.andon().unwrap().windows()[&1].false_stops, 0);
    assert_eq!(
        supervisor.host().unwrap().inspect().control.ledger.charged,
        0
    );
    assert_eq!(supervisor.host().unwrap().inspect().executions, 0);
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn uninstalled_subsystems_are_observed_separately_from_empty_configured_accounting() {
    let root = Directory::new();
    let (host, _) = FileOversight::create(root.store(), fixture::profile()).unwrap();
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, grant) = supervisor.governance_ledger_port(3).unwrap();
    let absent = read_unchanged(&supervisor, &root, &port);
    assert!(absent.credibility().is_none());
    assert!(absent.andon().is_none());
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.enable_credibility(revision, fixture::protocol())
            .unwrap();
    }
    let evaluated = read_unchanged(&supervisor, &root, &port);
    assert_eq!(evaluated.credibility().unwrap().retained_cases(), 0);
    assert!(evaluated.andon().is_none());
    {
        let mut host = supervisor.host_mut().unwrap();
        let revision = host.revision();
        host.enable_andon(revision, &andon()).unwrap();
    }
    let configured = read_unchanged(&supervisor, &root, &port);
    assert!(configured.credibility().is_some());
    assert_eq!(configured.andon().unwrap().windows().len(), 1);
    assert_eq!(configured.andon().unwrap().windows()[&1].false_stops, 0);
    assert!(absent.credibility().is_none());
    assert!(evaluated.andon().is_none());
    assert_eq!(grant.remaining_queries(), 0);
}

#[test]
fn finite_shared_queries_and_custody_revocation_cannot_be_renewed_by_cloning() {
    let root = Directory::new();
    let (host, _, _) = evaluated(&root, true);
    let (_, supervisor) = host.into_actor_gateway();
    assert!(matches!(
        supervisor.governance_ledger_port(0),
        Err(JournalError::Contract(Error::InvalidInput))
    ));
    assert!(matches!(
        supervisor.governance_ledger_port(MAX_GOVERNANCE_LEDGER_QUERIES + 1),
        Err(JournalError::Contract(Error::Limit))
    ));
    let (_maximum, maximum_grant) = supervisor
        .governance_ledger_port(MAX_GOVERNANCE_LEDGER_QUERIES)
        .unwrap();
    assert_eq!(
        maximum_grant.remaining_queries(),
        MAX_GOVERNANCE_LEDGER_QUERIES
    );
    let (port, grant) = supervisor.governance_ledger_port(2).unwrap();
    let clone = port.clone();
    known(port.ledger());
    known(clone.ledger());
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(port.ledger(), Err(ActorError::Capacity));
    assert_eq!(clone.clone().ledger(), Err(ActorError::Capacity));
    let (revocable, custody) = supervisor.governance_ledger_port(2).unwrap();
    known(revocable.ledger());
    custody.revoke();
    assert_eq!(
        revocable.ledger(),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_governance_ledger"
        })
    );
    assert_eq!(
        revocable.clone().ledger(),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_governance_ledger"
        })
    );
    let (dropped, custody) = supervisor.governance_ledger_port(1).unwrap();
    drop(custody);
    assert_eq!(
        dropped.ledger(),
        Ok(Knowledge::Withheld {
            authority_required: "supervisor_governance_ledger"
        })
    );
}

#[test]
fn borrow_storage_fault_and_recovery_never_return_a_cached_original_ledger() {
    for faulted in [false, true] {
        let root = Directory::new();
        let (host, _, _) = evaluated(&root, true);
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.governance_ledger_port(3).unwrap();
        known(port.ledger());
        {
            let mut host = supervisor.host_mut().unwrap();
            unknown(port.ledger());
            assert_eq!(grant.remaining_queries(), 2);
            if faulted {
                std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
            }
            let revision = host.revision();
            assert_eq!(
                host.observe_time(revision, ElapsedTick(2)).is_err(),
                faulted
            );
        }
        if faulted {
            unknown(port.ledger());
            assert_eq!(grant.remaining_queries(), 2);
            assert!(matches!(
                supervisor.governance_ledger_port(1),
                Err(JournalError::Unavailable)
            ));
        } else {
            assert_eq!(
                known(port.ledger()).basis().recorded_at,
                Some(ElapsedTick(2))
            );
            assert_eq!(grant.remaining_queries(), 1);
        }
        let remaining = grant.remaining_queries();
        let before = image(&root);
        drop(supervisor);
        unknown(port.ledger());
        assert_eq!(image(&root), before);
        // A surviving port/grant holds no authority lock. Only a fresh native
        // owner and an explicitly fresh grant may observe the recovered ledger.
        let (host, _) = FileOversight::open(root.store(), fixture::profile()).unwrap();
        let (_, reopened) = host.into_actor_gateway();
        unknown(port.ledger());
        let (fresh, _fresh_grant) = reopened.governance_ledger_port(1).unwrap();
        assert!(known(fresh.ledger()).andon().is_some());
        assert_eq!(grant.remaining_queries(), remaining);
    }
}

#[test]
fn equal_scope_or_changed_scope_replacements_cannot_impersonate_the_original_owner() {
    for changed_field in 0..=5 {
        let root = Directory::new();
        let foreign_root = Directory::new();
        let (host, _, _) = evaluated(&root, true);
        let (_, mut supervisor) = host.into_actor_gateway();
        let (port, grant) = supervisor.governance_ledger_port(2).unwrap();
        let original = known(port.ledger());
        let mut foreign_profile = fixture::profile();
        match changed_field {
            0 => {}
            1 => foreign_profile.delivery.scope.tenant += 1,
            2 => foreign_profile.delivery.scope.principal += 1,
            3 => foreign_profile.delivery.scope.run += 1,
            4 => foreign_profile.delivery.scope.branch += 1,
            5 => foreign_profile.delivery.scope.authority += 1,
            _ => unreachable!(),
        }
        let expected = foreign_profile.delivery.scope;
        let (foreign, _) = FileOversight::create(foreign_root.store(), foreign_profile).unwrap();
        let retained_original = {
            let mut slot = supervisor.host_mut().unwrap();
            std::mem::replace(&mut *slot, foreign)
        };
        // Keep the original issuer alive: refusal must bind the actual owner,
        // not just notice that the old object's weak issuer has expired.
        assert!(retained_original.andon_config().unwrap().is_some());
        unknown(port.ledger());
        assert_eq!(grant.remaining_queries(), 1);
        let (fresh, _custody) = supervisor.governance_ledger_port(1).unwrap();
        let replacement = known(fresh.ledger());
        assert_eq!(replacement.basis().scope, expected);
        assert!(replacement.credibility().is_none());
        assert!(original.credibility().is_some());
    }
}

#[test]
fn rejected_foreign_label_cannot_change_accounting_but_original_evaluator_can() {
    let root = Directory::new();
    let other_root = Directory::new();
    let (mut host, _, evaluator) = evaluated(&root, true);
    let (mut foreign, _, foreign_evaluator) = evaluated(&other_root, true);
    fixture::review(&mut host, 1, Verdict::Hold);
    fixture::review(&mut foreign, 1, Verdict::Hold);
    let (_, mut supervisor) = host.into_actor_gateway();
    let (port, _grant) = supervisor.governance_ledger_port(3).unwrap();
    let pending = read_unchanged(&supervisor, &root, &port);
    {
        let mut host = supervisor.host_mut().unwrap();
        let ticket = host.evaluation_ticket(101).unwrap();
        let revision = host.revision();
        assert!(matches!(
            foreign_evaluator.assess(
                &mut host,
                revision,
                &ticket,
                fixture::assessment(1, GroundTruth::Benign)
            ),
            Err(JournalError::Contract(Error::Binding))
        ));
    }
    assert_eq!(read_unchanged(&supervisor, &root, &port), pending);
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
    let accepted = read_unchanged(&supervisor, &root, &port);
    assert_eq!(accepted.credibility().unwrap().pending_cases(), 0);
    assert_eq!(accepted.credibility().unwrap().benign_origins(), 1);
    assert_eq!(accepted.andon().unwrap().windows()[&1].false_stops, 1);
    assert_eq!(pending.andon().unwrap().windows()[&1].false_stops, 0);
}
