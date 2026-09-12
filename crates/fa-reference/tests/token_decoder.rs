//! Full-sequence layer-major oracle versus the incremental token-major engine.
//! Fixtures are explicit weights, not evidence for a named trained model.
#[path = "support/decoder_fixture.rs"] mod fixture;
use fixture::*;
use fa_reference::action::consequence::activation::tensor::kv::decoder::*;
use fa_reference::Error;

type Oracle = (Vec<Vec<f32>>, Vec<Vec<Vec<f32>>>, Vec<Vec<Vec<f32>>>);
fn oracle(p: &DecoderProfile, tokens: &[u32], embedding: &[f32], ws: &[DecoderLayerWeights], out: &[f32]) -> Oracle {
    let s = p.shape();
    let multiply = |w: &[f32], x: &[f32]| -> Vec<f32> {
        (0..w.len() / x.len()).map(|row| {
            (0..x.len()).fold(0.0, |sum, j| sum + f64::from(w[row * x.len() + j]) * f64::from(x[j])) as f32
        }).collect()
    };
    let norm = |x: &[f32], w: &[f32]| -> Vec<f32> {
        let denominator = (x.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / x.len() as f64 + p.epsilon()).sqrt();
        x.iter().zip(w).map(|(x, w)| ((f64::from(*x) / denominator) * f64::from(*w)) as f32).collect()
    };
    let rotate = |x: &mut [f32], position: usize| {
        let d = p.head_width();
        for head in x.chunks_exact_mut(d) {
            let old = head.to_vec();
            for j in 0..d / 2 {
                let angle = position as f64 / p.theta().powf((2 * j) as f64 / d as f64);
                let (sine, cosine) = angle.sin_cos();
                head[j] = (f64::from(old[j]) * cosine - f64::from(old[j + d / 2]) * sine) as f32;
                head[j + d / 2] = (f64::from(old[j]) * sine + f64::from(old[j + d / 2]) * cosine) as f32;
            }
        }
    };
    let mut states: Vec<Vec<f32>> = tokens.iter().map(|t| embedding[*t as usize * s.hidden..(*t as usize + 1) * s.hidden].to_vec()).collect();
    let mut all_keys = Vec::new();
    let mut all_values = Vec::new();
    for w in ws {
        let normalized: Vec<_> = states.iter().map(|row| norm(row, &w.attention_norm)).collect();
        let mut queries: Vec<_> = normalized.iter().map(|row| multiply(&w.queries, row)).collect();
        let mut keys: Vec<_> = normalized.iter().map(|row| multiply(&w.keys, row)).collect();
        let values: Vec<_> = normalized.iter().map(|row| multiply(&w.values, row)).collect();
        for position in 0..tokens.len() { rotate(&mut queries[position], position); rotate(&mut keys[position], position); }
        for position in 0..tokens.len() {
            let mut mixed = vec![0.0; s.hidden];
            for head in 0..s.query_heads {
                let cache_head = head / (s.query_heads / s.cache_heads);
                let d = p.head_width();
                let q = &queries[position][head * d..(head + 1) * d];
                let scores: Vec<_> = keys[..=position].iter().map(|row| {
                    (0..d).fold(0.0, |sum, j| sum + f64::from(q[j]) * f64::from(row[cache_head * d + j])) / (d as f64).sqrt()
                }).collect();
                let maximum = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let weights: Vec<_> = scores.iter().map(|score| (*score - maximum).exp()).collect();
                let sum: f64 = weights.iter().sum();
                for j in 0..d {
                    mixed[head * d + j] = (0..=position).fold(0.0, |total, t|
                        total + weights[t] / sum * f64::from(values[t][cache_head * d + j])) as f32;
                }
            }
            let projected = multiply(&w.attention_output, &mixed);
            let residual: Vec<_> = states[position].iter().zip(projected).map(|(x, y)| (f64::from(*x) + f64::from(y)) as f32).collect();
            let normalized = norm(&residual, &w.feed_forward_norm);
            let gate = multiply(&w.gate, &normalized);
            let up = multiply(&w.up, &normalized);
            let activated: Vec<_> = gate.iter().zip(up).map(|(g, u)| {
                let x = f64::from(*g);
                let silu = (x / (1.0 + (-x).exp())) as f32;
                (f64::from(silu) * f64::from(u)) as f32
            }).collect();
            let down = multiply(&w.down, &activated);
            states[position] = residual.iter().zip(down).map(|(x, y)| (f64::from(*x) + f64::from(y)) as f32).collect();
        }
        all_keys.push(keys); all_values.push(values);
    }
    let logits = states.iter().map(|row| multiply(out, &norm(row, &vec![1.0; s.hidden]))).collect();
    (logits, all_keys, all_values)
}
fn close(a: &[f32], b: &[f32]) {
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) { assert!((*a - *b).abs() <= 2e-6 * (1.0 + b.abs()), "{a} != {b}"); }
}
fn bits(values: &[f32]) -> Vec<u32> { values.iter().map(|v| v.to_bits()).collect() }
fn budget(model: &DecoderModel, position: usize, count: usize) -> DecoderBudget {
    DecoderBudget { scalar_products: model.estimate(position, count).unwrap().scalar_products().unwrap() }
}

