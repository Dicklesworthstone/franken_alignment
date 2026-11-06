use super::*;

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
            "serve-create",
            "config",
            "actor",
            "reviewer",
            "--requests",
            "7,8",
            FLAG,
            "1",
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
            "p",
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
            "serve-create",
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
