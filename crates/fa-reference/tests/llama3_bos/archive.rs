use super::fixture::{BOS, BOS_NAME, merges, names, profile, tokenizer, vocabulary};
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, PreTokenization, TokenizationBudget,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderIdentity, DecoderProfile, RotaryScaling,
};
use fa_reference::Error;
use std::collections::BTreeMap;

// Independent complete archive fixture. No production encoder, profile accessor,
// vocabulary constructor or merge constructor supplies any golden bytes.
fn golden(domain: &[u8; 8], extension: &[u8], named: bool, scaled: bool) -> Vec<u8> {
    let mut bytes = domain.to_vec();
    for word in [11_u64, 12, 13, 14, 15, 261, 2, 2, 1, 1, 1, 64,
        1e-5_f64.to_bits(), 10000.0_f64.to_bits()]
    { bytes.extend_from_slice(&word.to_be_bytes()); }
    bytes.extend_from_slice(extension);
    if scaled {
        for word in [1_u64, 2.0_f64.to_bits(), 0, 0, 0] {
            bytes.extend_from_slice(&word.to_be_bytes());
        }
    }
    for id in 0..256_u16 {
        bytes.extend_from_slice(&[1, 0, 0, 0, 1, ((id + 239) % 256) as u8]);
    }
    for word in [b"aa".as_slice(), b"a1".as_slice(), &[0xc3, 0xa9]] {
        bytes.extend_from_slice(&[1, 0, 0, 0, 2]);
        bytes.extend_from_slice(word);
    }
    bytes.extend_from_slice(&[0, 0]); // Original controls 259 and 260.
    bytes.extend_from_slice(&3_u32.to_be_bytes());
    for word in [114_u32, 114, 256, 114, 66, 257, 212, 186, 258] {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    if named {
        bytes.extend_from_slice(&2_u32.to_be_bytes());
        for (id, name) in [(259_u32, b"<|begin_of_text|>".as_slice()), (260, b"<end>".as_slice())] {
            bytes.extend_from_slice(&id.to_be_bytes());
            bytes.extend_from_slice(&(name.len() as u32).to_be_bytes());
            bytes.extend_from_slice(name);
        }
    }
    bytes
}

#[test]
fn bos_archive_has_exact_v6_bytes_and_retains_all_five_original_archive_profiles() {
    for named in [false, true] {
        for scaled in [false, true] {
            let p = if scaled { profile().with_rotary_scaling(RotaryScaling::linear(2.0).unwrap()).unwrap() }
                else { profile() };
            let registered = if named { names() } else { BTreeMap::new() };
            let raw = ByteBpe::new_with_pre_tokenization(p.clone(), vocabulary(), merges(),
                registered.clone(), PreTokenization::RawBytes).unwrap();
            let domain = match (named, scaled) {
                (false, false) => b"FABBPE01", (true, false) => b"FABBPE02",
                (false, true) => b"FABBPE03", (true, true) => b"FABBPE04",
            };
            let expected = golden(domain, &[], named, scaled);
            assert_eq!(raw.to_bytes().unwrap(), expected);
            assert_eq!(ByteBpe::from_bytes(&p, &expected).unwrap().to_bytes().unwrap(), expected);
            for (mode, tag) in [(PreTokenization::Gpt2Unicode17, 1), (PreTokenization::Llama3Unicode17, 2)] {
                let unicode = ByteBpe::new_with_pre_tokenization(p.clone(), vocabulary(), merges(),
                    registered.clone(), mode).unwrap();
                let extension = [tag, 17, 0, 0, u8::from(named) | (u8::from(scaled) << 1), 0];
                let expected = golden(b"FABBPE05", &extension, named, scaled);
                assert_eq!(unicode.to_bytes().unwrap(), expected);
                let restored = ByteBpe::from_bytes(&p, &expected).unwrap();
                assert_eq!(restored.to_bytes().unwrap(), expected);
                assert_eq!(restored.bos_token(), None);
                assert_eq!(restored.pre_tokenization(), mode);
            }
            if named {
                let bos = ByteBpe::new_with_bos_token(p.clone(), vocabulary(), merges(), names(), BOS).unwrap();
                // Unicode metadata, prefix=true, followed by original u32 ID259.
                let extension = [2, 17, 0, 0, 1 | (u8::from(scaled) << 1), 1, 0, 0, 1, 3];
                let expected = golden(b"FABBPE06", &extension, true, scaled);
                assert_eq!(bos.to_bytes().unwrap(), expected);
                let restored = ByteBpe::from_bytes(&p, &expected).unwrap();
                assert_eq!(restored.to_bytes().unwrap(), expected);
                assert_eq!(restored.bos_token(), Some(BOS));
                assert_eq!(restored.encode(b"aa", TokenizationBudget::default()).unwrap().tokens(), &[259, 256]);
            }
        }
    }
}

#[test]
fn bos_archive_refuses_changed_interpretation_malformed_prefix_and_wrong_model() {
    let bytes = tokenizer().to_bytes().unwrap();
    for (offset, value) in [(7, b'5'), (120, 1), (121, 16), (122, 1), (123, 1),
        (124, 0), (124, 3), (124, 5), (125, 0), (125, 2)]
    {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert_eq!(ByteBpe::from_bytes(&profile(), &bad).unwrap_err(), Error::Binding);
    }
    for bos in [114_u32, 261, u32::MAX] {
        let mut bad = bytes.clone();
        bad[126..130].copy_from_slice(&bos.to_be_bytes());
        assert_eq!(ByteBpe::from_bytes(&profile(), &bad).unwrap_err(), Error::Binding);
    }
    for length in [0, 8, 119, 120, 125, 126, 127, 129, 130, bytes.len() - 1] {
        assert!(ByteBpe::from_bytes(&profile(), &bytes[..length]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert_eq!(ByteBpe::from_bytes(&profile(), &trailing).unwrap_err(), Error::InvalidInput);
    let original = profile();
    let changed = DecoderProfile::new(DecoderIdentity { tokenizer_generation: 99, ..original.identity() },
        original.shape(), original.epsilon(), original.theta()).unwrap();
    assert_eq!(ByteBpe::from_bytes(&changed, &bytes).unwrap_err(), Error::Binding);
}

#[test]
fn bos_archive_binds_the_actual_prefix_even_when_prompt_bytes_are_identical() {
    let original = tokenizer();
    let mut changed_bytes = original.to_bytes().unwrap();
    changed_bytes[126..130].copy_from_slice(&260_u32.to_be_bytes());
    let changed = ByteBpe::from_bytes(&profile(), &changed_bytes).unwrap();
    assert_eq!(changed.bos_token(), Some(260));
    assert_eq!(changed.to_bytes().unwrap(), changed_bytes);
    assert_ne!(changed.to_bytes().unwrap(), original.to_bytes().unwrap());
    let original_input = original.encode(BOS_NAME.as_bytes(), TokenizationBudget::default()).unwrap();
    let changed_input = changed.encode(BOS_NAME.as_bytes(), TokenizationBudget::default()).unwrap();
    assert_eq!(original_input.source(), changed_input.source());
    assert_eq!(original_input.tokens(), &[259, 259]);
    assert_eq!(changed_input.tokens(), &[260, 259]);
    assert_eq!(original_input.spans(), changed_input.spans());
}
