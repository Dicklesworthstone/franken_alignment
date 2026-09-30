//! Real numerical steps and journal cuts; scripted ballots remain test controls.
use super::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderLayerWeights;
use fa_reference::action::consequence::delivery::persistent::observed::identity::decoder::transaction::{
    ComputedIdentityStatus as Status, FileComputedIdentityRun,
};

fn begin(host: &FileOversight, roles: &mut FileOversightRoles, check: &FileIdentityChallenge)
    -> FileComputedIdentityRun
{
    roles.identity_observer.take().unwrap().begin_computed_learned(host, host.revision(), check,
        input(check.evidence().passport(), 700 + check.id()), ElapsedTick(1)).unwrap()
}
fn drive(host: &FileOversight, run: &mut FileComputedIdentityRun) {
    let limit = run.work().planned_tokens;
    for _ in 0..limit {
        if run.status() != Status::Computing { break; }
        let before = run.work();
        run.advance_with_clock(host, run.revision(), || ElapsedTick(1)).unwrap();
        assert_eq!(run.work().entered_tokens, before.entered_tokens + 1);
        assert!(run.result().is_none());
    }
    assert_eq!(run.status(), Status::ReadyToCommit);
}

#[test]
fn cooperative_record_is_byte_identical_to_synchronous_computation_at_the_same_cut() {
    let root = Directory::new();
    let (mut host, mut roles, config, _) = setup(&root, true, fixture::model(1.0));
    step(&mut host); let check = challenge(&mut host, 1);
    let numerical = host.learned_generation_inspection().unwrap();
    let initial = root.bytes(); let revision = host.revision();
    let mut run = begin(&host, &mut roles, &check);
    assert_eq!(run.work().entered_tokens, 0);
    assert_eq!(run.take_observer().err(), Some(Error::WrongState));
    assert_eq!(run.commit_with_clock(&mut host, 0, || panic!("unfinished computation cannot commit")).err(),
        Some(Error::Incomplete.into()));
    drive(&host, &mut run);
    assert_eq!(host.revision(), revision); assert_eq!(root.bytes(), initial);
    assert!(host.identity_report(1).unwrap().observations.is_empty());
    assert!(host.identity_report(1).unwrap().manifest.is_none());
    let work = run.work();
    let result = run.commit_with_clock(&mut host, run.revision(), || ElapsedTick(1)).unwrap().clone();
    assert_eq!(run.work(), work); assert_eq!(result.work, work);
    assert_eq!(host.revision(), revision + 1);
    let after = host.learned_generation_inspection().unwrap();
    assert_eq!(after.numerical, numerical.numerical);
    assert_eq!((after.paused, after.pending), (numerical.paused, numerical.pending));
    assert_eq!(after.journal_revision, numerical.journal_revision + 1);
    assert!(host.identity_installation(1).unwrap().is_none());
    let bytes = root.bytes();
    FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    drop(run); drop(host); drop(roles);
    // Recreate ONLY this test's store at the identical canonical path. Path is
    // part of the original journal binding, so separate directories cannot be
    // compared for complete byte equality even with equivalent computations.
    std::fs::remove_dir_all(root.store()).unwrap();
    let (mut host, roles, _, _) = setup(&root, true, fixture::model(1.0));
    step(&mut host); let check = challenge(&mut host, 1);
    let synchronous = computed(&mut host, roles.identity_observer.as_ref().unwrap(), &check, 1);
    assert_eq!(synchronous, result); assert_eq!(root.bytes(), bytes);
}

#[test]
fn strict_cooperative_match_requires_separate_installation_congress_and_both_keys() {
    let root = Directory::new();
    let (mut host, mut roles, config, _) = setup(&root, true, fixture::model(1.0));
    step(&mut host); step(&mut host); let check = challenge(&mut host, 1);
    let mut run = begin(&host, &mut roles, &check); drive(&host, &mut run);
    assert!(host.apply_identity_check(host.revision(), &check,
        check.evidence().control_sequence(), check.evidence().revocation_epoch()).is_err());
    run.commit_with_clock(&mut host, run.revision(), || ElapsedTick(1)).unwrap();
    assert_eq!(run.status(), Status::Committed);
    roles.identity_observer = Some(run.take_observer().unwrap());
    assert_eq!(run.take_observer().err(), Some(Error::Missing));
    install(&mut host, &check); publish(&mut host, &roles);
    let disk = FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    assert_eq!(disk.payload, b"visible"); assert_eq!(disk.executions, 1);
}

