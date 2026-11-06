//! The actual source-only supervisor handoff behind the Tower's admission gates.
//! Source capture, worker review, independent human approval and publication
//! remain in their original owners; no supplied snapshot replaces this path.
#![cfg(unix)]

#[path = "support/file_source_intake.rs"]
mod fixture;

use fa_reference::Error;
use fa_reference::action::consequence::delivery::EndpointOutcome;
use fa_reference::action::consequence::delivery::persistent::observed::driver::FileDriverEvent;
use fa_reference::action::consequence::delivery::persistent::{JournalError, Reconciliation};
use fa_reference::action::consequence::oversight::actor::{
    ActorError, ActorOutcome, Knowledge, UnknownReason,
};
use fa_reference::action::consequence::oversight::actor_wire::{WireError, WireResponse};
use fa_reference::action::consequence::oversight::policy_state::StateLimits;
use fa_reference::action::{ElapsedTick, MAX_PAYLOAD_BYTES};
use fa_reference::address_resolver::{Address, CallerScope, Noun};
use fa_reference::tower::publication::{
    PublicationLimits, PublicationResponse, PublicationResult, PublicationTower,
};
use fa_reference::tower::{
    ReadArguments, ReasonCode, TowerAccess, TowerRequest, Verb, encode_request,
};
use fa_reference::typestate_cx::PurposeTag;
use fixture::{FileRig, cold, document, driver, replace};

fn tower(file: &FileRig) -> PublicationTower {
    let scope = driver::profile().delivery.scope;
    PublicationTower::new(
        Address::new(
            scope.tenant.to_string(),
            Noun::Run,
            scope.run.to_string(),
            Some(41),
        )
        .unwrap(),
        file.rig.port.clone(),
        PublicationLimits::default(),
    )
    .unwrap()
}

fn access() -> TowerAccess {
    TowerAccess::new(
        CallerScope::full_tenant_access("1"),
        false,
        vec![PurposeTag::Permit],
        0,
    )
}

fn reader() -> TowerAccess {
    TowerAccess::new(CallerScope::full_tenant_access("1"), false, Vec::new(), 0)
}

fn submit(file: &FileRig, tower: &PublicationTower, key: u64) -> TowerRequest {
    let mut request = TowerRequest::new(
        "source-submit",
        tower.mount_address().clone(),
        Verb::Propose,
    );
    request.arguments = ReadArguments::Propose {
        request: key,
        proposal: file.rig.proposal(),
    };
    request.idempotency_key = Some(key.to_string());
    request
}

fn poll(tower: &PublicationTower, key: u64) -> String {
    encode_request(&TowerRequest::new(
        "source-poll",
        tower.request_address(key).unwrap(),
        Verb::Get,
    ))
    .unwrap()
}

fn native(response: &PublicationResponse) -> &WireResponse {
    let PublicationResult::Request(native) = &response.result else {
        panic!("expected original actor observation: {response:?}");
    };
    native
}

fn image(file: &FileRig) -> Vec<u8> {
    std::fs::read(file.rig.root.store().join("delivery.bin")).unwrap()
}

