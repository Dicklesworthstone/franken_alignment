//! Public model-liveness controls through the original stream/oversight ledger.
//! Captures are supplied fixtures, not execution of stimuli by a real model.

#[path = "support/stream_fixture.rs"]
mod support;

use support::*;
use fa_reference::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame};
use fa_reference::action::consequence::activation::identity::{IdentityAnchor, ModelManifest, ModelPassport};
use fa_reference::action::consequence::gate::{ReviewBinding, TargetCeiling};
use fa_reference::action::consequence::gate::containment::{ActorState, ResetRequest, RestartGrade, RestartProfile};
use fa_reference::action::consequence::oversight::{ObservedReview, OversightBroker, ReviewWindow};
use fa_reference::action::consequence::oversight::identity::{
    IdentityChallenge, IdentityMismatch, IdentityObserver, IdentityOutcome, IdentityPolicy, IdentityStatus,
};
use fa_reference::action::consequence::oversight::human::{HumanDisposition, HumanReviewPolicy};
use fa_reference::action::{ActionState, ElapsedTick};
use fa_reference::round::Verdict;
use fa_reference::Error;

fn passport() -> ModelPassport {
    let manifest = ModelManifest {
        tenant: 1, model: 2, model_generation: 1, host_generation: 1, tokenizer_generation: 1,
        weights: [1; 32], adapters: [2; 32], tokenizer: [3; 32],
        architecture: [4; 32], numeric_profile: [5; 32],
    };
    ModelPassport::new(9, 1, manifest, vec![
        IdentityAnchor::new(11, CaptureProfile { tenant: 1, model: 2, model_generation: 1,
            tap: 4, layout_generation: 5 }, 6, vec![101], &[[1.0, 2.0]]).unwrap(),
        IdentityAnchor::new(22, CaptureProfile { tenant: 1, model: 2, model_generation: 1,
            tap: 7, layout_generation: 5 }, 8, vec![102, 103], &[[3.0, 4.0]]).unwrap(),
    ]).unwrap()
}

fn enable(broker: &mut OversightBroker, max_checks: usize) -> IdentityObserver {
    broker.enable_identity_checks(passport(), IdentityPolicy {
        observer_id: 17, timeout_ticks: 5, validity_ticks: 30, max_checks,
    }).unwrap()
}

fn begin(broker: &mut OversightBroker, id: u64) -> IdentityChallenge {
    broker.begin_identity_check(id, broker.inspect().sequence, broker.actor_revision()).unwrap()
}

fn source(challenge: &IdentityChallenge, anchor: u64, sequence: u64, value: f32) -> SourceFrame {
    let registered = &challenge.passport().anchors()[&anchor];
    SourceFrame::capture(FrameIdentity {
        profile: registered.profile(), stream: registered.stream(), sequence,
        position: (registered.stimulus().len() - 1) as u64,
    }, &[value]).unwrap()
}

fn match_all(observer: &IdentityObserver, challenge: &IdentityChallenge, sequence: u64, now: ElapsedTick) {
    observer.observe_manifest(challenge, challenge.passport().manifest().clone(), now).unwrap();
    for (anchor, value) in [(11, 1.5), (22, 3.5)] {
        observer.observe_anchor(challenge, anchor, &source(challenge, anchor, sequence, value), now).unwrap();
    }
}

fn establish(broker: &mut OversightBroker, observer: &IdentityObserver, id: u64, sequence: u64) -> IdentityChallenge {
    let challenge = begin(broker, id);
    match_all(observer, &challenge, sequence, broker.inspect().ledger.elapsed.unwrap());
    let inspection = broker.inspect();
    let installed = broker.apply_identity_check(&challenge, inspection.sequence, inspection.ledger.epoch).unwrap();
    assert_eq!(installed.report.outcome, IdentityOutcome::Matched);
    assert_eq!(installed.refunded_units, 0);
    challenge
}

fn completed_review(broker: &mut OversightBroker, id: u64, round: u64) -> ObservedReview {
    let now = broker.inspect().ledger.elapsed.unwrap();
    let mut session = broker.begin_review(id, round, [1; 32], ReviewWindow {
        commit_by: ElapsedTick(now.0 + 5), reveal_by: ElapsedTick(now.0 + 10),
    }, &snapshot()).unwrap();
    let commitment = session.commitment("helper", Verdict::Allow, b"salt").unwrap();
    session.commit("helper", commitment, now).unwrap();
    session.open_reveals(now).unwrap();
    session.reveal("helper", Verdict::Allow, b"salt", now).unwrap();
    session.finish(now).unwrap()
}