#[test]
fn cancellation_at_every_token_boundary_keeps_work_and_requires_a_fresh_challenge() {
    for count in 0..=4 {
        let root = Directory::new();
        let (mut host, mut roles, _, _) = setup(&root, true, fixture::model(1.0));
        let check = challenge(&mut host, 1); let mut run = begin(&host, &mut roles, &check);
        for _ in 0..count { run.advance_with_clock(&host, run.revision(), || ElapsedTick(1)).unwrap(); }
        let work = run.work(); assert_eq!(work.completed_tokens, count);
        assert!(run.result().is_none());
        run.cancel(&mut host, run.revision()).unwrap();
        assert_eq!(run.status(), Status::Cancelled); assert_eq!(run.work(), work);
        assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Unavailable);
        let observer = run.take_observer().unwrap(); let revision = host.revision();
        assert!(observer.observe_computed_learned(&mut host, revision, &check,
            input(check.evidence().passport(), 701), || panic!("cancelled check cannot restart")).is_err());
        let fresh = challenge(&mut host, 2);
        assert_eq!(computed(&mut host, &observer, &fresh, 1).observation.measurement.unwrap().outcome,
            IdentityOutcome::Matched);
        assert!(run.result().is_none()); assert_eq!(host.inspect().executions, 0);
    }
}

#[test]
fn setup_refusal_returns_the_unique_observer_without_consuming_work_or_challenge() {
    let root = Directory::new(); let model = fixture::model(1.0);
    let (mut host, mut roles, _, passport) = setup(&root, true, model.clone());
    let check = challenge(&mut host, 1); let before = root.bytes();
    let products: u64 = passport.anchors().values().map(|a|
        model.estimate(0, a.stimulus().len()).unwrap().scalar_products().unwrap()).sum();
    let mut request = input(&passport, 701); request.budget.scalar_products = products - 1;
    let error = roles.identity_observer.take().unwrap().begin_computed_learned(&host,
        host.revision(), &check, request.clone(), ElapsedTick(1)).unwrap_err();
    assert_eq!(error.error, Error::Limit.into()); assert_eq!(root.bytes(), before);
    request.budget.scalar_products = products;
    let mut run = error.observer.begin_computed_learned(&host, host.revision(), &check,
        request, ElapsedTick(1)).unwrap();
    drive(&host, &mut run);
    assert_eq!(run.work().completed_scalar_products, products);
    assert_eq!(run.commit_with_clock(&mut host, run.revision(), || ElapsedTick(1)).unwrap()
        .observation.measurement.as_ref().unwrap().outcome, IdentityOutcome::Matched);
}

#[test]
fn stale_cursor_and_foreign_host_do_no_work_but_changed_owner_cut_retires_preparation() {
    for changed_source in [false, true] {
        let root = Directory::new();
        let (mut host, mut roles, _, _) = setup(&root, true, fixture::model(1.0));
        let check = challenge(&mut host, 1); let mut run = begin(&host, &mut roles, &check);
        let work = run.work();
        assert_eq!(run.advance_with_clock(&host, 1, || panic!("stale cursor")), Err(Error::Stale.into()));
        let other_root = Directory::new();
        let (other, _, _, _) = setup(&other_root, true, fixture::model(1.0));
        assert_eq!(run.advance_with_clock(&other, 0, || panic!("foreign owner")), Err(Error::Binding.into()));
        assert_eq!(run.revision(), 0); assert_eq!(run.work(), work);
        run.advance_with_clock(&host, 0, || ElapsedTick(1)).unwrap();
        let work = run.work();
        if changed_source { step(&mut host); }
        else { host.observe_time(host.revision(), ElapsedTick(2)).unwrap(); }
        assert_eq!(run.advance_with_clock(&host, run.revision(), || panic!("changed cut before inference")),
            Err(Error::Stale.into()));
        assert_eq!(run.status(), Status::Failed); assert_eq!(run.work(), work);
        assert_eq!(run.take_observer().err(), Some(Error::WrongState));
        run.cancel(&mut host, run.revision()).unwrap();
        assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
        assert!(run.take_observer().is_ok());
    }
}

