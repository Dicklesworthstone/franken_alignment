use super::*;

#[derive(Clone, Copy, Debug)]
enum Damage {
    Basis,
    Summary,
    Offset,
    Scope,
    Cursor,
    Duplicate,
    Malformed,
    Correlation,
    Completion,
}

fn damage(original: &str, first_cursor: u64, next_cursor: u64, damage: Damage) -> String {
    match damage {
        Damage::Basis => replace_once(original, "\"recorded_at\":\"1\"", "\"recorded_at\":\"2\""),
        Damage::Summary => replace_once(original, "\"suspended\":false", "\"suspended\":true"),
        Damage::Offset => replace_once(original, "\"offset\":\"16\"", "\"offset\":\"17\""),
        Damage::Scope => replace_once(original, "\"principal\":\"2\"", "\"principal\":\"9\""),
        Damage::Cursor => replace_once(
            original,
            &format!("\"continuation\":\"{next_cursor}\""),
            &format!("\"continuation\":\"{first_cursor}\""),
        ),
        Damage::Duplicate => {
            let changed = replace_once(original, "\"attempt\":\"17\"", "\"attempt\":\"1\"");
            replace_once(
                &changed,
                "\"address\":\"fa://1/attempt/17\"",
                "\"address\":\"fa://1/attempt/1\"",
            )
        }
        Damage::Malformed => replace_once(
            original,
            "\"offset\":\"16\"",
            "\"offset\":\"16\",\"offset\":\"16\"",
        ),
        Damage::Correlation => replace_once(
            original,
            "\"operation_id\":\"tower-situation-page-2\"",
            "\"operation_id\":\"tower-situation-page-1\"",
        ),
        Damage::Completion => replace_once(
            original,
            "\"enumeration_complete\":false",
            "\"enumeration_complete\":true",
        ),
    }
}

#[test]
fn damaged_second_page_is_never_emitted_or_followed_and_keeps_first_original_page() {
    for fault in [
        Damage::Basis,
        Damage::Summary,
        Damage::Offset,
        Damage::Scope,
        Damage::Cursor,
        Damage::Duplicate,
        Damage::Malformed,
        Damage::Correlation,
        Damage::Completion,
    ] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(33, 4);
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, None));
            let first = fixture.reply(&document);
            let cursor = native_page(&first).continuation().unwrap().id();
            let first = first.to_json().unwrap();
            send(&mut socket, &first);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(2, Some(cursor)));
            let second = fixture.reply(&document);
            let next_cursor = native_page(&second).continuation().unwrap().id();
            assert_ne!(cursor, next_cursor);
            let changed = damage(&second.to_json().unwrap(), cursor, next_cursor, fault);
            send(&mut socket, &changed);
            disconnected(&mut socket, &listener);
            assert_eq!(
                fixture.remaining(),
                2,
                "malformed reply triggered another native page"
            );
            first
        });
        let mut output = Vec::new();
        let error = command(&args(&profile, 4), &mut output).unwrap_err();
        assert!(error.contains("invalid"), "{fault:?}: {error}");
        assert_eq!(
            output,
            (server.join().unwrap() + "\n").as_bytes(),
            "{fault:?}"
        );
        assert_eq!(emitted(&output).len(), 1, "{fault:?}");
    }
}

#[test]
fn genuine_page_from_a_new_owner_cut_cannot_replace_the_original_capture() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 6);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(33, 6);
        let mut socket = bounded_socket(&listener);
        let first = fixture.reply(&read_document(&mut socket));
        let old_cursor = native_page(&first).continuation().unwrap().id();
        let first_basis = native_page(&first).basis();
        let first = first.to_json().unwrap();
        send(&mut socket, &first);
        fixture.advance();
        let document = read_document(&mut socket);
        assert_eq!(
            decode_request(&document).unwrap(),
            request(2, Some(old_cursor))
        );
        // The server deliberately substitutes a genuine fresh capture. Neither
        // its valid encoding nor the same registration permits mixing cuts.
        let fresh = fixture.reply(encode_request(&request(99, None)).unwrap().as_bytes());
        let fresh_cursor = native_page(&fresh).continuation().unwrap().id();
        let replacement = fixture.reply(
            encode_request(&request(2, Some(fresh_cursor)))
                .unwrap()
                .as_bytes(),
        );
        let page = native_page(&replacement);
        assert_eq!(page.offset(), 16);
        assert_ne!(page.basis(), first_basis);
        assert_eq!(page.summary().attempts, 34);
        send(&mut socket, &replacement.to_json().unwrap());
        disconnected(&mut socket, &listener);
        assert_eq!(fixture.remaining(), 3);
        first
    });
    let mut output = Vec::new();
    let error = command(&args(&profile, 6), &mut output).unwrap_err();
    assert!(error.contains("inconsistent native situation"));
    assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
}

