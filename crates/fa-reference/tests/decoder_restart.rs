//! Actual numerical continuation after original all-layer restore/recapture.
#[path = "support/decoder_fixture.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::action::consequence::activation::tensor::kv::model::ModelKvImage;
use fa_reference::Error;

fn budget(model: &DecoderModel, position: usize, count: usize) -> DecoderBudget {
    DecoderBudget { scalar_products: model.estimate(position, count).unwrap().scalar_products().unwrap() }
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }
fn same_cache(left: &ModelKvImage, right: &ModelKvImage) {
    assert_eq!(left.profile(), right.profile()); assert_eq!(left.len(), right.len());
    for id in left.profile().layers().keys() {
        // Source streams/revisions intentionally differ; compare every scalar bit.
        let a = left.layer(*id).unwrap().encode().unwrap();
        let b = right.layer(*id).unwrap().encode().unwrap();
        assert_eq!(&a[156..], &b[156..]);
    }
}
fn restore_budget(checkpoint: &DecoderCheckpoint) -> DecoderRestoreBudget {
    DecoderRestoreBudget { cache_values: checkpoint.cache().normalized_values() }
}

#[test]
fn restored_and_recomputed_sessions_match_uninterrupted_free_running_continuation() {
    let model = model(profile(48)); let prefix = [0, 3, 1, 5];
    let mut uninterrupted = model.recompute(1, &prefix, budget(&model, 0, prefix.len())).unwrap();
    let checkpoint = uninterrupted.checkpoint().unwrap();
    let (mut restored, receipt) = model.restore_checkpoint(&checkpoint, 2, restore_budget(&checkpoint)).unwrap();
    let mut recomputed = model.recompute_checkpoint(&checkpoint, 3, budget(&model, 0, prefix.len())).unwrap();
    assert_eq!(receipt.position, prefix.len() as u64);
    assert_eq!(receipt.values_restored, prefix.len() * model.cache_profile().values_per_token());
    assert_eq!(receipt.bytes_written, receipt.values_restored * 4);
    assert_eq!(receipt.bytes_recaptured, receipt.bytes_written);
    assert!(receipt.staged_write_bytes >= receipt.bytes_written);
    assert_eq!(restored.work(), DecoderWork::default());
    for _ in 0..32 {
        let position = uninterrupted.position();
        assert_eq!(restored.greedy_token().unwrap(), uninterrupted.greedy_token().unwrap());
        assert_eq!(recomputed.greedy_token().unwrap(), uninterrupted.greedy_token().unwrap());
        let a = uninterrupted.advance_greedy(position, budget(&model, position as usize, 1)).unwrap();
        let b = restored.advance_greedy(position, budget(&model, position as usize, 1)).unwrap();
        let c = recomputed.advance_greedy(position, budget(&model, position as usize, 1)).unwrap();
        assert_eq!(a.token, b.token); assert_eq!(a.token, c.token);
        assert_eq!(bits(&a.logits), bits(&b.logits)); assert_eq!(bits(&a.logits), bits(&c.logits));
        same_cache(&uninterrupted.cache_image().unwrap(), &restored.cache_image().unwrap());
        same_cache(&uninterrupted.cache_image().unwrap(), &recomputed.cache_image().unwrap());
    }
    assert_eq!(restored.work().tokens, 32);
    assert_eq!(checkpoint.tokens(), &prefix);
}

#[test]
fn teacher_forced_suffix_matches_fresh_full_prefix_at_every_step() {
    let model = model(profile(20)); let prefix = [2, 4, 1];
    let original = model.recompute(1, &prefix, budget(&model, 0, 3)).unwrap();
    let checkpoint = original.checkpoint().unwrap();
    let (mut session, _) = model.restore_checkpoint(&checkpoint, 2, restore_budget(&checkpoint)).unwrap();
    let mut tokens = prefix.to_vec();
    for token in [5, 0, 3, 2, 5, 1] {
        let position = session.position();
        let step = session.advance(position, token, budget(&model, position as usize, 1)).unwrap();
        tokens.push(token);
        let full = model.recompute(20 + position, &tokens, budget(&model, 0, tokens.len())).unwrap();
        assert_eq!(bits(&step.logits), bits(full.logits().unwrap()));
        same_cache(&session.cache_image().unwrap(), &full.cache_image().unwrap());
    }
}

