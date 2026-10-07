//! Checked accounting only; all numerical work remains in the original monitor.
use super::{Error, LearnedMonitorBudget, LearnedMonitorWork};

pub(super) fn scale_budget(budget: LearnedMonitorBudget, jobs: usize) -> Result<LearnedMonitorBudget, Error> {
    let jobs_u64 = u64::try_from(jobs).map_err(|_| Error::Overflow)?;
    Ok(LearnedMonitorBudget {
        encoded_bytes: budget.encoded_bytes.checked_mul(jobs).ok_or(Error::Overflow)?,
        probe_coordinates: budget.probe_coordinates.checked_mul(jobs).ok_or(Error::Overflow)?,
        reconstruction_products: budget.reconstruction_products.checked_mul(jobs_u64).ok_or(Error::Overflow)?,
        materialized_values: budget.materialized_values.checked_mul(jobs).ok_or(Error::Overflow)?,
        refinements: budget.refinements.checked_mul(jobs).ok_or(Error::Overflow)?,
    })
}

/// Subtract before intersecting: saturation would hide a violated lifetime cap.
/// The returned budget always satisfies the monitor's validated per-job ceiling.
pub(super) fn allowance(lifetime: LearnedMonitorBudget, used: LearnedMonitorWork,
    per_job: LearnedMonitorBudget) -> Result<LearnedMonitorBudget, Error>
{
    Ok(LearnedMonitorBudget {
        encoded_bytes: lifetime.encoded_bytes.checked_sub(used.encoded_bytes)
            .ok_or(Error::Binding)?.min(per_job.encoded_bytes),
        probe_coordinates: lifetime.probe_coordinates.checked_sub(used.probe_coordinates)
            .ok_or(Error::Binding)?.min(per_job.probe_coordinates),
        reconstruction_products: lifetime.reconstruction_products.checked_sub(used.reconstruction_products)
            .ok_or(Error::Binding)?.min(per_job.reconstruction_products),
        materialized_values: lifetime.materialized_values.checked_sub(used.materialized_values)
            .ok_or(Error::Binding)?.min(per_job.materialized_values),
        refinements: lifetime.refinements.checked_sub(used.refinements)
            .ok_or(Error::Binding)?.min(per_job.refinements),
    })
}

pub(super) fn add_work(a: LearnedMonitorWork, b: LearnedMonitorWork) -> Result<LearnedMonitorWork, Error> {
    Ok(LearnedMonitorWork {
        encoded_bytes: a.encoded_bytes.checked_add(b.encoded_bytes).ok_or(Error::Overflow)?,
        probe_coordinates: a.probe_coordinates.checked_add(b.probe_coordinates).ok_or(Error::Overflow)?,
        reconstruction_products: a.reconstruction_products.checked_add(b.reconstruction_products).ok_or(Error::Overflow)?,
        materialized_values: a.materialized_values.checked_add(b.materialized_values).ok_or(Error::Overflow)?,
        refinements: a.refinements.checked_add(b.refinements).ok_or(Error::Overflow)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_integer_counter_rejects_overflow_and_overdraw() {
        for field in 0..5 {
            let mut budget = LearnedMonitorBudget { encoded_bytes: 0, probe_coordinates: 0,
                reconstruction_products: 0, materialized_values: 0, refinements: 0 };
            let mut full = LearnedMonitorWork::default();
            let mut one = LearnedMonitorWork::default();
            match field {
                0 => { budget.encoded_bytes = usize::MAX; full.encoded_bytes = usize::MAX; one.encoded_bytes = 1; }
                1 => { budget.probe_coordinates = usize::MAX; full.probe_coordinates = usize::MAX; one.probe_coordinates = 1; }
                2 => { budget.reconstruction_products = u64::MAX; full.reconstruction_products = u64::MAX; one.reconstruction_products = 1; }
                3 => { budget.materialized_values = usize::MAX; full.materialized_values = usize::MAX; one.materialized_values = 1; }
                _ => { budget.refinements = usize::MAX; full.refinements = usize::MAX; one.refinements = 1; }
            }
            assert_eq!(add_work(full, one), Err(Error::Overflow));
            assert_eq!(scale_budget(budget, 2), Err(Error::Overflow));
            assert_eq!(scale_budget(budget, 1).unwrap(), budget);
            let zero = scale_budget(budget, 0).unwrap();
            assert_eq!(allowance(budget, full, budget).unwrap(), zero);
            assert_eq!(allowance(zero, one, budget), Err(Error::Binding));
        }
    }

    #[test]
    fn lifetime_remainder_never_enlarges_a_frozen_per_job_budget() {
        let per_job = LearnedMonitorBudget { encoded_bytes: 5, probe_coordinates: 7,
            reconstruction_products: 11, materialized_values: 13, refinements: 17 };
        let lifetime = scale_budget(per_job, 3).unwrap();
        assert_eq!(allowance(lifetime, LearnedMonitorWork::default(), per_job).unwrap(), per_job);
        let used = LearnedMonitorWork { encoded_bytes: 13, probe_coordinates: 18,
            reconstruction_products: 29, materialized_values: 34, refinements: 45 };
        assert_eq!(allowance(lifetime, used, per_job).unwrap(), LearnedMonitorBudget {
            encoded_bytes: 2, probe_coordinates: 3, reconstruction_products: 4,
            materialized_values: 5, refinements: 6,
        });
    }
}
