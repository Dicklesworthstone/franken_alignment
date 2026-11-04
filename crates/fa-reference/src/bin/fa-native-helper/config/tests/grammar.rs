use super::*;
use fa_reference::action::consequence::oversight::helper_client::native::{NativeEvaluationError, NativeEvaluationStatus};
use fa_reference::round::Verdict;

#[test]
fn worker_v5_selects_grammar_after_every_original_checkpoint_loader() {
    for sharded in [false, true] {
        for json in [false, true] {
            let mut fixture = assets::Fixture::new(false); grammar_assets::prefer_invalid_token(&fixture);
            v2_assets::configure(&mut fixture, sharded, json);
            grammar_assets::configure(&mut fixture, "finite-words-v1");
            let manifest = Manifest::parse(fixture.manifest.as_bytes()).unwrap();
            assert_eq!(manifest.verdict_grammar, NativeVerdictGrammar::FiniteWordsV1);
            assert_eq!(manifest.policy, fixture.policy); assert!(manifest.adapter.is_none());
            let mut worker = manifest.load(&mut manifest.asset_budget().unwrap(),
                &mut WeightReadBudget::new(1_048_576, 4096).unwrap()).unwrap();
            assert_eq!(worker.status(), NativeEvaluationStatus::AwaitingInput);
            assert_eq!(worker.verdict_grammar(), NativeVerdictGrammar::FiniteWordsV1);
            assert_eq!(worker.evaluate(&assets::input(b"?")), Ok(Verdict::Allow));
            assert_eq!(worker.sampled_draws(), 2);
        }
    }
}

#[test]
fn worker_v5_unrestricted_selection_and_legacy_schemas_keep_invalid_output_failure() {
    let mut fixture = assets::Fixture::new(false); grammar_assets::prefer_invalid_token(&fixture);
    for phase in 0..3 {
        if phase == 1 { v2_assets::configure(&mut fixture, false, false); }
        if phase == 2 { grammar_assets::configure(&mut fixture, "unrestricted"); }
        let manifest = Manifest::parse(fixture.manifest.as_bytes()).unwrap();
        assert_eq!(manifest.verdict_grammar, NativeVerdictGrammar::Unrestricted);
        let mut worker = manifest.load(&mut manifest.asset_budget().unwrap(),
            &mut WeightReadBudget::new(1_048_576, 4096).unwrap()).unwrap();
        assert_eq!(worker.evaluate(&assets::input(b"?")), Err(NativeEvaluationError::InvalidVerdict));
        assert_eq!(worker.report().unwrap().bytes().unwrap(), b"x");
    }
}

#[test]
fn worker_v5_requires_exact_grammar_and_explicit_adapter_declarations() {
    let mut fixture = assets::Fixture::new(false); v2_assets::configure(&mut fixture, false, false);
    grammar_assets::configure(&mut fixture, "finite-words-v1");
    for (from, to) in [
        (",\"verdict_grammar\":\"finite-words-v1\"", ""),
        ("\"verdict_grammar\":\"finite-words-v1\"", "\"verdict_grammar\":null"),
        ("finite-words-v1", "auto"),
        ("finite-words-v1", "finite-words-v2"),
        (",\"adapter\":null", ""),
        ("\"adapter\":null", "\"adapter\":{}"),
        (",\"rotary\":{\"kind\":\"none\"}", ""),
        ("fa.native-worker/5", "fa.native-worker/4"),
    ] {
        assert!(fixture.manifest.contains(from));
        assert!(Manifest::parse(fixture.manifest.replacen(from, to, 1).as_bytes()).is_err(), "{from}");
    }
    let old = assets::Fixture::new(false);
    assert!(Manifest::parse(old.manifest.replace("\"stream\":12",
        "\"stream\":12,\"verdict_grammar\":\"finite-words-v1\"").as_bytes()).is_err());
}
