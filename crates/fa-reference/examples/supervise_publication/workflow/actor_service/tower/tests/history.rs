use super::*;

fn arguments(checked: bool) -> Vec<String> {
    let mut args: Vec<_> = [
        if checked {
            "serve-open-checked"
        } else {
            "serve-open"
        },
        "unread-config",
        "unread-actor",
        "unread-reviewer",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if checked {
        args.push("unread-witness".into());
    }
    args.extend(
        [
            FLAG,
            "41",
            "--requests",
            "7,8",
            "--andon",
            "unread-accounting",
            "--evaluator-profile",
            "unread-evaluator",
            "--guarded",
            "unread-recovery",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    args
}

#[test]
fn historical_evaluation_is_a_unary_mode_at_every_complete_option_boundary() {
    for checked in [false, true] {
        let ordinary = arguments(checked);
        assert!(!Arguments::parse(&ordinary).unwrap().evaluate_only);
        let positional = if checked { 5 } else { 4 };
        for index in (positional..=ordinary.len()).step_by(2) {
            let mut args = ordinary.clone();
            args.insert(index, "--evaluate-only".into());
            let selected = Arguments::parse(&args).unwrap();
            assert!(selected.open);
            assert!(selected.evaluate_only);
            assert_eq!(selected.generation, 41);
            assert_eq!(selected.requests, Some(vec![7, 8]));
            assert_eq!(selected.andon, Some(Path::new("unread-accounting")));
            assert_eq!(selected.evaluator, Some(Path::new("unread-evaluator")));
            assert_eq!(selected.guarded, Some(Path::new("unread-recovery")));
            assert_eq!(selected.publication.is_some(), checked);
            assert!(selected.operator.is_none());
            assert!(selected.precheck_schedule.is_none());
            assert!(selected.retain_history.is_none());
        }
    }
}

fn refused_before_inputs(args: Vec<String>) {
    let expected = Arguments::parse(&args)
        .err()
        .expect("unsupported selection");
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

#[test]
fn historical_evaluation_rejects_missing_and_conflicting_custody_before_reads() {
    for checked in [false, true] {
        let mut selected = arguments(checked);
        selected.push("--evaluate-only".into());
        for option in [
            FLAG,
            "--requests",
            "--andon",
            "--evaluator-profile",
            "--guarded",
        ] {
            let mut args = selected.clone();
            let index = args.iter().position(|value| value == option).unwrap();
            args.drain(index..index + 2);
            refused_before_inputs(args);
        }
        for (option, value) in [
            ("--operator-profile", "unread-operator"),
            ("--precheck-profile", "unread-precheck"),
            ("--precheck-schedule", "unread-schedule"),
            ("--topology-profile", "unread-topology"),
            ("--governor-profile", "unread-governor"),
            ("--retain-history-anchor", "new-output"),
        ] {
            let mut args = selected.clone();
            args.extend([option.into(), value.into()]);
            refused_before_inputs(args);
        }
        for extra in [
            vec!["--evaluate-only"],
            vec!["false"],
            vec!["--evaluator-profile"],
            vec!["--requests"],
            vec!["--unknown-option", "value"],
            vec!["--receipts-only"],
            vec!["--native-text", "unread-recipe"],
        ] {
            let mut args = selected.clone();
            args.extend(extra.into_iter().map(str::to_owned));
            // Native-text owns its distinct complete grammar in the outer
            // dispatcher; this ordinary parser still cannot consume it.
            if args.iter().any(|arg| arg == "--native-text") {
                assert!(Arguments::parse(&args).is_err());
            } else {
                refused_before_inputs(args);
            }
        }
        let mut create = selected.clone();
        create[0] = if checked {
            "serve-create-checked"
        } else {
            "serve-create"
        }
        .into();
        refused_before_inputs(create);

        // A unary flag cannot replace a paired option's required value.
        let mut misplaced = arguments(checked);
        let index = misplaced.iter().position(|arg| arg == "--andon").unwrap();
        misplaced[index + 1] = "--evaluate-only".into();
        refused_before_inputs(misplaced);

        let mut explicit_value = arguments(checked);
        explicit_value.extend(["--evaluate-only".into(), "true".into()]);
        refused_before_inputs(explicit_value);

        let mut output = Vec::new();
        selected.extend(["--credibility-activation".into(), "unread-capsule".into()]);
        assert!(
            crate::command_with_output(selected, &mut output)
                .unwrap_err()
                .contains("does not select a credibility activation capsule")
        );
        assert!(output.is_empty());
    }
}
