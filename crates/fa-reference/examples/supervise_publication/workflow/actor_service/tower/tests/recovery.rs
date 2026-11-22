//! Explicit independent recovery selection owns its entire grammar before I/O.
use super::*;

#[test]
fn guarded_precheck_schedules_keep_complete_plain_or_mediated_governor_selection() {
    for checked in [false, true] {
        for governed in [false, true] {
            for mediated in [false, true] {
                let mut positional = vec![
                    if checked { "serve-open-checked" } else { "serve-open" },
                    "config", "actor", "reviewer",
                ];
                if checked { positional.push("witness"); }
                let mut pairs = vec![
                    [FLAG, "91"], ["--requests", "7,8"],
                    ["--guarded", "recovery"], ["--precheck-schedule", "schedule"],
                    ["--operator-profile", "operator"],
                ];
                if governed { pairs.push(["--governor-profile", "governor"]); }
                if mediated { pairs.push(["--topology-profile", "topology"]); }
                for first in 0..pairs.len() {
                    let mut args = positional.iter().map(|part| (*part).to_owned()).collect::<Vec<_>>();
                    for pair in pairs.iter().cycle().skip(first).take(pairs.len()) {
                        args.extend(pair.iter().map(|part| (*part).to_owned()));
                    }
                    let selected = Arguments::parse(&args).unwrap();
                    assert!(selected.open);
                    assert_eq!(selected.requests.as_deref(), Some([7, 8].as_slice()));
                    assert_eq!(selected.precheck_schedule, Some(Path::new("schedule")));
                    assert_eq!(selected.guarded, Some(Path::new("recovery")));
                    assert_eq!(selected.publication, checked.then(|| Path::new("witness")));
                    assert_eq!(selected.governor, governed.then(|| Path::new("governor")));
                    assert_eq!(selected.topology, mediated.then(|| Path::new("topology")));
                    for missing in ["--guarded", "--requests"] {
                        let mut invalid = args.clone();
                        let index = invalid.iter().position(|part| part == missing).unwrap();
                        invalid.drain(index..index + 2);
                        assert!(Arguments::parse(&invalid).is_err());
                    }
                    for extra in ["--precheck-schedule", "--precheck-profile"] {
                        let mut invalid = args.clone();
                        invalid.extend([extra.into(), "another".into()]);
                        let expected = Arguments::parse(&invalid).err().unwrap();
                        assert_eq!(command_with_clock(&invalid, None,
                            || panic!("invalid recovered schedule reached clock")).unwrap_err(), expected);
                    }
                }
            }
        }
    }
}

#[test]
fn plain_guarded_recovery_accepts_singletons_and_optional_operator_without_topology() {
    for checked in [false, true] {
        for operator in [false, true] {
            for scheduled in [false, true] {
                for requests in ["7", "7,8"] {
                    let mut positional = vec![
                        if checked { "serve-open-checked" } else { "serve-open" },
                        "config", "actor", "reviewer",
                    ];
                    if checked { positional.push("witness"); }
                    let mut pairs = vec![
                        [FLAG, "91"], ["--requests", requests], ["--guarded", "recovery"],
                    ];
                    if operator { pairs.push(["--operator-profile", "operator"]); }
                    if scheduled { pairs.push(["--precheck-schedule", "schedule"]); }
                    for first in 0..pairs.len() {
                        let mut args: Vec<_> = positional.iter().map(|part| (*part).to_owned()).collect();
                        for pair in pairs.iter().cycle().skip(first).take(pairs.len()) {
                            args.extend(pair.iter().map(|part| (*part).to_owned()));
                        }
                        let selected = Arguments::parse(&args).unwrap();
                        assert!(selected.open);
                        assert_eq!(selected.generation, 91);
                        assert_eq!(selected.requests, Some(if requests == "7" { vec![7] } else { vec![7, 8] }));
                        assert_eq!(selected.guarded, Some(Path::new("recovery")));
                        assert_eq!(selected.publication, checked.then(|| Path::new("witness")));
                        assert_eq!(selected.operator, operator.then(|| Path::new("operator")));
                        assert_eq!(selected.precheck_schedule, scheduled.then(|| Path::new("schedule")));
                        assert!(selected.governor.is_none());
                        assert!(selected.topology.is_none());
                        assert!(selected.precheck.is_none());
                    }
                }
            }
        }
    }
}