#[test]
fn interrupted_or_late_token_receipts_retain_completed_work_without_publishing_frames() {
    for kind in 0..3 {
        let root = Directory::new();
        let (mut host, mut roles, _, _) = setup(&root, true, fixture::model(1.0));
        let check = challenge(&mut host, 1); let mut run = begin(&host, &mut roles, &check);
        let before = root.bytes(); let mut calls = 0;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run.advance_with_clock(&host, 0, || {
                calls += 1;
                if calls == 2 {
                    if kind == 0 { panic!("post-token interruption"); }
                    return ElapsedTick(if kind == 1 { 0 } else { 21 });
                }
                ElapsedTick(1)
            })
        }));
        assert_eq!(calls, 2);
        if kind == 0 { assert!(result.is_err()); }
        else { assert_eq!(result.unwrap(), Err(Error::Stale.into())); }
        assert_eq!(run.status(), Status::Failed); assert_eq!(run.work().completed_tokens, 1);
        assert!(!run.interrupted()); assert!(run.result().is_none());
        assert_eq!(root.bytes(), before); assert!(host.identity_report(1).unwrap().observations.is_empty());
        assert_eq!(run.take_observer().err(), Some(Error::WrongState));
        run.cancel(&mut host, run.revision()).unwrap();
        assert!(run.take_observer().is_ok());
        assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Unavailable);
    }
}

#[test]
fn late_final_commit_records_original_withdrawal_instead_of_backdating_the_match() {
    for expired in [false, true] {
        let root = Directory::new();
        let (mut host, mut roles, config, _) = setup(&root, true, fixture::model(1.0));
        let check = challenge(&mut host, 1); let mut run = begin(&host, &mut roles, &check);
        drive(&host, &mut run); let work = run.work();
        let result = run.commit_with_clock(&mut host, run.revision(),
            || ElapsedTick(if expired { 21 } else { 20 })).unwrap().clone();
        assert_eq!(result.work, work); assert_eq!(run.status(), Status::Committed);
        if expired {
            assert_eq!(result.observation.measurement, Err(Error::Stale));
            assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Unavailable);
            assert!(host.identity_installation(1).unwrap().is_none());
        } else { install(&mut host, &check); }
        assert!(run.take_observer().is_ok());
        FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    }
}

#[test]
fn failed_canonical_commit_keeps_candidate_private_and_recovery_withdraws_old_challenge() {
    let root = Directory::new();
    let (mut host, mut roles, config, passport) = setup(&root, true, fixture::model(1.0));
    let check = challenge(&mut host, 1); let mut run = begin(&host, &mut roles, &check);
    drive(&host, &mut run); let work = run.work(); let before = root.bytes();
    let expected = requirements(&host, &passport);
    let result = run.commit_with_clock(&mut host, run.revision(), || {
        std::fs::write(root.store().join("delivery.pending"), b"staging conflict").unwrap(); ElapsedTick(1)
    });
    assert!(matches!(result, Err(JournalError::Io(_))));
    assert_eq!(run.work(), work); assert!(run.result().is_none()); assert_eq!(run.status(), Status::Failed);
    assert_eq!(root.bytes(), before); assert_eq!(host.identity_report(1).err(), Some(JournalError::Unavailable));
    assert_eq!(run.take_observer().err(), Some(Error::WrongState));
    assert_eq!(run.cancel(&mut host, run.revision()), Err(JournalError::Unavailable));
    drop(host);
    let (mut host, fresh) = FileOversight::open_guarded_with_learned_generation(root.store(), profile(), &expected, &config).unwrap();
    host.observe_time(host.revision(), ElapsedTick(2)).unwrap();
    assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Unavailable);
    assert_eq!(host.identity_status().unwrap(), IdentityStatus::Missing);
    let next = challenge(&mut host, 2);
    computed(&mut host, fresh.identity_observer.as_ref().unwrap(), &next, 2); install(&mut host, &next);
    assert!(host.learned_generation_inspection().unwrap().paused);
    assert!(run.cancel(&mut host, run.revision()).is_err());
}

#[test]
fn complete_actual_mismatch_and_manifest_refusal_use_original_containment_at_acknowledgment() {
    for manifest_mismatch in [false, true] {
        let root = Directory::new();
        let (mut host, mut roles, config, passport) = setup(&root, true,
            fixture::model(if manifest_mismatch { 1.0 } else { 2.0 }));
        let check = challenge(&mut host, 1); let mut request = input(&passport, 701);
        if manifest_mismatch { request.observed_manifest.weights[0] ^= 1; }
        let mut run = roles.identity_observer.take().unwrap().begin_computed_learned(&host,
            host.revision(), &check, request, ElapsedTick(1)).unwrap();
        assert!(run.result().is_none()); assert!(!host.inspect().control.suspended);
        if manifest_mismatch { assert_eq!(run.status(), Status::ReadyToCommit); }
        else { drive(&host, &mut run); }
        let result = run.commit_with_clock(&mut host, run.revision(), || ElapsedTick(1)).unwrap();
        assert!(matches!(result.observation.measurement.as_ref().unwrap().outcome, IdentityOutcome::Mismatch(_)));
        assert!(result.observation.containment.as_ref().unwrap().is_ok());
        assert_eq!(result.work.entered_tokens, if manifest_mismatch { 0 } else { 4 });
        assert!(host.inspect().control.suspended); assert_eq!(host.inspect().executions, 0);
        FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    }
}