fn at_mut<'a>(value: &'a mut Json, path: &[&str]) -> &'a mut Json {
    let mut selected = value;
    for field in path {
        let Json::Object(object) = selected else {
            panic!("not an object at {field}");
        };
        selected = object.get_mut(*field).unwrap();
    }
    selected
}

fn page_mut(value: &mut Json) -> &mut Json {
    at_mut(value, &["result", "value", "knowledge", "value"])
}

fn rows_mut(value: &mut Json) -> &mut Vec<Json> {
    let Json::Array(rows) = at_mut(page_mut(value), &["attempts"]) else {
        panic!("not rows");
    };
    rows
}

fn checked_first(fixture: &mut Fixture) -> Json {
    let request = request(1, None);
    let response = fixture.reply(encode_request(&request).unwrap().as_bytes());
    super::super::super::parse_response(response.to_json().unwrap().as_bytes(), &request).unwrap()
}

#[test]
fn every_scope_coordinate_and_live_origin_are_required_before_accepting_first_page() {
    let mut fixture = Fixture::new(17, 16);
    let original = checked_first(&mut fixture);
    let scope = native::profile().delivery.scope;
    assert!(matches!(
        Capture::new(scope, 4).accept(&original),
        Ok(Progress::Continue(_))
    ));
    for field in [
        "tenant",
        "principal",
        "run",
        "branch",
        "authority",
        "purpose",
    ] {
        let mut changed = original.clone();
        *at_mut(
            &mut changed,
            &["result", "value", "knowledge", "basis", "scope", field],
        ) = Json::String(
            if field == "purpose" {
                "experiment"
            } else {
                "99"
            }
            .into(),
        );
        assert!(Capture::new(scope, 4).accept(&changed).is_err(), "{field}");
    }
    for (field, value) in [
        ("origin", Json::String("canonical_image".into())),
        ("anchored_through", Json::String("0".into())),
        ("recorded_at", Json::String("01".into())),
    ] {
        let mut changed = original.clone();
        *at_mut(
            &mut changed,
            &["result", "value", "knowledge", "basis", field],
        ) = value;
        assert!(Capture::new(scope, 4).accept(&changed).is_err(), "{field}");
    }
}

#[test]
fn canonical_integer_strings_preserve_full_width_and_refuse_numeric_aliases_or_overflow() {
    let mut fixture = Fixture::new(17, 4);
    let original = checked_first(&mut fixture);
    let scope = native::profile().delivery.scope;
    let mut full_width = original.clone();
    *at_mut(
        &mut full_width,
        &["result", "value", "knowledge", "basis", "journal_revision"],
    ) = Json::String(u64::MAX.to_string());
    for name in ["total_units", "available_units"] {
        *at_mut(page_mut(&mut full_width), &["summary", name]) = Json::String(u64::MAX.to_string());
    }
    assert!(matches!(
        Capture::new(scope, 4).accept(&full_width),
        Ok(Progress::Continue(_))
    ));
    for spelling in [
        "",
        "00",
        "017",
        "+17",
        "-17",
        "17.0",
        "1e1",
        " 17",
        "18446744073709551616",
    ] {
        for path in [
            vec!["result", "value", "knowledge", "basis", "journal_revision"],
            vec!["result", "value", "knowledge", "value", "offset"],
            vec!["result", "value", "knowledge", "value", "continuation"],
            vec![
                "result",
                "value",
                "knowledge",
                "value",
                "summary",
                "attempts",
            ],
        ] {
            let mut changed = original.clone();
            *at_mut(&mut changed, &path) = Json::String(spelling.into());
            assert!(
                Capture::new(scope, 4).accept(&changed).is_err(),
                "{path:?}: {spelling}"
            );
        }
    }
    for field in [
        "attempt",
        "units",
        "payload_bytes",
        "proposal_policy_epoch",
        "deadline",
    ] {
        let mut changed = original.clone();
        *at_mut(&mut rows_mut(&mut changed)[0], &[field]) = parse("17");
        assert!(
            Capture::new(scope, 4).accept(&changed).is_err(),
            "numeric {field}"
        );
    }
    let mut zero_cursor = original.clone();
    *at_mut(page_mut(&mut zero_cursor), &["continuation"]) = Json::String("0".into());
    assert!(Capture::new(scope, 4).accept(&zero_cursor).is_err());
}

