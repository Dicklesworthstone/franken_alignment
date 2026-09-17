//! Preserve independent labels and every guard across atomic startup/recovery.
#![cfg(unix)]
#[path = "support/file_credibility.rs"] mod fixture;
#[path = "support/file_decoder.rs"] mod numerical;
use fixture::*;
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{EndpointOutcome, StopRequest};
use fa_reference::action::consequence::delivery::persistent::{JournalError, PolicyUpdate};
use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::*;
use fa_reference::action::consequence::gate::containment::session::policy::Policy;
use fa_reference::action::consequence::oversight::credibility::GroundTruth;
use fa_reference::round::Verdict;
use fa_reference::Error;

fn guards() -> FileGuardSet {
    FileGuardSet { stream: None, decoder: None, decoder_stop: None, source: None,
        identity: None, campaigns: None, credential: None }
}
fn expected(host: &FileOversight, guards: FileGuardSet) -> FileRecoveryRequirements {
    let c = host.inspect().control;
    FileRecoveryRequirements { guards, effective_policy: host.current_policy().unwrap().clone(), credential_epoch: None,
        minimum: FileRecoveryFloor { journal_revision: host.revision(), control_sequence: c.sequence, authority_epoch: c.ledger.epoch } }
}

#[test]
fn independent_labels_continue_after_one_guarded_recovery_then_native_promotion_and_publication() {
    let root = Directory::new(); let g = guards();
    let (mut host, roles) = FileOversight::create_evaluated_guarded(root.store(), profile(), &g, None, protocol()).unwrap();
    assert_eq!(host.revision(), 2); assert!(!host.clock_ready());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    review(&mut host, 1, Verdict::Hold); label(&mut host, &roles.evaluator, 1, 1, GroundTruth::Violation);
    review(&mut host, 2, Verdict::Allow); label(&mut host, &roles.evaluator, 2, 2, GroundTruth::Censored);
    let old_ticket = host.evaluation_ticket(102).unwrap();
    let requirement = expected(&host, g); let report = host.credibility_report().unwrap(); drop(host);
    let (mut host, fresh) = FileOversight::open_evaluated_guarded(root.store(), profile(), &requirement, &protocol()).unwrap();
    assert_eq!(host.revision(), requirement.minimum.journal_revision + 1);
    assert_eq!(host.credibility_report().unwrap(), report); assert!(!host.clock_ready());
    let ticket = host.evaluation_ticket(102).unwrap(); let r = host.revision();
    assert_eq!(roles.evaluator.assess(&mut host, r, &ticket, assessment(2, GroundTruth::Benign)), Err(JournalError::Contract(Error::Binding)));
    assert_eq!(fresh.evaluator.assess(&mut host, r, &old_ticket, assessment(2, GroundTruth::Benign)), Err(JournalError::Contract(Error::Binding)));
    assert!(fresh.evaluator.assess(&mut host, r, &ticket, assessment(2, GroundTruth::Benign)).unwrap());
    assert!(!host.clock_ready()); assert!(host.credibility_report().unwrap().qualified());
    let request = update(&host, 1);
    assert_eq!(host.promote_credibility(host.revision(), &request), Err(JournalError::Contract(Error::Incomplete)));
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    let request = update(&host, 1); host.promote_credibility(host.revision(), &request).unwrap();
    let keys = ordinary::ready(&mut host, &fresh.oversight.human, 3, b"new review after labeled promotion");
    ordinary::dispatch(&mut host, &keys);
    assert_eq!(host.publish_checked(host.revision(), 3, Some(&keys.inputs), snapshot(), ElapsedTick(2)).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    let disk = FileOversight::read_credibility(root.store(), &profile(), &requirement, &protocol()).unwrap();
    assert_eq!(disk.journal, host.inspect()); assert_eq!(disk.report, host.credibility_report().unwrap());
    assert_eq!(disk.promotions.len(), 1);
}

#[test]
fn exact_evaluation_contract_floors_and_entire_suffix_are_required_without_cleanup() {
    let root = Directory::new(); let g = guards();
    let (mut host, roles) = FileOversight::create_evaluated_guarded(root.store(), profile(), &g, None, protocol()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); qualified(&mut host, &roles.evaluator);
    let requirement = expected(&host, g); let original_report = host.credibility_report().unwrap();
    let path = root.store().join("delivery.bin"); let pending = root.store().join("delivery.pending");
    let bytes = std::fs::read(&path).unwrap(); std::fs::write(&pending, b"preserve").unwrap();
    let disk = FileOversight::read_credibility(root.store(), &profile(), &requirement, &protocol()).unwrap();
    assert_eq!(disk.report, original_report);
    assert!(host.observe_time(host.revision(), ElapsedTick(2)).is_err());
    assert_eq!(host.credibility_report(), Err(JournalError::Unavailable));
    assert_eq!(FileOversight::read_credibility(root.store(), &profile(), &requirement, &protocol()).unwrap().report, original_report);
    drop(host);
    for variant in 0..12 {
        let mut wrong = protocol();
        match variant {
            0 => wrong.domain += 1, 1 => wrong.stratum += 1, 2 => wrong.period += 1,
            3 => wrong.minimum_violation_origins += 1, 4 => wrong.minimum_benign_origins += 1,
            5 => wrong.precision_floor.numerator += 1, 6 => wrong.precision_floor.denominator += 1,
            7 => wrong.recall_floor.numerator += 1, 8 => wrong.recall_floor.denominator += 1,
            9 => wrong.false_positive_ceiling.numerator += 1, 10 => wrong.false_positive_ceiling.denominator += 1,
            _ => wrong.false_stop_budget += 1,
        }
        assert!(matches!(FileOversight::open_evaluated_guarded(root.store(), profile(), &requirement, &wrong),
            Err(JournalError::Contract(Error::Binding))));
        assert_eq!(std::fs::read(&path).unwrap(), bytes); assert_eq!(std::fs::read(&pending).unwrap(), b"preserve");
    }
    assert!(FileOversight::open_guarded(root.store(), profile(), &requirement).is_err());
    let mut stale = requirement.clone(); stale.minimum.journal_revision += 1;
    assert_eq!(FileOversight::read_credibility(root.store(), &profile(), &stale, &protocol()), Err(JournalError::Contract(Error::Stale)));
    let mut corrupt = bytes.clone(); corrupt.push(0); std::fs::write(&path, &corrupt).unwrap();
    assert!(FileOversight::read_credibility(root.store(), &profile(), &requirement, &protocol()).is_err());
    assert_eq!(std::fs::read(&pending).unwrap(), b"preserve");
    std::fs::write(&path, bytes).unwrap();
    FileOversight::open_evaluated_guarded(root.store(), profile(), &requirement, &protocol()).unwrap();
    assert!(!pending.exists());
    let invalid = Directory::new(); let mut p = protocol(); p.recall_floor.denominator = 0;
    assert!(FileOversight::create_evaluated_guarded(invalid.store(), profile(), &guards(), None, p).is_err());
    assert!(!invalid.store().exists());
    let plain = Directory::new();
    let (plain_host, _) = FileOversight::create_guarded(plain.store(), profile(), &guards(), None).unwrap();
    let plain_requirement = expected(&plain_host, guards()); drop(plain_host);
    let plain_path = plain.store().join("delivery.bin"); let plain_bytes = std::fs::read(&plain_path).unwrap();
    assert!(matches!(FileOversight::open_evaluated_guarded(plain.store(), profile(), &plain_requirement, &protocol()),
        Err(JournalError::Contract(Error::Binding))));
    assert_eq!(std::fs::read(&plain_path).unwrap(), plain_bytes);
    FileOversight::open_guarded(plain.store(), profile(), &plain_requirement).unwrap();
}

#[test]
fn labels_do_not_resume_a_stopped_numerical_owner_or_disappear_on_reopen() {
    let root = Directory::new(); let mut g = guards(); g.decoder = Some(numerical::configuration(100.0));
    let (mut host, roles) = FileOversight::create_evaluated_guarded(root.store(), profile(), &g, None, protocol()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); numerical::forced(&mut host, 0);
    review(&mut host, 1, Verdict::Hold);
    let c = host.inspect().control;
    host.request_stop(host.revision(), StopRequest { operation: 8, expected_control_sequence: c.sequence,
        expected_authority_epoch: c.ledger.epoch }).unwrap();
    let before = host.inspect().control; let n = host.decoder_inspection().unwrap();
    label(&mut host, &roles.evaluator, 1, 1, GroundTruth::Benign);
    assert_eq!(host.inspect().control, before); assert_eq!(host.decoder_inspection().unwrap().numerical, n.numerical);
    assert!(host.decoder_inspection().unwrap().paused); assert_eq!(host.credibility_report().unwrap().lifetime_false_stops, 1);
    let requirement = expected(&host, g); drop(host);
    let (mut host, fresh) = FileOversight::open_evaluated_guarded(root.store(), profile(), &requirement, &protocol()).unwrap();
    assert!(!label(&mut host, &fresh.evaluator, 1, 1, GroundTruth::Benign));
    assert!(host.inspect().stop.is_some()); assert!(host.decoder_inspection().unwrap().paused);
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert!(host.resume_decoder(host.revision(), n.numerical.actor_revision, n.numerical.position).is_err());
    let request = update(&host, 1); assert!(host.promote_credibility(host.revision(), &request).is_err());
}

#[test]
fn false_stop_history_survives_exact_policy_change_and_guarded_recovery() {
    let root = Directory::new(); let g = guards(); let mut p = protocol(); p.false_stop_budget = 1;
    let (mut host, roles) = FileOversight::create_evaluated_guarded(root.store(), profile(), &g, None, p.clone()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    review(&mut host, 1, Verdict::Hold); label(&mut host, &roles.evaluator, 1, 1, GroundTruth::Benign);
    assert!(host.credibility_report().unwrap().calibration_incident_open);
    let c = host.inspect().control;
    let change = PolicyUpdate::new(7, c.sequence, c.ledger.epoch,
        Policy::new(2, host.current_policy().unwrap().nodes().to_vec()).unwrap()).unwrap();
    host.replace_policy(host.revision(), &change).unwrap();
    let report = host.credibility_report().unwrap();
    assert_eq!(report.scoped_cases, 0); assert_eq!(report.retained_cases, 1);
    assert_eq!(report.lifetime_false_stops, 1); assert!(report.calibration_incident_open);
    let requirement = expected(&host, g); drop(host);
    let (host, _) = FileOversight::open_evaluated_guarded(root.store(), profile(), &requirement, &p).unwrap();
    assert_eq!(host.credibility_report().unwrap(), report);
}

#[path = "support/file_guarded.rs"] mod composed;

#[test]
fn every_guard_composes_with_label_promotion_withdrawal_and_fresh_credentialed_stream_publication() {
    use fa_reference::action::consequence::oversight::evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource, MAX_EVIDENCE_FILE_BYTES};
    use fa_reference::action::consequence::oversight::identity::IdentityStatus;
    use fa_reference::action::consequence::oversight::policy_governance::CampaignDisposition;
    let root = Directory::new(); let mut g = composed::all_guards();
    g.decoder = Some(numerical::configuration(100.0));
    let mut p = composed::profile_for(&g); p.delivery.congress = profile().delivery.congress;
    let inventory = composed::inventory(); let route = composed::binding();
    let (mut host, roles) = FileOversight::create_evaluated_guarded(root.store(), p.clone(), &g,
        Some(FileCredentialRegistration { inventory: &inventory, binding: &route }), protocol()).unwrap();
    assert_eq!(host.revision(), 7); assert!(roles.oversight.identity_observer.is_some());
    assert!(roles.oversight.policy_governor.is_some());
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap(); numerical::forced(&mut host, 0);
    let observation = EvidenceSnapshot::new(EvidenceIdentity { source: 17, generation: 1, scope: p.delivery.scope },
        snapshot(), ordinary::MEMBERS.into_iter().map(|m| (m.to_owned(), b"independent case evidence".to_vec())).collect()).unwrap();
    let source_path = root.0.join("evidence.json"); std::fs::write(&source_path, observation.encode()).unwrap();
    let mut source = FileEvidenceSource::new(source_path, 17, p.delivery.scope, MAX_EVIDENCE_FILE_BYTES).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    composed::identity::matched(&mut host, roles.oversight.identity_observer.as_ref().unwrap(), 1, 1);
    for (id, verdict, truth) in [(1, Verdict::Hold, GroundTruth::Violation), (2, Verdict::Allow, GroundTruth::Benign)] {
        let spec = host.stream_message_spec("reviewed message", ElapsedTick(100)).unwrap();
        let action = host.propose(host.revision(), id, spec, snapshot()).unwrap();
        let inputs = observation.inputs_for(&action, &p.committee).unwrap();
        host.record_inputs(host.revision(), id, 0, inputs.clone()).unwrap();
        host.begin_review(host.revision(), id, 100 + id, ordinary::ROOT, ordinary::window(&host), snapshot()).unwrap();
        ordinary::votes(&mut host, 100 + id, verdict);
        host.finish_review(host.revision(), 100 + id, Some(&inputs), snapshot()).unwrap().unwrap();
        label(&mut host, &roles.evaluator, id, id, truth);
    }
    let c = host.inspect().control;
    let campaign_update = PolicyUpdate::new(81, c.sequence, c.ledger.epoch,
        Policy::new(2, host.current_policy().unwrap().nodes().to_vec()).unwrap()).unwrap();
    let campaign = host.request_policy_campaign(host.revision(), &campaign_update).unwrap(); let r = host.revision();
    let campaign_key = roles.oversight.policy_governor.as_ref().unwrap().approve(&mut host, r, &campaign, false).unwrap();
    let request = update(&host, 1); host.promote_credibility(host.revision(), &request).unwrap();
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(host.policy_campaign(81).unwrap().observed_disposition(), CampaignDisposition::Revoked);
    assert!(host.promote_policy_campaign(host.revision(), &campaign_key).is_err());
    composed::identity::matched(&mut host, roles.oversight.identity_observer.as_ref().unwrap(), 2, 2);
    let spec = host.stream_message_spec("fresh after evaluation", ElapsedTick(100)).unwrap();
    let action = host.propose(host.revision(), 3, spec, snapshot()).unwrap();
    let inputs = observation.inputs_for(&action, &p.committee).unwrap();
    ordinary::review_existing(&mut host, 3, 103, &inputs);
    let automatic = host.authorize(host.revision(), 3, &inputs, snapshot()).unwrap();
    let human_request = host.request_human_approval(host.revision(), 1003, 3, &inputs, ElapsedTick(20)).unwrap();
    let r = host.revision(); let human = roles.oversight.human.approve(&mut host, r, &human_request).unwrap();
    host.dispatch(host.revision(), &automatic, &human, &action, &inputs, snapshot()).unwrap();
    let credential = composed::credential(&host);
    assert_eq!(host.publish_checked_with_credential(host.revision(), 3, Some(&inputs), snapshot(), ElapsedTick(1), &credential).unwrap().outcome,
        EndpointOutcome::Executed { resulting_version: 2 });
    host.reconcile(host.revision(), 3).unwrap();
    let requirement = composed::requirements(&host, g); let report = host.credibility_report().unwrap(); drop(host);
    let (host, fresh) = FileOversight::open_evaluated_guarded(root.store(), p, &requirement, &protocol()).unwrap();
    assert_eq!(host.credibility_report().unwrap(), report);
    assert!(fresh.oversight.identity_observer.is_some() && fresh.oversight.policy_governor.is_some());
    assert_eq!(host.stream_snapshot().unwrap().confirmed.messages().collect::<Vec<_>>(), vec!["fresh after evaluation"]);
    assert!(host.decoder_inspection().unwrap().paused); assert!(!host.clock_ready());
}
