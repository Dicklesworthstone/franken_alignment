//! Explicit route credentials drive the original actor authority and real file
//! endpoints. Trusted provisioning and a surviving controller remain required;
//! this does not establish provider authentication or whole-process recovery.
#![cfg(unix)]

#[path = "support/actor_gateway.rs"]
mod gateway;
#[path = "perimeter_route_credentials/support.rs"]
mod support;

use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::{
    EndpointOutcome, EndpointStatus, NonExecutionReason, PublicationEndpoint,
    credential_broker::{
        BrokerCredential, CredentialBroker, CredentialRevocationRequest, CredentialRotationRequest,
        DISPOSABLE_FILE_PROFILE, ProviderCredential,
    },
};
use fa_reference::action::consequence::oversight::{
    DispatchKeys,
    actor::{ActorOutcome, Knowledge},
};
use fa_reference::perimeter::{
    MAX_CREDENTIALS_PER_FAMILY, MAX_FAMILY_TEXT_BYTES, MAX_RESIDUAL_NONCLAIMS,
    MAX_ROUTES_PER_FAMILY, MAX_TEXT_BYTES,
};
use fa_reference::perimeter_inventory::{
    ActorCredentialDisposition, InventoryLoadError, LoadedPerimeterInventory,
};
use fa_reference::{Error, strict_json};
use std::fs;
use support::*;

#[test]
fn explicit_routes_select_exact_credentials_without_changing_v1_resolution() {
    let root = Directory::new();
    let path = root.path().join("perimeter.json");
    let bytes = inventory_json(2, 2);
    fs::write(&path, &bytes).unwrap();
    let loaded = LoadedPerimeterInventory::load_path(&path).unwrap();
    assert_eq!(
        loaded,
        LoadedPerimeterInventory::from_json_bytes(bytes.as_bytes()).unwrap()
    );
    assert_eq!(loaded.family_count(), 1);
    for (index, route) in ROUTES.iter().enumerate() {
        let selected = loaded
            .route_for(perimeter_scope(), "publication", route)
            .unwrap();
        assert_eq!(
            selected.metadata().actor_credential(),
            &ActorCredentialDisposition::BrokerMediated
        );
        assert_eq!(
            selected.metadata().broker_credential_name(),
            Some(NAMES[index])
        );
        assert_eq!(
            loaded.broker_credential_for_route(perimeter_scope(), "publication", route),
            Ok(NAMES[index])
        );
    }
    let mut foreign = perimeter_scope();
    foreign.tenant += 1;
    assert_eq!(
        loaded.broker_credential_for_route(foreign, "publication", ROUTES[0]),
        Err(Error::Missing)
    );
    assert_eq!(
        loaded.broker_credential_for_route(perimeter_scope(), "other-family", ROUTES[0]),
        Err(Error::Missing)
    );
    assert_eq!(
        loaded.broker_credential_for_route(perimeter_scope(), "publication", "other-route"),
        Err(Error::Missing)
    );

    // Old documents are interpreted as before. In particular, v1 cannot silently
    // inherit the v2 declaration merely because its family has multiple names.
    for count in [1, 2] {
        let legacy =
            LoadedPerimeterInventory::from_json_bytes(inventory_json(1, count).as_bytes()).unwrap();
        for route in ROUTES {
            assert_eq!(
                legacy
                    .route_for(perimeter_scope(), "publication", route)
                    .unwrap()
                    .metadata()
                    .broker_credential_name(),
                None
            );
            assert_eq!(
                legacy.broker_credential_for_route(perimeter_scope(), "publication", route),
                if count == 1 {
                    Ok(NAMES[0])
                } else {
                    Err(Error::Binding)
                }
            );
        }
    }
    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(
            bytes
                .replacen("\"version\":2", "\"version\":1", 1)
                .as_bytes()
        ),
        Err(InventoryLoadError::Schema(Error::InvalidInput))
    );
    // A v2 route must name its credential even if only one family name exists.
    let unnamed = inventory_json(1, 1).replacen("\"version\":1", "\"version\":2", 1);
    assert_eq!(
        LoadedPerimeterInventory::from_json_bytes(unnamed.as_bytes()),
        Err(InventoryLoadError::Schema(Error::InvalidInput))
    );
}

