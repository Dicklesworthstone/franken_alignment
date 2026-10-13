//! Operator composition checked against the original durable admission and solver.
#[path = "fixture.rs"]
mod fixture;
use super::*;
use fixture::{Case, document, CANDIDATES};
use fa_reference::action::consequence::experiment::{Minimality, MAX_INTERVENTION_BYTES, MAX_SEARCH_CANDIDATES};
use fa_reference::action::consequence::gate::containment::session::policy::Truth;
use fa_reference::strict_json::{self, Json, Limits};
use std::{fs, io};

fn json(bytes: &[u8]) -> Json { strict_json::parse(bytes, Limits::default()).unwrap() }
fn text<'a>(value: &'a Json, field: &str) -> &'a str { value.get(field).unwrap().as_str().unwrap() }
fn array<'a>(value: &'a Json, field: &str) -> &'a [Json] { value.get(field).unwrap().as_array().unwrap() }
fn elapsed() -> Duration { Duration::ZERO }
fn maximum() -> ProposalSearchBudget {
    ProposalSearchBudget { cases: MAX_PROPOSAL_SEARCH_CASES, retained_edit_bytes: MAX_PROPOSAL_SEARCH_EDIT_BYTES }
}
fn invoke(args: &[String], out: &mut impl Write) -> Result<(), String> {
    super::super::command_with_elapsed(args, &mut elapsed, out)
}
fn unchanged(case: &Case, before: &std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>, phase: &str) {
    assert_eq!(&case.files(), before);
    assert_eq!(case.host.inspect().executions, 0);
    assert_eq!(case.config.source.status().read_attempts, 1);
    eprintln!("{{\"suite\":\"operator_proposal_search\",\"control\":\"{phase}\",\"revision\":{},\"executions\":0,\"source_reads\":1}}", case.host.revision());
}

#[test]
fn complete_search_retains_all_native_cases_and_subset_minimal_repairs_without_promoting_them() {
    let case = Case::new(); let before = case.files(); let input = document(CANDIDATES);
    let menu = Menu::decode(input.as_bytes()).unwrap();
    let original = case.host.investigate_proposal(2, menu.scope.clone()).unwrap();
    let mut oracle = original.begin_repair_search(&menu.candidates, menu.budget).unwrap();
    while oracle.status() == ProposalSearchStatus::Running { oracle.advance().unwrap(); }
    let oracle = oracle.finish().unwrap();
    let actual = evaluate(&case.investigation(901, &menu), &menu, Duration::from_secs(60), &mut elapsed).unwrap();
    assert_eq!(actual.origin(), oracle.origin()); assert_eq!(actual.search().work(), oracle.search().work());
    for (a, b) in actual.search().cases().iter().zip(oracle.search().cases()) {
        assert_eq!(a, b);
    }
    assert_eq!(actual.search().cases().len(), 16); assert!(actual.search().unreported_masks().next().is_none());
    assert_eq!(actual.search().sufficient_repairs().iter().map(|r| (r.mask, r.minimality)).collect::<Vec<_>>(),
        vec![(5, Minimality::EstablishedWithinMenu), (6, Minimality::EstablishedWithinMenu)]);
    let result = |mask| &actual.search().cases().iter().find(|c| c.mask == mask).unwrap().outcome;
    assert_eq!(result(3).as_ref().err(), Some(&fa_reference::Error::Duplicate));
    assert_eq!(result(5).as_ref().unwrap().counterfactual(), Truth::Satisfied);
    // Adding a valid edit makes this superset fail. No monotone-success shortcut.
    assert_eq!(result(13).as_ref().unwrap().counterfactual(), Truth::Violated);
    let mut out = Vec::new(); invoke(&case.args(901, &input), &mut out).unwrap();
    assert_eq!(json(&out), json(&render::encode(&oracle, &menu).unwrap()));
    let output = json(&out);
    assert_eq!(text(&output, "permission"), "not_issued"); assert_eq!(text(&output, "purpose"), "experiment");
    assert_eq!(text(&output, "status"), "complete"); assert_eq!(text(&output, "attempt"), "2");
    assert_eq!(array(&output, "repairs").len(), 2); assert_eq!(array(&output, "cases").len(), 16);
    crate::command(case.args(901, &input)).unwrap();
    assert!(!case.root.join("evidence.json").exists()); assert!(!case.root.join("helpers").exists());
    unchanged(&case, &before, "complete_original_oracle_parity");
}

