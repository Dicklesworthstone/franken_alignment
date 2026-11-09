//! Explicit paired recovery selection owns its entire grammar before any I/O.
use super::*;

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
