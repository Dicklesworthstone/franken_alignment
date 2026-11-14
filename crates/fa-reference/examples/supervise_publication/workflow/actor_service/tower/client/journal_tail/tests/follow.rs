//! Follow remains a finite sequence of original native captures. These tests
//! exercise the real owner and transport; altered replies are client-boundary
//! adversaries, never evidence that the native owner emitted invalid history.

use super::*;

fn follow_args(profile: &Path, after: u64, pages: usize, captures: usize) -> Vec<String> {
    let mut selected = args(profile, after, pages);
    selected.extend(["--follow-captures".into(), captures.to_string()]);
    selected
}

#[test]
fn live_follow_covers_arrivals_during_pagination_once_without_changing_the_original_cut() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8, 8, 2);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(3, 8, 2);
        let mut socket = bounded_socket(&listener);
        let mut cursor = None;
        let mut replies = Vec::new();
        let mut revisions = Vec::new();
        for (index, after, through, complete) in [
            (1, 0, 3, false),
            (2, 0, 3, true),
            (3, 3, 5, true),
            (4, 5, 6, true),
        ] {
            let document = read_document(&mut socket);
            assert_eq!(
                decode_request(&document).unwrap(),
                request(index, after, cursor)
            );
            let response = fixture.reply(&document);
            let parsed = parse(&response);
            assert_eq!(
                number(basis(&parsed).get("journal_revision").unwrap()).unwrap(),
                through
            );
            assert_eq!(
                page(&parsed)
                    .get("enumeration_complete")
                    .and_then(Json::as_bool),
                Some(complete)
            );
            revisions.extend(
                page(&parsed)
                    .get("events")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|event| number(event.get("revision").unwrap()).unwrap()),
            );
            cursor = optional_number(page(&parsed).get("continuation").unwrap()).unwrap();
            if index == 1 {
                fixture.advance();
                fixture.advance();
            } else if index == 3 {
                fixture.advance();
            }
            send(&mut socket, &response);
            replies.push(response);
        }
        assert_eq!(revisions, (1..=6).collect::<Vec<_>>());
        assert_eq!(fixture.remaining(), 4);
        assert_eq!(
            fixture
                .supervisor
                .as_ref()
                .unwrap()
                .host()
                .unwrap()
                .revision(),
            6
        );
        disconnected(&mut socket, &listener);
        replies
    });
    let mut output = Vec::new();
    crate::command_with_output(follow_args(&profile, 0, 8, 3), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn empty_captures_keep_following_until_the_explicit_capture_limit() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 4, 4, 2);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(1, 4, 2);
        let mut socket = bounded_socket(&listener);
        let mut replies = Vec::new();
        for index in 1..=3 {
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(index, 1, None));
            let response = fixture.reply(&document);
            let parsed = parse(&response);
            assert_eq!(
                page(&parsed)
                    .get("events")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .len(),
                usize::from(index == 3)
            );
            assert_eq!(
                page(&parsed)
                    .get("enumeration_complete")
                    .and_then(Json::as_bool),
                Some(true)
            );
            if index == 2 {
                fixture.advance();
            }
            send(&mut socket, &response);
            replies.push(response);
        }
        assert_eq!(fixture.remaining(), 1);
        disconnected(&mut socket, &listener);
        replies
    });
    let mut output = Vec::new();
    command(&follow_args(&profile, 1, 4, 3), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn follow_never_recaptures_an_incomplete_suffix_or_renews_any_shared_page_allowance() {
    for (max_pages, exchanges, selected_queries, actual_queries) in
        [(1, 4, 4, 4), (4, 1, 4, 4), (4, 4, 1, 4), (4, 4, 4, 1)]
    {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), exchanges, selected_queries, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(3, actual_queries, 2);
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, 0, None));
            let response = fixture.reply(&document);
            assert_eq!(
                page(&parse(&response))
                    .get("enumeration_complete")
                    .and_then(Json::as_bool),
                Some(false)
            );
            send(&mut socket, &response);
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), actual_queries - 1);
            response
        });
        let mut output = Vec::new();
        assert!(
            command(&follow_args(&profile, 0, max_pages, 2), &mut output)
                .unwrap_err()
                .contains("incomplete")
        );
        assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
    }
}

#[test]
fn completed_captures_do_not_renew_total_pages_and_a_short_follow_reports_incomplete() {
    for (max_pages, exchanges, queries) in [(2, 4, 4), (4, 2, 4), (4, 4, 2)] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), exchanges, queries, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(0, 4, 2);
            let mut socket = bounded_socket(&listener);
            let mut replies = Vec::new();
            for index in 1..=2 {
                let document = read_document(&mut socket);
                assert_eq!(decode_request(&document).unwrap(), request(index, 0, None));
                let response = fixture.reply(&document);
                send(&mut socket, &response);
                replies.push(response);
            }
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), 2);
            replies
        });
        let mut output = Vec::new();
        let error = command(&follow_args(&profile, 0, max_pages, 3), &mut output).unwrap_err();
        assert!(
            error.contains("incomplete") && error.contains("2 of 3"),
            "{error}"
        );
        assert_eq!(
            output,
            (server.join().unwrap().join("\n") + "\n").as_bytes()
        );
    }
}

