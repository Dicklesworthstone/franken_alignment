//! Public Tower continuations retain one original native capture and allowance.
//! These checks exercise reference inspection, not production qualification.

use super::*;
use fa_reference::action::consequence::oversight::actor::ActorProposal;

const FIRST_REQUEST: u64 = 1001;

fn populated(root: &Directory, attempts: usize) -> (Port, support::Supervisor, ActorProposal) {
    let mut config = profile();
    config.delivery.max_attempts = attempts + 1;
    let (mut host, _reviewer) = FileOversight::create(root.store(), config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let proposal = support::proposal(&oversight::spec(&host, b"published"));
    let (actor, mut supervisor) = host.into_actor_gateway();
    for offset in 0..attempts {
        observe(&mut supervisor);
        let key = FIRST_REQUEST + offset as u64;
        let ticket = actor.submit(key, &proposal).unwrap();
        assert_eq!(
            actor.poll(&ticket),
            ActorKnowledge::Pending { request: key }
        );
    }
    (actor, supervisor, proposal)
}

fn page_request(adapter: &PublicationTower, cursor: u64) -> TowerRequest {
    let mut call = request(adapter);
    call.operation_id = format!("inspect-continuation-{cursor}");
    call.arguments = ReadArguments::SituationPage { cursor };
    call
}

fn continue_bytes(adapter: &mut PublicationTower, cursor: u64) -> PublicationResponse {
    let call = page_request(adapter, cursor);
    let encoded = encode_request(&call).unwrap();
    let response = adapter.exchange(&access(true), encoded.as_bytes());
    assert_eq!(response.envelope.operation_id, call.operation_id);
    response
}

fn cursor(response: &PublicationResponse) -> u64 {
    known(response).continuation().unwrap().id()
}

fn conflict(response: &PublicationResponse) {
    assert_eq!(native(response), &Err(ActorError::IdempotencyConflict));
    assert_eq!(
        response.envelope.reason_code,
        Some(ReasonCode::IntegrityFailure)
    );
    assert!(
        response
            .to_json()
            .unwrap()
            .contains("\"code\":\"idempotency_conflict\"")
    );
    no_global_claims(response);
}

fn check_wire_page(response: &PublicationResponse) {
    let page = known(response);
    let encoded = response.to_json().unwrap();
    assert!(encoded.len() <= PublicationLimits::default().max_response_bytes);
    let parsed = strict_json::parse(encoded.as_bytes(), KNOWLEDGE_JSON_LIMITS).unwrap();
    let result = parsed.as_object().unwrap()["result"].as_object().unwrap();
    assert_eq!(result["kind"].as_str(), Some("situation"));
    let situation = result["value"].as_object().unwrap();
    assert_eq!(
        situation["schema"].as_str(),
        Some("fa.publication-situation/1")
    );
    assert_eq!(
        situation["current_eligibility"].as_str(),
        Some("not_observed")
    );
    let knowledge = situation["knowledge"].as_object().unwrap();
    let basis = knowledge["basis"].as_object().unwrap();
    assert_eq!(
        basis["journal_revision"].as_str(),
        Some(page.basis().journal_revision.to_string().as_str())
    );
    let value = knowledge["value"].as_object().unwrap();
    assert_eq!(
        value["offset"].as_str(),
        Some(page.offset().to_string().as_str())
    );
    assert_eq!(
        value["enumeration_complete"].as_bool(),
        Some(page.enumeration_complete())
    );
    let attempts = value["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), page.attempts().len());
    for (encoded, row) in attempts.iter().zip(page.attempts()) {
        assert_eq!(
            encoded.as_object().unwrap()["attempt"].as_str(),
            Some(row.attempt.to_string().as_str())
        );
    }
    match page.continuation() {
        Some(next) => {
            assert_ne!(next.id(), 0);
            assert_eq!(
                value["continuation"].as_str(),
                Some(next.id().to_string().as_str())
            );
        }
        None => assert!(value["continuation"].is_null()),
    }
}

#[test]
fn byte_continuations_enumerate_one_original_cut_while_the_live_owner_changes() {
    let root = Directory::new();
    let count = MAX_INSPECTION_ATTEMPTS * 2 + 1;
    let (actor, mut supervisor, proposal) = populated(&root, count);
    let (port, grant) = supervisor.inspection_port(7).unwrap();
    let mut adapter = tower(actor.clone());
    adapter.set_inspection(Some(port)).unwrap();
    let initial_image = files(&root);
    let first = inspect(&mut adapter);
    let basis = known(&first).basis();
    let summary = known(&first).summary();
    assert_eq!(summary.attempts, count);
    assert_eq!(summary.pending_attempts, count);
    assert_eq!(files(&root), initial_image);

    // Move an attempt from the second page to the terminal group and append
    // another real request. Re-capturing and applying an offset would now skip
    // or duplicate rows and report a different population and accounting cut.
    let changed_attempt = MAX_INSPECTION_ATTEMPTS as u64 + 2;
    let ticket = actor
        .submit(FIRST_REQUEST + changed_attempt - 1, &proposal)
        .unwrap();
    actor.cancel(&ticket).unwrap();
    observe(&mut supervisor);
    actor
        .submit(FIRST_REQUEST + count as u64, &proposal)
        .unwrap();
    let current = supervisor.host().unwrap().inspect();
    assert!(current.revision > basis.journal_revision);
    let current_image = files(&root);

    let second = continue_bytes(&mut adapter, cursor(&first));
    let third = continue_bytes(&mut adapter, cursor(&second));
    let pages = [&first, &second, &third];
    for (index, response) in pages.iter().enumerate() {
        let page = known(response);
        assert_eq!(page.basis(), basis);
        assert_eq!(page.summary(), summary);
        assert_eq!(page.offset(), index * MAX_INSPECTION_ATTEMPTS);
        assert!(
            page.attempts()
                .iter()
                .all(|row| row.stage == ActionState::Reviewing)
        );
        check_wire_page(response);
    }
    let attempts = pages
        .iter()
        .flat_map(|response| known(response).attempts())
        .map(|row| row.attempt)
        .collect::<Vec<_>>();
    assert_eq!(attempts, (1..=count as u64).collect::<Vec<_>>());
    assert_eq!(known(&third).attempts().len(), 1);
    assert!(known(&third).enumeration_complete());
    assert!(known(&third).continuation().is_none());
    assert_ne!(cursor(&first), cursor(&second));
    assert_eq!(grant.remaining_queries(), 4);
    assert_eq!(files(&root), current_image);

    let fresh = inspect(&mut adapter);
    assert_eq!(known(&fresh).basis().journal_revision, current.revision);
    assert_eq!(known(&fresh).summary().attempts, count + 1);
    assert_eq!(known(&fresh).summary().pending_attempts, count);
    assert_eq!(known(&fresh).summary().terminal_attempts, 1);
    let fresh_second = continue_bytes(&mut adapter, cursor(&fresh));
    let fresh_third = continue_bytes(&mut adapter, cursor(&fresh_second));
    let fresh_rows = [&fresh, &fresh_second, &fresh_third]
        .into_iter()
        .flat_map(|response| known(response).attempts())
        .copied()
        .collect::<Vec<_>>();
    let mut expected = (1..=count as u64 + 1)
        .filter(|attempt| *attempt != changed_attempt)
        .collect::<Vec<_>>();
    expected.push(changed_attempt);
    assert_eq!(
        fresh_rows.iter().map(|row| row.attempt).collect::<Vec<_>>(),
        expected
    );
    assert_eq!(fresh_rows.last().unwrap().stage, ActionState::Cancelled);
    for response in [&fresh, &fresh_second, &fresh_third] {
        assert_eq!(known(response).basis(), known(&fresh).basis());
        assert_eq!(known(response).summary(), known(&fresh).summary());
        check_wire_page(response);
    }
    assert!(known(&fresh_third).enumeration_complete());
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(files(&root), current_image);
}

#[test]
fn malformed_stale_and_unissued_scalars_preserve_the_original_pending_page() {
    let root = Directory::new();
    let (actor, supervisor, _proposal) = populated(&root, MAX_INSPECTION_ATTEMPTS + 1);
    let (port, grant) = supervisor.inspection_port(3).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(port)).unwrap();
    let image = files(&root);
    let first = inspect(&mut adapter);
    let next = cursor(&first);
    let zero = page_request(&adapter, 0);
    assert!(encode_request(&zero).is_err());
    refused(
        &adapter.dispatch(&access(true), zero),
        ReasonCode::IntegrityFailure,
    );
    conflict(&continue_bytes(&mut adapter, u64::MAX));
    let mut stale = page_request(&adapter, next);
    stale.address.generation = Some(REGISTRATION - 1);
    refused(
        &adapter.dispatch(&access(true), stale),
        ReasonCode::StaleGeneration,
    );
    let unprivileged = page_request(&adapter, next);
    let denied = adapter.dispatch(&access(false), unprivileged);
    assert!(matches!(
        denied.result,
        PublicationResult::Refused(Knowledge::Withheld { .. })
    ));
    assert_eq!(grant.remaining_queries(), 2);
    let last = continue_bytes(&mut adapter, next);
    assert_eq!(known(&last).offset(), MAX_INSPECTION_ATTEMPTS);
    assert!(known(&last).enumeration_complete());
    check_wire_page(&last);
    assert_eq!(grant.remaining_queries(), 1);
    assert_eq!(files(&root), image);
}

#[test]
fn new_captures_and_successful_pages_retire_old_cursors_without_retiring_the_current_one() {
    let root = Directory::new();
    let (actor, supervisor, _proposal) = populated(&root, MAX_INSPECTION_ATTEMPTS * 2 + 1);
    let (port, grant) = supervisor.inspection_port(6).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(port)).unwrap();
    let image = files(&root);
    let old = inspect(&mut adapter);
    let fresh = inspect(&mut adapter);
    assert_ne!(cursor(&old), cursor(&fresh));
    conflict(&continue_bytes(&mut adapter, cursor(&old)));
    assert_eq!(grant.remaining_queries(), 4);
    let second = continue_bytes(&mut adapter, cursor(&fresh));
    conflict(&continue_bytes(&mut adapter, cursor(&fresh)));
    assert_eq!(grant.remaining_queries(), 3);
    let last = continue_bytes(&mut adapter, cursor(&second));
    assert!(known(&last).enumeration_complete());
    conflict(&continue_bytes(&mut adapter, cursor(&second)));
    assert_eq!(grant.remaining_queries(), 2);
    check_wire_page(&last);
    assert_eq!(files(&root), image);
}

