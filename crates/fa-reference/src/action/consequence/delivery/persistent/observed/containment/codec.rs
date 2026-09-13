//! Bounded operation inputs using the original journal primitives and native
//! ActorState/TargetCeiling validators. Existing bootstrap/event bytes are intact.
use super::{FileResetRequest, FileStateUpdate};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::MAX_ATTEMPTS;
use crate::action::consequence::gate::ReviewBinding;
use crate::action::consequence::gate::containment::{ActorState, RestartGrade, RestartProfile,
    MAX_CACHE_BYTES, MAX_SAMPLER_BYTES, MAX_TOKENS};
use crate::Error;

pub(in super::super) fn write_update(w: &mut Writer, update: &FileStateUpdate) -> Result<(), Error> {
    if update.operation == 0 { return Err(Error::InvalidInput); }
    w.u64(update.operation)?; w.u64(update.expected_actor_revision)?; w.u64(update.expected_authority_epoch)?;
    let actor = &update.state; let p = actor.profile();
    for value in [p.id, p.generation, p.host_generation, p.model_generation,
        p.tokenizer_generation, p.state_schema_generation, actor.next_position()] { w.u64(value)?; }
    w.u8(match p.grade { RestartGrade::AuditOnly => 0, RestartGrade::FunctionalRestart => 1, RestartGrade::ExactRestart => 2 })?;
    w.count(actor.tokens().len())?;
    for token in actor.tokens() { w.u32(*token)?; }
    w.blob(actor.cache())?; w.blob(actor.sampler())?;
    Ok(())
}
pub(in super::super) fn read_update(r: &mut Reader<'_>) -> Result<FileStateUpdate, Error> {
    let operation = r.u64()?; let expected_actor_revision = r.u64()?; let expected_authority_epoch = r.u64()?;
    if operation == 0 { return Err(Error::InvalidInput); }
    let mut profile = RestartProfile { id: r.u64()?, generation: r.u64()?, host_generation: r.u64()?,
        model_generation: r.u64()?, tokenizer_generation: r.u64()?, state_schema_generation: r.u64()?, grade: RestartGrade::AuditOnly };
    let position = r.u64()?;
    profile.grade = match r.u8()? { 0 => RestartGrade::AuditOnly, 1 => RestartGrade::FunctionalRestart,
        2 => RestartGrade::ExactRestart, _ => return Err(Error::InvalidInput) };
    let count = r.count(MAX_TOKENS)?;
    let tokens = r.take(count.checked_mul(4).ok_or(Error::Limit)?)?;
    let cache = r.blob(MAX_CACHE_BYTES)?; let sampler = r.blob(MAX_SAMPLER_BYTES)?;
    // Check all payload spans before allocating any original state vectors.
    if position != count as u64 { return Err(Error::Binding); }
    let tokens = tokens.chunks_exact(4).map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])).collect();
    let state = ActorState::new(profile, tokens, cache.to_vec(), sampler.to_vec(), position)?;
    Ok(FileStateUpdate { operation, expected_actor_revision, expected_authority_epoch, state })
}
pub(in super::super) fn write_reset(w: &mut Writer, request: &FileResetRequest) -> Result<(), Error> {
    request.validate()?;
    for value in [request.operation, request.expected_control_sequence, request.expected_actor_revision,
        request.expected_authority_epoch, request.binding.round, request.binding.reducer_generation] { w.u64(value)?; }
    w.raw(&request.binding.evidence_root)?;
    w.count(request.retained_targets.len())?;
    for target in &request.retained_targets { w.target(*target)?; }
    Ok(())
}
pub(in super::super) fn read_reset(r: &mut Reader<'_>) -> Result<FileResetRequest, Error> {
    let operation = r.u64()?; let expected_control_sequence = r.u64()?; let expected_actor_revision = r.u64()?;
    let expected_authority_epoch = r.u64()?; let round = r.u64()?; let reducer_generation = r.u64()?;
    let evidence_root = r.take(32)?.try_into().map_err(|_| Error::Incomplete)?;
    let count = r.count(MAX_ATTEMPTS)?;
    let mut retained_targets = Vec::new();
    retained_targets.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count { retained_targets.push(r.target()?); }
    let request = FileResetRequest { operation, expected_control_sequence, expected_actor_revision,
        expected_authority_epoch, binding: ReviewBinding { round, reducer_generation, evidence_root }, retained_targets };
    request.validate()?;
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn update() -> FileStateUpdate {
        FileStateUpdate { operation: 1, expected_actor_revision: 9, expected_authority_epoch: 10,
            state: ActorState::new(RestartProfile { id: 1, generation: 2, host_generation: 3,
                model_generation: 4, tokenizer_generation: 5, state_schema_generation: 6,
                grade: RestartGrade::ExactRestart }, vec![0, u32::MAX], vec![0, 128, 255], vec![255, 0], 2).unwrap() }
    }
    #[test]
    fn lossless_state_encoding_rejects_every_truncation_and_inconsistent_position() {
        let original = update(); let mut w = Writer::new(4096); write_update(&mut w, &original).unwrap();
        let encoded = w.finish(); let mut r = Reader::new(&encoded);
        assert_eq!(read_update(&mut r).unwrap(), original); r.end().unwrap();
        for end in 0..encoded.len() { assert!(read_update(&mut Reader::new(&encoded[..end])).is_err()); }
        let mut bad = encoded.clone(); bad[72..80].copy_from_slice(&3_u64.to_be_bytes());
        assert_eq!(read_update(&mut Reader::new(&bad)), Err(Error::Binding));
        let mut bad = encoded; bad[81..85].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(read_update(&mut Reader::new(&bad)), Err(Error::Limit));
    }
    #[test]
    fn reset_is_an_exact_operation_input_not_a_serialized_authority_receipt() {
        let original = FileResetRequest { operation: 1, expected_control_sequence: 4,
            expected_actor_revision: 5, expected_authority_epoch: 6,
            binding: ReviewBinding { round: 9, evidence_root: [42; 32], reducer_generation: 1 },
            retained_targets: Vec::new() };
        let mut w = Writer::new(4096); write_reset(&mut w, &original).unwrap(); let encoded = w.finish();
        let mut r = Reader::new(&encoded); assert_eq!(read_reset(&mut r).unwrap(), original); r.end().unwrap();
        for end in 0..encoded.len() { assert!(read_reset(&mut Reader::new(&encoded[..end])).is_err()); }
        let mut invalid = original; invalid.binding.evidence_root = [0; 32];
        assert_eq!(write_reset(&mut Writer::new(4096), &invalid), Err(Error::InvalidInput));
    }
}