#[test]
fn startup_requires_actual_all_anchor_results_then_normal_review_and_publication() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let observer = enable(&mut broker, 8);
    let (action, inputs) = prepare(&mut broker, &contracts, 1, Some("ok"));
    assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(broker.authorize(1, Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
    let challenge = begin(&mut broker, 1);
    observer.observe_manifest(&challenge, passport().manifest().clone(), ElapsedTick(1)).unwrap();
    observer.observe_anchor(&challenge, 11, &source(&challenge, 11, 1, 1.5), ElapsedTick(1)).unwrap();
    assert_eq!(broker.apply_identity_check(&challenge, 0, 0), Err(Error::Incomplete));
    assert_eq!(broker.identity_report(1).unwrap().observations.len(), 1);
    observer.observe_anchor(&challenge, 22, &source(&challenge, 22, 1, 3.0), ElapsedTick(1)).unwrap();
    assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Missing);
    broker.apply_identity_check(&challenge, 0, 0).unwrap();
    review(&mut broker, 1, 11, &inputs, Verdict::Allow);
    let permit = broker.authorize(1, Some(&inputs), &snapshot()).unwrap();
    let envelope = broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"ok");
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    conserved(&broker);
}

#[test]
fn discarded_manifest_mismatch_blocks_old_permits_and_fences_the_original_reservation() {
    for component in 0..5 {
        let (mut broker, _, contracts) = fixture();
        let observer = enable(&mut broker, 8);
        establish(&mut broker, &observer, 1, 1);
        let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("pending"));
        let before = broker.inspect();
        let challenge = begin(&mut broker, 2);
        let mut changed = passport().manifest().clone();
        match component {
            0 => changed.weights[0] ^= 1,
            1 => changed.adapters[0] ^= 1,
            2 => changed.tokenizer[0] ^= 1,
            3 => changed.architecture[0] ^= 1,
            _ => changed.numeric_profile[0] ^= 1,
        }
        drop(observer.observe_manifest(&challenge, changed, ElapsedTick(1)).unwrap());
        assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Mismatch { check: 2 });
        assert_eq!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Binding);
        assert_eq!(broker.inspect(), before);
        let installation = broker.apply_identity_check(&challenge, before.sequence, before.ledger.epoch).unwrap();
        assert_eq!(installation.report.outcome, IdentityOutcome::Mismatch(IdentityMismatch::Manifest));
        assert_eq!(installation.cancelled, vec![1]);
        assert_eq!(installation.refunded_units, action.spec().units);
        assert_eq!(installation.revocation_floor, before.ledger.epoch + 1);
        assert!(broker.inspect().suspended);
        let after = broker.inspect();
        assert_eq!(broker.apply_identity_check(&challenge, after.sequence, after.ledger.epoch), Err(Error::Duplicate));
        assert_eq!(broker.inspect(), after);
        conserved(&broker);
    }
}

#[test]
fn anchor_mismatch_preserves_unknown_disclosure_and_only_refunds_undispatched_sibling() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let observer = enable(&mut broker, 8);
    establish(&mut broker, &observer, 1, 1);
    let (first, first_input, first_key) = ready(&mut broker, &contracts, 1, Some("already sent"));
    let (second, _, _) = ready(&mut broker, &contracts, 2, Some("not sent"));
    let envelope = broker.dispatch(&first_key, &first, Some(&first_input), &snapshot()).unwrap();
    broker.acknowledgment_lost(1).unwrap();
    let challenge = begin(&mut broker, 2);
    observer.observe_manifest(&challenge, passport().manifest().clone(), ElapsedTick(1)).unwrap();
    let report = observer.observe_anchor(&challenge, 11, &source(&challenge, 11, 2, 9.0), ElapsedTick(1)).unwrap();
    assert_eq!(report.observations[&11].first_outlier().unwrap().observed_bits, 9.0_f32.to_bits());
    let before = broker.inspect();
    let installed = broker.apply_identity_check(&challenge, before.sequence, before.ledger.epoch).unwrap();
    assert_eq!(installed.cancelled, vec![2]);
    assert_eq!(installed.refunded_units, second.spec().units);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Unknown);
    assert_eq!(broker.inspect().ledger.charged, first.spec().units);
    assert_eq!(broker.stream_pending(), Some(1));
    // Admission fencing is not a claim that an already returned envelope vanished.
    let receipt = endpoint.deliver(&envelope).unwrap();
    assert_eq!(endpoint.payload(), b"already sent");
    broker.accept_receipt(receipt).unwrap();
    assert_eq!(broker.stream_pending(), None);
    assert_eq!(broker.inspect().ledger.stages[&1], ActionState::Confirmed);
    conserved(&broker);
}

