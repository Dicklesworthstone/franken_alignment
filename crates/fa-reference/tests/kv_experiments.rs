//! Public sparse-KV intervention, exact probe and paired-buffer consumers.
//! Inputs are captured fixtures, not a measured model continuation campaign.

use fa_reference::action::consequence::activation::CaptureProfile;
use fa_reference::action::consequence::activation::probe::{LinearProbe, ProbeOutcome};
use fa_reference::action::consequence::activation::tensor::{
    BufferIdentity, ByteOrder, HostTensor, ScalarEncoding, TensorContract, TensorLayout,
};
use fa_reference::action::consequence::activation::tensor::kv::{KvAppend, KvBudget, KvCapture, KvContract};
use fa_reference::action::consequence::activation::tensor::kv::image::KvImage;
use fa_reference::action::consequence::activation::tensor::kv::experiment::{
    KvBranch, KvBranchKind, KvCell, KvEdit, KvEditScope, KvExperiment, KvExperimentLimits, KvSide,
    MAX_KV_BRANCHES, MAX_KV_EDITS_PER_FORK,
};
use fa_reference::action::consequence::activation::tensor::kv::restore::{
    HostTensorMut, KvDestination, KvRestoreWindow,
};
use fa_reference::Error;

fn layout(encoding: ScalarEncoding, order: ByteOrder) -> TensorLayout {
    let width = encoding.bytes();
    TensorLayout::new([1, 2, 1, 2], [0, 2 * width, 0, width], 0, encoding, order).unwrap()
}

fn image(half: bool) -> KvImage {
    let p = CaptureProfile { tenant: 1, model: 2, model_generation: 3, tap: 4, layout_generation: 5 };
    let ke = if half { ScalarEncoding::Binary16 } else { ScalarEncoding::Binary32 };
    let ve = if half { ScalarEncoding::BFloat16 } else { ScalarEncoding::Binary32 };
    let keys = TensorContract::new(p, ke, ByteOrder::Little, 1, 2).unwrap();
    let values = TensorContract::new(CaptureProfile { tap: 6, ..p }, ve, ByteOrder::Big, 1, 2).unwrap();
    let mut capture = KvCapture::new(KvContract::new(keys, values, 4).unwrap(), 7, 0, 10, 20,
        KvBudget { positions: 2, normalized_values: 8 }).unwrap();
    let kl = layout(ke, ByteOrder::Little);
    let vl = layout(ve, ByteOrder::Big);
    let kb: Vec<u8> = if half {
        [0x3c00_u16, 0xc000, 0x4200, 0x4400].into_iter().flat_map(u16::to_le_bytes).collect()
    } else { [1_f32, -2.0, 3.0, 4.0].into_iter().flat_map(f32::to_le_bytes).collect() };
    let vb: Vec<u8> = if half {
        [0x40a0_u16, 0x40c0, 0x40e0, 0x4100].into_iter().flat_map(u16::to_be_bytes).collect()
    } else { [5_f32, 6.0, 7.0, 8.0].into_iter().flat_map(f32::to_be_bytes).collect() };
    capture.append(0, KvAppend {
        keys: HostTensor { identity: BufferIdentity { object: 1, generation: 1 }, layout: &kl, bytes: &kb },
        values: HostTensor { identity: BufferIdentity { object: 2, generation: 1 }, layout: &vl, bytes: &vb },
        first_token: 0, token_count: 2, buffer_first_position: 10, first_sequence: 20,
    }).unwrap();
    capture.snapshot(1).unwrap()
}

fn space(image: KvImage) -> KvExperiment {
    KvExperiment::new(9, image, KvEditScope {
        first_position: 10, token_count: 2, keys: true, values: true,
    }, KvExperimentLimits { branches: 16, retained_edits: 64, resolution_depth: 2 }).unwrap()
}

