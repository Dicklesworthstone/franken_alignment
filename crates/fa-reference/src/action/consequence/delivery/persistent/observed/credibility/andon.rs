//! Native Andon activation and windows in the ORIGINAL whole-input journal.
//! No compact accounting seed or serialized score can replace source replay.
use super::CredibilityEvent;
use super::super::{Event, FileOversight, JournalError};
use super::super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::oversight::credibility::native_andon::{
    NativeAndonBinding, NativeAndonSource, NativeEvaluatedFalseHold,
};
use crate::round::review::andon::{AndonBudget, AndonLedger, LedgerIdentity, MAX_WINDOWS};
use crate::Error;

/// Independently retained bootstrap input. Windows are canonicalized by ID;
/// their actual overlap/identity/limit semantics belong to the original ledger.
/// This creates NEW accounting. Continue an existing deployment by recovering
/// its original journal, not by enabling an empty replacement owner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAndonConfig {
    identity: LedgerIdentity,
    windows: Vec<AndonBudget>,
}
impl FileAndonConfig {
    pub fn new(identity: LedgerIdentity, mut windows: Vec<AndonBudget>) -> Result<Self, Error> {
        if windows.len() > MAX_WINDOWS { return Err(Error::Limit); }
        if windows.is_empty() { return Err(Error::Incomplete); }
        let mut ledger = AndonLedger::new(identity)?;
        for window in &windows {
            if !ledger.register_window(*window)? { return Err(Error::Duplicate); }
        }
        windows.sort_by_key(|window| window.window);
        Ok(Self { identity, windows })
    }
    pub fn identity(&self) -> LedgerIdentity { self.identity }
    pub fn windows(&self) -> &[AndonBudget] { &self.windows }
    pub(in super::super) fn accounting(&self) -> Result<AndonLedger, Error> {
        let mut ledger = AndonLedger::new(self.identity)?;
        for window in &self.windows { ledger.register_window(*window)?; }
        Ok(ledger)
    }
}

impl FileOversight {
    /// Activate before any proposal/request/review. The original transaction
    /// publishes the enable event BEFORE returning a binding. Exact retries do
    /// not write; changed bootstrap inputs cannot replace or reset accounting.
    pub fn enable_andon(&mut self, revision: u64, config: &FileAndonConfig)
        -> Result<NativeAndonBinding, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        if let Some(existing) = self.machine.andon_contract() {
            if existing != config { return Err(Error::Binding.into()); }
            return Ok(self.machine.broker.andon_binding()?.clone());
        }
        self.transact(revision, Event::Credibility(CredibilityEvent::EnableAndon(config.clone())))?;
        Ok(self.machine.broker.andon_binding()?.clone())
    }

    /// Persist the original immutable window transition. New windows may cover
    /// late labels, but cannot change the price/allowance of an existing window.
    pub fn register_andon_window(&mut self, revision: u64, budget: AndonBudget)
        -> Result<bool, JournalError>
    {
        let ledger = self.andon_ledger()?;
        if let Some(existing) = ledger.windows().get(&budget.window) {
            return if existing.budget == budget { Ok(false) } else { Err(Error::Binding.into()) };
        }
        self.transact(revision, Event::Credibility(CredibilityEvent::AndonWindow(budget)))?;
        Ok(true)
    }
    pub fn andon_config(&self) -> Result<Option<&FileAndonConfig>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.andon_contract())
    }
    pub fn andon_binding(&self) -> Result<&NativeAndonBinding, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.andon_binding()?)
    }
    pub fn andon_ledger(&self) -> Result<&AndonLedger, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.andon_ledger()?)
    }
    pub fn andon_receipt(&self, round: u64) -> Result<&NativeEvaluatedFalseHold, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.andon_receipt(round)?)
    }
    /// Complete native evidence from the acknowledged owner, not a selected
    /// receipt list. Borrowing this source grants no mutable broker access.
    pub fn andon_source(&self) -> Result<NativeAndonSource<'_>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.andon_source()?)
    }
}

pub(in super::super) fn write_config(w: &mut Writer, config: &FileAndonConfig) -> Result<(), Error> {
    w.u64(config.identity.deployment)?;
    w.u64(config.identity.evaluator)?;
    w.count(config.windows.len())?;
    for window in &config.windows { write_window(w, *window)?; }
    Ok(())
}
pub(in super::super) fn read_config(r: &mut Reader<'_>) -> Result<FileAndonConfig, Error> {
    let identity = LedgerIdentity { deployment: r.u64()?, evaluator: r.u64()? };
    let count = r.count(MAX_WINDOWS)?;
    let mut windows = Vec::with_capacity(count);
    let mut previous = None;
    for _ in 0..count {
        let window = read_window(r)?;
        if previous.is_some_and(|id| id >= window.window) { return Err(Error::InvalidInput); }
        previous = Some(window.window);
        windows.push(window);
    }
    FileAndonConfig::new(identity, windows)
}
pub(in super::super) fn write_window(w: &mut Writer, budget: AndonBudget) -> Result<(), Error> {
    for value in [budget.window, budget.start, budget.end, budget.false_stops_allowed, budget.cost_per_stop] {
        w.u64(value)?;
    }
    Ok(())
}
pub(in super::super) fn read_window(r: &mut Reader<'_>) -> Result<AndonBudget, Error> {
    let budget = AndonBudget { window: r.u64()?, start: r.u64()?, end: r.u64()?,
        false_stops_allowed: r.u64()?, cost_per_stop: r.u64()? };
    if budget.window == 0 || budget.start >= budget.end { return Err(Error::InvalidInput); }
    Ok(budget)
}

#[cfg(test)]
pub(in super::super) mod tests;
