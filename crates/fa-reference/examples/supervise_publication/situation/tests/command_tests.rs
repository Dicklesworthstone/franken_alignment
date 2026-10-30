use super::*;

#[test]
fn closed_grammar_precedes_all_shared_mutation_options_and_file_reads() {
    for args in [vec![], vec!["situation"], vec!["situation", ""], vec!["inspect", "missing"],
        vec!["situation", "missing", "--page-size"], vec!["situation", "missing", "--page-size", "0"],
        vec!["situation", "missing", "--page-size", "65"], vec!["situation", "missing", "--page-size", "+1"],
        vec!["situation", "missing", "--page-size", "01"], vec!["situation", "missing", "--page-size", "18446744073709551616"],
        vec!["situation", "missing", "--page-size", "1", "--page-size", "2"],
        vec!["situation", "missing", "--history-anchor", "a", "--history-anchor", "b"],
    ] {
        let args: Vec<_> = args.into_iter().map(str::to_owned).collect(); let mut out = Vec::new();
        assert_eq!(command(&args, &mut out), Err(USAGE.to_owned())); assert!(out.is_empty());
    }
    for option in ["--andon", "--durable", "--guarded", "--credibility-activation", "--precheck-profile", "--retain-history-anchor"] {
        let args: Vec<_> = ["situation", "missing-config", option, "missing"].into_iter().map(str::to_owned).collect();
        assert_eq!(crate::command(args), Err(USAGE.to_owned()));
    }
    for args in [vec!["situation", "c"], vec!["situation", "c", "--page-size", "64"],
        vec!["situation", "c", "--history-anchor", "a", "--page-size", "1"],
        vec!["situation", "c", "--page-size", "1", "--history-anchor", "a"]] {
        let args: Vec<_> = args.into_iter().map(str::to_owned).collect(); assert!(Arguments::parse(&args).is_ok());
    }
}

#[test]
fn live_owner_can_be_inspected_without_source_helper_or_recovery_work() {
    let mut fixture = Fixture::new(); fixture.clock();
    fixture.propose(1, false); fixture.propose(3, true); fixture.propose(2, true);
    let files = fixture.files(); let state = fixture.host.inspect(); let mut out = Vec::new();
    command(&fixture.args(1), &mut out).unwrap(); let pages = lines(&out);
    assert_eq!(pages.len(), 3);
    for (index, page) in pages.iter().enumerate() {
        assert_eq!(text(page, "schema"), "fa.publication-situation/1");
        assert_eq!(text(knowledge(page), "state"), "known");
        assert_eq!(text(knowledge(page).get("basis").unwrap(), "journal_revision"), state.revision.to_string());
        assert_eq!(text(knowledge(page).get("basis").unwrap(), "origin"), "canonical_image");
        assert_eq!(text(summary(page), "attempts"), "3"); assert_eq!(text(summary(page), "pending_attempts"), "2");
        assert_eq!(text(value(page), "offset"), index.to_string()); assert_eq!(complete(page), index == 2);
    }
    assert_eq!(pages.iter().map(|page| text(&rows(page)[0], "attempt")).collect::<Vec<_>>(), vec!["2", "3", "1"]);
    assert_eq!(fixture.files(), files); assert_eq!(fixture.host.inspect(), state);
    assert_eq!(fixture.config.source.status().read_attempts, 0);
    let json = String::from_utf8(out).unwrap();
    for private in ["secret-payload", "alpha", "beta", "payload_hex", "permit", "question_hex"] {
        assert!(!json.contains(private));
    }
}

#[test]
fn unknown_abandonment_is_reported_as_liability_and_success_only_means_read_success() {
    let mut fixture = Fixture::new(); fixture.clock(); fixture.propose(1, true); fixture.dispatch(7);
    fixture.host.fence(fixture.host.revision()).unwrap();
    let state = fixture.host.inspect();
    fixture.host.abandon_unknown(fixture.host.revision(), FileAbandonRequest {
        operation: 9, attempt: 7, expected_control_sequence: state.control.sequence,
        expected_authority_epoch: state.control.ledger.epoch,
    }).unwrap();
    let files = fixture.files(); let state = fixture.host.inspect(); let mut out = Vec::new();
    command(&fixture.args(1), &mut out).unwrap(); let pages = lines(&out);
    assert_eq!(text(&rows(&pages[0])[0], "stage"), "irrecoverably_unknown");
    assert_eq!(text(summary(&pages[0]), "unresolved_attempts"), "1");
    assert_eq!(text(summary(&pages[0]), "irrecoverably_unknown_attempts"), "1");
    assert_eq!(text(summary(&pages[0]), "charged_units"), "16");
    assert_eq!(text(summary(&pages[0]), "executions"), "0");
    assert_eq!(text(&rows(&pages[1])[0], "stage"), "cancelled");
    assert_eq!(fixture.files(), files); assert_eq!(fixture.host.inspect(), state);
}

#[test]
fn exact_custody_accepts_successors_but_refuses_rollback_without_output_or_repair() {
    let mut fixture = Fixture::new(); fixture.clock(); let older = fixture.files();
    fixture.propose(1, true); let anchor = fixture.anchor(); let anchor_revision = fixture.host.revision();
    fixture.propose(2, false); let current = fixture.files();
    let mut args = fixture.args(64); args.extend(["--history-anchor".into(), anchor.to_str().unwrap().into()]);
    let mut out = Vec::new(); command(&args, &mut out).unwrap();
    let pages = lines(&out); let basis = knowledge(&pages[0]).get("basis").unwrap();
    assert_eq!(text(basis, "anchored_through"), anchor_revision.to_string());
    assert_eq!(text(basis, "journal_revision"), fixture.host.revision().to_string());
    assert_eq!(fixture.files(), current);
    for (path, bytes) in &older { fs::write(path, bytes).unwrap(); }
    let mut out = Vec::new(); assert!(command(&args, &mut out).is_err()); assert!(out.is_empty());
    assert_eq!(fixture.files(), older);
    for (path, bytes) in &current { fs::write(path, bytes).unwrap(); }
    let alias = fixture.root.0.join("anchor-alias"); std::os::unix::fs::symlink(&anchor, &alias).unwrap();
    *args.last_mut().unwrap() = alias.to_str().unwrap().into();
    assert!(command(&args, &mut out).is_err()); assert!(out.is_empty()); assert_eq!(fixture.files(), current);
}

#[test]
fn complete_empty_history_is_distinct_from_missing_or_corrupt_history() {
    let fixture = Fixture::new(); let mut out = Vec::new(); command(&fixture.args(64), &mut out).unwrap();
    let pages = lines(&out); assert_eq!(pages.len(), 1); assert!(complete(&pages[0]) && rows(&pages[0]).is_empty());
    let files = fixture.files(); let (path, bytes) = files.iter().max_by_key(|(_, bytes)| bytes.len()).unwrap();
    fs::write(path, &bytes[..bytes.len() - 1]).unwrap();
    let broken = fixture.files(); out.clear(); assert!(command(&fixture.args(64), &mut out).is_err());
    assert!(out.is_empty()); assert_eq!(fixture.files(), broken);
    let original = fs::read_to_string(&fixture.config_file).unwrap();
    let missing = fixture.root.0.join("missing-store");
    fs::write(&fixture.config_file, original.replace(fixture.config.store.to_str().unwrap(), missing.to_str().unwrap())).unwrap();
    assert!(command(&fixture.args(64), &mut out).is_err()); assert!(out.is_empty()); assert!(!missing.exists());
}
