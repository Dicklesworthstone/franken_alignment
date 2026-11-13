//! Original native reports plus hostile transport/reply changes. These checks
//! establish client correlation, not independent proof of a server's ledger.
use super::*;
use fa_reference::action::consequence::oversight::actor_peer::PeerCredentials;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "tests/inspection.rs"]
mod inspection;

fn field<'a>(value: &'a mut Json, keys: &[&str]) -> &'a mut Json {
    if keys.is_empty() {
        return value;
    }
    let Json::Object(fields) = value else {
        panic!("object path required")
    };
    field(fields.get_mut(keys[0]).unwrap(), &keys[1..])
}
fn parsed(bytes: &[u8]) -> Json {
    strict_json::parse(bytes, Limits::default()).unwrap()
}
fn report_change(response: &[u8], change: impl FnOnce(&mut Json)) -> Vec<u8> {
    let mut reply = parsed(response);
    let slot = field(&mut reply, &["knowledge", "value", "reviewed_report"]);
    let mut report = parsed(slot.as_str().unwrap().as_bytes());
    change(&mut report);
    *slot = Json::String(protocol::json(&report));
    protocol::json(&reply).into_bytes()
}

#[test]
fn actual_complete_report_and_delegated_approval_validate_with_exact_scope_cut_candidate_and_limits()
 {
    let (review, report, approve, ack) = super::super::tests::original_documents();
    let request = protocol::decode(&review, protocol::MAX_REPORT_BYTES).unwrap();
    validate_response(&request, &report, protocol::MAX_REPORT_BYTES).unwrap();
    let approval = protocol::decode(&approve, protocol::MAX_REPORT_BYTES).unwrap();
    validate_response(&approval, &ack, protocol::MAX_REPORT_BYTES).unwrap();
    for path in [
        vec!["scope", "principal"],
        vec!["scope", "branch"],
        vec!["scope", "authority"],
        vec!["campaign"],
        vec!["knowledge", "value", "journal_revision"],
    ] {
        let mut rebound = parsed(&report);
        *field(&mut rebound, &path) = Json::String("999999".into());
        assert!(
            validate_response(
                &request,
                protocol::json(&rebound).as_bytes(),
                protocol::MAX_REPORT_BYTES
            )
            .is_err()
        );
    }
    for path in [
        vec!["knowledge", "value", "cut", "journal_revision"],
        vec!["knowledge", "value", "cut", "authority_epoch"],
        vec!["knowledge", "value", "candidate_policy", "generation"],
        vec!["knowledge", "value", "previous_policy", "generation"],
        vec!["knowledge", "value", "limits", "cases"],
        vec!["knowledge", "value", "case_count"],
    ] {
        let rebound = report_change(&report, |value| *field(value, &path) = parsed(b"999999"));
        assert!(validate_response(&request, &rebound, protocol::MAX_REPORT_BYTES).is_err());
    }
    let rebound = report_change(&report, |value| {
        let Json::Array(cases) = field(value, &["knowledge", "value", "cases"]) else {
            panic!("cases")
        };
        *field(&mut cases[0], &["action", "scope", "principal"]) = parsed(b"999999");
    });
    assert!(validate_response(&request, &rebound, protocol::MAX_REPORT_BYTES).is_err());
    for name in ["policy_changed", "effect_authority"] {
        let mut rebound = parsed(&ack);
        *field(&mut rebound, &["knowledge", "value", name]) = Json::Bool(true);
        assert!(
            validate_response(
                &approval,
                protocol::json(&rebound).as_bytes(),
                protocol::MAX_REPORT_BYTES
            )
            .is_err()
        );
    }
}

