//! The original checked owner, native evaluator and exact-history recovery.
//! Synthetic votes test composition, not evaluator accuracy or independence.
#![forbid(unsafe_code)]
#[path = "support/file_oversight.rs"]
mod fixture;

use fixture::{Directory, profile, snapshot};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::{RecoveryReserve,
    observed::{FileOversight, credibility::andon::FileAndonConfig,
        guarded::{FileGuardSet, FileRecoveryFloor, FileRecoveryRequirements,
            evaluation::{FileEvaluatedOversightRoles, andon::FileAndonRecoveryRequirements}}}};
use fa_reference::action::consequence::delivery::publication_gate::PublicationLimits;
use fa_reference::action::consequence::oversight::credibility::{Assessment, EvaluationProtocol, Fraction, GroundTruth};
use fa_reference::action::consequence::oversight::joint_credibility::{JointPromotionPolicy, JointReplayBudget};
use fa_reference::round::{Verdict, review::andon::{AndonBudget, LedgerIdentity}};
use fa_reference::witness::refinement::RefinementBudget;

fn protocol() -> EvaluationProtocol {
    EvaluationProtocol { domain: 1, stratum: 2, period: 3,
        minimum_violation_origins: 1, minimum_benign_origins: 1,
        precision_floor: Fraction { numerator: 1, denominator: 2 },
        recall_floor: Fraction { numerator: 1, denominator: 2 },
        false_positive_ceiling: Fraction { numerator: 1, denominator: 2 }, false_stop_budget: 10 }
}
fn joint() -> JointPromotionPolicy {
    JointPromotionPolicy::new(71, 1, Fraction { numerator: 0, denominator: 1 },
        Fraction { numerator: 1, denominator: 2 }, JointReplayBudget::default()).unwrap()
}
fn config() -> FileAndonConfig {
    FileAndonConfig::new(LedgerIdentity { deployment: 101, evaluator: 202 }, vec![AndonBudget {
        window: 1, start: 0, end: 20, false_stops_allowed: 0, cost_per_stop: 7,
    }]).unwrap()
}
fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn limits() -> PublicationLimits {
    PublicationLimits { bindings: 8, validation: RefinementBudget { steps: 32, value_bytes: 4096 } }
}
fn create(root: &Directory, coupled: bool, reserved: bool)
    -> (FileOversight, FileEvaluatedOversightRoles)
{
    match (coupled, reserved) {
        (false, false) => FileOversight::create_checked_andon_guarded(root.store(), profile(),
            &guards(), None, protocol(), &config(), limits()),
        (false, true) => FileOversight::create_checked_andon_guarded_reserved(root.store(), profile(),
            &guards(), None, protocol(), &config(), limits(), RecoveryReserve::terminal()),
        (true, false) => FileOversight::create_checked_joint_andon_guarded(root.store(), profile(),
            &guards(), None, protocol(), joint(), &config(), limits()),
        (true, true) => FileOversight::create_checked_joint_andon_guarded_reserved(root.store(), profile(),
            &guards(), None, protocol(), joint(), &config(), limits(), RecoveryReserve::terminal()),
    }.unwrap()
}
fn expected(host: &FileOversight, coupled: bool) -> FileAndonRecoveryRequirements {
    let state = host.inspect();
    let base = FileRecoveryRequirements { guards: guards(), effective_policy: profile().delivery.policy,
        credential_epoch: None, minimum: FileRecoveryFloor { journal_revision: state.revision,
            control_sequence: state.control.sequence, authority_epoch: state.control.ledger.epoch } };
    let expected = FileAndonRecoveryRequirements::new(base, config(), config().windows().to_vec(),
        host.credibility_report().unwrap().revision).unwrap().require_publication(limits()).unwrap();
    if coupled { expected.require_joint(joint()).unwrap() } else { expected }
}

#[test]
fn all_four_bootstraps_and_recovery_preserve_the_exact_selected_gates() {
    for coupled in [false, true] {
        for reserved in [false, true] {
            let root = Directory::new(); let (host, _) = create(&root, coupled, reserved);
            assert_eq!(host.revision(), if reserved { 4 } else { 3 });
            assert_eq!(host.publication_validation_profile().unwrap(), Some(limits()));
            assert_eq!(host.journal_capacity().unwrap().reserve(), reserved.then(RecoveryReserve::terminal));
            assert_eq!(host.joint_credibility_policy().unwrap(), coupled.then(joint));
            let requirements = expected(&host, coupled); let anchor = host.history_anchor().unwrap();
            let original = std::fs::read(root.store().join("delivery.bin")).unwrap();
            let historical = FileOversight::read_andon_guarded(root.store(), &profile(),
                &requirements, &protocol(), &anchor).unwrap();
            assert_eq!(historical.credibility.journal, host.inspect());
            assert_eq!(std::fs::read(root.store().join("delivery.bin")).unwrap(), original);
            drop(host);
            let (host, _) = FileOversight::open_andon_guarded(root.store(), profile(),
                &requirements, &protocol(), &anchor).unwrap();
            assert_eq!(host.publication_validation_profile().unwrap(), Some(limits()));
            assert_eq!(host.journal_capacity().unwrap().reserve(), reserved.then(RecoveryReserve::terminal));
            assert_eq!(host.joint_credibility_policy().unwrap(), coupled.then(joint));
            assert_eq!(host.andon_ledger().unwrap(), &historical.accounting);
            assert!(!host.clock_ready());
        }
    }
}

