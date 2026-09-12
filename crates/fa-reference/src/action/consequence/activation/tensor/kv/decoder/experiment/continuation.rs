//! One private suffix owner shared by sparse interventions and lossy baselines.
//! All numerical forward work still runs through DecoderModel::forward_token.
use super::super::{ComputedLayers, DecoderBudget, DecoderHistory, DecoderModel, DecoderWork};
use super::super::super::attention;
use super::super::super::experiment::{KvCell, KvSide};
use crate::Error;
use std::rc::Rc;

pub(super) trait Prefix {
    fn model(&self) -> &DecoderModel;
    fn stream(&self) -> u64;
    fn len(&self) -> usize;
    fn bits(&self, layer: u64, cell: KvCell) -> Result<u32, Error>;
}
#[derive(Default)]
pub(super) struct Continuation {
    appended: Vec<ComputedLayers>,
    tokens: Vec<u32>,
    logits: Option<Rc<[f32]>>,
    work: DecoderWork,
}
pub(super) struct Step {
    pub token: u32,
    pub position: u64,
    pub logits: Rc<[f32]>,
    pub work: DecoderWork,
}
impl Continuation {
    pub fn position(&self, prefix: &dyn Prefix) -> u64 { (prefix.len() + self.tokens.len()) as u64 }
    pub fn tokens(&self) -> &[u32] { &self.tokens }
    pub fn work(&self) -> DecoderWork { self.work }
    pub fn logits(&self) -> Result<&[f32], Error> { self.logits.as_deref().ok_or(Error::Incomplete) }
    pub fn greedy_token(&self) -> Result<u32, Error> {
        let logits = self.logits()?;
        let mut selected = 0;
        for index in 1..logits.len() { if logits[index] > logits[selected] { selected = index; } }
        Ok(selected as u32)
    }
    pub fn advance(&mut self, prefix: &dyn Prefix, expected_position: u64, token: u32,
        budget: DecoderBudget) -> Result<Step, Error>
    {
        let position = self.position(prefix);
        if expected_position != position { return Err(Error::Stale); }
        let model = prefix.model();
        if token as usize >= model.profile().shape().vocabulary { return Err(Error::InvalidInput); }
        let work = model.estimate(position as usize, 1)?;
        work.check(budget)?;
        let next_work = self.work.add(work)?;
        let history = History { prefix, suffix: self };
        let computed = model.forward_token(&history, prefix.stream(), position, token, false)?;
        self.tokens.try_reserve(1).map_err(|_| Error::Limit)?;
        self.appended.try_reserve(1).map_err(|_| Error::Limit)?;
        let step = Step { token, position, logits: Rc::clone(&computed.logits), work };
        self.appended.push(computed.staged);
        self.tokens.push(token);
        self.logits = Some(computed.logits);
        self.work = next_work;
        Ok(step)
    }
    pub fn bits(&self, prefix: &dyn Prefix, layer: u64, cell: KvCell) -> Result<u32, Error> {
        let profile = prefix.model().profile(); let shape = profile.shape();
        if layer == 0 || layer > shape.layers as u64 || cell.head >= shape.cache_heads
            || cell.channel >= profile.head_width() { return Err(Error::InvalidInput); }
        if cell.position >= self.position(prefix) { return Err(Error::Missing); }
        if cell.position < prefix.len() as u64 { return prefix.bits(layer, cell); }
        let index = cell.head * profile.head_width() + cell.channel;
        let row = self.appended.get((cell.position - prefix.len() as u64) as usize)
            .and_then(|layers| layers.get(layer as usize - 1)).ok_or(Error::Missing)?;
        if row.0 != layer { return Err(Error::Binding); }
        let bytes = match cell.side { KvSide::Key => &row.1, KvSide::Value => &row.2 };
        let word: [u8; 4] = bytes.get(index * 4..index * 4 + 4).ok_or(Error::Missing)?
            .try_into().map_err(|_| Error::Binding)?;
        Ok(u32::from_le_bytes(word))
    }
}
struct History<'a> { prefix: &'a dyn Prefix, suffix: &'a Continuation }
impl DecoderHistory for History<'_> {
    fn scalar(&self, layer: u64, values: bool, position: u64, head: usize, channel: usize) -> Result<f64, Error> {
        attention::finite(self.suffix.bits(self.prefix, layer, KvCell {
            side: if values { KvSide::Value } else { KvSide::Key }, position, head, channel,
        })?)
    }
}
