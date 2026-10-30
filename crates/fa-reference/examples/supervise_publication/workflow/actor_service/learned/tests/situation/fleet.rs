//! A mixed declared fleet consumes the original ordinary and numerical readers.
use super::*;

fn manifest_member(id: usize, config: &Path, recipe: Option<&Path>, anchor: Option<&Path>) -> String {
    let optional = |path: Option<&Path>| path.map_or_else(|| "null".to_owned(),
        |path| format!("\"{}\"", path.display()));
    format!(r#"{{"id":{id},"config":"{}","learned_recipe":{},"history_anchor":{}}}"#,
        config.display(), optional(recipe), optional(anchor))
}
fn fleet_args(root: &Root, entries: &[String]) -> Vec<String> {
    let path = root.0.join("fleet-situation.json");
    fs::write(&path, format!(r#"{{"schema":"fa.publication-situation-fleet/1","runtime_ms":60000,"input_bytes":268435456,"journal_bytes":268435456,"output_bytes":4194304,"replay_events":65536,"page_size":1,"members":[{}]}}"#,
        entries.join(","))).unwrap();
    vec!["situation".into(), "--fleet".into(), path.to_str().unwrap().into()]
}
fn fleet_pages(bytes: &[u8]) -> BTreeMap<String, Json> {
    assert!(bytes.ends_with(b"\n"));
    bytes[..bytes.len() - 1].split(|byte| *byte == b'\n').map(|line| {
        let json = strict_json::parse(line, Limits { max_bytes: 66_560, max_depth: 16,
            max_items: 4096, max_string_bytes: 256 }).unwrap();
        (field(&json, &["member"]).as_str().unwrap().to_owned(), json)
    }).collect()
}

#[test]
fn mixed_ordinary_plain_raw_and_owned_fleet_replays_without_live_assets_and_isolates_recipe_failure() {
    let mut selected = Vec::new(); let mut entries = Vec::new(); let mut saved = Vec::new();
    for lane in 0..3 {
        let root = Root::new(); let mut config = configured(&root);
        let recipe_path = if lane == 0 { fixture::write(&root, b"allow") } else {
            let path = super::super::continuation::write(&root);
            super::super::forecast::install(&root, &path, lane == 2, 1000); path
        };
        evidence(&root, &config, 1, true, false);
        let loaded = recipe::load(&recipe_path, &config, true).unwrap();
        let (host, _reviewer, _observer, _identity) = recovery::create_selected(&config, &loaded).unwrap();
        let (_port, mut driver) = host.into_supervised_driver();
        driver.prepare_file_intake(&mut config.source, &mut || ElapsedTick(1000)).result.unwrap();
        {
            let mut host = driver.supervisor_mut().host_mut().unwrap();
            let n = host.learned_generation_inspection().unwrap().numerical;
            let revision = host.revision();
            host.advance_learned_generation(revision, n.actor_revision, n.position).unwrap().unwrap();
        }
        let config_path = config_file(&root, &config);
        {
            let host = driver.supervisor().host().unwrap();
            let anchor = save_anchor(&root, &host.history_anchor().unwrap());
            entries.push(manifest_member(lane + 1, &config_path, Some(&recipe_path), Some(&anchor)));
            saved.push((host.inspect(), host.learned_generation_inspection().unwrap(), files(&config)));
        }
        for name in ["evidence.json", "native-roster.json", "helper-model.json", "helper-weights.safetensors"] {
            fs::remove_file(root.0.join(name)).unwrap();
        }
        selected.push((root, config, driver));
    }
    let ordinary_root = Root::new(); let ordinary = configured(&ordinary_root);
    let (host, _) = FileOversight::create(&ordinary.store, ordinary.profile.clone()).unwrap();
    let ordinary_path = config_file(&ordinary_root, &ordinary);
    entries.push(manifest_member(4, &ordinary_path, None, None));
    let before = (host.inspect(), files(&ordinary));
    let argv = fleet_args(&ordinary_root, &entries); let mut out = Vec::new();
    crate::situation::command(&argv, &mut out).unwrap();
    let pages = fleet_pages(&out); assert_eq!(pages.len(), 4);
    for page in pages.values() {
        assert_eq!(field(page, &["member_state"]).as_str(), Some("known"));
        assert_eq!(field(page, &["coverage", "known_members"]).as_str(), Some("4"));
        assert_eq!(field(page, &["all_members_known"]).as_bool(), Some(true));
        assert_eq!(field(page, &["atomic_cut"]).as_bool(), Some(false));
    }
    for (lane, (_, _, driver)) in selected.iter().enumerate() {
        let host = driver.supervisor().host().unwrap();
        assert_eq!(field(&pages[&(lane + 1).to_string()], &["situation", "knowledge", "basis", "journal_revision"])
            .as_str().unwrap(), host.revision().to_string());
    }
    let prompt = selected[1].0.0.join("prompt.txt"); let mut changed = fs::read(&prompt).unwrap();
    changed.push(b'x'); fs::write(&prompt, changed).unwrap(); out.clear();
    assert!(crate::situation::command(&argv, &mut out).unwrap_err().contains("unknown members"));
    let pages = fleet_pages(&out);
    assert_eq!(field(&pages["2"], &["member_state"]).as_str(), Some("unknown"));
    assert!(matches!(field(&pages["2"], &["situation"]), Json::Null));
    for id in ["1", "3", "4"] {
        assert_eq!(field(&pages[id], &["member_state"]).as_str(), Some("known"));
        assert_eq!(field(&pages[id], &["coverage", "unknown_members"]).as_str(), Some("1"));
    }
    for ((_, config, driver), (state, numerical, original)) in selected.iter().zip(saved) {
        let host = driver.supervisor().host().unwrap();
        assert_eq!(host.inspect(), state); assert_eq!(host.learned_generation_inspection().unwrap(), numerical);
        assert_eq!(files(config), original);
    }
    assert_eq!(before, (host.inspect(), files(&ordinary)));
}

#[test]
fn fleet_loader_conserves_intake_allowance_and_preserves_each_original_deadline_boundary() {
    let root = Root::new(); let config = configured(&root); let path = fixture::write(&root, b"allow");
    let loaded = recipe::load(&path, &config, false).unwrap();
    let (host, _) = recovery::create(&config, &loaded).unwrap();
    let roots = vec![fs::canonicalize(&config.store).unwrap()]; let original = files(&config);
    let initial = 128 * 1024 * 1024; let mut remaining = initial;
    let cursor = config.begin_learned_fleet_situation(&path, None, &mut remaining, &roots, &mut || Ok(())).unwrap();
    assert!(remaining < initial && remaining > initial - 64 * 1024 * 1024);
    assert_eq!(cursor.progress().checked_events, 0); drop(cursor);
    let spent = remaining;
    assert!(config.begin_learned_fleet_situation(&root.0.join("missing"), None,
        &mut remaining, &roots, &mut || Ok(())).is_err());
    assert_eq!(remaining, spent - 64 * 1024 * 1024);
    for cut in 0..3 {
        let mut remaining = initial; let mut calls = 0;
        let error = config.begin_learned_fleet_situation(&path, None, &mut remaining, &roots, &mut || {
            let at = calls; calls += 1;
            if at == cut { Err("selected elapsed boundary".to_owned()) } else { Ok(()) }
        }).unwrap_err();
        assert_eq!(error, "selected elapsed boundary"); assert_eq!(calls, cut + 1);
        if cut == 0 { assert_eq!(remaining, initial); }
    }
    assert_eq!(files(&config), original); assert_eq!(host.inspect().executions, 0);
}
