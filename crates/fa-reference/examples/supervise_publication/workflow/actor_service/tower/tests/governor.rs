use super::*;

fn selected() -> Vec<String> {
    [
        "serve-create",
        "config",
        "actor",
        "reviewer",
        FLAG,
        "41",
        "--requests",
        "7,8",
        "--operator-profile",
        "operator",
        "--governor-profile",
        "governor",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[test]
fn explicit_independent_governor_selects_only_a_new_plain_scheduled_tower_owner() {
    let args = selected();
    let parsed = Arguments::parse(&args).unwrap();
    assert_eq!(parsed.governor, Some(Path::new("governor")));
    assert_eq!(parsed.operator, Some(Path::new("operator")));
    assert_eq!(parsed.requests.as_deref(), Some([7, 8].as_slice()));
    assert_eq!(parsed.generation, 41);
    assert!(!parsed.open);
    let mut reordered = args[..4].to_vec();
    for pair in args[4..].chunks_exact(2).rev() {
        reordered.extend_from_slice(pair);
    }
    assert!(Arguments::parse(&reordered).is_ok());
}

#[test]
fn governor_selection_cannot_downgrade_recovery_topology_or_actor_only_grammars() {
    let args = selected();
    let mut invalid = Vec::new();
    for mode in [
        "serve-open",
        "serve-open-checked",
        "create-generated",
        "actor-submit",
    ] {
        let mut changed = args.clone();
        changed[0] = mode.into();
        if mode.ends_with("-checked") {
            changed.insert(4, "witness".into());
        }
        invalid.push(changed);
    }
    for flag in [
        "--topology-profile",
        "--precheck-profile",
        "--guarded",
        "--native-text",
    ] {
        let mut changed = args.clone();
        changed.extend([flag.into(), "selected".into()]);
        invalid.push(changed);
    }
    for missing in [FLAG, "--requests", "--operator-profile"] {
        let mut changed = args.clone();
        let at = changed.iter().position(|arg| arg == missing).unwrap();
        changed.drain(at..at + 2);
        invalid.push(changed);
    }
    let mut singleton = args.clone();
    singleton[7] = "7".into();
    invalid.push(singleton);
    let mut duplicate = args;
    duplicate.extend(["--governor-profile".into(), "another-governor".into()]);
    invalid.push(duplicate);
    for args in invalid {
        assert!(Arguments::parse(&args).is_err(), "{args:?}");
        let failure = command_with_clock(&args, None, || {
            panic!("invalid governor selection observed time")
        });
        assert!(failure.is_err());
    }
}

#[test]
fn checked_governor_requires_its_explicit_witness_profile_and_retains_closed_options() {
    let mut args = selected();
    args[0] = "serve-create-checked".into();
    args.insert(4, "witness".into());
    let selected = Arguments::parse(&args).unwrap();
    assert_eq!(selected.publication, Some(Path::new("witness")));
    assert_eq!(selected.governor, Some(Path::new("governor")));
    assert_eq!(selected.operator, Some(Path::new("operator")));
    assert!(!selected.open);
    let mut reordered = args[..5].to_vec();
    for pair in args[5..].chunks_exact(2).rev() {
        reordered.extend_from_slice(pair);
    }
    assert_eq!(
        Arguments::parse(&reordered).unwrap().publication,
        Some(Path::new("witness"))
    );
    for broken in 0..4 {
        let mut changed = args.clone();
        match broken {
            0 => {
                changed.remove(4);
            }
            1 => changed[4] = "--operator-profile".into(),
            2 => changed.extend(["--topology-profile".into(), "topology".into()]),
            _ => changed.extend(["--precheck-profile".into(), "precheck".into()]),
        }
        assert!(Arguments::parse(&changed).is_err(), "{changed:?}");
        assert!(
            command_with_clock(&changed, None, || panic!(
                "invalid checked grammar observed time"
            ))
            .is_err()
        );
    }
}