#[test]
fn whole_menu_budgets_refuse_before_cases_and_exact_preflight_allowances_succeed() {
    let case = Case::new(); let before = case.files();
    let mut menu = Menu::decode(document(CANDIDATES).as_bytes()).unwrap();
    let investigation = case.investigation(901, &menu);
    let complete = evaluate(&investigation, &menu, Duration::from_secs(60), &mut elapsed).unwrap();
    let work = complete.search().work();
    menu.budget = ProposalSearchBudget { cases: work.planned_cases, retained_edit_bytes: work.planned_edit_bytes };
    assert_eq!(evaluate(&investigation, &menu, Duration::from_secs(60), &mut elapsed).unwrap().search(), complete.search());
    for dimension in 0..2 {
        let original = menu.budget;
        if dimension == 0 { menu.budget.cases -= 1; } else { menu.budget.retained_edit_bytes -= 1; }
        let mut calls = 0; let mut out = Vec::new();
        let result = run(&investigation, &menu, Duration::from_secs(60), &mut || { calls += 1; Duration::ZERO }, &mut out);
        assert!(result.unwrap_err().contains("Limit")); assert!(out.is_empty()); assert_eq!(calls, 1);
        menu.budget = original; unchanged(&case, &before, "whole_menu_budget_refusal");
    }
}

#[test]
fn every_explicit_case_cut_retains_native_work_missing_masks_and_qualified_minimality() {
    let case = Case::new(); let before = case.files();
    let mut menu = Menu::decode(document(CANDIDATES).as_bytes()).unwrap();
    let investigation = case.investigation(901, &menu);
    for cut in 0..=16 {
        menu.stop_after_cases = cut;
        let report = evaluate(&investigation, &menu, Duration::from_secs(60), &mut elapsed).unwrap();
        let mut oracle = investigation.begin_repair_search(&menu.candidates, menu.budget).unwrap();
        for _ in 0..cut { oracle.advance().unwrap(); }
        if cut < 16 { oracle.cancel().unwrap(); }
        assert_eq!(report.search(), oracle.report().search());
        assert_eq!(report.search().work().entered_cases, cut);
        assert_eq!(report.search().work().completed_cases, cut);
        assert_eq!(report.search().unreported_masks().collect::<Vec<_>>(), (cut as u16..16).collect::<Vec<_>>());
        let mut out = Vec::new();
        let result = run(&investigation, &menu, Duration::from_secs(60), &mut elapsed, &mut out);
        assert_eq!(result.is_ok(), cut == 16);
        let output = json(&out);
        assert_eq!(text(&output, "status"), if cut == 16 { "complete" } else { "cancelled" });
        assert_eq!(array(&output, "cases").len(), cut);
        assert_eq!(array(&output, "unreported_masks").len(), 16-cut);
        if cut < 16 {
            assert!(report.search().sufficient_repairs().iter().all(|r| r.minimality != Minimality::EstablishedWithinMenu));
        }
        unchanged(&case, &before, "explicit_case_cut");
    }
    let input = document(CANDIDATES).replacen('{', "{\"stop_after_cases\":7,", 1);
    let mut out = Vec::new(); assert!(invoke(&case.args(901, &input), &mut out).unwrap_err().contains("incomplete"));
    assert_eq!(array(&json(&out), "cases").len(), 7);
}

#[test]
fn every_cumulative_deadline_checkpoint_discards_the_search_without_partial_output_or_writes() {
    let case = Case::new(); let before = case.files(); let args = case.args(901, &document(CANDIDATES));
    let wall = Duration::from_millis(case.config.timing.runtime_ms); let below = wall - Duration::from_nanos(1);
    let mut count = 0;
    super::super::command_with_elapsed(&args, &mut || { count += 1; below }, &mut Vec::new()).unwrap();
    assert_eq!(count, case.host.revision() as usize + 16 + 11);
    for cut in 0..count {
        let mut calls = 0; let mut out = Vec::new();
        let result = super::super::command_with_elapsed(&args, &mut || {
            let current = calls; calls += 1; if current == cut { wall } else { below }
        }, &mut out);
        assert_eq!(result.err().as_deref(), Some(super::super::DEADLINE));
        assert_eq!(calls, cut+1); assert!(out.is_empty());
        unchanged(&case, &before, "deadline_discards_private_search");
    }
    // A caught unwind cannot return a partial diagnostic or retained authority.
    let mut calls = 0; let mut out = Vec::new();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::super::command_with_elapsed(&args, &mut || {
            calls += 1; assert_ne!(calls, case.host.revision() as usize + 10, "injected between native masks"); Duration::ZERO
        }, &mut out)
    }));
    assert!(result.is_err()); assert!(out.is_empty()); unchanged(&case, &before, "search_unwind");
    invoke(&args, &mut Vec::new()).unwrap();
}

