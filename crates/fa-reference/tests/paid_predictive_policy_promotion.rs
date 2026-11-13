//! A numerical replay allowance cannot replace a predictor's policy binding.
//! Real request-keyed forecasts and their native likelihood observations remain
//! intact when the original paid promotion port refuses this unsupported lane.

#[path = "support/file_consistency.rs"]
mod prediction;
#[path = "support/file_oversight.rs"]
mod support;

use fa_reference::action::consequence::delivery::persistent::{
    governance::PolicyUpdate,
    observed::FileOversight,
    requests::{
        FileRequestDisposition,
        actor::{
            FilePolicyPromotionBlockReason, FilePolicyPromotionError, FilePolicyPromotionRequest,
            MAX_POLICY_PROMOTION_REPLAY_EVENTS,
        },
    },
};
use fa_reference::action::consequence::gate::containment::session::policy::Policy;
use fa_reference::action::consequence::oversight::actor::{ActorProposal, Knowledge};
use fa_reference::action::consequence::policy_campaign::ReplayLimits;
use fa_reference::action::{ActionState, ElapsedTick};
use support::{Directory, snapshot};

const FIRST: u64 = 701;
const NEXT: u64 = 702;
const CAMPAIGN: u64 = 901;
const OPERATION: u64 = 1001;

