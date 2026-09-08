//! Public integration boundaries for the declared FA-002 perimeter inventory.
//!
//! These tests exercise only the in-memory declaration and lookup API.  A
//! passing inventory records neither a live credential nor a prevented bypass;
//! it is not evidence of a broker, endpoint, OS boundary, or complete deployed
//! perimeter.

use fa_reference::Error;
use fa_reference::perimeter::{
    BypassDisposition, CredentialExposure, CredentialHolder, EffectFamilyRecord,
    MAX_CREDENTIALS_PER_FAMILY, MAX_FAMILIES, MAX_FAMILY_TEXT_BYTES, MAX_RESIDUAL_NONCLAIMS,
    MAX_ROUTES_PER_FAMILY, MAX_TEXT_BYTES, Mediation, PerimeterInventory, PerimeterScope,
    RouteRecord, ThreatClass, TrustDomain,
};

fn scope() -> PerimeterScope {
    PerimeterScope {
        tenant: 41,
        principal: 42,
        purpose: 43,
    }
}

fn trust_domains() -> Vec<TrustDomain> {
    vec![
        TrustDomain::Actor,
        TrustDomain::ObservationAndAnalysis,
        TrustDomain::Enforcement,
        TrustDomain::GovernanceAndInvestigation,
    ]
}

fn record(family: &str, mediation: Mediation) -> EffectFamilyRecord {
    EffectFamilyRecord {
        scope: scope(),
        family: family.to_owned(),
        trust_domains: trust_domains(),
        credentials: if mediation == Mediation::BrokeredEffects {
            vec![CredentialExposure {
                credential: "broker-only-publication".to_owned(),
                holder: CredentialHolder::Broker,
            }]
        } else {
            Vec::new()
        },
        routes: vec![RouteRecord {
            route: "adapter:disposable".to_owned(),
            threat: Some(ThreatClass::DirectCredentialOrEgress),
            mediation,
            bypass: BypassDisposition::Blocked,
        }],
        residual_nonclaims: vec!["No endpoint or enforcement is represented here".to_owned()],
    }
}

fn brokered_control() -> EffectFamilyRecord {
    record("publication", Mediation::BrokeredEffects)
}

#[test]
fn exact_complete_brokered_declaration_is_returned_only_for_its_declared_scope_family_and_route() {
    let inventory = PerimeterInventory::new(vec![brokered_control()]).unwrap();

    assert_eq!(
        inventory
            .mediation_for(scope(), "publication", "adapter:disposable")
            .unwrap(),
        Mediation::BrokeredEffects
    );
    assert_eq!(inventory.records().len(), 1);
    assert_eq!(inventory.records()[0].scope, scope());
}

#[test]
fn observe_only_and_cooperative_declarations_are_not_upgraded_to_brokered() {
    for mediation in [Mediation::ObserveOnly, Mediation::CooperativeGate] {
        let inventory = PerimeterInventory::new(vec![record("publication", mediation)]).unwrap();
        assert_eq!(
            inventory
                .mediation_for(scope(), "publication", "adapter:disposable")
                .unwrap(),
            mediation
        );
    }
}

#[test]
fn nonbrokered_residual_bypass_is_preserved_but_unmodeled_bypass_refuses() {
    for mediation in [Mediation::ObserveOnly, Mediation::CooperativeGate] {
        let control = record("publication", mediation);
        assert!(PerimeterInventory::new(vec![control.clone()]).is_ok());

        let mut residual = control.clone();
        residual.routes[0].bypass = BypassDisposition::ResidualUncovered;
        let inventory = PerimeterInventory::new(vec![residual]).unwrap();
        assert_eq!(
            inventory
                .route_for(scope(), "publication", "adapter:disposable")
                .unwrap()
                .bypass,
            BypassDisposition::ResidualUncovered,
            "{mediation:?} residual declaration changed"
        );

        let mut unmodeled = control;
        unmodeled.routes[0].bypass = BypassDisposition::Unmodeled;
        assert_eq!(
            PerimeterInventory::new(vec![unmodeled]),
            Err(Error::Incomplete),
            "{mediation:?} must not retain an unmodeled bypass"
        );
    }
}

#[test]
fn actor_direct_credential_refuses_next_to_the_broker_holder_control() {
    let control = brokered_control();
    assert!(PerimeterInventory::new(vec![control.clone()]).is_ok());

    let mut direct = control;
    direct.credentials[0].holder = CredentialHolder::ActorDirect;
    assert_eq!(PerimeterInventory::new(vec![direct]), Err(Error::Binding));
}

