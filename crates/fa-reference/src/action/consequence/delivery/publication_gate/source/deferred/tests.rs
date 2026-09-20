//! Original slot transition tests. Real file/driver execution is covered by the
//! durable adapter tests; these do not claim to acquire or authenticate a source.
use super::*;
use super::super::{PublicationInputCutStatus, PublicationSourceStatus, SourceState};
use crate::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};

fn cut(through: u64) -> PublicationInputCut { PublicationInputCut { source: 41, through } }
fn feed(through: u64) -> PublicationChangeStatus {
    PublicationChangeStatus { source: 41, through, observed_through: through, unavailable: false }
}
fn empty() -> PublicationInputs { PublicationInputs { structured: None, opaque: None } }
fn opaque(value: u8) -> PublicationInputs {
    PublicationInputs { structured: None, opaque: Some(ActualHelperInput::new(vec![value],
        InputProfileBinding { profile_id: 1, profile_bytes: b"deferred-test".to_vec(),
            tokenizer_epoch: 1, policy_epoch: 0, model_epoch: 1 },
        vec![SubmittedPart { kind: PartKind::Other, span: ByteSpan { start: 0, end: 1 } }],
        Vec::new()).unwrap()) }
}
fn slot(last: u64, required: u64) -> Slot {
    let action = FrozenAction::freeze(ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: 0,
        deadline: ElapsedTick(100), units: 16,
    }).unwrap();
    Slot { action, judgment: None, revision: 0, current: None, floor: None, last: None,
        source: Some(SourceState { status: PublicationSourceStatus { source: 91, generation: 8,
            capture_pending: true, fresh: false }, last: empty(),
            input_cut: Some(PublicationInputCutStatus { last: cut(last), required_through: required }) }) }
}

#[test]
fn exhaustive_neighbors_distinguish_waiting_from_rollback_and_unknown_coverage() {
    for last in 0..6 {
        for required in 0..6 {
            for supplied in 0..6 {
                for through in 0..6 {
                    let mut slot = slot(last, required);
                    let before = format!("{slot:?}");
                    let result = slot.capture_or_defer(0, 91, 9, empty(), cut(supplied), feed(through));
                    if supplied < last {
                        assert_eq!(result, Err(Error::Stale));
                    } else if supplied > through {
                        assert_eq!(result, Err(Error::Incomplete));
                    } else {
                        let deferred = supplied < required;
                        let expected = if deferred { PublicationCaptureOutcome::Deferred {
                            revision: 1, observed: cut(supplied), required_through: required,
                        } } else { PublicationCaptureOutcome::Installed { revision: 1 } };
                        assert_eq!(result, Ok(expected));
                        assert_eq!(expected.revision(), 1);
                        assert_eq!(expected.deferred(), deferred);
                        assert_eq!(slot.current.is_none(), deferred);
                        assert_eq!(slot.source.as_ref().unwrap().status.fresh, !deferred);
                        assert_eq!(slot.source.as_ref().unwrap().input_cut.unwrap().required_through, required);
                        assert_eq!(slot.consume_capture(), !deferred);
                        assert!(!slot.consume_capture());
                    }
                    if result.is_err() { assert_eq!(format!("{slot:?}"), before); }
                }
            }
        }
    }
}

#[test]
fn deferred_observation_retains_exact_generation_contents_and_requires_a_new_read() {
    let mut slot = slot(2, 5);
    let data = opaque(1);
    assert!(slot.capture_or_defer(0, 91, 9, data.clone(), cut(3), feed(5)).unwrap().deferred());
    let source = slot.source.as_ref().unwrap();
    assert_eq!(source.status.generation, 9);
    assert_eq!(source.last, data);
    assert_eq!(source.input_cut.unwrap().last, cut(3));
    assert!(!source.status.capture_pending); assert!(!source.status.fresh);
    assert_eq!(slot.capture_or_defer(1, 91, 9, data.clone(), cut(3), feed(5)), Err(Error::WrongState));
    slot.record_inputs(1, None).unwrap();
    let before = format!("{slot:?}");
    assert_eq!(slot.capture_or_defer(2, 91, 8, data.clone(), cut(3), feed(5)), Err(Error::Stale));
    assert_eq!(slot.capture_or_defer(2, 91, 9, opaque(2), cut(3), feed(5)), Err(Error::Binding));
    assert_eq!(slot.capture_or_defer(2, 91, 9, data.clone(), cut(5), feed(5)), Err(Error::Binding));
    assert_eq!(format!("{slot:?}"), before);
    assert_eq!(slot.capture_or_defer(2, 91, 10, data.clone(), cut(5), feed(5)),
        Ok(PublicationCaptureOutcome::Installed { revision: 3 }));
    assert_eq!(slot.current, Some(data));
    assert!(slot.consume_capture()); assert!(!slot.consume_capture());
}