#[test]
fn a_following_capture_cannot_relabel_its_start_or_regress_its_acknowledged_basis() {
    for mode in 0..4 {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4, 4, 4);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(2, 4, 4);
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, 0, None));
            let first = fixture.reply(&document);
            if mode != 1 {
                fixture.advance();
            }
            send(&mut socket, &first);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(2, 2, None));
            let second = fixture.reply(&document);
            let changed = match mode {
                0 => replace_once(
                    &second,
                    "\"after_revision\":\"2\"",
                    "\"after_revision\":\"1\"",
                ),
                // An unchanged revision cannot have a different clock basis.
                1 => replace_once(&second, "\"recorded_at\":\"2\"", "\"recorded_at\":\"3\""),
                // Even a later cut cannot erase its last recorded time.
                2 => replace_once(&second, "\"recorded_at\":\"3\"", "\"recorded_at\":null"),
                _ => replace_once(&second, "\"recorded_at\":\"3\"", "\"recorded_at\":\"1\""),
            };
            send(&mut socket, &changed);
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), 2);
            first
        });
        let mut output = Vec::new();
        assert!(
            command(&follow_args(&profile, 0, 4, 3), &mut output)
                .unwrap_err()
                .contains("invalid")
        );
        assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
    }
}

#[test]
fn loss_of_owner_grant_or_native_queries_stops_follow_with_the_original_unknown_or_error() {
    for mode in 0..3 {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4, 4, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(1, if mode == 2 { 1 } else { 4 }, 2);
            let mut socket = bounded_socket(&listener);
            let first = fixture.reply(&read_document(&mut socket));
            match mode {
                0 => drop(fixture.supervisor.take()),
                1 => drop(fixture.grant.take()),
                _ => {}
            }
            send(&mut socket, &first);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(2, 1, None));
            let second = fixture.reply(&document);
            assert!(!second.contains("\"state\":\"known\""));
            send(&mut socket, &second);
            disconnected(&mut socket, &listener);
            vec![first, second]
        });
        let mut output = Vec::new();
        assert!(
            command(&follow_args(&profile, 0, 4, 3), &mut output)
                .unwrap_err()
                .contains("incomplete")
        );
        assert_eq!(
            output,
            (server.join().unwrap().join("\n") + "\n").as_bytes()
        );
    }
}

#[test]
fn failed_output_of_a_complete_capture_cannot_start_the_next_capture() {
    struct Broken {
        on_write: bool,
    }
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.on_write {
                Err(io::ErrorKind::BrokenPipe.into())
            } else {
                Ok(bytes.len())
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    for on_write in [false, true] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4, 4, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(1, 4, 2);
            let mut socket = bounded_socket(&listener);
            let response = fixture.reply(&read_document(&mut socket));
            assert_eq!(
                page(&parse(&response))
                    .get("enumeration_complete")
                    .and_then(Json::as_bool),
                Some(true)
            );
            send(&mut socket, &response);
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), 3);
        });
        assert!(
            command(&follow_args(&profile, 0, 4, 2), &mut Broken { on_write })
                .unwrap_err()
                .contains("output failed")
        );
        server.join().unwrap();
    }
}

#[test]
fn deadline_expiry_after_empty_output_cannot_start_a_following_capture() {
    struct SlowOutput(Vec<u8>);
    impl Write for SlowOutput {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            std::thread::sleep(Duration::from_millis(1100));
            Ok(())
        }
    }
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 4, 4, 2);
    let document = fs::read_to_string(&profile)
        .unwrap()
        .replace("\"runtime_ms\":10000", "\"runtime_ms\":1000");
    fs::write(&profile, document).unwrap();
    let listener = UnixListener::bind(root.socket()).unwrap();
    let (ready, started) = std::sync::mpsc::channel();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(0, 4, 2);
        ready.send(()).unwrap();
        let mut socket = bounded_socket(&listener);
        let response = fixture.reply(&read_document(&mut socket));
        send(&mut socket, &response);
        disconnected(&mut socket, &listener);
        assert_eq!(fixture.remaining(), 3);
        response
    });
    started.recv_timeout(Duration::from_secs(10)).unwrap();
    let mut output = SlowOutput(Vec::new());
    assert!(
        command(&follow_args(&profile, 0, 4, 2), &mut output)
            .unwrap_err()
            .contains("deadline")
    );
    assert_eq!(output.0, (server.join().unwrap() + "\n").as_bytes());
}

#[test]
fn follow_requires_an_explicit_canonical_finite_capture_count_before_connecting() {
    let missing = Path::new("/missing-follow-profile.json");
    for count in [1, MAX_JOURNAL_TAIL_QUERIES] {
        assert!(selection(&follow_args(missing, 0, 4, count)).is_ok());
    }
    for text in [
        "0",
        "00",
        "01",
        "+1",
        "-1",
        "1025",
        "18446744073709551616",
        "",
        "--follow",
    ] {
        let mut selected = follow_args(missing, 0, 4, 2);
        selected[8] = text.into();
        assert_eq!(command(&selected, &mut Vec::new()).unwrap_err(), USAGE);
    }
    let mut absent = follow_args(missing, 0, 4, 2);
    absent.pop();
    assert_eq!(command(&absent, &mut Vec::new()).unwrap_err(), USAGE);
    let mut duplicate = follow_args(missing, 0, 4, 2);
    duplicate.extend(["--follow-captures".into(), "2".into()]);
    assert_eq!(command(&duplicate, &mut Vec::new()).unwrap_err(), USAGE);
}