#[test]
fn actor_direct_credential_refuses_across_the_whole_brokered_family_not_only_one_route() {
    let mut control = brokered_control();
    control.routes.push(RouteRecord {
        route: "adapter:cooperative-view".to_owned(),
        threat: Some(ThreatClass::PromptInjection),
        mediation: Mediation::CooperativeGate,
        bypass: BypassDisposition::ResidualUncovered,
    });
    assert!(PerimeterInventory::new(vec![control.clone()]).is_ok());

    let mut direct = control;
    direct.credentials.push(CredentialExposure {
        credential: "actor-visible-family-credential".to_owned(),
        holder: CredentialHolder::ActorDirect,
    });
    assert_eq!(PerimeterInventory::new(vec![direct]), Err(Error::Binding));
}

#[test]
fn missing_threat_and_unmodeled_bypass_each_refuse_the_same_complete_route() {
    let control = brokered_control();
    assert!(PerimeterInventory::new(vec![control.clone()]).is_ok());

    let mut absent_threat = control.clone();
    absent_threat.routes[0].threat = None;
    assert_eq!(
        PerimeterInventory::new(vec![absent_threat]),
        Err(Error::Incomplete)
    );

    let mut unmodeled_bypass = control;
    unmodeled_bypass.routes[0].bypass = BypassDisposition::Unmodeled;
    assert_eq!(
        PerimeterInventory::new(vec![unmodeled_bypass]),
        Err(Error::Binding)
    );
}

#[test]
fn unknown_route_family_and_each_wrong_nonzero_scope_component_refuse_exact_lookup() {
    let inventory = PerimeterInventory::new(vec![brokered_control()]).unwrap();
    assert_eq!(
        inventory.mediation_for(scope(), "publication", "adapter:unknown"),
        Err(Error::Missing)
    );
    assert_eq!(
        inventory.mediation_for(scope(), "unknown-family", "adapter:disposable"),
        Err(Error::Missing)
    );

    for (name, changed_scope) in [
        (
            "tenant",
            PerimeterScope {
                tenant: scope().tenant + 1,
                ..scope()
            },
        ),
        (
            "principal",
            PerimeterScope {
                principal: scope().principal + 1,
                ..scope()
            },
        ),
        (
            "purpose",
            PerimeterScope {
                purpose: scope().purpose + 1,
                ..scope()
            },
        ),
    ] {
        assert_eq!(
            inventory.mediation_for(changed_scope, "publication", "adapter:disposable"),
            Err(Error::Missing),
            "wrong {name} must not select the declared record"
        );
    }
}

#[test]
fn duplicate_family_route_and_credential_each_refuse_without_reinterpreting_the_control() {
    let control = brokered_control();
    assert!(PerimeterInventory::new(vec![control.clone()]).is_ok());

    assert_eq!(
        PerimeterInventory::new(vec![control.clone(), control.clone()]),
        Err(Error::Duplicate)
    );

    let mut duplicate_route = control.clone();
    duplicate_route
        .routes
        .push(duplicate_route.routes[0].clone());
    assert_eq!(
        PerimeterInventory::new(vec![duplicate_route]),
        Err(Error::Duplicate)
    );

    let mut duplicate_credential = control;
    duplicate_credential
        .credentials
        .push(duplicate_credential.credentials[0].clone());
    assert_eq!(
        PerimeterInventory::new(vec![duplicate_credential]),
        Err(Error::Duplicate)
    );
}

#[test]
fn missing_or_duplicate_trust_domain_and_empty_residual_refuse_complete_brokered_record() {
    let control = brokered_control();
    assert!(PerimeterInventory::new(vec![control.clone()]).is_ok());

    let mut missing_domain = control.clone();
    missing_domain.trust_domains.pop();
    assert_eq!(
        PerimeterInventory::new(vec![missing_domain]),
        Err(Error::Incomplete)
    );

    let mut duplicate_domain = control.clone();
    duplicate_domain.trust_domains[3] = TrustDomain::Actor;
    assert_eq!(
        PerimeterInventory::new(vec![duplicate_domain]),
        Err(Error::Incomplete)
    );

    let mut empty_residual = control;
    empty_residual.residual_nonclaims.clear();
    assert_eq!(
        PerimeterInventory::new(vec![empty_residual]),
        Err(Error::Incomplete)
    );
}

