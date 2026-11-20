//! Reserve absent live-source names from the same strict publication profile.
use super::*;

fn document(schema: &str) -> String {
    let captures = matches!(
        schema,
        "fa.supervised-witnesses/1" | "fa.supervised-witnesses/2" | "fa.supervised-witnesses/4"
    );
    let history = if matches!(
        schema,
        "fa.supervised-witnesses/5" | "fa.supervised-whole-input/2"
    ) {
        r#", "history":"exact_current_snapshot""#
    } else {
        ""
    };
    let waiting = if schema == "fa.supervised-witnesses/4" {
        r#", "max_retries":2"#
    } else {
        ""
    };
    let sources = if captures {
        r#", "original":"/declared/original.bin", "current":"/declared/current.bin""#
    } else {
        r#", "producer":{"path":"/declared/producer.bin","scope":{"tenant":1,"principal":2,"run":3,"branch":4,"authority":5}}"#
    };
    let feed = if schema == "fa.supervised-witnesses/1" {
        String::new()
    } else {
        let path = if captures {
            r#", "path":"/declared/feed.bin""#
        } else {
            ""
        };
        format!(
            r#", "feed":{{"source":41{path},"after":0,"clock":"unix_milliseconds","max_age_ms":5000,"lookup":{{"steps":10000,"bytes":1048576}}}}"#
        )
    };
    let requests = if schema.starts_with("fa.supervised-whole-input/") {
        ""
    } else {
        r#"{"kind":"absent_key","key":1}"#
    };
    format!(
        r#"{{"schema":"{schema}"{history}{waiting},"source":91{sources}{feed},"limits":{{"bindings":8,"steps":10000,"value_bytes":1048576}},"requests":[{requests}]}}"#
    )
}

fn joint(inner: &str) -> String {
    format!(
        r#"{{"schema":"fa.supervised-joint-publication/1","joint":{{"id":71,"generation":1,"minimum_safe_roots":1,"minimum_violation_roots":2,"maximum_escape_ppm":0,"maximum_false_stop_ppm":0,"max_cases":3,"max_member_outcomes":12}},"publication":{inner}}}"#
    )
}

#[test]
fn original_decode_retains_every_declared_source_path_through_joint_envelopes() {
    for schema in [
        "fa.supervised-witnesses/1",
        "fa.supervised-witnesses/2",
        "fa.supervised-witnesses/4",
        "fa.supervised-witnesses/3",
        "fa.supervised-witnesses/5",
        "fa.supervised-whole-input/1",
        "fa.supervised-whole-input/2",
    ] {
        for wrapped in [false, true] {
            let text = document(schema);
            let text = if wrapped { joint(&text) } else { text };
            let selected = PublicationProfile::decode(text.as_bytes()).unwrap();
            let expected: Vec<PathBuf> = match schema {
                "fa.supervised-witnesses/1" => {
                    vec!["/declared/original.bin", "/declared/current.bin"]
                }
                "fa.supervised-witnesses/2" | "fa.supervised-witnesses/4" => vec![
                    "/declared/original.bin",
                    "/declared/current.bin",
                    "/declared/feed.bin",
                ],
                _ => vec!["/declared/producer.bin"],
            }
            .into_iter()
            .map(PathBuf::from)
            .collect();
            assert_eq!(
                selected.input_paths(),
                expected.as_slice(),
                "{schema}, joint={wrapped}"
            );
            assert_eq!(selected.requires_joint(), wrapped);
        }
    }
}

#[test]
fn custody_projection_never_admits_paths_outside_the_original_closed_schema() {
    let captures = document("fa.supervised-witnesses/2");
    let producer = document("fa.supervised-witnesses/3");
    for invalid in [
        captures.replace("/declared/current.bin", "relative/current.bin"),
        captures.replace("/declared/feed.bin", "relative/feed.bin"),
        captures.replace(r#""path":"/declared/feed.bin""#, r#""path":42"#),
        producer.replace(
            r#""feed":{"source":41"#,
            r#""feed":{"path":"/undeclared/feed.bin","source":41"#,
        ),
        producer.replace(
            r#""producer":{"path"#,
            r#""producer":{"alternate":"/undeclared/producer.bin","path"#,
        ),
    ] {
        assert!(
            PublicationProfile::decode(invalid.as_bytes()).is_err(),
            "{invalid}"
        );
        assert!(PublicationProfile::decode(joint(&invalid).as_bytes()).is_err());
    }
    assert!(PublicationProfile::decode(captures.as_bytes()).is_ok());
    assert!(PublicationProfile::decode(producer.as_bytes()).is_ok());
}