fn cell(side: KvSide, position: u64, channel: usize) -> KvCell {
    KvCell { side, position, head: 0, channel }
}
fn edit(cell: KvCell, expected: f32, replacement: f32) -> KvEdit {
    KvEdit { cell, expected_bits: expected.to_bits(), replacement_bits: replacement.to_bits() }
}
fn window() -> KvRestoreWindow {
    KvRestoreWindow { first_position: 10, token_count: 2, batch: 0, first_token: 0, buffer_first_position: 10 }
}
fn destination<'a>(
    keys: &'a mut [u8], values: &'a mut [u8], kl: &'a TensorLayout, vl: &'a TensorLayout, id: u64,
) -> KvDestination<'a> {
    KvDestination::Separate {
        keys: HostTensorMut { identity: BufferIdentity { object: id, generation: 1 }, layout: kl, bytes: keys },
        values: HostTensorMut { identity: BufferIdentity { object: id + 1, generation: 1 }, layout: vl, bytes: values },
    }
}
fn probe(branch: &KvBranch) -> LinearProbe {
    LinearProbe::new(1, 1, branch.basis().source.contract.keys().profile(), &[1.0, 0.0], 0.0, 0.0).unwrap()
}
fn bytes(values: &[f32], order: ByteOrder) -> Vec<u8> {
    values.iter().flat_map(|v| match order { ByteOrder::Little => v.to_le_bytes(), ByteOrder::Big => v.to_be_bytes() }).collect()
}

#[test]
fn paired_restore_writes_real_control_and_intervened_values_without_changing_the_source() {
    let original = image(false);
    let original_bytes = original.encode().unwrap();
    let mut experiment = space(original.clone());
    let root = experiment.baseline();
    let changed = experiment.fork(1, &root, &[
        edit(cell(KvSide::Key, 10, 0), 1.0, -1.0),
        edit(cell(KvSide::Value, 11, 1), 8.0, 12.0),
    ]).unwrap();
    let kl = layout(ScalarEncoding::Binary32, ByteOrder::Little);
    let vl = layout(ScalarEncoding::Binary32, ByteOrder::Big);
    let (mut lk, mut lv, mut rk, mut rv) = (vec![0xcc; 16], vec![0xcc; 16], vec![0xcc; 16], vec![0xcc; 16]);
    let plan = changed.prepare_twin_restore(&root,
        destination(&mut lk, &mut lv, &kl, &vl, 10),
        destination(&mut rk, &mut rv, &kl, &vl, 20), window()).unwrap();
    assert!(plan.staged_bytes() > 0);
    let receipt = plan.commit();
    assert_eq!(receipt.reference.branch.kind, KvBranchKind::Baseline);
    assert_eq!(receipt.candidate.branch.branch, 1);
    assert_eq!(receipt.reference.copied_values, 0);
    assert_eq!(receipt.candidate.copied_values, 4);
    assert_eq!(receipt.candidate.restoration.normalized_values, 8);
    assert_eq!(lk, bytes(&[1.0, -2.0, 3.0, 4.0], ByteOrder::Little));
    assert_eq!(rk, bytes(&[-1.0, -2.0, 3.0, 4.0], ByteOrder::Little));
    assert_eq!(lv, bytes(&[5.0, 6.0, 7.0, 8.0], ByteOrder::Big));
    assert_eq!(rv, bytes(&[5.0, 6.0, 7.0, 12.0], ByteOrder::Big));
    assert_eq!(original.encode().unwrap(), original_bytes);
}

#[test]
fn a_late_candidate_destination_failure_leaves_both_legs_untouched() {
    let mut experiment = space(image(false));
    let root = experiment.baseline();
    let changed = experiment.fork(1, &root, &[edit(cell(KvSide::Key, 10, 0), 1.0, -1.0)]).unwrap();
    let kl = layout(ScalarEncoding::Binary32, ByteOrder::Little);
    let vl = layout(ScalarEncoding::Binary32, ByteOrder::Big);
    let (mut lk, mut lv, mut rk, mut rv) = (vec![0xaa; 16], vec![0xbb; 16], vec![0xcc; 16], vec![0xdd; 15]);
    assert_eq!(changed.prepare_twin_restore(&root,
        destination(&mut lk, &mut lv, &kl, &vl, 10),
        destination(&mut rk, &mut rv, &kl, &vl, 20), window()).unwrap_err(), Error::Incomplete);
    assert_eq!(lk, vec![0xaa; 16]); assert_eq!(lv, vec![0xbb; 16]);
    assert_eq!(rk, vec![0xcc; 16]); assert_eq!(rv, vec![0xdd; 15]);
}

