//! The selected mediated lifecycle cannot be dropped by another option parser.
use super::*;

fn positional(mode: &str) -> Vec<String> {
    let mut args: Vec<_> = [mode, "unread-config", "unread-actor", "unread-reviewer"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    if mode.ends_with("-checked") {
        args.push("unread-witness".into());
    }
    args
}

fn refused_before_reads(args: &[String]) {
    let expected = Arguments::parse(args)
        .err()
        .expect("invalid selection admitted");
    assert_eq!(
        command_with_clock(args, None, || panic!("refused grammar acquired time")).unwrap_err(),
        expected,
    );
    assert_eq!(
        super::super::super::command(args, None).unwrap_err(),
        expected
    );
    let mut output = Vec::new();
    let root_error = crate::command_with_output(args.to_vec(), &mut output).unwrap_err();
    if matches!(
        args.first().map(String::as_str),
        Some("resume" | "resume-checked")
    ) {
        // These are root recovery commands, not actor-service entry points.
        // Their closed arity rejects here without routing through Tower.
        assert!(root_error.starts_with("usage: supervise_publication"));
    } else {
        assert_eq!(root_error, expected);
    }
    assert!(output.is_empty());
}

#[test]
fn create_topology_and_optional_operator_are_selected_in_every_option_order() {
    for mode in ["serve-create", "serve-create-checked"] {
        for options in [
            [
                FLAG,
                "18446744073709551615",
                "--topology-profile",
                "topology.json",
            ],
            [
                "--topology-profile",
                "topology.json",
                FLAG,
                "18446744073709551615",
            ],
        ] {
            let mut args = positional(mode);
            args.extend(options.into_iter().map(str::to_owned));
            let selected = Arguments::parse(&args).unwrap();
            assert_eq!(selected.topology, Some(Path::new("topology.json")));
            assert_eq!(selected.generation, u64::MAX);
            assert!(!selected.open);
            assert_eq!(selected.publication.is_some(), mode.ends_with("-checked"));
            assert!(selected.requests.is_none());
            assert!(selected.precheck.is_none());
            assert!(selected.operator.is_none());
        }
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let pairs = [
                [FLAG, "11"],
                ["--topology-profile", "topology.json"],
                ["--operator-profile", "operator.json"],
            ];
            let mut args = positional(mode);
            for index in order {
                args.extend(pairs[index].into_iter().map(str::to_owned));
            }
            let selected = Arguments::parse(&args).unwrap();
            assert_eq!(selected.topology, Some(Path::new("topology.json")));
            assert_eq!(selected.operator, Some(Path::new("operator.json")));
            assert_eq!(selected.generation, 11);
            assert!(!selected.open);
            assert!(selected.requests.is_none());
            assert!(selected.precheck.is_none());
        }
    }
}

#[test]
fn topology_cannot_enter_open_recovery_scheduled_precheck_or_other_service_modes() {
    for mode in [
        "serve-open",
        "serve-open-checked",
        "actor-submit",
        "create-generated",
        "create-learned-generated",
        "serve-learned-generated",
        "resume",
        "resume-checked",
    ] {
        let mut args = positional(mode);
        args.extend(
            [FLAG, "11", "--topology-profile", "topology.json"]
                .into_iter()
                .map(str::to_owned),
        );
        refused_before_reads(&args);
    }
    for mode in ["serve-create", "serve-create-checked"] {
        for forbidden in [
            ["--requests", "7,8"],
            ["--precheck-profile", "precheck.json"],
            ["--peers", "peer.json"],
            ["--native-text", "native.json"],
            ["--guarded", "recovery.json"],
            ["--history-anchor", "checkpoint.json"],
            ["--retain-history-anchor", "checkpoint.json"],
            ["--reconcile-receipts", "receipts.json"],
        ] {
            for extra_first in [true, false] {
                let mut args = positional(mode);
                let topology = [FLAG, "11", "--topology-profile", "topology.json"];
                if extra_first {
                    args.extend(forbidden.into_iter().map(str::to_owned));
                    args.extend(topology.into_iter().map(str::to_owned));
                } else {
                    args.extend(topology.into_iter().map(str::to_owned));
                    args.extend(forbidden.into_iter().map(str::to_owned));
                }
                refused_before_reads(&args);
            }
        }
        let mut args = positional(mode);
        args.extend(
            [
                FLAG,
                "11",
                "--topology-profile",
                "topology.json",
                "--receipts-only",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        refused_before_reads(&args);
    }
}

#[test]
fn topology_paths_and_duplicates_cannot_be_consumed_as_another_option() {
    for suffix in [
        vec![FLAG, "11", "--topology-profile"],
        vec![FLAG, "11", "--topology-profile", ""],
        vec![FLAG, "11", "--topology-profile", "--operator-profile"],
        vec![
            FLAG,
            "11",
            "--topology-profile",
            "first",
            "--topology-profile",
            "second",
        ],
        vec!["--topology-profile", "first", FLAG, "11", FLAG, "12"],
        vec![
            "--topology-profile",
            "first",
            "--operator-profile",
            "operator",
        ],
        vec![
            FLAG,
            "11",
            "--topology-profile",
            "first",
            "--operator-profile",
            "",
        ],
        vec![
            FLAG,
            "11",
            "--topology-profile",
            "first",
            "--operator-profile",
            "--requests",
        ],
        vec![
            FLAG,
            "11",
            "--topology-profile",
            "first",
            "--operator-profile",
            "operator",
            "--requests",
            "7",
        ],
        vec![
            FLAG,
            "11",
            "--topology-profile",
            "first",
            "--operator-profile",
            "first",
            "--operator-profile",
            "second",
        ],
    ] {
        for mode in ["serve-create", "serve-create-checked"] {
            let mut args = positional(mode);
            args.extend(suffix.iter().map(|value| (*value).to_owned()));
            refused_before_reads(&args);
        }
    }
    for index in 1..=3 {
        let mut args = positional("serve-create");
        args[index] = "--topology-profile".into();
        args.extend(
            [FLAG, "11", "--topology-profile", "topology.json"]
                .into_iter()
                .map(str::to_owned),
        );
        refused_before_reads(&args);
    }
}

#[test]
fn topology_still_requires_explicit_nonzero_canonical_registration_before_reads() {
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
        let mut args = positional("serve-create");
        args.extend(
            ["--topology-profile", "topology.json", FLAG, generation]
                .into_iter()
                .map(str::to_owned),
        );
        refused_before_reads(&args);
    }
}

#[test]
fn create_topology_cannot_activate_credibility_from_an_existing_journal() {
    for mode in ["serve-create", "serve-create-checked"] {
        let mut args = positional(mode);
        args.extend(
            [FLAG, "11", "--topology-profile", "topology.json"]
                .into_iter()
                .map(str::to_owned),
        );
        assert!(Arguments::parse(&args).is_ok());
        let error = command_with_clock(&args, Some(Path::new("unread-capsule")), || {
            panic!("create cannot acquire time for credibility activation")
        })
        .unwrap_err();
        assert_eq!(
            error,
            "live credibility activation requires an existing journal"
        );
    }
}
