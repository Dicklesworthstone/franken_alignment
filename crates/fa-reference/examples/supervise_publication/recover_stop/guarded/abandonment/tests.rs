//! Closed operator input; runtime controls reuse the original native fixtures.
use super::*;

// Test-only bridge for descendants of recover_stop::tests. Production keeps
// its profile and elapsed seam private; no caller-supplied recovery path exists.
pub(in crate::recover_stop) fn run(
    config: &Config,
    profile: &[u8],
    request: FileAbandonRequest,
    elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write,
) -> Result<(), String> {
    let profile = AbandonProfile::decode(profile)?;
    run_with_elapsed(config, &profile, request, elapsed, out)
}

fn request_document() -> String {
    r#"{"schema_version":"fa.abandon-unknown/1","operation":71,"attempt":1,"expected_control_sequence":0,"expected_authority_epoch":0}"#.into()
}

#[test]
fn abandonment_request_is_closed_bounded_and_keeps_the_exact_original_predecessor() {
    let good = request_document();
    assert_eq!(
        decode_request(good.as_bytes()).unwrap(),
        FileAbandonRequest {
            operation: 71,
            attempt: 1,
            expected_control_sequence: 0,
            expected_authority_epoch: 0,
        }
    );
    for field in [
        "schema_version",
        "operation",
        "attempt",
        "expected_control_sequence",
        "expected_authority_epoch",
    ] {
        assert!(
            decode_request(
                good.replace(&format!("\"{field}\""), "\"unexpected\"")
                    .as_bytes()
            )
            .is_err()
        );
    }
    for bad in [
        good.replace("fa.abandon-unknown/1", "fa.guarded-stop/1"),
        good.replace("\"operation\":71", "\"operation\":0"),
        good.replace("\"attempt\":1", "\"attempt\":0"),
        good.replace("\"operation\":71", "\"operation\":\"71\""),
        good.replace(
            "\"expected_control_sequence\":0",
            "\"expected_control_sequence\":-1",
        ),
        good.replace(
            "\"expected_authority_epoch\":0",
            "\"expected_authority_epoch\":1.5",
        ),
        good.replace("\"operation\":71", "\"operation\":18446744073709551616"),
        good.replacen('{', "{\"operation\":71,", 1),
        good.replacen('{', "{\"effect_outcome\":\"not_executed\",", 1),
        "[]".into(),
        "null".into(),
    ] {
        assert!(decode_request(bad.as_bytes()).is_err(), "{bad}");
    }
    let mut oversized = good.into_bytes();
    oversized.resize(REQUEST_BYTES + 1, b' ');
    assert!(decode_request(&oversized).is_err());
}

#[test]
fn abandonment_profile_requires_its_own_schema_complete_floor_and_explicit_output_custody() {
    let good = r#"{"schema_version":"fa.guarded-abandon/1","minimum":{"journal_revision":1,"control_sequence":0,"authority_epoch":0},"history_anchor":"/independent/input.anchor","retain_history_anchor":"/independent/output.anchor"}"#;
    assert!(AbandonProfile::decode(good.as_bytes()).is_ok());
    for field in [
        "journal_revision",
        "control_sequence",
        "authority_epoch",
        "history_anchor",
        "retain_history_anchor",
    ] {
        assert!(
            AbandonProfile::decode(
                good.replace(&format!("\"{field}\""), "\"unexpected\"")
                    .as_bytes()
            )
            .is_err()
        );
    }
    for bad in [
        good.replace("fa.guarded-abandon/1", "fa.guarded-stop/1"),
        good.replace("fa.guarded-abandon/1", "fa.guarded-resume/1"),
        good.replace("fa.guarded-abandon/1", "fa.guarded-abandon/2"),
        good.replace(
            ",\"retain_history_anchor\":\"/independent/output.anchor\"",
            "",
        ),
        good.replace("/independent/input.anchor", "relative.anchor"),
        good.replace("/independent/output.anchor", "relative-output.anchor"),
        good.replace("\"journal_revision\":1", "\"journal_revision\":-1"),
        good.replacen('{', "{\"permitting\":true,", 1),
        good.replacen('{', "{\"schema_version\":\"fa.guarded-abandon/1\",", 1),
    ] {
        assert!(AbandonProfile::decode(bad.as_bytes()).is_err(), "{bad}");
    }
    let mut oversized = good.as_bytes().to_vec();
    oversized.resize(16 * 1024 + 1, b' ');
    assert!(AbandonProfile::decode(&oversized).is_err());
}

