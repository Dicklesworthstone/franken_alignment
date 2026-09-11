//! Atomic-in-call restoration of the entire registered layer set.
//! Reuses the original checked scalar writer; there is no inference callback.

use super::{ModelKvDescriptor, ModelKvImage};
use super::super::restore::{KvDestination, KvRestoreReceipt, KvRestoreWindow, PreparedKvRestore};
use crate::Error;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub struct LayerRestore<'a> {
    pub destination: KvDestination<'a>,
    /// All layers restore the SAME source interval. Destination placement can
    /// differ by layer, but must preserve the declared absolute positions.
    pub window: KvRestoreWindow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerRestoreReceipt {
    pub layer: u64,
    pub restoration: KvRestoreReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelKvRestoreReceipt {
    pub source: ModelKvDescriptor,
    pub first_position: u64,
    pub token_count: usize,
    pub normalized_values: usize,
    pub bytes_written: usize,
    pub layers: Vec<LayerRestoreReceipt>,
}

/// All mutable buffers and write lists are retained before commit. Dropping
/// the plan writes nothing. Receipts contain data, never a live restart grant.
#[derive(Debug)]
#[must_use = "no layer is written until commit consumes the complete restore plan"]
pub struct PreparedModelKvRestore<'a> {
    plans: Vec<(u64, PreparedKvRestore<'a>)>,
    receipt: ModelKvRestoreReceipt,
    staged_bytes: usize,
}

impl PreparedModelKvRestore<'_> {
    /// Sum of the original writer's offset/scalar staging metric. Excludes
    /// vector capacity, source images, destination buffers and allocator data.
    pub fn staged_bytes(&self) -> usize { self.staged_bytes }

    pub fn commit(self) -> ModelKvRestoreReceipt {
        let mut receipt = self.receipt;
        for (layer, plan) in self.plans {
            receipt.layers.push(LayerRestoreReceipt { layer, restoration: plan.commit() });
        }
        receipt
    }
}

impl ModelKvImage {
    /// Requires exactly the registered layers. All Result-producing checks and
    /// allocations finish before any destination write. Source masks, sampler,
    /// RNG, tokens and serving state are outside this cache-only operation.
    pub fn prepare_restore<'a>(
        &self, destinations: BTreeMap<u64, LayerRestore<'a>>,
    ) -> Result<PreparedModelKvRestore<'a>, Error> {
        if !destinations.keys().eq(self.profile.layers().keys()) { return Err(Error::Binding); }
        let first = destinations.values().next().ok_or(Error::Incomplete)?.window;
        if first.token_count == 0 { return Err(Error::InvalidInput); }
        let mut object_ids = BTreeSet::new();
        for target in destinations.values() {
            if (target.window.first_position, target.window.token_count)
                != (first.first_position, first.token_count)
            { return Err(Error::Binding); }
            // Within one layer the existing Shared destination remains legal.
            // Across layers this profile requires disjoint declared objects as
            // well as the exclusive borrows enforced by safe Rust.
            match &target.destination {
                KvDestination::Separate { keys, values } => {
                    if !object_ids.insert(keys.identity.object) || !object_ids.insert(values.identity.object) {
                        return Err(Error::Binding);
                    }
                }
                KvDestination::Shared { identity, .. } => {
                    if !object_ids.insert(identity.object) { return Err(Error::Binding); }
                }
            }
        }
        let mut plans = Vec::new();
        plans.try_reserve_exact(destinations.len()).map_err(|_| Error::Limit)?;
        let mut receipts = Vec::new();
        receipts.try_reserve_exact(destinations.len()).map_err(|_| Error::Limit)?;
        let mut staged_bytes = 0_usize;
        for (id, target) in destinations {
            let plan = self.layer(id)?.prepare_restore(target.destination, target.window)?;
            staged_bytes = staged_bytes.checked_add(plan.staged_bytes()).ok_or(Error::Overflow)?;
            plans.push((id, plan));
        }
        let normalized_values = self.profile.values_per_token().checked_mul(first.token_count).ok_or(Error::Overflow)?;
        let bytes_written = self.profile.bytes_per_token().checked_mul(first.token_count).ok_or(Error::Overflow)?;
        let receipt = ModelKvRestoreReceipt {
            source: self.descriptor(), first_position: first.first_position, token_count: first.token_count,
            normalized_values, bytes_written, layers: receipts,
        };
        Ok(PreparedModelKvRestore { plans, receipt, staged_bytes })
    }
}
