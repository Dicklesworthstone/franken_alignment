//! Each adversarial continuation below remains valid as an isolated page.
//! Inventory validation must additionally preserve the first page's domain,
//! cardinality and revision laws before exposing the changed successor.
use super::*;
use protocol::{
    cases::{self, CasesRequest},
    pending::{self, PendingRequest},
};
use std::os::unix::net::UnixListener;

fn replace(wire: &[u8], from: &str, to: &str) -> Vec<u8> {
    let text = std::str::from_utf8(wire).unwrap();
    assert_eq!(text.matches(from).count(), 1, "ambiguous inventory mutation");
    text.replacen(from, to, 1).into_bytes()
}

fn exchange_frames(
    fixture: &Fixture,
    name: &str,
    selection: &[&str],
    responses: Vec<Vec<u8>>,
) -> (Result<(), String>, Vec<u8>, Vec<Vec<u8>>) {
    let socket = fixture.root.0.join(format!("{name}.sock"));
    let profile = fixture.root.0.join(format!("{name}.json"));
    fs::write(
        &profile,
        profile_json(&socket, &fixture.actor, fixture.actor.supervisor, 8),
    )
    .unwrap();
    let listener = UnixListener::bind(&socket).unwrap();
    let identity = fixture.actor.supervisor;
    let peer = thread::spawn(move || {
        responses
            .into_iter()
            .map(|response| {
                super::super::case_client::peer(
                    listener.try_clone().unwrap(),
                    identity,
                    Some(response),
                )
                .join()
                .unwrap()
                .unwrap()
            })
            .collect::<Vec<_>>()
    });
    let mut output = Vec::new();
    let result = client::command(&args(&profile, selection), &mut output);
    (result, output, peer.join().unwrap())
}

fn pending_pair(fixture: &Fixture) -> (PendingRequest, Vec<u8>, PendingRequest, Vec<u8>) {
    let host = fixture.driver.supervisor().host().unwrap();
    let evaluator = fixture.evaluator.as_ref().unwrap();
    let first = PendingRequest::prepare(&fixture.settings, None).unwrap();
    let first_response = first.observe(&host, evaluator).unwrap();
    let parsed = pending::parse_response(&first_response).unwrap();
    let knowledge = parsed.get("knowledge").unwrap();
    let revision = number(knowledge.get("basis").unwrap(), "evaluation_revision");
    let after = number(knowledge.get("value").unwrap(), "next_after_round");
    let second = PendingRequest::prepare(&fixture.settings, Some((revision, after))).unwrap();
    let second_response = second.observe(&host, evaluator).unwrap();
    (first, first_response, second, second_response)
}

#[test]
fn pending_walk_rejects_individually_valid_changed_totals_truncation_and_basis_rollback() {
    let mut fixture = Fixture::with_case_capacity(32);
    for id in 1..=17 {
        fixture.review(id);
    }
    {
        let mut host = fixture.driver.supervisor_mut().host_mut().unwrap();
        let revision = host.revision();
        host.fence(revision).unwrap();
    }
    let before = fixture.image();
    let (first, first_response, second, response) = pending_pair(&fixture);
    let parsed = pending::parse_response(&response).unwrap();
    let basis = parsed.get("knowledge").unwrap().get("basis").unwrap();
    let mut mutations = vec![replace(
        &response,
        "\"total_pending\":\"17\"",
        "\"total_pending\":\"18\"",
    )];
    for field in ["journal_revision", "control_sequence", "authority_epoch"] {
        let original = number(basis, field);
        assert!(original > 0);
        mutations.push(replace(
            &response,
            &format!("\"{field}\":\"{original}\""),
            &format!("\"{field}\":\"{}\"", original - 1),
        ));
    }
    let text = std::str::from_utf8(&response).unwrap();
    let begin = text.find("\"cases\":[").unwrap() + "\"cases\":[".len();
    let end = begin + text[begin..].find("],\"next_after_round\"").unwrap();
    mutations.push(replace(&response, &text[begin..end], ""));
    for (index, mutated) in mutations.into_iter().enumerate() {
        assert!(
            pending::validate(&second, &mutated).unwrap(),
            "negative must reach the cross-page contract: {index}"
        );
        let (result, output, received) = exchange_frames(
            &fixture,
            &format!("pending-cross-page-{index}"),
            &["--pending", "--max-pages", "3"],
            vec![first_response.clone(), mutated],
        );
        assert!(result.unwrap_err().contains("incomplete"));
        let mut expected = first_response.clone();
        expected.push(b'\n');
        assert_eq!(output, expected, "disclosed contradictory page {index}");
        assert_eq!(received, vec![first.wire.clone(), second.wire.clone()]);
        assert_eq!(fixture.image(), before);
    }
    // The near-identical native continuation is accepted after all mutations.
    let (result, output, received) = exchange_frames(
        &fixture,
        "pending-cross-page-original",
        &["--pending", "--max-pages", "2"],
        vec![first_response, response],
    );
    result.unwrap();
    assert_eq!(rounds(&pages(&output)), (101..=117).collect::<Vec<_>>());
    assert_eq!(received, vec![first.wire, second.wire]);
    assert_read_only(&fixture, &before, 17);
}

