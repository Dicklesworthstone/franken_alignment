//! Public input, bounded cursor, canonical archive and actual inference controls.
#[path = "unicode_pretokenization/fixture.rs"]
mod fixture;
#[path = "unicode_pretokenization/native.rs"]
mod native;
#[path = "unicode_pretokenization/binding.rs"]
mod binding;

use fixture::{Fixture, byte_id, quote};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, PreTokenization, TokenizationBudget, TokenizationProgress, MAX_INPUT_BYTES,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::RotaryScaling;
use fa_reference::Error;

fn encoded(tokenizer: &ByteBpe, input: &[u8]) -> Vec<u32> {
    tokenizer.encode(input, TokenizationBudget::default()).unwrap().tokens().to_vec()
}

#[test]
fn unicode_import_keeps_permitted_merges_but_causally_blocks_cross_piece_pairs() {
    let f = Fixture::new(true);
    let raw = f.imported(PreTokenization::RawBytes);
    assert_eq!(encoded(&raw, b"a1"), vec![f.id(b"a1")]);
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let tokenizer = f.imported(mode);
        let result = tokenizer.encode(b"a1", TokenizationBudget::default()).unwrap();
        assert_eq!(result.tokens(), &[byte_id(b'a'), byte_id(b'1')]);
        assert_eq!(result.spans(), &[0..1, 1..2]);
        assert_eq!(result.source(), b"a1");
        assert_eq!(tokenizer.decode(result.tokens(), 2).unwrap(), b"a1");
        assert_eq!(encoded(&tokenizer, b" a"), vec![f.id(b" a")]);
        assert_eq!(encoded(&tokenizer, "é".as_bytes()), vec![f.id("é".as_bytes())]);
        assert_eq!(tokenizer.pre_tokenization(), mode);
        assert_eq!(tokenizer.to_bytes().unwrap(), f.tokenizer(mode).to_bytes().unwrap());
    }
    // Same inventory, distinct grammars: four numeric scalars vs three at a time.
    assert_eq!(encoded(&f.imported(PreTokenization::Gpt2Unicode17), b"1234"), vec![f.id(b"1234")]);
    assert_eq!(encoded(&f.imported(PreTokenization::Llama3Unicode17), b"1234"),
        vec![f.id(b"123"), byte_id(b'4')]);
}

#[test]
fn unicode_contraction_case_rules_preserve_original_bytes_and_scalar_boundaries() {
    let f = Fixture::new(false);
    let gpt2 = f.imported(PreTokenization::Gpt2Unicode17);
    let llama3 = f.imported(PreTokenization::Llama3Unicode17);
    assert_eq!(encoded(&gpt2, b"'S"), vec![byte_id(b'\''), byte_id(b'S')]);
    assert_eq!(encoded(&llama3, b"'S"), vec![f.id(b"'S")]);
    assert_eq!(encoded(&gpt2, "'ſa".as_bytes()), vec![byte_id(b'\''), f.id("ſa".as_bytes())]);
    let result = llama3.encode("'ſa".as_bytes(), TokenizationBudget::default()).unwrap();
    assert_eq!(result.tokens(), &[f.id("'ſ".as_bytes()), byte_id(b'a')]);
    assert_eq!(result.spans(), &[0..3, 3..4]);
    assert_eq!(llama3.decode(result.tokens(), 4).unwrap(), "'ſa".as_bytes());
    // Combining marks remain separate original bytes; neither mode NFC-folds.
    for tokenizer in [gpt2, llama3] {
        let input = "e\u{301}".as_bytes();
        let result = tokenizer.encode(input, TokenizationBudget::default()).unwrap();
        assert_eq!(result.source(), input);
        assert_eq!(tokenizer.decode(result.tokens(), input.len()).unwrap(), input);
        assert_ne!(result.tokens(), &[f.id("é".as_bytes())]);
    }
}

#[test]
fn unicode_special_recognition_precedes_segmentation_and_closes_lookahead_scope() {
    let f = Fixture::new(true);
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let tokenizer = f.imported(mode);
        assert_eq!(encoded(&tokenizer, b"  a"), vec![byte_id(b' '), f.id(b" a")]);
        let result = tokenizer.encode("  <qé>a1".as_bytes(), TokenizationBudget::default()).unwrap();
        assert_eq!(result.tokens(), &[f.id(b"  "), f.named_id("<qé>".as_bytes()), byte_id(b'a'), byte_id(b'1')]);
        assert_eq!(result.spans(), &[0..2, 2..7, 7..8, 8..9]);
        assert_eq!(result.source(), "  <qé>a1".as_bytes());
        assert_eq!(tokenizer.decode(result.tokens(), 9), Err(Error::Binding));
    }
}

