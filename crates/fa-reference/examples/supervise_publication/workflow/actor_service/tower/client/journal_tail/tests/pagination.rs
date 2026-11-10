use super::*;

#[test]
fn original_revisions_follow_issued_cursors_at_one_cut_while_owner_advances() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8, 4, 2);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(6, 4, 2);
        let mut socket = bounded_socket(&listener);
        let mut next = None;
        let mut original_basis = None;
        let mut revisions = Vec::new();
        let mut replies = Vec::new();
        for index in 0..3 {
            let document = read_document(&mut socket);
            assert_eq!(
                decode_request(&document).unwrap(),
                request(index + 1, 1, next)
            );
            let response = fixture.reply(&document);
            let parsed = parse(&response);
            if index == 0 {
                original_basis = Some(basis(&parsed).clone());
            }
            assert_eq!(Some(basis(&parsed)), original_basis.as_ref());
            assert_eq!(
                number(basis(&parsed).get("journal_revision").unwrap()).unwrap(),
                6
            );
            assert_eq!(
                number(page(&parsed).get("after_revision").unwrap()).unwrap(),
                1
            );
            assert_eq!(
                number(page(&parsed).get("offset").unwrap()).unwrap(),
                index as u64 * 2
            );
            let rows = page(&parsed).get("events").unwrap().as_array().unwrap();
            assert_eq!(rows.len(), if index == 2 { 1 } else { 2 });
            for row in rows {
                revisions.push(number(row.get("revision").unwrap()).unwrap());
                assert_eq!(row.get("kind").and_then(Json::as_str), Some("time"));
            }
            assert_eq!(
                page(&parsed)
                    .get("enumeration_complete")
                    .and_then(Json::as_bool),
                Some(index == 2)
            );
            next = optional_number(page(&parsed).get("continuation").unwrap()).unwrap();
            send(&mut socket, &response);
            replies.push(response);
            if index == 0 {
                fixture.advance();
            }
        }
        assert_eq!(revisions, (2..=6).collect::<Vec<_>>());
        assert_eq!(fixture.remaining(), 1);
        assert!(next.is_none());
        assert_eq!(
            fixture
                .supervisor
                .as_ref()
                .unwrap()
                .host()
                .unwrap()
                .revision(),
            7
        );
        disconnected(&mut socket, &listener);
        replies
    });
    let mut output = Vec::new();
    crate::command_with_output(args(&profile, 1, 8), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn empty_suffix_and_exact_full_page_complete_on_the_last_query() {
    for (ticks, after) in [(0, 0), (3, 3), (3, 1)] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 1, 1, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(ticks, 1, 2);
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, after, None));
            let response = fixture.reply(&document);
            let parsed = parse(&response);
            assert_eq!(
                page(&parsed)
                    .get("events")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .len() as u64,
                ticks - after
            );
            assert_eq!(
                page(&parsed)
                    .get("enumeration_complete")
                    .and_then(Json::as_bool),
                Some(true)
            );
            assert!(page(&parsed).get("continuation").unwrap().is_null());
            send(&mut socket, &response);
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), 0);
            response
        });
        let mut output = Vec::new();
        command(&args(&profile, after, 1), &mut output).unwrap();
        assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
    }
}

#[test]
fn caller_transport_profile_and_native_caps_preserve_explicitly_partial_output() {
    for (max_pages, exchanges, selected_queries, actual_queries, cursor_expected) in [
        (1, 4, 4, 4, true),
        (4, 1, 4, 4, true),
        (4, 4, 1, 4, true),
        (4, 4, 4, 1, false),
    ] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), exchanges, selected_queries, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(3, actual_queries, 2);
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, 0, None));
            let response = fixture.reply(&document);
            let parsed = parse(&response);
            assert_eq!(
                page(&parsed)
                    .get("enumeration_complete")
                    .and_then(Json::as_bool),
                Some(false)
            );
            assert_eq!(
                !page(&parsed).get("continuation").unwrap().is_null(),
                cursor_expected
            );
            send(&mut socket, &response);
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), actual_queries - 1);
            response
        });
        let mut output = Vec::new();
        let error = command(&args(&profile, 0, max_pages), &mut output).unwrap_err();
        assert!(error.contains("incomplete"), "{error}");
        assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
    }
}

#[test]
fn unknown_withheld_future_refusal_and_capacity_never_become_empty_completion() {
    for mode in 0..5 {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4, 4, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let after = if mode == 2 { 1 } else { 0 };
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(0, if mode == 3 { 0 } else { 1 }, 2);
            match mode {
                0 => drop(fixture.supervisor.take()),
                1 => drop(fixture.grant.take()),
                4 => {
                    fixture.reply(encode_request(&request(99, 0, None)).unwrap().as_bytes());
                }
                _ => {}
            }
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, after, None));
            let response = fixture.reply(&document);
            assert!(!response.contains("\"state\":\"known\""));
            send(&mut socket, &response);
            disconnected(&mut socket, &listener);
            response
        });
        let mut output = Vec::new();
        let error = command(&args(&profile, after, 4), &mut output).unwrap_err();
        assert!(error.contains("incomplete"), "{error}");
        assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
    }
}

#[test]
fn owner_loss_after_page_keeps_original_output_and_unknown_without_recapture() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 4, 4, 2);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(3, 4, 2);
        let mut socket = bounded_socket(&listener);
        let first = fixture.reply(&read_document(&mut socket));
        let cursor = number(page(&parse(&first)).get("continuation").unwrap()).unwrap();
        send(&mut socket, &first);
        drop(fixture.supervisor.take());
        let next = read_document(&mut socket);
        assert_eq!(decode_request(&next).unwrap(), request(2, 0, Some(cursor)));
        let second = fixture.reply(&next);
        assert!(second.contains("\"state\":\"unknown\""));
        send(&mut socket, &second);
        disconnected(&mut socket, &listener);
        vec![first, second]
    });
    let mut output = Vec::new();
    let error = command(&args(&profile, 0, 4), &mut output).unwrap_err();
    assert!(error.contains("Unknown"), "{error}");
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
}

#[test]
fn output_loss_cannot_spend_the_next_page_or_restore_query_budget() {
    struct Broken {
        write: bool,
    }
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.write {
                Err(io::ErrorKind::BrokenPipe.into())
            } else {
                Ok(bytes.len())
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    for write in [true, false] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4, 4, 2);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(3, 4, 2);
            let mut socket = bounded_socket(&listener);
            let response = fixture.reply(&read_document(&mut socket));
            send(&mut socket, &response);
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), 3);
        });
        assert!(
            command(&args(&profile, 0, 4), &mut Broken { write })
                .unwrap_err()
                .contains("output failed")
        );
        server.join().unwrap();
    }
}
