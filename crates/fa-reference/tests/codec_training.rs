//! Original decoder and fitter controls; small weights are not training quality.
#![forbid(unsafe_code)]
#[path = "support/restart_model.rs"]
#[allow(dead_code)]
mod fixture;
use fa_reference::Error;
use fa_reference::action::consequence::activation::tensor::kv::{
    decoder::{DecoderBudget, DecoderLayerWeights, DecoderModel, MAX_DECODER_PRODUCTS,
        monitoring::{LearnedDecoderPolicy, LearnedStreamRetention},
        sampling::{SamplingPolicy, SamplingStart, monitored::{GenerationBudget, GenerationSpec, GenerationStatus, GenerationStop}}},
    model::{ModelKvImage, learned::{FitBudget, LearnedKvCodec, LearnedKvPolicy,
        replay::{LearnedKvFitCheckpoint, archive::{LearnedKvFitArchive, MAX_FIT_ARCHIVE_BYTES},
            training::{CodecTraining, CodecTrainingBudget, CodecTrainingInput, CodecTrainingStatus as Status}}}},
};
use std::collections::{BTreeMap, BTreeSet};

fn inputs() -> Vec<CodecTrainingInput> {
    vec![CodecTrainingInput { origin: 102, stream: 12, tokens: vec![2, 0, 1] },
        CodecTrainingInput { origin: 101, stream: 11, tokens: vec![0, 1] }]
}
fn policy() -> LearnedKvPolicy { LearnedKvPolicy::new(1, 1, 1, 8).unwrap() }
fn original(model: &DecoderModel, corpus: &[CodecTrainingInput]) -> (LearnedKvCodec, LearnedKvFitCheckpoint) {
    let sources: BTreeMap<u64, ModelKvImage> = corpus.iter().map(|input| {
        (input.origin, model.recompute(input.stream, &input.tokens, DecoderBudget { scalar_products: MAX_DECODER_PRODUCTS })
            .unwrap().cache_image().unwrap())
    }).collect();
    LearnedKvCodec::fit_with_checkpoint(policy(), &sources, FitBudget::default()).unwrap()
}
fn captured(run: &mut CodecTraining) {
    while run.status() == Status::Capturing { run.advance(run.revision()).unwrap(); }
    assert_eq!(run.status(), Status::ReadyToFit);
    assert!(!run.report().fit_attempted);
}
fn fitted(mut run: CodecTraining) -> (LearnedKvCodec, LearnedKvFitCheckpoint) {
    captured(&mut run);
    assert_eq!(run.advance(run.revision()), Ok(Status::Fitted));
    let (codec, checkpoint, report) = run.finish().unwrap();
    assert!(report.fit_attempted && report.all_inference_reported);
    assert_eq!(report.fit.as_ref(), Some(codec.fit_report()));
    (codec, checkpoint)
}
fn same(a: &LearnedKvCodec, b: &LearnedKvCodec) {
    assert_eq!(a.policy(), b.policy()); assert_eq!(a.profile(), b.profile());
    assert_eq!(a.fit_report(), b.fit_report());
    assert!(a.groups().keys().eq(b.groups().keys()));
    for (key, basis) in a.groups() {
        assert_eq!(fixture::logits(basis.mean()), fixture::logits(b.groups()[key].mean()));
        assert_eq!(fixture::logits(basis.axes()), fixture::logits(b.groups()[key].axes()));
    }
}

#[test]
fn actual_token_corpus_matches_independent_inference_fit_and_exact_archive() {
    let model = fixture::model(); let corpus = inputs();
    let (expected, saved) = original(&model, &corpus);
    let mut run = model.begin_codec_training(corpus, policy(), CodecTrainingBudget::default()).unwrap();
    assert_eq!(run.current_origin(), Some(101));
    let mut accepted = 0;
    while run.status() == Status::Capturing {
        let revision = run.revision(); run.advance(revision).unwrap();
        let total = run.report().documents.values().map(|row| row.accepted.tokens).sum::<u64>();
        assert_eq!(total, accepted + 1); accepted = total;
        assert!(!run.report().fit_attempted);
        assert_eq!(run.revision(), revision + 1);
    }
    assert_eq!(accepted, 5); assert_eq!(run.status(), Status::ReadyToFit);
    assert!(run.report().documents.values().all(|row| row.complete));
    assert_eq!(run.report().declared_values, 5 * model.cache_profile().values_per_token());
    assert_eq!(run.report().documents.values().map(|row| row.reserved_decoder_products).sum::<u64>(),
        run.report().admitted_decoder_products);
    run.advance(run.revision()).unwrap();
    let (actual, checkpoint, report) = run.finish().unwrap();
    same(&actual, &expected);
    assert_eq!(checkpoint.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap(), saved.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap());
    let bytes = checkpoint.encode_archive(MAX_FIT_ARCHIVE_BYTES).unwrap();
    let (replayed, _) = LearnedKvFitArchive::decode(&bytes, &saved.binding(), MAX_FIT_ARCHIVE_BYTES)
        .unwrap().replay(FitBudget::default()).unwrap();
    same(&actual, &replayed);
    assert_eq!(report.fit.unwrap().rows_per_group, 5);
}

