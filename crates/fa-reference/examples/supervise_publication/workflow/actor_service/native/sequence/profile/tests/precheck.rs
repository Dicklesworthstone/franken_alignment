//! Complete resident disclosure admission precedes every numerical asset read.
use super::*;
use crate::workflow::actor_service::precheck::Settings;
use std::fs;

fn checked_entry(recipe: &str, actor: &str, release: &str, precheck: &str) -> String {
    format!(r#"{{"recipe":"{recipe}","actor_profile":"{actor}","release":"{release}","precheck_profile":{precheck}}}"#)
}

fn checked_document(entries: &[String]) -> String {
    format!(r#"{{"schema":"fa.native-publication-sequence/2","entries":[{}]}}"#, entries.join(","))
}

fn settings(socket: &Path, id: u64, queries: usize) -> String {
    format!(r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":0,"disclosure_id":{id},"query_limit":{queries},"exchange_limit":16,"candidate_limit":8,"connection_limit":8,"runtime_ms":15000}}"#, socket.display())
}

fn selection(socket: &Path, id: u64, queries: usize) -> exact_precheck::Selection {
    exact_precheck::Selection::new(Settings::decode(settings(socket, id, queries).as_bytes()).unwrap()).unwrap()
}

#[test]
fn native_sequence_v2_requires_explicit_release_disclosures_without_reinterpreting_v1() {
    let base = Path::new("/operator/native");
    let first = checked_entry("first.json", "actor-1.json", "message", r#""precheck-1.json""#);
    let second = checked_entry("second.json", "actor-2.json", "message", "null");
    let finish = checked_entry("second.json", "actor-3.json", "finish", r#""/operator/finish-precheck.json""#);
    let valid = checked_document(&[first.clone(), second.clone(), finish]);
    let parsed = decode(valid.as_bytes(), base).unwrap();
    assert_eq!(parsed[0].precheck, Some(base.join("precheck-1.json")));
    assert!(parsed[1].precheck.is_none());
    assert_eq!(parsed[2].precheck, Some(PathBuf::from("/operator/finish-precheck.json")));
    assert!(matches!(parsed[2].release, Release::Finish));
    let original = document(&[
        entry("first.json", "actor-1.json", "message"),
        entry("second.json", "actor-2.json", "message"),
    ]);
    assert!(decode(original.as_bytes(), base).unwrap().iter().all(|entry| entry.precheck.is_none()));
    for invalid in [
        valid.replace("sequence/2", "sequence/1"),
        valid.replace("sequence/2", "sequence/3"),
        original.replace("sequence/1", "sequence/2"),
        valid.replacen(",\"precheck_profile\":null", "", 1),
        checked_document(&[first, second.replace("null", "true")]),
        valid.replace("precheck-1.json", "../precheck.json"),
        valid.replace("precheck-1.json", ""),
        valid.replacen(r#""precheck_profile":null"#, r#""precheck_profile":null,"precheck_profile":null"#, 1),
    ] {
        assert_ne!(invalid, valid, "a mutation must change the selected grammar");
        assert!(decode(invalid.as_bytes(), base).is_err(), "accepted {invalid}");
    }
    let finish = checked_document(&[checked_entry("second.json", "actor-3.json", "finish", "null")]);
    assert!(decode(finish.as_bytes(), base).is_err());
    assert!(decode_selected(finish.as_bytes(), base, Some(92)).unwrap()[0].precheck.is_none());
}

#[test]
fn native_sequence_v2_loader_freezes_every_disclosure_before_first_model_input() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let plan = root.0.join("sequence.json");
    fs::write(root.0.join("check-first.json"), settings(&root.0.join("check-first.sock"), 81, 1)).unwrap();
    fs::write(root.0.join("check-next.json"), "{}").unwrap();
    fs::write(&plan, checked_document(&[
        checked_entry("missing-model-recipe.json", "actor-0.json", "message", r#""check-first.json""#),
        checked_entry("missing-model-recipe.json", "actor-1.json", "message", r#""check-next.json""#),
    ])).unwrap();
    let error = load(&plan, &config, &peers).err().unwrap();
    assert!(error.contains("missing precheck field schema"), "late disclosure must precede missing recipe: {error}");
    fs::write(root.0.join("check-next.json"), settings(&entries[0].actor.socket, 82, 1)).unwrap();
    let error = load(&plan, &config, &peers).err().unwrap();
    assert!(error.contains("conflicts"), "future alias must precede missing recipe: {error}");
    fs::write(root.0.join("check-next.json"), settings(&root.0.join("check-next.sock"), 82, 1)).unwrap();
    let error = load(&plan, &config, &peers).err().unwrap();
    assert!(!error.contains("precheck") && !error.contains("conflicts"), "valid disclosure must reach original recipe admission: {error}");
    assert!(!config.store.exists());
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
}

#[test]
fn native_sequence_v2_loads_message_and_finish_disclosures_and_explicit_null() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let recipe = fs::read_to_string(root.0.join("recipe.json")).unwrap();
    fs::write(root.0.join("second.json"), recipe.replace("\"generation\":7", "\"generation\":8")).unwrap();
    for (index, id) in [(0, 81), (2, 83)] {
        fs::write(root.0.join(format!("check-{index}.json")), settings(&root.0.join(format!("check-{index}.sock")), id, 1)).unwrap();
    }
    let path = root.0.join("sequence.json");
    fs::write(&path, checked_document(&[
        checked_entry("recipe.json", "actor-0.json", "message", r#""check-0.json""#),
        checked_entry("second.json", "actor-1.json", "message", "null"),
        checked_entry("second.json", "actor-2.json", "finish", r#""check-2.json""#),
    ])).unwrap();
    let loaded = load(&path, &config, &peers).unwrap();
    assert!(loaded[0].precheck.is_some() && loaded[1].precheck.is_none() && loaded[2].precheck.is_some());
    assert_eq!(loaded[1].inputs.generation, loaded[2].inputs.generation);
    assert!(matches!(loaded[2].release, Release::Finish));
    assert!(!config.store.exists());
}

#[test]
fn resident_native_disclosures_share_one_finite_schedule_ceiling_and_unique_identity() {
    let root = Root::new();
    let (config, peers, mut entries) = selected(&root);
    entries.truncate(2);
    for (index, entry) in entries.iter_mut().enumerate() {
        entry.precheck = Some(selection(&root.0.join(format!("check-{index}.sock")), 81 + index as u64, 64));
    }
    validate(&config, &entries, &peers).unwrap();
    entries[1].precheck = Some(selection(&root.0.join("check-1.sock"), 82, 65));
    assert!(validate(&config, &entries, &peers).unwrap_err().contains("aggregate"));
    entries[1].precheck = Some(selection(&root.0.join("check-1.sock"), 81, 64));
    assert!(validate(&config, &entries, &peers).unwrap_err().contains("distinct disclosure IDs"));
    for (field, value) in [("exchange_limit", 128), ("candidate_limit", 1024)] {
        let first = settings(&root.0.join("check-0.sock"), 81, 1)
            .replace(&format!("\"{field}\":{}", if field == "exchange_limit" { 16 } else { 8 }), &format!("\"{field}\":{value}"));
        entries[0].precheck = Some(exact_precheck::Selection::new(Settings::decode(first.as_bytes()).unwrap()).unwrap());
        entries[1].precheck = Some(selection(&root.0.join("check-1.sock"), 82, 1));
        assert!(validate(&config, &entries, &peers).unwrap_err().contains("aggregate"));
    }
    assert!(!config.store.exists());
}

#[test]
fn resident_native_disclosure_admission_protects_all_future_names_and_canonical_aliases() {
    let root = Root::new();
    let (config, peers, mut entries) = selected(&root);
    entries[0].precheck = Some(selection(&root.0.join("check-0.sock"), 81, 1));
    validate(&config, &entries, &peers).unwrap();
    for collision in [
        entries[2].actor.socket.clone(),
        peers.socket(entries[2].actor.request),
        crate::workflow::control::socket_path(&peers, entries[2].actor.request),
        config.source.path().to_owned(),
        config.store.join("query.sock"),
    ] {
        entries[0].precheck = Some(selection(&collision, 81, 1));
        assert!(validate(&config, &entries, &peers).is_err(), "accepted future collision {collision:?}");
        assert!(!config.store.exists());
    }
    let real = root.0.join("real");
    let alias = root.0.join("alias");
    fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    entries[0].precheck = Some(selection(&real.join("query.sock"), 81, 1));
    entries[1].precheck = Some(selection(&alias.join("other.sock"), 82, 1));
    validate(&config, &entries, &peers).unwrap();
    entries[1].precheck = Some(selection(&alias.join("query.sock"), 82, 1));
    assert!(validate(&config, &entries, &peers).unwrap_err().contains("conflicts"));
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
}
