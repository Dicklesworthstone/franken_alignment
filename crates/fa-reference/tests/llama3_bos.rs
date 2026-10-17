//! Public controls for the exact opt-in BOS template and original BPE cursor.
#[path = "llama3_bos/fixture.rs"]
mod fixture;
#[path = "llama3_bos/import.rs"]
mod import;
#[path = "llama3_bos/archive.rs"]
mod archive;

use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, PreTokenization, TokenizationBudget, TokenizationProgress, TokenizationWork, MAX_INPUT_BYTES,
};
use fa_reference::Error;
use fixture::{BOS, BOS_NAME, TEMPLATE, document, merges, names, plain, profile, tokenizer, vocabulary};

#[test]
fn bos_ids_and_synthetic_span_precede_original_unicode_and_literal_control_spans() {
    for tokenizer in [tokenizer(), fixture::import(&document(TEMPLATE)).unwrap()] {
        let source = "a1é<|begin_of_text|>aa".as_bytes();
        let result = tokenizer.encode(source, TokenizationBudget::default()).unwrap();
        // Explicit original IDs; the existing a1 merge is blocked by the grammar,
        // while é and aa still merge. Literal BOS remains a second original ID.
        assert_eq!(result.tokens(), &[259, 114, 66, 258, 259, 256]);
        assert_eq!(result.spans(), &[0..0, 0..1, 1..2, 2..4, 4..21, 21..23]);
        assert_eq!(result.source(), source);
        assert_eq!(tokenizer.bos_token(), Some(BOS));
        assert_eq!(tokenizer.pre_tokenization(), PreTokenization::Llama3Unicode17);
        assert_eq!(tokenizer.decode(result.tokens(), source.len()), Err(Error::Binding));
    }
    let raw = plain(PreTokenization::RawBytes);
    assert_eq!(raw.encode(b"a1", TokenizationBudget::default()).unwrap().tokens(), &[257]);
    let original = plain(PreTokenization::Llama3Unicode17);
    assert_eq!(original.encode(b"a1", TokenizationBudget::default()).unwrap().tokens(), &[114, 66]);
    assert_eq!(original.bos_token(), None);
}

#[test]
fn bos_empty_and_literal_inputs_follow_the_fixed_template_without_deduplication() {
    let tokenizer = tokenizer();
    let empty = tokenizer.encode(b"", TokenizationBudget::default()).unwrap();
    assert_eq!(empty.tokens(), &[259]);
    assert_eq!(empty.spans(), &[0..0]);
    assert!(empty.source().is_empty());
    assert_eq!(empty.work(), TokenizationWork::default());
    let literal = tokenizer.encode(BOS_NAME.as_bytes(), TokenizationBudget::default()).unwrap();
    assert_eq!(literal.tokens(), &[259, 259]);
    assert_eq!(literal.spans(), &[0..0, 0..17]);
    assert_eq!(literal.source(), BOS_NAME.as_bytes());
}

#[test]
fn bos_cooperative_encoding_retains_exact_original_work_and_sticky_failures() {
    let tokenizer = tokenizer();
    let mut cursor = tokenizer.begin_encode(b"aaa", TokenizationBudget::default()).unwrap();
    assert_eq!(cursor.work(), TokenizationWork { input_bytes: 3, ..TokenizationWork::default() });
    while cursor.advance().unwrap() != TokenizationProgress::Complete {}
    assert_eq!(cursor.advance().unwrap(), TokenizationProgress::Complete);
    let result = cursor.finish().unwrap();
    assert_eq!(result.tokens(), &[259, 256, 114]);
    assert_eq!(result.spans(), &[0..0, 0..2, 2..3]);
    assert_eq!(result.work(), TokenizationWork { input_bytes: 3, pair_lookups: 3, heap_pops: 2, merges: 1 });
    let synchronous = tokenizer.encode(b"aaa", TokenizationBudget::default()).unwrap();
    assert_eq!(synchronous.work(), result.work());
    assert_eq!(synchronous.tokens(), result.tokens());
    let budget = TokenizationBudget { input_bytes: 3, pair_lookups: 2, heap_pops: 2 };
    let mut cursor = tokenizer.begin_encode(b"aaa", budget).unwrap();
    let failure = loop {
        match cursor.advance() {
            Err(failure) => break failure,
            Ok(TokenizationProgress::Pending) => {}
            Ok(TokenizationProgress::Complete) => panic!("underfunded encoding completed"),
        }
    };
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work, TokenizationWork { input_bytes: 3, pair_lookups: 2, heap_pops: 1, merges: 1 });
    assert_eq!(cursor.advance(), Err(failure));
    assert_eq!(cursor.finish().unwrap_err(), failure);
    let cursor = tokenizer.begin_encode(b"aaa", TokenizationBudget::default()).unwrap();
    let work = cursor.work();
    assert_eq!(cursor.cancel(), work);
    assert_eq!(tokenizer.begin_encode(b"", TokenizationBudget::default()).unwrap()
        .finish().unwrap_err().error, Error::Incomplete);
}

