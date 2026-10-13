use super::*;
use super::super::mutations::{self as engine, Mutation};
use fa_reference::action::{ActionSpec, ElapsedTick, VERSION};
use fa_reference::action::consequence::delivery::persistent::observed::guarded::evaluation::andon::FileNativeAndonSnapshot;
use fa_reference::action::consequence::oversight::{ReviewWindow,
    credibility::{Assessment, GroundTruth},
    evidence_source::{EvidenceIdentity, EvidenceSnapshot, FileEvidenceSource}};
use fa_reference::round::{Verdict, commitment};
use fa_reference::{Snapshot, round::review::andon::AndonBudget};
use std::{cell::Cell, collections::BTreeMap};

fn prepared() -> (Directory, Mutation) {
    let root = Directory::new(); root.create();
    let config = root.config(); let settings = Settings::decode(SETTINGS).unwrap();
    let resume = ResumeProfile::read(&root.resume(None)).unwrap();
    let custody = resume.admit(&config).unwrap();
    let expected = settings.requirements(&config, resume.minimum()).unwrap();
    let (mut host, _) = FileOversight::open_andon_guarded(&config.store, config.profile.clone(),
        &expected, &settings.protocol, custody.anchor()).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let evidence = EvidenceSnapshot::new(EvidenceIdentity { source: config.source_policy.source.source,
        generation: 1, scope: config.profile.delivery.scope },
        Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) },
        ["alpha", "beta"].into_iter().map(|name| (name.to_owned(), b"test context".to_vec())).collect()).unwrap();
    let evidence_path = root.0.join("evidence.json");
    fs::write(&evidence_path, evidence.encode()).unwrap();
    let mut source = FileEvidenceSource::new(evidence_path.clone(), config.source_policy.source.source,
        config.profile.delivery.scope, 1048576).unwrap();
    host.refresh_file_source(host.revision(), &mut source, ElapsedTick(1)).unwrap();
    let action = host.propose(host.revision(), 1, ActionSpec { version: VERSION,
        scope: config.profile.delivery.scope, target: Some(host.inspect().target),
        payload: b"reviewed".to_vec(), required_witnesses: Vec::new(), units: 16,
        policy_epoch: host.inspect().control.ledger.epoch, deadline: ElapsedTick(100) }, evidence.snapshot().clone()).unwrap();
    let inputs = evidence.inputs_for(&action, &config.profile.committee).unwrap();
    host.record_inputs(host.revision(), 1, 0, inputs.clone()).unwrap();
    let root_id = evidence.reference_root();
    host.begin_review(host.revision(), 1, 101, root_id, ReviewWindow {
        commit_by: ElapsedTick(5), reveal_by: ElapsedTick(8) }, evidence.snapshot().clone()).unwrap();
    for (name, verdict) in [("alpha", Verdict::Hold), ("beta", Verdict::Allow)] {
        host.commit_review(host.revision(), 101, name, commitment(101, name, &root_id, verdict, b"s").unwrap()).unwrap();
    }
    host.open_reveals(host.revision(), 101).unwrap();
    for (name, verdict) in [("alpha", Verdict::Hold), ("beta", Verdict::Allow)] {
        host.reveal_review(host.revision(), 101, name, verdict, b"s".to_vec()).unwrap();
    }
    host.finish_review(host.revision(), 101, Some(&inputs), evidence.snapshot().clone()).unwrap().unwrap();
    let case = host.evaluation_ticket(101).unwrap().case().clone();
    assert!(case.stopped);
    let mutation = Mutation::Assess { round: case.round, attempt: case.attempt,
        policy: case.policy_generation, congress: case.congress_generation, root: case.evidence_root,
        expected_revision: host.credibility_report().unwrap().revision,
        assessment: Assessment { origin: 1, evidence_id: [17; 32], truth: GroundTruth::Benign } };
    drop(host);
    fs::remove_file(evidence_path).unwrap(); // All later commands must remain offline.
    (root, mutation)
}
fn state(root: &Directory, settings: &Settings) -> FileNativeAndonSnapshot {
    let config = root.config();
    let request = ResumeProfile::read(&root.resume(None)).unwrap();
    let custody = request.admit(&config).unwrap();
    FileOversight::read_andon_guarded(&config.store, &config.profile,
        &settings.requirements(&config, request.minimum()).unwrap(), &settings.protocol, custody.anchor()).unwrap()
}
fn run(root: &Directory, settings: &Settings, mutation: &Mutation, output: &str,
    clock: &mut impl FnMut() -> Result<ElapsedTick, String>) -> Result<Vec<u8>, String>
{
    let mut bytes = Vec::new();
    engine::run(&root.config(), settings, &root.resume(Some(&root.0.join(output))), mutation,
        &mut || Ok(()), clock, &mut bytes)?;
    valid_json(&bytes); Ok(bytes)
}