#[test]
fn cold_tower_submission_reaches_original_file_review_two_keys_and_one_execution() {
    let mut file = cold(10, StateLimits::default());
    let mut tower = tower(&file);
    let request = submit(&file, &tower, 1);
    let bytes = encode_request(&request).unwrap();
    assert_eq!(
        native(&tower.exchange(&access(), bytes.as_bytes())).result,
        Err(WireError::Unavailable)
    );
    let admitted = file
        .rig
        .driver
        .exchange_tower_from_file(
            &mut tower,
            &access(),
            bytes.as_bytes(),
            &mut file.source,
            || ElapsedTick(1),
        )
        .unwrap();
    assert_eq!(
        native(&admitted.response).result,
        Ok(Knowledge::Pending { request: 1 })
    );
    let intake = admitted.intake.unwrap();
    assert_eq!(intake.result, Ok(document(1).identity()));
    assert_eq!(intake.observations, vec![Ok(document(1).identity())]);
    assert_eq!(intake.source_updates, vec![Ok(document(1).identity())]);
    assert_eq!(file.source.status().read_attempts, 1);
    assert_eq!(
        file.rig.port.submit(2, &file.rig.proposal()).unwrap_err(),
        ActorError::Unavailable
    );

    file.reviewed();
    let human = file.human();
    file.dispatch(&human);
    let status = poll(&tower, 1);
    let uncertain = file
        .rig
        .driver
        .exchange_tower_request_observe(&mut tower, &reader(), 1, status.as_bytes())
        .unwrap();
    assert_eq!(
        native(&uncertain).result,
        Ok(Knowledge::Unknown {
            reason: UnknownReason::OutcomeUnknown
        })
    );
    let published = file
        .rig
        .driver
        .step_from_file(&mut file.source, || ElapsedTick(1), None);
    assert!(
        matches!(published.result, Ok(FileDriverEvent::PublicationChecked { publication, .. })
        if publication.outcome == EndpointOutcome::Executed { resulting_version: 2 })
    );
    std::fs::remove_file(&file.path).unwrap();
    let settled = file
        .rig
        .driver
        .step_from_file(&mut file.source, || ElapsedTick(2), None);
    assert!(settled.source_updates.is_empty());
    assert!(matches!(
        settled.result,
        Ok(FileDriverEvent::Reconciled {
            outcome: Reconciliation::Resolved(EndpointOutcome::Executed { .. }),
            ..
        })
    ));
    let completed = file
        .rig
        .driver
        .exchange_tower_request_observe(&mut tower, &reader(), 1, status.as_bytes())
        .unwrap();
    assert!(matches!(
        native(&completed).result,
        Ok(Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        })
    ));
    let final_image = image(&file);
    let reads = file.source.status().read_attempts;
    let retried = file
        .rig
        .driver
        .exchange_tower_from_file(
            &mut tower,
            &access(),
            bytes.as_bytes(),
            &mut file.source,
            || panic!("an executed retry must not renew source or time"),
        )
        .unwrap();
    assert_eq!(native(&retried.response).result, native(&completed).result);
    assert!(retried.intake.is_none());
    assert_eq!(image(&file), final_image);
    assert_eq!(file.source.status().read_attempts, reads);
    let host = file.rig.driver.supervisor().host().unwrap();
    assert_eq!(host.inspect().executions, 1);
    assert_eq!(host.inspect().control.ledger.charged, 16);
}

#[test]
fn complete_tower_and_native_typed_validation_precede_source_clock_and_journal_work() {
    let mut file = cold(10, StateLimits::default());
    let mut tower = tower(&file);
    let valid = submit(&file, &tower, 1);
    let bytes = encode_request(&valid).unwrap();
    let before = image(&file);
    for malformed in [
        b"{}".to_vec(),
        format!("{bytes}{{}}").into_bytes(),
        bytes.replacen('{', "{\"operator\":true,", 1).into_bytes(),
    ] {
        let refused = file
            .rig
            .driver
            .exchange_tower_from_file(&mut tower, &access(), &malformed, &mut file.source, || {
                panic!("malformed bytes sampled time")
            })
            .unwrap();
        assert_eq!(
            refused.response.envelope.reason_code,
            Some(ReasonCode::IntegrityFailure)
        );
        assert!(refused.intake.is_none());
    }
    let mut invalid = Vec::new();
    let mut old = valid.clone();
    old.address.generation = Some(40);
    invalid.push((old, ReasonCode::StaleGeneration));
    let mut foreign_mount = valid.clone();
    foreign_mount.address.id = "999".to_owned();
    invalid.push((foreign_mount, ReasonCode::MissingRequiredObservation));
    let mut unsupported = valid.clone();
    unsupported.verb = Verb::Verify;
    unsupported.arguments = ReadArguments::None;
    invalid.push((unsupported, ReasonCode::CapabilityUnsupported));
    let mut bad_target = valid.clone();
    let ReadArguments::Propose { proposal, .. } = &mut bad_target.arguments else {
        unreachable!()
    };
    proposal.target.generation = 0;
    invalid.push((bad_target, ReasonCode::IntegrityFailure));
    let mut bad_payload = valid.clone();
    let ReadArguments::Propose { proposal, .. } = &mut bad_payload.arguments else {
        unreachable!()
    };
    proposal.payload = vec![0; MAX_PAYLOAD_BYTES + 1];
    invalid.push((bad_payload, ReasonCode::BudgetExhausted));
    for (request, reason) in invalid {
        let refused = file
            .rig
            .driver
            .dispatch_tower_from_file(&mut tower, &access(), request, &mut file.source, || {
                panic!("invalid typed request acquired source or time")
            })
            .unwrap();
        assert_eq!(refused.response.envelope.reason_code, Some(reason));
        assert!(refused.intake.is_none());
    }
    let denied = file
        .rig
        .driver
        .dispatch_tower_from_file(
            &mut tower,
            &reader(),
            valid.clone(),
            &mut file.source,
            || panic!("missing purpose acquired source or time"),
        )
        .unwrap();
    assert_eq!(
        denied.response.envelope.reason_code,
        Some(ReasonCode::PurposeMismatch)
    );
    let foreign = TowerAccess::new(
        CallerScope::full_tenant_access("other"),
        true,
        vec![PurposeTag::Permit],
        0,
    );
    let denied = file
        .rig
        .driver
        .dispatch_tower_from_file(
            &mut tower,
            &foreign,
            valid.clone(),
            &mut file.source,
            || panic!("foreign caller acquired source or time"),
        )
        .unwrap();
    assert_eq!(
        denied.response.envelope.reason_code,
        Some(ReasonCode::DeniedByPolicy)
    );
    assert!(denied.intake.is_none());
    let unseen = poll(&tower, 1);
    let hidden = file
        .rig
        .driver
        .exchange_tower_from_file(
            &mut tower,
            &reader(),
            unseen.as_bytes(),
            &mut file.source,
            || panic!("a ticket read acquired source or time"),
        )
        .unwrap();
    assert!(hidden.intake.is_none());
    assert_eq!(file.source.status().read_attempts, 0);
    assert_eq!(image(&file), before);
    let accepted = file
        .rig
        .driver
        .dispatch_tower_from_file(&mut tower, &access(), valid, &mut file.source, || {
            ElapsedTick(1)
        })
        .unwrap();
    assert_eq!(
        native(&accepted.response).result,
        Ok(Knowledge::Pending { request: 1 })
    );
    assert!(accepted.intake.unwrap().result.is_ok());
}

