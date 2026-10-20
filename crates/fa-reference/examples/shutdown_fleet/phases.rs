//! Closed phase selection before profile intake or any option stripping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Phase { Admission, Drain }
impl Phase {
    pub(super) fn label(self) -> &'static str {
        match self { Self::Admission => "admission", Self::Drain => "drain" }
    }
}

pub(super) fn take_flag(args: &[String]) -> Result<Option<(Phase, Vec<String>)>, String> {
    if !args.iter().skip(2).any(|arg| matches!(arg.as_str(), "--admission-only" | "--drain-only")) {
        return Ok(None);
    }
    let mut clean = args[..2].to_vec();
    let mut selected = None; let mut index = 2;
    while index < args.len() {
        match args[index].as_str() {
            "--admission-only" | "--drain-only" => {
                if selected.is_some() { return Err("select exactly one fleet terminal phase".into()); }
                selected = Some(if args[index] == "--admission-only" { Phase::Admission } else { Phase::Drain });
                index += 1;
            }
            "--coordinator-anchor" | "--retain-coordinator-anchor" => {
                let value = args.get(index + 1).ok_or(super::USAGE)?;
                if value.is_empty() || value.starts_with("--") { return Err(super::USAGE.into()); }
                clean.extend_from_slice(&args[index..index + 2]); index += 2;
            }
            _ => return Err("terminal phases cannot be combined with other execution flags".into()),
        }
    }
    Ok(selected.map(|phase| (phase, clean)))
}

#[cfg(test)]
#[path = "phase_tests.rs"] mod tests;