#[test]
fn plain_guarded_recovery_refuses_incomplete_or_conflicting_selection_before_reads() {
    for checked in [false, true] {
        for (create, suffix) in [
            (false, vec![FLAG, "91", "--guarded", "recovery"]),
            (false, vec![FLAG, "91", "--guarded", "recovery", "--precheck-profile", "precheck"]),
            (false, vec![FLAG, "91", "--guarded", "recovery", "--requests", "7", "--precheck-profile", "precheck"]),
            (false, vec![FLAG, "91", "--guarded", "recovery", "--requests", "7", "--guarded", "other"]),
            (false, vec![FLAG, "91", "--requests", "7", "--precheck-schedule", "schedule"]),
            (false, vec![FLAG, "91", "--guarded", "recovery", "--requests", "7", "--governor-profile", "governor"]),
            (true, vec![FLAG, "91", "--guarded", "recovery", "--requests", "7"]),
        ] {
            let mode = match (create, checked) {
                (false, false) => "serve-open",
                (false, true) => "serve-open-checked",
                (true, false) => "serve-create",
                (true, true) => "serve-create-checked",
            };
            let mut args: Vec<_> = [mode, "unread-config", "unread-actor", "unread-reviewer"]
                .into_iter().map(str::to_owned).collect();
            if checked { args.push("unread-witness".into()); }
            args.extend(suffix.into_iter().map(str::to_owned));
            let expected = Arguments::parse(&args).err().unwrap();
            assert_eq!(command_with_clock(&args, None,
                || panic!("invalid ordinary guarded recovery reached clock")).unwrap_err(), expected);
        }
    }
}

#[test]
fn guarded_topology_schedules_keep_all_five_options_in_both_open_modes() {
    for checked in [false, true] {
        let mut positional = vec![
            if checked {
                "serve-open-checked"
            } else {
                "serve-open"
            },
            "config",
            "actor",
            "reviewer",
        ];
        if checked {
            positional.push("witness");
        }
        let pairs = [
            [FLAG, "91"],
            ["--requests", "7,18446744073709551615"],
            ["--guarded", "recovery"],
            ["--topology-profile", "topology"],
            ["--operator-profile", "operator"],
        ];
        for first in 0..pairs.len() {
            let mut args: Vec<_> = positional.iter().map(|v| (*v).to_owned()).collect();
            for pair in pairs.iter().cycle().skip(first).take(pairs.len()) {
                args.extend(pair.iter().map(|v| (*v).to_owned()));
            }
            let selected = Arguments::parse(&args).unwrap();
            assert!(selected.open);
            assert_eq!(selected.requests, Some(vec![7, u64::MAX]));
            assert_eq!(selected.generation, 91);
            assert_eq!(selected.publication, checked.then(|| Path::new("witness")));
            assert_eq!(selected.guarded, Some(Path::new("recovery")));
            assert_eq!(selected.topology, Some(Path::new("topology")));
            assert_eq!(selected.operator, Some(Path::new("operator")));
            assert!(selected.precheck.is_none());

            args.extend(["--precheck-profile".to_owned(), "precheck".to_owned()]);
            let expected = Arguments::parse(&args).err().unwrap();
            assert_eq!(
                command_with_clock(&args, None, || panic!(
                    "unsupported recovery disclosure must refuse before clock or store"
                ))
                .unwrap_err(),
                expected
            );
        }
    }
}