#[test]
fn recapture_invalidates_old_congress_and_human_keys_but_not_reserved_rights() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let observer = enable(&mut broker, 8);
    let human = broker.enable_human_review(HumanReviewPolicy {
        reviewer_id: 90, max_validity_ticks: 20, max_requests: 8,
    }).unwrap();
    establish(&mut broker, &observer, 1, 1);
    let (action, inputs, automatic) = ready(&mut broker, &contracts, 1, Some("two keys"));
    let request = broker.request_human_approval(101, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let old_key = human.approve(&request, ElapsedTick(1)).unwrap();
    let old_review = completed_review(&mut broker, 1, 12);
    let reserved = broker.inspect().ledger.reserved;
    let challenge = begin(&mut broker, 2);
    assert_eq!(broker.dispatch_with_human(&automatic, &old_key, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
    match_all(&observer, &challenge, 2, ElapsedTick(1));
    let current = broker.inspect();
    broker.apply_identity_check(&challenge, current.sequence, current.ledger.epoch).unwrap();
    assert_eq!(broker.apply_review(old_review, Some(&inputs), &snapshot()), Err(Error::Stale));
    review(&mut broker, 1, 13, &inputs, Verdict::Allow);
    assert_eq!(broker.dispatch_with_human(&automatic, &old_key, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    assert_eq!(broker.human_status(101).unwrap().disposition, HumanDisposition::Approved);
    assert_eq!(broker.inspect().ledger.reserved, reserved);
    let fresh = broker.request_human_approval(102, 1, Some(&inputs), ElapsedTick(10)).unwrap();
    let fresh_key = human.approve(&fresh, ElapsedTick(1)).unwrap();
    let envelope = broker.dispatch_with_human(&automatic, &fresh_key, &action, Some(&inputs), &snapshot()).unwrap();
    broker.accept_receipt(endpoint.deliver(&envelope).unwrap()).unwrap();
    assert_eq!(endpoint.payload(), b"two keys");
    conserved(&broker);
}

#[test]
fn observer_time_cannot_advance_the_broker_and_expiry_is_strict() {
    let (mut broker, _, contracts) = fixture();
    let observer = enable(&mut broker, 8);
    let challenge = begin(&mut broker, 1);
    match_all(&observer, &challenge, 1, ElapsedTick(2));
    assert_eq!(broker.apply_identity_check(&challenge, 0, 0), Err(Error::Stale));
    broker.observe_time(ElapsedTick(2)).unwrap();
    broker.apply_identity_check(&challenge, 0, 0).unwrap();
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("later"));
    broker.observe_time(challenge.valid_until()).unwrap();
    assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Expired { check: 1 });
    assert_eq!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Stale);
    conserved(&broker);
}

#[test]
fn timeout_does_not_count_missing_anchors_and_new_check_can_recover_with_new_evidence() {
    let (mut broker, mut endpoint, contracts) = fixture();
    let observer = enable(&mut broker, 8);
    let challenge = begin(&mut broker, 1);
    observer.observe_manifest(&challenge, passport().manifest().clone(), ElapsedTick(1)).unwrap();
    assert_eq!(broker.expire_identity_check(&challenge), Err(Error::WrongState));
    broker.observe_time(challenge.deadline()).unwrap();
    broker.expire_identity_check(&challenge).unwrap();
    assert_eq!(broker.identity_report(1).unwrap().outcome, IdentityOutcome::Expired);
    assert_eq!(broker.apply_identity_check(&challenge, 0, 0), Err(Error::Incomplete));
    assert_eq!(observer.observe_anchor(&challenge, 11, &source(&challenge, 11, 1, 1.5), ElapsedTick(6)), Err(Error::WrongState));
    establish(&mut broker, &observer, 2, 1);
    publish(&mut broker, &mut endpoint, &contracts, 1, Some("recovered"));
    assert_eq!(endpoint.payload(), b"recovered");
}

#[test]
fn lost_capture_and_exhausted_quota_cannot_restore_an_older_matching_lease() {
    let (mut broker, _, contracts) = fixture();
    let observer = enable(&mut broker, 1);
    let old = establish(&mut broker, &observer, 1, 1);
    let (action, inputs, permit) = ready(&mut broker, &contracts, 1, Some("pending"));
    let before = broker.inspect();
    let old_basis = broker.identity_basis().unwrap();
    assert_eq!(broker.begin_identity_check(2, before.sequence, broker.actor_revision()).unwrap_err(), Error::Limit);
    assert!(broker.identity_basis().unwrap() > old_basis);
    assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Missing);
    assert_eq!(broker.apply_identity_check(&old, before.sequence, before.ledger.epoch), Err(Error::Duplicate));
    assert_eq!(broker.dispatch(&permit, &action, Some(&inputs), &snapshot()).unwrap_err(), Error::Incomplete);
    assert_eq!(broker.inspect(), before);
    drop(observer);
    broker.cancel(1).unwrap();
    assert_eq!(broker.inspect().ledger.available, TOTAL);
    conserved(&broker);
}

