//! Finite original disclosure custody after independently anchored promotion.
use super::*;
use crate::workflow::actor_service::precheck::schedule::Schedule;
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    PrecheckRequest, PrecheckWireError, client::PrecheckPeerClient,
};

pub(super) fn schedule(root: &Directory) -> Schedule {
    schedule_selected(root, Some(2), 2)
}

pub(in super::super) fn schedule_selected(
    root: &Directory,
    first: Option<u64>,
    second: u64,
) -> Schedule {
    let profiles: [PathBuf; 2] =
        std::array::from_fn(|index| root.0.join(format!("precheck-{}.json", index + 7)));
    for (index, path) in profiles.iter().enumerate() {
        fs::write(path, format!(
            r#"{{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"{}","expected_policy_epoch":{},"disclosure_id":{},"query_limit":1,"exchange_limit":4,"candidate_limit":4,"connection_limit":2,"runtime_ms":15000}}"#,
            root.0.join(format!("actors/precheck-{}.sock", index + 7)).display(),
            if index == 0 { first.unwrap_or(second) } else { second }, 81 + index,
        )).unwrap();
    }
    let path = root.0.join("precheck-schedule.json");
    fs::write(&path, format!(
        r#"{{"schema":"fa.actor-precheck-schedule/1","entries":[{{"request":7,"precheck_profile":{}}},{{"request":8,"precheck_profile":"{}"}}]}}"#,
        if first.is_some() { format!("\"{}\"", profiles[0].display()) } else { "null".into() },
        profiles[1].display(),
    )).unwrap();
    let selected = Schedule::read(&path, &[7, 8]).unwrap();
    // A later row must use the already admitted independent selection, never
    // reload edited profile bytes or invent epoch three after promotion.
    fs::write(&profiles[1], b"changed after schedule admission").unwrap();
    selected
}

pub(in super::super) fn proposal(
    config: &Config,
    version: u64,
    payload: &[u8],
    epoch: u64,
) -> ActorProposal {
    let mut target = config.profile.delivery.target;
    target.expected_version = version;
    ActorProposal {
        target,
        payload: payload.to_vec(),
        units: 7,
        deadline: ElapsedTick(100000),
        expected_policy_epoch: epoch,
    }
}

pub(in super::super) fn document(
    config: &Config,
    version: u64,
    payload: &[u8],
    epoch: u64,
) -> String {
    let scope = config.profile.delivery.scope;
    let mut request = TowerRequest::new(
        "recovered-precheck",
        Address::parse(&format!(
            "fa://{}/run/{}@{GENERATION}",
            scope.tenant, scope.run
        ))
        .unwrap(),
        Verb::Precheck,
    );
    request.arguments = ReadArguments::Precheck {
        query: 9001,
        proposal: proposal(config, version, payload, epoch),
    };
    encode_request(&request).unwrap()
}

fn knowledge(reply: &Json) -> &Json {
    let result = field(reply, "result");
    assert_eq!(field(result, "kind").as_str(), Some("precheck"));
    field(field(result, "value"), "knowledge")
}

pub(in super::super) fn passed(reply: &Json, disclosure: &str, epoch: &str) {
    let knowledge = knowledge(reply);
    assert_eq!(field(knowledge, "state").as_str(), Some("known"));
    assert_eq!(field(knowledge, "value").as_str(), Some("precheck_passed"));
    assert_eq!(field(knowledge, "review_required").as_bool(), Some(true));
    let basis = field(knowledge, "basis");
    assert_eq!(field(basis, "disclosure").as_str(), Some(disclosure));
    assert_eq!(field(basis, "policy_epoch").as_str(), Some(epoch));
}

pub(in super::super) fn withheld(reply: &Json) {
    let knowledge = knowledge(reply);
    assert_eq!(field(knowledge, "state").as_str(), Some("withheld"));
    assert_eq!(
        field(knowledge, "authority_required").as_str(),
        Some("exact_predicates")
    );
}

pub(in super::super) fn exhausted(profile: &Profile, socket: &Path, proposal: ActorProposal) {
    wait(socket);
    let epoch = proposal.expected_policy_epoch;
    let mut client = PrecheckPeerClient::connect(
        UnixStream::connect(socket).unwrap(),
        profile.supervisor,
        PrecheckRequest {
            query: 9001,
            proposal,
        },
        epoch,
    )
    .unwrap();
    let started = Instant::now();
    loop {
        assert!(started.elapsed() < Duration::from_secs(10));
        let report = client
            .drive(
                fa_reference::action::consequence::oversight::actor_transport::DriveBudget::default(
                ),
            )
            .unwrap();
        assert!(report.status.failure.is_none(), "{:?}", report.status);
        if let Some(response) = client.take_response() {
            assert_eq!(response.result, Err(PrecheckWireError::Capacity));
            return;
        }
        assert!(!report.status.closed());
        pause(1);
    }
}
