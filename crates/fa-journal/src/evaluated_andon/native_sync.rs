//! Synchronize the complete native broker cut, not a caller-selected receipt list.
//! The original Andon transitions and file writer remain the only accounting and
//! persistence implementations. This borrows the source; it does not recover the
//! native evaluation engine or make a compact journal an evaluator authenticator.

use super::{ImportFailure, ImportProgress, ImportStatus};
use crate::file::{Error, andon::FileAndonLedger};
use fa_reference::Error as ModelError;
use fa_reference::action::consequence::oversight::OversightBroker;
use fa_reference::action::consequence::oversight::credibility::native_andon::{
    NativeAndonBinding, NativeAndonSource,
};
use fa_reference::round::review::andon::AndonBudget;

/// Holds the complete original source immutable and the original file exclusive.
/// Windows are registered first; receipts follow actual label-acceptance order.
/// Counts include window registrations AND receipt calls, including exact retries.
/// A complete result means full accounting equality at this borrowed native cut,
/// not durable native labels, future labels, source authenticity or disk atomicity.
///
/// ```compile_fail,E0502
/// use fa_journal::evaluated_andon::native_sync::NativeAndonSync;
/// use fa_journal::file::andon::FileAndonLedger;
/// use fa_reference::action::consequence::oversight::OversightBroker;
/// use fa_reference::action::consequence::oversight::credibility::native_andon::NativeAndonBinding;
/// fn mutate_source(host: &mut OversightBroker, file: &mut FileAndonLedger, binding: &NativeAndonBinding) {
///     let sync = NativeAndonSync::begin(file, binding, host).unwrap();
///     host.revoke_epoch().unwrap();
///     sync.finish().unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct NativeAndonSync<'a> {
    journal: &'a mut FileAndonLedger,
    source: NativeAndonSource<'a>,
    windows: Vec<AndonBudget>,
    progress: ImportProgress,
}

impl<'a> NativeAndonSync<'a> {
    /// Semantic preflight is all-or-nothing and makes no file calls. A missing
    /// historical seed, extra target record, repriced window, wrong incident or
    /// changed source binding refuses before any new registration or charge.
    /// Supply the bootstrap binding independently, not from an untrusted source.
    pub fn begin(
        journal: &'a mut FileAndonLedger,
        binding: &NativeAndonBinding,
        host: &'a OversightBroker,
    ) -> Result<Self, Error> {
        let source = host.andon_source().map_err(crate::Error::Model)?;
        let ledger = journal.ledger()?;
        if source.binding() != binding || ledger.identity() != binding.identity() {
            return Err(crate::Error::Binding.into());
        }
        let mut windows = Vec::new();
        windows.try_reserve_exact(source.ledger().windows().len())
            .map_err(|_| crate::Error::Limit)?;
        windows.extend(source.ledger().windows().values().map(|window| window.budget));
        let mut candidate = ledger.clone();
        for budget in &windows {
            candidate.register_window(*budget).map_err(crate::Error::Model)?;
        }
        for receipt in source.receipts() {
            if receipt.binding() != binding
                || candidate.windows().get(&receipt.budget().window).map(|w| w.budget)
                    != Some(receipt.budget())
            {
                return Err(crate::Error::Binding.into());
            }
            candidate.record_trusted(receipt.confirmation()).map_err(crate::Error::Model)?;
        }
        // Counts alone are insufficient: incident opener, all contributors,
        // immutable budgets, exact confirmations and retained seed must agree.
        if &candidate != source.ledger() { return Err(crate::Error::Binding.into()); }
        let total = windows.len().checked_add(source.receipts().len()).ok_or(crate::Error::Limit)?;
        let progress = ImportProgress {
            total, completed: 0, newly_recorded: 0, already_recorded: 0,
            head: journal.acknowledged_head(),
            status: if total == 0 { ImportStatus::Complete } else { ImportStatus::Pending },
        };
        Ok(Self { journal, source, windows, progress })
    }

    pub fn source_revision(&self) -> u64 { self.source.revision() }
    pub fn window_count(&self) -> usize { self.windows.len() }
    pub fn receipt_count(&self) -> usize { self.source.receipts().len() }
    pub fn progress(&self) -> ImportProgress { self.progress }

    /// Each budget unit enters at most one original window/confirmation call.
    /// Synchronous storage may exceed an elapsed deadline. No writes on zero.
    pub fn advance(&mut self, max_records: usize) -> Result<ImportProgress, ImportFailure> {
        if self.progress.status == ImportStatus::Failed {
            return Err(self.failure(crate::Error::Model(ModelError::WrongState).into()));
        }
        for _ in 0..max_records.min(self.progress.total - self.progress.completed) {
            self.progress.status = ImportStatus::Failed; // includes caught unwind
            let index = self.progress.completed;
            let result = if index < self.windows.len() {
                self.journal.register_window(self.windows[index])
            } else {
                self.journal.record_trusted(
                    self.source.receipts()[index - self.windows.len()].confirmation(),
                ).map(|(_, head)| head)
            };
            let head = result.map_err(|error| self.failure(error))?;
            if head == self.progress.head { self.progress.already_recorded += 1; }
            else { self.progress.newly_recorded += 1; }
            self.progress.completed += 1;
            self.progress.head = head;
            if self.progress.completed == self.progress.total {
                let ledger = self.journal.ledger().map_err(|error| self.failure(error))?;
                if ledger != self.source.ledger() {
                    return Err(self.failure(crate::Error::Binding.into()));
                }
                self.progress.status = ImportStatus::Complete;
            } else { self.progress.status = ImportStatus::Pending; }
        }
        Ok(self.progress)
    }

    /// Drop or incomplete finish never refunds a published prefix. Reopen with
    /// the independently retained head and the same source before resuming.
    pub fn finish(self) -> Result<ImportProgress, ImportFailure> {
        if self.progress.status != ImportStatus::Complete {
            return Err(self.failure(crate::Error::Model(ModelError::Incomplete).into()));
        }
        Ok(self.progress)
    }

    fn failure(&self, error: Error) -> ImportFailure { ImportFailure { error, progress: self.progress } }
}
