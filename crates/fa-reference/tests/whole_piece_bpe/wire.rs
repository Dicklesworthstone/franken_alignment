use super::fixture::{CONTROL, merges, names, profile, vocabulary};
use fa_reference::action::consequence::activation::{
    monitor::decoder::sampled::generation::tokenizer::{ByteBpe, MergePolicy, PreTokenization},
    tensor::kv::decoder::RotaryScaling,
};
use fa_reference::Error;
use std::collections::BTreeMap;

// A complete independent wire expectation. Original model fields, token IDs,
// byte spellings and rank triples are literal fixture facts, not serializer output.
fn golden(domain: &[u8; 8], extension: &[u8], named: bool, scaled: bool) -> Vec<u8> {
    let mut bytes = domain.to_vec();
    for value in [11_u64, 12, 13, 14, 15, 261, 2, 2, 1, 1, 1, 64,
        1e-5_f64.to_bits(), 10000.0_f64.to_bits()]
    { bytes.extend_from_slice(&value.to_be_bytes()); }
    bytes.extend_from_slice(extension);
    if scaled {
        for value in [1_u64, 2.0_f64.to_bits(), 0, 0, 0] {
            bytes.extend_from_slice(&value.to_be_bytes());
        }
    }
    for id in 0..256_u16 {
        bytes.extend_from_slice(&[1, 0, 0, 0, 1, ((id + 239) % 256) as u8]);
    }
    for word in [b"ab".as_slice(), b"bc", b"abc", b"a1"] {
        bytes.push(1);
        bytes.extend_from_slice(&(word.len() as u32).to_be_bytes());
        bytes.extend_from_slice(word);
    }
    bytes.push(0);
    bytes.extend_from_slice(&4_u32.to_be_bytes());
    for value in [114_u32, 115, 256, 115, 116, 257, 114, 257, 258, 114, 66, 259] {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    if named {
        for value in [1_u32, 260, 3] { bytes.extend_from_slice(&value.to_be_bytes()); }
        bytes.extend_from_slice(b"<q>");
    }
    bytes
}

#[test]
fn v7_binds_the_whole_interpretation_and_every_original_extension() {
    for named in [false, true] {
        for scaled in [false, true] {
            let profile = if scaled { profile().with_rotary_scaling(RotaryScaling::linear(2.0).unwrap()).unwrap() }
                else { profile() };
            for (mode, algorithm, major) in [(PreTokenization::RawBytes, 0, 0),
                (PreTokenization::Gpt2Unicode17, 1, 17), (PreTokenization::Llama3Unicode17, 2, 17)]
            {
                for bos in [false, true] {
                    if bos && (!named || mode != PreTokenization::Llama3Unicode17) { continue; }
                    let registered = if named { names() } else { BTreeMap::new() };
                    let tokenizer = if bos {
                        ByteBpe::new_with_bos_token_and_merge_policy(profile.clone(), vocabulary(false),
                            merges(false), registered, CONTROL, MergePolicy::WholePieceFirst)
                    } else {
                        ByteBpe::new_with_merge_policy(profile.clone(), vocabulary(false), merges(false),
                            registered, mode, MergePolicy::WholePieceFirst)
                    }.unwrap();
                    let mut extension = vec![algorithm, major, 0, 0,
                        u8::from(named) | (u8::from(scaled) << 1), u8::from(bos), 1, 0];
                    if bos { extension.extend_from_slice(&260_u32.to_be_bytes()); }
                    let expected = golden(b"FABBPE07", &extension, named, scaled);
                    assert_eq!(tokenizer.to_bytes().unwrap(), expected);
                    let restored = ByteBpe::from_bytes(&profile, &expected).unwrap();
                    assert_eq!(restored.to_bytes().unwrap(), expected);
                    assert_eq!(restored.merge_policy(), MergePolicy::WholePieceFirst);
                    assert_eq!(restored.pre_tokenization(), mode);
                    assert_eq!(restored.bos_token(), bos.then_some(CONTROL));
                    let expected_ids: &[u32] = if bos { &[260, 258] } else { &[258] };
                    assert_eq!(restored.encode(b"abc", Default::default()).unwrap().tokens(), expected_ids);
                }
            }
        }
    }
}

#[test]
fn all_six_legacy_formats_keep_their_exact_bytes_and_ranked_semantics() {
    for named in [false, true] {
        for scaled in [false, true] {
            let profile = if scaled { profile().with_rotary_scaling(RotaryScaling::linear(2.0).unwrap()).unwrap() }
                else { profile() };
            let registered = if named { names() } else { BTreeMap::new() };
            let domain = match (named, scaled) {
                (false, false) => b"FABBPE01", (true, false) => b"FABBPE02",
                (false, true) => b"FABBPE03", (true, true) => b"FABBPE04",
            };
            let raw = ByteBpe::new_with_pre_tokenization(profile.clone(), vocabulary(false),
                merges(false), registered.clone(), PreTokenization::RawBytes).unwrap();
            assert_eq!(raw.to_bytes().unwrap(), golden(domain, &[], named, scaled));
            for (mode, tag) in [(PreTokenization::Gpt2Unicode17, 1), (PreTokenization::Llama3Unicode17, 2)] {
                let unicode = ByteBpe::new_with_pre_tokenization(profile.clone(), vocabulary(false),
                    merges(false), registered.clone(), mode).unwrap();
                let expected = golden(b"FABBPE05", &[tag, 17, 0, 0,
                    u8::from(named) | (u8::from(scaled) << 1), 0], named, scaled);
                assert_eq!(unicode.to_bytes().unwrap(), expected);
                let restored = ByteBpe::from_bytes(&profile, &expected).unwrap();
                assert_eq!(restored.merge_policy(), MergePolicy::RankedOnly);
                assert_eq!(restored.encode(b"abc", Default::default()).unwrap().tokens(), &[256, 116]);
            }
            if named {
                let bos = ByteBpe::new_with_bos_token(profile.clone(), vocabulary(false),
                    merges(false), names(), CONTROL).unwrap();
                let mut extension = vec![2, 17, 0, 0, 1 | (u8::from(scaled) << 1), 1];
                extension.extend_from_slice(&260_u32.to_be_bytes());
                let expected = golden(b"FABBPE06", &extension, true, scaled);
                assert_eq!(bos.to_bytes().unwrap(), expected);
                let restored = ByteBpe::from_bytes(&profile, &expected).unwrap();
                assert_eq!(restored.merge_policy(), MergePolicy::RankedOnly);
                assert_eq!(restored.encode(b"abc", Default::default()).unwrap().tokens(), &[260, 256, 116]);
            }
        }
    }
}

#[test]
fn v7_refuses_unknown_policy_version_flags_profile_truncation_and_trailing_bytes() {
    let tokenizer = ByteBpe::new_with_bos_token_and_merge_policy(profile(), vocabulary(false),
        merges(false), names(), CONTROL, MergePolicy::WholePieceFirst).unwrap();
    let bytes = tokenizer.to_bytes().unwrap();
    for (offset, value) in [(120, 3), (121, 16), (122, 1), (123, 1), (124, 5),
        (125, 2), (126, 0), (126, 2), (127, 1)]
    {
        let mut changed = bytes.clone(); changed[offset] = value;
        assert_eq!(ByteBpe::from_bytes(&profile(), &changed).unwrap_err(), Error::Binding);
    }
    for end in 0..bytes.len() {
        assert!(ByteBpe::from_bytes(&profile(), &bytes[..end]).is_err(), "prefix {end}");
    }
    let mut changed = bytes.clone(); changed.push(0);
    assert!(ByteBpe::from_bytes(&profile(), &changed).is_err());
    let mut changed = bytes.clone(); changed[15] ^= 1;
    assert_eq!(ByteBpe::from_bytes(&profile(), &changed).unwrap_err(), Error::Binding);
    let mut changed = bytes; changed[..8].copy_from_slice(b"FABBPE06");
    assert!(ByteBpe::from_bytes(&profile(), &changed).is_err());
}
