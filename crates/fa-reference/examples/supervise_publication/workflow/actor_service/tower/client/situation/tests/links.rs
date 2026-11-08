//! Original linked native pages over one bounded operator connection. Altered
//! reply copies below test the reader; they are not claims about native history.

use super::*;
use fa_reference::action::consequence::oversight::actor::MAX_ACTOR_REQUESTS;
use fa_reference::tower::publication::request_address_for;

fn linked_args(profile: &Path, max_pages: usize) -> Vec<String> {
    let mut selected = args(profile, max_pages);
    selected.push("--request-links".into());
    selected
}

fn linked_request(page: usize, cursor: Option<u64>) -> TowerRequest {
    let mut selected = request(page, cursor);
    selected.arguments = ReadArguments::SituationLinks { cursor };
    selected
}

fn linked_page(
    response: &PublicationResponse,
) -> &fa_reference::action::consequence::delivery::persistent::requests::actor::FileInspectionPage {
    let PublicationResult::SituationLinks(linked) = &response.result else {
        panic!("expected sealed linked situation: {response:?}")
    };
    let Ok(Knowledge::Known { value, basis }) = linked.native() else {
        panic!("expected original linked native page: {response:?}")
    };
    assert_eq!(value.basis(), *basis);
    value
}

fn linked_capture(max_pages: usize) -> Capture {
    Capture::new(native::profile().delivery.scope, max_pages).with_request_links(mount())
}

fn checked(fixture: &mut Fixture, request: &TowerRequest) -> (PublicationResponse, Json) {
    let response = fixture.reply(encode_request(request).unwrap().as_bytes());
    let parsed = parse_response(response.to_json().unwrap().as_bytes(), request).unwrap();
    (response, parsed)
}

fn at_mut<'a>(value: &'a mut Json, path: &[&str]) -> &'a mut Json {
    let mut selected = value;
    for field in path {
        let Json::Object(object) = selected else {
            panic!("object at {field}")
        };
        selected = object.get_mut(*field).unwrap();
    }
    selected
}

fn rows_mut(value: &mut Json) -> &mut Vec<Json> {
    let Json::Array(rows) = at_mut(
        value,
        &["result", "value", "knowledge", "value", "attempts"],
    ) else {
        panic!("native attempt rows")
    };
    rows
}

fn row_mut(value: &mut Json, index: usize) -> &mut BTreeMap<String, Json> {
    let Json::Object(row) = &mut rows_mut(value)[index] else {
        panic!("native attempt row")
    };
    row
}

fn link_address(request: u64) -> String {
    request_address_for(native::profile().delivery.scope, &mount(), request)
        .unwrap()
        .canonical_string()
}

fn assign_link(value: &mut Json, index: usize, request: u64) {
    let row = row_mut(value, index);
    row.insert("external_request".into(), Json::String(request.to_string()));
    row.insert(
        "request_address".into(),
        Json::String(link_address(request)),
    );
}

