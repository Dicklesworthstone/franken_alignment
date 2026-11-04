//! Original operator Config and canonical journals, never shutdown recovery.
use super::*;
use super::super::fleet as operator;
use std::{cell::Cell, rc::Rc};

fn member(id: u64, config: &Path, recipe: Option<&Path>, anchor: Option<&Path>) -> String {
    let optional = |path: Option<&Path>| path.map_or_else(|| "null".to_owned(),
        |path| format!("\"{}\"", path.display()));
    format!(r#"{{"id":{id},"config":"{}","learned_recipe":{},"history_anchor":{}}}"#,
        config.display(), optional(recipe), optional(anchor))
}
fn manifest(entries: &[String]) -> String {
    format!(r#"{{"schema":"fa.publication-situation-fleet/1","runtime_ms":60000,"input_bytes":268435456,"journal_bytes":268435456,"output_bytes":4194304,"replay_events":65536,"page_size":1,"members":[{}]}}"#,
        entries.join(","))
}
fn args(fixture: &Fixture, text: &str) -> Vec<String> {
    let path = fixture.root.0.join("fleet.json"); fs::write(&path, text).unwrap();
    vec!["situation".into(), "--fleet".into(), path.to_str().unwrap().into()]
}
fn invoke(args: &[String], out: &mut impl Write) -> Result<(), String> {
    operator::command_with(args, &mut || Duration::ZERO, out)
}
fn pages(bytes: &[u8]) -> Vec<Json> {
    assert!(bytes.ends_with(b"\n"));
    bytes[..bytes.len() - 1].split(|byte| *byte == b'\n').map(|line| {
        strict_json::parse(line, Limits { max_bytes: 65_536 + 1024,
            max_depth: 12, max_items: 4096, max_string_bytes: 128 }).unwrap()
    }).collect()
}
fn observed(page: &Json) -> &Json { page.get("situation").unwrap() }
fn flag(page: &Json, key: &str) -> bool { page.get(key).unwrap().as_bool().unwrap() }

#[test]
fn real_fleet_retains_each_cut_empty_members_and_unresolved_liabilities_without_mutation() {
    let mut a = Fixture::new(); a.clock(); a.propose(2, true); a.propose(4, true);
    let mut b = Fixture::new(); b.clock(); b.dispatch(9);
    let c = Fixture::new();
    let anchor = a.anchor();
    let selected = args(&a, &manifest(&[
        member(1, &a.config_file, None, Some(&anchor)),
        member(2, &b.config_file, None, None), member(3, &c.config_file, None, None)]));
    let before = (a.files(), b.files(), c.files(), a.host.inspect(), b.host.inspect());
    let mut out = Vec::new(); invoke(&selected, &mut out).unwrap(); let pages = pages(&out);
    assert_eq!(pages.iter().map(|p| text(p, "member")).collect::<Vec<_>>(), vec!["2", "1", "1", "3"]);
    assert_eq!(text(&rows(observed(&pages[0]))[0], "stage"), "dispatching");
    assert_eq!(text(summary(observed(&pages[0])), "unresolved_attempts"), "1");
    assert_eq!(text(summary(observed(&pages[1])), "attempts"), "2");
    assert_eq!(text(value(observed(&pages[2])), "offset"), "1");
    assert!(rows(observed(&pages[3])).is_empty());
    for (index, page) in pages.iter().enumerate() {
        assert_eq!(text(page, "schema"), "fa.publication-situation-fleet-page/1");
        assert_eq!(text(page, "member_state"), "known");
        assert!(!flag(page, "atomic_cut")); assert!(flag(page, "all_members_known"));
        assert_eq!(flag(page, "enumeration_complete"), index == 3);
        assert_eq!(text(page.get("coverage").unwrap(), "known_members"), "3");
    }
    let basis = knowledge(observed(&pages[1])).get("basis").unwrap();
    assert_eq!(text(basis, "journal_revision"), a.host.revision().to_string());
    assert_eq!(text(basis, "anchored_through"), a.host.revision().to_string());
    assert_eq!(before, (a.files(), b.files(), c.files(), a.host.inspect(), b.host.inspect()));
    let json = String::from_utf8(out).unwrap();
    for private in ["secret-payload", "alpha", "question_hex", a.root.0.to_str().unwrap()] {
        assert!(!json.contains(private));
    }
    // Exercise the actual top-level selector too, not only the clock seam.
    assert!(super::super::command(&selected, &mut Vec::new()).is_ok());
}