#[test]
fn each_declared_count_cap_is_accepted_at_the_boundary_and_family_cap_refuses_one_more() {
    let families: Vec<_> = (0..MAX_FAMILIES)
        .map(|index| record(&format!("family-{index}"), Mediation::BrokeredEffects))
        .collect();
    let inventory = PerimeterInventory::new(families).unwrap();
    assert_eq!(inventory.records().len(), MAX_FAMILIES);
    assert_eq!(
        inventory
            .mediation_for(
                scope(),
                &format!("family-{}", MAX_FAMILIES - 1),
                "adapter:disposable"
            )
            .unwrap(),
        Mediation::BrokeredEffects
    );

    let too_many: Vec<_> = (0..=MAX_FAMILIES)
        .map(|index| record(&format!("extra-family-{index}"), Mediation::BrokeredEffects))
        .collect();
    assert_eq!(PerimeterInventory::new(too_many), Err(Error::Limit));

    let mut at_route_credential_and_residual_caps = brokered_control();
    at_route_credential_and_residual_caps.routes = (0..MAX_ROUTES_PER_FAMILY)
        .map(|index| RouteRecord {
            route: format!("adapter:route-{index}"),
            threat: Some(ThreatClass::DirectCredentialOrEgress),
            mediation: Mediation::BrokeredEffects,
            bypass: BypassDisposition::Blocked,
        })
        .collect();
    at_route_credential_and_residual_caps.credentials = (0..MAX_CREDENTIALS_PER_FAMILY)
        .map(|index| CredentialExposure {
            credential: format!("broker-{index}"),
            holder: CredentialHolder::Broker,
        })
        .collect();
    at_route_credential_and_residual_caps.residual_nonclaims = (0..MAX_RESIDUAL_NONCLAIMS)
        .map(|index| format!("declared-residual-{index}"))
        .collect();
    let at_cap =
        PerimeterInventory::new(vec![at_route_credential_and_residual_caps.clone()]).unwrap();
    assert_eq!(
        at_cap
            .mediation_for(
                scope(),
                "publication",
                &format!("adapter:route-{}", MAX_ROUTES_PER_FAMILY - 1)
            )
            .unwrap(),
        Mediation::BrokeredEffects
    );

    let mut too_many_routes = at_route_credential_and_residual_caps.clone();
    too_many_routes.routes.push(RouteRecord {
        route: "adapter:one-too-many".to_owned(),
        threat: Some(ThreatClass::DirectCredentialOrEgress),
        mediation: Mediation::BrokeredEffects,
        bypass: BypassDisposition::Blocked,
    });
    assert_eq!(
        PerimeterInventory::new(vec![too_many_routes]),
        Err(Error::Limit)
    );

    let mut too_many_credentials = at_route_credential_and_residual_caps.clone();
    too_many_credentials.credentials.push(CredentialExposure {
        credential: "broker-one-too-many".to_owned(),
        holder: CredentialHolder::Broker,
    });
    assert_eq!(
        PerimeterInventory::new(vec![too_many_credentials]),
        Err(Error::Limit)
    );

    let mut too_many_residuals = at_route_credential_and_residual_caps;
    too_many_residuals
        .residual_nonclaims
        .push("one residual declaration too many".to_owned());
    assert_eq!(
        PerimeterInventory::new(vec![too_many_residuals]),
        Err(Error::Limit)
    );
}

#[test]
fn aggregate_text_limit_refuses_individually_valid_credential_and_residual_strings() {
    let control = brokered_control();
    assert!(PerimeterInventory::new(vec![control.clone()]).is_ok());

    let mut aggregate = control;
    aggregate.credentials = (0..MAX_CREDENTIALS_PER_FAMILY)
        .map(|index| CredentialExposure {
            credential: format!("{index:02}{}", "c".repeat(MAX_TEXT_BYTES - 2)),
            holder: CredentialHolder::Broker,
        })
        .collect();
    aggregate.residual_nonclaims = (0..MAX_RESIDUAL_NONCLAIMS)
        .map(|index| format!("{index:02}{}", "r".repeat(MAX_TEXT_BYTES - 2)))
        .collect();

    assert!(
        aggregate
            .credentials
            .iter()
            .all(|credential| credential.credential.len() <= MAX_TEXT_BYTES)
    );
    assert!(
        aggregate
            .residual_nonclaims
            .iter()
            .all(|nonclaim| nonclaim.len() <= MAX_TEXT_BYTES)
    );
    let text_bytes = aggregate.family.len()
        + aggregate
            .routes
            .iter()
            .map(|route| route.route.len())
            .sum::<usize>()
        + aggregate
            .credentials
            .iter()
            .map(|credential| credential.credential.len())
            .sum::<usize>()
        + aggregate
            .residual_nonclaims
            .iter()
            .map(String::len)
            .sum::<usize>();
    let excess = text_bytes.checked_sub(MAX_FAMILY_TEXT_BYTES).unwrap();
    assert!(excess > 0);
    let mut at_cap = aggregate.clone();
    let last = at_cap.residual_nonclaims.last_mut().unwrap();
    last.truncate(last.len().checked_sub(excess).unwrap());
    assert!(PerimeterInventory::new(vec![at_cap.clone()]).is_ok());
    at_cap.residual_nonclaims.last_mut().unwrap().push('x');
    assert_eq!(PerimeterInventory::new(vec![at_cap]), Err(Error::Limit));
    assert_eq!(PerimeterInventory::new(vec![aggregate]), Err(Error::Limit));
}
