use super::*;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{Merge, TokenBytes};
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape};

fn tokenizer() -> ByteBpe {
    let mut vocabulary: Vec<_> = (0_u8..=255).map(|b| TokenBytes::Content(vec![b])).collect();
    let mut merges = Vec::new();
    for (left, right, bytes) in [(u32::from(b'a'), u32::from(b'l'), b"al".as_slice()),
        (u32::from(b'l'), u32::from(b'o'), b"lo"), (257, u32::from(b'w'), b"low")] {
        let result = vocabulary.len() as u32;
        vocabulary.push(TokenBytes::Content(bytes.to_vec()));
        merges.push(Merge { left, right, result });
    }
    vocabulary.push(TokenBytes::Control); vocabulary.push(TokenBytes::Control);
    let profile = DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2,
        model_generation: 3, tokenizer_generation: 4, profile_generation: 5 },
        DecoderShape { vocabulary: vocabulary.len(), hidden: 2, intermediate: 2,
            layers: 1, query_heads: 1, cache_heads: 1, context: 16 }, 1e-5, 10000.0).unwrap();
    ByteBpe::new(profile, vocabulary, merges).unwrap()
}

#[test]
fn all_words_accept_original_byte_tokens_and_require_a_separate_terminal() {
    let tokenizer = tokenizer();
    for word in WORDS {
        let mut grammar = VerdictGrammar::new(&tokenizer, &[259, 260]).unwrap();
        for byte in word {
            assert!(!grammar.allowed().unwrap().contains(&259));
            let next = grammar.next(u32::from(*byte)).unwrap(); grammar.commit(next);
        }
        assert_eq!(grammar.allowed().unwrap(), &[259, 260]);
        let next = grammar.next(260).unwrap(); grammar.commit(next);
        assert_eq!(grammar.allowed(), Err(Error::WrongState));
    }
}

#[test]
fn merged_tokens_can_cross_multiple_grammar_edges_without_retokenizing() {
    let tokenizer = tokenizer(); let mut grammar = VerdictGrammar::new(&tokenizer, &[259, 260]).unwrap();
    // Original ID 256 decodes to "al", 258 to "low". The grammar does not
    // insist that the tokenizer would have encoded this word with these ranks.
    let next = grammar.next(256).unwrap(); grammar.commit(next);
    let next = grammar.next(258).unwrap(); grammar.commit(next);
    assert_eq!(grammar.allowed().unwrap(), &[259, 260]);
    assert_eq!(tokenizer.decode(&[256, 258], 5).unwrap(), b"allow");
}

#[test]
fn grammar_rejects_wrong_case_unicode_bytes_control_prefixes_and_suffixes() {
    let tokenizer = tokenizer(); let mut grammar = VerdictGrammar::new(&tokenizer, &[259, 260]).unwrap();
    for token in [u32::from(b'A'), u32::from(b' '), 0xc3, 0xff, 259] {
        assert_eq!(grammar.next(token), Err(Error::Binding));
    }
    for byte in b"allow" {
        let next = grammar.next(u32::from(*byte)).unwrap(); grammar.commit(next);
    }
    for byte in [b'\n', b' ', b'a'] { assert_eq!(grammar.next(u32::from(byte)), Err(Error::Binding)); }
    assert!(VerdictGrammar::new(&tokenizer, &[]).is_err());
    assert!(VerdictGrammar::new(&tokenizer, &[259]).is_err());
    assert!(VerdictGrammar::new(&tokenizer, &[u32::from(b'a'), 259, 260]).is_err());
}