#[test]
fn missing_or_foreign_feed_coverage_is_not_an_ordinary_producer_wait() {
    for invalid in [PublicationChangeStatus { source: 42, ..feed(5) },
        PublicationChangeStatus { observed_through: 6, ..feed(5) },
        PublicationChangeStatus { unavailable: true, ..feed(5) }] {
        let mut slot = slot(2, 5); let before = format!("{slot:?}");
        let result = slot.capture_or_defer(0, 91, 9, empty(), cut(2), invalid);
        assert_eq!(result, Err(if invalid.source != 41 { Error::Binding } else { Error::Incomplete }));
        assert_eq!(format!("{slot:?}"), before);
        assert!(slot.capture_or_defer(0, 91, 9, empty(), cut(2), feed(5)).unwrap().deferred());
    }
}

#[test]
fn source_substitution_future_cuts_and_metadata_edits_never_become_deferrals() {
    for (source, generation, supplied, expected) in [
        (92, 9, cut(2), Error::Binding), (91, 7, cut(2), Error::Stale),
        (91, 9, cut(1), Error::Stale), (91, 8, cut(3), Error::Binding),
        (91, 9, cut(6), Error::Incomplete),
        (91, 9, PublicationInputCut { source: 0, through: 2 }, Error::InvalidInput),
        (91, 9, PublicationInputCut { source: 42, through: 2 }, Error::Binding),
    ] {
        let mut slot = slot(2, 5); let before = format!("{slot:?}");
        assert_eq!(slot.capture_or_defer(0, source, generation, empty(), supplied, feed(5)), Err(expected));
        assert_eq!(format!("{slot:?}"), before);
        assert!(slot.capture_or_defer(0, 91, 9, empty(), cut(2), feed(5)).unwrap().deferred());
    }
}

#[test]
fn deferred_images_cannot_reset_floors_or_install_a_legacy_binding() {
    let mut slot = slot(2, 5); slot.floor = Some((11, 13, 17));
    slot.capture_or_defer(0, 91, 9, empty(), cut(3), feed(5)).unwrap();
    assert_eq!(slot.floor, Some((11, 13, 17)));
    slot.record_inputs(1, None).unwrap(); slot.require_capture_through(8); slot.require_capture_through(4);
    assert_eq!(slot.floor, Some((11, 13, 17)));
    assert_eq!(slot.source.as_ref().unwrap().input_cut.unwrap().required_through, 8);
    let before = format!("{slot:?}");
    assert_eq!(slot.capture_or_defer(2, 91, 10, empty(), cut(2), feed(8)), Err(Error::Stale));
    assert_eq!(format!("{slot:?}"), before);
    slot.source.as_mut().unwrap().input_cut = None;
    let before = format!("{slot:?}");
    assert_eq!(slot.capture_or_defer(2, 91, 10, empty(), cut(8), feed(8)), Err(Error::Binding));
    assert_eq!(format!("{slot:?}"), before);
}

#[test]
fn consumed_or_wrong_revision_captures_cannot_be_reclassified_as_waiting() {
    let mut slot = slot(2, 5); let before = format!("{slot:?}");
    assert_eq!(slot.capture_or_defer(1, 91, 9, empty(), cut(2), feed(5)), Err(Error::Stale));
    assert_eq!(format!("{slot:?}"), before);
    slot.source.as_mut().unwrap().status.capture_pending = false;
    assert_eq!(slot.capture_or_defer(0, 91, 9, empty(), cut(2), feed(5)), Err(Error::WrongState));
    slot.source.as_mut().unwrap().status.capture_pending = true; slot.current = Some(empty());
    assert_eq!(slot.capture_or_defer(0, 91, 9, empty(), cut(2), feed(5)), Err(Error::WrongState));
    slot.current = None;
    assert!(slot.capture_or_defer(0, 91, 9, empty(), cut(2), feed(5)).unwrap().deferred());
}

#[test]
fn sequence_and_generation_maxima_never_saturate_or_erase_a_wait() {
    let mut waiting = slot(u64::MAX - 1, u64::MAX);
    assert!(waiting.capture_or_defer(0, 91, u64::MAX, empty(), cut(u64::MAX - 1), feed(u64::MAX)).unwrap().deferred());
    waiting.record_inputs(1, None).unwrap();
    assert_eq!(waiting.capture_or_defer(2, 91, u64::MAX, empty(), cut(u64::MAX), feed(u64::MAX)), Err(Error::Binding));
    let mut installed = slot(u64::MAX - 1, u64::MAX);
    assert_eq!(installed.capture_or_defer(0, 91, u64::MAX, empty(), cut(u64::MAX), feed(u64::MAX)),
        Ok(PublicationCaptureOutcome::Installed { revision: 1 }));
    for through in [2, 5] {
        let mut exhausted = slot(2, 5); exhausted.revision = u64::MAX;
        let before = format!("{exhausted:?}");
        assert_eq!(exhausted.capture_or_defer(u64::MAX, 91, 9, empty(), cut(through), feed(5)), Err(Error::Overflow));
        assert_eq!(format!("{exhausted:?}"), before);
    }
}