#[test]
fn nonzero_multilayer_gqa_matches_independent_layer_major_full_sequence() {
    let p = profile(16); let s = p.shape();
    let embeddings = values(s.vocabulary * s.hidden, 11);
    let output = values(s.vocabulary * s.hidden, 12);
    let ws = layers(&p); let tokens = [0, 3, 1, 4, 2, 5];
    let (logits, keys, vals) = oracle(&p, &tokens, &embeddings, &ws, &output);
    let model = DecoderModel::new(p.clone(), embeddings, ws, vec![1.0; s.hidden], output).unwrap();
    let mut session = model.session(9).unwrap();
    for (position, token) in tokens.into_iter().enumerate() {
        let step = session.advance(position as u64, token, budget(&model, position, 1)).unwrap();
        close(&step.logits, &logits[position]);
        assert_eq!(step.layers.len(), s.layers);
        for observation in &step.layers {
            assert_eq!(observation.query.source().identity().position, position as u64);
            assert_eq!(observation.residual.source().dimensions(), s.hidden);
        }
    }
    let image = session.cache_image().unwrap();
    for layer in 0..s.layers {
        for position in 0..tokens.len() {
            let row = image.layer(layer as u64 + 1).unwrap().token(position as u64).unwrap();
            // Export uses original scalar serialization, independently read here.
            let encoded = image.layer(layer as u64 + 1).unwrap().encode().unwrap();
            let start = 156 + position * p.cache_width() * 8;
            let decoded: Vec<_> = encoded[start..start + p.cache_width() * 8].chunks_exact(4)
                .map(|b| f32::from_le_bytes(b.try_into().unwrap())).collect();
            close(&decoded[..p.cache_width()], &keys[layer][position]);
            close(&decoded[p.cache_width()..], &vals[layer][position]);
            assert_eq!(row.key().identity().position, position as u64);
        }
    }
}

#[test]
fn original_ids_recompute_same_cache_and_logits_as_incremental_execution() {
    let model = model(profile(16)); let tokens = [5, 0, 5, 2, 3];
    let mut incremental = model.session(7).unwrap();
    for (i, token) in tokens.iter().copied().enumerate() { incremental.advance(i as u64, token, budget(&model, i, 1)).unwrap(); }
    let replay = model.recompute(7, &tokens, budget(&model, 0, tokens.len())).unwrap();
    assert_eq!(replay.tokens(), &tokens);
    assert_eq!(bits(replay.logits().unwrap()), bits(incremental.logits().unwrap()));
    assert_eq!(replay.cache_image().unwrap().encode().unwrap(), incremental.cache_image().unwrap().encode().unwrap());
    assert_eq!(replay.work(), model.estimate(0, tokens.len()).unwrap());
}

