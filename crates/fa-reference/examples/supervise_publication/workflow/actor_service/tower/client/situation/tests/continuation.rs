use super::*;

#[test]
fn thirty_three_native_attempts_follow_exact_cursors_from_one_cut_while_owner_advances() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(33, 4);
        let mut socket = bounded_socket(&listener);
        let mut next = None;
        let mut original_basis = None;
        let mut original_summary = None;
        let mut ids = Vec::new();
        let mut replies = Vec::new();
        for index in 0..3 {
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(index + 1, next));
            let response = fixture.reply(&document);
            let page = native_page(&response);
            if index == 0 {
                original_basis = Some(page.basis());
                original_summary = Some(page.summary());
            }
            assert_eq!(Some(page.basis()), original_basis);
            assert_eq!(Some(page.summary()), original_summary);
            assert_eq!(page.summary().attempts, 33);
            assert_eq!(page.summary().terminal_attempts, 0);
            assert_eq!(page.basis().recorded_at, Some(ElapsedTick(1)));
            assert_eq!(page.offset(), index * 16);
            assert_eq!(page.attempts().len(), if index < 2 { 16 } else { 1 });
            assert_eq!(page.enumeration_complete(), index == 2);
            ids.extend(page.attempts().iter().map(|row| row.attempt));
            next = page.continuation().map(|cursor| cursor.id());
            let original = response.to_json().unwrap();
            send(&mut socket, &original);
            replies.push(original);
            if index == 0 {
                fixture.advance();
            }
        }
        assert_eq!(ids, (1..=33).collect::<Vec<_>>());
        assert_eq!(fixture.remaining(), 1);
        assert!(next.is_none());
        disconnected(&mut socket, &listener);
        replies
    });
    let mut output = Vec::new();
    crate::command_with_output(args(&profile, 8), &mut output).unwrap();
    let original = server.join().unwrap().join("\n") + "\n";
    assert_eq!(
        output,
        original.as_bytes(),
        "the client rewrote or omitted an original page"
    );
}

#[test]
fn empty_and_exactly_one_full_page_are_complete_at_the_last_allowed_query() {
    for attempts in [0, 16] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 1);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(attempts, 1);
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, None));
            let response = fixture.reply(&document);
            let page = native_page(&response);
            assert_eq!(page.attempts().len() as u64, attempts);
            assert!(page.enumeration_complete());
            assert!(page.continuation().is_none());
            let original = response.to_json().unwrap();
            send(&mut socket, &original);
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), 0);
            original
        });
        let mut output = Vec::new();
        command(&args(&profile, 1), &mut output).unwrap();
        assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
    }
}

#[test]
fn page_exchange_and_native_grant_caps_keep_valid_partial_output_without_recapture() {
    for (max_pages, exchanges, queries, cursor_expected) in
        [(1, 4, 4, true), (4, 1, 4, true), (4, 4, 1, false)]
    {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), exchanges);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(33, queries);
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, None));
            let response = fixture.reply(&document);
            let page = native_page(&response);
            assert_eq!(page.attempts().len(), 16);
            assert!(!page.enumeration_complete());
            assert_eq!(page.continuation().is_some(), cursor_expected);
            let original = response.to_json().unwrap();
            send(&mut socket, &original);
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), queries - 1);
            original
        });
        let mut output = Vec::new();
        let error = command(&args(&profile, max_pages), &mut output).unwrap_err();
        assert!(error.contains("incomplete"));
        assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
        assert_eq!(emitted(&output).len(), 1);
    }
}

#[test]
fn valid_totals_larger_than_client_row_allowance_remain_explicitly_partial() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 64);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(33, 4);
        let mut socket = bounded_socket(&listener);
        let response = fixture.reply(&read_document(&mut socket));
        // A larger wire total exercises bounded client work, not a claim that
        // this native fixture admitted more than its configured 64 attempts.
        let original = response.to_json().unwrap();
        let enlarged = replace_once(&original, "\"attempts\":\"33\"", "\"attempts\":\"1025\"");
        let enlarged = replace_once(
            &enlarged,
            "\"pending_attempts\":\"33\"",
            "\"pending_attempts\":\"1025\"",
        );
        send(&mut socket, &enlarged);
        disconnected(&mut socket, &listener);
        assert_eq!(fixture.remaining(), 3);
        enlarged
    });
    let mut output = Vec::new();
    let error = command(&args(&profile, 1), &mut output).unwrap_err();
    assert!(error.contains("allowance exhausted"), "{error}");
    assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
}

