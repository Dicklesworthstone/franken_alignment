use super::*;
use crate::action::{ActionSpec, Purpose, ResolvedTarget, Scope, VERSION};
use crate::full_input::{ActualHelperInput, ByteSpan, InputProfileBinding, PartKind, SubmittedPart};
use crate::product_frontier::{FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker};
use crate::witness::{AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry};

fn profile(after: u64) -> PublicationProducerProfile {
    PublicationProducerProfile { source: 91, scope: action().spec().scope, feed: 41, clock_domain: 1, after }
}
fn inputs(revision: u64, epoch: u64, rows: &[(u64, u64, u8)]) -> FilePublicationInputs {
    let key = ProjectionKey { source: 4, branch: 2, projection: 3, source_epoch: 1 };
    let marker = TrustedClosingMarker { key, final_sequence: 1, marker_generation: 1 };
    let mut frontiers = ProductFrontiers::new(1, 2).unwrap();
    frontiers.accept(key, FrontierStage::Authenticated, 1).unwrap();
    frontiers.record_close(marker).unwrap();
    FilePublicationInputs::new(Some(FileWitnessInput::new(revision, revision, epoch,
        AdapterDomainInput::new(DomainProjection::new(1, 1, key), DomainClosure::Closed(marker)),
        rows.iter().map(|&(key, version, value)| SnapshotEntry::new(key, version, vec![value]).unwrap()).collect(),
        &frontiers).unwrap()), None)
}
fn opaque(value: u8) -> ActualHelperInput {
    ActualHelperInput::new(vec![value], InputProfileBinding { profile_id: 1,
        profile_bytes: vec![1], tokenizer_epoch: 1, model_epoch: 1, policy_epoch: 1 },
        vec![SubmittedPart { span: ByteSpan { start: 0, end: 1 }, kind: PartKind::Question }], vec![]).unwrap()
}
fn action() -> FrozenAction {
    FrozenAction::freeze(ActionSpec { version: VERSION,
        scope: Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect },
        target: Some(ResolvedTarget { adapter: 1, object: 2, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), required_witnesses: vec![], policy_epoch: 0,
        deadline: ElapsedTick(100), units: 16 }).unwrap()
}

#[test]
fn exhaustive_small_snapshots_emit_every_insert_and_delete_without_spurious_keys() {
    for old_mask in 0..16 {
        for new_mask in 0..16 {
            let make = |mask: u32| (0..4).filter(|key| mask & (1 << key) != 0)
                .map(|key| (key, 1, 8)).collect::<Vec<_>>();
            let old = inputs(1, 1, &make(old_mask));
            let new = inputs(2, 1, &make(new_mask));
            let actual = derive_changes(&old, &new).unwrap().into_iter().map(|change| match change {
                WitnessChange::Key { key, .. } => key, _ => panic!("small changes must be exact"),
            }).collect::<Vec<_>>();
            let expected = (0..4).filter(|key| (old_mask ^ new_mask) & (1 << key) != 0).collect::<Vec<_>>();
            assert_eq!(actual, expected);
        }
    }
}

#[test]
fn value_and_version_changes_include_maximum_key_without_range_overflow() {
    let old = inputs(1, 1, &[(0, 1, 8), (u64::MAX, 2, 9)]);
    for rows in [vec![(0, 2, 8), (u64::MAX, 2, 9)], vec![(0, 1, 8), (u64::MAX, 2, 0)],
        vec![(0, 1, 8), (u64::MAX, 3, 9)]] {
        let changes = derive_changes(&old, &inputs(2, 1, &rows)).unwrap();
        assert_eq!(changes.len(), 1);
        assert!(matches!(changes[0], WitnessChange::Key { key, .. } if key == if rows[0].1 == 2 { 0 } else { u64::MAX }));
    }
}

#[test]
fn metadata_lane_loss_and_whole_helper_drift_emit_conservative_notifications() {
    let old = inputs(1, 1, &[(0, 1, 8)]);
    assert_eq!(derive_changes(&old, &inputs(2, 2, &[(0, 1, 8)])).unwrap(), vec![WitnessChange::All]);
    let mut missing_close = old.clone(); missing_close.structured.as_mut().unwrap().admitted_close = None;
    assert_eq!(derive_changes(&old, &missing_close).unwrap(), vec![WitnessChange::All]);
    assert_eq!(derive_changes(&old, &FilePublicationInputs::new(None, None)).unwrap(), vec![WitnessChange::All]);
    let mut with_helper = old.clone(); with_helper.opaque = Some(opaque(8));
    let mut changed_helper = with_helper.clone(); changed_helper.opaque = Some(opaque(9));
    assert_eq!(derive_changes(&with_helper, &changed_helper).unwrap(), vec![WitnessChange::All]);
    assert_eq!(derive_changes(&with_helper, &old).unwrap(), vec![WitnessChange::All]);
}

#[test]
fn too_many_changed_keys_collapse_to_one_domain_without_dropping_a_change() {
    let old_rows = (0..256).map(|key| (key, 1, 8)).collect::<Vec<_>>();
    let new_rows = (256..512).map(|key| (key, 1, 8)).collect::<Vec<_>>();
    let old = inputs(1, 1, &old_rows);
    let exact = derive_changes(&inputs(0, 1, &[]), &old).unwrap();
    assert_eq!(exact.len(), MAX_FEED_RECORDS);
    let domain = old.structured().unwrap().snapshot().domain_input().domain();
    assert_eq!(derive_changes(&old, &inputs(2, 1, &new_rows)).unwrap(), vec![WitnessChange::Domain { domain }]);
}

