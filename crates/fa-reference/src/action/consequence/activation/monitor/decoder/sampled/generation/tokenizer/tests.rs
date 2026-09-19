use super::*;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderShape};

fn profile(vocabulary: usize) -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary,
        hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 64 },
        1e-5, 10000.0).unwrap()
}
fn bytes() -> Vec<TokenBytes> {
    (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect()
}
fn plain() -> ByteBpe { ByteBpe::new(profile(256), bytes(), Vec::new()).unwrap() }
fn merged() -> ByteBpe {
    let mut vocabulary = bytes();
    for token in [b"aa".as_slice(), b"ab", b"bb", b"aaa", b"aab", b"bba", b"aabb"] {
        vocabulary.push(TokenBytes::Content(token.to_vec()));
    }
    let rules = [(97, 97, 256), (97, 98, 257), (98, 98, 258),
        (256, 97, 259), (256, 98, 260), (258, 97, 261), (256, 258, 262)];
    ByteBpe::new(profile(vocabulary.len()), vocabulary,
        rules.into_iter().map(|(left, right, result)| Merge { left, right, result }).collect()).unwrap()
}

// Independent full-vector oracle: no heap, stale-candidate checks, node links,
// or production rank lookup. Rules and original bytes define the reference.
fn reference(tokenizer: &ByteBpe, source: &[u8]) -> Vec<u32> {
    let mut tokens: Vec<u32> = source.iter().map(|byte| {
        tokenizer.0.vocabulary.iter().position(|value| matches!(value,
            TokenBytes::Content(bytes) if bytes.len() == 1 && bytes[0] == *byte)).unwrap() as u32
    }).collect();
    loop {
        let mut selected = None;
        'rules: for rule in &tokenizer.0.merges {
            for index in 0..tokens.len().saturating_sub(1) {
                if (tokens[index], tokens[index + 1]) == (rule.left, rule.right) {
                    selected = Some((index, rule.result));
                    break 'rules;
                }
            }
        }
        let Some((index, token)) = selected else { return tokens; };
        tokens[index] = token;
        tokens.remove(index + 1);
    }
}
fn partition(tokenizer: &ByteBpe, encoded: &TokenizedInput) {
    assert_eq!(encoded.tokens().len(), encoded.spans().len());
    let mut through = 0;
    for (&token, span) in encoded.tokens().iter().zip(encoded.spans()) {
        assert_eq!(span.start, through);
        assert!(span.start < span.end);
        assert_eq!(tokenizer.content_bytes(token).unwrap(), &encoded.source()[span.clone()]);
        through = span.end;
    }
    assert_eq!(through, encoded.source().len());
}

#[test]
fn arbitrary_bytes_utf8_and_literal_controls_are_preserved_without_normalization() {
    let mut vocabulary = bytes(); vocabulary.push(TokenBytes::Control);
    let tokenizer = ByteBpe::new(profile(257), vocabulary, Vec::new()).unwrap();
    for source in [b"<eos>\0\xff\xc0\x80".as_slice(), " e\u{301}\r\n\t世界 ".as_bytes(), &[]] {
        let encoded = tokenizer.encode(source, TokenizationBudget::default()).unwrap();
        assert_eq!(encoded.source(), source);
        assert!(!encoded.tokens().contains(&256));
        assert_eq!(tokenizer.decode(encoded.tokens(), source.len()).unwrap(), source);
        partition(&tokenizer, &encoded);
    }
}

#[test]
fn byte_values_are_not_assumed_to_equal_original_token_ids() {
    let mut vocabulary = bytes(); vocabulary.rotate_left(37);
    let tokenizer = ByteBpe::new(profile(256), vocabulary, Vec::new()).unwrap();
    let encoded = tokenizer.encode(&[0, 37, 255], TokenizationBudget::default()).unwrap();
    assert_eq!(encoded.tokens(), &[219, 0, 218]);
    assert_eq!(tokenizer.decode(encoded.tokens(), 3).unwrap(), [0, 37, 255]);
}

#[test]
fn rank_precedes_longest_match_or_token_id_and_ties_are_leftmost() {
    let mut vocabulary = bytes();
    vocabulary.push(TokenBytes::Content(b"ab".to_vec()));
    vocabulary.push(TokenBytes::Content(b"bc".to_vec()));
    let tokenizer = ByteBpe::new(profile(258), vocabulary, vec![
        Merge { left: 98, right: 99, result: 257 },
        Merge { left: 97, right: 98, result: 256 },
    ]).unwrap();
    assert_eq!(tokenizer.encode(b"abc", TokenizationBudget::default()).unwrap().tokens(), &[97, 257]);
    let tokenizer = merged();
    assert_eq!(tokenizer.encode(b"aaa", TokenizationBudget::default()).unwrap().tokens(), &[259]);
    assert_eq!(tokenizer.encode(b"aaaa", TokenizationBudget::default()).unwrap().tokens(), &[256, 256]);
}