#[test]
fn dropping_a_prepared_pair_never_publishes_either_leg() {
    let experiment = space(image(false));
    let root = experiment.baseline();
    let kl = layout(ScalarEncoding::Binary32, ByteOrder::Little);
    let vl = layout(ScalarEncoding::Binary32, ByteOrder::Big);
    let (mut lk, mut lv, mut rk, mut rv) = (vec![0xaa; 16], vec![0xaa; 16], vec![0xaa; 16], vec![0xaa; 16]);
    drop(root.prepare_twin_restore(&root,
        destination(&mut lk, &mut lv, &kl, &vl, 10),
        destination(&mut rk, &mut rv, &kl, &vl, 20), window()).unwrap());
    for values in [&lk, &lv, &rk, &rv] { assert_eq!(values.as_slice(), &[0xaa; 16]); }
}

#[test]
fn probe_comparison_uses_exact_same_probe_and_labels_both_bases() {
    let mut experiment = space(image(false));
    let root = experiment.baseline();
    let changed = experiment.fork(1, &root, &[edit(cell(KvSide::Key, 10, 0), 1.0, -1.0)]).unwrap();
    let report = changed.compare_probe(&root, KvSide::Key, 10, &probe(&root)).unwrap();
    assert_eq!(report.reference_outcome, ProbeOutcome::CertifiedAlarm);
    assert_eq!(report.candidate_outcome, ProbeOutcome::CertifiedQuiet);
    assert_eq!(report.reference_score.lower, report.reference_score.upper);
    assert_eq!(report.candidate_score.lower, report.candidate_score.upper);
    assert_eq!(report.reference.branch, 0);
    assert_eq!(report.candidate.branch, 1);
    assert_eq!(report.copied_values, 2);
    assert_eq!(report.encoded_block_bytes, 2 * (78 + 8));
    assert_eq!(root.bits(cell(KvSide::Key, 10, 0)).unwrap(), 1_f32.to_bits());
}

#[test]
fn unchanged_controls_and_undone_edits_share_rows_and_preserve_exact_scores() {
    let mut experiment = space(image(false));
    let root = experiment.baseline();
    let c = cell(KvSide::Key, 10, 0);
    let changed = experiment.fork(1, &root, &[edit(c, 1.0, -1.0)]).unwrap();
    let undone = experiment.fork(2, &changed, &[edit(c, -1.0, 1.0)]).unwrap();
    let control = experiment.fork(3, &root, &[]).unwrap();
    for branch in [&undone, &control] {
        let report = branch.compare_probe(&root, KvSide::Key, 10, &probe(&root)).unwrap();
        assert_eq!(report.reference_score, report.candidate_score);
        assert_eq!(report.copied_values, 0);
        assert!(branch.delta().unwrap().is_empty());
    }
    assert_eq!(undone.lineage().len(), 3);
    assert_eq!(experiment.retained_edit_count(), 2);
}

#[test]
fn half_width_interventions_refuse_rounding_and_restore_exact_original_encodings() {
    let mut experiment = space(image(true));
    let root = experiment.baseline();
    let c = cell(KvSide::Key, 10, 0);
    let bad = KvEdit { cell: c, expected_bits: 1_f32.to_bits(), replacement_bits: 0x3f80_0001 };
    assert_eq!(experiment.fork(1, &root, &[bad]).unwrap_err(), Error::Binding);
    let changed = experiment.fork(1, &root, &[
        edit(c, 1.0, 1.5), edit(cell(KvSide::Value, 11, 1), 8.0, 8.5),
    ]).unwrap();
    let kl = layout(ScalarEncoding::Binary16, ByteOrder::Little);
    let vl = layout(ScalarEncoding::BFloat16, ByteOrder::Big);
    let (mut keys, mut values) = (vec![0; 8], vec![0; 8]);
    let receipt = changed.prepare_restore(destination(&mut keys, &mut values, &kl, &vl, 10), window()).unwrap().commit();
    assert_eq!(keys, [0x3e00_u16, 0xc000, 0x4200, 0x4400].into_iter().flat_map(u16::to_le_bytes).collect::<Vec<_>>());
    assert_eq!(values, [0x40a0_u16, 0x40c0, 0x40e0, 0x4108].into_iter().flat_map(u16::to_be_bytes).collect::<Vec<_>>());
    assert_eq!(receipt.restoration.bytes_written, 16);
}

