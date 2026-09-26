//! New numerical-input family; all pre-existing journal encodings stay intact.
use super::{DecoderEvent, FileDecoderConfig, StepRequest, MAX_WITNESS_BYTES};
use super::super::super::codec::shared::{Reader, Writer};
use crate::Error;
use crate::action::consequence::activation::tensor::kv::decoder::safetensors::OutputHead;
use crate::action::consequence::oversight::decoder_host::HostedStopPolicy;
use std::rc::Rc;

pub(in super::super) fn write(w: &mut Writer, event: &DecoderEvent) -> Result<(), Error> {
    match event {
        DecoderEvent::Tokenizer(bytes) => {
            if bytes.is_empty() { return Err(Error::Incomplete); }
            if bytes.len() > super::text::MAX_FILE_TOKENIZER_BYTES { return Err(Error::Limit); }
            w.u8(9)?; w.blob(bytes)?;
        }
        DecoderEvent::CancelGeneration { id, revision } => {
            super::progress::check_progress_key(*id, *revision)?;
            w.u8(11)?; w.u64(*id)?; w.u64(*revision)?;
        }
        DecoderEvent::TextIntent(command) => { w.u8(10)?; super::text::write(w, command)?; }
        DecoderEvent::Enable(config) => {
            // Existing single-file tags/bodies remain byte-identical. Sharded
            // sources use distinct tags; old readers refuse, never concatenate.
            w.u8(match (config.is_sharded(), config.output_head()) {
                (false, OutputHead::Independent) => 0,
                (false, OutputHead::TiedEmbeddings) => 12,
                (true, OutputHead::Independent) => 13,
                (true, OutputHead::TiedEmbeddings) => 14,
            })?;
            config.write(w)?;
        }
        DecoderEvent::StopPolicy(policy) => {
            w.u8(5)?; w.u64(policy.id())?; w.u64(policy.generation())?; w.u64(policy.operation())?;
        }
        DecoderEvent::Step(request, witness) => {
            if witness.is_empty() { return Err(Error::Incomplete); }
            if witness.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
            match request {
                StepRequest::Forced { revision, position, token, products } => {
                    w.u8(1)?; w.u64(*revision)?; w.u64(*position)?; w.u32(*token)?; w.u64(*products)?;
                }
                StepRequest::Sampled { revision, position, products, vocabulary } => {
                    w.u8(2)?; w.u64(*revision)?; w.u64(*position)?; w.u64(*products)?; w.count(*vocabulary)?;
                }
            }
            w.blob(witness)?;
        }
        DecoderEvent::Resume { revision, position } => { w.u8(3)?; w.u64(*revision)?; w.u64(*position)?; }
        DecoderEvent::Checkpoint(request, witness) => {
            if witness.is_empty() { return Err(Error::Incomplete); }
            if witness.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
            w.u8(4)?; super::checkpoint::write_request(w, request)?; w.blob(witness)?;
        }
        DecoderEvent::Generate(command, witness) => {
            if witness.is_empty() { return Err(Error::Incomplete); }
            if witness.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
            w.u8(6)?; super::generation::write_command(w, command)?; w.blob(witness)?;
        }
        DecoderEvent::BeginGeneration(command) => {
            w.u8(7)?; super::generation::write_command(w, command)?;
        }
        DecoderEvent::AdvanceGeneration { id, revision, witness } => {
            super::progress::check_progress_key(*id, *revision)?;
            if witness.is_empty() { return Err(Error::Incomplete); }
            if witness.len() > MAX_WITNESS_BYTES { return Err(Error::Limit); }
            w.u8(8)?; w.u64(*id)?; w.u64(*revision)?; w.blob(witness)?;
        }
    }
    Ok(())
}
pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<DecoderEvent, Error> {
    let tag = r.u8()?;
    Ok(match tag {
        0 => DecoderEvent::Enable(Rc::new(FileDecoderConfig::read(r)?)),
        12 => DecoderEvent::Enable(Rc::new(FileDecoderConfig::read_with_output_head(r,
            OutputHead::TiedEmbeddings)?)),
        13 | 14 => DecoderEvent::Enable(Rc::new(FileDecoderConfig::read_sharded(r,
            if tag == 13 { OutputHead::Independent } else { OutputHead::TiedEmbeddings })?)),
        1 | 2 => {
            let revision = r.u64()?; let position = r.u64()?;
            let request = if tag == 1 {
                StepRequest::Forced { revision, position, token: r.u32()?, products: r.u64()? }
            } else { StepRequest::Sampled { revision, position, products: r.u64()?, vocabulary: r.count(usize::MAX)? } };
            let witness = r.blob(MAX_WITNESS_BYTES)?;
            if witness.is_empty() { return Err(Error::Incomplete); }
            DecoderEvent::Step(request, Rc::from(witness))
        }
        3 => DecoderEvent::Resume { revision: r.u64()?, position: r.u64()? },
        4 => {
            let request = super::checkpoint::read_request(r)?;
            let witness = r.blob(MAX_WITNESS_BYTES)?;
            if witness.is_empty() { return Err(Error::Incomplete); }
            DecoderEvent::Checkpoint(request, Rc::from(witness))
        }
        5 => DecoderEvent::StopPolicy(HostedStopPolicy::new(r.u64()?, r.u64()?, r.u64()?)?),
        6 => {
            let command = super::generation::read_command(r)?;
            let witness = r.blob(MAX_WITNESS_BYTES)?;
            if witness.is_empty() { return Err(Error::Incomplete); }
            DecoderEvent::Generate(Rc::new(command), Rc::from(witness))
        }
        7 => DecoderEvent::BeginGeneration(Rc::new(super::generation::read_command(r)?)),
        8 => {
            let id = r.u64()?; let revision = r.u64()?;
            super::progress::check_progress_key(id, revision)?;
            let witness = r.blob(MAX_WITNESS_BYTES)?;
            if witness.is_empty() { return Err(Error::Incomplete); }
            DecoderEvent::AdvanceGeneration { id, revision, witness: Rc::from(witness) }
        }
        9 => {
            let bytes = r.blob(super::text::MAX_FILE_TOKENIZER_BYTES)?;
            if bytes.is_empty() { return Err(Error::Incomplete); }
            DecoderEvent::Tokenizer(Rc::from(bytes))
        }
        10 => DecoderEvent::TextIntent(Rc::new(super::text::read(r)?)),
        11 => {
            let id = r.u64()?; let revision = r.u64()?;
            super::progress::check_progress_key(id, revision)?;
            DecoderEvent::CancelGeneration { id, revision }
        }
        _ => return Err(Error::InvalidInput),
    })
}
