//! Actual recovered-command admission before numerical assets, clock or mutation.
//! The paired admitted suffix reaches a deliberately inadmissible asset; it does
//! not claim successful replay or publication under the selected policy.
use super::*;
use super::super::super::super::anchors;

fn policy_json(maximum: usize) -> String {
    format!(r#"{{"generation":2,"nodes":[{{"op":"exact_value","key":7,"value_hex":"6f6b"}},{{"op":"payload_at_most","maximum":{maximum}}},{{"op":"units_at_most","maximum":512}},{{"op":"all","children":[0,1,2]}}]}}"#)
}

fn wrapped(original: &str, maximum: usize, cases: usize) -> String {
    format!(r#"{{"schema":"fa.learned-recovery-policy/2","effective_policy":{},"campaigns":{{"cases":{cases},"input_bytes":1048576,"max_campaigns":4}},"recipe":{original}}}"#,
        policy_json(maximum))
}

fn remove_option(args: &mut Vec<String>, flag: &str) {
    let at = args.iter().position(|value| value == flag).unwrap();
    assert_eq!(args.iter().filter(|value| value.as_str() == flag).count(), 1);
    drop(args.drain(at..at + 2));
}

#[test]
fn recovered_command_admits_exact_message_finish_suffix_and_rejects_missing_or_conflicting_custody_before_assets() {
    let root = Root::new();
    let (config, paths, mut entries) = selected(&root, None, None);
    let (governor, _, _) = selections(&root, &entries[0].actor,
        std::process::id().checked_add(10_000).unwrap());
    let campaigns = live_governor::Settings::read(&governor).unwrap().campaign_requirement();

    // Retain a genuine independently selected anchor before taking assets away.
    // This is setup, never evidence that the later policy expectation replayed.
    entries[0].loaded.expected_campaigns = Some(campaigns);
    entries[0].loaded.creation_campaigns = Some(campaigns);
    let ((host, _human, _prediction, _identity), _governor) =
        recovery::create_governed(&config, &entries[0].loaded).unwrap();
    let anchor = root.0.join("admission-input.anchor");
    anchors::export_acknowledged(&host.history_anchor().unwrap(), &anchor).unwrap();
    drop(host);
    let canonical = fs::read(config.store.join("delivery.bin")).unwrap();
    let anchor_bytes = fs::read(&anchor).unwrap();
    let staged = config.store.join("delivery.pending");
    fs::write(&staged, b"preserve pre-admission staging").unwrap();

    // Select the real resident suffix, not a manufactured alternate manifest.
    let mut args = inputs(&root, &config, &paths[1..], &entries[1..], None);
    remove_option(&mut args, "--campaign-profile");
    args.extend(["--after-message".into(), "1".into(), "--history-anchor".into(),
        anchor.display().to_string()]);
    let originals = paths[1..].iter().map(|path| fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>();
    let documents = originals.iter().map(|original| wrapped(original, 256, 16))
        .collect::<Vec<_>>();
    for (path, document) in paths[1..].iter().zip(&documents) {
        fs::write(path, document).unwrap();
    }

    // The first asset is intentionally a directory. Its exact original loader
    // diagnostic proves the complete command passed admission before the clock.
    let blocked_asset = root.0.join("fit-binding.json");
    fs::remove_file(&blocked_asset).unwrap();
    fs::create_dir(&blocked_asset).unwrap();
    let admitted_error = resident::command_with_clock(&args, None,
        || panic!("asset refusal must precede the invocation clock")).unwrap_err();
    assert_eq!(admitted_error, format!("not an admitted regular file: {blocked_asset:?}"));

    for case in 0..5 {
        let mut selected_args = args.clone();
        for (path, document) in paths[1..].iter().zip(&documents) {
            fs::write(path, document).unwrap();
        }
        let expected = match case {
            0 => {
                fs::write(&paths[1], &originals[0]).unwrap();
                "explicit full effective policy"
            }
            1 => {
                fs::write(&paths[1], format!(r#"{{"schema":"fa.learned-recovery-policy/1","effective_policy":{},"recipe":{}}}"#,
                    policy_json(256), originals[0])).unwrap();
                "exact original campaign limits"
            }
            2 => {
                remove_option(&mut selected_args, "--history-anchor");
                "recovery with --history-anchor"
            }
            3 => {
                // Same generation and campaign bounds; only a later predicate changes.
                fs::write(&paths[2], wrapped(&originals[1], 257, 16)).unwrap();
                "same complete effective policy"
            }
            4 => {
                fs::write(&paths[2], wrapped(&originals[1], 256, 17)).unwrap();
                "exact original campaign limits"
            }
            _ => unreachable!(),
        };
        let refused = resident::command_with_clock(&selected_args, None,
            || panic!("incomplete governed admission reached the clock")).unwrap_err();
        assert!(refused.contains(expected), "case {case}: {refused}");
        assert_ne!(refused, admitted_error, "case {case} reached model assets");
        assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
        assert_eq!(fs::read(&staged).unwrap(), b"preserve pre-admission staging");
        assert_eq!(fs::read(&anchor).unwrap(), anchor_bytes);
        assert!(!root.0.join("final.anchor").exists());
        quiescent(&root);
    }
    for (path, document) in paths[1..].iter().zip(&documents) {
        fs::write(path, document).unwrap();
    }
    let admitted_again = resident::command_with_clock(&args, None,
        || panic!("the restored suffix must still stop at its original asset")).unwrap_err();
    assert_eq!(admitted_again, admitted_error);
    assert_eq!(fs::read(config.store.join("delivery.bin")).unwrap(), canonical);
    assert_eq!(fs::read(&staged).unwrap(), b"preserve pre-admission staging");
    assert_eq!(fs::read(&anchor).unwrap(), anchor_bytes);
    assert!(!root.0.join("final.anchor").exists());
    quiescent(&root);
}
