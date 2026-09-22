use super::*;
use super::super::TokenizationBudget;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderShape};

fn profile(vocabulary: usize) -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary,
        hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 32 },
        1e-5, 10000.0).unwrap()
}
fn vocabulary(controls: usize) -> Vec<TokenBytes> {
    let mut words: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    words.extend((0..controls).map(|_| TokenBytes::Control));
    words
}
fn rules() -> Vec<Merge> {
    vec![Merge { left: 97, right: 98, result: 264 }, Merge { left: 98, right: 98, result: 265 }]
}
fn make(names: &[(u32, &[u8])]) -> ByteBpe {
    let mut words = vocabulary(8);
    words.extend([TokenBytes::Content(b"ab".to_vec()), TokenBytes::Content(b"bb".to_vec())]);
    ByteBpe::new_with_special_tokens(profile(266), words, rules(),
        names.iter().map(|(id, name)| (*id, name.to_vec())).collect()).unwrap()
}
fn ids(tokenizer: &ByteBpe, source: &[u8]) -> Vec<u32> {
    tokenizer.encode(source, TokenizationBudget::default()).unwrap().tokens().to_vec()
}

#[test]
fn named_controls_keep_exact_spans_and_segment_original_merges() {
    let t = make(&[(256, b"<s>"), (257, b"<s>long")]);
    let input = t.encode(b"ab<s>longbb<s>", TokenizationBudget::default()).unwrap();
    assert_eq!(input.tokens(), &[264, 257, 265, 256]);
    assert_eq!(input.spans(), &[0..2, 2..9, 9..11, 11..14]);
    assert_eq!(input.source(), b"ab<s>longbb<s>");
    assert_eq!(input.work().merges, 2);
    assert_eq!(t.decode(input.tokens(), 100), Err(Error::Binding));
    assert_eq!(ids(&make(&[]), b"<s>"), vec![60, 115, 62]);
    assert_eq!(ids(&t, b"a<s>b"), vec![97, 256, 98]);
}

#[test]
fn leftmost_longest_is_not_earliest_end_id_order_or_longest_anywhere() {
    let t = make(&[(256, b"a"), (257, b"aba"), (258, b"baba")]);
    assert_eq!(ids(&t, b"ababa"), vec![257, 98, 256]);
    assert_eq!(ids(&t, b"baba"), vec![258]);
    assert_eq!(ids(&t, b"aaba"), vec![256, 257]);
    assert_eq!(ids(&t, b"abababa"), vec![257, 258]);
}

#[test]
fn unicode_names_are_literal_and_binary_input_is_not_rewritten() {
    let t = make(&[(256, "é".as_bytes()), (257, "éé".as_bytes()), (258, b"\0")]);
    let input = t.encode(&[0xff, 0xc3, 0xa9, 0xc3, 0xa9, 0, 0xc3], TokenizationBudget::default()).unwrap();
    assert_eq!(input.tokens(), &[255, 257, 258, 195]);
    assert_eq!(input.spans(), &[0..1, 1..5, 5..6, 6..7]);
    // Decomposed text is not normalized to the registered composed spelling.
    assert_eq!(ids(&t, "e\u{301}".as_bytes()), vec![101, 204, 129]);
}

// Independent whole-input scanner followed by a deliberately slow greedy BPE.
// It shares no trie, failure links, candidate heap or native encoding function.
fn oracle(t: &ByteBpe, source: &[u8]) -> (Vec<u32>, Vec<std::ops::Range<usize>>) {
    let mut pieces = Vec::new();
    let mut at = 0;
    while at < source.len() {
        let matched = t.special_tokens().iter()
            .filter(|(_, name)| source[at..].starts_with(name))
            .max_by_key(|(_, name)| name.len());
        match matched {
            Some((id, name)) => { pieces.push((*id, at..at + name.len())); at += name.len(); }
            None => { pieces.push((u32::from(source[at]), at..at + 1)); at += 1; }
        }
    }
    loop {
        let mut best = None;
        for (rank, rule) in rules().iter().enumerate() {
            for at in 0..pieces.len().saturating_sub(1) {
                if pieces[at].0 == rule.left && pieces[at + 1].0 == rule.right {
                    let candidate = (rank, at, rule.result);
                    if best.is_none_or(|old| candidate < old) { best = Some(candidate); }
                }
            }
        }
        let Some((_, at, result)) = best else { break; };
        pieces[at].0 = result;
        pieces[at].1.end = pieces[at + 1].1.end;
        pieces.remove(at + 1);
    }
    pieces.into_iter().unzip()
}