#[test]
fn native_stopped_case_is_assessed_offline_and_exact_retry_never_redates_or_recharges() {
    let (root, mutation) = prepared(); let settings = Settings::decode(SETTINGS).unwrap();
    let calls = Cell::new(0);
    let output = run(&root, &settings, &mutation, "first", &mut || {
        calls.set(calls.get() + 1); Ok(ElapsedTick(20))
    }).unwrap();
    assert!(String::from_utf8(output).unwrap().contains("\"status\":\"applied\""));
    assert_eq!(calls.get(), 1);
    let first = state(&root, &settings);
    assert_eq!(first.accounting.windows()[&1].false_stops, 1);
    assert_eq!(first.receipts[0].confirmation().confirmed_at, 20);
    assert!(root.0.join("first").is_file());
    let output = run(&root, &settings, &mutation, "second", &mut || panic!("exact label retry sampled time")).unwrap();
    assert!(String::from_utf8(output).unwrap().contains("\"status\":\"already_applied\""));
    let second = state(&root, &settings);
    assert_eq!(first.accounting, second.accounting); assert_eq!(first.receipts, second.receipts);
    assert_eq!(first.credibility.report, second.credibility.report);
    // One original recovery fence per invocation; the repeat adds no Time/Assess.
    assert_eq!(second.credibility.journal.revision, first.credibility.journal.revision + 1);
}

#[test]
fn censored_and_violation_labels_do_not_require_time_or_create_false_stop_charges() {
    for truth in [GroundTruth::Censored, GroundTruth::Violation] {
        let (root, mut mutation) = prepared(); let settings = Settings::decode(SETTINGS).unwrap();
        if let Mutation::Assess { assessment, .. } = &mut mutation { assessment.truth = truth; }
        run(&root, &settings, &mutation, "label", &mut || panic!("non-benign label sampled time")).unwrap();
        let report = state(&root, &settings);
        assert!(report.receipts.is_empty()); assert_eq!(report.accounting.windows()[&1].false_stops, 0);
        if truth == GroundTruth::Censored {
            assert_eq!(report.credibility.report.censored_cases, 1);
            if let Mutation::Assess { assessment, expected_revision, .. } = &mut mutation {
                assessment.truth = GroundTruth::Benign; *expected_revision = report.credibility.report.revision;
            }
            run(&root, &settings, &mutation, "resolved", &mut || Ok(ElapsedTick(30))).unwrap();
            assert_eq!(state(&root, &settings).accounting.windows()[&1].false_stops, 1);
        }
    }
}

#[test]
fn wrong_applied_case_and_stale_evaluation_revision_refuse_before_clock_or_label() {
    for field in 0..5 {
        let (root, mut mutation) = prepared(); let settings = Settings::decode(SETTINGS).unwrap();
        if let Mutation::Assess { attempt, policy, congress, root, expected_revision, .. } = &mut mutation {
            match field { 0 => *attempt += 1, 1 => *policy += 1, 2 => *congress += 1,
                3 => root[0] ^= 1, _ => *expected_revision += 1 }
        }
        assert!(run(&root, &settings, &mutation, "refusal", &mut || panic!("invalid case sampled time")).is_err());
        let report = state(&root, &settings);
        assert!(report.receipts.is_empty()); assert_eq!(report.credibility.report.pending_cases, 1);
        assert!(root.0.join("refusal").is_file()); // Acknowledged recovery is retained.
    }
}