#[test]
fn unicode_refuses_the_complete_invalid_input_while_raw_preserves_arbitrary_bytes() {
    let f = Fixture::new(false);
    let input = b"a\xff1";
    let raw = f.tokenizer(PreTokenization::RawBytes);
    assert_eq!(raw.decode(&encoded(&raw, input), input.len()).unwrap(), input);
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let tokenizer = f.tokenizer(mode);
        let failure = tokenizer.begin_encode(input, TokenizationBudget::default()).unwrap_err();
        assert_eq!(failure.error, Error::InvalidInput);
        assert_eq!(failure.work.input_bytes, input.len());
        assert_eq!((failure.work.pair_lookups, failure.work.heap_pops, failure.work.merges), (0, 0, 0));
        let budget = TokenizationBudget { input_bytes: input.len() - 1, ..TokenizationBudget::default() };
        let failure = tokenizer.begin_encode(input, budget).unwrap_err();
        assert_eq!(failure.error, Error::Limit);
        assert_eq!(failure.work.input_bytes, 0);
        assert_eq!(encoded(&tokenizer, b"a1"), vec![byte_id(b'a'), byte_id(b'1')]);
    }
}

#[test]
fn unicode_cursor_keeps_original_work_budgets_terminal_failures_and_cancellation() {
    let f = Fixture::new(true);
    let source = b" a1  <eos>1234";
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let tokenizer = f.tokenizer(mode);
        let complete = tokenizer.encode(source, TokenizationBudget::default()).unwrap();
        let work = complete.work();
        assert!(work.pair_lookups > 0 && work.heap_pops > 0 && work.merges > 0);
        let exact = TokenizationBudget { input_bytes: source.len(), pair_lookups: work.pair_lookups,
            heap_pops: work.heap_pops };
        let mut cursor = tokenizer.begin_encode(source, exact).unwrap();
        while cursor.advance().unwrap() != TokenizationProgress::Complete {}
        assert_eq!(cursor.advance().unwrap(), TokenizationProgress::Complete);
        let result = cursor.finish().unwrap();
        assert_eq!(result.tokens(), complete.tokens());
        assert_eq!(result.spans(), complete.spans());
        assert_eq!(result.work(), work);
        for budget in [TokenizationBudget { pair_lookups: exact.pair_lookups - 1, ..exact },
            TokenizationBudget { heap_pops: exact.heap_pops - 1, ..exact }]
        {
            let mut cursor = tokenizer.begin_encode(source, budget).unwrap();
            let failure = loop {
                match cursor.advance() {
                    Err(failure) => break failure,
                    Ok(TokenizationProgress::Pending) => {}
                    Ok(TokenizationProgress::Complete) => panic!("underfunded encoding completed"),
                }
            };
            assert_eq!(failure.error, Error::Limit);
            assert_eq!(cursor.advance(), Err(failure));
            assert_eq!(cursor.finish().unwrap_err(), failure);
        }
        let mut cursor = tokenizer.begin_encode(source, exact).unwrap();
        assert_eq!(cursor.advance().unwrap(), TokenizationProgress::Pending);
        let spent = cursor.work();
        assert_eq!(cursor.cancel(), spent);
        assert_eq!(tokenizer.begin_encode(source, exact).unwrap().finish().unwrap_err().error, Error::Incomplete);
    }
}

#[test]
fn unicode_original_byte_ceiling_is_exact_and_empty_input_is_complete() {
    let f = Fixture::new(false);
    let source = "é".repeat(MAX_INPUT_BYTES / 2);
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let tokenizer = f.tokenizer(mode);
        let result = tokenizer.encode(source.as_bytes(), TokenizationBudget::default()).unwrap();
        assert_eq!(result.work().input_bytes, MAX_INPUT_BYTES);
        assert_eq!(tokenizer.decode(result.tokens(), MAX_INPUT_BYTES).unwrap(), source.as_bytes());
        let too_long = format!("{source}a");
        let failure = tokenizer.encode(too_long.as_bytes(), TokenizationBudget::default()).unwrap_err();
        assert_eq!(failure.error, Error::Limit);
        assert_eq!(failure.work.input_bytes, 0);
        let empty = tokenizer.encode(b"", TokenizationBudget::default()).unwrap();
        assert!(empty.tokens().is_empty() && empty.spans().is_empty());
        assert_eq!(empty.work().input_bytes, 0);
    }
}