#[test]
fn opt_in_follows_thirty_three_original_linked_rows_across_owner_advance_without_other_reads() {
    let root = Directory::new();
    let profile = root.settings(credentials().pid(), 8);
    let listener = UnixListener::bind(root.socket()).unwrap();
    let server = std::thread::spawn(move || {
        let mut fixture = Fixture::new(33, 4);
        let mut socket = bounded_socket(&listener);
        let mut next = None;
        let mut original_basis = None;
        let mut original_summary = None;
        let mut identities = Vec::new();
        let mut replies = Vec::new();
        for index in 0..3 {
            let document = read_document(&mut socket);
            assert_eq!(
                decode_request(&document).unwrap(),
                linked_request(index + 1, next)
            );
            let response = fixture.reply(&document);
            let page = linked_page(&response);
            if index == 0 {
                original_basis = Some(page.basis());
                original_summary = Some(page.summary());
            }
            assert_eq!(Some(page.basis()), original_basis);
            assert_eq!(Some(page.summary()), original_summary);
            assert_eq!(page.summary().attempts, 33);
            assert_eq!(page.summary().terminal_attempts, 0);
            assert_eq!(page.offset(), index * MAX_INSPECTION_ATTEMPTS);
            assert_eq!(page.enumeration_complete(), index == 2);
            identities.extend(
                page.attempts()
                    .iter()
                    .map(|row| (row.attempt, row.external_request)),
            );
            next = page.continuation().map(|cursor| cursor.id());
            let original = response.to_json().unwrap();
            assert!(original.contains("\"schema\":\"fa.publication-situation/2\""));
            send(&mut socket, &original);
            replies.push(original);
            if index == 0 {
                fixture.advance();
            }
        }
        assert_eq!(
            identities,
            (1_u64..=33)
                .map(|attempt| (attempt, Some(1000 + attempt)))
                .collect::<Vec<_>>()
        );
        assert_eq!(fixture.remaining(), 1);
        assert!(next.is_none());
        // Any automatic Explain/Get, retry or reconnect is a failure here.
        disconnected(&mut socket, &listener);
        replies
    });
    let mut output = Vec::new();
    crate::command_with_output(linked_args(&profile, 8), &mut output).unwrap();
    assert_eq!(
        output,
        (server.join().unwrap().join("\n") + "\n").as_bytes()
    );
    assert_eq!(emitted(&output).len(), 3);
}

#[test]
fn original_full_width_and_direct_links_require_explicit_schema_selection() {
    let mut fixture = Fixture::new(0, 2);
    let supervisor = fixture.supervisor.as_mut().unwrap();
    {
        let mut host = supervisor.host_mut().unwrap();
        let direct = native::oversight::spec(&host, b"direct");
        let revision = host.revision();
        host.propose(revision, 1, direct, native::snapshot())
            .unwrap();
    }
    native::observe(supervisor);
    fixture.actor.submit(u64::MAX, &fixture.proposal).unwrap();
    let (response, original) = checked(&mut fixture, &linked_request(1, None));
    assert_eq!(
        linked_page(&response)
            .attempts()
            .iter()
            .map(|row| (row.attempt, row.external_request))
            .collect::<Vec<_>>(),
        vec![(1, None), (2, Some(u64::MAX))]
    );
    assert!(
        response
            .to_json()
            .unwrap()
            .contains("\"external_request\":\"18446744073709551615\"")
    );
    assert!(matches!(
        linked_capture(1).accept(&original),
        Ok(Progress::Complete)
    ));
    assert!(
        Capture::new(native::profile().delivery.scope, 1)
            .accept(&original)
            .is_err()
    );

    let (_response, legacy) = checked(&mut fixture, &request(1, None));
    assert!(matches!(
        Capture::new(native::profile().delivery.scope, 1).accept(&legacy),
        Ok(Progress::Complete)
    ));
    assert!(linked_capture(1).accept(&legacy).is_err());
    let mut downgrade = original;
    *at_mut(&mut downgrade, &["result", "value", "schema"]) =
        Json::String("fa.publication-situation/1".into());
    assert!(linked_capture(1).accept(&downgrade).is_err());
    let mut upgrade = legacy;
    *at_mut(&mut upgrade, &["result", "value", "schema"]) =
        Json::String("fa.publication-situation/2".into());
    assert!(
        linked_capture(1).accept(&upgrade).is_err(),
        "v1 rows cannot be relabelled as linked rows"
    );
}

