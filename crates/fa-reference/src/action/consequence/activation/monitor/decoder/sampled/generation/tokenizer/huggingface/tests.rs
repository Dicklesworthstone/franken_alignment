use super::*;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderShape};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::TokenizationBudget;

fn profile() -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: 262,
        hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 32 },
        1e-5, 10000.0).unwrap()
}

// Independent piecewise spelling oracle, not the production inverse-table builder.
fn glyph(byte: u8) -> char {
    let code = match byte {
        0..=32 => u32::from(byte) + 256,
        33..=126 | 161..=172 | 174..=255 => u32::from(byte),
        127..=160 => u32::from(byte) + 162,
        173 => 323,
    };
    char::from_u32(code).unwrap()
}
fn id(byte: u8) -> u32 { (u32::from(byte) + 17) % 256 }
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
fn document(legacy: bool) -> String {
    // Permuted IDs and reverse input order prevent a byte/id/map-order shortcut.
    let mut vocabulary = (0..=255_u8).rev().map(|byte| {
        format!("{}:{}", quote(&glyph(byte).to_string()), id(byte))
    }).collect::<Vec<_>>();
    for (word, token) in [("bc", 256), ("ab", 257), ("abc", 258),
        ("Ġa", 259), ("Ġabc", 260), ("Ã©", 261)]
    {
        vocabulary.push(format!("{}:{token}", quote(word)));
    }
    let merges = [("b", "c"), ("a", "b"), ("a", "bc"),
        ("Ġ", "a"), ("Ġ", "abc"), ("Ã", "©")].into_iter().map(|(a, b)| {
        if legacy { quote(&format!("{a} {b}")) }
        else { format!("[{},{}]", quote(a), quote(b)) }
    }).collect::<Vec<_>>().join(",");
    format!(concat!(
        "{{\"version\":\"1.0\",\"truncation\":null,\"padding\":null,\"added_tokens\":[],",
        "\"normalizer\":null,\"pre_tokenizer\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"post_processor\":null,\"decoder\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"model\":{{\"type\":\"BPE\",\"dropout\":null,\"unk_token\":null,",
        "\"continuing_subword_prefix\":null,\"end_of_word_suffix\":null,",
        "\"fuse_unk\":false,\"byte_fallback\":false,\"ignore_merges\":false,",
        "\"vocab\":{{{}}},\"merges\":[{}]}}}}"), vocabulary.join(","), merges)
}
fn import(document: &str) -> Result<ByteBpe, Error> {
    ByteBpe::from_huggingface_json(&profile(), document.as_bytes(), document.len())
}
fn encode(tokenizer: &ByteBpe, bytes: &[u8]) -> Vec<u32> {
    tokenizer.encode(bytes, TokenizationBudget::default()).unwrap().tokens().to_vec()
}

#[test]
fn hf_pair_merges_keep_external_ids_rank_and_exact_byte_spans() {
    let tokenizer = import(&document(false)).unwrap();
    let encoded = tokenizer.encode(" abc é".as_bytes(), TokenizationBudget::default()).unwrap();
    assert_eq!(encoded.source(), " abc é".as_bytes());
    assert_eq!(encoded.tokens(), &[260, 49, 261]);
    assert_eq!(encoded.spans(), &[0..4, 4..5, 5..7]);
    assert_eq!(tokenizer.decode(encoded.tokens(), 7).unwrap(), " abc é".as_bytes());
    assert_eq!(encode(&tokenizer, b"abcabc"), vec![258, 258]);
    assert_eq!(encode(&tokenizer, b"aab"), vec![114, 257]);
    assert_eq!(tokenizer.profile(), &profile());
    assert!(tokenizer.control_tokens().is_empty());
}

#[test]
fn hf_legacy_merges_and_native_archive_preserve_the_same_inventory() {
    let pairs = import(&document(false)).unwrap();
    let legacy = import(&document(true)).unwrap();
    assert_eq!(pairs.to_bytes().unwrap(), legacy.to_bytes().unwrap());
    let bytes = pairs.to_bytes().unwrap();
    let restored = ByteBpe::from_bytes(&profile(), &bytes).unwrap();
    assert_eq!(encode(&restored, b" abc"), vec![260]);
    let p = profile();
    let identity = DecoderIdentity { tokenizer_generation: 9, ..p.identity() };
    let other = DecoderProfile::new(identity, p.shape(), p.epsilon(), p.theta()).unwrap();
    assert_eq!(ByteBpe::from_bytes(&other, &bytes).unwrap_err(), Error::Binding);
}