#[test]
fn guarded_topology_recovery_accepts_both_open_modes_and_all_option_orders() {
    for checked in [false, true] {
        let positional = if checked {
            vec![
                "serve-open-checked",
                "config",
                "actor",
                "reviewer",
                "witness",
            ]
        } else {
            vec!["serve-open", "config", "actor", "reviewer"]
        };
        let pairs = [
            [FLAG, "91"],
            ["--guarded", "recovery"],
            ["--topology-profile", "topology"],
            ["--operator-profile", "operator"],
        ];
        for first in 0..4 {
            for second in 0..4 {
                for third in 0..4 {
                    for fourth in 0..4 {
                        let order = [first, second, third, fourth];
                        if order.iter().copied().collect::<BTreeSet<_>>().len() != 4 {
                            continue;
                        }
                        let mut args: Vec<_> =
                            positional.iter().map(|value| (*value).to_owned()).collect();
                        for index in order {
                            args.extend(pairs[index].map(str::to_owned));
                        }
                        let selected = Arguments::parse(&args).unwrap();
                        assert!(selected.open);
                        assert_eq!(selected.generation, 91);
                        assert_eq!(selected.publication, checked.then(|| Path::new("witness")));
                        assert_eq!(selected.guarded, Some(Path::new("recovery")));
                        assert_eq!(selected.topology, Some(Path::new("topology")));
                        assert_eq!(selected.operator, Some(Path::new("operator")));
                        assert!(selected.requests.is_none());
                        assert!(selected.precheck.is_none());
                    }
                }
            }
        }
        let mut args: Vec<_> = positional.iter().map(|value| (*value).to_owned()).collect();
        for pair in &pairs[..3] {
            args.extend(pair.map(str::to_owned));
        }
        assert!(Arguments::parse(&args).unwrap().operator.is_none());
    }
}

#[test]
fn guarded_topology_recovery_cannot_select_other_lifecycles_or_consume_missing_values() {
    for mode in [
        "serve-open",
        "serve-open-checked",
        "serve-create",
        "serve-create-checked",
    ] {
        let checked = mode.ends_with("-checked");
        let base = if checked {
            vec![mode, "config", "actor", "reviewer", "witness"]
        } else {
            vec![mode, "config", "actor", "reviewer"]
        };
        let invalid = [
            vec![FLAG, "1", "--guarded", "recovery"],
            vec![FLAG, "1", "--guarded", "recovery", "--topology-profile", ""],
            vec![
                FLAG,
                "1",
                "--guarded",
                "--topology-profile",
                "--topology-profile",
                "topology",
            ],
            vec![FLAG, "1", "--guarded", "recovery", "--guarded", "recovery"],
            vec![
                FLAG,
                "1",
                "--guarded",
                "recovery",
                "--topology-profile",
                "topology",
                "--precheck-profile",
                "precheck",
            ],
            vec![
                FLAG,
                "1",
                "--guarded",
                "recovery",
                "--topology-profile",
                "topology",
                "--native-text",
                "recipe",
            ],
            vec![
                FLAG,
                "1",
                "--guarded",
                "recovery",
                "--topology-profile",
                "topology",
                "--operator-profile",
            ],
            vec!["--guarded", "recovery", "--topology-profile", "topology"],
        ];
        for suffix in invalid {
            let args: Vec<_> = base
                .iter()
                .copied()
                .chain(suffix)
                .map(str::to_owned)
                .collect();
            let expected = Arguments::parse(&args).err().unwrap();
            assert_eq!(
                command_with_clock(&args, None, || panic!(
                    "invalid recovery grammar cannot read time"
                ))
                .unwrap_err(),
                expected
            );
        }
        if !mode.starts_with("serve-open") {
            let args: Vec<_> = base
                .iter()
                .copied()
                .chain([
                    FLAG,
                    "1",
                    "--guarded",
                    "recovery",
                    "--topology-profile",
                    "topology",
                ])
                .map(str::to_owned)
                .collect();
            let expected = Arguments::parse(&args).err().unwrap();
            assert_eq!(
                command_with_clock(&args, None, || panic!(
                    "create cannot consume guarded recovery"
                ))
                .unwrap_err(),
                expected
            );
        }
    }
}