#[test]
fn heartbeat_only_refresh_does_not_relabel_input_generation_or_advance_its_cut() {
    let first = PublicationProducerImage::new(profile(3), inputs(1, 1, &[]), ElapsedTick(1)).unwrap();
    let next = first.advance(1, first.inputs().clone(), ElapsedTick(2)).unwrap();
    assert_eq!(next.generation(), 2); assert_eq!(next.input_generation(), 1);
    assert_eq!(next.batch().heartbeat().through, 3); assert!(next.batch().records().is_empty());
    let capture = next.capture(7, &action()).unwrap();
    assert_eq!(capture.identity().generation, 1);
    assert_eq!(capture.input_cut(), Some(PublicationInputCut { source: 41, through: 3 }));
    assert_eq!(first.batch().heartbeat().produced_at, ElapsedTick(1));
    assert_eq!(PublicationProducerImage::from_bytes(&next.to_bytes().unwrap()).unwrap(), next);
}

#[test]
fn rolling_window_retains_a_complete_suffix_and_no_old_record_is_rewritten() {
    let mut image = PublicationProducerImage::new(profile(10), inputs(1, 1, &[(0, 1, 0)]), ElapsedTick(1)).unwrap();
    for version in 2..=270 {
        let old_records = image.batch().records().to_vec();
        image = image.advance(image.generation(), inputs(version, 1, &[(0, version, 0)]), ElapsedTick(version)).unwrap();
        for old in old_records {
            if old.sequence > image.batch().after() { assert!(image.batch().records().contains(&old)); }
        }
    }
    assert_eq!(image.batch().records().len(), MAX_FEED_RECORDS);
    assert_eq!(image.batch().heartbeat().through, 279);
    assert_eq!(image.batch().after(), 23);
    assert_eq!(image.capture(1, &action()).unwrap().input_cut().unwrap().through, 279);
    assert_eq!(PublicationProducerImage::from_bytes(&image.to_bytes().unwrap()).unwrap(), image);
}

#[test]
fn lost_lanes_retain_monotone_floors_and_failed_advances_do_not_change_the_image() {
    let initial = PublicationProducerImage::new(profile(0), inputs(5, 3, &[]), ElapsedTick(10)).unwrap();
    let missing = initial.advance(1, FilePublicationInputs::new(None, None), ElapsedTick(11)).unwrap();
    let missing = PublicationProducerImage::from_bytes(&missing.to_bytes().unwrap()).unwrap();
    let before = missing.to_bytes().unwrap();
    assert_eq!(missing.advance(2, inputs(4, 3, &[]), ElapsedTick(12)), Err(Error::Stale));
    assert_eq!(missing.advance(2, inputs(6, 2, &[]), ElapsedTick(12)), Err(Error::Stale));
    assert_eq!(missing.advance(1, inputs(6, 3, &[]), ElapsedTick(12)), Err(Error::Stale));
    assert_eq!(missing.advance(2, inputs(6, 3, &[]), ElapsedTick(9)), Err(Error::Stale));
    assert_eq!(missing.to_bytes().unwrap(), before);
    assert!(missing.advance(2, inputs(6, 3, &[]), ElapsedTick(12)).is_ok());
}

#[test]
fn sequence_exhaustion_refuses_changes_but_allows_a_real_heartbeat_observation() {
    let image = PublicationProducerImage::new(profile(u64::MAX), inputs(1, 1, &[]), ElapsedTick(1)).unwrap();
    assert_eq!(image.advance(1, inputs(2, 1, &[(0, 1, 8)]), ElapsedTick(2)), Err(Error::Overflow));
    assert!(image.advance(1, image.inputs().clone(), ElapsedTick(2)).is_ok());
    assert_eq!(image.generation(), 1);
}

#[test]
fn producer_packet_is_bounded_canonical_and_rejects_truncation_or_foreign_feed() {
    let image = PublicationProducerImage::new(profile(0), inputs(1, 1, &[(0, 1, 8)]), ElapsedTick(1)).unwrap();
    let bytes = image.to_bytes().unwrap();
    for length in 0..bytes.len() { assert!(PublicationProducerImage::from_bytes(&bytes[..length]).is_err()); }
    let mut trailing = bytes.clone(); trailing.push(0);
    assert!(PublicationProducerImage::from_bytes(&trailing).is_err());
    let mut foreign = bytes.clone(); foreign[23] ^= 1; // profile.feed, not the embedded original feed
    assert_eq!(PublicationProducerImage::from_bytes(&foreign), Err(Error::Binding));
    let mut bad_version = bytes; bad_version[7] = b'2';
    assert_eq!(PublicationProducerImage::from_bytes(&bad_version), Err(Error::Binding));
}

#[test]
fn a_source_cannot_bind_its_image_to_an_action_in_another_scope() {
    let image = PublicationProducerImage::new(profile(0), inputs(1, 1, &[]), ElapsedTick(1)).unwrap();
    let mut spec = action().spec().clone(); spec.scope.tenant += 1;
    assert_eq!(image.capture(1, &FrozenAction::freeze(spec).unwrap()), Err(Error::Binding));
    assert!(image.capture(1, &action()).is_ok());
}
