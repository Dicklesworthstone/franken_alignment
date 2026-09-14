//! Structural dependencies of the ORIGINAL coefficient vector. No approximate
//! score, coefficient threshold or floating-point importance heuristic is used.
use super::{KvGroup, KvRow, LearnedKvView, LinearProbe};
use crate::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LearnedProbeDependency {
    pub group: KvGroup,
    /// Products removed from this probe's learned_work after this exact group
    /// is promoted. Counts nonzero coefficients, including subnormal weights.
    pub reconstruction_products: u64,
}

impl LinearProbe {
    /// Unrefined groups with a nonzero source-checked radius and at least one
    /// nonzero coefficient, in stored-head order. Missing residuals remain
    /// dependencies; the caller must not turn absence into evidence of quiet.
    /// Zero-radius groups still incur ordinary coarse reconstruction costs but
    /// cannot shrink this probe's interval through further promotion.
    pub fn learned_dependencies(&self, view: &LearnedKvView, row: KvRow)
        -> Result<Vec<LearnedProbeDependency>, Error>
    {
        let (_, channels) = self.check_learned(view, row)?;
        let rank = view.source().image().codec().policy().rank() as u64;
        let mut dependencies = Vec::new();
        for (head, weights) in self.weights.chunks_exact(channels).enumerate() {
            let nonzero = weights.iter().filter(|word| **word & 0x7fff_ffff != 0).count() as u64;
            let group = KvGroup { row, head };
            if nonzero == 0 || view.is_refined(group)? || view.source().radius(group)? == 0 { continue; }
            dependencies.try_reserve(1).map_err(|_| Error::Limit)?;
            dependencies.push(LearnedProbeDependency { group,
                reconstruction_products: nonzero.checked_mul(rank).ok_or(Error::Overflow)? });
        }
        Ok(dependencies)
    }
}