#[test]
fn facade_and_grant_replacements_cannot_rebind_an_observed_scalar_to_another_capture() {
    let root = Directory::new();
    let (actor, supervisor, _proposal) = populated(&root, MAX_INSPECTION_ATTEMPTS * 2 + 1);
    let (port, grant) = supervisor.inspection_port(6).unwrap();
    let mut first = tower(actor.clone());
    let mut second = tower(actor);
    first.set_inspection(Some(port.clone())).unwrap();
    second.set_inspection(Some(port.clone())).unwrap();
    let image = files(&root);
    let original = inspect(&mut first);
    conflict(&continue_bytes(&mut second, cursor(&original)));
    assert_eq!(grant.remaining_queries(), 5);
    let original_second = continue_bytes(&mut first, cursor(&original));
    let replacement = inspect(&mut second);
    conflict(&continue_bytes(&mut first, cursor(&original_second)));
    assert_eq!(grant.remaining_queries(), 3);
    let replacement_second = continue_bytes(&mut second, cursor(&replacement));
    first.set_inspection(Some(port)).unwrap();
    conflict(&continue_bytes(&mut first, cursor(&replacement_second)));
    assert_eq!(grant.remaining_queries(), 2);
    let complete = continue_bytes(&mut second, cursor(&replacement_second));
    assert!(known(&complete).enumeration_complete());
    assert_eq!(grant.remaining_queries(), 1);

    let (new_port, new_grant) = supervisor.inspection_port(3).unwrap();
    second.set_inspection(Some(new_port)).unwrap();
    let new_capture = inspect(&mut second);
    for prior in [
        &original,
        &original_second,
        &replacement,
        &replacement_second,
    ] {
        assert_ne!(cursor(prior), cursor(&new_capture));
        conflict(&continue_bytes(&mut second, cursor(prior)));
    }
    assert_eq!(new_grant.remaining_queries(), 2);
    assert_eq!(grant.remaining_queries(), 1);
    let new_second = continue_bytes(&mut second, cursor(&new_capture));
    let new_last = continue_bytes(&mut second, cursor(&new_second));
    assert!(known(&new_last).enumeration_complete());
    assert_eq!(new_grant.remaining_queries(), 0);
    check_wire_page(&new_last);
    assert_eq!(files(&root), image);
}

