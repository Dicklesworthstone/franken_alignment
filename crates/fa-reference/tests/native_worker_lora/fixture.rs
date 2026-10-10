//! The original process fixture plus one causal FFN adapter. Base down is zero;
//! gate/up are identity. The adapter swaps Allow/Deny for the two original inputs,
//! while its row-sum zero preserves the explicit stop step.
use super::{assets, rotary_assets};
use fa_reference::action::consequence::activation::tensor::kv::decoder::DecoderIdentity;
use fa_reference::action::consequence::activation::tensor::kv::decoder::safetensors::lora::adapted_profile;
use fa_reference::full_input::InputProfileBinding;
use fa_reference::strict_json::{self, Limits};
#[allow(dead_code)]
#[path = "../support/pretrained_fixture.rs"]
mod tensors;

pub fn expected() -> InputProfileBinding {
    InputProfileBinding { profile_id: 8, profile_bytes: b"adapted native categorical fixture".to_vec(),
        model_epoch: 1, tokenizer_epoch: 0, policy_epoch: 0 }
}
pub fn configure(fixture: &mut assets::Fixture, rank: usize, zero: bool, json: bool) {
    rotary_assets::configure(fixture, rotary_assets::LINEAR, false, json);
    let old = fixture.policy.decoder_profile.clone();
    let new = DecoderIdentity { model_generation: 6, profile_generation: 7, ..old.identity() };
    fixture.policy.decoder_profile = adapted_profile(&old, new).unwrap();
    fixture.policy.input_profile = expected();
    let monitor = std::fs::read_to_string(fixture.root.join("monitor.json")).unwrap()
        .replace("\"model_generation\":3", "\"model_generation\":6")
        .replace("\"profile_generation\":5", "\"profile_generation\":7");
    std::fs::write(fixture.root.join("monitor.json"), monitor).unwrap();
    if !json {
        std::fs::write(fixture.root.join("tokenizer.bin"),
            rotary_assets::tokenizer(&fixture.policy.decoder_profile)).unwrap();
    }
    let path = fixture.root.join("weights.safetensors");
    let mut bytes = std::fs::read(&path).unwrap();
    let length = u64::from_le_bytes(bytes[..8].try_into().unwrap()) as usize;
    let header = strict_json::parse(&bytes[8..8 + length], Limits {
        max_bytes: 65536, max_depth: 5, max_items: 4096, max_string_bytes: 4096 }).unwrap();
    for name in ["model.layers.0.mlp.gate_proj.weight", "model.layers.0.mlp.up_proj.weight"] {
        let offset = header.get(name).unwrap().get("data_offsets").unwrap().as_array().unwrap()[0].as_u64().unwrap() as usize;
        let data = [1.0_f32, 0.0, 0.0, 1.0].into_iter().flat_map(f32::to_le_bytes).collect::<Vec<_>>();
        bytes[8 + length + offset..8 + length + offset + 16].copy_from_slice(&data);
    }
    std::fs::write(path, bytes).unwrap();

    assert!([1, 2].contains(&rank));
    let a = if rank == 1 { vec![1.0_f32, -1.0] } else { vec![1.0, -1.0, 0.0, 0.0] };
    let mut b = if rank == 1 { vec![-0.25_f32, 0.25] } else { vec![-0.5, 0.0, 0.5, 0.0] };
    if zero { b.fill(0.0); }
    let rows = [("A", vec![rank, 2], a), ("B", vec![2, rank], b)].into_iter().map(|(name, shape, values)|
        tensors::Tensor { name: format!("base_model.model.model.layers.0.mlp.down_proj.lora_{name}.weight"),
            shape, dtype: "F32", bytes: values.into_iter().flat_map(f32::to_le_bytes).collect() }).collect::<Vec<_>>();
    std::fs::write(fixture.root.join("adapter.safetensors"), tensors::encode(&rows)).unwrap();
    std::fs::write(fixture.root.join("adapter_config.json"), format!(
        r#"{{"peft_type":"LORA","task_type":"CAUSAL_LM","inference_mode":true,"r":{rank},"lora_alpha":4,"target_modules":["down_proj"],"bias":"none","use_dora":false,"use_rslora":false,"fan_in_fan_out":false,"lora_bias":false,"modules_to_save":null,"rank_pattern":{{}},"alpha_pattern":{{}},"init_lora_weights":true}}"#)).unwrap();
    fixture.manifest = fixture.manifest.replace("fa.native-worker/3", "fa.native-worker/4")
        .replace("\"model_generation\":3", "\"model_generation\":6")
        .replace("\"profile_generation\":5", "\"profile_generation\":7");
    let old_input = assets::expected(); let input = expected();
    let old_hex = old_input.profile_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let hex = input.profile_bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    fixture.manifest = fixture.manifest.replace(&format!("\"id\":7,\"bytes_hex\":\"{old_hex}\",\"model_epoch\":0"),
        &format!("\"id\":8,\"bytes_hex\":\"{hex}\",\"model_epoch\":1"));
    let quote = super::v2_assets::quote;
    let configuration = quote(fixture.root.join("adapter_config.json").to_str().unwrap());
    let weights = quote(fixture.root.join("adapter.safetensors").to_str().unwrap());
    fixture.manifest = format!("{},\"adapter\":{{\"base_identity\":{},\"configuration\":{configuration},\"weights\":{weights},\"merge_products\":{}}}}}",
        fixture.manifest.strip_suffix('}').unwrap(), base_identity(), 4 * rank);
    fixture.save();
}
pub fn base_identity() -> &'static str {
    r#"{"tenant":1,"model":2,"model_generation":3,"tokenizer_generation":4,"profile_generation":5}"#
}
pub fn frame(prompt: &[u8]) -> Vec<u8> {
    let profile = expected();
    let mut out = b"FAHW1".to_vec(); out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&9_u64.to_be_bytes()); out.extend_from_slice(&[7; 32]);
    out.extend_from_slice(&6_u16.to_be_bytes()); out.extend_from_slice(b"native");
    out.extend_from_slice(&64_u16.to_be_bytes());
    for number in [profile.profile_id, profile.model_epoch, profile.tokenizer_epoch, profile.policy_epoch] { out.extend_from_slice(&number.to_be_bytes()); }
    for bytes in [profile.profile_bytes.as_slice(), prompt] {
        out.extend_from_slice(&u32::try_from(bytes.len()).unwrap().to_be_bytes()); out.extend_from_slice(bytes);
    }
    out.extend_from_slice(&1_u16.to_be_bytes()); out.extend_from_slice(&0_u32.to_be_bytes());
    out.extend_from_slice(&u32::try_from(prompt.len()).unwrap().to_be_bytes()); out.push(1);
    out.extend_from_slice(&0_u16.to_be_bytes());
    let length = u32::try_from(out.len() - 9).unwrap(); out[5..9].copy_from_slice(&length.to_be_bytes()); out
}