#[test]
fn original_request_identity_scope_and_key_baselines_cannot_be_substituted() {
    let case = Case::new(); let before = case.files();
    let mut out = Vec::new(); invoke(&case.args(2, &document(CANDIDATES)), &mut out).unwrap();
    let output = json(&out);
    assert_eq!(text(&output, "attempt"), "3"); assert_eq!(text(&output, "admission_state"), "Reviewing");
    assert_eq!(text(&output, "ending_state"), "Cancelled");
    let repairs = array(&output, "repairs");
    assert_eq!(repairs[0].get("mask").unwrap().as_u64(), Some(0));
    assert_eq!(repairs[0].get("untouched").unwrap().as_bool(), Some(true));
    assert_eq!(text(&repairs[0], "minimality"), "established_within_menu");
    assert!(repairs[1..].iter().all(|r| text(r, "minimality") == "not_minimal"));
    for key in [1, 777] {
        let mut out = Vec::new(); assert!(invoke(&case.args(key, &document(CANDIDATES)), &mut out).unwrap_err().contains("Missing"));
        assert!(out.is_empty());
    }
    // Bad expected values are original refused CASES, not silently removed
    // candidates or a replacement baseline. Other cases remain reportable.
    let wrong = document(CANDIDATES).replace("\"expected_hex\":\"6f6b\"", "\"expected_hex\":null");
    let mut out = Vec::new(); invoke(&case.args(901, &wrong), &mut out).unwrap();
    let output = json(&out); let cases = array(&output, "cases");
    assert_eq!(cases.len(), 16);
    assert_eq!(text(cases[8].get("result").unwrap(), "error"), "Stale");
    let unseen = document(CANDIDATES).replace("\"keys\":[7]", "\"keys\":[8]").replace("\"key\":7", "\"key\":8");
    let mut out = Vec::new(); assert!(invoke(&case.args(901, &unseen), &mut out).is_err()); assert!(out.is_empty());
    let mut menu = Menu::decode(document(CANDIDATES).as_bytes()).unwrap(); let investigation = case.investigation(901, &menu);
    menu.scope = InterventionScope::new(82, true, true, true, &[7]).unwrap();
    assert!(evaluate(&investigation, &menu, Duration::from_secs(60), &mut elapsed).unwrap_err().contains("registration differs"));
    unchanged(&case, &before, "request_scope_key_refusals");
}