#[test]
fn native_clone_progress_spends_the_same_allowance_and_invalidates_the_facade_cursor() {
    let root = Directory::new();
    let (actor, supervisor, _proposal) = populated(&root, MAX_INSPECTION_ATTEMPTS * 2 + 1);
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(port.clone())).unwrap();
    let image = files(&root);
    let first = inspect(&mut adapter);
    let original = known(&first).continuation().unwrap().clone();
    let clone = port.clone();
    let ActorKnowledge::Known {
        value: second,
        basis,
    } = clone.situation_page(Some(&original)).unwrap()
    else {
        panic!("clone did not preserve the native capture")
    };
    assert_eq!(basis, known(&first).basis());
    assert_eq!(second.offset(), MAX_INSPECTION_ATTEMPTS);
    assert_eq!(grant.remaining_queries(), 2);
    conflict(&continue_bytes(&mut adapter, original.id()));
    assert_eq!(grant.remaining_queries(), 2);
    let ActorKnowledge::Known { value: last, basis } =
        port.situation_page(second.continuation()).unwrap()
    else {
        panic!("native continuation did not finish its original capture")
    };
    assert_eq!(basis, known(&first).basis());
    assert_eq!(last.offset(), MAX_INSPECTION_ATTEMPTS * 2);
    assert!(last.enumeration_complete());
    assert_eq!(grant.remaining_queries(), 1);
    let exhausted_page = inspect(&mut adapter);
    assert!(!known(&exhausted_page).enumeration_complete());
    assert!(known(&exhausted_page).continuation().is_none());
    check_wire_page(&exhausted_page);
    assert_eq!(clone.situation(), Err(ActorError::Capacity));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(files(&root), image);
}

