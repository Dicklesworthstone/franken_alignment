//! Paired nonlinear attention calculations on capability-free KV branches.
//! Both legs use the identical captured query, contract, base and causal window.
//! No branch is promoted into captured evidence or a live model state.

use super::attention::{
    AttentionBudget, AttentionContract, AttentionRows, AttentionValues, AttentionWork,
    finite, prepare,
};
use super::experiment::{KvBranch, KvBranchBasis, KvCell, KvSide};
use super::super::TensorCapture;
use crate::Error;

/// Numerical intervention evidence, not native behavioral or safety evidence.
///
/// ```compile_fail,E0308
/// use fa_reference::action::consequence::activation::tensor::kv::attention_experiment::KvAttentionComparison;
/// use fa_reference::action::Permit;
/// fn authorize(comparison: KvAttentionComparison) -> Permit { comparison }
/// ```
#[derive(Clone, Debug)]
pub struct KvAttentionComparison {
    pub reference: KvBranchBasis,
    pub candidate: KvBranchBasis,
    pub query: TensorCapture,
    pub contract: AttentionContract,
    pub reference_values: AttentionValues,
    pub candidate_values: AttentionValues,
    pub work: AttentionWork,
}

impl KvAttentionComparison {
    pub fn changed_output_coordinates(&self) -> usize {
        self.reference_values.output().iter().zip(self.candidate_values.output())
            .filter(|(left, right)| left.to_bits() != right.to_bits()).count()
    }
    pub fn maximum_absolute_output_change(&self) -> f64 {
        self.reference_values.output().iter().zip(self.candidate_values.output())
            .map(|(left, right)| (left - right).abs()).fold(0.0, f64::max)
    }
}

impl KvBranch {
    /// A fixed-query attention intervention, not an autoregressive rollout.
    /// Full-prefix/window coverage and both legs' combined costs are checked
    /// before either leg reads cache scalars or allocates numerical output.
    pub fn compare_attention(
        &self, reference: &KvBranch, contract: &AttentionContract,
        query: &TensorCapture, budget: AttentionBudget,
    ) -> Result<KvAttentionComparison, Error> {
        self.check_common_base(reference)?;
        let reference_basis = reference.basis();
        let candidate_basis = self.basis();
        let plan = prepare(contract, query, &reference_basis.source)?;
        // bits() reads the immutable base then visits at most depth+1 nodes.
        // This accounts for sparse lookup depth instead of hiding it in a
        // scalar-product count. Map-key comparisons remain separately unmeasured.
        let left = plan.work(reference_basis.resolution_depth as u64 + 2)?;
        let right = plan.work(candidate_basis.resolution_depth as u64 + 2)?;
        let work = AttentionWork {
            scalar_products: left.scalar_products.checked_add(right.scalar_products).ok_or(Error::Overflow)?,
            exponentials: left.exponentials.checked_add(right.exponentials).ok_or(Error::Overflow)?,
            resolution_step_bound: left.resolution_step_bound.checked_add(right.resolution_step_bound).ok_or(Error::Overflow)?,
            workspace_bytes: left.workspace_bytes.checked_add(right.workspace_bytes).ok_or(Error::Overflow)?,
        };
        work.check(budget)?;
        let reference_values = plan.evaluate(reference)?;
        let candidate_values = plan.evaluate(self)?;
        Ok(KvAttentionComparison { reference: reference_basis, candidate: candidate_basis,
            query: query.clone(), contract: contract.clone(), reference_values, candidate_values, work })
    }
}

impl AttentionRows for KvBranch {
    fn scalar(&self, values: bool, position: u64, head: usize, channel: usize) -> Result<f64, Error> {
        let side = if values { KvSide::Value } else { KvSide::Key };
        finite(self.bits(KvCell { side, position, head, channel })?)
    }
}