#[test]
fn complete_scope_registration_and_key_pair_are_validated_before_retaining_rows() {
    let mut fixture = Fixture::new(17, 2);
    let (_response, original) = checked(&mut fixture, &linked_request(1, None));
    for address in [
        "fa://2/attempt/request-3-2-4-5-1001@11",
        "fa://1/attempt/request-9-2-4-5-1001@11",
        "fa://1/attempt/request-3-9-4-5-1001@11",
        "fa://1/attempt/request-3-2-9-5-1001@11",
        "fa://1/attempt/request-3-2-4-9-1001@11",
        "fa://1/attempt/request-3-2-4-5-1001@12",
        "fa://1/attempt/request-3-2-4-5-1002@11",
        "fa://1/attempt/request-3-2-4-5-01001@11",
        "fa://1/attempt/request-3-2-4-5-1001@011",
        "fa://1/attempt/request-3-2-4-5-1001",
        "fa://1/run/request-3-2-4-5-1001@11",
        "fa://1/attempt/1@11",
    ] {
        let mut changed = original.clone();
        row_mut(&mut changed, 0).insert("request_address".into(), Json::String(address.into()));
        let mut capture = linked_capture(2);
        assert!(capture.accept(&changed).is_err(), "{address}");
        assert!(
            matches!(capture.accept(&original), Ok(Progress::Continue(_))),
            "rejected row poisoned {address}"
        );
    }
    for key in [
        Json::Null,
        Json::Bool(true),
        parse("1001"),
        Json::String("0".into()),
        Json::String("01001".into()),
        Json::String("+1001".into()),
        Json::String("18446744073709551616".into()),
    ] {
        let mut changed = original.clone();
        row_mut(&mut changed, 0).insert("external_request".into(), key.clone());
        assert!(linked_capture(2).accept(&changed).is_err(), "{key:?}");
    }
    for address in [Json::Null, Json::Bool(true), parse("1001")] {
        let mut changed = original.clone();
        row_mut(&mut changed, 0).insert("request_address".into(), address);
        assert!(linked_capture(2).accept(&changed).is_err());
    }
    for missing in ["external_request", "request_address"] {
        let mut changed = original.clone();
        row_mut(&mut changed, 0).remove(missing);
        assert!(
            linked_capture(2).accept(&changed).is_err(),
            "missing {missing}"
        );
    }
    let mut extra = original.clone();
    row_mut(&mut extra, 0).insert("ticket".into(), Json::Bool(true));
    assert!(linked_capture(2).accept(&extra).is_err());
    let mut duplicate = original;
    assign_link(&mut duplicate, 1, 1001);
    assert!(
        linked_capture(2).accept(&duplicate).is_err(),
        "distinct attempts cannot repeat one request key"
    );
}

#[test]
fn duplicate_request_across_original_pages_is_rejected_without_consuming_client_state() {
    let mut fixture = Fixture::new(33, 3);
    let (first_response, first) = checked(&mut fixture, &linked_request(1, None));
    let first_cursor = linked_page(&first_response).continuation().unwrap().id();
    let (second_response, second) = checked(&mut fixture, &linked_request(2, Some(first_cursor)));
    let second_cursor = linked_page(&second_response).continuation().unwrap().id();
    let mut capture = linked_capture(3);
    assert!(
        matches!(capture.accept(&first), Ok(Progress::Continue(value)) if value == first_cursor)
    );
    let mut duplicate = second.clone();
    // An earlier valid row must remain uncommitted when a later row fails.
    assign_link(&mut duplicate, 1, 1001);
    assert!(capture.accept(&duplicate).is_err());
    assert!(
        matches!(capture.accept(&second), Ok(Progress::Continue(value)) if value == second_cursor)
    );
    let (_last_response, last) = checked(&mut fixture, &linked_request(3, Some(second_cursor)));
    assert!(matches!(capture.accept(&last), Ok(Progress::Complete)));
    assert_eq!(fixture.remaining(), 0);
}

#[derive(Clone, Copy, Debug)]
enum Damage {
    Authority,
    DuplicateRequest,
    UnpairedNull,
    Downgrade,
}

fn damage(original: &str, fault: Damage) -> String {
    match fault {
        Damage::Authority => replace_once(
            original,
            "\"request_address\":\"fa://1/attempt/request-3-2-4-5-1017@11\"",
            "\"request_address\":\"fa://1/attempt/request-3-2-4-99-1017@11\"",
        ),
        Damage::DuplicateRequest => {
            let changed = replace_once(
                original,
                "\"external_request\":\"1017\"",
                "\"external_request\":\"1001\"",
            );
            replace_once(
                &changed,
                "\"request_address\":\"fa://1/attempt/request-3-2-4-5-1017@11\"",
                "\"request_address\":\"fa://1/attempt/request-3-2-4-5-1001@11\"",
            )
        }
        Damage::UnpairedNull => replace_once(
            original,
            "\"external_request\":\"1017\"",
            "\"external_request\":null",
        ),
        Damage::Downgrade => replace_once(
            original,
            "\"schema\":\"fa.publication-situation/2\"",
            "\"schema\":\"fa.publication-situation/1\"",
        ),
    }
}