#[test]
fn exact_conflicting_and_new_session_retries_do_not_reopen_missing_source() {
    for refused in [false, true] {
        let mut file = cold(10, StateLimits::default());
        let mut adapter = tower(&file);
        let mut request = submit(&file, &adapter, 1);
        if refused {
            let ReadArguments::Propose { proposal, .. } = &mut request.arguments else {
                unreachable!()
            };
            proposal.expected_policy_epoch += 1;
        }
        let bytes = encode_request(&request).unwrap();
        let original = file
            .rig
            .driver
            .exchange_tower_from_file(
                &mut adapter,
                &access(),
                bytes.as_bytes(),
                &mut file.source,
                || ElapsedTick(1),
            )
            .unwrap();
        assert!(original.intake.unwrap().result.is_ok());
        if refused {
            assert!(matches!(
                native(&original.response).result,
                Ok(Knowledge::Known {
                    value: ActorOutcome::NotAdmitted,
                    ..
                })
            ));
        } else {
            assert_eq!(
                native(&original.response).result,
                Ok(Knowledge::Pending { request: 1 })
            );
        }
        std::fs::remove_file(&file.path).unwrap();
        let before = image(&file);
        for new_session in [false, true] {
            if new_session {
                adapter = tower(&file);
            }
            let retry = file
                .rig
                .driver
                .exchange_tower_from_file(
                    &mut adapter,
                    &access(),
                    bytes.as_bytes(),
                    &mut file.source,
                    || panic!("a retained exact request reacquired source"),
                )
                .unwrap();
            assert_eq!(native(&retry.response), native(&original.response));
            assert!(retry.intake.is_none());
            let mut different = request.clone();
            let ReadArguments::Propose { proposal, .. } = &mut different.arguments else {
                unreachable!()
            };
            proposal.payload = b"other".to_vec();
            let conflict = file
                .rig
                .driver
                .dispatch_tower_from_file(
                    &mut adapter,
                    &access(),
                    different,
                    &mut file.source,
                    || panic!("a conflicting retained request reacquired source"),
                )
                .unwrap();
            assert_eq!(
                native(&conflict.response).result,
                Err(WireError::IdempotencyConflict)
            );
            assert!(conflict.intake.is_none());
        }
        assert_eq!(image(&file), before);
        assert_eq!(file.source.status().read_attempts, 1);
        assert_eq!(
            file.rig
                .driver
                .supervisor()
                .host()
                .unwrap()
                .retained_requests(),
            1
        );
    }
}

