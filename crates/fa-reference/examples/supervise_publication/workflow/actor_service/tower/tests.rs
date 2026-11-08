use super::*;

#[path = "tests/operator.rs"]
mod operator;

#[path = "tests/topology.rs"]
mod topology;

#[path = "tests/recovery.rs"]
mod recovery;

#[test]
fn explicit_generation_selects_only_the_four_ordinary_service_profiles() {
    for (mode, count, open) in [
        ("serve-create", 4, false),
        ("serve-open", 4, true),
        ("serve-create-checked", 5, false),
        ("serve-open-checked", 5, true),
    ] {
        let mut args = vec![mode.to_owned()];
        args.extend((1..count).map(|index| format!("path-{index}")));
        args.extend([FLAG.to_owned(), u64::MAX.to_string()]);
        let parsed = Arguments::parse(&args).unwrap();
        assert_eq!(parsed.open, open);
        assert_eq!(parsed.generation, u64::MAX);
        assert_eq!(parsed.config, Path::new("path-1"));
        assert_eq!(parsed.publication.is_some(), count == 5);
        assert!(parsed.requests.is_none());
        assert!(parsed.precheck.is_none());
    }
}

#[test]
fn tower_selection_cannot_be_silently_consumed_by_another_service_grammar() {
    for values in [
        vec!["serve-create", "config", "actor", "reviewer", FLAG],
        vec![
            "serve-create",
            "config",
            "actor",
            "reviewer",
            FLAG,
            "1",
            FLAG,
            "2",
        ],
        vec![
            "serve-open",
            "config",
            "actor",
            "reviewer",
            FLAG,
            "1",
            "--peers",
            "p",
        ],
        vec![
            "serve-create",
            "config",
            "actor",
            "reviewer",
            "--precheck-profile",
            "",
            FLAG,
            "1",
        ],
        vec![
            "serve-open",
            "config",
            "actor",
            "reviewer",
            "--receipts-only",
            FLAG,
            "1",
        ],
        vec![
            "serve-open",
            "config",
            "actor",
            "reviewer",
            "--topology-profile",
            "t",
            FLAG,
            "1",
        ],
        vec![
            "serve-create",
            "config",
            "actor",
            "reviewer",
            "--native-text",
            "n",
            FLAG,
            "1",
        ],
        vec![
            "create-generated",
            "config",
            "recipe",
            "reviewer",
            FLAG,
            "1",
        ],
        vec![
            "create-learned-generated",
            "config",
            "recipe",
            "reviewer",
            FLAG,
            "1",
        ],
        vec![
            "serve-learned-generated",
            "config",
            "recipe",
            "reviewer",
            FLAG,
            "1",
        ],
        vec!["actor-submit", "actor", "proposal", FLAG, "1"],
        vec![
            "serve-create",
            "config",
            "--precheck-profile",
            "reviewer",
            FLAG,
            "1",
        ],
    ] {
        let args: Vec<_> = values.into_iter().map(str::to_owned).collect();
        assert!(Arguments::parse(&args).is_err(), "accepted {args:?}");
        // The containing dispatcher also rejects before any nonexistent path
        // is read; this covers the new early grammar selection itself.
        assert!(
            super::super::command(&args, None)
                .unwrap_err()
                .contains("serve-create")
        );
    }
    for generation in [
        "",
        "0",
        "01",
        "+1",
        "-1",
        "1.0",
        "1e0",
        " 1",
        "18446744073709551616",
    ] {
        let args: Vec<_> = [
            "serve-create",
            "config",
            "actor",
            "reviewer",
            FLAG,
            generation,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert!(Arguments::parse(&args).is_err());
    }
}

#[test]
fn explicit_tower_precheck_selects_the_original_sidecar_in_either_option_order() {
    for mode in [
        "serve-create",
        "serve-open",
        "serve-create-checked",
        "serve-open-checked",
    ] {
        for generation_first in [false, true] {
            let mut args: Vec<String> = [mode, "config", "actor", "reviewer"]
                .into_iter()
                .map(str::to_owned)
                .collect();
            if mode.ends_with("-checked") {
                args.push("witness".into());
            }
            let options = if generation_first {
                [FLAG, "41", "--precheck-profile", "precheck.json"]
            } else {
                ["--precheck-profile", "precheck.json", FLAG, "41"]
            };
            args.extend(options.into_iter().map(str::to_owned));
            let selected = Arguments::parse(&args).unwrap();
            assert_eq!(selected.generation, 41);
            assert_eq!(selected.precheck, Some(Path::new("precheck.json")));
            assert!(selected.requests.is_none());
            assert_eq!(selected.open, mode.starts_with("serve-open"));
            assert_eq!(selected.publication.is_some(), mode.ends_with("-checked"));
        }
    }
}

#[test]
fn tower_precheck_rejects_incomplete_duplicate_and_scheduled_options_before_reads() {
    for suffix in [
        vec![FLAG, "41", "--precheck-profile"],
        vec![FLAG, "41", "--precheck-profile", ""],
        vec![FLAG, "41", "--precheck-profile", "--requests"],
        vec![
            "--precheck-profile",
            "first",
            "--precheck-profile",
            "second",
            FLAG,
            "41",
        ],
        vec![FLAG, "41", "--precheck-profile", "first", FLAG, "42"],
        vec![FLAG, "41", "--precheck-profile", "first", "--requests", "7"],
        vec![
            "--requests",
            "7,8",
            FLAG,
            "41",
            "--precheck-profile",
            "first",
        ],
        vec![FLAG, "41", "--precheck-profile", "first", "--peers", "peer"],
        vec![
            FLAG,
            "41",
            "--precheck-profile",
            "first",
            "--topology-profile",
            "topology",
        ],
        vec![FLAG, "41", "--precheck-profile", "first", "--receipts-only"],
    ] {
        let args: Vec<_> = [
            "serve-open",
            "unread-config",
            "unread-actor",
            "unread-reviewer",
        ]
        .into_iter()
        .chain(suffix)
        .map(str::to_owned)
        .collect();
        let expected = Arguments::parse(&args).err().unwrap();
        assert_eq!(super::super::command(&args, None).unwrap_err(), expected);
        let mut output = Vec::new();
        assert_eq!(
            crate::command_with_output(args, &mut output).unwrap_err(),
            expected
        );
        assert!(output.is_empty());
    }
}

#[test]
fn explicit_tower_schedule_uses_the_original_bounded_key_grammar() {
    for mode in [
        "serve-create",
        "serve-open",
        "serve-create-checked",
        "serve-open-checked",
    ] {
        for generation_first in [false, true] {
            let mut args: Vec<String> = [mode, "config", "actor", "reviewer"]
                .into_iter()
                .map(str::to_owned)
                .collect();
            if mode.ends_with("-checked") {
                args.push("witness".into());
            }
            let options = if generation_first {
                [FLAG, "41", "--requests", "7,8,18446744073709551615"]
            } else {
                ["--requests", "7,8,18446744073709551615", FLAG, "41"]
            };
            args.extend(options.into_iter().map(str::to_owned));
            let selected = Arguments::parse(&args).unwrap();
            assert_eq!(selected.generation, 41);
            assert_eq!(selected.requests, Some(vec![7, 8, u64::MAX]));
        }
    }
    let mut args: Vec<String> = [
        "serve-create",
        "config",
        "actor",
        "reviewer",
        FLAG,
        "41",
        "--requests",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    args.push(
        (1..=64)
            .map(|key| key.to_string())
            .collect::<Vec<_>>()
            .join(","),
    );
    assert_eq!(Arguments::parse(&args).unwrap().requests.unwrap().len(), 64);
    args.last_mut().unwrap().push_str(",65");
    assert!(Arguments::parse(&args).is_err());
}

#[test]
fn malformed_tower_schedules_and_mixed_profiles_refuse_before_reading_files() {
    for keys in [
        "",
        "0",
        "7,0",
        "07,8",
        "7,7",
        "7,",
        "7,,8",
        "+7",
        "7, 8",
        "18446744073709551616",
    ] {
        let args: Vec<_> = [
            "serve-create",
            "unread-config",
            "unread-actor",
            "unread-reviewer",
            FLAG,
            "41",
            "--requests",
            keys,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let expected = Arguments::parse(&args).err().unwrap();
        assert_eq!(super::super::command(&args, None).unwrap_err(), expected);
    }
    for suffix in [
        vec![FLAG, "41", "--requests"],
        vec![FLAG, "41", "--requests", "7,8", "--requests", "9"],
        vec![FLAG, "41", "--requests", "7,8", FLAG, "42"],
        vec![
            FLAG,
            "41",
            "--requests",
            "7,8",
            "--precheck-profile",
            "unread",
        ],
        vec![FLAG, "41", "--requests", "7,8", "--peers", "unread"],
        vec![
            FLAG,
            "41",
            "--requests",
            "7,8",
            "--topology-profile",
            "unread",
        ],
        vec![FLAG, "41", "--requests", "7,8", "--receipts-only"],
    ] {
        let args: Vec<_> = [
            "serve-open",
            "unread-config",
            "unread-actor",
            "unread-reviewer",
        ]
        .into_iter()
        .chain(suffix)
        .map(str::to_owned)
        .collect();
        let expected = Arguments::parse(&args).err().unwrap();
        assert_eq!(super::super::command(&args, None).unwrap_err(), expected);
    }
}