#[test]
fn cancellation_and_partial_finish_cannot_release_a_partial_codebook() {
    let model = fixture::model();
    for cut in 0..=5 {
        let mut run = model.begin_codec_training(inputs(), policy(), CodecTrainingBudget::default()).unwrap();
        for _ in 0..cut { run.advance(run.revision()).unwrap(); }
        let before = run.report().clone();
        assert_eq!(run.advance(run.revision() + 1), Err(Error::Stale));
        assert_eq!(run.report(), &before);
        run.cancel(run.revision()).unwrap();
        assert_eq!(run.status(), Status::Cancelled);
        assert_eq!(run.report().documents, before.documents);
        assert!(!run.report().fit_attempted && run.report().fit.is_none());
        assert_eq!(run.advance(run.revision()), Err(Error::WrongState));
        assert!(matches!(run.finish(), Err(Error::WrongState)));
    }
    let partial = model.begin_codec_training(inputs(), policy(), CodecTrainingBudget::default()).unwrap();
    assert!(matches!(partial.finish(), Err(Error::Incomplete)));
    fitted(model.begin_codec_training(inputs(), policy(), CodecTrainingBudget::default()).unwrap());
}

#[test]
fn whole_corpus_limits_are_summed_not_reset_for_each_document() {
    let model = fixture::model(); let corpus = inputs();
    let products = corpus.iter().map(|input| model.estimate(0, input.tokens.len()).unwrap().scalar_products().unwrap()).sum();
    let values = 5 * model.cache_profile().values_per_token();
    let exact = CodecTrainingBudget { tokens: 5, source_values: values, decoder_products: products,
        fitting: FitBudget { source_values: values, ..FitBudget::default() } };
    fitted(model.begin_codec_training(corpus.clone(), policy(), exact).unwrap());
    for short in [CodecTrainingBudget { tokens: 4, ..exact },
        CodecTrainingBudget { source_values: values - 1, ..exact },
        CodecTrainingBudget { decoder_products: products - 1, ..exact },
        CodecTrainingBudget { fitting: FitBudget { source_values: values - 1, ..exact.fitting }, ..exact }] {
        assert!(matches!(model.begin_codec_training(corpus.clone(), policy(), short), Err(Error::Limit)));
    }
}

#[test]
fn original_id_stream_vocabulary_and_context_admission_precedes_execution() {
    let model = fixture::model();
    for mutation in 0..7 {
        let mut corpus = inputs();
        let expected = match mutation {
            0 => { corpus[0].origin = 0; Error::InvalidInput }
            1 => { corpus[0].stream = 0; Error::InvalidInput }
            2 => { corpus[0].origin = corpus[1].origin; Error::Duplicate }
            3 => { corpus[0].stream = corpus[1].stream; Error::Duplicate }
            4 => { corpus[0].tokens.clear(); Error::InvalidInput }
            5 => { corpus[0].tokens[0] = 3; Error::InvalidInput }
            _ => { corpus[0].tokens = vec![0; 17]; Error::Limit }
        };
        assert_eq!(model.begin_codec_training(corpus, policy(), CodecTrainingBudget::default()).err(), Some(expected));
    }
    let single = vec![CodecTrainingInput { origin: 1, stream: 1, tokens: vec![0] }];
    assert_eq!(model.begin_codec_training(single, policy(), CodecTrainingBudget::default()).err(), Some(Error::Incomplete));
    let (a, _) = fitted(model.begin_codec_training(inputs(), policy(), CodecTrainingBudget::default()).unwrap());
    let mut reverse = inputs(); reverse.reverse();
    let (b, _) = fitted(model.begin_codec_training(reverse, policy(), CodecTrainingBudget::default()).unwrap());
    same(&a, &b);
}