#[test]
fn explicit_gap_invalidates_a_completed_but_unapplied_check() {
    let (mut broker, _, _) = fixture();
    let observer = enable(&mut broker, 8);
    let old = begin(&mut broker, 1);
    match_all(&observer, &old, 1, ElapsedTick(1));
    broker.identity_unavailable(old.basis()).unwrap();
    assert_eq!(broker.apply_identity_check(&old, 0, 0), Err(Error::Stale));
    assert_eq!(broker.identity_report(1).unwrap().outcome, IdentityOutcome::Matched);
    let fresh = begin(&mut broker, 2);
    observer.observe_manifest(&fresh, passport().manifest().clone(), ElapsedTick(1)).unwrap();
    assert_eq!(observer.observe_anchor(&fresh, 11, &source(&old, 11, 1, 1.5), ElapsedTick(1)), Err(Error::Stale));
    for (anchor, value) in [(11, 1.5), (22, 3.5)] {
        observer.observe_anchor(&fresh, anchor, &source(&fresh, anchor, 2, value), ElapsedTick(1)).unwrap();
    }
    broker.apply_identity_check(&fresh, 0, 0).unwrap();
    assert!(matches!(broker.identity_status().unwrap(), IdentityStatus::Matching { check: 2, .. }));
}

#[test]
fn foreign_observer_and_foreign_challenge_are_not_authorized_by_matching_ids() {
    let (mut first, _, _) = fixture();
    let (mut second, _, _) = fixture();
    let observer = enable(&mut first, 8);
    let foreign = enable(&mut second, 8);
    let challenge = begin(&mut first, 1);
    let other = begin(&mut second, 1);
    assert_eq!(foreign.observe_manifest(&challenge, passport().manifest().clone(), ElapsedTick(1)), Err(Error::Binding));
    assert_eq!(first.apply_identity_check(&other, 0, 0), Err(Error::Binding));
    assert_eq!(first.identity_report(1).unwrap().outcome, IdentityOutcome::Collecting);
    match_all(&observer, &challenge, 1, ElapsedTick(1));
    first.apply_identity_check(&challenge, 0, 0).unwrap();
    assert_eq!(second.identity_status().unwrap(), IdentityStatus::Pending { check: 1 });
}

