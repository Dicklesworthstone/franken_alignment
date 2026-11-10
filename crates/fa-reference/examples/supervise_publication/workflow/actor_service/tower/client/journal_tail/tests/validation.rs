use super::*;

#[derive(Clone, Copy, Debug)]
enum Damage {
    Basis,
    Offset,
    After,
    Scope,
    Cursor,
    Revision,
    Through,
    EventKind,
    PrivateField,
    Registration,
    Query,
    Completion,
    DuplicateKey,
    Correlation,
}

fn damage(original: &str, first_cursor: u64, next_cursor: u64, fault: Damage) -> String {
    match fault {
        Damage::Basis => replace_once(original, "\"recorded_at\":\"6\"", "\"recorded_at\":\"7\""),
        Damage::Offset => replace_once(original, "\"offset\":\"2\"", "\"offset\":\"3\""),
        Damage::After => replace_once(
            original,
            "\"after_revision\":\"1\"",
            "\"after_revision\":\"2\"",
        ),
        Damage::Scope => replace_once(original, "\"principal\":\"2\"", "\"principal\":\"9\""),
        Damage::Cursor => replace_once(
            original,
            &format!("\"continuation\":\"{next_cursor}\""),
            &format!("\"continuation\":\"{first_cursor}\""),
        ),
        Damage::Revision => replace_once(original, "\"revision\":\"4\"", "\"revision\":\"3\""),
        Damage::Through => replace_once(
            original,
            "\"through_revision\":\"5\"",
            "\"through_revision\":\"6\"",
        ),
        Damage::EventKind => {
            original.replacen("\"kind\":\"time\"", "\"kind\":\"effect_succeeded\"", 1)
        }
        Damage::PrivateField => original.replacen(
            "\"kind\":\"time\"",
            "\"kind\":\"time\",\"payload\":\"private\"",
            1,
        ),
        Damage::Registration => replace_once(
            original,
            &format!("\"registration\":\"{}\"", mount().canonical_string()),
            "\"registration\":\"fa://1/run/3?generation=12\"",
        ),
        Damage::Query => replace_once(
            original,
            &format!("\"query_address\":\"{}\"", mount().canonical_string()),
            "\"query_address\":\"fa://1/run/3?generation=12\"",
        ),
        Damage::Completion => replace_once(
            original,
            "\"enumeration_complete\":false",
            "\"enumeration_complete\":true",
        ),
        Damage::DuplicateKey => replace_once(
            original,
            "\"offset\":\"2\"",
            "\"offset\":\"2\",\"offset\":\"2\"",
        ),
        Damage::Correlation => replace_once(
            original,
            "\"operation_id\":\"tower-tail-page-2\"",
            "\"operation_id\":\"tower-tail-page-1\"",
        ),
    }
}

#[test]
fn damaged_second_page_is_not_emitted_or_followed_and_keeps_first_original_page() {
    for fault in [
        Damage::Basis,
        Damage::Offset,
        Damage::After,
        Damage::Scope,
        Damage::Cursor,
        Damage::Revision,
        Damage::Through,
        Damage::EventKind,
        Damage::PrivateField,
        Damage::Registration,
        Damage::Query,
        Damage::Completion,
        Damage::DuplicateKey,
        Damage::Correlation,
    ] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4, 4, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(6, 4, 2);
            let mut socket = bounded_socket(&listener);
            let first = fixture.reply(&read_document(&mut socket));
            let cursor = number(page(&parse(&first)).get("continuation").unwrap()).unwrap();
            send(&mut socket, &first);
            let document = read_document(&mut socket);
            assert_eq!(
                decode_request(&document).unwrap(),
                request(2, 1, Some(cursor))
            );
            let second = fixture.reply(&document);
            let next = number(page(&parse(&second)).get("continuation").unwrap()).unwrap();
            assert_ne!(cursor, next);
            send(&mut socket, &damage(&second, cursor, next, fault));
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), 2, "{fault:?} caused another query");
            first
        });
        let mut output = Vec::new();
        let error = command(&args(&profile, 1, 4), &mut output).unwrap_err();
        assert!(error.contains("invalid"), "{fault:?}: {error}");
        assert_eq!(
            output,
            (server.join().unwrap() + "\n").as_bytes(),
            "{fault:?}"
        );
    }
}