#[cfg(target_os = "linux")]
#[test]
fn missing_bad_and_wrong_recipe_members_are_reported_unknown_not_omitted_or_retried() {
    let mut a = Fixture::new(); a.clock(); a.propose(1, true);
    let b = Fixture::new(); fs::write(b.config.store.join("delivery.bin"), b"bad image").unwrap();
    let c = Fixture::new();
    let missing = a.root.0.join("missing-config.json");
    let wrong = a.root.0.join("missing-recipe.json");
    let selected = args(&a, &manifest(&[member(1, &a.config_file, None, None),
        member(2, &b.config_file, None, None), member(3, &missing, None, None),
        member(4, &c.config_file, Some(&wrong), None)]));
    let before = (a.files(), b.files(), c.files());
    let mut out = Vec::new(); assert!(invoke(&selected, &mut out).unwrap_err().contains("unknown members"));
    let pages = pages(&out); assert_eq!(pages.len(), 4);
    for page in &pages[..3] {
        assert_eq!(text(page, "member_state"), "unknown");
        assert!(matches!(page.get("situation"), Some(Json::Null)));
        assert_eq!(text(page.get("coverage").unwrap(), "unknown_members"), "3");
        assert!(!flag(page, "all_members_known"));
    }
    assert_eq!(text(&pages[3], "member"), "1");
    assert!(flag(&pages[3], "enumeration_complete")); assert!(!flag(&pages[3], "all_members_known"));
    assert_eq!(before, (a.files(), b.files(), c.files())); assert!(!missing.exists());
}

#[test]
fn closed_fleet_grammar_and_roster_schema_reject_mutation_options_before_member_reads() {
    for invalid in [vec!["situation", "--fleet"], vec!["situation", "--fleet", ""],
        vec!["situation", "--fleet", "missing", "--page-size", "1"],
        vec!["situation", "missing", "--fleet", "missing"],
        vec!["situation", "--fleet", "missing", "--fleet", "missing"],
        vec!["situation", "--fleet", "missing", "--retain-history-anchor", "new"],
        vec!["situation", "--fleet", "missing", "--credibility-activation", "capsule"]] {
        let invalid: Vec<_> = invalid.into_iter().map(str::to_owned).collect();
        assert_eq!(crate::command(invalid), Err(operator::USAGE.to_owned()));
    }
    let a = Fixture::new(); let missing = a.root.0.join("missing.json");
    let entry = member(1, &missing, None, None); let valid = manifest(std::slice::from_ref(&entry));
    for bad in [manifest(&[]), manifest(&[entry.clone(), entry]),
        valid.replace("\"id\":1", "\"id\":0"), valid.replace("\"page_size\":1", "\"page_size\":65"),
        valid.replace("\"runtime_ms\":60000", "\"runtime_ms\":0"),
        valid.replace("\"replay_events\":65536", "\"replay_events\":1048577"),
        valid.replace("\"history_anchor\":null", "\"history_anchor\":null,\"recover\":true"),
        valid.replace("\"learned_recipe\":null,", ""),
        valid.replace("\"schema\":", "\"unexpected\":true,\"schema\":")] {
        let selected = args(&a, &bad); let mut out = Vec::new();
        assert!(invoke(&selected, &mut out).is_err()); assert!(out.is_empty()); assert!(!missing.exists());
    }
}

