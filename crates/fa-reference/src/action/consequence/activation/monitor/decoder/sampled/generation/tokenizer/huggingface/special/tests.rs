mod native;

use super::*;
use super::super::ByteBpe;
use crate::action::consequence::activation::tensor::kv::decoder::{DecoderIdentity, DecoderProfile, DecoderShape};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::TokenizationBudget;

fn profile(count: usize) -> DecoderProfile {
    DecoderProfile::new(DecoderIdentity { tenant: 1, model: 2, model_generation: 3,
        tokenizer_generation: 4, profile_generation: 5 }, DecoderShape { vocabulary: count,
        hidden: 2, intermediate: 2, layers: 1, query_heads: 1, cache_heads: 1, context: 64 },
        1e-5, 10000.0).unwrap()
}
fn byte_id(byte: u8) -> u32 { (u32::from(byte) + 17) % 256 }
fn glyph(byte: u8) -> char {
    char::from_u32(match byte {
        0..=32 => u32::from(byte) + 256,
        33..=126 | 161..=172 | 174..=255 => u32::from(byte),
        127..=160 => u32::from(byte) + 162,
        173 => 323,
    }).unwrap()
}
fn quote(value: &str) -> String {
    let mut output = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            c if c <= '\u{1f}' => output.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => output.push(c),
        }
    }
    output.push('"'); output
}
fn added(id: u32, name: &str) -> String {
    format!(concat!("{{\"id\":{},\"content\":{},\"single_word\":false,",
        "\"lstrip\":false,\"rstrip\":false,\"normalized\":false,\"special\":true}}"), id, quote(name))
}
fn document(added: &[String], reserved: &[(&str, u32)]) -> String {
    let mut vocabulary = (0..=255_u8).rev().map(|byte| {
        format!("{}:{}", quote(&glyph(byte).to_string()), byte_id(byte))
    }).collect::<Vec<_>>();
    for (word, token) in [("al", 256), ("all", 257), ("allo", 258), ("allow", 259),
        ("de", 260), ("den", 261), ("deny", 262)].into_iter().chain(reserved.iter().copied()) {
        vocabulary.push(format!("{}:{token}", quote(word)));
    }
    let merges = [("a", "l"), ("al", "l"), ("all", "o"), ("allo", "w"),
        ("d", "e"), ("de", "n"), ("den", "y")].into_iter()
        .map(|(a, b)| format!("[{},{}]", quote(a), quote(b))).collect::<Vec<_>>().join(",");
    format!(concat!(
        "{{\"version\":\"1.0\",\"truncation\":null,\"padding\":null,\"added_tokens\":[{}],",
        "\"normalizer\":null,\"pre_tokenizer\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"post_processor\":null,\"decoder\":{{\"type\":\"ByteLevel\",",
        "\"add_prefix_space\":false,\"trim_offsets\":false,\"use_regex\":false}},",
        "\"model\":{{\"type\":\"BPE\",\"dropout\":null,\"unk_token\":null,",
        "\"continuing_subword_prefix\":null,\"end_of_word_suffix\":null,",
        "\"fuse_unk\":false,\"byte_fallback\":false,\"ignore_merges\":false,",
        "\"vocab\":{{{}}},\"merges\":[{}]}}}}"), added.join(","), vocabulary.join(","), merges)
}
fn import(source: &str, count: usize) -> Result<ByteBpe, Error> {
    ByteBpe::from_huggingface_json(&profile(count), source.as_bytes(), source.len())
}

#[test]
fn hf_special_appended_and_reserved_eos_preserve_original_ids_and_archives() {
    let appended = document(&[added(263, "<eos>")], &[]);
    let reserved = document(&[added(263, "<eos>")], &[("<eos>", 263)]);
    let a = import(&appended, 264).unwrap(); let b = import(&reserved, 264).unwrap();
    assert_eq!(a.to_bytes().unwrap(), b.to_bytes().unwrap());
    assert_eq!(a.control_tokens(), &[263]);
    let input = a.encode(b"allow<eos>deny", TokenizationBudget::default()).unwrap();
    assert_eq!(input.tokens(), &[259, 263, 262]);
    assert_eq!(input.spans(), &[0..5, 5..10, 10..14]);
    assert_eq!(input.source(), b"allow<eos>deny");
    assert_eq!(a.decode(input.tokens(), 100), Err(Error::Binding));
    let wire = a.to_bytes().unwrap(); assert_eq!(&wire[..8], b"FABBPE02");
    let recovered = ByteBpe::from_bytes(&profile(264), &wire).unwrap();
    assert_eq!(recovered.encode(input.source(), TokenizationBudget::default()).unwrap().tokens(), input.tokens());
    let old = import(&document(&[], &[]), 263).unwrap().to_bytes().unwrap();
    assert_eq!(&old[..8], b"FABBPE01");
}