#[test]
fn damaged_linked_second_page_is_never_emitted_or_followed_and_preserves_first_bytes() {
    for fault in [
        Damage::Authority,
        Damage::DuplicateRequest,
        Damage::UnpairedNull,
        Damage::Downgrade,
    ] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(33, 4);
            let mut socket = bounded_socket(&listener);
            let first_document = read_document(&mut socket);
            assert_eq!(
                decode_request(&first_document).unwrap(),
                linked_request(1, None)
            );
            let first = fixture.reply(&first_document);
            let cursor = linked_page(&first).continuation().unwrap().id();
            let first = first.to_json().unwrap();
            send(&mut socket, &first);
            let second_document = read_document(&mut socket);
            assert_eq!(
                decode_request(&second_document).unwrap(),
                linked_request(2, Some(cursor))
            );
            let second = fixture.reply(&second_document);
            assert!(linked_page(&second).continuation().is_some());
            send(&mut socket, &damage(&second.to_json().unwrap(), fault));
            disconnected(&mut socket, &listener);
            assert_eq!(
                fixture.remaining(),
                2,
                "damaged link triggered more native work"
            );
            first
        });
        let mut output = Vec::new();
        let error = command(&linked_args(&profile, 4), &mut output).unwrap_err();
        assert!(error.contains("invalid"), "{fault:?}: {error}");
        assert_eq!(
            output,
            (server.join().unwrap() + "\n").as_bytes(),
            "{fault:?}"
        );
        assert_eq!(emitted(&output).len(), 1);
    }
}

#[test]
fn genuine_unselected_response_profile_is_refused_before_any_output_or_continuation() {
    for requested_links in [false, true] {
        let root = Directory::new();
        let profile = root.settings(credentials().pid(), 4);
        let listener = UnixListener::bind(root.socket()).unwrap();
        let server = std::thread::spawn(move || {
            let mut fixture = Fixture::new(17, 2);
            let mut socket = bounded_socket(&listener);
            let document = read_document(&mut socket);
            let expected = if requested_links {
                linked_request(1, None)
            } else {
                request(1, None)
            };
            assert_eq!(decode_request(&document).unwrap(), expected);
            // Return a genuine original response to the opposite opt-in. Its
            // operation/address correlation is valid; its selected schema is not.
            let opposite = if requested_links {
                request(1, None)
            } else {
                linked_request(1, None)
            };
            let response = fixture.reply(encode_request(&opposite).unwrap().as_bytes());
            send(&mut socket, &response.to_json().unwrap());
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), 1);
        });
        let query = if requested_links {
            linked_args(&profile, 4)
        } else {
            args(&profile, 4)
        };
        let mut output = Vec::new();
        assert!(
            command(&query, &mut output)
                .unwrap_err()
                .contains("invalid")
        );
        assert!(output.is_empty());
        server.join().unwrap();
    }
}

#[test]
fn linked_page_exchange_and_native_query_caps_keep_partial_output_without_recapture() {
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
            assert_eq!(decode_request(&document).unwrap(), linked_request(1, None));
            let response = fixture.reply(&document);
            assert!(!linked_page(&response).enumeration_complete());
            assert_eq!(
                linked_page(&response).continuation().is_some(),
                cursor_expected
            );
            let original = response.to_json().unwrap();
            send(&mut socket, &original);
            disconnected(&mut socket, &listener);
            assert_eq!(fixture.remaining(), queries - 1);
            original
        });
        let mut output = Vec::new();
        assert!(
            command(&linked_args(&profile, max_pages), &mut output)
                .unwrap_err()
                .contains("incomplete")
        );
        assert_eq!(output, (server.join().unwrap() + "\n").as_bytes());
        assert_eq!(emitted(&output).len(), 1);
    }
}