#[test]
fn invalid_token_stale_position_and_one_short_budget_leave_every_layer_unchanged() {
    let model = model(profile(4)); let mut session = model.session(1).unwrap();
    session.advance(0, 1, budget(&model, 0, 1)).unwrap();
    let before = session.cache_image().unwrap().encode().unwrap(); let work = session.work();
    assert_eq!(session.advance(0, 1, budget(&model, 1, 1)).unwrap_err(), Error::Stale);
    assert_eq!(session.advance(1, 99, budget(&model, 1, 1)).unwrap_err(), Error::InvalidInput);
    let mut short = budget(&model, 1, 1); short.scalar_products -= 1;
    assert_eq!(session.advance(1, 2, short).unwrap_err(), Error::Limit);
    assert_eq!(session.cache_image().unwrap().encode().unwrap(), before);
    assert_eq!(session.work(), work); assert_eq!(session.tokens(), &[1]);
    session.advance(1, 2, budget(&model, 1, 1)).unwrap();
    assert_eq!(session.tokens(), &[1, 2]);
}

#[test]
fn vocabulary_head_overflow_after_all_layers_does_not_advance_any_prefix() {
    let p = profile(4); let s = p.shape();
    let mut embeddings = vec![0.0; s.vocabulary * s.hidden];
    embeddings[1] = 1.0; embeddings[s.hidden] = 1.0;
    let mut out = vec![0.0; s.vocabulary * s.hidden]; out[0] = f32::MAX;
    let model = DecoderModel::new(p.clone(), embeddings, zero_layers(&p), vec![1.0; s.hidden], out).unwrap();
    let mut session = model.session(1).unwrap(); session.advance(0, 0, budget(&model, 0, 1)).unwrap();
    let before = session.cache_image().unwrap().encode().unwrap(); let logits = bits(session.logits().unwrap());
    let work = session.work();
    assert_eq!(session.advance(1, 1, budget(&model, 1, 1)).unwrap_err(), Error::Overflow);
    assert_eq!(session.cache_image().unwrap().encode().unwrap(), before);
    assert_eq!(bits(session.logits().unwrap()), logits); assert_eq!(session.work(), work);
    session.advance(1, 0, budget(&model, 1, 1)).unwrap();
}

#[test]
fn complete_parameter_inventory_and_finiteness_are_required() {
    let p = profile(4); let s = p.shape();
    let mut bad = layers(&p); bad[1].down.pop();
    assert_eq!(DecoderModel::new(p.clone(), values(24, 1), bad, vec![1.0; 4], values(24, 2)).unwrap_err(), Error::Binding);
    let mut bad = layers(&p); bad[1].up[0] = f32::NAN;
    assert_eq!(DecoderModel::new(p.clone(), values(24, 1), bad, vec![1.0; 4], values(24, 2)).unwrap_err(), Error::InvalidInput);
    let mut missing = layers(&p); missing.pop();
    assert_eq!(DecoderModel::new(p.clone(), values(24, 1), missing, vec![1.0; 4], values(24, 2)).unwrap_err(), Error::Binding);
    assert!(DecoderModel::new(p.clone(), values(s.vocabulary * s.hidden, 1), layers(&p), vec![1.0; 4], values(24, 2)).is_ok());
}

#[test]
fn shape_numeric_and_full_context_limits_are_checked_before_parameter_allocation() {
    let p = profile(4); let id = p.identity(); let s = p.shape();
    for bad in [DecoderShape { hidden: 3, ..s }, DecoderShape { cache_heads: 3, ..s },
        DecoderShape { context: 0, ..s }, DecoderShape { query_heads: 0, ..s }] {
        assert!(DecoderProfile::new(id, bad, p.epsilon(), p.theta()).is_err());
    }
    for (eps, theta) in [(0.0, 100.0), (f64::NAN, 100.0), (0.1, f64::INFINITY), (0.1, 0.0)] {
        assert!(DecoderProfile::new(id, s, eps, theta).is_err());
    }
    let large = DecoderShape { hidden: 2048, intermediate: 8192, vocabulary: 65536, ..s };
    assert_eq!(DecoderProfile::new(id, large, p.epsilon(), p.theta()).unwrap_err(), Error::Limit);
}

