//! Admission tests use original recipe/model/tokenizer and peer constructors.
//! They do not claim runtime publication or numerical qualification.

use super::*;
use crate::workflow::actor_service::native::*;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, Merge, TokenBytes,
};
use fa_reference::action::consequence::delivery::stream::StreamProfile;
use fa_reference::action::consequence::oversight::actor_peer::PeerPolicy;
use std::os::unix::net::UnixStream;

mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/supervise_publication/workflow/actor_service/native/tests/fixture.rs"
    ));
}
use fixture::{Root, configured, inputs, profiles};

#[path = "tests/operator.rs"]
mod operator_tests;
#[path = "tests/precheck.rs"]
mod precheck_tests;
#[path = "tests/combined.rs"]
mod combined;
#[path = "tests/qualification.rs"]
mod qualification_tests;

fn entry(recipe: &str, actor: &str, release: &str) -> String {
    format!(r#"{{"recipe":"{recipe}","actor_profile":"{actor}","release":"{release}"}}"#)
}

fn document(entries: &[String]) -> String {
    format!(
        r#"{{"schema":"fa.native-publication-sequence/1","entries":[{}]}}"#,
        entries.join(",")
    )
}

fn copy_inputs(original: &Inputs, generation: u64) -> Inputs {
    Inputs {
        decoder: original.decoder.clone(),
        tokenizer: original.tokenizer.clone(),
        stream: original.stream,
        generation,
        request: original.request.clone(),
    }
}

fn selected(root: &Root) -> (Config, PeerProfile, Vec<Entry>) {
    let config = configured(root);
    let original = inputs(root, &config);
    let (actor, peers) = profiles(root, &config);
    let entries = [
        (7, Release::Message),
        (8, Release::Message),
        (8, Release::Finish),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (generation, release))| {
        let mut actor = actor.clone();
        actor.request += index as u64;
        actor.socket = root.0.join(format!("actor-{index}.sock"));
        Entry {
            actor,
            inputs: copy_inputs(&original, generation),
            release,
            precheck: None,
        }
    })
    .collect();
    (config, peers, entries)
}