#[test]
fn original_acknowledgment_basis_is_required_and_command_errors_are_not_knowledge_variants() {
    let (review, report, approve, ack) = super::super::tests::original_documents();
    for (request_bytes, response) in [(review, report), (approve, ack)] {
        let request = protocol::decode(&request_bytes, protocol::MAX_REPORT_BYTES).unwrap();
        validate_response(&request, &response, protocol::MAX_REPORT_BYTES).unwrap();
        let original = parsed(&response);
        let knowledge = original.get("knowledge").unwrap();
        let basis = knowledge.get("basis").unwrap();
        assert_eq!(
            basis.get("origin").unwrap().as_str(),
            Some("acknowledged_owner")
        );
        assert_eq!(basis.get("scope"), original.get("scope"));
        assert_eq!(basis.get("campaign"), original.get("campaign"));
        assert_eq!(
            basis.get("journal_revision"),
            knowledge.get("value").unwrap().get("journal_revision")
        );
        assert!(original.get("error").is_none());

        for path in [
            vec!["knowledge", "basis", "origin"],
            vec!["knowledge", "basis", "scope", "tenant"],
            vec!["knowledge", "basis", "scope", "principal"],
            vec!["knowledge", "basis", "scope", "run"],
            vec!["knowledge", "basis", "scope", "branch"],
            vec!["knowledge", "basis", "scope", "authority"],
            vec!["knowledge", "basis", "scope", "purpose"],
            vec!["knowledge", "basis", "campaign"],
            vec!["knowledge", "basis", "journal_revision"],
        ] {
            let mut rebound = original.clone();
            *field(&mut rebound, &path) = Json::String("999999".into());
            assert!(
                validate_response(
                    &request,
                    protocol::json(&rebound).as_bytes(),
                    protocol::MAX_REPORT_BYTES
                )
                .is_err(),
                "changed acknowledgment basis accepted: {path:?}"
            );
        }
        let mut missing = original.clone();
        let Json::Object(fields) = field(&mut missing, &["knowledge"]) else {
            panic!("knowledge")
        };
        assert!(fields.remove("basis").is_some());
        assert!(
            validate_response(
                &request,
                protocol::json(&missing).as_bytes(),
                protocol::MAX_REPORT_BYTES
            )
            .is_err()
        );

        let mut invented = original.clone();
        *field(&mut invented, &["knowledge"]) =
            parsed(br#"{"state":"refused","reason":"stale_predecessor"}"#);
        assert!(
            validate_response(
                &request,
                protocol::json(&invented).as_bytes(),
                protocol::MAX_REPORT_BYTES
            )
            .is_err()
        );

        let error = protocol::refused(&request, "unconfirmed");
        validate_response(&request, &error, protocol::MAX_REPORT_BYTES).unwrap();
        let mut mixed = parsed(&error);
        assert!(mixed.get("knowledge").is_none());
        assert_eq!(
            mixed.get("error").unwrap().get("code").unwrap().as_str(),
            Some("unconfirmed")
        );
        let Json::Object(fields) = &mut mixed else {
            panic!("response")
        };
        fields.insert("knowledge".into(), knowledge.clone());
        assert!(
            validate_response(
                &request,
                protocol::json(&mixed).as_bytes(),
                protocol::MAX_REPORT_BYTES
            )
            .is_err()
        );
    }
}

#[test]
fn complete_native_report_refuses_omitted_dispositions_cases_basis_and_evaluations() {
    let (review, report, _, _) = super::super::tests::original_documents();
    let request = protocol::decode(&review, protocol::MAX_REPORT_BYTES).unwrap();
    validate_response(&request, &report, protocol::MAX_REPORT_BYTES).unwrap();
    for omitted in ["newly_reviewable", "newly_blocked"] {
        let incomplete = report_change(&report, |document| {
            let Json::Object(value) = field(document, &["knowledge", "value"]) else {
                panic!("value")
            };
            assert!(value.remove(omitted).is_some());
        });
        assert!(validate_response(&request, &incomplete, protocol::MAX_REPORT_BYTES).is_err());
    }
    let omitted_case = report_change(&report, |document| {
        let Json::Array(cases) = field(document, &["knowledge", "value", "cases"]) else {
            panic!("cases")
        };
        let scope = cases[0]
            .get("action")
            .unwrap()
            .get("scope")
            .unwrap()
            .clone();
        cases[0] = Json::Object(std::collections::BTreeMap::from([(
            "action".into(),
            Json::Object(std::collections::BTreeMap::from([("scope".into(), scope)])),
        )]));
    });
    assert!(validate_response(&request, &omitted_case, protocol::MAX_REPORT_BYTES).is_err());
    for removed in ["executions", "charged", "reserved"] {
        let incomplete = report_change(&report, |document| {
            let Json::Object(cut) = field(document, &["knowledge", "value", "cut"]) else {
                panic!("cut")
            };
            assert!(cut.remove(removed).is_some());
        });
        assert!(validate_response(&request, &incomplete, protocol::MAX_REPORT_BYTES).is_err());
    }
    for damaged in [
        report_change(&report, |document| {
            *field(document, &["knowledge", "basis", "scope"]) =
                Json::String("selected_cases".into());
        }),
        report_change(&report, |document| {
            *field(document, &["knowledge", "value", "cut", "charged"]) = Json::String("0".into());
        }),
        report_change(&report, |document| {
            let Json::Array(cases) = field(document, &["knowledge", "value", "cases"]) else {
                panic!("cases")
            };
            *field(&mut cases[0], &["original", "trace"]) = Json::Array(Vec::new());
        }),
        report_change(&report, |document| {
            let Json::Array(cases) = field(document, &["knowledge", "value", "cases"]) else {
                panic!("cases")
            };
            let Json::Object(candidate) = field(&mut cases[0], &["candidate"]) else {
                panic!("candidate")
            };
            assert!(candidate.remove("result").is_some());
        }),
        report_change(&report, |document| {
            let Json::Array(cases) = field(document, &["knowledge", "value", "cases"]) else {
                panic!("cases")
            };
            *field(&mut cases[0], &["action", "payload_hex"]) = Json::String("zz".into());
        }),
        report_change(&report, |document| {
            let Json::Array(blocked) = field(document, &["knowledge", "value", "newly_blocked"])
            else {
                panic!("blocked")
            };
            assert!(
                !blocked.is_empty(),
                "the actual candidate blocks the retained original proposal"
            );
            blocked.clear();
        }),
    ] {
        assert!(validate_response(&request, &damaged, protocol::MAX_REPORT_BYTES).is_err());
    }
}

#[test]
fn a_larger_connection_bound_cannot_widen_the_candidates_selected_report_output_bound() {
    let (review, report, _, _) = super::super::tests::original_documents();
    let original = protocol::decode(&review, protocol::MAX_REPORT_BYTES).unwrap();
    validate_response(&original, &report, protocol::MAX_REPORT_BYTES).unwrap();
    let original_reply = parsed(&report);
    let report_text = original_reply
        .get("knowledge")
        .unwrap()
        .get("value")
        .unwrap()
        .get("reviewed_report")
        .unwrap()
        .as_str()
        .unwrap();
    let selected = (report_text.len() / 2).max(1);
    let number = parsed(selected.to_string().as_bytes());
    let mut smaller_request = parsed(&review);
    *field(
        &mut smaller_request,
        &["candidate", "limits", "output_bytes"],
    ) = number.clone();
    let request = protocol::decode(
        protocol::json(&smaller_request).as_bytes(),
        protocol::MAX_REPORT_BYTES,
    )
    .unwrap();
    let oversized = report_change(&report, |document| {
        *field(document, &["knowledge", "value", "limits", "output_bytes"]) = number;
    });
    let reply = parsed(&oversized);
    let report_text = reply
        .get("knowledge")
        .unwrap()
        .get("value")
        .unwrap()
        .get("reviewed_report")
        .unwrap()
        .as_str()
        .unwrap();
    assert!(report_text.len() > selected && report_text.len() < protocol::MAX_REPORT_BYTES);
    assert!(validate_response(&request, &oversized, protocol::MAX_REPORT_BYTES).is_err());
}

#[test]
fn refusal_is_correlated_and_closed_and_no_reply_can_claim_approval_as_effect_authority() {
    let (_, _, approve, ack) = super::super::tests::original_documents();
    let request = protocol::decode(&approve, protocol::MAX_REPORT_BYTES).unwrap();
    let refused = protocol::refused(&request, "stale_predecessor");
    validate_response(&request, &refused, protocol::MAX_REPORT_BYTES).unwrap();
    for key in ["schema", "verb", "phase"] {
        let mut rebound = parsed(&refused);
        *field(&mut rebound, &[key]) = Json::String("unrelated".into());
        assert!(
            validate_response(
                &request,
                protocol::json(&rebound).as_bytes(),
                protocol::MAX_REPORT_BYTES
            )
            .is_err()
        );
    }
    let mut unknown = parsed(&refused);
    *field(&mut unknown, &["error", "code"]) = Json::String("retry_with_more_authority".into());
    assert!(
        validate_response(
            &request,
            protocol::json(&unknown).as_bytes(),
            protocol::MAX_REPORT_BYTES
        )
        .is_err()
    );
    let mut extra = parsed(&ack);
    let Json::Object(value) = field(&mut extra, &["knowledge", "value"]) else {
        panic!("value")
    };
    value.insert("permit".into(), Json::String("invented".into()));
    assert!(
        validate_response(
            &request,
            protocol::json(&extra).as_bytes(),
            protocol::MAX_REPORT_BYTES
        )
        .is_err()
    );
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = crate::test_directory::create(format!(
            "fa-gov-client-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("governor client cleanup: {error}");
        }
    }
}

fn files(root: &Directory, request: &[u8], server_pid: u32) -> (PathBuf, Vec<String>) {
    let (socket, _) = UnixStream::pair().unwrap();
    let peer = PeerCredentials::observe(&socket).unwrap();
    let scope = protocol::decode(request, protocol::MAX_REPORT_BYTES)
        .unwrap()
        .scope;
    let endpoint = root.0.join("governor.sock");
    let profile = root.0.join("profile.json");
    let input = root.0.join("request.json");
    fs::write(&profile, format!(concat!(
        "{{\"schema\":\"fa.policy-governor-service/1\",\"scope\":{{\"tenant\":{},\"principal\":{},\"run\":{},\"branch\":{},\"authority\":{}}},",
        "\"socket\":{},\"supervisor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},\"governor\":{{\"uid\":{},\"gid\":{},\"pid\":{}}},",
        "\"candidate_limit\":4,\"connection_limit\":4,\"exchange_limit\":8,\"runtime_ms\":2000,\"poll_ms\":1,",
        "\"campaigns\":{{\"cases\":16,\"input_bytes\":1048576,\"max_campaigns\":4}},\"campaign_limit\":2,\"report_bytes\":1048576}}"),
        scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
        protocol::quote(endpoint.to_str().unwrap()), peer.uid(), peer.gid(), server_pid,
        peer.uid(), peer.gid(), peer.pid(),
    )).unwrap();
    fs::write(&input, request).unwrap();
    (
        endpoint,
        vec![
            "policy-governor-exchange".into(),
            profile.to_str().unwrap().into(),
            input.to_str().unwrap().into(),
        ],
    )
}

#[test]
fn foreign_request_is_rejected_before_connect_and_wrong_server_receives_no_protocol_byte() {
    let (review, _, _, _) = super::super::tests::original_documents();
    let root = Directory::new();
    let (endpoint, args) = files(&root, &review, std::process::id());
    let listener = UnixListener::bind(&endpoint).unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut foreign = parsed(&review);
    *field(&mut foreign, &["scope", "authority"]) = Json::String("999999".into());
    fs::write(&args[2], protocol::json(&foreign)).unwrap();
    let mut output = Vec::new();
    assert!(
        command(&args, &mut output)
            .unwrap_err()
            .contains("audience")
    );
    assert!(output.is_empty());
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
    drop(listener);
    fs::remove_file(&endpoint).unwrap();
    let (endpoint, args) = files(&root, &review, std::process::id().checked_add(1).unwrap());
    let listener = UnixListener::bind(endpoint).unwrap();
    let peer = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut byte = [0];
        assert_eq!(socket.read(&mut byte).unwrap(), 0);
    });
    assert!(
        command(&args, &mut output)
            .unwrap_err()
            .contains("credentials")
    );
    assert!(output.is_empty());
    peer.join().unwrap();
}