#[test]
fn reverse_failure_links_agree_with_independent_scan_on_9331_inputs() {
    let t = make(&[(256, b"aba"), (257, b"baba"), (258, b"bc"), (259, b"<x>"),
        (260, b"<"), (261, b"caba"), (262, b"bbbc"), (263, b"abc")]);
    let alphabet = b"abc<x>";
    let mut cases = 0;
    for len in 0..=5_u32 {
        for mut number in 0..6_usize.pow(len) {
            let mut source = vec![0; len as usize];
            for byte in &mut source { *byte = alphabet[number % 6]; number /= 6; }
            let (expected, spans) = oracle(&t, &source);
            let actual = t.encode(&source, TokenizationBudget::default()).unwrap();
            assert_eq!(actual.tokens(), expected, "source={source:?}");
            assert_eq!(actual.spans(), spans, "source={source:?}");
            assert_eq!(actual.source(), source);
            cases += 1;
        }
    }
    assert_eq!(cases, 9331);
}

#[test]
fn names_refuse_duplicates_wrong_ids_invalid_utf8_and_empty_values() {
    for (names, error) in [
        (BTreeMap::from([(256, b"x".to_vec()), (257, b"x".to_vec())]), Error::Duplicate),
        (BTreeMap::from([(97, b"x".to_vec())]), Error::Binding),
        (BTreeMap::from([(999, b"x".to_vec())]), Error::Binding),
        (BTreeMap::from([(256, Vec::new())]), Error::InvalidInput),
        (BTreeMap::from([(256, vec![255])]), Error::InvalidInput),
    ] {
        assert_eq!(ByteBpe::new_with_special_tokens(profile(258), vocabulary(2), Vec::new(), names)
            .unwrap_err(), error);
    }
}

#[test]
fn exact_spelling_byte_and_control_count_limits_are_enforced() {
    let make_names = |count: usize, length: usize| -> BTreeMap<_, _> {
        (0..count).map(|id| {
            let mut name = vec![b'x'; length];
            name[..2].copy_from_slice(&(id as u16).to_be_bytes());
            (256 + id as u32, name)
        }).collect()
    };
    // IDs 0..16 are ASCII bytes, so these are distinct valid UTF-8 names.
    let names = make_names(16, MAX_TOKEN_BYTES);
    assert!(ByteBpe::new_with_special_tokens(profile(273), vocabulary(17), Vec::new(), names.clone()).is_ok());
    let mut over = names; over.insert(272, b"y".to_vec());
    assert_eq!(ByteBpe::new_with_special_tokens(profile(273), vocabulary(17), Vec::new(), over).unwrap_err(), Error::Limit);
    assert_eq!(ByteBpe::new_with_special_tokens(profile(257), vocabulary(1), Vec::new(),
        BTreeMap::from([(256, vec![b'x'; MAX_TOKEN_BYTES + 1])])).unwrap_err(), Error::Limit);
    let names: BTreeMap<_, _> = (0..256).map(|id| (256 + id, format!("<{id}>").into_bytes())).collect();
    assert!(ByteBpe::new_with_special_tokens(profile(512), vocabulary(256), Vec::new(), names.clone()).is_ok());
    let mut over = names; over.insert(512, b"extra".to_vec());
    assert_eq!(ByteBpe::new_with_special_tokens(profile(513), vocabulary(257), Vec::new(), over).unwrap_err(), Error::Limit);
}

#[test]
fn shared_prefix_stress_and_budget_refusal_never_return_partial_prompt() {
    let mut name = vec![b'a'; MAX_TOKEN_BYTES]; name[0] = b'b';
    let t = ByteBpe::new_with_special_tokens(profile(257), vocabulary(1), Vec::new(),
        BTreeMap::from([(256, name)])).unwrap();
    let source = vec![b'a'; MAX_INPUT_BYTES];
    assert_eq!(ids(&t, &source).len(), MAX_INPUT_BYTES);
    let failure = t.encode(&source, TokenizationBudget { input_bytes: MAX_INPUT_BYTES - 1,
        ..TokenizationBudget::default() }).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work.input_bytes, 0);
    let t = make(&[(256, b"<s>")]);
    let failure = t.encode(b"ab<s>ab", TokenizationBudget { pair_lookups: 0,
        ..TokenizationBudget::default() }).unwrap_err();
    assert_eq!(failure.error, Error::Limit);
    assert_eq!(failure.work.input_bytes, 7);
}