#[test]
fn revoked_or_dropped_custody_withholds_a_previously_issued_continuation() {
    for explicit_revoke in [false, true] {
        let root = Directory::new();
        let (actor, supervisor, _proposal) = populated(&root, MAX_INSPECTION_ATTEMPTS + 1);
        let (port, grant) = supervisor.inspection_port(2).unwrap();
        let mut adapter = tower(actor);
        adapter.set_inspection(Some(port)).unwrap();
        let first = inspect(&mut adapter);
        let next = cursor(&first);
        assert_eq!(grant.remaining_queries(), 1);
        let image = files(&root);
        if explicit_revoke {
            grant.revoke();
        } else {
            drop(grant);
        }
        let revoked = continue_bytes(&mut adapter, next);
        assert_eq!(
            native(&revoked),
            &Ok(ActorKnowledge::Withheld {
                authority_required: "supervisor_inspection",
            })
        );
        assert_eq!(
            revoked.envelope.reason_code,
            Some(ReasonCode::DeniedByPolicy)
        );
        assert!(
            revoked
                .to_json()
                .unwrap()
                .contains("\"authority_required\":\"supervisor_inspection\"")
        );
        assert_eq!(native(&inspect(&mut adapter)), native(&revoked));
        assert_eq!(files(&root), image);
    }
}

#[test]
fn borrowed_and_faulted_owners_invalidate_retained_pages_without_spending_more_work() {
    let root = Directory::new();
    let (actor, mut supervisor, _proposal) = populated(&root, MAX_INSPECTION_ATTEMPTS + 1);
    let (port, grant) = supervisor.inspection_port(4).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(port)).unwrap();
    let first = inspect(&mut adapter);
    {
        let _borrow = supervisor.host_mut().unwrap();
        unknown(&continue_bytes(&mut adapter, cursor(&first)));
        assert_eq!(grant.remaining_queries(), 3);
    }
    conflict(&continue_bytes(&mut adapter, cursor(&first)));
    let recovered = inspect(&mut adapter);
    assert_eq!(grant.remaining_queries(), 2);
    {
        let mut host = supervisor.host_mut().unwrap();
        std::fs::write(root.store().join("delivery.pending"), b"occupied stage").unwrap();
        let revision = host.revision();
        assert!(host.observe_time(revision, ElapsedTick(2)).is_err());
        assert!(host.storage_failure().is_some());
    }
    let image = files(&root);
    unknown(&continue_bytes(&mut adapter, cursor(&recovered)));
    unknown(&inspect(&mut adapter));
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(files(&root), image);
}