#[test]
fn hf_alphabet_covers_every_byte_without_unicode_reinterpretation() {
    let tokenizer = import(&document(false)).unwrap();
    for byte in 0..=255_u8 {
        assert_eq!(encode(&tokenizer, &[byte]), vec![id(byte)]);
        assert_eq!(tokenizer.content_bytes(id(byte)).unwrap(), &[byte]);
    }
    for (byte, spelling) in [(0, 'Ā'), (9, 'ĉ'), (10, 'Ċ'), (32, 'Ġ'),
        (127, 'ġ'), (160, 'ł'), (173, 'Ń')]
    {
        assert_eq!(glyph(byte), spelling);
    }
    // UTF-8 input is encoded as its original bytes. No single-byte Latin-1
    // reinterpretation, character normalization or replacement string is used.
    assert_eq!(encode(&tokenizer, "é".as_bytes()), vec![261]);
    assert_ne!(encode(&tokenizer, "e\u{301}".as_bytes()), vec![261]);
    let bytes = (0..=255_u8).collect::<Vec<_>>();
    let tokens = encode(&tokenizer, &bytes);
    assert_eq!(tokenizer.decode(&tokens, bytes.len()).unwrap(), bytes);
}

#[test]
fn hf_unsupported_pipeline_semantics_never_fall_back_to_raw_bytes() {
    let valid = document(false);
    for (from, to) in [
        ("\"use_regex\":false", "\"use_regex\":true"),
        ("\"add_prefix_space\":false", "\"add_prefix_space\":true"),
        ("\"trim_offsets\":false", "\"trim_offsets\":true"),
        ("\"normalizer\":null", "\"normalizer\":{\"type\":\"Lowercase\"}"),
        ("\"post_processor\":null", "\"post_processor\":{\"type\":\"TemplateProcessing\"}"),
        ("\"truncation\":null", "\"truncation\":{}"),
        ("\"padding\":null", "\"padding\":{}"),
        ("\"added_tokens\":[]", "\"added_tokens\":[{\"id\":262,\"content\":\"<eos>\",\"special\":true}]"),
        ("\"dropout\":null", "\"dropout\":0.1"),
        ("\"unk_token\":null", "\"unk_token\":\"?\""),
        ("\"continuing_subword_prefix\":null", "\"continuing_subword_prefix\":\"##\""),
        ("\"end_of_word_suffix\":null", "\"end_of_word_suffix\":\"</w>\""),
        ("\"byte_fallback\":false", "\"byte_fallback\":true"),
        ("\"fuse_unk\":false", "\"fuse_unk\":true"),
        ("\"ignore_merges\":false", "\"ignore_merges\":true"),
        ("\"type\":\"BPE\"", "\"type\":\"WordPiece\""),
        ("\"version\":\"1.0\"", "\"version\":\"2.0\""),
    ] {
        assert!(valid.contains(from), "fixture must mutate {from}");
        let changed = valid.replacen(from, to, 1);
        assert_eq!(encode(&import(&valid).unwrap(), b"abc"), vec![258]);
        assert_eq!(import(&changed).unwrap_err(), Error::Binding, "{from}");
    }
    let empty_affixes = valid.replace("\"continuing_subword_prefix\":null", "\"continuing_subword_prefix\":\"\"")
        .replace("\"end_of_word_suffix\":null", "\"end_of_word_suffix\":\"\"");
    assert_eq!(encode(&import(&empty_affixes).unwrap(), b"abc"), vec![258]);
}

#[test]
fn hf_vocabulary_ids_and_original_graph_are_validated() {
    let valid = document(false);
    assert_eq!(encode(&import(&valid).unwrap(), b"abc"), vec![258]);
    for (from, to, error) in [
        ("\"a\":114", "\"a\":115", Error::Duplicate),
        ("\"a\":114", "\"a\":262", Error::Binding),
        ("\"a\":114", "\"a\":4294967296", Error::Limit),
        ("\"a\":114", "\"a\":114.0", Error::InvalidInput),
        ("\"a\":114", "\"a\":-1", Error::InvalidInput),
        ("\"a\":114", "\"🙂\":114", Error::Binding),
        ("\"a\":114", "\"\":114", Error::InvalidInput),
        ("\"Ā\":17", "\"xy\":17", Error::Incomplete),
    ] {
        assert!(valid.contains(from));
        assert_eq!(import(&valid.replacen(from, to, 1)).unwrap_err(), error);
    }
    let unreachable = valid.replace(",\"merges\":[[\"b\",\"c\"],[\"a\",\"b\"],[\"a\",\"bc\"],[\"Ġ\",\"a\"],[\"Ġ\",\"abc\"],[\"Ã\",\"©\"]]",
        ",\"merges\":[]");
    assert_ne!(unreachable, valid);
    assert_eq!(import(&unreachable).unwrap_err(), Error::Incomplete);
}

