//! A specialised Andon reader cannot ignore ordinary campaign expectations.
use super::*;

#[test]
fn andon_refuses_campaign_profile_before_clock_source_or_store_admission() {
    let config = Config::decode(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/fixtures/supervised_publication.json"
    )))
    .unwrap();
    let settings = Settings::decode(br#"{
      "schema_version":"fa.native-andon-operator/1",
      "evaluation":{"domain":1,"stratum":2,"period":3,"minimum_violation_origins":1,"minimum_benign_origins":1,
        "precision_floor":{"numerator":1,"denominator":2},"recall_floor":{"numerator":1,"denominator":2},
        "false_positive_ceiling":{"numerator":1,"denominator":2},"false_stop_budget":10},
      "accounting":{"deployment":101,"evaluator":202,"initial_windows":[
        {"window":1,"start":0,"end":18446744073709551615,"false_stops_allowed":2,"cost_per_stop":7}]},
      "current_windows":[{"window":1,"start":0,"end":18446744073709551615,"false_stops_allowed":2,"cost_per_stop":7}],
      "minimum_evaluation_revision":0
    }"#).unwrap();
    let profile = SubmitProfile::decode(
        br#"{
      "schema_version":"fa.guarded-submit/2",
      "minimum":{"journal_revision":4,"control_sequence":1,"authority_epoch":1},
      "history_anchor":"/operator/independent.anchor",
      "effective_policy":{"generation":2,"nodes":[{"op":"payload_at_most","maximum":64}]},
      "campaigns":{"cases":16,"input_bytes":1048576,"max_campaigns":8}
    }"#,
    )
    .unwrap();
    // An unsupported expectation must be rejected even before actor-document
    // parsing, rather than being lost while assembling specialised requirements.
    let error = run(
        config,
        Submission {
            document: b"not an actor document",
            settings: &settings,
            mode: Mode::Existing(&profile),
            peers: None,
            publication: None,
        },
        Instant::now(),
        || panic!("unsupported policy selected a clock"),
        &mut || panic!("unsupported policy selected recovery"),
    )
    .err()
    .unwrap();
    assert!(
        error.contains("does not support fa.guarded-submit/2"),
        "{error}"
    );
}
