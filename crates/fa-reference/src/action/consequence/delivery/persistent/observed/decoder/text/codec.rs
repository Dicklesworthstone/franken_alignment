//! Exact raw-byte request encoding. No tokenizer or output is inferred from IDs.
use super::FileTextGenerationCommand;
use super::super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::activation::monitor::decoder::sampled::generation::{
    GenerationBudget, MAX_GENERATION_TOKENS, MAX_STOP_TOKENS,
    text::{TextGenerationRequest, MAX_PREFIX_CONTROLS},
    tokenizer::{TokenizationBudget, MAX_DECODE_BYTES, MAX_HEAP_POPS, MAX_INPUT_BYTES, MAX_PAIR_LOOKUPS},
};
use crate::Error;

pub(in super::super) fn write(w: &mut Writer, command: &FileTextGenerationCommand) -> Result<(), Error> {
    command.check()?;
    w.u64(command.id())?; w.u64(command.actor_revision())?; w.u64(command.position())?;
    let r = command.request();
    w.blob(&r.prompt)?;
    w.count(r.prefix_controls.len())?;
    for token in &r.prefix_controls { w.u32(*token)?; }
    w.count(r.max_new_tokens)?; w.count(r.stop_tokens.len())?;
    for token in &r.stop_tokens { w.u32(*token)?; }
    w.count(r.tokenization.input_bytes)?; w.count(r.tokenization.pair_lookups)?;
    w.count(r.tokenization.heap_pops)?;
    w.u64(r.generation.scalar_products)?; w.u64(r.generation.sampling_entries)?;
    w.count(r.max_output_bytes)
}

pub(in super::super) fn read(r: &mut Reader<'_>) -> Result<FileTextGenerationCommand, Error> {
    let id = r.u64()?; let revision = r.u64()?; let position = r.u64()?;
    let bytes = r.blob(MAX_INPUT_BYTES)?;
    let mut prompt = Vec::new();
    prompt.try_reserve_exact(bytes.len()).map_err(|_| Error::Limit)?;
    prompt.extend_from_slice(bytes);
    let prefix_controls = tokens(r, MAX_PREFIX_CONTROLS)?;
    let max_new_tokens = r.count(MAX_GENERATION_TOKENS)?;
    let stop_tokens = tokens(r, MAX_STOP_TOKENS)?;
    let tokenization = TokenizationBudget { input_bytes: r.count(MAX_INPUT_BYTES)?,
        pair_lookups: r.count(MAX_PAIR_LOOKUPS)?, heap_pops: r.count(MAX_HEAP_POPS)? };
    let generation = GenerationBudget { scalar_products: r.u64()?, sampling_entries: r.u64()? };
    let max_output_bytes = r.count(MAX_DECODE_BYTES)?;
    FileTextGenerationCommand::new(id, revision, position, TextGenerationRequest {
        prompt, prefix_controls, max_new_tokens, stop_tokens, tokenization, generation, max_output_bytes,
    })
}

fn tokens(r: &mut Reader<'_>, maximum: usize) -> Result<Vec<u32>, Error> {
    let count = r.count(maximum)?;
    let mut tokens = Vec::new(); tokens.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count { tokens.push(r.u32()?); }
    Ok(tokens)
}