#[test]
fn explicit_empty_and_unsuccessful_menus_retain_denominators_without_inventing_a_repair() {
    let case = Case::new(); let before = case.files();
    for (candidates, required) in [("", 1), (r#"{"kind":"units","value":1}"#, 2)] {
        let mut out = Vec::new(); invoke(&case.args(901, &document(candidates)), &mut out).unwrap();
        let output = json(&out); assert_eq!(text(&output, "status"), "complete");
        assert_eq!(array(&output, "cases").len(), required); assert!(array(&output, "repairs").is_empty());
        assert!(array(&output, "unreported_masks").is_empty());
    }
    // Maximum ordered menu preserves all duplicate alternatives and 256 cases.
    let candidates = vec![r#"{"kind":"units","value":1}"#; MAX_SEARCH_CANDIDATES].join(",");
    let mut out = Vec::new(); invoke(&case.args(901, &document(&candidates)), &mut out).unwrap();
    assert_eq!(array(&json(&out), "cases").len(), 1 << MAX_SEARCH_CANDIDATES);
    unchanged(&case, &before, "complete_unsuccessful_menu");
}

#[test]
fn closed_menu_admission_bounds_every_occurrence_and_never_ignores_an_option() {
    let good = document(CANDIDATES); assert!(Menu::decode(good.as_bytes()).is_ok());
    for bad in [good.replace("fa.proposal-search/1", "fa.proposal-search/2"),
        good.replacen('{', "{\"surprise\":1,", 1), good.replacen('{', "{\"schema_version\":\"fa.proposal-search/1\",", 1),
        good.replace("\"payload\":true", "\"payload\":false"), good.replace("\"units\":true", "\"units\":1"),
        good.replace("\"keys\":[7]", "\"keys\":[7,7]"), good.replace("\"id\":81", "\"id\":0"),
        good.replace("\"cases\":256", "\"cases\":0"), good.replacen('{', "{\"stop_after_cases\":257,", 1),
        good.replace("\"replacement_hex\":\"6e6f\"", "\"other\":null"),
        good.replace("\"value\":1", "\"value\":1.0"), good.replace("\"value\":1", "\"value\":-1"),
        good.replace("6f6b", "6F6B"), good.replace("6f6b", "f"), good.replace("\"value\":1", "\"value\":18446744073709551616"),
        document(&vec![r#"{"kind":"units","value":1}"#; MAX_SEARCH_CANDIDATES+1].join(","))] {
        assert!(Menu::decode(bad.as_bytes()).is_err(), "{bad}");
    }
    let exact = "ff".repeat(MAX_INTERVENTION_BYTES);
    assert!(Menu::decode(document(&format!("{{\"kind\":\"payload\",\"bytes_hex\":\"{exact}\"}}")).as_bytes()).is_ok());
    let repeated = format!("{{\"kind\":\"key\",\"key\":7,\"expected_hex\":\"{exact}\",\"replacement_hex\":\"00\"}}");
    assert!(Menu::decode(document(&repeated).as_bytes()).is_err());
    assert!(Menu::decode(&vec![b' '; menu::MENU_BYTES+1]).is_err());
    let target = r#"{"kind":"target","adapter":1,"object":18446744073709551615,"contract_version":3,"expected_version":4,"generation":5}"#;
    let input = Menu::decode(document(target).as_bytes()).unwrap();
    assert!(matches!(input.candidates[0], Intervention::Target(t) if t.object == u64::MAX));
    assert!(Menu::decode(document(target).replace("\"target\":true", "\"target\":false").as_bytes()).is_err());
    let case = Case::new(); let before = case.files(); let valid = case.args(901, &good);
    for tail in [vec!["--search"], vec!["--search", "absent", "--payload", "absent"], vec!["--unknown", "absent"]] {
        let mut args = valid[..4].to_vec(); args.extend(tail.into_iter().map(str::to_owned)); args[1] = "absent-config".into();
        assert_eq!(super::super::command_with_elapsed(&args, &mut || panic!("syntax first"), &mut Vec::new())
            .err().as_deref(), Some(super::super::USAGE));
    }
    fs::remove_file(&valid[5]).unwrap(); let mut out = Vec::new();
    assert!(invoke(&valid, &mut out).is_err()); assert!(out.is_empty());
    unchanged(&case, &before, "closed_menu_and_cli");
}

#[test]
fn binary_candidates_and_null_vs_empty_key_values_are_retained_exactly() {
    let case = Case::new(); let before = case.files();
    for value in ["null", "\"\"", "\"001b225cff\""] {
        let input = document(&format!("{{\"kind\":\"payload\",\"bytes_hex\":\"001bff\"}},{{\"kind\":\"key\",\"key\":7,\"expected_hex\":\"6f6b\",\"replacement_hex\":{value}}}"));
        let mut out = Vec::new(); invoke(&case.args(901, &input), &mut out).unwrap();
        assert!(out.is_ascii()); assert!(!out.contains(&0x1b));
        let output = json(&out); let candidates = array(&output, "candidates");
        assert_eq!(text(candidates[0].get("edit").unwrap(), "bytes_hex"), "001bff");
        assert_eq!(candidates[1].get("edit").unwrap().get("replacement_hex").unwrap(), &json(value.as_bytes()));
        assert_eq!(array(&output, "cases").len(), 4);
    }
    unchanged(&case, &before, "binary_and_nullable_keys");
}

#[test]
fn search_output_failures_and_later_writer_progress_do_not_change_or_relabel_the_original_image() {
    struct Broken(bool);
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { if self.0 { Err(io::ErrorKind::BrokenPipe.into()) } else { Ok(bytes.len()) } }
        fn flush(&mut self) -> io::Result<()> { Err(io::ErrorKind::BrokenPipe.into()) }
    }
    let mut case = Case::new(); let input = document(CANDIDATES); let before = case.files(); let args = case.args(901, &input);
    for fail_write in [true, false] {
        assert!(invoke(&args, &mut Broken(fail_write)).unwrap_err().contains("output")); unchanged(&case, &before, "output_failure");
    }
    let revision = case.host.revision(); let mut calls = 0; let mut out = Vec::new();
    super::super::command_with_elapsed(&args, &mut || {
        calls += 1;
        // Config/profile/menu checkpoints, then custody/open: the sixth call
        // runs AFTER one canonical image has already been captured.
        if calls == 6 { case.host.observe_time(case.host.revision(), fa_reference::action::ElapsedTick(3)).unwrap(); }
        Duration::ZERO
    }, &mut out).unwrap();
    assert_eq!(text(&json(&out), "journal_revision"), revision.to_string());
    assert_eq!(case.host.revision(), revision+1);
    let after = case.files(); let mut next = Vec::new(); invoke(&args, &mut next).unwrap();
    assert_eq!(text(&json(&next), "journal_revision"), (revision+1).to_string());
    unchanged(&case, &after, "captured_image_not_latest");
}

#[test]
fn subset_minimality_is_not_relabelled_as_global_minimum_cardinality() {
    use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    let policy = Policy::new(1, vec![Predicate::UnitsAtMost(1), Predicate::PayloadAtMost(4),
        Predicate::ExactValue { key: 7, value: b"no".to_vec() }, Predicate::All(vec![1,2]), Predicate::Any(vec![0,3])]).unwrap();
    let case = Case::with_policy(Some(policy)); let before = case.files();
    let candidates = r#"{"kind":"units","value":1},{"kind":"payload","bytes_hex":"6f6b"},{"kind":"key","key":7,"expected_hex":"6f6b","replacement_hex":"6e6f"}"#;
    let menu = Menu::decode(document(candidates).as_bytes()).unwrap();
    let investigation = case.investigation(901, &menu);
    let report = evaluate(&investigation, &menu, Duration::from_secs(60), &mut elapsed).unwrap();
    let minimal: Vec<_> = report.search().sufficient_repairs().iter()
        .filter(|r| r.minimality == Minimality::EstablishedWithinMenu).map(|r| r.mask).collect();
    assert_eq!(minimal, vec![1,6]); // One edit and TWO edits are both subset-minimal.
    assert!(report.search().sufficient_repairs().iter().any(|r| r.mask == 7 && r.minimality == Minimality::NotMinimal));
    let output = json(&render::encode(&report, &menu).unwrap());
    let repairs = array(&output, "repairs");
    let pair = repairs.iter().find(|r| r.get("mask").unwrap().as_u64() == Some(6)).unwrap();
    assert_eq!(array(pair, "selected").len(), 2); assert_eq!(text(pair, "minimality"), "established_within_menu");
    unchanged(&case, &before, "subset_not_global_optimality");
}

#[test]
fn deleting_a_positive_range_witness_does_not_manufacture_closed_world_evidence() {
    use fa_reference::action::consequence::gate::containment::session::policy::{Policy, Predicate};
    let policy = Policy::new(1, vec![Predicate::EmptyRange { start: 7, end: 9 },
        Predicate::PayloadIs(b"ok".to_vec()), Predicate::Any(vec![0,1])]).unwrap();
    let case = Case::with_policy(Some(policy)); let before = case.files();
    let menu = Menu::decode(document(r#"{"kind":"key","key":7,"expected_hex":"6f6b","replacement_hex":null}"#).as_bytes()).unwrap();
    let mut out = Vec::new(); run(&case.investigation(901, &menu), &menu, Duration::from_secs(60), &mut elapsed, &mut out).unwrap();
    let output = json(&out); let cases = array(&output, "cases");
    assert_eq!(text(cases[0].get("result").unwrap(), "counterfactual"), "violated");
    assert_eq!(text(cases[1].get("result").unwrap(), "counterfactual"), "unknown");
    assert_eq!(text(cases[1].get("result").unwrap(), "next_requirement"), "more_evidence");
    assert!(array(&output, "repairs").is_empty()); assert!(array(&output, "unreported_masks").is_empty());
    unchanged(&case, &before, "unknown_not_closed_absence");
}