#[test]
fn page_shape_duplicates_offsets_and_accounting_cannot_manufacture_completion() {
    let mut fixture = Fixture::new(17, 4);
    let original = checked_first(&mut fixture);
    let scope = native::profile().delivery.scope;
    for count in [0, 15, 17] {
        let mut changed = original.clone();
        let rows = rows_mut(&mut changed);
        if count == 17 {
            rows.push(rows[0].clone());
        } else {
            rows.truncate(count);
        }
        assert!(
            Capture::new(scope, 4).accept(&changed).is_err(),
            "rows {count}"
        );
    }
    let mut duplicate = original.clone();
    let rows = rows_mut(&mut duplicate);
    rows[1] = rows[0].clone();
    assert!(Capture::new(scope, 4).accept(&duplicate).is_err());
    for offset in ["1", "18446744073709551615"] {
        let mut changed = original.clone();
        *at_mut(page_mut(&mut changed), &["offset"]) = Json::String(offset.into());
        assert!(Capture::new(scope, 4).accept(&changed).is_err());
    }
    for (name, value) in [
        ("attempts", "15"),
        ("total_units", "101"),
        ("pending_attempts", "16"),
        ("irrecoverably_unknown_attempts", "1"),
    ] {
        let mut changed = original.clone();
        *at_mut(page_mut(&mut changed), &["summary", name]) = Json::String(value.into());
        assert!(Capture::new(scope, 4).accept(&changed).is_err(), "{name}");
    }
    let mut single = Fixture::new(16, 1);
    let complete = checked_first(&mut single);
    assert!(matches!(
        Capture::new(scope, 1).accept(&complete),
        Ok(Progress::Complete)
    ));
    let mut unexpected_cursor = complete;
    *at_mut(page_mut(&mut unexpected_cursor), &["continuation"]) = Json::String("42".into());
    assert!(Capture::new(scope, 1).accept(&unexpected_cursor).is_err());
}

#[test]
fn closed_native_profile_never_accepts_relabelled_live_eligibility_or_extra_fields() {
    let mut fixture = Fixture::new(17, 4);
    let original = checked_first(&mut fixture);
    let scope = native::profile().delivery.scope;
    for (name, value) in [
        ("schema", Json::String("fa.publication-situation/2".into())),
        ("operation", Json::String("why-held".into())),
        ("historical", Json::Bool(false)),
        ("current_eligibility", Json::String("eligible".into())),
    ] {
        let mut changed = original.clone();
        *at_mut(&mut changed, &["result", "value", name]) = value;
        assert!(Capture::new(scope, 4).accept(&changed).is_err(), "{name}");
    }
    for path in [
        vec!["result", "value"],
        vec!["result", "value", "knowledge"],
        vec!["result", "value", "knowledge", "basis"],
        vec!["result", "value", "knowledge", "value"],
    ] {
        let mut changed = original.clone();
        let Json::Object(object) = at_mut(&mut changed, &path) else {
            unreachable!()
        };
        object.insert("permit".into(), Json::Bool(true));
        assert!(Capture::new(scope, 4).accept(&changed).is_err(), "{path:?}");
    }
}