#[test]
fn genuine_new_capture_cannot_replace_the_original_cut() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 6, 6, 2);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(6, 6, 2);
        let mut socket = bounded_socket(&listener);
        let first = fixture.reply(&read_document(&mut socket));
        let original_cursor = number(page(&parse(&first)).get("continuation").unwrap()).unwrap();
        send(&mut socket, &first);
        fixture.advance();
        let document = read_document(&mut socket);
        assert_eq!(
            decode_request(&document).unwrap(),
            request(2, 1, Some(original_cursor))
        );
        let fresh = fixture.reply(encode_request(&request(99, 1, None)).unwrap().as_bytes());
        let fresh_cursor = number(page(&parse(&fresh)).get("continuation").unwrap()).unwrap();
        let replacement = fixture.reply(
            encode_request(&request(2, 1, Some(fresh_cursor)))
                .unwrap()
                .as_bytes(),
        );
        assert_ne!(basis(&parse(&first)), basis(&parse(&replacement)));
        send(&mut socket, &replacement);
        disconnected(&mut socket, &listener);
        assert_eq!(fixture.remaining(), 3);
        first
    });
    let mut output = Vec::new();
    let error = command(&args(&profile, 1, 6), &mut output).unwrap_err();
    assert!(error.contains("inconsistent native journal"), "{error}");
    assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
}

#[test]
fn profile_page_bound_and_authenticated_server_identity_precede_more_work() {
    for wrong_peer in [false, true] {
        let root = Directory::new();
        let pid = credentials().pid();
        let selected_pid = if wrong_peer {
            if pid == 1 { 2 } else { 1 }
        } else {
            pid
        };
        let profile = root.settings(selected_pid, 4, 4, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(4, 4, 3);
            let mut socket = bounded_socket(&listener);
            if !wrong_peer {
                let response = fixture.reply(&read_document(&mut socket));
                send(&mut socket, &response);
            }
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), if wrong_peer { 4 } else { 3 });
        });
        let mut output = Vec::new();
        assert!(command(&args(&profile, 0, 4), &mut output).is_err());
        assert!(output.is_empty());
        server.join().unwrap();
    }
}

fn at_mut<'a>(value: &'a mut Json, path: &[&str]) -> &'a mut Json {
    let mut selected = value;
    for field in path {
        let Json::Object(object) = selected else {
            panic!("not an object at {field}")
        };
        selected = object.get_mut(*field).unwrap();
    }
    selected
}