#[test]
fn actual_native_reply_is_emitted_only_after_validation_and_malformed_reply_never_causes_a_resend()
{
    let (review, report, _, _) = super::super::tests::original_documents();
    for valid in [true, false] {
        let root = Directory::new();
        let (endpoint, args) = files(&root, &review, std::process::id());
        let listener = UnixListener::bind(endpoint).unwrap();
        let response = if valid {
            report.clone()
        } else {
            let mut reply = parsed(&report);
            *field(&mut reply, &["campaign"]) = Json::String("999999".into());
            protocol::json(&reply).into_bytes()
        };
        let expected = protocol::decode(&review, protocol::MAX_REPORT_BYTES)
            .unwrap()
            .canonical;
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut header = [0; 4];
            socket.read_exact(&mut header).unwrap();
            let size = u32::from_be_bytes(header) as usize;
            assert!(size <= protocol::MAX_REQUEST_BYTES);
            let mut request = vec![0; size];
            socket.read_exact(&mut request).unwrap();
            assert_eq!(request, expected);
            socket
                .write_all(&(response.len() as u32).to_be_bytes())
                .unwrap();
            socket.write_all(&response).unwrap();
            let mut byte = [0];
            assert_eq!(socket.read(&mut byte).unwrap(), 0);
            listener.set_nonblocking(true).unwrap();
            assert_eq!(
                listener.accept().unwrap_err().kind(),
                io::ErrorKind::WouldBlock
            );
        });
        let mut output = Vec::new();
        let result = command(&args, &mut output);
        if valid {
            result.unwrap();
            let mut expected = report.clone();
            expected.push(b'\n');
            assert_eq!(output, expected);
        } else {
            assert!(result.is_err());
            assert!(output.is_empty());
        }
        server.join().unwrap();
    }
}