#[test]
fn named_credentials_are_family_owned_and_closed_without_weakening_perimeter_rules() {
    let valid = inventory_json(2, 2);
    let selected = format!(r#""kind":"broker_mediated", "credential":"{}""#, NAMES[0]);
    let invalid = [
        (
            valid.replacen("\"version\":2", "\"version\":3", 1),
            Error::InvalidInput,
        ),
        (
            valid.replacen(&selected, "\"kind\":\"broker_mediated\"", 1),
            Error::InvalidInput,
        ),
        (
            valid.replacen(
                &selected,
                "\"kind\":\"broker_mediated\",\"credential\":null",
                1,
            ),
            Error::InvalidInput,
        ),
        (
            valid.replacen(
                &selected,
                "\"kind\":\"broker_mediated\",\"credential\":\"\"",
                1,
            ),
            Error::InvalidInput,
        ),
        (
            valid.replacen(
                &selected,
                "\"kind\":\"broker_mediated\",\"credential\":\"undeclared\"",
                1,
            ),
            Error::Binding,
        ),
        (
            valid.replacen(&selected, &(selected.clone() + ",\"extra\":true"), 1),
            Error::InvalidInput,
        ),
        (
            valid.replacen(
                &credential_json(NAMES[0], "broker"),
                &credential_json(NAMES[0], "actor_direct"),
                1,
            ),
            Error::Binding,
        ),
        (
            valid.replacen(
                "\"mediation\":\"brokered_effects\"",
                "\"mediation\":\"observe_only\"",
                1,
            ),
            Error::Binding,
        ),
        (
            valid.replacen(
                "\"bypass\":\"blocked\"",
                "\"bypass\":\"residual_uncovered\"",
                1,
            ),
            Error::Binding,
        ),
        (
            valid.replacen("\"bypass\":\"blocked\"", "\"bypass\":\"unmodeled\"", 1),
            Error::Binding,
        ),
        (
            valid.replacen(
                "\"kind\":\"broker_mediated\"",
                "\"kind\":\"none_declared\"",
                1,
            ),
            Error::InvalidInput,
        ),
    ];
    for (candidate, cause) in invalid {
        assert_ne!(candidate, valid);
        assert_eq!(
            LoadedPerimeterInventory::from_json_bytes(candidate.as_bytes()),
            Err(InventoryLoadError::Schema(cause)),
            "{candidate}"
        );
    }
    let duplicate = valid.replacen(
        &selected,
        &format!(r#"{selected},"credential":"{}""#, NAMES[1]),
        1,
    );
    assert!(
        matches!(LoadedPerimeterInventory::from_json_bytes(duplicate.as_bytes()),
        Err(InventoryLoadError::Parse(error))
        if error.kind == strict_json::ErrorKind::DuplicateKey("credential".into()))
    );

    // A name in a different family must not satisfy this route. The only causal
    // difference from the admitted control is the first route's explicit name.
    let family_b = family_json(
        "second-family",
        &[credential_json(NAMES[1], "broker")],
        &[route_json(
            ROUTES[1],
            DISPOSABLE_FILE_PROFILE,
            Some(NAMES[1]),
        )],
    );
    for correct in [true, false] {
        let selected = if correct { NAMES[0] } else { NAMES[1] };
        let family_a = family_json(
            "publication",
            &[credential_json(NAMES[0], "broker")],
            &[route_json(
                ROUTES[0],
                DISPOSABLE_FILE_PROFILE,
                Some(selected),
            )],
        );
        let candidate = document(2, &[family_a, family_b.clone()]);
        let loaded = LoadedPerimeterInventory::from_json_bytes(candidate.as_bytes());
        if correct {
            let loaded = loaded.unwrap();
            assert_eq!(loaded.family_count(), 2);
            assert_eq!(
                loaded.broker_credential_for_route(perimeter_scope(), "publication", ROUTES[0]),
                Ok(NAMES[0])
            );
            assert_eq!(
                loaded.broker_credential_for_route(perimeter_scope(), "second-family", ROUTES[1]),
                Ok(NAMES[1])
            );
        } else {
            assert_eq!(loaded, Err(InventoryLoadError::Schema(Error::Binding)));
        }
    }

    // Legitimate v2 observation profiles keep their original classifications.
    // Neither an actor-held credential nor no credential becomes broker access.
    for direct in [false, true] {
        let disposition = if direct {
            format!(r#""kind":"actor_direct","credential":"{}""#, NAMES[0])
        } else {
            "\"kind\":\"none_declared\"".into()
        };
        let route = route_json(ROUTES[0], DISPOSABLE_FILE_PROFILE, None)
            .replace("\"kind\":\"broker_mediated\"", &disposition)
            .replace(
                "\"mediation\":\"brokered_effects\"",
                "\"mediation\":\"observe_only\"",
            )
            .replace(
                "\"bypass\":\"blocked\"",
                "\"bypass\":\"residual_uncovered\"",
            );
        let credentials = if direct {
            vec![credential_json(NAMES[0], "actor_direct")]
        } else {
            vec![]
        };
        let bytes = document(2, &[family_json("publication", &credentials, &[route])]);
        let loaded = LoadedPerimeterInventory::from_json_bytes(bytes.as_bytes()).unwrap();
        let metadata = loaded
            .route_for(perimeter_scope(), "publication", ROUTES[0])
            .unwrap()
            .metadata();
        assert_eq!(metadata.broker_credential_name(), None);
        assert_eq!(
            metadata.actor_credential(),
            &if direct {
                ActorCredentialDisposition::ActorDirect {
                    credential: NAMES[0].into(),
                }
            } else {
                ActorCredentialDisposition::NoneDeclared
            }
        );
        assert_eq!(
            loaded.broker_credential_for_route(perimeter_scope(), "publication", ROUTES[0]),
            Err(Error::Binding)
        );
        let escalated = bytes.replace(
            "\"mediation\":\"observe_only\"",
            "\"mediation\":\"brokered_effects\"",
        );
        assert_eq!(
            LoadedPerimeterInventory::from_json_bytes(escalated.as_bytes()),
            Err(InventoryLoadError::Schema(Error::Binding))
        );
    }
}

#[test]
fn explicit_name_references_consume_the_original_text_and_count_allowances() {
    for length in [MAX_TEXT_BYTES, MAX_TEXT_BYTES + 1] {
        let name = "x".repeat(length);
        let candidate = document(
            2,
            &[family_json(
                "publication",
                &[credential_json(&name, "broker")],
                &[route_json("route", "p", Some(&name))],
            )],
        );
        let loaded = LoadedPerimeterInventory::from_json_bytes(candidate.as_bytes());
        if length == MAX_TEXT_BYTES {
            assert_eq!(
                loaded.unwrap().broker_credential_for_route(
                    perimeter_scope(),
                    "publication",
                    "route"
                ),
                Ok(name.as_str())
            );
        } else {
            assert!(matches!(loaded, Err(InventoryLoadError::Parse(error))
                if error.kind == strict_json::ErrorKind::StringLimit));
        }
    }

    // Every explicit reference is owned text, even when it repeats a declaration.
    // This exact 4096/4097 pair fails if the new reference is not charged.
    let count = MAX_CREDENTIALS_PER_FAMILY - 2;
    let names = (0..count)
        .map(|index| {
            let prefix = format!("credential-{index}");
            format!("{prefix}{}", "x".repeat(MAX_TEXT_BYTES - prefix.len()))
        })
        .collect::<Vec<_>>();
    let declarations = names
        .iter()
        .map(|name| credential_json(name, "broker"))
        .collect::<Vec<_>>()
        .join(",");
    let residuals = (0..MAX_RESIDUAL_NONCLAIMS)
        .map(|index| {
            let prefix = format!("residual-{index}");
            format!(r#""{prefix}{}""#, "n".repeat(MAX_TEXT_BYTES - prefix.len()))
        })
        .collect::<Vec<_>>()
        .join(",");
    let family_length = MAX_FAMILY_TEXT_BYTES
        - count * MAX_TEXT_BYTES
        - MAX_RESIDUAL_NONCLAIMS * MAX_TEXT_BYTES
        - MAX_TEXT_BYTES
        - 3;
    assert!(family_length + 1 <= MAX_TEXT_BYTES);
    for extra in [0, 1] {
        let family = "f".repeat(family_length + extra);
        let candidate = format!(
            r#"{{"version":2,"families":[{{"scope":{{"tenant":1,"principal":2,"purpose":1}},"family":"{family}","trust_domains":["actor","observation_and_analysis","enforcement","governance_and_investigation"],"credentials":[{declarations}],"routes":[{{"route":"r","effect":"file_write","profile":{{"id":"p","generation":1}},"trust_path":["actor","enforcement"],"threat":"direct_credential_or_egress","actor_credential":{{"kind":"broker_mediated","credential":"{}"}},"mediation":"brokered_effects","bypass":"blocked","residual_nonclaims":["n"]}}],"residual_nonclaims":[{residuals}]}}]}}"#,
            names[0]
        );
        let loaded = LoadedPerimeterInventory::from_json_bytes(candidate.as_bytes());
        if extra == 0 {
            assert_eq!(
                loaded
                    .unwrap()
                    .broker_credential_for_route(perimeter_scope(), &family, "r"),
                Ok(names[0].as_str())
            );
        } else {
            assert_eq!(loaded, Err(InventoryLoadError::Schema(Error::Limit)));
        }
    }

    for extra in [0, 1] {
        let names = (0..MAX_CREDENTIALS_PER_FAMILY + extra)
            .map(|index| format!("c{index}"))
            .collect::<Vec<_>>();
        let declared = names
            .iter()
            .map(|name| credential_json(name, "broker"))
            .collect::<Vec<_>>();
        let routes = (0..MAX_ROUTES_PER_FAMILY)
            .map(|index| route_json(&format!("r{index}"), "p", Some(&names[index % names.len()])))
            .collect::<Vec<_>>();
        let candidate = document(2, &[family_json("publication", &declared, &routes)]);
        let loaded = LoadedPerimeterInventory::from_json_bytes(candidate.as_bytes());
        if extra == 0 {
            assert!(loaded.is_ok());
            let mut over = routes;
            over.push(route_json("one-more-route", "p", Some(&names[0])));
            assert_eq!(
                LoadedPerimeterInventory::from_json_bytes(
                    document(2, &[family_json("publication", &declared, &over)]).as_bytes()
                ),
                Err(InventoryLoadError::Schema(Error::Limit))
            );
        } else {
            assert_eq!(loaded, Err(InventoryLoadError::Schema(Error::Limit)));
        }
    }
}

#[test]
fn two_named_routes_publish_distinct_real_files_through_original_actor_authority() {
    let mut rigs = [Rig::new(0), Rig::new(1)];
    assert_ne!(rigs[0].root.endpoint(), rigs[1].root.endpoint());
    assert_eq!(
        rigs[0].perimeter.broker().target(),
        rigs[1].perimeter.broker().target()
    );
    for (index, rig) in rigs.iter_mut().enumerate() {
        let payload = if index == 0 {
            b"provider-a"
        } else {
            b"provider-b"
        };
        let (ticket, inputs, permit) = rig.reviewed(1, payload);
        assert_eq!(rig.supervisor.broker().inspect().ledger.reserved, 16);
        let receipt = rig
            .supervisor
            .deliver_request_brokered(
                1,
                DispatchKeys::single(&permit),
                Some(&inputs),
                &gateway::snapshot(),
                rig.perimeter.broker_mut(),
            )
            .unwrap();
        assert_eq!(
            receipt.outcome(),
            EndpointOutcome::Executed {
                resulting_version: 2
            }
        );
        assert!(matches!(
            rig.port.poll(&ticket),
            Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            }
        ));
        let visible = PublicationEndpoint::read_file_publication(rig.root.endpoint()).unwrap();
        assert_eq!(visible.payload, payload);
        assert_eq!(visible.execution_count, 1);
        assert_eq!(visible.terminal_receipts, 1);
        assert_eq!(visible.target.expected_version, 2);
        assert_eq!(rig.perimeter.broker().inspect().route, ROUTES[index]);
        assert_eq!(rig.perimeter.broker().inspect().credential_exercises, 1);
        let ledger = rig.supervisor.broker().inspect().ledger;
        assert_eq!(ledger.available, 84);
        assert_eq!(ledger.reserved, 0);
        assert_eq!(ledger.charged, 16);
        rig.assert_no_secret();
    }
}

#[test]
fn foreign_envelopes_and_cross_provider_secrets_cannot_substitute_named_routes() {
    // Mismatched material refuses before changing either endpoint or authority.
    // Its independently retained original recovery key then permits a correctly
    // provisioned retry, followed by real original actor/congress publication.
    for index in 0..2 {
        let mut original = Unbound::new();
        let before = PublicationEndpoint::read_file_publication(original.root.endpoint()).unwrap();
        let authority = original.supervisor.broker().inspect();
        let failed = CredentialBroker::new(
            inventory(),
            binding(index),
            scope(),
            BrokerCredential::new(SECRETS[1 - index].to_vec()).unwrap(),
            ProviderCredential::new(SECRETS[index].to_vec()).unwrap(),
            original.endpoint.take().unwrap(),
        )
        .unwrap_err();
        assert_eq!(failed, Error::Binding);
        assert_eq!(
            PublicationEndpoint::read_file_publication(original.root.endpoint()).unwrap(),
            before
        );
        assert_eq!(original.supervisor.broker().inspect(), authority);
        let mut endpoint = original.recovery.as_ref().unwrap().reopen().unwrap();
        endpoint.observe_time(ElapsedTick(2)).unwrap();
        original
            .supervisor
            .broker_mut()
            .observe_time(ElapsedTick(2))
            .unwrap();
        original.endpoint = Some(endpoint);
        let mut rig = original.bind(index);
        let (ticket, inputs, permit) = rig.reviewed(1, b"matched-pair");
        rig.supervisor
            .deliver_request_brokered(
                1,
                DispatchKeys::single(&permit),
                Some(&inputs),
                &gateway::snapshot(),
                rig.perimeter.broker_mut(),
            )
            .unwrap();
        assert!(matches!(
            rig.port.poll(&ticket),
            Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            }
        ));
        assert_eq!(
            PublicationEndpoint::read_file_publication(rig.root.endpoint())
                .unwrap()
                .execution_count,
            1
        );
        rig.assert_no_secret();
    }

    let mut left = Rig::new(0);
    let mut right = Rig::new(1);
    let (left_ticket, left_inputs, left_permit) = left.reviewed(1, b"left-original");
    let (right_ticket, right_inputs, right_permit) = right.reviewed(1, b"right-original");
    let left_message = left
        .supervisor
        .dispatch_request(
            1,
            DispatchKeys::single(&left_permit),
            Some(&left_inputs),
            &gateway::snapshot(),
        )
        .unwrap();
    let right_message = right
        .supervisor
        .dispatch_request(
            1,
            DispatchKeys::single(&right_permit),
            Some(&right_inputs),
            &gateway::snapshot(),
        )
        .unwrap();
    assert_eq!(
        left_message.request().target(),
        right_message.request().target()
    );
    assert_eq!(
        left.perimeter.broker_mut().deliver(&right_message),
        Err(Error::Binding)
    );
    assert_eq!(
        right.perimeter.broker_mut().deliver(&left_message),
        Err(Error::Binding)
    );
    left.supervisor.acknowledgment_lost(1).unwrap();
    right.supervisor.acknowledgment_lost(1).unwrap();
    assert!(matches!(
        left.port.poll(&left_ticket),
        Knowledge::Unknown { .. }
    ));
    assert!(matches!(
        right.port.poll(&right_ticket),
        Knowledge::Unknown { .. }
    ));
    for rig in [&left, &right] {
        let visible = PublicationEndpoint::read_file_publication(rig.root.endpoint()).unwrap();
        assert_eq!(visible.execution_count, 0);
        assert_eq!(visible.payload, b"old");
        assert_eq!(visible.terminal_receipts, 0);
        assert_eq!(rig.supervisor.broker().inspect().ledger.charged, 16);
        // The existing broker counts presentation before the endpoint rejects a
        // foreign issuer. That failed presentation never counts as execution.
        assert_eq!(rig.perimeter.broker().inspect().credential_exercises, 1);
    }
    let left_receipt = left.perimeter.broker_mut().deliver(&left_message).unwrap();
    let right_receipt = right
        .perimeter
        .broker_mut()
        .deliver(&right_message)
        .unwrap();
    left.supervisor.accept_receipt(left_receipt).unwrap();
    right.supervisor.accept_receipt(right_receipt).unwrap();
    for (rig, ticket, payload) in [
        (&left, &left_ticket, b"left-original".as_slice()),
        (&right, &right_ticket, b"right-original".as_slice()),
    ] {
        assert!(matches!(
            rig.port.poll(ticket),
            Knowledge::Known {
                value: ActorOutcome::Executed,
                ..
            }
        ));
        let visible = PublicationEndpoint::read_file_publication(rig.root.endpoint()).unwrap();
        assert_eq!(visible.payload, payload);
        assert_eq!(visible.execution_count, 1);
        assert_eq!(rig.perimeter.broker().inspect().credential_exercises, 2);
        assert_eq!(rig.supervisor.broker().inspect().ledger.charged, 16);
        rig.assert_no_secret();
    }
}

#[test]
fn named_routes_keep_receipts_rotation_and_revocation_through_original_worker_reopen() {
    let mut left = Rig::new(0);
    let mut right = Rig::new(1);
    let (left_ticket, left_inputs, left_permit) = left.reviewed(1, b"rotated-a");
    let (right_ticket, right_inputs, right_permit) = right.reviewed(1, b"revoked-b");
    let left_message = left
        .supervisor
        .dispatch_request(
            1,
            DispatchKeys::single(&left_permit),
            Some(&left_inputs),
            &gateway::snapshot(),
        )
        .unwrap();
    let right_message = right
        .supervisor
        .dispatch_request(
            1,
            DispatchKeys::single(&right_permit),
            Some(&right_inputs),
            &gateway::snapshot(),
        )
        .unwrap();
    let rotate = CredentialRotationRequest {
        operation: 7,
        expected_generation: 1,
        next_generation: 2,
    };
    assert_eq!(
        left.perimeter.broker_mut().rotate_credential(
            rotate,
            BrokerCredential::new(ROTATED.to_vec()).unwrap(),
            ProviderCredential::new(SECRETS[1].to_vec()).unwrap()
        ),
        Err(Error::Binding)
    );
    assert_eq!(left.perimeter.broker().inspect().credential_generation, 1);
    let rotation = left
        .perimeter
        .broker_mut()
        .rotate_credential(
            rotate,
            BrokerCredential::new(ROTATED.to_vec()).unwrap(),
            ProviderCredential::new(ROTATED.to_vec()).unwrap(),
        )
        .unwrap();
    let executed = left.perimeter.broker_mut().deliver(&left_message).unwrap();
    left.supervisor.acknowledgment_lost(1).unwrap();
    let revoke = CredentialRevocationRequest {
        operation: 8,
        expected_generation: 1,
    };
    let revocation = right
        .perimeter
        .broker_mut()
        .revoke_credential(revoke)
        .unwrap();
    assert_eq!(
        right.perimeter.broker_mut().deliver(&right_message),
        Err(Error::WrongState)
    );
    right.supervisor.acknowledgment_lost(1).unwrap();

    // The controller, opaque capabilities and recovery keys survive; only their
    // actual endpoint workers are reopened. No new effect permit is constructed.
    left.perimeter = left.perimeter.into_offline().reopen().unwrap();
    right.perimeter = right.perimeter.into_offline().reopen().unwrap();
    assert_eq!(left.perimeter.broker().inspect().credential_generation, 2);
    assert!(right.perimeter.broker().inspect().credential_revoked);
    assert_eq!(
        left.perimeter.broker_mut().rotate_credential(
            rotate,
            BrokerCredential::new(ROTATED.to_vec()).unwrap(),
            ProviderCredential::new(ROTATED.to_vec()).unwrap()
        ),
        Ok(rotation)
    );
    assert_eq!(
        right.perimeter.broker_mut().revoke_credential(revoke),
        Ok(revocation)
    );
    for rig in [&mut left, &mut right] {
        assert_eq!(rig.supervisor.broker().inspect().ledger.charged, 16);
        rig.supervisor
            .broker_mut()
            .observe_time(ElapsedTick(2))
            .unwrap();
        rig.perimeter
            .broker_mut()
            .observe_time(ElapsedTick(2))
            .unwrap();
        rig.supervisor
            .restart_brokered_dispatcher(rig.perimeter.broker_mut())
            .unwrap();
    }
    let left_outcomes = left
        .supervisor
        .reconcile_brokered_pending(left.perimeter.broker_mut())
        .unwrap();
    assert!(
        matches!(&left_outcomes[&1], Ok(EndpointStatus::Resolved(receipt)) if *receipt == executed)
    );
    assert!(matches!(
        left.port.poll(&left_ticket),
        Knowledge::Known {
            value: ActorOutcome::Executed,
            ..
        }
    ));
    assert_eq!(left.supervisor.broker().inspect().ledger.charged, 16);
    assert_eq!(left.perimeter.broker().inspect().credential_exercises, 1);
    let pending = right
        .supervisor
        .reconcile_brokered_pending(right.perimeter.broker_mut())
        .unwrap();
    assert_eq!(pending[&1], Ok(EndpointStatus::AwaitingResolution));
    assert!(matches!(
        right.port.poll(&right_ticket),
        Knowledge::Unknown { .. }
    ));
    assert_eq!(right.supervisor.broker().inspect().ledger.charged, 16);
    assert_eq!(
        right.perimeter.broker_mut().deliver(&right_message),
        Err(Error::WrongState)
    );

    right
        .supervisor
        .broker_mut()
        .observe_time(ElapsedTick(100))
        .unwrap();
    right
        .perimeter
        .broker_mut()
        .observe_time(ElapsedTick(100))
        .unwrap();
    let outcomes = right
        .supervisor
        .reconcile_brokered_pending(right.perimeter.broker_mut())
        .unwrap();
    assert!(
        matches!(&outcomes[&1], Ok(EndpointStatus::Resolved(receipt))
        if receipt.outcome() == (EndpointOutcome::NotExecuted { reason: NonExecutionReason::DeadlineElapsed }))
    );
    assert!(matches!(
        right.port.poll(&right_ticket),
        Knowledge::Known {
            value: ActorOutcome::ConfirmedNotExecuted,
            ..
        }
    ));
    assert_eq!(right.supervisor.broker().inspect().ledger.available, 100);
    assert_eq!(right.supervisor.broker().inspect().ledger.charged, 0);
    assert_eq!(right.perimeter.broker().inspect().credential_exercises, 0);
    let visible_left = PublicationEndpoint::read_file_publication(left.root.endpoint()).unwrap();
    let visible_right = PublicationEndpoint::read_file_publication(right.root.endpoint()).unwrap();
    assert_eq!(visible_left.payload, b"rotated-a");
    assert_eq!(visible_left.execution_count, 1);
    assert_eq!(visible_right.payload, b"old");
    assert_eq!(visible_right.execution_count, 0);
    assert_eq!(visible_right.terminal_receipts, 1);
    left.assert_no_secret();
    right.assert_no_secret();
}