#[test]
fn hf_special_literal_unicode_and_longest_matching_are_not_bytelevel_decoded() {
    let source = document(&[added(263, "<s>"), added(264, "<s>long"), added(265, "<é>")], &[]);
    let t = import(&source, 266).unwrap();
    let bytes = "allow<s>long<é>deny<s>".as_bytes();
    let input = t.encode(bytes, TokenizationBudget::default()).unwrap();
    assert_eq!(input.tokens(), &[259, 264, 265, 262, 263]);
    assert_eq!(input.spans(), &[0..5, 5..12, 12..16, 16..20, 20..23]);
    assert_eq!(t.special_tokens()[&265], "<é>".as_bytes());
    assert_eq!(t.encode(b"a<s>l", TokenizationBudget::default()).unwrap().tokens(),
        &[byte_id(b'a'), 263, byte_id(b'l')]);
}

#[test]
fn hf_special_unsupported_flags_missing_fields_and_unknown_keys_refuse() {
    let valid = document(&[added(263, "<eos>")], &[]);
    for flag in ["single_word", "lstrip", "rstrip", "normalized"] {
        let changed = valid.replacen(&format!("\"{flag}\":false"), &format!("\"{flag}\":true"), 1);
        assert_ne!(changed, valid);
        assert_eq!(import(&changed, 264).unwrap_err(), Error::Binding);
    }
    assert_eq!(import(&valid.replace("\"special\":true", "\"special\":false"), 264).unwrap_err(), Error::Binding);
    assert_eq!(import(&valid.replace(",\"normalized\":false", ""), 264).unwrap_err(), Error::Incomplete);
    assert_eq!(import(&valid.replace("\"lstrip\":false", "\"lstrip\":null"), 264).unwrap_err(), Error::InvalidInput);
    assert_eq!(import(&valid.replace("\"special\":true", "\"special\":true,\"unknown\":0"), 264).unwrap_err(), Error::Binding);
    assert!(import(&valid, 264).is_ok());
}

#[test]
fn hf_special_id_coverage_and_order_never_renumber_or_overwrite_content() {
    for (rows, reserved, count, error) in [
        (vec![added(264, "<eos>")], vec![], 264, Error::Binding),
        (vec![added(264, "<eos>")], vec![], 265, Error::Incomplete),
        (vec![added(263, "<eos>"), added(263, "<other>")], vec![], 264, Error::Duplicate),
        (vec![added(263, "<eos>"), added(264, "<eos>")], vec![], 265, Error::Duplicate),
        (vec![added(264, "<other>"), added(263, "<eos>")], vec![], 265, Error::Binding),
        (vec![added(263, "<other>")], vec![("<eos>", 263)], 264, Error::Binding),
        (vec![added(264, "<eos>")], vec![("<eos>", 263)], 265, Error::Binding),
        (vec![added(byte_id(b'a'), "<eos>")], vec![], 263, Error::Binding),
    ] {
        assert_eq!(import(&document(&rows, &reserved), count).unwrap_err(), error);
    }
}

#[test]
fn hf_specials_cannot_be_bpe_merge_operands_results_or_replace_singletons() {
    for (id, spelling) in [(256, "al"), (259, "allow"), (byte_id(b'a'), "a")] {
        let result = import(&document(&[added(id, spelling), added(263, "<eos>")], &[]), 264);
        assert!(result.is_err());
    }
    // An explicit reserved control with NO merge participation is admissible.
    assert!(import(&document(&[added(263, "<eos>")], &[("<eos>", 263)]), 264).is_ok());
}

#[test]
fn hf_special_name_count_length_and_aggregate_limits_precede_retention() {
    assert_eq!(import(&document(&[added(263, "")], &[]), 264).unwrap_err(), Error::InvalidInput);
    assert_eq!(import(&document(&[added(263, &"x".repeat(MAX_TOKEN_BYTES + 1))], &[]), 264).unwrap_err(), Error::Limit);
    let names: Vec<_> = (0..17).map(|id| {
        let prefix = format!("<{id}>");
        added(263 + id, &(prefix.clone() + &"x".repeat(MAX_TOKEN_BYTES - prefix.len())))
    }).collect();
    assert!(import(&document(&names[..16], &[]), 279).is_ok());
    assert_eq!(import(&document(&names, &[]), 280).unwrap_err(), Error::Limit);
    let too_many: Vec<_> = (0..257).map(|id| added(263 + id, &format!("<{id}>"))).collect();
    assert_eq!(import(&document(&too_many, &[]), 520).unwrap_err(), Error::Limit);
}

#[test]
fn hf_special_only_prompt_needs_no_bpe_merge_budget_but_never_truncates() {
    let t = import(&document(&[added(263, "<eos>")], &[]), 264).unwrap();
    let budget = TokenizationBudget { input_bytes: 10, pair_lookups: 0, heap_pops: 0 };
    let input = t.encode(b"<eos>", budget).unwrap();
    assert_eq!(input.tokens(), &[263]); assert_eq!(input.spans(), &[0..5]);
    let refusal = t.encode(b"allow<eos>", budget).unwrap_err();
    assert_eq!(refusal.error, Error::Limit); assert_eq!(refusal.work.input_bytes, 10);
    assert!(t.encode(b"<eos><eos>x", budget).is_err());
}
