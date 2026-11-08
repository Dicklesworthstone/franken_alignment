use super::*;

#[test]
fn separate_operator_profile_composes_with_single_request_precheck_in_every_order() {
    for mode in [
        "serve-create",
        "serve-open",
        "serve-create-checked",
        "serve-open-checked",
    ] {
        let mut positional: Vec<_> = [mode, "config", "actor", "reviewer"]
            .into_iter()
            .map(str::to_owned)
            .collect();
        if mode.ends_with("-checked") {
            positional.push("witness".into());
        }
        for ordered in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let pairs = [
                [FLAG, "41"],
                ["--precheck-profile", "precheck.json"],
                ["--operator-profile", "operator.json"],
            ];
            let mut args = positional.clone();
            for index in ordered {
                args.extend(pairs[index].into_iter().map(str::to_owned));
            }
            let selected = Arguments::parse(&args).unwrap();
            assert_eq!(selected.operator, Some(Path::new("operator.json")));
            assert_eq!(selected.precheck, Some(Path::new("precheck.json")));
            assert!(selected.requests.is_none());
            assert_eq!(selected.generation, 41);
            assert_eq!(selected.open, mode.starts_with("serve-open"));
            assert_eq!(selected.publication.is_some(), mode.ends_with("-checked"));
        }
        for ordered in [
            [FLAG, "41", "--operator-profile", "operator.json"],
            ["--operator-profile", "operator.json", FLAG, "41"],
        ] {
            let mut args = positional.clone();
            args.extend(ordered.into_iter().map(str::to_owned));
            let selected = Arguments::parse(&args).unwrap();
            assert_eq!(selected.operator, Some(Path::new("operator.json")));
            assert!(selected.precheck.is_none());
        }
    }
}

#[test]
fn operator_selection_owns_its_entire_grammar_before_configuration_reads() {
    for suffix in [
        vec!["--operator-profile", "operator.json"],
        vec![FLAG, "41", "--operator-profile"],
        vec![FLAG, "41", "--operator-profile", ""],
        vec![FLAG, "41", "--operator-profile", "--requests"],
        vec![
            FLAG,
            "41",
            "--operator-profile",
            "first",
            "--operator-profile",
            "second",
        ],
        vec![
            FLAG,
            "41",
            "--operator-profile",
            "operator",
            "--requests",
            "7,8",
        ],
        vec![
            FLAG,
            "41",
            "--operator-profile",
            "operator",
            "--precheck-profile",
            "precheck",
            "--requests",
            "7",
        ],
        vec![
            FLAG,
            "41",
            "--operator-profile",
            "operator",
            "--peers",
            "other",
        ],
        vec![
            FLAG,
            "41",
            "--operator-profile",
            "operator",
            "--topology-profile",
            "topology",
        ],
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
        assert_eq!(
            super::super::super::command(&args, None).unwrap_err(),
            expected
        );
        let mut output = Vec::new();
        assert_eq!(
            crate::command_with_output(args, &mut output).unwrap_err(),
            expected
        );
        assert!(output.is_empty());
    }
    for mode in [
        "actor-submit",
        "create-generated",
        "create-learned-generated",
        "serve-learned-generated",
    ] {
        let args: Vec<_> = [
            mode,
            "unread-config",
            "unread-profile",
            "unread-reviewer",
            "--operator-profile",
            "operator",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let expected = Arguments::parse(&args).err().unwrap();
        assert_eq!(
            super::super::super::command(&args, None).unwrap_err(),
            expected
        );
    }
}
