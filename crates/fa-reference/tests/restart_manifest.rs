use fa_reference::action::consequence::gate::containment::{
    ActorState, RestartGrade, RestartProfile, MAX_CACHE_BYTES, MAX_SAMPLER_BYTES, MAX_TOKENS,
};
use fa_reference::Error;

#[path = "../src/action/consequence/gate/containment/restart_manifest.rs"]
mod restart_manifest;

use restart_manifest::{RestartManifest, MAX_RESTART_MANIFEST_BYTES};

fn profile() -> RestartProfile {
    RestartProfile { id: 11, generation: 12, host_generation: 13, model_generation: 14,
        tokenizer_generation: 15, state_schema_generation: 16, grade: RestartGrade::ExactRestart }
}

fn actor() -> ActorState {
    ActorState::new(profile(), vec![10, 20, 30], vec![1, 2, 3, 4], vec![5, 6, 7], 3).unwrap()
}

#[test]
fn portable_actor_state_round_trips_bit_exactly() {
    let bytes = RestartManifest::capture(&actor()).unwrap().encode().unwrap();
    let restored = RestartManifest::decode_for(&bytes, profile()).unwrap();
    assert_eq!(restored.actor(), &actor());
}

#[test]
fn every_single_byte_truncation_refuses() {
    let bytes = RestartManifest::capture(&actor()).unwrap().encode().unwrap();
    for end in 0..bytes.len() { assert!(RestartManifest::decode(&bytes[..end]).is_err(), "end={end}"); }
}

#[test]
fn component_bitmap_cannot_hide_missing_mutable_state() {
    let bytes = RestartManifest::capture(&actor()).unwrap().encode().unwrap();
    for bit in [1_u8, 2, 4, 8] {
        let mut changed = bytes.clone(); changed[8] &= !bit;
        assert_eq!(RestartManifest::decode(&changed), Err(Error::Incomplete));
    }
}

#[test]
fn exact_profile_identity_is_not_shape_compatibility() {
    let bytes = RestartManifest::capture(&actor()).unwrap().encode().unwrap();
    for field in 0..6 {
        let mut expected = profile();
        match field {
            0 => expected.id += 1,
            1 => expected.generation += 1,
            2 => expected.host_generation += 1,
            3 => expected.model_generation += 1,
            4 => expected.tokenizer_generation += 1,
            _ => expected.state_schema_generation += 1,
        }
        assert_eq!(RestartManifest::decode_for(&bytes, expected), Err(Error::Binding));
    }
}

#[test]
fn exact_registered_limits_are_encodable() {
    let state = ActorState::new(profile(), vec![1; MAX_TOKENS], vec![2; MAX_CACHE_BYTES],
        vec![3; MAX_SAMPLER_BYTES], MAX_TOKENS as u64).unwrap();
    let manifest = RestartManifest::capture(&state).unwrap();
    let bytes = manifest.encode().unwrap();
    assert_eq!(bytes.len(), MAX_RESTART_MANIFEST_BYTES);
    assert_eq!(RestartManifest::decode(&bytes).unwrap().actor(), &state);
}