fn save_actor(path: &Path, actor: &Profile) {
    let identity = |policy: PeerPolicy| {
        let pid = policy
            .pid()
            .map_or_else(|| "null".to_owned(), |pid| pid.to_string());
        format!(
            r#"{{"uid":{},"gid":{},"pid":{pid}}}"#,
            policy.uid(),
            policy.gid()
        )
    };
    let scope = actor.scope;
    let bytes = format!(
        r#"{{"schema":"fa.actor-service/1","clock":"unix_milliseconds","request":{},
        "scope":{{"tenant":{},"principal":{},"run":{},"branch":{},"authority":{}}},
        "socket":"{}","supervisor":{},"actor":{},"candidate_limit":{},
        "connection_limit":{},"exchange_limit":{},"runtime_ms":{},"poll_ms":{},"reply_ms":{}}}"#,
        actor.request,
        scope.tenant,
        scope.principal,
        scope.run,
        scope.branch,
        scope.authority,
        actor.socket.display(),
        identity(actor.supervisor),
        identity(actor.actor),
        actor.candidates,
        actor.connections,
        actor.exchanges,
        actor.runtime_ms,
        actor.poll_ms,
        actor.reply_ms,
    );
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn native_resident_profile_admits_complete_stream_and_bounded_message_schedules() {
    let base = Path::new("/operator/native");
    let complete = document(&[
        entry("recipe.json", "actor-1.json", "message"),
        entry("recipe.json", "/operator/actor-2.json", "finish"),
    ]);
    let parsed = decode(complete.as_bytes(), base).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].recipe, base.join("recipe.json"));
    assert_eq!(parsed[1].actor, Path::new("/operator/actor-2.json"));
    assert!(matches!(parsed[0].release, Release::Message));
    assert!(matches!(parsed[1].release, Release::Finish));
    let maximum = (0..MAX_ENTRIES)
        .map(|index| {
            entry(
                &format!("recipe-{index}.json"),
                &format!("actor-{index}.json"),
                "message",
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        decode(document(&maximum).as_bytes(), base).unwrap().len(),
        MAX_ENTRIES
    );
}

#[test]
fn native_resident_profile_rejects_ambiguous_implicit_and_unbounded_releases() {
    let base = Path::new("/operator/native");
    let message = entry("recipe.json", "actor-1.json", "message");
    let finish = entry("recipe.json", "actor-2.json", "finish");
    for entries in [
        vec![],
        vec![message.clone()],
        vec![message.clone(); MAX_ENTRIES + 1],
        vec![finish.clone(), message.clone()],
        vec![message.clone(), finish.clone(), message.clone()],
        vec![
            message.clone(),
            entry("recipe.json", "actor-2.json", "continue"),
        ],
    ] {
        assert!(decode(document(&entries).as_bytes(), base).is_err());
    }
    let valid = document(&[message, finish]);
    for invalid in [
        valid.replace("sequence/1", "sequence/3"),
        valid.replace(r#""entries":["#, r#""entries":[],"entries":["#),
        valid.replacen(
            r#""release":"message""#,
            r#""release":"message","automatic_human_approval":true"#,
            1,
        ),
        valid.replacen(r#""actor_profile":"actor-1.json","#, "", 1),
        valid.replace("recipe.json", "../recipe.json"),
        valid.replace("recipe.json", "./recipe.json"),
        valid.replace("recipe.json", "nested/../recipe.json"),
        valid.replace("recipe.json", ""),
        valid.replace("recipe.json", "recipe\\u0000.json"),
        valid.replace(
            r#""entries":["#,
            r#""operator_profile":"operator.json","entries":["#,
        ),
    ] {
        assert!(
            decode(invalid.as_bytes(), base).is_err(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn native_resident_command_owns_all_options_before_input_or_clock_work() {
    let base = [
        "serve-native-generated",
        "/missing/config",
        "/missing/sequence",
        "/missing/reviewer",
    ]
    .map(str::to_owned)
    .to_vec();
    assert_eq!(options(&base, None).unwrap(), (None, None));
    let mut checked = base.clone();
    checked.extend([
        "--publication-profile".to_owned(),
        "/missing/witness".to_owned(),
    ]);
    assert_eq!(
        options(&checked, None).unwrap(),
        (Some(PathBuf::from("/missing/witness")), None)
    );
    let mut rejected = vec![vec![], base[..3].to_vec()];
    for flag in [
        "--continue-generation",
        "--native-text",
        "--operator-profile",
        "--tower-generation",
        "--precheck-profile",
        "--precheck-schedule",
        "--governor-profile",
        "--topology-profile",
        "--guarded",
        "--finish-after-message",
        "--requests",
    ] {
        let mut args = base.clone();
        args.extend([flag.to_owned(), "1".to_owned()]);
        rejected.push(args);
    }
    let mut missing = base.clone();
    missing.push("--publication-profile".to_owned());
    rejected.push(missing);
    let mut duplicate = checked.clone();
    duplicate.extend([
        "--publication-profile".to_owned(),
        "/second/witness".to_owned(),
    ]);
    rejected.push(duplicate);
    let mut wrong = base.clone();
    wrong[0] = "serve-open".into();
    rejected.push(wrong);
    let mut option_value = checked;
    option_value[5] = "--native-text".into();
    rejected.push(option_value);
    for args in rejected {
        let mut output = Vec::new();
        assert_eq!(
            command_with_clock(
                &args,
                None,
                || panic!("invalid argv acquired a clock"),
                &mut output
            ),
            Err(USAGE.to_owned()),
        );
        assert!(output.is_empty());
    }
    assert_eq!(
        command_with_clock(
            &base,
            Some(Path::new("/missing/credibility")),
            || panic!("unsupported qualification acquired a clock"),
            &mut Vec::new()
        ),
        Err(USAGE.to_owned()),
    );
}

#[test]
fn native_resident_recovery_options_bind_one_canonical_predecessor_before_io() {
    let base = [
        "serve-native-generated",
        "/missing/config",
        "/missing/sequence",
        "/missing/reviewer",
    ]
    .map(str::to_owned)
    .to_vec();
    for after in [1, u64::MAX] {
        let mut resumed = base.clone();
        resumed.extend(["--after-message".to_owned(), after.to_string()]);
        assert_eq!(options(&resumed, None).unwrap(), (None, Some(after)));
        let mut first_publication = base.clone();
        first_publication.extend([
            "--publication-profile".to_owned(),
            "/missing/witness".to_owned(),
            "--after-message".to_owned(),
            after.to_string(),
        ]);
        let mut first_recovery = resumed;
        first_recovery.extend([
            "--publication-profile".to_owned(),
            "/missing/witness".to_owned(),
        ]);
        for args in [first_publication, first_recovery] {
            assert_eq!(
                options(&args, None).unwrap(),
                (Some(PathBuf::from("/missing/witness")), Some(after))
            );
            assert_eq!(
                options(&args, Some(Path::new("/missing/capsule"))).unwrap(),
                (Some(PathBuf::from("/missing/witness")), Some(after))
            );
            // The admitted grammar now reaches the missing config, still
            // before any clock, owner or qualification source can be acquired.
            assert_ne!(
                command_with_clock(
                    &args,
                    Some(Path::new("/missing/capsule")),
                    || panic!("missing recovery config acquired time"),
                    &mut Vec::new()
                ),
                Err(USAGE.to_owned())
            );
        }
    }
    let mut rejected = Vec::new();
    for after in [
        "0",
        "00",
        "01",
        "+1",
        "-1",
        "1.0",
        " 1",
        "1 ",
        "",
        "18446744073709551616",
    ] {
        let mut args = base.clone();
        args.extend(["--after-message".to_owned(), after.to_owned()]);
        rejected.push(args);
    }
    let mut missing = base.clone();
    missing.push("--after-message".into());
    rejected.push(missing);
    let mut duplicate = base.clone();
    duplicate.extend(["--after-message", "91", "--after-message", "92"].map(str::to_owned));
    rejected.push(duplicate);
    let mut unselected = base.clone();
    unselected.extend(["--after-message", "91", "--continue-generation", "8"].map(str::to_owned));
    rejected.push(unselected);
    for args in rejected {
        let mut output = Vec::new();
        assert_eq!(
            command_with_clock(
                &args,
                None,
                || panic!("invalid recovery options acquired time"),
                &mut output
            ),
            Err(USAGE.to_owned())
        );
        assert!(output.is_empty());
    }
}

#[test]
fn native_resident_remaining_manifest_requires_explicit_recovery_and_a_final_finish() {
    let base = Path::new("/operator/native");
    for release in ["message", "finish"] {
        let remaining = document(&[entry("recipe.json", "actor.json", release)]);
        assert!(decode(remaining.as_bytes(), base).is_err());
        let parsed = decode_selected(remaining.as_bytes(), base, Some(91)).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            matches!(parsed[0].release, Release::Finish),
            release == "finish"
        );
        assert!(decode_selected(remaining.as_bytes(), base, Some(0)).is_err());
        let unknown = remaining.replace("\"entries\":[", "\"automatic_resume\":true,\"entries\":[");
        assert!(decode_selected(unknown.as_bytes(), base, Some(91)).is_err());
    }
    for invalid in [
        vec![],
        vec![entry("recipe.json", "actor.json", "message"); MAX_ENTRIES + 1],
        vec![
            entry("first.json", "actor-1.json", "finish"),
            entry("next.json", "actor-2.json", "message"),
        ],
        vec![
            entry("first.json", "actor-1.json", "message"),
            entry("next.json", "actor-2.json", "finish"),
            entry("third.json", "actor-3.json", "finish"),
        ],
    ] {
        assert!(decode_selected(document(&invalid).as_bytes(), base, Some(91)).is_err());
    }
    let maximum = (0..MAX_ENTRIES)
        .map(|index| {
            entry(
                &format!("recipe-{index}.json"),
                &format!("actor-{index}.json"),
                "message",
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        decode_selected(document(&maximum).as_bytes(), base, Some(91))
            .unwrap()
            .len(),
        MAX_ENTRIES
    );
}

#[test]
fn native_resident_remaining_admission_keeps_creation_and_predecessor_boundaries() {
    let root = Root::new();
    let (config, peers, mut entries) = selected(&root);
    validate(&config, &entries, &peers).unwrap();
    for after in [0, 91, 92, 93] {
        assert!(validate_selected(&config, &entries, &peers, Some(after)).is_err());
    }
    entries.remove(0);
    validate_selected(&config, &entries, &peers, Some(91)).unwrap();
    let finish = entries.remove(1);
    assert!(validate(&config, &entries, &peers).is_err());
    validate_selected(&config, &entries, &peers, Some(91)).unwrap();
    entries[0] = finish;
    assert!(validate(&config, &entries, &peers).is_err());
    validate_selected(&config, &entries, &peers, Some(92)).unwrap();
    entries[0].inputs.generation = 0;
    assert!(validate_selected(&config, &entries, &peers, Some(92)).is_err());
    assert!(!config.store.exists());
    assert!(!entries[0].actor.socket.exists());
}

#[test]
fn native_resident_remaining_loader_rejects_predecessor_reuse_before_recipe_reads() {
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let path = root.0.join("remaining.json");
    std::fs::write(
        &path,
        document(&[
            entry("missing-recipe.json", "actor-1.json", "message"),
            entry("missing-recipe.json", "actor-2.json", "finish"),
        ]),
    )
    .unwrap();
    let error = load_selected(&path, &config, &peers, Some(93))
        .err()
        .unwrap();
    assert!(
        error.contains("predecessor cannot be a remaining actor request"),
        "{error}"
    );
    let recipe = std::fs::read_to_string(root.0.join("recipe.json")).unwrap();
    std::fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    )
    .unwrap();
    std::fs::write(
        &path,
        document(&[entry("second.json", "actor-2.json", "finish")]),
    )
    .unwrap();
    assert!(load(&path, &config, &peers).is_err());
    let remaining = load_selected(&path, &config, &peers, Some(92)).unwrap();
    assert_eq!(remaining.len(), 1);
    assert!(matches!(remaining[0].release, Release::Finish));
    assert_eq!(remaining[0].inputs.generation, 8);
    assert_eq!(remaining[0].actor.request, 93);
    assert!(!config.store.exists());
    assert!(!remaining[0].actor.socket.exists());
}

#[test]
fn native_resident_admission_keeps_each_selected_actor_policy_and_original_source_contract() {
    let root = Root::new();
    let (config, peers, mut entries) = selected(&root);
    let policy = entries[1].actor.actor;
    entries[1].actor.actor = PeerPolicy::new(policy.uid(), policy.gid(), None).unwrap();
    entries[1].actor.exchanges = 17;
    entries[1].actor.connections = 2;
    validate(&config, &entries, &peers).unwrap();
    let first = entries.remove(0);
    entries[0].inputs.generation = first.inputs.generation;
    entries[1].inputs.generation = first.inputs.generation;
    validate(&config, &entries, &peers).unwrap();
    assert!(!config.store.exists());
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
}

#[test]
fn native_resident_admission_refuses_generation_reuse_and_invented_finish_predecessors() {
    let root = Root::new();
    for scenario in 0..7 {
        let (config, peers, mut entries) = selected(&root);
        match scenario {
            0 => entries[1].actor.request = entries[0].actor.request,
            1 => entries[1].inputs.generation = entries[0].inputs.generation,
            2 => entries[1].inputs.generation = 0,
            3 => entries[0].release = Release::Finish,
            4 => entries[1].release = Release::Finish,
            5 => entries[2].inputs.generation = entries[0].inputs.generation,
            _ => entries[2].inputs.request.prompt.push(b'!'),
        }
        assert!(
            validate(&config, &entries, &peers).is_err(),
            "accepted scenario {scenario}"
        );
        assert!(!config.store.exists());
    }
}

#[test]
fn native_resident_admission_compares_complete_tokenizer_stream_and_model_inputs() {
    let root = Root::new();
    let (config, peers, mut entries) = selected(&root);
    let original = entries[1].inputs.tokenizer.clone();
    let mut vocabulary = (0..=255)
        .map(|byte| TokenBytes::Content(vec![byte]))
        .collect::<Vec<_>>();
    vocabulary.extend([
        TokenBytes::Control,
        TokenBytes::Content(b"ab".to_vec()),
        TokenBytes::Content("é".as_bytes().to_vec()),
    ]);
    entries[1].inputs.tokenizer = ByteBpe::new(
        original.profile().clone(),
        vocabulary,
        vec![
            Merge {
                left: 195,
                right: 169,
                result: 258,
            },
            Merge {
                left: 97,
                right: 98,
                result: 257,
            },
        ],
    )
    .unwrap();
    assert!(validate(&config, &entries, &peers).is_err());
    entries[1].inputs.tokenizer = original;
    validate(&config, &entries, &peers).unwrap();
    let original_stream = entries[1].inputs.stream;
    entries[1].inputs.stream = StreamProfile::new(10, 1, 4, 64, 256).unwrap();
    assert!(validate(&config, &entries, &peers).is_err());
    entries[1].inputs.stream = original_stream;
    let recipe = std::fs::read_to_string(root.0.join("recipe.json")).unwrap();
    std::fs::write(
        root.0.join("other.json"),
        recipe.replace("\"cache_stream\":5", "\"cache_stream\":6"),
    )
    .unwrap();
    entries[1].inputs.decoder = recipe::load(
        &root.0.join("other.json"),
        1,
        config.profile.delivery.limits.bytes,
    )
    .unwrap()
    .decoder;
    assert!(validate(&config, &entries, &peers).is_err());
    assert!(!config.store.exists());
}

#[test]
fn native_resident_admission_resolves_cross_entry_source_store_and_symlink_endpoint_aliases() {
    let root = Root::new();
    let (config, peers, mut entries) = selected(&root);
    let original = entries[1].actor.socket.clone();
    for socket in [
        entries[0].actor.socket.clone(),
        peers.socket(entries[0].actor.request),
        crate::workflow::control::socket_path(&peers, entries[0].actor.request),
        config.source.path().to_path_buf(),
        config.store.clone(),
    ] {
        entries[1].actor.socket = socket;
        assert!(validate(&config, &entries, &peers).is_err());
    }
    let real = root.0.join("real");
    std::fs::create_dir(&real).unwrap();
    std::fs::create_dir(real.join("peers")).unwrap();
    let alias = root.0.join("alias");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    entries[0].actor.socket = real.join("peers/actor.sock");
    entries[1].actor.socket = alias.join("peers/actor.sock");
    assert!(validate(&config, &entries, &peers).is_err());
    entries[1].actor.socket = original;
    validate(&config, &entries, &peers).unwrap();
    assert!(!config.store.exists());
}

#[test]
fn native_resident_loader_freezes_original_recipes_and_rejects_endpoint_aliases_before_model_reads()
{
    let root = Root::new();
    let (config, peers, entries) = selected(&root);
    for (index, entry) in entries.iter().enumerate() {
        save_actor(&root.0.join(format!("actor-{index}.json")), &entry.actor);
    }
    let recipe = std::fs::read_to_string(root.0.join("recipe.json")).unwrap();
    std::fs::write(
        root.0.join("second.json"),
        recipe.replace("\"generation\":7", "\"generation\":8"),
    )
    .unwrap();
    let plan = document(&[
        entry("recipe.json", "actor-0.json", "message"),
        entry("second.json", "actor-1.json", "message"),
        entry("second.json", "actor-2.json", "finish"),
    ]);
    let path = root.0.join("sequence.json");
    std::fs::write(&path, plan).unwrap();
    let loaded = load(&path, &config, &peers).unwrap();
    assert_eq!(
        loaded
            .iter()
            .map(|entry| entry.actor.request)
            .collect::<Vec<_>>(),
        vec![91, 92, 93]
    );
    assert_eq!(loaded[1].inputs.generation, 8);
    assert!(matches!(loaded[2].release, Release::Finish));
    assert_eq!(loaded[1].inputs.request, loaded[2].inputs.request);
    let mut conflicting = entries[1].actor.clone();
    conflicting.socket = entries[0].actor.socket.clone();
    save_actor(&root.0.join("actor-1.json"), &conflicting);
    std::fs::write(
        &path,
        document(&[
            entry("missing-model-recipe.json", "actor-0.json", "message"),
            entry("missing-model-recipe.json", "actor-1.json", "message"),
        ]),
    )
    .unwrap();
    assert!(
        load(&path, &config, &peers)
            .err()
            .unwrap()
            .contains("endpoints must be distinct")
    );
    assert!(!config.store.exists());
    assert!(entries.iter().all(|entry| !entry.actor.socket.exists()));
}