#[test]
fn bos_input_admission_precedes_insertion_and_never_returns_an_invalid_prefix() {
    let tokenizer = tokenizer();
    let budget = TokenizationBudget { input_bytes: 1, ..TokenizationBudget::default() };
    let oversized = tokenizer.begin_encode(b"aa", budget).unwrap_err();
    assert_eq!(oversized.error, Error::Limit);
    assert_eq!(oversized.work, TokenizationWork::default());
    let invalid = tokenizer.begin_encode(b"a\xff", TokenizationBudget::default()).unwrap_err();
    assert_eq!(invalid.error, Error::InvalidInput);
    assert_eq!(invalid.work, TokenizationWork { input_bytes: 2, ..TokenizationWork::default() });
    assert_eq!(tokenizer.encode(b"a", budget).unwrap().tokens(), &[259, 114]);
}

#[test]
fn bos_counts_toward_the_original_token_ceiling_without_reducing_the_byte_ceiling() {
    let tokenizer = tokenizer();
    let exact = vec![b'b'; MAX_INPUT_BYTES - 1];
    let result = tokenizer.encode(&exact, TokenizationBudget::default()).unwrap();
    assert_eq!(result.tokens().len(), MAX_INPUT_BYTES);
    assert_eq!(result.tokens()[0], BOS);
    assert!(result.tokens()[1..].iter().all(|token| *token == 115));
    assert_eq!(result.spans().last(), Some(&(MAX_INPUT_BYTES - 2..MAX_INPUT_BYTES - 1)));
    let uncompressed = vec![b'b'; MAX_INPUT_BYTES];
    let mut cursor = tokenizer.begin_encode(&uncompressed, TokenizationBudget::default()).unwrap();
    let failure = loop {
        match cursor.advance() {
            Err(failure) => break failure,
            Ok(TokenizationProgress::Pending) => {}
            Ok(TokenizationProgress::Complete) => panic!("one-over token count completed"),
        }
    };
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work, TokenizationWork { input_bytes: MAX_INPUT_BYTES,
        pair_lookups: MAX_INPUT_BYTES - 1, heap_pops: 0, merges: 0 });
    assert_eq!(cursor.advance(), Err(failure));
    assert_eq!(cursor.finish().unwrap_err(), failure);
    // Maximum source bytes remain legal when the original merge pass fits the
    // output ceiling, so the prefix does not silently lower the input limit.
    let compressed = vec![b'a'; MAX_INPUT_BYTES];
    let result = tokenizer.encode(&compressed, TokenizationBudget::default()).unwrap();
    assert_eq!(result.tokens().len(), MAX_INPUT_BYTES / 2 + 1);
    assert_eq!(result.tokens()[0], BOS);
    assert!(result.tokens()[1..].iter().all(|token| *token == 256));
    assert_eq!(result.source(), compressed);
    let too_many_bytes = vec![b'a'; MAX_INPUT_BYTES + 1];
    let failure = tokenizer.begin_encode(&too_many_bytes, TokenizationBudget::default()).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work, TokenizationWork::default());
}

#[test]
fn bos_construction_requires_an_exact_registered_control_and_binds_its_actual_id() {
    let admitted = tokenizer();
    for bos in [114, 261, u32::MAX] {
        assert_eq!(ByteBpe::new_with_bos_token(profile(), vocabulary(), merges(), names(), bos)
            .unwrap_err(), Error::Binding);
    }
    let mut missing = names();
    missing.remove(&BOS);
    assert_eq!(ByteBpe::new_with_bos_token(profile(), vocabulary(), merges(), missing, BOS)
        .unwrap_err(), Error::Binding);
    let different = ByteBpe::new_with_bos_token(profile(), vocabulary(), merges(), names(), 260).unwrap();
    assert_eq!(different.encode(b"a", TokenizationBudget::default()).unwrap().tokens(), &[260, 114]);
    assert_ne!(different.to_bytes().unwrap(), admitted.to_bytes().unwrap());
}