#[test]
fn clock_error_or_slow_clock_retains_fence_but_does_not_accept_a_label() {
    for slow in [false, true] {
        let (root, mutation) = prepared(); let settings = Settings::decode(SETTINGS).unwrap();
        let expired = Cell::new(false);
        let result = engine::run(&root.config(), &settings, &root.resume(Some(&root.0.join("clock-refusal"))),
            &mutation, &mut || if expired.get() { Err("deadline".into()) } else { Ok(()) },
            &mut || { if slow { expired.set(true); Ok(ElapsedTick(20)) } else { Err("clock unavailable".into()) } },
            &mut Vec::new());
        assert!(result.is_err());
        assert!(root.0.join("clock-refusal").exists());
        let report = state(&root, &settings);
        assert_eq!(report.credibility.report.pending_cases, 1); assert!(report.receipts.is_empty());
        run(&root, &settings, &mutation, "corrected", &mut || Ok(ElapsedTick(20))).unwrap();
        assert_eq!(state(&root, &settings).accounting.windows()[&1].false_stops, 1);
    }
}

#[test]
fn every_replay_checkpoint_including_ready_cancels_without_cleanup_or_fence() {
    let (basis, _) = prepared(); let settings = Settings::decode(SETTINGS).unwrap();
    let config = basis.config(); let req = ResumeProfile::read(&basis.resume(None)).unwrap();
    let custody = req.admit(&config).unwrap();
    let cursor = FileOversight::begin_open_andon_guarded(&config.store, config.profile.clone(),
        &settings.requirements(&config, req.minimum()).unwrap(), &settings.protocol, custody.anchor()).unwrap();
    let total = cursor.progress().total_events; drop(cursor);
    for fail_at in 1..=total + 2 {
        let (root, mutation) = prepared(); let original = root.bytes();
        fs::write(root.0.join("store/delivery.pending"), b"diagnostic staging").unwrap();
        let mut calls = 0;
        assert!(engine::run(&root.config(), &settings, &root.resume(Some(&root.0.join("unused"))), &mutation,
            &mut || { calls += 1; if calls == fail_at { Err("deadline".into()) } else { Ok(()) } },
            &mut || panic!("cancelled replay sampled time"), &mut Vec::new()).is_err());
        assert_eq!(root.bytes(), original);
        assert_eq!(fs::read(root.0.join("store/delivery.pending")).unwrap(), b"diagnostic staging");
        assert!(!root.0.join("unused").exists());
    }
}

#[test]
fn failed_time_publication_never_exports_a_stale_successor_or_claims_assessment() {
    let (root, mutation) = prepared(); let settings = Settings::decode(SETTINGS).unwrap();
    let old_anchor = fs::read(root.anchor()).unwrap();
    let error = run(&root, &settings, &mutation, "unconfirmed", &mut || {
        fs::write(root.0.join("store/delivery.pending"), b"interrupted writer").unwrap();
        Ok(ElapsedTick(20))
    }).unwrap_err();
    assert!(error.contains("unconfirmed")); assert!(!root.0.join("unconfirmed").exists());
    assert_eq!(fs::read(root.anchor()).unwrap(), old_anchor);
    assert!(state(&root, &settings).receipts.is_empty());
    // Original validated reopen, not test-side deletion, cleans the diagnostic tail.
    run(&root, &settings, &mutation, "recovered", &mut || Ok(ElapsedTick(20))).unwrap();
    assert_eq!(state(&root, &settings).accounting.windows()[&1].false_stops, 1);
}

#[test]
fn window_registration_is_original_immutable_accounting_and_requires_complete_successor_inventory() {
    let root = Directory::new(); root.create(); let mut settings = Settings::decode(SETTINGS).unwrap();
    let window = AndonBudget { window: 2, start: 2000000000000, end: 3000000000000,
        false_stops_allowed: 3, cost_per_stop: 10 };
    run(&root, &settings, &Mutation::Window(window), "window", &mut || panic!("window sampled time")).unwrap();
    assert!(inspect(&root.config(), &settings, &root.resume(None), &mut || Ok(()), &mut Vec::new()).is_err());
    settings.windows.push(window);
    assert_eq!(state(&root, &settings).accounting.windows().len(), 2);
    run(&root, &settings, &Mutation::Window(window), "repeat", &mut || panic!("retry sampled time")).unwrap();
    assert!(run(&root, &settings, &Mutation::Window(AndonBudget { cost_per_stop: 11, ..window }),
        "changed", &mut || panic!("changed budget sampled time")).is_err());
    assert_eq!(state(&root, &settings).accounting.windows()[&2].budget, window);
}

