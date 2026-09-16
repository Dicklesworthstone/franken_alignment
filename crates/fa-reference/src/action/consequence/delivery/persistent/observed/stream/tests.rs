//! Bootstrap and codec controls for the original durable stream owner.
use super::*;
use crate::action::{Purpose, Scope};
use crate::action::consequence::congress::{CongressPolicy, MemberPolicy};
use crate::action::consequence::delivery::persistent::{FileDeliveryProfile, JournalLimits};
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile};
use crate::action::consequence::gate::containment::session::policy::{Policy, Predicate};
use crate::action::consequence::oversight::{CommitteeContract, HelperContract, human::HumanReviewPolicy};
use crate::full_input::InputProfileBinding;
use crate::reducer::Caps;
use std::collections::BTreeMap;

fn profile() -> FileOversightProfile {
    let target = ResolvedTarget { adapter: 10, object: 11, contract_version: 1, expected_version: 1, generation: 1 };
    FileOversightProfile {
        delivery: FileDeliveryProfile {
            scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
            total: 4096, max_attempts: 8,
            actor: ActorState::new(RestartProfile { id: 1, generation: 1, host_generation: 1,
                model_generation: 1, tokenizer_generation: 1, state_schema_generation: 1,
                grade: RestartGrade::FunctionalRestart }, vec![1], vec![2], vec![3], 1).unwrap(),
            suspend_at_incident: 3,
            policy: Policy::new(1, vec![Predicate::ExactValue { key: 7, value: b"ok".to_vec() }]).unwrap(),
            congress: CongressPolicy { generation: 1, members: BTreeMap::from([("reviewer".to_owned(),
                MemberPolicy { cohort: "reference".to_owned(), weight: 1 })]),
                caps: Caps { per_member: 1, per_cohort: 1 }, continue_minimum: 1, continue_hold_maximum: 0,
                narrow_at: 2, suspend_at: 3, minimum_members: 1, minimum_cohorts: 1 },
            narrowed_targets: vec![target], target, initial_payload: Vec::new(),
            retention_ticks: 1000, max_deliveries: 8, clock_domain: 99, limits: JournalLimits::default(),
        },
        committee: CommitteeContract::new(BTreeMap::from([("reviewer".to_owned(), HelperContract::new(
            InputProfileBinding { profile_id: 1, profile_bytes: b"exact-view-v1".to_vec(),
                tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 }, 7, b"approve?".to_vec(),
        ).unwrap())])).unwrap(),
        human: HumanReviewPolicy { reviewer_id: 77, max_validity_ticks: 50, max_requests: 16 },
    }
}
fn stream() -> StreamProfile { StreamProfile::new(9, 1, 4, 16, 64).unwrap() }

#[test]
fn a_stream_marker_cannot_reset_a_clock_guard_fence_or_existing_stream() {
    let p = profile();
    for first in [Event::Core(BaseEvent::Time(ElapsedTick(1))), Event::PublicationGuard,
        Event::Core(BaseEvent::Fence), Event::StreamBootstrap(stream())]
    {
        let mut machine = Machine::new(&p).unwrap();
        machine.apply(&first).unwrap();
        let before = machine.snapshot(1);
        assert!(matches!(machine.apply(&Event::StreamBootstrap(stream())), Err(Error::WrongState)));
        assert_eq!(machine.snapshot(1), before);
    }
    let machine = Machine::replay(&p, &[Event::StreamBootstrap(stream())]).unwrap();
    assert!(machine.publication_guard);
    assert_eq!(machine.stream_snapshot(1).unwrap().published, StreamView::empty(stream()));
}

#[test]
fn stream_bootstrap_preserves_native_capacity_and_nonempty_replacement_refusals() {
    let mut p = profile(); p.delivery.initial_payload = b"not reviewed".to_vec();
    assert!(matches!(Machine::replay(&p, &[Event::StreamBootstrap(stream())]), Err(Error::Binding)));
    let mut p = profile(); p.delivery.max_deliveries = 4;
    assert!(matches!(Machine::replay(&p, &[Event::StreamBootstrap(stream())]), Err(Error::Limit)));
    p.delivery.max_deliveries = 5;
    assert!(Machine::replay(&p, &[Event::StreamBootstrap(stream())]).is_ok());
}

#[test]
fn original_profile_constructor_controls_codec_bounds_and_truncation() {
    for profile in [stream(), StreamProfile::new(u64::MAX, u64::MAX,
        MAX_STREAM_MESSAGES, MAX_MESSAGE_BYTES, MAX_STREAM_BYTES).unwrap()]
    {
        let mut w = Writer::new(28); write_profile(&mut w, profile).unwrap();
        let bytes = w.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(read_profile(&mut reader).unwrap(), profile);
        reader.end().unwrap();
        for end in 0..bytes.len() { assert!(read_profile(&mut Reader::new(&bytes[..end])).is_err()); }
        let mut zero = bytes.clone(); zero[..8].fill(0);
        assert_eq!(read_profile(&mut Reader::new(&zero)), Err(Error::InvalidInput));
        for (offset, limit) in [(16, MAX_STREAM_MESSAGES), (20, MAX_MESSAGE_BYTES), (24, MAX_STREAM_BYTES)] {
            let mut changed = bytes.clone(); changed[offset..offset + 4].copy_from_slice(&((limit + 1) as u32).to_be_bytes());
            assert_eq!(read_profile(&mut Reader::new(&changed)), Err(Error::Limit));
        }
    }
}