#[test]
fn same_scope_foreign_owner_refuses_before_any_source_or_original_owner_change() {
    let mut first = cold(10, StateLimits::default());
    let second = cold(10, StateLimits::default());
    let mut foreign = tower(&second);
    let request = submit(&first, &foreign, 1);
    let bytes = encode_request(&request).unwrap();
    let first_image = image(&first);
    let second_image = image(&second);
    for document in [bytes.as_bytes(), b"{}"] {
        let refused = first.rig.driver.exchange_tower_from_file(
            &mut foreign,
            &access(),
            document,
            &mut first.source,
            || panic!("foreign owner sampled time"),
        );
        assert!(matches!(
            refused,
            Err(JournalError::Contract(Error::Binding))
        ));
    }
    let refused = first.rig.driver.dispatch_tower_from_file(
        &mut foreign,
        &access(),
        request,
        &mut first.source,
        || panic!("foreign typed owner sampled time"),
    );
    assert!(matches!(
        refused,
        Err(JournalError::Contract(Error::Binding))
    ));
    assert_eq!(first.source.status().read_attempts, 0);
    assert_eq!(image(&first), first_image);
    assert_eq!(image(&second), second_image);
    let mut original = tower(&first);
    let allowed = first
        .rig
        .driver
        .exchange_tower_from_file(
            &mut original,
            &access(),
            bytes.as_bytes(),
            &mut first.source,
            || ElapsedTick(1),
        )
        .unwrap();
    assert_eq!(
        native(&allowed.response).result,
        Ok(Knowledge::Pending { request: 1 })
    );
}

#[test]
fn missing_source_withdraws_old_snapshot_and_keeps_private_failure_in_host_report() {
    let mut file = cold(10, StateLimits::default());
    let mut tower = tower(&file);
    file.rig
        .driver
        .prepare_file_intake(&mut file.source, || ElapsedTick(1))
        .result
        .unwrap();
    let request = submit(&file, &tower, 1);
    let bytes = encode_request(&request).unwrap();
    std::fs::remove_file(&file.path).unwrap();
    let refused = file
        .rig
        .driver
        .exchange_tower_from_file(
            &mut tower,
            &access(),
            bytes.as_bytes(),
            &mut file.source,
            || ElapsedTick(2),
        )
        .unwrap();
    assert_eq!(
        native(&refused.response).result,
        Err(WireError::Unavailable)
    );
    let intake = refused.intake.unwrap();
    assert!(intake.result.is_err());
    assert_eq!(intake.observations.len(), 1);
    assert_eq!(intake.source_updates.len(), 1);
    assert!(intake.observations[0].is_err());
    let encoded = refused.response.to_json().unwrap();
    assert!(!encoded.contains("evidence.json"));
    assert!(!encoded.contains(file.path.to_str().unwrap()));
    assert_eq!(
        file.rig
            .driver
            .supervisor()
            .host()
            .unwrap()
            .retained_requests(),
        0
    );
    assert_eq!(
        file.rig.port.submit(1, &file.rig.proposal()).unwrap_err(),
        ActorError::Unavailable
    );
    replace(&file.path, &document(3));
    let repaired = file
        .rig
        .driver
        .exchange_tower_from_file(
            &mut tower,
            &access(),
            bytes.as_bytes(),
            &mut file.source,
            || ElapsedTick(3),
        )
        .unwrap();
    assert_eq!(
        native(&repaired.response).result,
        Ok(Knowledge::Pending { request: 1 })
    );
    assert_eq!(repaired.intake.unwrap().result, Ok(document(3).identity()));
    assert_eq!(file.source.status().read_attempts, 3);
}