#[test]
fn cli_checks_closed_grammar_and_canonical_registered_run_before_opening_profile() {
    let missing = Path::new("missing-situation-profile");
    let normal = args(missing, 4);
    for invalid in [
        Vec::new(),
        vec!["tower-situation".into()],
        normal[..4].to_vec(),
        [normal.clone(), vec!["extra".into()]].concat(),
    ] {
        assert_eq!(command(&invalid, &mut Vec::new()).unwrap_err(), USAGE);
    }
    for (index, value) in [
        (0, "tower-exchange"),
        (1, ""),
        (1, "--operator"),
        (3, "--pages"),
        (2, "fa://1/run/3"),
        (2, "fa://1/run/3@0"),
        (2, "fa://1/run/3@011"),
        (2, "fa://01/run/3@11"),
        (2, "fa://1/run/03@11"),
        (2, "fa://0/run/3@11"),
        (2, "fa://1/run/0@11"),
        (2, "fa://1/attempt/3@11"),
        (2, "run#3"),
        (4, "0"),
        (4, "65"),
        (4, "04"),
        (4, "+4"),
        (4, "4.0"),
        (4, "4e0"),
        (4, "18446744073709551616"),
    ] {
        let mut invalid = normal.clone();
        invalid[index] = value.into();
        let mut output = Vec::new();
        assert_eq!(
            command(&invalid, &mut output).unwrap_err(),
            USAGE,
            "{value}"
        );
        assert!(output.is_empty());
    }
    for limit in [1, 64] {
        assert_ne!(
            command(&args(missing, limit), &mut Vec::new()).unwrap_err(),
            USAGE
        );
    }
    // The executable must select this complete grammar before consuming shared
    // live-service options, including options that would open unrelated files.
    for extra in [
        vec!["--credibility-activation", "missing-evidence"],
        vec!["--retain-history-anchor", "unexpected-output"],
        vec!["--guarded", "missing-recovery"],
        vec!["--tower-generation", "11"],
        vec!["--operator-profile", "other-profile"],
    ] {
        let mut invalid = normal.clone();
        invalid.extend(extra.into_iter().map(str::to_owned));
        let mut output = Vec::new();
        assert_eq!(
            crate::command_with_output(invalid, &mut output).unwrap_err(),
            USAGE
        );
        assert!(output.is_empty());
    }
}

#[test]
fn tenant_and_run_are_bound_to_operator_profile_before_connecting() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 4);
    let listener = UnixListener::bind(root.socket()).unwrap();
    listener.set_nonblocking(true).unwrap();
    for address in ["fa://2/run/3@11", "fa://1/run/4@11"] {
        let mut query = args(&profile, 4);
        query[2] = address.into();
        let mut output = Vec::new();
        let error = command(&query, &mut output).unwrap_err();
        assert!(error.contains("does not match the operator profile scope"));
        assert!(output.is_empty());
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
}

#[test]
fn connected_supervisor_identity_is_checked_before_any_generated_request() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid().checked_add(1).unwrap(), 4);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let mut output = Vec::new();
    assert!(command(&args(&profile, 4), &mut output).is_err());
    assert!(output.is_empty());
    let mut socket = bounded_socket(&listener);
    disconnected(&mut socket, &listener);
}

#[test]
fn stale_registration_is_a_refusal_and_never_spends_native_inspection() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 4);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(17, 4);
        let mut socket = bounded_socket(&listener);
        let document = read_document(&mut socket);
        assert_eq!(
            decode_request(&document).unwrap().address.generation,
            Some(GENERATION + 1)
        );
        let response = fixture.reply(&document);
        assert!(matches!(&response.result, PublicationResult::Refused(_)));
        let original = response.to_json().unwrap();
        send(&mut socket, &original);
        disconnected(&mut socket, &listener);
        assert_eq!(fixture.remaining(), 4);
        original
    });
    let mut query = args(&profile, 4);
    query[2] = format!("fa://1/run/3@{}", GENERATION + 1);
    let mut output = Vec::new();
    assert!(
        command(&query, &mut output)
            .unwrap_err()
            .contains("refused")
    );
    assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
}
