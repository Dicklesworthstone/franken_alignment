//! Portable expectations for learned-audited generation. Importing bytes never
//! installs cache, RNG, counters, model weights, probes or weaker resource caps.
//! The caller supplies the complete intended recipe independently; every byte
//! of its V1 encoding must match before the original replay can be started.
mod binding;
mod state;
mod wire;
#[cfg(unix)]
pub mod file;

use super::{CheckpointLimits, GenerationCheckpoint, GenerationReplay, Recipe, ReplayableGeneration,
    ReplayBudget, ReplayReceipt, State, MAX_REPLAY_STATE_BYTES};
use crate::Error;
use std::fmt;
use std::rc::Rc;
use wire::{Reader, Writer};

const DOMAIN: &[u8; 8] = b"FALGA\0\0\x01";
pub const ARCHIVE_HEADER_BYTES: usize = 24;
pub const MAX_RECIPE_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_GENERATION_ARCHIVE_BYTES: usize = ARCHIVE_HEADER_BYTES + MAX_RECIPE_BYTES + MAX_REPLAY_STATE_BYTES;

/// Independent import/export ceilings, not run-budget refills or restart rights.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArchiveLimits {
    pub bytes: usize,
    pub recipe_bytes: usize,
    pub state: CheckpointLimits,
}
impl Default for ArchiveLimits {
    fn default() -> Self {
        Self { bytes: MAX_GENERATION_ARCHIVE_BYTES, recipe_bytes: MAX_RECIPE_BYTES,
            state: CheckpointLimits::default() }
    }
}
impl ArchiveLimits {
    fn check(self) -> Result<(), Error> {
        self.state.check()?;
        if self.bytes > MAX_GENERATION_ARCHIVE_BYTES || self.recipe_bytes > MAX_RECIPE_BYTES {
            return Err(Error::Limit);
        }
        Ok(())
    }
}

/// Parsed and recipe-matched EXPECTATIONS, not a trusted captured checkpoint.
/// No mutable owner or sampled output is exposed until original replay verifies
/// the whole state. IDs and equality are not file/host authentication.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::{
///     GenerationCheckpoint, archive::GenerationArchive,
/// };
/// fn trust(parsed: GenerationArchive) -> GenerationCheckpoint { parsed }
/// ```
/// ```compile_fail,E0599
/// use fa_reference::action::consequence::activation::tensor::kv::decoder::sampling::replay::archive::GenerationArchive;
/// fn skip(parsed: GenerationArchive) { parsed.generation_mut(); }
/// ```
pub struct GenerationArchive {
    recipe: Rc<Recipe>,
    expected: Rc<State>,
    encoded_bytes: usize,
    recipe_bytes: usize,
}
impl fmt::Debug for GenerationArchive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GenerationArchive").field("positions", &self.positions())
            .field("encoded_bytes", &self.encoded_bytes).finish_non_exhaustive()
    }
}
impl GenerationCheckpoint {
    /// Includes exact model/codebook/probe parameters and sampling state. Protect
    /// these sensitive bytes like the original checkpoint; this is not encryption.
    /// The complete size is admitted before allocating the export buffer.
    pub fn encode_archive(&self, limits: ArchiveLimits) -> Result<Vec<u8>, Error> {
        limits.check()?;
        let state_bytes = state::size(&self.expected)?;
        if self.positions() > limits.state.positions || state_bytes > limits.state.state_bytes {
            return Err(Error::Limit);
        }
        let mut count = Writer::count(limits.recipe_bytes);
        binding::write(&mut count, &self.recipe)?;
        let recipe_bytes = count.len();
        let total = ARCHIVE_HEADER_BYTES.checked_add(recipe_bytes)
            .and_then(|n| n.checked_add(state_bytes)).ok_or(Error::Limit)?;
        if total > limits.bytes { return Err(Error::Limit); }
        let mut w = Writer::collect(total)?;
        w.bytes(DOMAIN)?; w.size(recipe_bytes)?; w.size(state_bytes)?;
        binding::write(&mut w, &self.recipe)?;
        state::write(&mut w, &self.expected)?;
        w.finish()
    }
}
impl GenerationArchive {
    /// The intended recipe is taken from an independently constructed original
    /// owner, NOT from this file. Reading does not advance or reset that owner,
    /// even if it is currently held. Exact recipe matching also protects empty
    /// prefixes and unused weights whose mismatch a prefix replay cannot reveal.
    pub fn decode(bytes: &[u8], intended: &ReplayableGeneration, limits: ArchiveLimits) -> Result<Self, Error> {
        limits.check()?;
        if bytes.len() > limits.bytes { return Err(Error::Limit); }
        let mut r = Reader::new(bytes);
        if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::InvalidInput); }
        let recipe_bytes = r.count(limits.recipe_bytes)?;
        let state_bytes = r.count(limits.state.state_bytes)?;
        let total = ARCHIVE_HEADER_BYTES.checked_add(recipe_bytes)
            .and_then(|n| n.checked_add(state_bytes)).ok_or(Error::Limit)?;
        if bytes.len() != total { return Err(Error::Binding); }
        let supplied = r.take(recipe_bytes)?;
        let mut comparison = Writer::compare(supplied);
        binding::write(&mut comparison, &intended.recipe)?;
        if comparison.len() != supplied.len() { return Err(Error::Binding); }
        let expected = state::read(r.take(state_bytes)?, &intended.recipe, limits)?;
        r.end()?;
        Ok(Self { recipe: Rc::clone(&intended.recipe), expected: Rc::new(expected),
            encoded_bytes: bytes.len(), recipe_bytes })
    }
    pub fn positions(&self) -> usize { self.expected.tokens.len() }
    pub fn encoded_bytes(&self) -> usize { self.encoded_bytes }
    pub fn recipe_bytes(&self) -> usize { self.recipe_bytes }
    pub fn state_bytes(&self) -> usize { self.expected.logical_bytes }

    /// Delegate to the original verifier. Saved values remain comparisons, not
    /// assignment inputs, and every prompt/sample is learned-audited again.
    pub fn begin_replay(&self, budget: ReplayBudget) -> Result<GenerationReplay, Error> {
        GenerationCheckpoint { recipe: Rc::clone(&self.recipe), expected: Rc::clone(&self.expected) }
            .begin_replay(budget)
    }
    pub fn replay(&self, budget: ReplayBudget) -> Result<(ReplayableGeneration, ReplayReceipt), Error> {
        let mut replay = self.begin_replay(budget)?;
        replay.advance(self.positions())?;
        replay.finish()
    }
}