#[test]
fn branches_share_checkpoint_not_mutable_cache_and_keep_original_token_ids() {
    let model = model(profile(8)); let session = model.recompute(1, &[5, 0], budget(&model, 0, 2)).unwrap();
    let checkpoint = session.checkpoint().unwrap(); let saved = checkpoint.cache().encode().unwrap();
    let (mut a, _) = model.restore_checkpoint(&checkpoint, 2, restore_budget(&checkpoint)).unwrap();
    let (mut b, _) = model.restore_checkpoint(&checkpoint, 3, restore_budget(&checkpoint)).unwrap();
    let b_before = b.cache_image().unwrap().encode().unwrap();
    a.advance(2, 1, budget(&model, 2, 1)).unwrap();
    assert_eq!(b.cache_image().unwrap().encode().unwrap(), b_before);
    b.advance(2, 4, budget(&model, 2, 1)).unwrap();
    assert_eq!(a.tokens(), &[5, 0, 1]); assert_eq!(b.tokens(), &[5, 0, 4]);
    assert_ne!(bits(a.logits().unwrap()), bits(b.logits().unwrap()));
    assert_eq!(checkpoint.cache().encode().unwrap(), saved); assert_eq!(session.tokens(), &[5, 0]);
}

#[test]
fn independently_reconstructed_same_named_weights_cannot_claim_checkpoint_ownership() {
    let first = model(profile(8)); let other = model(profile(8));
    let session = first.recompute(1, &[2], budget(&first, 0, 1)).unwrap();
    let checkpoint = session.checkpoint().unwrap();
    assert_eq!(other.restore_checkpoint(&checkpoint, 2, restore_budget(&checkpoint)).unwrap_err(), Error::Binding);
    assert_eq!(other.recompute_checkpoint(&checkpoint, 2, budget(&other, 0, 1)).unwrap_err(), Error::Binding);
    let shared = first.clone();
    assert!(shared.restore_checkpoint(&checkpoint, 2, restore_budget(&checkpoint)).is_ok());
}

#[test]
fn checkpoint_pins_parameters_cache_and_logits_after_original_owners_drop() {
    let checkpoint = {
        let model = model(profile(8));
        model.recompute(1, &[3, 0, 5], budget(&model, 0, 3)).unwrap().checkpoint().unwrap()
    };
    let model = checkpoint.model().clone();
    let expected = bits(checkpoint.logits().unwrap());
    let (mut restored, _) = model.restore_checkpoint(&checkpoint, 2, restore_budget(&checkpoint)).unwrap();
    drop(checkpoint);
    assert_eq!(bits(restored.logits().unwrap()), expected);
    restored.advance_greedy(3, budget(&model, 3, 1)).unwrap();
    assert_eq!(restored.position(), 4);
}

#[test]
fn restore_limits_and_source_stream_reuse_refuse_without_mutating_checkpoint() {
    let model = model(profile(4)); let session = model.recompute(1, &[0, 1], budget(&model, 0, 2)).unwrap();
    let checkpoint = session.checkpoint().unwrap(); let before = checkpoint.cache().encode().unwrap();
    let mut short = restore_budget(&checkpoint); short.cache_values -= 1;
    assert_eq!(model.restore_checkpoint(&checkpoint, 2, short).unwrap_err(), Error::Limit);
    for stream in [0, 1] {
        assert_eq!(model.restore_checkpoint(&checkpoint, stream, restore_budget(&checkpoint)).unwrap_err(), Error::InvalidInput);
        assert_eq!(model.recompute_checkpoint(&checkpoint, stream, budget(&model, 0, 2)).unwrap_err(), Error::InvalidInput);
    }
    assert_eq!(checkpoint.cache().encode().unwrap(), before);
    assert!(model.restore_checkpoint(&checkpoint, 2, restore_budget(&checkpoint)).is_ok());
}