#[test]
fn full_recompute_budget_is_not_replenished_for_each_token() {
    let model = model(profile(4));
    assert_eq!(model.recompute(1, &[0, 1, 2], budget(&model, 0, 1)).unwrap_err(), Error::Limit);
    let session = model.recompute(1, &[0, 1, 2], budget(&model, 0, 3)).unwrap();
    assert_eq!(session.work().tokens, 3);
    assert_eq!(model.recompute(1, &[0, 99], budget(&model, 0, 2)).unwrap_err(), Error::InvalidInput);
}

#[test]
fn full_context_refuses_next_token_without_replacing_last_logits() {
    let model = model(profile(2)); let mut session = model.recompute(1, &[0, 1], budget(&model, 0, 2)).unwrap();
    let logits = bits(session.logits().unwrap());
    assert_eq!(session.advance(2, 0, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS }).unwrap_err(), Error::Limit);
    assert_eq!(bits(session.logits().unwrap()), logits); assert_eq!(session.position(), 2);
}

#[test]
fn empty_prefix_is_not_a_prediction_and_zero_stream_is_rejected() {
    let model = model(profile(2)); let session = model.recompute(1, &[], DecoderBudget { scalar_products: 0 }).unwrap();
    assert!(session.cache_image().unwrap().is_empty());
    assert_eq!(session.logits(), Err(Error::Incomplete)); assert_eq!(session.greedy_token(), Err(Error::Incomplete));
    assert_eq!(model.session(0).unwrap_err(), Error::InvalidInput);
}

#[test]
fn exact_greedy_ties_use_lowest_token_id_without_changing_state() {
    let p = profile(4); let s = p.shape();
    let model = DecoderModel::new(p.clone(), vec![1.0; 24], zero_layers(&p), vec![1.0; 4], vec![1.0; 24]).unwrap();
    let session = model.recompute(1, &[3], budget(&model, 0, 1)).unwrap();
    assert_eq!(session.logits().unwrap().len(), s.vocabulary);
    assert_eq!(session.greedy_token().unwrap(), 0); assert_eq!(session.greedy_token().unwrap(), 0);
    assert_eq!(session.tokens(), &[3]);
}

#[test]
fn earlier_snapshot_and_step_remain_immutable_after_cache_growth() {
    let model = model(profile(4)); let mut session = model.session(9).unwrap();
    let step = session.advance(0, 2, budget(&model, 0, 1)).unwrap();
    let old = session.cache_image().unwrap(); let bytes = old.encode().unwrap(); let logits = bits(&step.logits);
    session.advance(1, 4, budget(&model, 1, 1)).unwrap();
    assert_eq!(old.encode().unwrap(), bytes); assert_eq!(old.len(), 1);
    assert_eq!(bits(&step.logits), logits); assert_eq!(session.cache_image().unwrap().len(), 2);
}

#[test]
fn work_counts_match_independent_dense_and_attention_formula() {
    let model = model(profile(4)); let p = model.profile(); let s = p.shape();
    let h = s.hidden as u64; let k = p.cache_width() as u64; let i = s.intermediate as u64;
    let single = s.layers as u64 * (h*h + k*h + k*h + h*h + i*h + i*h + h*i) + s.vocabulary as u64*h;
    let work = model.estimate(0, 3).unwrap();
    assert_eq!(work.matrix_products, single * 3);
    assert_eq!(work.attention_products, s.layers as u64 * s.query_heads as u64 * p.head_width() as u64 * 2 * (1 + 2 + 3));
    assert_eq!(work.cache_values_appended, (3 * s.layers * p.cache_width() * 2) as u64);
}