#[test]
fn exhaustive_binary_strings_match_an_independent_rule_scan_and_exact_partition() {
    let tokenizer = merged();
    for length in 0..=10 {
        for mask in 0..(1 << length) {
            let source: Vec<_> = (0..length).map(|bit| if mask & (1 << bit) == 0 { b'a' } else { b'b' }).collect();
            let encoded = tokenizer.encode(&source, TokenizationBudget::default()).unwrap();
            assert_eq!(encoded.tokens(), reference(&tokenizer, &source));
            assert_eq!(tokenizer.decode(encoded.tokens(), source.len()).unwrap(), source);
            assert!(encoded.work().pair_lookups <= 3 * source.len());
            assert!(encoded.work().heap_pops <= 3 * source.len());
            partition(&tokenizer, &encoded);
        }
    }
}

#[test]
fn exact_work_budgets_succeed_and_one_less_returns_no_partial_encoding() {
    let tokenizer = merged(); let source = b"aaaaaaaabbaa";
    let full = tokenizer.encode(source, TokenizationBudget::default()).unwrap();
    let work = full.work();
    assert!(work.heap_pops > work.merges); // stale candidates really occur
    let budget = TokenizationBudget { input_bytes: source.len(),
        pair_lookups: work.pair_lookups, heap_pops: work.heap_pops };
    assert_eq!(tokenizer.encode(source, budget).unwrap().tokens(), full.tokens());
    let failure = tokenizer.encode(source, TokenizationBudget { pair_lookups: budget.pair_lookups - 1, ..budget }).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work.pair_lookups, budget.pair_lookups - 1);
    let failure = tokenizer.encode(source, TokenizationBudget { heap_pops: budget.heap_pops - 1, ..budget }).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work.heap_pops, budget.heap_pops - 1);
    // A refused pure computation does not mutate or poison shared vocabulary.
    assert_eq!(tokenizer.encode(source, budget).unwrap().tokens(), full.tokens());
}

#[test]
fn exact_input_and_decode_limits_have_positive_and_one_over_controls() {
    let tokenizer = plain(); let source = vec![42; MAX_INPUT_BYTES];
    let encoded = tokenizer.encode(&source, TokenizationBudget::default()).unwrap();
    assert_eq!(encoded.tokens().len(), MAX_INPUT_BYTES);
    assert_eq!(encoded.work().pair_lookups, MAX_INPUT_BYTES - 1);
    assert_eq!(tokenizer.decode(encoded.tokens(), source.len()).unwrap(), source);
    assert_eq!(tokenizer.decode(encoded.tokens(), source.len() - 1), Err(Error::Limit));
    let oversized = vec![42; MAX_INPUT_BYTES + 1];
    assert_eq!(tokenizer.encode(&oversized, TokenizationBudget::default()).unwrap_err(),
        TokenizationFailure { error: Error::Limit, work: TokenizationWork::default() });
    assert_eq!(tokenizer.encode(&[], TokenizationBudget { input_bytes: 0, pair_lookups: 0, heap_pops: 0 }).unwrap().tokens(), &[]);
}

#[test]
fn missing_duplicate_empty_and_unreachable_vocabulary_entries_refuse() {
    let mut missing = bytes(); missing[3] = TokenBytes::Control;
    assert!(matches!(ByteBpe::new(profile(256), missing, Vec::new()), Err(Error::Incomplete)));
    let mut duplicate = bytes(); duplicate[3] = TokenBytes::Content(vec![2]);
    assert!(matches!(ByteBpe::new(profile(256), duplicate, Vec::new()), Err(Error::Duplicate)));
    let mut empty = bytes(); empty[3] = TokenBytes::Content(Vec::new());
    assert!(matches!(ByteBpe::new(profile(256), empty, Vec::new()), Err(Error::InvalidInput)));
    let mut extra = bytes(); extra.push(TokenBytes::Content(b"ab".to_vec()));
    assert!(matches!(ByteBpe::new(profile(257), extra, Vec::new()), Err(Error::Incomplete)));
    assert!(matches!(ByteBpe::new(profile(257), bytes(), Vec::new()), Err(Error::Binding)));
    assert!(ByteBpe::new(profile(256), bytes(), Vec::new()).is_ok());
}