#[test]
fn restored_rebase_is_byte_identical_after_the_capture_and_original_image_are_gone() {
    let saved = image(false);
    let descriptor = saved.descriptor().clone();
    let wire = saved.encode().unwrap();
    drop(saved);
    let imported = KvImage::decode(&wire, &descriptor).unwrap();
    let mut experiment = space(imported);
    let root = experiment.baseline();
    let a = experiment.fork(1, &root, &[edit(cell(KvSide::Key, 10, 0), 1.0, -1.0)]).unwrap();
    let b = experiment.fork(2, &a, &[edit(cell(KvSide::Value, 11, 1), 8.0, 9.0)]).unwrap();
    let rebased = experiment.rebase(3, &b).unwrap();
    drop(experiment); drop(root); drop(a);
    let kl = layout(ScalarEncoding::Binary32, ByteOrder::Little);
    let vl = layout(ScalarEncoding::Binary32, ByteOrder::Big);
    let (mut lk, mut lv, mut rk, mut rv) = (vec![0; 16], vec![0; 16], vec![0; 16], vec![0; 16]);
    rebased.prepare_twin_restore(&b, destination(&mut lk, &mut lv, &kl, &vl, 10),
        destination(&mut rk, &mut rv, &kl, &vl, 20), window()).unwrap().commit();
    assert_eq!(lk, rk); assert_eq!(lv, rv);
    assert_eq!(rebased.basis().resolution_depth, 1);
    assert_eq!(rebased.lineage().len(), 4);
}

#[test]
fn foreign_base_and_wrong_probe_contract_cannot_enter_a_paired_comparison() {
    let left = space(image(false)).baseline();
    let right = space(image(false)).baseline();
    assert_eq!(left.compare_probe(&right, KvSide::Key, 10, &probe(&left)).unwrap_err(), Error::Binding);
    let wrong_profile = CaptureProfile { tap: 99, ..left.basis().source.contract.keys().profile() };
    let wrong = LinearProbe::new(1, 1, wrong_profile, &[1.0, 0.0], 0.0, 0.0).unwrap();
    assert_eq!(left.compare_probe(&left, KvSide::Key, 10, &wrong).unwrap_err(), Error::Binding);
    assert_eq!(left.compare_probe(&left, KvSide::Key, 9, &probe(&left)).unwrap_err(), Error::Missing);
    let wrong_size = LinearProbe::new(1, 1, left.basis().source.contract.keys().profile(), &[1.0], 0.0, 0.0).unwrap();
    assert_eq!(left.compare_probe(&left, KvSide::Key, 10, &wrong_size).unwrap_err(), Error::Binding);
}

#[test]
fn separately_borrowed_twin_buffers_cannot_claim_the_same_storage_object() {
    let root = space(image(false)).baseline();
    let kl = layout(ScalarEncoding::Binary32, ByteOrder::Little);
    let vl = layout(ScalarEncoding::Binary32, ByteOrder::Big);
    let (mut lk, mut lv, mut rk, mut rv) = (vec![0xaa; 16], vec![0xaa; 16], vec![0xaa; 16], vec![0xaa; 16]);
    assert_eq!(root.prepare_twin_restore(&root,
        destination(&mut lk, &mut lv, &kl, &vl, 10),
        destination(&mut rk, &mut rv, &kl, &vl, 11), window()).unwrap_err(), Error::Binding);
    for values in [&lk, &lv, &rk, &rv] { assert_eq!(values.as_slice(), &[0xaa; 16]); }
}

