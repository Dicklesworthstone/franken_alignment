//! Independent descriptor bytes; no fabricated generated output or effect key.
use super::*;

fn proposal() -> LearnedTextProposal {
    LearnedTextProposal { target: ResolvedTarget { adapter: 10, object: 11,
        contract_version: 1, expected_version: 2, generation: 3 },
        expected_policy_epoch: 0, deadline: ElapsedTick(u64::MAX), units: 2 }
}

#[test]
fn raw_intent_preserves_full_width_identifiers_and_effect_units_not_envelope_units() {
    let key = 0x0102_0304_0506_0708;
    let mut input = proposal(); input.units = 0x8877_6655_4433_2211;
    let encoded = FileLearnedTextActorPort::encode_request(key, input).unwrap();
    assert_eq!(encoded.payload, [70,65,76,84,88,84,0,1, 1,2,3,4,5,6,7,8,
        136,119,102,85,68,51,34,17]);
    assert_eq!(encoded.units, 24);
    assert_eq!(decode(key, &encoded), Ok(input));
    for units in [0, 1, 2, u64::MAX] {
        input.units = units;
        let encoded = FileLearnedTextActorPort::encode_request(u64::MAX, input).unwrap();
        assert_eq!(decode(u64::MAX, &encoded), Ok(input));
    }
}

#[test]
fn stream_intent_has_one_canonical_envelope_and_no_actor_selected_effect_coordinates() {
    let key = 0x0102_0304_0506_0708;
    let message = FileLearnedTextStreamActorPort::encode_release(key,
        LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    assert_eq!(message.payload, [70,65,76,82,69,76,0,1, 1,2,3,4,5,6,7,8, 0]);
    assert_eq!(message.units, 17);
    assert_eq!(stream::decode(key, &message), Ok(LearnedTextRelease::Message));
    let finish = FileLearnedTextStreamActorPort::encode_release(key,
        LearnedTextRelease::Finish, ElapsedTick(100)).unwrap();
    let mut golden = message.payload.clone(); golden[16] = 1;
    assert_eq!(finish.payload, golden);
    assert_eq!(stream::decode(key, &finish), Ok(LearnedTextRelease::Finish));
    for field in 0..8 {
        let mut bad = message.clone();
        match field {
            0 => bad.target.adapter += 1, 1 => bad.target.object += 1,
            2 => bad.target.contract_version += 1, 3 => bad.target.expected_version += 1,
            4 => bad.target.generation += 1, 5 => bad.expected_policy_epoch += 1,
            6 => bad.units += 1, _ => bad.payload[16] = 2,
        }
        assert_eq!(stream::decode(key, &bad), Err(ActorError::MalformedProposal));
    }
}

#[test]
fn both_intent_decoders_reject_every_truncation_suffix_wrong_key_and_cross_profile() {
    let raw = FileLearnedTextActorPort::encode_request(71, proposal()).unwrap();
    let framed = FileLearnedTextStreamActorPort::encode_release(71,
        LearnedTextRelease::Message, ElapsedTick(100)).unwrap();
    for end in 0..raw.payload.len() {
        let mut bad = raw.clone(); bad.payload.truncate(end);
        assert_eq!(decode(71, &bad), Err(ActorError::MalformedProposal));
    }
    for end in 0..framed.payload.len() {
        let mut bad = framed.clone(); bad.payload.truncate(end);
        assert_eq!(stream::decode(71, &bad), Err(ActorError::MalformedProposal));
    }
    for original in [&raw, &framed] {
        for mutation in 0..4 {
            let mut bad = original.clone();
            match mutation { 0 => bad.payload.push(0), 1 => bad.payload[0] ^= 1,
                2 => bad.payload[8] ^= 1, _ => bad.units += 1 }
            assert!(decode(71, &bad).is_err());
            assert!(stream::decode(71, &bad).is_err());
        }
        for key in [0, 72, u64::MAX] {
            assert!(decode(key, original).is_err());
            assert!(stream::decode(key, original).is_err());
        }
    }
    assert_eq!(decode(71, &framed), Err(ActorError::MalformedProposal));
    assert_eq!(stream::decode(71, &raw), Err(ActorError::MalformedProposal));
    assert!(FileLearnedTextActorPort::encode_request(0, proposal()).is_err());
    assert!(FileLearnedTextStreamActorPort::encode_release(0,
        LearnedTextRelease::Finish, ElapsedTick(100)).is_err());
}