#[test]
fn aliased_stores_and_self_supplied_anchors_cannot_claim_independent_coverage() {
    let mut a = Fixture::new(); a.clock();
    let alias = a.root.0.join("store-alias"); std::os::unix::fs::symlink(&a.config.store, &alias).unwrap();
    let other_config = a.root.0.join("other-config.json");
    fs::write(&other_config, fs::read_to_string(&a.config_file).unwrap()
        .replace(a.config.store.to_str().unwrap(), alias.to_str().unwrap())).unwrap();
    let selected = args(&a, &manifest(&[member(1, &a.config_file, None, None), member(2, &other_config, None, None)]));
    let mut out = Vec::new(); assert!(invoke(&selected, &mut out).unwrap_err().contains("alias")); assert!(out.is_empty());
    let independent = a.anchor(); let self_supplied = a.config.store.join("self-anchor.bin");
    fs::copy(independent, &self_supplied).unwrap(); let before = a.files();
    let selected = args(&a, &manifest(&[member(1, &a.config_file, None, Some(&self_supplied))]));
    assert!(invoke(&selected, &mut out).is_err());
    let page = pages(&out).remove(0); assert_eq!(text(&page, "member_state"), "unknown");
    assert_eq!(a.files(), before);
}

struct FirstWrite { bytes: Vec<u8>, calls: Rc<Cell<usize>>, first: Option<usize> }
impl Write for FirstWrite {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.first.get_or_insert(self.calls.get()); self.bytes.extend_from_slice(bytes); Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[test]
fn every_preoutput_deadline_checkpoint_aborts_even_a_one_shot_expiry_without_mutation() {
    let mut a = Fixture::new(); a.clock(); a.propose(1, true);
    let mut b = Fixture::new(); b.clock();
    let selected = args(&a, &manifest(&[member(1, &a.config_file, None, None), member(2, &b.config_file, None, None)]));
    let calls = Rc::new(Cell::new(0));
    let mut sink = FirstWrite { bytes: Vec::new(), calls: Rc::clone(&calls), first: None };
    operator::command_with(&selected, &mut || { calls.set(calls.get() + 1); Duration::ZERO }, &mut sink).unwrap();
    let before = (a.files(), b.files());
    for cut in 0..sink.first.unwrap() {
        let mut calls = 0; let mut out = Vec::new();
        let result = operator::command_with(&selected, &mut || {
            let at = calls; calls += 1; if at == cut { Duration::MAX } else { Duration::ZERO }
        }, &mut out);
        assert!(result.is_err(), "checkpoint {cut}"); assert!(out.is_empty(), "checkpoint {cut}");
        assert_eq!(before, (a.files(), b.files()));
    }
}

#[test]
fn global_work_and_output_caps_fail_without_recovery_and_partial_writes_are_not_retried() {
    let mut a = Fixture::new(); a.clock(); a.propose(1, true);
    let document = manifest(&[member(1, &a.config_file, None, None)]);
    let before = a.files();
    for text in [document.replace("\"journal_bytes\":268435456", "\"journal_bytes\":1"),
        document.replace("\"replay_events\":65536", "\"replay_events\":0"),
        document.replace("\"output_bytes\":4194304", "\"output_bytes\":1")] {
        let mut out = Vec::new(); assert!(invoke(&args(&a, &text), &mut out).is_err()); assert!(out.is_empty());
    }
    struct Failing { bytes: Vec<u8>, writes: usize }
    impl Write for Failing {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            if self.writes == 1 { self.bytes.extend_from_slice(&bytes[..7]); return Ok(7); }
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }
        fn flush(&mut self) -> io::Result<()> { panic!("failed output must not flush or retry"); }
    }
    let mut out = Failing { bytes: Vec::new(), writes: 0 };
    assert!(invoke(&args(&a, &document), &mut out).is_err()); assert_eq!(out.writes, 2); assert_eq!(out.bytes.len(), 7);
    assert_eq!(a.files(), before);
}
