use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::{
    MAX_POLICY_PROMOTION_QUERIES, MAX_POLICY_PROMOTION_REPLAY_EVENTS,
};

fn selected(profile: &str, queries: usize, events: usize, authority: &str) -> String {
    profile.replacen('{', &format!("{{\"policy_promotion\":{{\"queries\":{queries},\"max_replay_events\":{events},\"authority\":\"{authority}\"}},"), 1)
}

#[test]
fn promotion_selection_is_independent_finite_and_cannot_enable_a_numerical_operator() {
    let fixture = Fixture::new(1, 0);
    let profile = profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0);
    let valid = selected(&profile, 2, 1024, "promote_approved_policy");
    let settings = Settings::decode(valid.as_bytes()).unwrap();
    assert!(settings.policy_promotion.is_some());
    assert!(settings.check_generated_operator().is_err());
    assert!(settings.check_learned_operator().is_err());
    for (queries, events, authority) in [
        (0, 1024, "promote_approved_policy"),
        (
            MAX_POLICY_PROMOTION_QUERIES + 1,
            1024,
            "promote_approved_policy",
        ),
        (2, 0, "promote_approved_policy"),
        (
            2,
            MAX_POLICY_PROMOTION_REPLAY_EVENTS + 1,
            "promote_approved_policy",
        ),
        (2, 1024, "approve_policy"),
        (2, 1024, "compare_policy"),
    ] {
        assert!(
            Settings::decode(selected(&profile, queries, events, authority).as_bytes()).is_err()
        );
    }
    assert!(
        Settings::decode(
            valid
                .replace("\"queries\":2", "\"queries\":2,\"operations\":2")
                .as_bytes()
        )
        .is_err()
    );
}

#[test]
fn selected_operator_profile_alone_never_manufactures_a_campaign_or_governor() {
    let mut fixture = Fixture::new(1, 0);
    fixture.settings = Settings::decode(
        selected(
            &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
            2,
            1024,
            "promote_approved_policy",
        )
        .as_bytes(),
    )
    .unwrap();
    let saved = fixture.files();
    let mut service = fixture.start();
    assert!(service.promotion_available());
    assert!(service.active.as_ref().unwrap().policy_promotion.is_none());
    assert!(service.governor.is_none());
    let mut request = fixture.request(Verb::Next);
    request.arguments = ReadArguments::PolicyPromotionNext {
        campaign: 41,
        operation: 42,
    };
    let reply = fixture.query(&mut service, request);
    assert_eq!(
        field(&reply, &["envelope", "reason_code"]).as_str(),
        Some("CapabilityUnsupported")
    );
    assert_eq!(fixture.files(), saved);
    assert!(
        !fixture
            .driver
            .supervisor()
            .host()
            .unwrap()
            .policy_campaigns_required()
    );
}

#[test]
fn disconnected_exhausted_operator_refuses_delegation_before_its_next_retirement_checkpoint() {
    for connections in [1, 2] {
        let mut fixture = Fixture::new(1, 0);
        fixture.settings = Settings::decode(
            selected(
                &profile_json(&fixture.settings.socket, fixture.settings.operator, 0, 0),
                2,
                1024,
                "promote_approved_policy",
            )
            .as_bytes(),
        )
        .unwrap();
        fixture.settings.candidate_limit = connections;
        fixture.settings.connection_limit = connections;
        let saved = fixture.files();
        let mut service = fixture.start();
        let socket = UnixStream::connect(&fixture.settings.socket).unwrap();
        fixture.settings.supervisor.verify(&socket).unwrap();
        for _ in 0..64 {
            service.checkpoint();
            if service
                .active
                .as_ref()
                .unwrap()
                .session
                .status()
                .active
                .is_some()
            {
                break;
            }
        }
        let active = service.active.as_ref().unwrap();
        assert!(active.session.status().active.is_some());
        assert_eq!(active.session.status().connections_admitted, 1);
        let exchanges = active.session.remaining_exchanges();
        // The final permitted connection is still a usable original ingress
        // while attached; reaching the connection count alone cannot revoke it.
        assert!(service.promotion_available());
        socket.shutdown(Shutdown::Both).unwrap();
        drop(socket);
        for _ in 0..64 {
            service.checkpoint();
            if service
                .active
                .as_ref()
                .unwrap()
                .session
                .status()
                .active
                .is_none()
            {
                break;
            }
        }
        // Stop before the following checkpoint can retire the service. This is
        // the exact gap where a governor approval used to lose its new key.
        assert!(service.active.is_some());
        assert!(service.retirement.is_none());
        let active = service.active.as_ref().unwrap();
        assert!(active.session.status().active.is_none());
        assert_eq!(active.session.status().connections_admitted, 1);
        assert_eq!(active.session.remaining_exchanges(), exchanges);
        assert_eq!(service.candidates, 1);
        assert_eq!(service.promotion_available(), connections == 2);
        assert!(active.policy_promotion.is_none());
        assert_eq!(fixture.files(), saved);

        if connections == 2 {
            let socket = UnixStream::connect(&fixture.settings.socket).unwrap();
            fixture.settings.supervisor.verify(&socket).unwrap();
            for _ in 0..64 {
                service.checkpoint();
                if service
                    .active
                    .as_ref()
                    .unwrap()
                    .session
                    .status()
                    .active
                    .is_some()
                {
                    break;
                }
            }
            let active = service.active.as_ref().unwrap();
            assert!(active.session.status().active.is_some());
            assert_eq!(active.session.status().connections_admitted, 2);
            assert_eq!(active.session.remaining_exchanges(), exchanges);
            assert!(service.promotion_available());
            assert_eq!(fixture.files(), saved);
        }
    }
}