#[test]
fn write_or_flush_output_loss_cannot_send_the_next_page_or_restore_query_budget() {
    struct Broken {
        fail_write: bool,
    }
    impl Write for Broken {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.fail_write {
                Err(io::ErrorKind::BrokenPipe.into())
            } else {
                Ok(bytes.len())
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }
    for fail_write in [true, false] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(33, 4);
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, None));
            let response = fixture.reply(&document);
            assert!(native_page(&response).continuation().is_some());
            send(&mut socket, &response.to_json().unwrap());
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), 3);
        });
        let error = command(&args(&profile, 4), &mut Broken { fail_write }).unwrap_err();
        assert!(error.contains("output failed"));
        server.join().unwrap();
    }
}

#[test]
fn unknown_withheld_refusal_and_native_capacity_diagnostics_never_become_completion() {
    for mode in 0..4 {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(0, if mode == 2 { 0 } else { 1 });
            match mode {
                0 => drop(fixture.supervisor.take()),
                1 => drop(fixture.grant.take()),
                2 => {}
                _ => {
                    fixture.reply(encode_request(&request(99, None)).unwrap().as_bytes());
                }
            }
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            assert_eq!(decode_request(&document).unwrap(), request(1, None));
            let response = fixture.reply(&document);
            assert!(!matches!(
                &response.result,
                PublicationResult::Situation(Ok(Knowledge::Known { .. }))
            ));
            let original = response.to_json().unwrap();
            send(&mut socket, &original);
            disconnected(&mut socket, &listener);
            original
        });
        let mut output = Vec::new();
        let error = command(&args(&profile, 4), &mut output).unwrap_err();
        assert!(error.contains("incomplete"), "{error}");
        assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
        assert_eq!(emitted(&output).len(), 1);
    }
}

#[test]
fn owner_loss_after_first_page_keeps_the_original_page_and_unknown_without_recapture() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 4);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(33, 4);
        let mut socket = bounded_socket(&listener);
        let first = fixture.reply(&read_document(&mut socket));
        let cursor = native_page(&first).continuation().unwrap().id();
        let first = first.to_json().unwrap();
        send(&mut socket, &first);
        drop(fixture.supervisor.take());
        let document = read_document(&mut socket);
        assert_eq!(decode_request(&document).unwrap(), request(2, Some(cursor)));
        let second = fixture.reply(&document);
        assert!(matches!(
            &second.result,
            PublicationResult::Situation(Ok(Knowledge::Unknown { .. }))
        ));
        let second = second.to_json().unwrap();
        send(&mut socket, &second);
        disconnected(&mut socket, &listener);
        vec![first, second]
    });
    let mut output = Vec::new();
    let error = command(&args(&profile, 4), &mut output).unwrap_err();
    assert!(error.contains("Unknown"));
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
    assert_eq!(emitted(&output).len(), 2);
}

#[test]
fn transport_loss_after_one_valid_page_preserves_it_and_never_reconnects() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 4);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let check_listener = listener.try_clone().unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(33, 4);
        let mut socket = bounded_socket(&listener);
        let first = fixture.reply(&read_document(&mut socket));
        let cursor = native_page(&first).continuation().unwrap().id();
        let first = first.to_json().unwrap();
        send(&mut socket, &first);
        assert_eq!(
            decode_request(&read_document(&mut socket)).unwrap(),
            request(2, Some(cursor))
        );
        socket.write_all(b"{\"envelope\":").unwrap();
        socket.flush().unwrap();
        drop(socket);
        assert_eq!(
            fixture.remaining(),
            3,
            "an unserved next request spent native work"
        );
        first
    });
    let mut output = Vec::new();
    let error = command(&args(&profile, 4), &mut output).unwrap_err();
    assert!(error.contains("closed before a complete response"));
    assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
    check_listener.set_nonblocking(true).unwrap();
    assert_eq!(
        check_listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}