#[test]
fn hf_merge_shapes_missing_spellings_and_forward_references_refuse() {
    let valid = document(false);
    for (from, to, error) in [
        ("[\"b\",\"c\"]", "[\"b\",\"missing\"]", Error::Missing),
        ("[\"b\",\"c\"]", "[\"b\",\"a\"]", Error::Missing),
        ("[\"b\",\"c\"]", "[\"b\",\"c\",\"bc\"]", Error::InvalidInput),
        ("[\"b\",\"c\"]", "[\"\",\"c\"]", Error::InvalidInput),
        ("[\"a\",\"b\"]", "\"a b\"", Error::InvalidInput),
        ("[\"a\",\"b\"]", "[\"b\",\"c\"]", Error::Duplicate),
        ("[\"b\",\"c\"],[\"a\",\"b\"],[\"a\",\"bc\"]",
            "[\"a\",\"bc\"],[\"b\",\"c\"],[\"a\",\"b\"]", Error::Binding),
    ] {
        assert!(valid.contains(from));
        assert_eq!(encode(&import(&valid).unwrap(), b"abc"), vec![258]);
        assert_eq!(import(&valid.replacen(from, to, 1)).unwrap_err(), error);
    }
    let legacy = document(true);
    for bad in ["b", "b  c", " b", "b "] {
        assert_eq!(import(&legacy.replacen("\"b c\"", &quote(bad), 1)).unwrap_err(), Error::InvalidInput);
    }
}

#[test]
fn hf_duplicate_unknown_and_malformed_json_are_not_normalized_into_success() {
    let valid = document(false);
    assert_eq!(import(&valid.replacen("\"version\":", "\"version\":\"1.0\",\"version\":", 1)).unwrap_err(), Error::Duplicate);
    assert_eq!(import(&valid.replacen("\"vocab\":", "\"surprise\":0,\"vocab\":", 1)).unwrap_err(), Error::Binding);
    assert_eq!(import(&valid.replacen("\"model\":", "\"surprise\":0,\"model\":", 1)).unwrap_err(), Error::Binding);
    assert_eq!(import(&valid.replacen("\"use_regex\":false", "\"use_regex\":null", 1)).unwrap_err(), Error::InvalidInput);
    assert_eq!(import(&valid.replacen(",\"ignore_merges\":false", "", 1)).unwrap_err(), Error::Incomplete);
    assert_eq!(import(&format!("{valid}{{}}")).unwrap_err(), Error::InvalidInput);
    assert!(import(&valid[..valid.len() - 1]).is_err());
    assert!(ByteBpe::from_huggingface_json(&profile(), b"\xff", 1).is_err());
    let whitespace = format!(" \n{valid}\n\t");
    assert_eq!(encode(&import(&whitespace).unwrap(), b"abc"), vec![258]);
}

#[test]
fn hf_exact_byte_budget_and_structural_limits_refuse_before_admission() {
    let valid = document(false);
    assert!(ByteBpe::from_huggingface_json(&profile(), valid.as_bytes(), valid.len()).is_ok());
    assert_eq!(ByteBpe::from_huggingface_json(&profile(), valid.as_bytes(), valid.len() - 1).unwrap_err(), Error::Limit);
    assert_eq!(ByteBpe::from_huggingface_json(&profile(), b"", 0).unwrap_err(), Error::Limit);
    assert_eq!(ByteBpe::from_huggingface_json(&profile(), b"{}", MAX_JSON_BYTES + 1).unwrap_err(), Error::Limit);
    let deep = format!("{}0{}", "[".repeat(20), "]".repeat(20));
    assert_eq!(import(&deep).unwrap_err(), Error::Limit);
    let oversized = valid.replacen("\"a\":114", &format!("{}:114", quote(&"a".repeat(MAX_TOKEN_BYTES + 1))), 1);
    assert_eq!(import(&oversized).unwrap_err(), Error::Limit);
}

#[test]
fn hf_rank_changes_are_not_silently_sorted_by_id_or_spelling() {
    let valid = document(false);
    let different = valid.replace("[\"b\",\"c\"],[\"a\",\"b\"]", "[\"a\",\"b\"],[\"b\",\"c\"]");
    assert_eq!(encode(&import(&valid).unwrap(), b"abc"), vec![258]);
    assert_eq!(encode(&import(&different).unwrap(), b"abc"), vec![257, 116]);
    assert_ne!(import(&valid).unwrap().to_bytes().unwrap(), import(&different).unwrap().to_bytes().unwrap());
}

#[test]
fn hf_all_short_inputs_match_an_independent_greedy_pair_oracle() {
    let tokenizer = import(&document(false)).unwrap();
    let rules = [(115, 116, 256), (114, 115, 257), (114, 256, 258),
        (49, 114, 259), (49, 258, 260), (212, 186, 261)];
    let alphabet = [b'a', b'b', b'c', b' ', 0xc3, 0xa9];
    for length in 0..=4_u32 {
        for mut code in 0..6_usize.pow(length) {
            let mut input = Vec::new();
            for _ in 0..length { input.push(alphabet[code % 6]); code /= 6; }
            let mut expected = input.iter().copied().map(id).collect::<Vec<_>>();
            loop {
                let next = rules.iter().find_map(|&(left, right, result)| {
                    expected.windows(2).position(|pair| pair == [left, right])
                        .map(|offset| (offset, result))
                });
                let Some((offset, result)) = next else { break; };
                expected.splice(offset..offset + 2, [result]);
            }
            let actual = tokenizer.encode(&input, TokenizationBudget::default()).unwrap();
            assert_eq!(actual.tokens(), expected);
            assert_eq!(tokenizer.decode(actual.tokens(), input.len()).unwrap(), input);
        }
    }
}