#[test]
fn owner_loss_and_reopening_do_not_revive_old_continuations() {
    let root = Directory::new();
    let count = MAX_INSPECTION_ATTEMPTS + 1;
    let (actor, supervisor, _proposal) = populated(&root, count);
    let (port, grant) = supervisor.inspection_port(3).unwrap();
    let mut adapter = tower(actor);
    adapter.set_inspection(Some(port.clone())).unwrap();
    let first = inspect(&mut adapter);
    drop(supervisor);
    let image = files(&root);
    unknown(&continue_bytes(&mut adapter, cursor(&first)));
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(files(&root), image);

    let mut config = profile();
    config.delivery.max_attempts = count + 1;
    let (host, _reviewer) = FileOversight::open(root.store(), config).unwrap();
    // Native reopening durably fences the old owner; only that existing
    // transition may change history, never the subsequent situation reads.
    let reopened_image = files(&root);
    assert_ne!(reopened_image, image);
    let (actor, new_supervisor) = host.into_actor_gateway();
    let mut reopened = tower(actor);
    assert!(reopened.set_inspection(Some(port)).is_err());
    let (new_port, new_grant) = new_supervisor.inspection_port(2).unwrap();
    reopened.set_inspection(Some(new_port)).unwrap();
    let fresh = inspect(&mut reopened);
    assert_ne!(cursor(&first), cursor(&fresh));
    conflict(&continue_bytes(&mut reopened, cursor(&first)));
    assert_eq!(new_grant.remaining_queries(), 1);
    let last = continue_bytes(&mut reopened, cursor(&fresh));
    assert!(known(&last).enumeration_complete());
    check_wire_page(&last);
    unknown(&inspect(&mut adapter));
    assert_eq!(grant.remaining_queries(), 2);
    assert_eq!(files(&root), reopened_image);
}

#[test]
fn final_query_reports_incomplete_enumeration_without_an_unusable_cursor_or_budget_renewal() {
    let root = Directory::new();
    let count = MAX_INSPECTION_ATTEMPTS * 2 + 1;
    let (actor, supervisor, _proposal) = populated(&root, count);
    let (port, grant) = supervisor.inspection_port(2).unwrap();
    let mut first = tower(actor.clone());
    first.set_inspection(Some(port.clone())).unwrap();
    let image = files(&root);
    let page = inspect(&mut first);
    let final_page = continue_bytes(&mut first, cursor(&page));
    assert_eq!(known(&final_page).offset(), MAX_INSPECTION_ATTEMPTS);
    assert_eq!(known(&final_page).attempts().len(), MAX_INSPECTION_ATTEMPTS);
    assert_eq!(known(&final_page).summary().attempts, count);
    assert!(!known(&final_page).enumeration_complete());
    assert!(known(&final_page).continuation().is_none());
    check_wire_page(&final_page);
    assert_eq!(grant.remaining_queries(), 0);
    conflict(&continue_bytes(&mut first, cursor(&page)));
    let mut second = tower(actor);
    second.set_inspection(Some(port.clone())).unwrap();
    for response in [inspect(&mut first), inspect(&mut second)] {
        assert_eq!(native(&response), &Err(ActorError::Capacity));
        assert_eq!(
            response.envelope.reason_code,
            Some(ReasonCode::BudgetExhausted)
        );
    }
    assert_eq!(port.situation(), Err(ActorError::Capacity));
    assert_eq!(grant.remaining_queries(), 0);
    assert_eq!(files(&root), image);
}

#[test]
fn empty_and_exactly_full_pages_are_complete_without_a_continuation() {
    for count in [0, MAX_INSPECTION_ATTEMPTS] {
        let root = Directory::new();
        let (actor, supervisor, _proposal) = populated(&root, count);
        let (port, grant) = supervisor.inspection_port(2).unwrap();
        let mut adapter = tower(actor);
        adapter.set_inspection(Some(port)).unwrap();
        let image = files(&root);
        let complete = inspect(&mut adapter);
        assert_eq!(known(&complete).attempts().len(), count);
        assert_eq!(known(&complete).summary().attempts, count);
        assert_eq!(known(&complete).offset(), 0);
        assert!(known(&complete).enumeration_complete());
        assert!(known(&complete).continuation().is_none());
        check_wire_page(&complete);
        conflict(&continue_bytes(&mut adapter, u64::MAX));
        assert_eq!(grant.remaining_queries(), 1);
        check_wire_page(&inspect(&mut adapter));
        assert_eq!(grant.remaining_queries(), 0);
        assert_eq!(files(&root), image);
    }
}
