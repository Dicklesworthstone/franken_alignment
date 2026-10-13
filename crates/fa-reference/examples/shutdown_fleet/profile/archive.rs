//! Consume the ORIGINAL retained plan, with independently supplied member profiles.
//! This is alternate input custody, not another shutdown or replay implementation.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::MAX_SHUTDOWN_PLAN_BYTES;

pub(super) fn load(spec: &Spec, archive: &Path, configurations: Vec<Config>,
    remaining: &mut usize, check: &mut impl FnMut() -> Result<(), String>)
    -> Result<FileShutdownPlan, String>
{
    check()?;
    let bytes = charged(archive, MAX_SHUTDOWN_PLAN_BYTES, remaining)?;
    check()?;
    let members = spec.members.iter().zip(configurations).map(|(member, config)| FileShutdownMember {
        id: member.id, directory: config.store, profile: config.profile,
    }).collect();
    // The original decoder checks the exact full roster, profiles, paths and
    // embedded canonical prefixes. It never opens a domain, acquires a role or
    // turns a retained registration into a successful current observation.
    let plan = FileShutdownPlan::decode(&bytes, members).map_err(err)?;
    if plan.operation() != spec.operation || plan.max_attempts() != spec.visits
        || plan.max_head_bytes() != spec.head_bytes {
        return Err("retained plan differs from independently selected operation or limits".into());
    }
    check()?;
    Ok(plan)
}

#[cfg(test)]
mod tests;