#[test]
fn mediated_abandonment_profile_keeps_every_independent_topology_requirement() {
    let good = r#"{"schema_version":"fa.guarded-abandon/2","minimum":{"journal_revision":1,"control_sequence":0,"authority_epoch":0},"history_anchor":"/independent/input.anchor","retain_history_anchor":"/independent/output.anchor","topology":{"initial_topology":"/independent/initial.json","initial_inventory":"/independent/initial-inventory.json","current_topology":"/independent/current.json","current_inventory":"/independent/current-inventory.json","available":false}}"#;
    assert!(AbandonProfile::decode(good.as_bytes()).is_ok());
    assert!(
        AbandonProfile::decode(
            good.replace("\"available\":false", "\"available\":true")
                .as_bytes()
        )
        .is_ok()
    );
    for field in [
        "initial_topology",
        "initial_inventory",
        "current_topology",
        "current_inventory",
        "available",
    ] {
        assert!(
            AbandonProfile::decode(
                good.replace(&format!("\"{field}\""), "\"unexpected\"")
                    .as_bytes()
            )
            .is_err(),
            "missing {field}"
        );
    }
    for bad in [
        good.replace("fa.guarded-abandon/2", "fa.guarded-abandon/1"),
        good.replace("fa.guarded-abandon/2", "fa.guarded-stop/2"),
        good.replace("fa.guarded-abandon/2", "fa.guarded-resume/2"),
        good.replace("fa.guarded-abandon/2", "fa.guarded-submit/2"),
        good.replace("/independent/current.json", "relative-current.json"),
        good.replace("\"available\":false", "\"available\":\"false\""),
        good.replace(
            "\"available\":false",
            "\"available\":false,\"available\":false",
        ),
        good.replace(
            "\"available\":false",
            "\"available\":false,\"allow_downgrade\":true",
        ),
    ] {
        assert!(AbandonProfile::decode(bad.as_bytes()).is_err(), "{bad}");
    }
    // A separate command must not accidentally accept a disposition profile.
    assert!(super::super::profile::Profile::decode(good.as_bytes()).is_err());
    assert!(super::super::profile::ResumeProfile::decode(good.as_bytes()).is_err());
    assert!(super::super::profile::SubmitProfile::decode(good.as_bytes()).is_err());
}

#[test]
fn command_requires_the_explicit_guarded_selection_before_any_file_reads() {
    for args in [
        vec!["abandon-unknown".into()],
        vec![
            "abandon-unknown".into(),
            "missing-config".into(),
            "missing-request".into(),
        ],
        vec![
            "abandon-unknown".into(),
            "missing-config".into(),
            "missing-request".into(),
            "--unguarded".into(),
            "missing-profile".into(),
        ],
        vec![
            "abandon-unknown".into(),
            "missing-config".into(),
            "missing-request".into(),
            "--guarded".into(),
            "missing-profile".into(),
            "--credibility-activation".into(),
            "missing-evidence".into(),
        ],
        vec![
            "abandon-unknown".into(),
            "missing-config".into(),
            "missing-request".into(),
            "--guarded".into(),
            "missing-profile".into(),
            "--reviewer-profile".into(),
            "missing-reviewer".into(),
        ],
        vec![
            "recover-stop".into(),
            "missing-config".into(),
            "missing-request".into(),
            "--guarded".into(),
            "missing-profile".into(),
        ],
    ] {
        let mut output = Vec::new();
        assert_eq!(command(&args, &mut output).unwrap_err(), USAGE);
        assert!(output.is_empty());
        if args[0] == "abandon-unknown" {
            assert_eq!(crate::command(args).unwrap_err(), USAGE);
        }
    }
}