#[test]
fn original_fit_refusal_retains_capture_work_and_cannot_be_retried_with_a_new_budget() {
    let model = fixture::model();
    let limits = CodecTrainingBudget { fitting: FitBudget { parameter_values: 0, ..FitBudget::default() },
        ..CodecTrainingBudget::default() };
    let mut run = model.begin_codec_training(inputs(), policy(), limits).unwrap();
    captured(&mut run); let before = run.report().documents.clone();
    assert_eq!(run.advance(run.revision()), Err(Error::Limit));
    assert_eq!(run.status(), Status::Failed(Error::Limit));
    assert_eq!(run.report().documents, before);
    assert!(run.report().all_inference_reported && run.report().fit_attempted);
    assert!(run.report().fit.is_none());
    assert_eq!(run.advance(run.revision()), Err(Error::WrongState));
    assert!(matches!(run.finish(), Err(Error::Limit)));
    fitted(model.begin_codec_training(inputs(), policy(), CodecTrainingBudget::default()).unwrap());
}

#[test]
fn trained_codec_drives_original_monitored_generation_and_rejects_training_lineage_as_held_out() {
    let model = fixture::model();
    let (codec, _) = fitted(model.begin_codec_training(inputs(), policy(), CodecTrainingBudget::default()).unwrap());
    let selected = fixture::policy(&model, 0, 1);
    let monitoring = LearnedDecoderPolicy::new(codec, selected.monitor().clone(), LearnedStreamRetention::All,
        selected.preparation(), selected.inference()).unwrap();
    let spec = GenerationSpec::new(vec![0], 3, BTreeSet::new(), SamplingStart {
        policy: SamplingPolicy::new(1, 1, 3, 0.8, 1, 1.0).unwrap(), stream: 71, seed: 173,
    }).unwrap();
    for (stream, origin) in [(11, 201), (12, 201), (21, 101), (21, 102)] {
        assert_eq!(model.monitored_generation(stream, origin, spec.clone(), monitoring.clone(), GenerationBudget::default()).err(),
            Some(Error::Duplicate));
    }
    let mut run = model.monitored_generation(21, 201, spec, monitoring, GenerationBudget::default()).unwrap();
    assert_eq!(run.run_to_stop(), Ok(GenerationStatus::Finished(GenerationStop::TokenLimit)));
    assert_eq!(run.accepted_tokens().len(), 4);
    assert!(run.last_event().unwrap().audit().complete_quiet());
}

#[test]
fn numerical_failure_reports_the_attempt_but_never_a_complete_training_document() {
    for overflow in [false, true] {
        let profile = fixture::model().profile().clone();
        let q = if overflow { f32::MAX } else { 1.0 };
        let layer = DecoderLayerWeights { attention_norm: vec![1.0; 2], queries: vec![q; 4],
            keys: vec![1.0, 0.0, 0.0, 1.0], values: vec![1.0, 0.0, 0.0, 1.0], attention_output: vec![0.0; 4],
            feed_forward_norm: vec![1.0; 2], gate: vec![0.0; 4], up: vec![0.0; 4], down: vec![0.0; 4] };
        let model = DecoderModel::new(profile, vec![1.0; 6], vec![layer.clone(), layer], vec![1.0; 2], vec![0.0; 6]).unwrap();
        let corpus = vec![CodecTrainingInput { origin: 1, stream: 11, tokens: vec![0, 1] }];
        let mut run = model.begin_codec_training(corpus, policy(), CodecTrainingBudget::default()).unwrap();
        if overflow {
            assert_eq!(run.advance(0), Err(Error::Overflow));
            let report = run.report(); assert_eq!(report.documents[&1].attempted_tokens, 1);
            assert_eq!(report.documents[&1].accepted.tokens, 0);
            assert!(report.documents[&1].reserved_decoder_products > 0);
            assert!(!report.documents[&1].complete && !report.all_inference_reported);
            assert!(!report.fit_attempted && report.fit.is_none());
            assert!(matches!(run.finish(), Err(Error::Overflow)));
        } else { fitted(run); }
    }
}