#[test]
fn only_selected_positions_and_coordinates_are_written_into_padded_destinations() {
    let mut experiment = space(image(false));
    let root = experiment.baseline();
    let changed = experiment.fork(1, &root, &[edit(cell(KvSide::Value, 11, 1), 8.0, -8.0)]).unwrap();
    let kl = TensorLayout::new([1, 3, 1, 2], [0, 16, 0, 4], 3, ScalarEncoding::Binary32, ByteOrder::Little).unwrap();
    let vl = TensorLayout::new([1, 3, 1, 2], [0, 16, 0, 4], 3, ScalarEncoding::Binary32, ByteOrder::Big).unwrap();
    let (mut keys, mut values) = (vec![0xcc; 48], vec![0xdd; 48]);
    changed.prepare_restore(destination(&mut keys, &mut values, &kl, &vl, 10), KvRestoreWindow {
        first_position: 11, token_count: 1, batch: 0, first_token: 1, buffer_first_position: 10,
    }).unwrap().commit();
    let mut expected_keys = vec![0xcc; 48];
    let mut expected_values = vec![0xdd; 48];
    expected_keys[19..27].copy_from_slice(&bytes(&[3.0, 4.0], ByteOrder::Little));
    expected_values[19..27].copy_from_slice(&bytes(&[7.0, -8.0], ByteOrder::Big));
    assert_eq!(keys, expected_keys); assert_eq!(values, expected_values);
}

#[test]
fn constructor_and_fork_shape_limits_refuse_without_registering_a_partial_branch() {
    let scope = KvEditScope { first_position: 10, token_count: 2, keys: true, values: true };
    let limits = KvExperimentLimits { branches: MAX_KV_BRANCHES + 1, retained_edits: 64, resolution_depth: 2 };
    assert_eq!(KvExperiment::new(1, image(false), scope, limits).unwrap_err(), Error::Limit);
    let mut experiment = space(image(false));
    let root = experiment.baseline();
    let edits = vec![edit(cell(KvSide::Key, 10, 0), 1.0, 2.0); MAX_KV_EDITS_PER_FORK + 1];
    assert_eq!(experiment.fork(1, &root, &edits).unwrap_err(), Error::Limit);
    assert_eq!((experiment.branch_count(), experiment.retained_edit_count()), (0, 0));
    assert!(experiment.fork(1, &root, &[]).is_ok());
}

#[test]
fn isolated_kv_experiment_does_not_change_an_existing_effect_reservation() {
    use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ReferenceAuthority, ResolvedTarget, Scope, VERSION};
    use fa_reference::{Judgment, Snapshot};
    let scope = Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect };
    let mut authority = ReferenceAuthority::new(scope, 20, 4).unwrap();
    authority.observe_time(ElapsedTick(1)).unwrap();
    let action = FrozenAction::freeze(ActionSpec {
        version: VERSION, scope, target: Some(ResolvedTarget { adapter: 1, object: 2, contract_version: 3, expected_version: 1, generation: 1 }),
        payload: b"ok".to_vec(), required_witnesses: vec![], policy_epoch: 0, deadline: ElapsedTick(100), units: 2,
    }).unwrap();
    let snapshot = Snapshot { semantic_epoch: 0, complete: true, values: Default::default() };
    let judgment = Judgment::capture(&snapshot, vec![]).unwrap();
    authority.propose(1, action.clone()).unwrap(); authority.prepare(1).unwrap(); authority.begin_review(1).unwrap();
    let permit = authority.authorize(1, &judgment, &snapshot).unwrap();
    let before = authority.inspect();
    let mut experiment = space(image(false));
    let root = experiment.baseline();
    let changed = experiment.fork(1, &root, &[edit(cell(KvSide::Key, 10, 0), 1.0, -1.0)]).unwrap();
    changed.compare_probe(&root, KvSide::Key, 10, &probe(&root)).unwrap();
    assert_eq!(authority.inspect(), before);
    authority.dispatch(&permit, &action, &snapshot).unwrap();
    assert_eq!(authority.inspect().charged, 2);
    assert!(authority.dispatch(&permit, &action, &snapshot).is_err());
}