#[test]
fn all_case_walk_rejects_changed_totals_or_unapplied_inventory_before_output() {
    let mut fixture = Fixture::new();
    for id in 1..=3 {
        fixture.review(id);
    }
    let before = fixture.image();
    let first = CasesRequest::prepare(&fixture.settings, 2, None, None).unwrap();
    let first_response = first
        .observe(
            &fixture.driver.supervisor().host().unwrap(),
            fixture.evaluator.as_ref().unwrap(),
        )
        .unwrap();
    let basis = super::super::cases_service::page_basis(&first_response);
    let second = CasesRequest::prepare(&fixture.settings, 2, Some(102), Some(basis)).unwrap();
    let response = second
        .observe(
            &fixture.driver.supervisor().host().unwrap(),
            fixture.evaluator.as_ref().unwrap(),
        )
        .unwrap();
    for (index, (from, to)) in [
        ("\"total_cases\":\"3\"", "\"total_cases\":\"2\""),
        ("\"unapplied_rounds\":\"0\"", "\"unapplied_rounds\":\"1\""),
    ]
    .into_iter()
    .enumerate()
    {
        let mutated = replace(&response, from, to);
        assert!(cases::validate(&second, &mutated).unwrap());
        let (result, output, received) = exchange_frames(
            &fixture,
            &format!("all-cross-page-{index}"),
            &["--list", "2", "--max-pages", "3"],
            vec![first_response.clone(), mutated],
        );
        assert!(result.unwrap_err().contains("incomplete"));
        assert_eq!(rounds(&pages(&output)), vec![101, 102]);
        assert_eq!(received, vec![first.wire.clone(), second.wire.clone()]);
        assert_eq!(fixture.image(), before);
    }
    let (result, output, received) = exchange_frames(
        &fixture,
        "all-cross-page-original",
        &["--list", "2", "--max-pages", "2"],
        vec![first_response, response],
    );
    result.unwrap();
    assert_eq!(rounds(&pages(&output)), vec![101, 102, 103]);
    assert_eq!(received, vec![first.wire, second.wire]);
    assert_read_only(&fixture, &before, 3);
}

#[test]
fn all_case_walk_refuses_a_locally_valid_suffix_that_skips_an_original_case() {
    let mut fixture = Fixture::new();
    for id in 1..=3 {
        fixture.review(id);
    }
    let before = fixture.image();
    let first = CasesRequest::prepare(&fixture.settings, 1, None, None).unwrap();
    let first_response = first
        .observe(
            &fixture.driver.supervisor().host().unwrap(),
            fixture.evaluator.as_ref().unwrap(),
        )
        .unwrap();
    let basis = super::super::cases_service::page_basis(&first_response);
    let second = CasesRequest::prepare(&fixture.settings, 1, Some(101), Some(basis)).unwrap();
    let third = CasesRequest::prepare(&fixture.settings, 1, Some(102), Some(basis)).unwrap();
    let third_response = third
        .observe(
            &fixture.driver.supervisor().host().unwrap(),
            fixture.evaluator.as_ref().unwrap(),
        )
        .unwrap();
    let skipped = replace(
        &third_response,
        std::str::from_utf8(&third.wire).unwrap(),
        std::str::from_utf8(&second.wire).unwrap(),
    );
    assert!(cases::validate(&second, &skipped).unwrap());
    let (result, output, received) = exchange_frames(
        &fixture,
        "all-skipped-case",
        &["--list", "1", "--max-pages", "3"],
        vec![first_response, skipped],
    );
    assert!(result.unwrap_err().contains("incomplete"));
    assert_eq!(rounds(&pages(&output)), vec![101]);
    assert_eq!(received, vec![first.wire, second.wire]);
    assert_read_only(&fixture, &before, 3);
}