#[test]
fn malformed_duplicate_forward_and_control_merges_cannot_change_meaning() {
    let tokenizer = merged();
    for case in 0..5 {
        let mut vocabulary = tokenizer.0.vocabulary.clone();
        let mut rules = tokenizer.0.merges.clone();
        match case {
            0 => rules[0].result = 257,
            1 => rules.push(rules[0]),
            2 => rules.swap(0, 3),
            3 => rules[0].left = u32::MAX,
            4 => {
                vocabulary.push(TokenBytes::Control);
                assert!(ByteBpe::new(profile(vocabulary.len()), vocabulary.clone(), rules.clone()).is_ok());
                rules[0].left = vocabulary.len() as u32 - 1;
            },
            _ => unreachable!(),
        }
        assert!(ByteBpe::new(profile(vocabulary.len()), vocabulary, rules).is_err());
    }
    assert!(ByteBpe::new(tokenizer.profile().clone(), tokenizer.0.vocabulary.clone(), tokenizer.0.merges.clone()).is_ok());
}

#[test]
fn control_tokens_and_unknown_ids_never_silently_disappear_during_decode() {
    let mut vocabulary = bytes(); vocabulary.push(TokenBytes::Control);
    let tokenizer = ByteBpe::new(profile(257), vocabulary, Vec::new()).unwrap();
    assert_eq!(tokenizer.decode(&[65, 256, 66], 100), Err(Error::Binding));
    assert_eq!(tokenizer.decode(&[65, 257], 100), Err(Error::Missing));
    assert_eq!(tokenizer.decode(&[65, 66], 2).unwrap(), b"AB");
}

#[test]
fn canonical_wire_roundtrip_rejects_all_truncations_extra_bytes_and_header_mutations() {
    let tokenizer = merged(); let encoded = tokenizer.to_bytes().unwrap();
    let restored = ByteBpe::from_bytes(tokenizer.profile(), &encoded).unwrap();
    assert_eq!(restored.to_bytes().unwrap(), encoded);
    assert_eq!(restored.encode(b"aabbaa", TokenizationBudget::default()).unwrap().tokens(),
        tokenizer.encode(b"aabbaa", TokenizationBudget::default()).unwrap().tokens());
    for end in 0..encoded.len() { assert!(ByteBpe::from_bytes(tokenizer.profile(), &encoded[..end]).is_err()); }
    for index in 0..120 {
        let mut changed = encoded.clone(); changed[index] ^= 1;
        assert!(ByteBpe::from_bytes(tokenizer.profile(), &changed).is_err());
    }
    let mut extra = encoded; extra.push(0);
    assert!(ByteBpe::from_bytes(tokenizer.profile(), &extra).is_err());
}

#[test]
fn imported_inventory_tags_and_merge_bytes_reuse_full_constructor_validation() {
    let tokenizer = merged(); let encoded = tokenizer.to_bytes().unwrap();
    let mut changed = encoded.clone(); changed[120] = 255;
    assert!(matches!(ByteBpe::from_bytes(tokenizer.profile(), &changed), Err(Error::InvalidInput)));
    let mut changed = encoded.clone(); changed[121..125].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(ByteBpe::from_bytes(tokenizer.profile(), &changed), Err(Error::Limit)));
    let mut changed = encoded.clone(); let last = changed.len() - 4;
    changed[last..].copy_from_slice(&97_u32.to_be_bytes());
    assert!(ByteBpe::from_bytes(tokenizer.profile(), &changed).is_err());
    assert!(ByteBpe::from_bytes(tokenizer.profile(), &encoded).is_ok());
}

#[test]
fn model_and_tokenizer_generations_and_numerical_profile_are_bound() {
    let tokenizer = plain(); let old = tokenizer.profile(); let encoded = tokenizer.to_bytes().unwrap();
    for field in 0..7 {
        let mut id = old.identity(); let mut shape = old.shape(); let mut epsilon = old.epsilon();
        match field {
            0 => id.tenant += 1, 1 => id.model += 1, 2 => id.model_generation += 1,
            3 => id.tokenizer_generation += 1, 4 => id.profile_generation += 1,
            5 => shape.context += 1, 6 => epsilon *= 2.0, _ => unreachable!(),
        }
        let changed = DecoderProfile::new(id, shape, epsilon, old.theta()).unwrap();
        assert!(!tokenizer.binds(&changed));
        assert!(matches!(ByteBpe::from_bytes(&changed, &encoded), Err(Error::Binding)));
    }
    assert!(tokenizer.binds(old));
}