#[test]
fn checked_case_survives_censoring_and_recovery_then_charges_once_without_changing_authority() {
    for coupled in [false, true] {
        let root = Directory::new(); let (mut host, old_roles) = create(&root, coupled, false);
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let action = host.propose(host.revision(), 1, fixture::spec(&host, b"checked"), snapshot()).unwrap();
        let inputs = fixture::inputs(&action, b"complete original source");
        host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
        host.begin_review(host.revision(), 1, 101, fixture::ROOT, fixture::window(&host), snapshot()).unwrap();
        fixture::commit(&mut host, 101, "alpha", Verdict::Hold);
        fixture::commit(&mut host, 101, "beta", Verdict::Allow);
        host.open_reveals(host.revision(), 101).unwrap();
        for (member, vote) in [("alpha", Verdict::Hold), ("beta", Verdict::Allow)] {
            host.reveal_review(host.revision(), 101, member, vote, fixture::salt(member)).unwrap();
        }
        host.finish_review(host.revision(), 101, Some(&inputs), snapshot()).unwrap().unwrap();
        let old_ticket = host.evaluation_ticket(101).unwrap(); assert!(old_ticket.case().stopped);
        let censored = Assessment { origin: 17, evidence_id: [17; 32], truth: GroundTruth::Censored };
        let revision = host.revision(); old_roles.evaluator.assess(&mut host, revision, &old_ticket, censored).unwrap();
        let requirements = expected(&host, coupled); let anchor = host.history_anchor().unwrap(); drop(host);
        let (mut host, roles) = FileOversight::open_andon_guarded(root.store(), profile(),
            &requirements, &protocol(), &anchor).unwrap();
        let benign = Assessment { truth: GroundTruth::Benign, ..censored };
        let revision = host.revision();
        assert!(old_roles.evaluator.assess(&mut host, revision, &old_ticket, benign).is_err());
        let ticket = host.evaluation_ticket(101).unwrap();
        assert!(roles.evaluator.assess(&mut host, revision, &ticket, benign).is_err());
        assert_eq!(host.evaluation_history(101).unwrap(), &[censored]);
        host.observe_time(host.revision(), ElapsedTick(5)).unwrap();
        let before = host.inspect(); let revision = host.revision();
        assert!(roles.evaluator.assess(&mut host, revision, &ticket, benign).unwrap());
        assert_eq!(host.inspect().control, before.control);
        assert_eq!(host.inspect().executions, 0);
        assert_eq!(host.publication_validation_profile().unwrap(), Some(limits()));
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].false_stops, 1);
        assert_eq!(host.andon_ledger().unwrap().windows()[&1].total_cost, 7);
        let accounted = host.andon_ledger().unwrap().clone(); let revision = host.revision();
        assert!(!roles.evaluator.assess(&mut host, revision, &ticket, benign).unwrap());
        assert_eq!(host.revision(), revision); assert_eq!(host.andon_ledger().unwrap(), &accounted);
    }
}

#[test]
fn mismatched_checked_recovery_preserves_canonical_and_unacknowledged_staging() {
    let root = Directory::new(); let (host, _) = create(&root, false, false);
    let expected = expected(&host, false); let anchor = host.history_anchor().unwrap(); drop(host);
    let canonical = root.store().join("delivery.bin"); let bytes = std::fs::read(&canonical).unwrap();
    let staging = root.store().join("delivery.pending"); std::fs::write(&staging, b"keep this evidence").unwrap();
    for field in 0..4 {
        let base = FileAndonRecoveryRequirements::new(expected.base().clone(), config(),
            config().windows().to_vec(), expected.minimum_evaluation_revision()).unwrap();
        let mut different = limits();
        match field { 0 => {}, 1 => different.bindings += 1,
            2 => different.validation.steps += 1, _ => different.validation.value_bytes += 1 }
        let wrong = if field == 0 { base } else { base.require_publication(different).unwrap() };
        assert!(FileOversight::begin_open_andon_guarded(root.store(), profile(), &wrong, &protocol(), &anchor).is_err());
        assert_eq!(std::fs::read(&canonical).unwrap(), bytes);
        assert_eq!(std::fs::read(&staging).unwrap(), b"keep this evidence");
    }
    let (host, _) = FileOversight::open_andon_guarded(root.store(), profile(), &expected, &protocol(), &anchor).unwrap();
    assert_eq!(host.publication_validation_profile().unwrap(), Some(limits()));
}

#[test]
fn invalid_checked_bootstrap_fails_before_directory_creation() {
    for coupled in [false, true] {
        let root = Directory::new(); let mut invalid = limits(); invalid.bindings = 0;
        let result = if coupled {
            FileOversight::create_checked_joint_andon_guarded(root.store(), profile(), &guards(),
                None, protocol(), joint(), &config(), invalid)
        } else {
            FileOversight::create_checked_andon_guarded(root.store(), profile(), &guards(),
                None, protocol(), &config(), invalid)
        };
        assert!(result.is_err()); assert!(!root.store().exists());
        let (host, _) = create(&root, coupled, false);
        assert_eq!(host.publication_validation_profile().unwrap(), Some(limits()));
    }
}