fn overflow_model(overflow: bool) -> DecoderModel {
    let p = fixture::model(1.0).profile().clone();
    let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2],
        queries: vec![0.0, if overflow { f32::MAX } else { 0.0 }, 0.0, 0.0],
        keys: vec![0.0; 4], values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4],
        feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
    DecoderModel::new(p, vec![1.0, 0.0, -1.0, 0.0, 0.0, 1.0], vec![layer.clone(), layer],
        vec![1.0; 2], vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0]).unwrap()
}
#[test]
fn actual_numerical_failure_is_a_committed_failure_not_an_erased_attempt() {
    for overflow in [false, true] {
        let root = Directory::new();
        let (mut host, mut roles, config, _) = setup(&root, true, overflow_model(overflow));
        let check = challenge(&mut host, 1); let mut run = begin(&host, &mut roles, &check);
        drive(&host, &mut run);
        let result = run.commit_with_clock(&mut host, run.revision(), || ElapsedTick(1)).unwrap().clone();
        if overflow {
            assert_eq!(result.work.entered_tokens, 1); assert_eq!(result.work.completed_tokens, 0);
            assert!(result.work.entered_scalar_product_bound > 0);
            assert_eq!(result.observation.measurement, Err(Error::Overflow));
            assert_eq!(host.identity_report(1).unwrap().outcome, IdentityOutcome::Unavailable);
        } else { assert_eq!(result.observation.measurement.unwrap().outcome, IdentityOutcome::Matched); }
        assert!(run.take_observer().is_ok());
        FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    }
}

#[test]
fn cancellation_cannot_withdraw_a_successor_basis_and_completed_witnesses_still_verify() {
    let root = Directory::new();
    let (mut host, mut roles, config, _) = setup(&root, true, fixture::model(1.0));
    let check = challenge(&mut host, 1); let mut run = begin(&host, &mut roles, &check);
    run.advance_with_clock(&host, 0, || ElapsedTick(1)).unwrap();
    host.identity_unavailable(host.revision(), check.evidence().basis()).unwrap();
    let next = challenge(&mut host, 2); let basis = host.identity_basis().unwrap(); let revision = host.revision();
    run.cancel(&mut host, run.revision()).unwrap();
    assert_eq!(host.revision(), revision); assert_eq!(host.identity_basis().unwrap(), basis);
    let observer = run.take_observer().unwrap();
    computed(&mut host, &observer, &next, 1); install(&mut host, &next);
    let bytes = root.bytes(); let magic = b"FALIDP\0\x01";
    let offsets: Vec<_> = bytes.windows(magic.len()).enumerate().filter(|(_, b)| *b == magic).map(|(i, _)| i).collect();
    assert_eq!(offsets.len(), 1); drop(host);
    let mut corrupt = bytes.clone(); corrupt[offsets[0] + 24] ^= 1;
    std::fs::write(root.store().join("delivery.bin"), &corrupt).unwrap();
    assert_eq!(FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).err(),
        Some(Error::Binding.into()));
    std::fs::write(root.store().join("delivery.bin"), &bytes).unwrap();
    FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
}

#[test]
fn final_clock_interruption_returns_no_result_and_cannot_release_observer_for_retry() {
    for unwind in [false, true] {
        let root = Directory::new();
        let (mut host, mut roles, config, _) = setup(&root, true, fixture::model(1.0));
        let check = challenge(&mut host, 1); let mut run = begin(&host, &mut roles, &check);
        drive(&host, &mut run); let before = root.bytes(); let work = run.work();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run.commit_with_clock(&mut host, run.revision(), || {
                if unwind { panic!("commit receipt-time interruption"); }
                ElapsedTick(0)
            }).cloned()
        }));
        if unwind { assert!(result.is_err()); }
        else { assert_eq!(result.unwrap().err(), Some(Error::Stale.into())); }
        assert_eq!(run.status(), Status::Failed); assert!(run.result().is_none());
        assert_eq!(run.work(), work); assert_eq!(root.bytes(), before);
        assert_eq!(run.take_observer().err(), Some(Error::WrongState));
        assert_eq!(host.identity_report(1).err(), Some(JournalError::Unavailable));
        FileOversight::read_publication_with_learned_generation(root.store(), &profile(), &config).unwrap();
    }
}