#[test]
fn first_page_requires_every_scope_coordinate_live_origin_and_exact_metadata() {
    let mut fixture = Fixture::new(3, 4, 2);
    let request = request(1, 0, None);
    let original = parse(&fixture.reply(encode_request(&request).unwrap().as_bytes()));
    let scope = native::profile().delivery.scope;
    assert!(matches!(
        Capture::new(scope, 0, 2).accept(&original, &request),
        Ok(Progress::Continue(_))
    ));
    for name in ["tenant", "principal", "run", "branch", "authority"] {
        let mut changed = original.clone();
        *at_mut(
            &mut changed,
            &["result", "value", "knowledge", "basis", "scope", name],
        ) = Json::String("999".into());
        assert!(
            Capture::new(scope, 0, 2)
                .accept(&changed, &request)
                .is_err(),
            "{name}"
        );
    }
    for (path, replacement) in [
        (
            vec!["result", "value", "knowledge", "basis", "origin"],
            Json::String("unanchored_replay".into()),
        ),
        (
            vec!["result", "value", "knowledge", "basis", "anchored_through"],
            Json::String("3".into()),
        ),
        (
            vec!["result", "value", "disclosure"],
            Json::String("none".into()),
        ),
        (vec!["result", "value", "historical"], Json::Bool(false)),
        (
            vec!["result", "value", "current_eligibility"],
            Json::String("eligible".into()),
        ),
        (
            vec!["result", "value", "knowledge", "value", "after_revision"],
            Json::String("00".into()),
        ),
        (
            vec!["result", "value", "knowledge", "value", "offset"],
            Json::String(u64::MAX.to_string()),
        ),
        (
            vec!["result", "value", "knowledge", "value", "events"],
            Json::Array(Vec::new()),
        ),
    ] {
        let mut changed = original.clone();
        *at_mut(&mut changed, &path) = replacement;
        assert!(
            Capture::new(scope, 0, 2)
                .accept(&changed, &request)
                .is_err(),
            "{path:?}"
        );
    }
    assert_eq!(
        fixture.remaining(),
        3,
        "wire rejection called the native owner"
    );
}

#[test]
fn explicit_document_response_uses_the_same_closed_tail_schema() {
    let mut fixture = Fixture::new(1, 4, 2);
    let request = request(1, 0, None);
    let original = fixture.reply(encode_request(&request).unwrap().as_bytes());
    assert!(parse_response(original.as_bytes(), &request).is_ok());
    for (old, replacement) in [
        ("\"revision\":\"1\"", "\"revision\":1"),
        ("\"kind\":\"time\"", "\"kind\":\"effect_succeeded\""),
        ("\"historical\":true", "\"historical\":false"),
        ("\"after_revision\":\"0\"", "\"after_revision\":\"1\""),
    ] {
        assert!(
            parse_response(
                replace_once(&original, old, replacement).as_bytes(),
                &request
            )
            .is_err()
        );
    }
}

#[test]
fn grammar_scope_and_explicit_disclosure_refuse_before_connection() {
    let missing = Path::new("/missing-tail-profile.json");
    let valid = args(missing, 0, 4);
    assert!(selection(&valid).is_ok());
    for (index, value) in [
        (0, "tower-situation"),
        (2, "fa://1/run/3"),
        (2, "fa://1/attempt/3?generation=11"),
        (3, "--since-seq"),
        (4, "-1"),
        (4, "00"),
        (4, "18446744073709551616"),
        (5, "--follow"),
        (6, "0"),
        (6, "1025"),
        (6, "+1"),
    ] {
        let mut changed = valid.clone();
        changed[index] = value.into();
        let error = command(&changed, &mut Vec::new()).unwrap_err();
        assert_eq!(error, USAGE);
    }
    for extra in [
        "--operator-profile",
        "--guarded",
        "--tower-generation",
        "--follow",
    ] {
        let mut changed = valid.clone();
        changed.push(extra.into());
        assert_eq!(command(&changed, &mut Vec::new()).unwrap_err(), USAGE);
    }
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 4, 4, 2);
    let listener = UnixListener::bind(root.socket()).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut mismatched = args(&profile, 0, 4);
    mismatched[2] = Address::new("99", Noun::Run, "3", Some(GENERATION))
        .unwrap()
        .canonical_string();
    assert!(
        command(&mismatched, &mut Vec::new())
            .unwrap_err()
            .contains("does not match")
    );
    let document = fs::read_to_string(&profile).unwrap();
    let document = document.replace("\"inspection_queries\":0", "\"inspection_queries\":1")
        .replace("\"journal_tail\":{\"queries\":4,\"events_per_page\":2,\"disclosure\":\"redacted_journal_events\"}", "\"journal_tail\":null");
    fs::write(&profile, document).unwrap();
    assert!(
        command(&args(&profile, 0, 4), &mut Vec::new())
            .unwrap_err()
            .contains("explicit journal_tail")
    );
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}