#[test]
fn exactly_bounded_original_request_identities_leave_room_for_unlinked_direct_attempts() {
    let storage = native::Directory::new();
    let mut config = native::profile();
    config.delivery.max_attempts = MAX_ACTOR_REQUESTS + 1;
    let (mut host, _reviewer) = FileOversight::create(storage.store(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = native::proposal(&native::oversight::spec(&host, b"published"));
    let (actor, mut supervisor) = host.into_actor_gateway();
    for key in 1001..1001 + MAX_ACTOR_REQUESTS as u64 {
        native::observe(&mut supervisor);
        actor.submit(key, &proposal).unwrap();
    }
    {
        let mut host = supervisor.host_mut().unwrap();
        let direct = native::oversight::spec(&host, b"direct");
        let revision = host.revision();
        host.propose(
            revision,
            MAX_ACTOR_REQUESTS as u64 + 1,
            direct,
            native::snapshot(),
        )
        .unwrap();
    }
    let pages = (MAX_ACTOR_REQUESTS + 1).div_ceil(MAX_INSPECTION_ATTEMPTS);
    let (inspection, grant) = supervisor.inspection_port(pages).unwrap();
    let mut tower = PublicationTower::new(mount(), actor, PublicationLimits::default()).unwrap();
    tower.set_inspection(Some(inspection)).unwrap();
    let mut capture = linked_capture(pages);
    let mut cursor = None;
    for index in 1..=pages {
        let query = linked_request(index, cursor);
        let response = tower.exchange(&access(), encode_request(&query).unwrap().as_bytes());
        let page = linked_page(&response);
        let original = parse_response(response.to_json().unwrap().as_bytes(), &query).unwrap();
        if index == pages {
            assert_eq!(page.attempts().len(), 1);
            assert_eq!(page.attempts()[0].external_request, None);
            // This altered reply is deliberately NOT a native 129th request.
            // Its valid spelling cannot enlarge the retained original-key cap.
            let mut fabricated = original.clone();
            assign_link(&mut fabricated, 0, 9000);
            assert!(capture.accept(&fabricated).is_err());
            assert!(matches!(capture.accept(&original), Ok(Progress::Complete)));
        } else {
            cursor = page.continuation().map(|cursor| cursor.id());
            assert!(
                matches!(capture.accept(&original), Ok(Progress::Continue(next)) if Some(next) == cursor)
            );
        }
    }
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(
        supervisor.host().unwrap().retained_requests(),
        MAX_ACTOR_REQUESTS
    );
}

#[test]
fn final_request_links_flag_has_closed_grammar_before_any_profile_read() {
    let missing = Path::new("missing-request-links-profile");
    let valid = linked_args(missing, 4);
    let mut swapped = valid.clone();
    swapped.swap(3, 5);
    let mut valued = valid.clone();
    valued[5] = "--request-links=true".into();
    for invalid in [
        swapped,
        valued,
        [valid.clone(), vec!["--request-links".into()]].concat(),
        [valid.clone(), vec!["true".into()]].concat(),
        [
            valid.clone(),
            vec!["--operator-profile".into(), "unrelated-profile".into()],
        ]
        .concat(),
        [
            valid.clone(),
            vec!["--guarded".into(), "unrelated-recovery".into()],
        ]
        .concat(),
    ] {
        let mut output = Vec::new();
        assert_eq!(
            crate::command_with_output(invalid, &mut output).unwrap_err(),
            USAGE
        );
        assert!(output.is_empty());
    }
    for pages in [1, MAX_PAGES] {
        let mut output = Vec::new();
        assert_ne!(
            crate::command_with_output(linked_args(missing, pages), &mut output).unwrap_err(),
            USAGE
        );
        assert!(output.is_empty());
    }
}