#[test]
fn selected_request_and_observation_modes_cannot_consume_an_unrelated_or_new_snapshot() {
    let mut file = cold(10, StateLimits::default());
    let mut tower = tower(&file);
    file.rig
        .driver
        .prepare_file_intake(&mut file.source, || ElapsedTick(1))
        .result
        .unwrap();
    let first = encode_request(&submit(&file, &tower, 1)).unwrap();
    let second = encode_request(&submit(&file, &tower, 2)).unwrap();
    let before = image(&file);
    let wrong_key = file
        .rig
        .driver
        .exchange_tower_request_from_file(
            &mut tower,
            &access(),
            1,
            second.as_bytes(),
            &mut file.source,
            || panic!("unselected request sampled time"),
        )
        .unwrap();
    assert_eq!(native(&wrong_key.response).result, Err(WireError::Withheld));
    assert!(wrong_key.intake.is_none());
    let unrecorded = file
        .rig
        .driver
        .exchange_tower_request_observe(&mut tower, &access(), 1, first.as_bytes())
        .unwrap();
    assert_eq!(native(&unrecorded).result, Err(WireError::Unavailable));
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status().read_attempts, 1);
    // Positive control: the original explicitly prepared snapshot is still
    // usable by the ordinary native Tower, so observation did not consume it.
    assert_eq!(
        native(&tower.exchange(&access(), first.as_bytes())).result,
        Ok(Knowledge::Pending { request: 1 })
    );
    let selected = file
        .rig
        .driver
        .exchange_tower_request_from_file(
            &mut tower,
            &access(),
            2,
            second.as_bytes(),
            &mut file.source,
            || ElapsedTick(1),
        )
        .unwrap();
    assert_eq!(
        native(&selected.response).result,
        Ok(Knowledge::Pending { request: 2 })
    );
    assert!(selected.intake.unwrap().result.is_ok());
    std::fs::remove_file(&file.path).unwrap();
    let before = image(&file);
    let retry = file
        .rig
        .driver
        .exchange_tower_request_observe(&mut tower, &access(), 2, second.as_bytes())
        .unwrap();
    assert_eq!(native(&retry).result, native(&selected.response).result);
    let foreign = file
        .rig
        .driver
        .exchange_tower_request_observe(&mut tower, &access(), 2, first.as_bytes())
        .unwrap();
    assert_eq!(native(&foreign).result, Err(WireError::Withheld));
    assert_eq!(image(&file), before);
    assert_eq!(file.source.status().read_attempts, 2);
}

#[test]
fn exact_read_start_expiry_refuses_without_admitting_then_fresh_capture_recovers() {
    for completed in [4, 5] {
        let mut file = cold(3, StateLimits::default());
        let mut tower = tower(&file);
        let bytes = encode_request(&submit(&file, &tower, 1)).unwrap();
        let mut ticks = 0;
        let report = file
            .rig
            .driver
            .exchange_tower_from_file(
                &mut tower,
                &access(),
                bytes.as_bytes(),
                &mut file.source,
                || {
                    ticks += 1;
                    ElapsedTick(if ticks == 1 { 2 } else { completed })
                },
            )
            .unwrap();
        assert_eq!(ticks, 2);
        if completed == 4 {
            assert!(report.intake.unwrap().result.is_ok());
            assert_eq!(
                native(&report.response).result,
                Ok(Knowledge::Pending { request: 1 })
            );
        } else {
            assert_eq!(
                report.intake.unwrap().result,
                Err(JournalError::Contract(Error::Stale))
            );
            assert_eq!(native(&report.response).result, Err(WireError::Unavailable));
            assert_eq!(
                file.rig
                    .driver
                    .supervisor()
                    .host()
                    .unwrap()
                    .retained_requests(),
                0
            );
            let fresh = file
                .rig
                .driver
                .exchange_tower_from_file(
                    &mut tower,
                    &access(),
                    bytes.as_bytes(),
                    &mut file.source,
                    || ElapsedTick(5),
                )
                .unwrap();
            assert!(fresh.intake.unwrap().result.is_ok());
            assert_eq!(
                native(&fresh.response).result,
                Ok(Knowledge::Pending { request: 1 })
            );
        }
        assert_eq!(
            file.rig
                .driver
                .supervisor()
                .host()
                .unwrap()
                .inspect()
                .executions,
            0
        );
    }
}

#[test]
fn acknowledged_source_preparation_is_distinct_from_failed_durable_submission() {
    let mut file = cold(10, StateLimits::default());
    let mut tower = tower(&file);
    let bytes = encode_request(&submit(&file, &tower, 1)).unwrap();
    let pending = file.rig.root.store().join("delivery.pending");
    let mut calls = 0;
    let report = file
        .rig
        .driver
        .exchange_tower_from_file(
            &mut tower,
            &access(),
            bytes.as_bytes(),
            &mut file.source,
            || {
                calls += 1;
                if calls == 2 {
                    std::fs::write(&pending, b"occupied before submit").unwrap();
                }
                ElapsedTick(1)
            },
        )
        .unwrap();
    assert_eq!(calls, 2);
    assert!(report.intake.unwrap().result.is_ok());
    assert_eq!(native(&report.response).result, Err(WireError::Unavailable));
    let host = file.rig.driver.supervisor().host().unwrap();
    assert!(host.storage_failure().is_some());
    assert!(host.inspect().control.ledger.stages.is_empty());
    assert_eq!(file.source.status().read_attempts, 1);
}
