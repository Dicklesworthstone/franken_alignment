//! Paid work discovery must bind the client's independently selected limits.
use super::*;

fn original() -> (Settings, protocol::Request, Vec<u8>) {
    let (settings, request, response) =
        super::super::super::tests::numerical_tests::original_numerical_document();
    let request = protocol::decode(&request, settings.report_bytes).unwrap();
    (settings, request, response)
}

#[test]
fn numerical_inspection_binds_the_original_selected_and_remaining_allowances() {
    let (settings, request, response) = original();
    validate_selected_response(&request, &response, &settings).unwrap();
    let ValidatedResponse::Inspection(inspection) =
        validate_response(&request, &response, settings.report_bytes).unwrap()
    else {
        panic!("original numerical Get must return inspection evidence");
    };
    let observed = inspection.numerical_replay.unwrap();
    assert_eq!(Some(observed.selected), settings.numerical_replay);
    assert_eq!(observed.remaining_events, observed.selected.events);

    // An exhausted allowance is valid evidence, not a new grant or a promise
    // that a separately submitted native mutation will be admitted.
    let mut exhausted = parsed(&response);
    *field(
        &mut exhausted,
        &["knowledge", "value", "numerical_replay", "remaining_events"],
    ) = Json::String("0".into());
    validate_selected_response(&request, protocol::json(&exhausted).as_bytes(), &settings).unwrap();

    let mut ordinary = settings.clone();
    ordinary.numerical_replay = None;
    assert!(validate_selected_response(&request, &response, &ordinary).is_err());
    let mut missing = parsed(&response);
    let Json::Object(value) = field(&mut missing, &["knowledge", "value"]) else {
        panic!("inspection object");
    };
    value.remove("numerical_replay").unwrap();
    assert!(
        validate_selected_response(&request, protocol::json(&missing).as_bytes(), &settings)
            .is_err()
    );
}

#[test]
fn numerical_inspection_refuses_changed_limits_overdrawn_work_and_open_shapes() {
    let (settings, request, response) = original();
    let selected = settings.numerical_replay.unwrap();
    for (name, value) in [
        (
            "max_replay_events",
            Json::String((selected.max_replay_events - 1).to_string()),
        ),
        (
            "max_replay_events",
            Json::String((selected.max_replay_events + 1).to_string()),
        ),
        ("events", Json::String((selected.events - 1).to_string())),
        ("events", Json::String((selected.events + 1).to_string())),
        (
            "remaining_events",
            Json::String((selected.events + 1).to_string()),
        ),
        ("remaining_events", Json::String("00".into())),
        ("remaining_events", parsed(b"0")),
        ("remaining_events", Json::Null),
        ("events", Json::String("0".into())),
    ] {
        let mut changed = parsed(&response);
        *field(
            &mut changed,
            &["knowledge", "value", "numerical_replay", name],
        ) = value;
        assert!(
            validate_selected_response(&request, protocol::json(&changed).as_bytes(), &settings)
                .is_err(),
            "accepted changed numerical evidence: {name}"
        );
    }
    for name in ["max_replay_events", "events", "remaining_events"] {
        let mut changed = parsed(&response);
        let Json::Object(value) = field(&mut changed, &["knowledge", "value", "numerical_replay"])
        else {
            panic!("numerical object");
        };
        value.remove(name).unwrap();
        assert!(
            validate_selected_response(&request, protocol::json(&changed).as_bytes(), &settings)
                .is_err()
        );
    }
    for addition in ["refund", "refill", "effect_authority"] {
        let mut changed = parsed(&response);
        let Json::Object(value) = field(&mut changed, &["knowledge", "value", "numerical_replay"])
        else {
            panic!("numerical object");
        };
        value.insert(addition.into(), Json::Bool(true));
        assert!(
            validate_selected_response(&request, protocol::json(&changed).as_bytes(), &settings)
                .is_err()
        );
    }
}
