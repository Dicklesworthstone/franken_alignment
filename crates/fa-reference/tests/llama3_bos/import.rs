use super::fixture::{BOS, TEMPLATE, document, import, profile, tokenizer, wrapped};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, PreTokenization, TokenizationBudget,
};
use fa_reference::Error;

#[test]
fn bos_upstream_sequence_and_bare_template_have_one_effective_archive_identity() {
    let expected = tokenizer().to_bytes().unwrap();
    assert_eq!(import(&document(TEMPLATE)).unwrap().to_bytes().unwrap(), expected);
    // The official converter emits true,false,true. Only trim_offsets is read
    // during ByteLevel postprocessing; its other flags remain typed inert data.
    for prefix in [true, false] {
        for regex in [true, false] {
            let source = document(&wrapped(TEMPLATE, prefix, regex));
            let imported = import(&source).unwrap();
            assert_eq!(imported.to_bytes().unwrap(), expected);
            assert_eq!(imported.encode(b"aa", TokenizationBudget::default()).unwrap().tokens(), &[BOS, 256]);
        }
    }
}

#[test]
fn bos_profile_is_explicit_and_never_widens_existing_null_postprocessor_apis() {
    let source = document(TEMPLATE);
    let plain = document("null");
    let original = ByteBpe::from_huggingface_json_with_pre_tokenization(&profile(),
        plain.as_bytes(), plain.len(), PreTokenization::Llama3Unicode17).unwrap();
    assert_eq!(original.encode(b"aa", TokenizationBudget::default()).unwrap().tokens(), &[256]);
    assert_eq!(ByteBpe::from_huggingface_json_with_pre_tokenization(&profile(),
        source.as_bytes(), source.len(), PreTokenization::Llama3Unicode17).unwrap_err(), Error::Binding);
    assert_eq!(ByteBpe::from_huggingface_json(&profile(),
        source.as_bytes(), source.len()).unwrap_err(), Error::Binding);
    assert_eq!(import(&plain).unwrap_err(), Error::Binding);
    for (from, to) in [
        (r#""normalizer":null"#, r#""normalizer":{"type":"NFC"}"#),
        (r#""add_prefix_space":false"#, r#""add_prefix_space":true"#),
        (r#""behavior":"Isolated""#, r#""behavior":"Removed""#),
        (r#""invert":false"#, r#""invert":true"#),
        (r#""padding":null"#, r#""padding":{}"#),
        (r#""truncation":null"#, r#""truncation":{}"#),
    ] {
        assert!(source.contains(from));
        assert_eq!(import(&source.replacen(from, to, 1)).unwrap_err(), Error::Binding);
    }
    assert_eq!(import(&source).unwrap().bos_token(), Some(BOS));
}

#[test]
fn bos_template_requires_exact_sequence_types_original_id_and_name() {
    assert_eq!(import(&document(TEMPLATE)).unwrap().bos_token(), Some(BOS));
    for (from, to, error) in [
        (r#""ids":[259]"#, r#""ids":[114]"#, Error::Binding),
        (r#""ids":[259]"#, r#""ids":[260]"#, Error::Binding),
        (r#""ids":[259]"#, r#""ids":[261]"#, Error::Binding),
        (r#""ids":[259]"#, r#""ids":[]"#, Error::Binding),
        (r#""ids":[259]"#, r#""ids":[259,260]"#, Error::Binding),
        (r#""ids":[259]"#, r#""ids":[-1]"#, Error::InvalidInput),
        (r#""ids":[259]"#, r#""ids":[4294967296]"#, Error::Limit),
        (r#""tokens":["<|begin_of_text|>"]"#, r#""tokens":["<end>"]"#, Error::Binding),
        (r#""id":"A","type_id":0"#, r#""id":"B","type_id":0"#, Error::Binding),
        (r#""id":"B","type_id":1"#, r#""id":"B","type_id":0"#, Error::Binding),
        (r#""type_id":0"#, r#""type_id":1"#, Error::Binding),
        (r#""type_id":0"#, r#""type_id":null"#, Error::InvalidInput),
        (r#""type":"TemplateProcessing""#, r#""type":"BertProcessing""#, Error::Binding),
        (r#""type":"TemplateProcessing""#, r#""type":"TemplateProcessing","extra":0"#, Error::Binding),
        (r#""ids":[259]"#, r#""ids":[259],"ids":[259]"#, Error::Duplicate),
    ] {
        assert!(TEMPLATE.contains(from), "{from}");
        let changed = TEMPLATE.replacen(from, to, 1);
        assert_eq!(import(&document(&changed)).unwrap_err(), error, "{from} -> {to}");
    }
    let removed_bos = TEMPLATE.replacen(
        r#""single":[{"SpecialToken":{"id":"<|begin_of_text|>","type_id":0}},"#,
        r#""single":["#, 1);
    assert_ne!(removed_bos, TEMPLATE);
    assert_eq!(import(&document(&removed_bos)).unwrap_err(), Error::Binding);
    let suffix = TEMPLATE.replacen(
        r#""single":[{"SpecialToken":{"id":"<|begin_of_text|>","type_id":0}},{"Sequence":{"id":"A","type_id":0}}]"#,
        r#""single":[{"Sequence":{"id":"A","type_id":0}},{"SpecialToken":{"id":"<|begin_of_text|>","type_id":0}}]"#, 1);
    assert_ne!(suffix, TEMPLATE);
    assert_eq!(import(&document(&suffix)).unwrap_err(), Error::Binding);
}

#[test]
fn bos_wrapper_does_not_admit_offset_trimming_untyped_flags_or_extra_processors() {
    let wrapper = wrapped(TEMPLATE, true, true);
    let good = document(&wrapper);
    assert_eq!(import(&good).unwrap().bos_token(), Some(BOS));
    for (from, to, error) in [
        (r#""trim_offsets":false"#, r#""trim_offsets":true"#, Error::Binding),
        (r#""use_regex":true"#, r#""use_regex":null"#, Error::InvalidInput),
        (r#""add_prefix_space":true"#, r#""add_prefix_space":null"#, Error::InvalidInput),
        (r#""processors":["#, r#""processors":[{"type":"ByteLevel","add_prefix_space":true,"trim_offsets":false,"use_regex":true},"#, Error::Binding),
    ] {
        assert!(wrapper.contains(from));
        assert_eq!(import(&document(&wrapper.replacen(from, to, 1))).unwrap_err(), error);
    }
    let reordered = format!(r#"{{"type":"Sequence","processors":[{TEMPLATE},{{"type":"ByteLevel","add_prefix_space":true,"trim_offsets":false,"use_regex":true}}]}}"#);
    assert_eq!(import(&document(&reordered)).unwrap_err(), Error::Binding);
}

#[test]
fn bos_import_keeps_exact_byte_limits_and_full_original_graph_admission() {
    let source = document(TEMPLATE);
    assert!(ByteBpe::from_huggingface_llama3_bos_json(&profile(),
        source.as_bytes(), source.len()).is_ok());
    assert_eq!(ByteBpe::from_huggingface_llama3_bos_json(&profile(),
        source.as_bytes(), source.len() - 1).unwrap_err(), Error::Limit);
    assert_eq!(ByteBpe::from_huggingface_llama3_bos_json(&profile(), b"", 0).unwrap_err(), Error::Limit);
    assert_eq!(ByteBpe::from_huggingface_llama3_bos_json(&profile(), b"{}",
        ByteBpe::MAX_HUGGINGFACE_JSON_BYTES + 1).unwrap_err(), Error::Limit);
    let missing_merge = source.replace(r#""merges":[["a","a"],["a","1"],["Ã","©"]]"#, r#""merges":[]"#);
    assert_ne!(missing_merge, source);
    assert_eq!(import(&missing_merge).unwrap_err(), Error::Incomplete);
    assert!(import(&source[..source.len() - 1]).is_err());
    assert_eq!(import(&format!("{source}{{}}")).unwrap_err(), Error::InvalidInput);
}