#[test]
fn output_failure_keeps_the_accepted_label_and_retained_anchor_without_resubmission() {
    for flush_only in [false, true] {
        let (root, mutation) = prepared(); let settings = Settings::decode(SETTINGS).unwrap();
        let error = engine::run(&root.config(), &settings, &root.resume(Some(&root.0.join("saved"))), &mutation,
            &mut || Ok(()), &mut || Ok(ElapsedTick(20)), &mut BrokenOutput { flush_only }).unwrap_err();
        assert!(error.contains("assess applied")); assert!(root.0.join("saved").is_file());
        let before = state(&root, &settings);
        assert_eq!(before.accounting.windows()[&1].false_stops, 1);
        run(&root, &settings, &mutation, "retry", &mut || panic!("replayed label sampled time")).unwrap();
        assert_eq!(before.receipts, state(&root, &settings).receipts);
    }
}

#[test]
fn mutation_documents_are_closed_and_original_case_identity_is_required() {
    let valid = format!("{{\"schema_version\":\"fa.native-assessment/1\",\"round\":101,\"attempt\":1,\"policy_generation\":1,\"congress_generation\":1,\"case_evidence_root_hex\":\"{}\",\"expected_evaluation_revision\":1,\"origin\":1,\"evidence_id_hex\":\"{}\",\"truth\":\"censored\"}}", "09".repeat(32), "11".repeat(32));
    assert!(Mutation::decode("assess", valid.as_bytes()).is_ok());
    for changed in [valid.replace("censored", "safe"), valid.replace("\"attempt\":1", "\"attempt\":0"),
        valid.replace("\"round\":101", "\"round\":101,\"extra\":1"),
        valid.replace(&"11".repeat(32), &"00".repeat(32))] {
        assert!(Mutation::decode("assess", changed.as_bytes()).is_err());
    }
    assert!(Mutation::decode("register-window", valid.as_bytes()).is_err());
    let (root, mutation) = prepared(); let before = root.bytes();
    assert!(engine::run(&root.config(), &Settings::decode(SETTINGS).unwrap(), &root.resume(None), &mutation,
        &mut || Ok(()), &mut || panic!("missing custody sampled time"), &mut Vec::new()).is_err());
    assert_eq!(root.bytes(), before);
}

#[test]
fn actual_assessment_command_accepts_a_censored_label_without_current_source_access() {
    let (root, mutation) = prepared();
    let Mutation::Assess { round, attempt, policy, congress, root: evidence, expected_revision, .. } = mutation
        else { unreachable!("prepared assessment") };
    let config_path = root.0.join("config.json");
    fs::write(&config_path, std::str::from_utf8(CONFIG).unwrap().replace("/operator/publication-store",
        root.config().store.to_str().unwrap())).unwrap();
    let settings_path = root.0.join("settings.json"); fs::write(&settings_path, SETTINGS).unwrap();
    let document = root.0.join("assessment.json");
    let evidence: String = evidence.iter().map(|byte| format!("{byte:02x}")).collect();
    fs::write(&document, format!("{{\"schema_version\":\"fa.native-assessment/1\",\"round\":{round},\"attempt\":{attempt},\"policy_generation\":{policy},\"congress_generation\":{congress},\"case_evidence_root_hex\":\"{evidence}\",\"expected_evaluation_revision\":{expected_revision},\"origin\":1,\"evidence_id_hex\":\"{}\",\"truth\":\"censored\"}}", "11".repeat(32))).unwrap();
    let resume = root.resume(Some(&root.0.join("command-anchor")));
    let args = ["assess".into(), config_path.to_str().unwrap().into(), settings_path.to_str().unwrap().into(),
        resume.to_str().unwrap().into(), document.to_str().unwrap().into()];
    let mut out = Vec::new();
    command_with_elapsed(&args, &mut || Duration::ZERO, &mut out).unwrap();
    valid_json(&out);
    let result = state(&root, &Settings::decode(SETTINGS).unwrap());
    assert_eq!(result.credibility.report.censored_cases, 1);
    assert_eq!(result.credibility.report.pending_cases, 0);
    assert!(result.receipts.is_empty()); assert!(root.0.join("command-anchor").exists());
}