fn manual_archive(named: bool) -> Vec<u8> {
    let mut bytes = if named { b"FABBPE02".to_vec() } else { b"FABBPE01".to_vec() };
    for field in [1, 2, 3, 4, 5, 266, 2, 2, 1, 1, 1, 32, 1e-5_f64.to_bits(), 10000.0_f64.to_bits()] {
        bytes.extend_from_slice(&field.to_be_bytes());
    }
    for byte in 0..=255 { bytes.extend_from_slice(&[1, 0, 0, 0, 1, byte]); }
    bytes.extend_from_slice(&[0; 8]);
    for value in [b"ab", b"bb"] { bytes.extend_from_slice(&[1, 0, 0, 0, 2]); bytes.extend_from_slice(value); }
    bytes.extend_from_slice(&2_u32.to_be_bytes());
    for value in [97_u32, 98, 264, 98, 98, 265] { bytes.extend_from_slice(&value.to_be_bytes()); }
    if named {
        for value in [1_u32, 256, 3] { bytes.extend_from_slice(&value.to_be_bytes()); }
        bytes.extend_from_slice(b"<s>");
    }
    bytes
}

#[test]
fn manual_versioned_archives_preserve_legacy_bytes_and_named_semantics() {
    for named in [false, true] {
        let t = if named { make(&[(256, b"<s>")]) } else { make(&[]) };
        let golden = manual_archive(named);
        assert_eq!(t.to_bytes().unwrap(), golden);
        let imported = ByteBpe::from_bytes(&profile(266), &golden).unwrap();
        assert_eq!(imported.to_bytes().unwrap(), golden);
        assert_eq!(ids(&imported, b"ab<s>bb"), ids(&t, b"ab<s>bb"));
    }
}

#[test]
fn named_archives_reject_all_truncations_trailing_data_and_downgrades() {
    let golden = manual_archive(true);
    for end in 0..golden.len() { assert!(ByteBpe::from_bytes(&profile(266), &golden[..end]).is_err()); }
    let mut trailing = golden.clone(); trailing.push(0);
    assert!(ByteBpe::from_bytes(&profile(266), &trailing).is_err());
    let mut downgraded = golden.clone(); downgraded[..8].copy_from_slice(b"FABBPE01");
    assert!(ByteBpe::from_bytes(&profile(266), &downgraded).is_err());
    let mut empty = manual_archive(false); empty[..8].copy_from_slice(b"FABBPE02"); empty.extend_from_slice(&0_u32.to_be_bytes());
    assert!(ByteBpe::from_bytes(&profile(266), &empty).is_err());
    let table = golden.len() - 15;
    for (offset, value) in [(table, 257_u32), (table + 4, 97), (table + 8, (MAX_TOKEN_BYTES + 1) as u32)] {
        let mut bad = golden.clone(); bad[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        assert!(ByteBpe::from_bytes(&profile(266), &bad).is_err());
    }
    let mut wrong = golden; wrong[8..16].copy_from_slice(&99_u64.to_be_bytes());
    assert_eq!(ByteBpe::from_bytes(&profile(266), &wrong).unwrap_err(), Error::Binding);
}

#[test]
fn wire_name_order_and_duplicate_spellings_are_not_silently_canonicalized() {
    let t = make(&[(256, b"<s>"), (257, b"end")]);
    let bytes = t.to_bytes().unwrap();
    let table = bytes.len() - 26;
    let mut unordered = bytes.clone();
    unordered[table + 4..table + 8].copy_from_slice(&257_u32.to_be_bytes());
    unordered[table + 15..table + 19].copy_from_slice(&256_u32.to_be_bytes());
    assert!(ByteBpe::from_bytes(&profile(266), &unordered).is_err());
    let mut duplicate = bytes; let len = duplicate.len(); duplicate[len - 3..].copy_from_slice(b"<s>");
    assert_eq!(ByteBpe::from_bytes(&profile(266), &duplicate).unwrap_err(), Error::Duplicate);
}
