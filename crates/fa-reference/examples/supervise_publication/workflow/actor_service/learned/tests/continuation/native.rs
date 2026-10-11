//! Independently provision reusable BPE pieces for repeated complete reviews.
//! No request-specific output token, skipped input byte or policy-capture bypass.
use super::{Root, fs};
use super::super::models;
use fa_reference::action::consequence::activation::monitor::decoder::sampled::generation::tokenizer::{
    ByteBpe, Merge, TokenBytes, TokenizationBudget,
};
use fa_reference::action::consequence::activation::tensor::kv::decoder::{
    DecoderProfile, DecoderShape,
};

fn join(vocabulary: &mut Vec<TokenBytes>, merges: &mut Vec<Merge>, left: u32, right: u32) -> u32 {
    let TokenBytes::Content(mut value) = vocabulary[left as usize].clone() else { unreachable!() };
    let TokenBytes::Content(suffix) = &vocabulary[right as usize] else { unreachable!() };
    value.extend_from_slice(suffix);
    if let Some(id) = vocabulary.iter().position(|token| *token == TokenBytes::Content(value.clone())) {
        return id as u32;
    }
    let result = vocabulary.len() as u32;
    vocabulary.push(TokenBytes::Content(value));
    merges.push(Merge { left, right, result });
    result
}

fn word(vocabulary: &mut Vec<TokenBytes>, merges: &mut Vec<Merge>, bytes: &[u8]) -> u32 {
    bytes[1..].iter().fold(u32::from(bytes[0]), |left, right|
        join(vocabulary, merges, left, u32::from(*right)))
}

pub(super) fn provision(root: &Root) {
    let mut vocabulary: Vec<_> = (0..=255).map(|byte| TokenBytes::Content(vec![byte])).collect();
    let mut merges = Vec::new();
    let response = word(&mut vocabulary, &mut merges, b"allow");
    for bytes in [b"Should this exact publication proceed?".as_slice(),
        b"reference/helper/alpha", b"reference/helper/beta"] {
        word(&mut vocabulary, &mut merges, bytes);
    }
    let mut zeros = vec![0_u32];
    for _ in 0..6 {
        let previous = *zeros.last().unwrap();
        zeros.push(join(&mut vocabulary, &mut merges, previous, previous));
    }
    // Fixed reusable big-endian scalar fragments, selected before seeing any
    // message or sidecar. The full ORIGINAL input still passes byte-BPE encode,
    // exact prompt coverage and one original monitored decoder step per token.
    // Keep the vocabulary below the existing two-draw /4096 score allowance.
    for byte in 1_u32..=255 {
        let one = join(&mut vocabulary, &mut merges, zeros[0], byte);
        let two = join(&mut vocabulary, &mut merges, zeros[1], byte);
        let three = join(&mut vocabulary, &mut merges, zeros[1], one);
        join(&mut vocabulary, &mut merges, zeros[2], byte);
        join(&mut vocabulary, &mut merges, zeros[2], two);
        join(&mut vocabulary, &mut merges, zeros[2], three);
    }
    let stop = vocabulary.len();
    vocabulary.push(TokenBytes::Control);
    let count = vocabulary.len();
    assert!(count * 2 <= 4096, "retain the independently declared native sampling cap");
    let profile = DecoderProfile::new(models::identity(31), DecoderShape {
        vocabulary: count, hidden: 2, intermediate: 2, layers: 1, query_heads: 1,
        cache_heads: 1, context: 4096,
    }, 0.00001, 10000.0).unwrap();
    let tokenizer = ByteBpe::new(profile, vocabulary, merges).unwrap();
    // Exercise the actual ranked tokenizer: earlier zero-run merges must still
    // compose with the later scalar fragments, preserving every original byte.
    for value in [1_u64, 51, 71, 201, 255, 1001] {
        let bytes = value.to_be_bytes();
        let encoded = tokenizer.encode(&bytes, TokenizationBudget::default()).unwrap();
        assert!(encoded.tokens().len() <= 2);
        assert_eq!(tokenizer.decode(encoded.tokens(), bytes.len()).unwrap(), bytes);
    }
    fs::write(root.0.join("helper-model.json"), models::configuration(count, 4096)).unwrap();
    fs::write(root.0.join("helper-weights.safetensors"),
        models::weights(count, response as usize, stop, false)).unwrap();
    fs::write(root.0.join("helper-tokenizer.bbpe"), tokenizer.to_bytes().unwrap()).unwrap();
    fs::write(root.0.join("helper-sampling.json"), models::sampling(count)).unwrap();
    let roster = fs::read_to_string(root.0.join("native-roster.json")).unwrap();
    let marker = "\"stop_tokens\":[";
    let start = roster.find(marker).unwrap() + marker.len();
    let end = start + roster[start..].find(']').unwrap();
    let original = format!("{marker}{}]", &roster[start..end]);
    assert_eq!(roster.matches(&original).count(), 2);
    fs::write(root.0.join("native-roster.json"),
        roster.replace(&original, &format!("{marker}{stop}]"))).unwrap();
}