#[test]
fn empty_and_full_context_restarts_preserve_their_actual_frontiers() {
    let model = model(profile(2));
    let empty = model.session(1).unwrap().checkpoint().unwrap();
    let (mut restored, receipt) = model.restore_checkpoint(&empty, 2, DecoderRestoreBudget { cache_values: 0 }).unwrap();
    assert_eq!(receipt.values_restored, 0); assert_eq!(receipt.staged_write_bytes, 0);
    assert_eq!(restored.greedy_token(), Err(Error::Incomplete));
    restored.advance(0, 0, budget(&model, 0, 1)).unwrap();
    restored.advance(1, 1, budget(&model, 1, 1)).unwrap();
    let full = restored.checkpoint().unwrap();
    let (mut again, _) = model.restore_checkpoint(&full, 3, restore_budget(&full)).unwrap();
    let before = again.cache_image().unwrap().encode().unwrap();
    assert_eq!(again.advance_greedy(2, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap_err(), Error::Limit);
    assert_eq!(again.cache_image().unwrap().encode().unwrap(), before);
}

#[test]
fn failed_greedy_step_does_not_publish_a_selected_token_or_spend_compute_count() {
    let model = model(profile(4)); let mut session = model.recompute(1, &[1], budget(&model, 0, 1)).unwrap();
    let expected = session.greedy_token().unwrap(); let work = session.work();
    let before = session.cache_image().unwrap().encode().unwrap();
    assert_eq!(session.advance_greedy(1, DecoderBudget { scalar_products: 0 }).unwrap_err(), Error::Limit);
    assert_eq!(session.tokens(), &[1]); assert_eq!(session.work(), work);
    assert_eq!(session.greedy_token().unwrap(), expected);
    assert_eq!(session.cache_image().unwrap().encode().unwrap(), before);
    let step = session.advance_greedy(1, budget(&model, 1, 1)).unwrap(); assert_eq!(step.token, expected);
}

#[test]
fn restored_prefix_has_derived_capture_identity_not_forged_historical_receipts() {
    let model = model(profile(8)); let session = model.recompute(7, &[1, 4, 2], budget(&model, 0, 3)).unwrap();
    let checkpoint = session.checkpoint().unwrap();
    let (resumed, receipt) = model.restore_checkpoint(&checkpoint, 8, restore_budget(&checkpoint)).unwrap();
    assert_eq!(receipt.source, checkpoint.cache().descriptor());
    let image = resumed.cache_image().unwrap();
    for id in image.profile().layers().keys() {
        let old = checkpoint.cache().layer(*id).unwrap().descriptor();
        let new = image.layer(*id).unwrap().descriptor();
        assert_eq!(old.stream, 7); assert_eq!(new.stream, 8);
        assert_eq!(old.source_revision, 3); assert_eq!(new.source_revision, 1);
        assert_eq!(old.token_count, new.token_count);
    }
    same_cache(&image, checkpoint.cache());
}

#[test]
fn fixed_nonzero_logits_match_independently_computed_literal_reference() {
    // Computed separately with binary32-rounded, layer-major scalar arithmetic.
    // The tolerance is explicit; this does not certify vendor-kernel bit identity.
    let model = model(profile(8));
    let tokens = [0, 3, 1, 4, 2, 5];
    let session = model.recompute(1, &tokens, budget(&model, 0, tokens.len())).unwrap();
    let expected = [-0.16224180161952972_f64, -0.5785757899284363, 0.5557141900062561,
        -0.05130103603005409, -0.6218520998954773, 0.6666550040245056];
    for (actual, expected) in session.logits().unwrap().iter().zip(expected) {
        assert!((f64::from(*actual) - expected).abs() < 0.000002);
    }
}
