//! Explicit new work for named domains, before phase or custody option stripping.
use super::*;
use fa_reference::action::consequence::delivery::persistent::observed::shutdown::MAX_SHUTDOWN_DOMAINS;

pub(super) fn requested(args: &[String]) -> bool {
    args.iter().skip(2).any(|arg| arg == "--recover-domain")
}

struct Selection { args: Vec<String>, domains: Vec<u64>, phase: Option<phases::Phase> }

fn parse(args: &[String]) -> Result<Selection, String> {
    if args.len() < 4 || args.len() > 2 + 2 * MAX_SHUTDOWN_DOMAINS + 5
        || args[0] != "recover" { return Err("--recover-domain requires recover PROFILE and a bounded explicit selection".into()); }
    let mut clean = args[..2].to_vec(); let mut domains = Vec::new(); let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--recover-domain" => {
                let value = args.get(index + 1).ok_or(USAGE)?;
                if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) { return Err(USAGE.into()); }
                let id = value.parse::<u64>().map_err(|_| "invalid selected domain")?;
                if id == 0 { return Err("selected domain must be nonzero".into()); }
                if domains.contains(&id) { return Err("duplicate selected domain".into()); }
                if domains.len() == MAX_SHUTDOWN_DOMAINS { return Err("too many selected domains".into()); }
                domains.try_reserve(1).map_err(|_| "selection storage unavailable")?;
                domains.push(id); index += 2;
            }
            "--coordinator-anchor" | "--retain-coordinator-anchor" => {
                let value = args.get(index + 1).ok_or(USAGE)?;
                if value.is_empty() || value.starts_with("--") { return Err(USAGE.into()); }
                clean.extend_from_slice(&args[index..index + 2]); index += 2;
            }
            "--terminal-clocks" | "--admission-only" | "--drain-only" => {
                clean.push(args[index].clone()); index += 1;
            }
            _ => return Err(USAGE.into()),
        }
    }
    if domains.is_empty() { return Err("at least one selected domain is required".into()); }
    domains.sort_unstable();
    let (phase, clean) = match phases::take_flag(&clean)? {
        Some((phase, clean)) => (Some(phase), clean),
        None => (None, terminal::take_flag(&clean)?.unwrap_or(clean)),
    };
    // Validate custody duplicates/closed options now, before reading the profile.
    let _ = Options::parse(Mode::Recover, &clean[2..])?;
    Ok(Selection { args: clean, domains, phase })
}

pub(super) fn check_plan(plan: &FileShutdownPlan, domains: &[u64]) -> Result<(), String> {
    if domains.is_empty() || domains.len() > plan.domains().len() { return Err("invalid selected domain count".into()); }
    for (index, id) in domains.iter().enumerate() {
        if domains[..index].contains(id) { return Err("duplicate selected domain".into()); }
        if !plan.domains().iter().any(|domain| domain.id() == *id) { return Err("selected domain is not registered".into()); }
    }
    Ok(())
}

pub(super) fn command_with(args: &[String], clock: impl FnMut() -> Result<ElapsedTick, String>,
    elapsed: &mut impl FnMut() -> Duration, out: &mut impl Write) -> Result<(), String> {
    let selection = parse(args)?;
    terminal::command_for_selection(&selection.args, clock, elapsed, out,
        selection.phase, Some(&selection.domains))
}

#[cfg(test)]
#[path = "selected_tests.rs"] mod tests;