#[test]
fn paid_promotion_refuses_required_predictor_without_consuming_its_next_forecast_or_snapshot() {
    let root = Directory::new();
    let profile = support::profile();
    let (mut host, _human) = FileOversight::create(root.store(), profile.clone()).unwrap();
    let governor = host
        .enable_policy_campaigns(
            host.revision(),
            ReplayLimits {
                cases: 16,
                input_bytes: 1_048_576,
            },
            8,
        )
        .unwrap();
    let observer = host
        .enable_action_consistency(host.revision(), prediction::configuration())
        .unwrap();
    host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
    let frame = prediction::frame(&host, 1, 1.0);
    let revision = host.revision();
    let actor_revision = host.actor_snapshot().unwrap().actor_revision;
    observer
        .forecast_request(&mut host, revision, FIRST, actor_revision, &frame)
        .unwrap()
        .unwrap();
    let mut action = support::spec(&host, b"risk: first original observation");
    // The reused fixture defaults to sixteen units; the ORIGINAL delivery
    // resource check requires the complete actual payload's byte charge.
    action.units = action.payload.len() as u64;
    host.submit_request(host.revision(), FIRST, action, snapshot())
        .unwrap();
    let first = host.request_status(FIRST).unwrap();
    let FileRequestDisposition::Admitted {
        attempt: first_attempt,
        ..
    } = first.disposition
    else {
        panic!(
            "the first forecast must enter the original request history: {first:?}; predictor={:?}",
            host.action_consistency_snapshot().unwrap()
        );
    };
    assert_eq!(
        host.inspect().control.ledger.stages[&first_attempt],
        ActionState::Reviewing
    );
    assert!(
        host.action_consistency_observation(first_attempt)
            .unwrap()
            .event()
    );
    assert_eq!(
        host.action_consistency_snapshot()
            .unwrap()
            .evidence
            .samples(),
        1
    );
    assert!(!host.decoder_required());
    assert!(host.action_consistency_required());

    // Retain an actual fresh forecast before the campaign. Neither a pending
    // prediction nor numerical payment grants publication permission.
    let next_frame = prediction::frame(&host, 2, 1.0);
    let revision = host.revision();
    let actor_revision = host.actor_snapshot().unwrap().actor_revision;
    observer
        .forecast_request(&mut host, revision, NEXT, actor_revision, &next_frame)
        .unwrap()
        .unwrap();
    assert_eq!(host.pending_forecast_request().unwrap(), Some(NEXT));
    let current = host.inspect();
    let candidate = Policy::new(2, profile.delivery.policy.nodes().to_vec()).unwrap();
    let update = PolicyUpdate::new(
        CAMPAIGN,
        current.control.sequence,
        current.control.ledger.epoch,
        candidate,
    )
    .unwrap();
    let review = host
        .request_policy_campaign(host.revision(), &update)
        .unwrap();
    assert!(!review.report().cases().is_empty());
    assert!(review.report().newly_reviewable().is_empty());
    let revision = host.revision();
    let permit = governor
        .approve(&mut host, revision, &review, false)
        .unwrap();
    let (actor, mut supervisor) = host.into_actor_gateway();
    let before = supervisor.host().unwrap().inspect();
    supervisor
        .set_snapshot(before.revision, Some(snapshot()))
        .unwrap();
    let (port, grant) = supervisor
        .policy_promotion_port_with_numerical_budget(
            permit,
            4,
            MAX_POLICY_PROMOTION_REPLAY_EVENTS,
            MAX_POLICY_PROMOTION_REPLAY_EVENTS,
        )
        .unwrap();
    let forecast_before = supervisor
        .host()
        .unwrap()
        .action_consistency_snapshot()
        .unwrap();
    assert!(!forecast_before.coverage_lost);
    let canonical = std::fs::read(root.store().join("delivery.bin")).unwrap();
    let next = port.next(CAMPAIGN, OPERATION).unwrap();
    let Knowledge::Known { value, basis } = next else {
        panic!("the native campaign must remain inspectable: {next:?}");
    };
    assert_eq!(value.basis(), basis);
    assert_eq!(basis.scope, profile.delivery.scope);
    assert_eq!(basis.journal_revision, before.revision);
    assert_eq!(
        value.block_reason(),
        Some(FilePolicyPromotionBlockReason::NumericalReplayUnsupported)
    );
    assert!(value.request().is_none());
    let request = FilePolicyPromotionRequest::new(OPERATION, basis).unwrap();
    assert_eq!(
        port.clone().promote(request),
        Err(FilePolicyPromotionError::Unsupported)
    );
    assert_eq!(grant.remaining_queries(), 3);
    assert_eq!(grant.remaining_operations(), 1);
    assert_eq!(
        grant.remaining_numerical_replay_events(),
        Some(MAX_POLICY_PROMOTION_REPLAY_EVENTS)
    );
    {
        let host = supervisor.host().unwrap();
        assert_eq!(host.inspect(), before);
        assert_eq!(host.action_consistency_snapshot().unwrap(), forecast_before);
        assert_eq!(host.pending_forecast_request().unwrap(), Some(NEXT));
        assert_eq!(host.current_policy().unwrap(), &profile.delivery.policy);
        assert_eq!(
            std::fs::read(root.store().join("delivery.bin")).unwrap(),
            canonical
        );
    }

    // The same native request-keyed forecast and untouched one-shot observation
    // remain usable. Do not refresh either to hide preflight side effects.
    let proposal = {
        let host = supervisor.host().unwrap();
        let mut spec = support::spec(&host, b"risk: second original observation");
        spec.units = spec.payload.len() as u64;
        ActorProposal {
            target: spec.target.unwrap(),
            payload: spec.payload,
            units: spec.units,
            deadline: spec.deadline,
            expected_policy_epoch: spec.policy_epoch,
        }
    };
    let ticket = actor.submit(NEXT, &proposal).unwrap();
    assert!(matches!(
        actor.poll(&ticket),
        Knowledge::Pending { request: NEXT }
    ));
    let host = supervisor.host().unwrap();
    let FileRequestDisposition::Admitted { attempt, .. } =
        host.request_status(NEXT).unwrap().disposition
    else {
        panic!("the retained forecast and snapshot must admit the original successor");
    };
    assert_eq!(
        host.action_consistency_snapshot()
            .unwrap()
            .evidence
            .samples(),
        2
    );
    assert!(
        host.action_consistency_observation(attempt)
            .unwrap()
            .event()
    );
    assert_eq!(host.pending_forecast_request().unwrap(), None);
    assert!(!host.action_consistency_snapshot().unwrap().coverage_lost);
    assert_eq!(host.inspect().executions, 0);
    assert_eq!(host.inspect().control.ledger.charged, 0);
    assert_eq!(host.inspect().control.ledger.reserved, 0);
}
