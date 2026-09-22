//! Different valid recipes must refuse even at an empty checkpoint, where final
//! cache/logit comparisons cannot expose changed parameters or future monitors.
use super::*;
use super::super::archive::ArchiveLimits;

fn copy_recipe(r: &Recipe) -> Recipe {
    Recipe { model: r.model.clone(), stream: r.stream, evaluation_origin: r.evaluation_origin,
        spec: r.spec.clone(), policy: r.policy.clone(), budget: r.budget, telemetry: r.telemetry }
}
fn empty(recipe: Recipe) -> ReplayableGeneration {
    let run = recipe.start().unwrap();
    ReplayableGeneration { recipe: Rc::new(recipe), run }
}

#[test]
fn independent_valid_recipes_bind_unexercised_parameters_probes_lineage_and_caps() {
    let retained = checkpoint();
    let original = empty(copy_recipe(&retained.recipe));
    let bytes = original.checkpoint(CheckpointLimits::default()).unwrap().encode_archive(ArchiveLimits::default()).unwrap();
    let independent = empty(copy_recipe(&retained.recipe));
    independent.decode_archive(&bytes, ArchiveLimits::default()).unwrap().replay(ReplayBudget::default()).unwrap();
    for changed in 0..17 {
        let mut recipe = copy_recipe(&retained.recipe);
        match changed {
            0..=2 => {
                let model = &recipe.model.data;
                let mut embeddings = model.embeddings.clone();
                let mut output = model.output.clone();
                let mut layers: Vec<_> = model.layers.iter().map(|layer| layer.weights.clone()).collect();
                let word = match changed {
                    0 => &mut embeddings[0], 1 => &mut output[0], _ => &mut layers[1].down[0],
                };
                *word = f32::from_bits(word.to_bits() ^ 1);
                recipe.model = DecoderModel::new(model.profile.clone(), embeddings, layers,
                    model.final_norm.clone(), output).unwrap();
            }
            3..=6 => {
                let mut prompt = recipe.spec.prompt().to_vec();
                let mut sampling = recipe.spec.sampling().clone();
                let mut stops = recipe.spec.stop_tokens().clone();
                let mut count = recipe.spec.max_new_tokens();
                match changed {
                    3 => sampling.seed += 1,
                    4 => prompt[0] = 1,
                    5 => { stops.insert(2); },
                    _ => count -= 1,
                }
                recipe.spec = GenerationSpec::new(prompt, count, stops, sampling).unwrap();
            }
            7 => recipe.stream += 1,
            8 => recipe.evaluation_origin += 1,
            9 => recipe.budget.decoder_products -= 1,
            10 => recipe.telemetry.monitor_encoded_bytes -= 1,
            11 => recipe.policy = LearnedDecoderPolicy::new(recipe.policy.codec().clone(),
                recipe.policy.monitor().clone(), LearnedStreamRetention::None,
                recipe.policy.preparation(), recipe.policy.inference()).unwrap(),
            12..=14 => {
                let original = recipe.policy.monitor();
                let mut taps = original.taps().clone();
                let (tap, row) = original.taps().iter().next().unwrap();
                let tensor = &recipe.model.cache_profile().layers()[&tap.layer];
                let tensor = match tap.side { KvSide::Key => tensor.keys(), KvSide::Value => tensor.values() };
                let mut weights = vec![0.0; tensor.dimensions()];
                let mut threshold = 1.0;
                let mut budget = row.budget();
                match changed {
                    12 => weights[0] = -0.0,
                    13 => threshold = 1.25,
                    _ => budget.encoded_bytes -= 1,
                }
                let probe = LinearProbe::new(1, 1, tensor.profile(), &weights, 0.0, threshold).unwrap();
                taps.insert(*tap, LearnedRefinementMonitor::new(vec![probe], budget).unwrap());
                let monitor = LearnedModelMonitor::new(original.profile().clone(), taps, original.budget()).unwrap();
                recipe.policy = LearnedDecoderPolicy::new(recipe.policy.codec().clone(), monitor,
                    LearnedStreamRetention::All, recipe.policy.preparation(), recipe.policy.inference()).unwrap();
            }
            15..=16 => {
                let stream = if changed == 15 { 12 } else { 11 };
                let origin = if changed == 16 { 102 } else { 101 };
                let source = recipe.model.recompute(stream, &[0, 1], recipe.policy.inference()).unwrap().cache_image().unwrap();
                let codec = LearnedKvCodec::fit(recipe.policy.codec().policy(),
                    &BTreeMap::from([(origin, source)]), FitBudget::default()).unwrap();
                recipe.policy = LearnedDecoderPolicy::new(codec, recipe.policy.monitor().clone(),
                    LearnedStreamRetention::All, recipe.policy.preparation(), recipe.policy.inference()).unwrap();
            }
            _ => unreachable!(),
        }
        let changed_owner = empty(recipe);
        assert!(matches!(changed_owner.decode_archive(&bytes, ArchiveLimits::default()), Err(Error::Binding)), "recipe {changed}");
        assert_eq!(changed_owner.generation().position(), 0);
        assert_eq!(original.generation().position(), 0);
    }
}