#[test]
fn unicode_archive_binds_grammar_unicode_prefix_and_optional_original_extensions() {
    for named in [false, true] {
        for scaled in [false, true] {
            let mut f = Fixture::new(named);
            if scaled { f.profile = f.profile.with_rotary_scaling(RotaryScaling::linear(2.0).unwrap()).unwrap(); }
            let raw = f.tokenizer(PreTokenization::RawBytes).to_bytes().unwrap();
            let original = ByteBpe::new_with_special_tokens(f.profile.clone(), f.vocabulary.clone(),
                f.merges.clone(), f.names.clone()).unwrap().to_bytes().unwrap();
            assert_eq!(raw, original);
            let raw_domain: &[u8] = match (named, scaled) {
                (false, false) => b"FABBPE01", (true, false) => b"FABBPE02",
                (false, true) => b"FABBPE03", (true, true) => b"FABBPE04",
            };
            assert_eq!(&raw[..8], raw_domain);
            for (mode, tag) in [(PreTokenization::Gpt2Unicode17, 1), (PreTokenization::Llama3Unicode17, 2)] {
                let tokenizer = f.tokenizer(mode);
                let bytes = tokenizer.to_bytes().unwrap();
                assert_eq!(&bytes[..8], b"FABBPE05");
                assert_eq!(&bytes[120..126], &[tag, 17, 0, 0, u8::from(named) | (u8::from(scaled) << 1), 0]);
                assert_eq!(&bytes[8..120], &raw[8..120]);
                assert_eq!(&bytes[126..], &raw[120..]);
                let restored = ByteBpe::from_bytes(&f.profile, &bytes).unwrap();
                assert_eq!(restored.pre_tokenization(), mode);
                assert_eq!(restored.to_bytes().unwrap(), bytes);
                assert_eq!(encoded(&restored, b"a1 1234"), encoded(&tokenizer, b"a1 1234"));
                for (offset, changed) in [(120, 0), (120, 3), (121, 16), (122, 1), (123, 1), (124, 4), (125, 1)] {
                    let mut bad = bytes.clone(); bad[offset] = changed;
                    assert_eq!(ByteBpe::from_bytes(&f.profile, &bad).unwrap_err(), Error::Binding);
                }
                for end in [0, 8, 119, 120, 121, 125, 126, bytes.len() - 1] {
                    assert!(ByteBpe::from_bytes(&f.profile, &bytes[..end]).is_err());
                }
                let mut trailing = bytes.clone(); trailing.push(0);
                assert!(ByteBpe::from_bytes(&f.profile, &trailing).is_err());
            }
        }
    }
}

#[test]
fn unicode_json_requires_exact_selected_pipeline_and_does_not_widen_legacy_import() {
    let f = Fixture::new(true);
    for mode in [PreTokenization::Gpt2Unicode17, PreTokenization::Llama3Unicode17] {
        let source = f.json(mode);
        assert_eq!(ByteBpe::from_huggingface_json(&f.profile, source.as_bytes(), source.len()).unwrap_err(), Error::Binding);
        let other = if mode == PreTokenization::Gpt2Unicode17 { PreTokenization::Llama3Unicode17 }
            else { PreTokenization::Gpt2Unicode17 };
        assert_eq!(ByteBpe::from_huggingface_json_with_pre_tokenization(&f.profile,
            source.as_bytes(), source.len(), other).unwrap_err(), Error::Binding);
        for (from, to) in [
            ("\"add_prefix_space\":false", "\"add_prefix_space\":true"),
            ("\"normalizer\":null", "\"normalizer\":{\"type\":\"NFC\"}"),
            ("\"post_processor\":null", "\"post_processor\":{\"type\":\"TemplateProcessing\"}"),
            ("\"trim_offsets\":true", "\"trim_offsets\":null"),
            ("\"padding\":null", "\"padding\":{}"),
            ("\"truncation\":null", "\"truncation\":{}"),
        ] {
            assert!(source.contains(from));
            let bad = source.replacen(from, to, 1);
            assert!(ByteBpe::from_huggingface_json_with_pre_tokenization(&f.profile,
                bad.as_bytes(), bad.len(), mode).is_err(), "{from}");
        }
        assert_eq!(encoded(&f.imported(mode), b"a1"), vec![byte_id(b'a'), byte_id(b'1')]);
    }
    let source = f.json(PreTokenization::Llama3Unicode17);
    for (from, to) in [("\"behavior\":\"Isolated\"".to_string(), "\"behavior\":\"Removed\"".to_string()),
        ("\"invert\":false".to_string(), "\"invert\":true".to_string()),
        (quote(fixture::LLAMA3_REGEX), quote(&fixture::LLAMA3_REGEX.replace("{1,3}", "+")))]
    {
        let bad = source.replacen(&from, &to, 1);
        assert_ne!(bad, source);
        assert_eq!(ByteBpe::from_huggingface_json_with_pre_tokenization(&f.profile,
            bad.as_bytes(), bad.len(), PreTokenization::Llama3Unicode17).unwrap_err(), Error::Binding);
    }
}