#[test]
fn actor_change_and_policy_rotation_refuse_stale_positive_identity_evidence() {
    let (mut broker, _, _) = fixture();
    let observer = enable(&mut broker, 8);
    let old = begin(&mut broker, 1);
    match_all(&observer, &old, 1, ElapsedTick(1));
    broker.replace_actor_state(0, ActorState::new(RestartProfile {
        id: 1, generation: 1, host_generation: 1, model_generation: 1,
        tokenizer_generation: 1, state_schema_generation: 1, grade: RestartGrade::FunctionalRestart,
    }, vec![1, 2], vec![2], vec![3], 2).unwrap()).unwrap();
    assert_eq!(broker.apply_identity_check(&old, 0, 0), Err(Error::Stale));
    establish(&mut broker, &observer, 2, 2);
    let current = broker.inspect();
    broker.replace_policy(current.sequence, current.ledger.epoch, policy(2)).unwrap();
    assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Stale { check: 2 });
    establish(&mut broker, &observer, 3, 3);
    assert!(matches!(broker.identity_status().unwrap(), IdentityStatus::Matching { check: 3, .. }));
}

#[test]
fn actor_reset_and_outage_do_not_erase_a_latched_mismatch_or_its_capture_history() {
    let (mut broker, _, contracts) = fixture();
    let observer = enable(&mut broker, 8);
    let checkpoint = broker.capture_checkpoint(1, 0).unwrap();
    establish(&mut broker, &observer, 1, 1);
    let _ = ready(&mut broker, &contracts, 1, Some("pending"));
    let challenge = begin(&mut broker, 2);
    let mut changed = passport().manifest().clone(); changed.adapters[0] ^= 1;
    observer.observe_manifest(&challenge, changed, ElapsedTick(1)).unwrap();
    let reset = broker.reset(ResetRequest {
        checkpoint, expected_control_sequence: broker.inspect().sequence,
        expected_actor_revision: broker.actor_revision(),
        binding: ReviewBinding { round: 99, evidence_root: [9; 32], reducer_generation: 1 },
        retained_targets: TargetCeiling::new(&[target(1), target(2)]).unwrap(),
    }).unwrap();
    assert!(reset.restored);
    broker.identity_unavailable(broker.identity_basis().unwrap()).unwrap();
    assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Mismatch { check: 2 });
    assert_eq!(broker.begin_identity_check(3, broker.inspect().sequence, broker.actor_revision()).unwrap_err(), Error::WrongState);
    let current = broker.inspect();
    broker.apply_identity_check(&challenge, current.sequence, current.ledger.epoch).unwrap();
    assert!(broker.inspect().suspended);
    assert_eq!(broker.incident_count(), 1);
    assert_eq!(broker.identity_report(1).unwrap().outcome, IdentityOutcome::Matched);
    conserved(&broker);
}

#[test]
fn altered_capture_contract_latches_instead_of_reusing_same_shaped_values() {
    let (mut broker, _, _) = fixture();
    let observer = enable(&mut broker, 8);
    let challenge = begin(&mut broker, 1);
    observer.observe_manifest(&challenge, passport().manifest().clone(), ElapsedTick(1)).unwrap();
    let mut identity = source(&challenge, 11, 1, 1.5).identity();
    identity.profile.model_generation += 1;
    let forged = SourceFrame::capture(identity, &[1.5]).unwrap();
    let report = observer.observe_anchor(&challenge, 11, &forged, ElapsedTick(1)).unwrap();
    assert!(matches!(report.outcome, IdentityOutcome::Mismatch(IdentityMismatch::CaptureContract { .. })));
    assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Mismatch { check: 1 });
    broker.apply_identity_check(&challenge, 0, 0).unwrap();
    assert!(broker.inspect().suspended);
}

#[test]
fn restrictive_reviews_and_unconfigured_publication_keep_their_original_paths() {
    let (mut broker, _, contracts) = fixture();
    let observer = enable(&mut broker, 8);
    let (_, inputs) = prepare(&mut broker, &contracts, 1, Some("hold"));
    review(&mut broker, 1, 11, &inputs, Verdict::Hold);
    assert_eq!(broker.identity_status().unwrap(), IdentityStatus::Missing);
    broker.cancel(1).unwrap();
    drop(observer);
    conserved(&broker);
    let (mut unconfigured, mut endpoint, contracts) = fixture();
    assert_eq!(unconfigured.identity_status().unwrap(), IdentityStatus::Unconfigured);
    publish(&mut unconfigured, &mut endpoint, &contracts, 1, Some("original"));
    assert_eq!(endpoint.payload(), b"original");
}
