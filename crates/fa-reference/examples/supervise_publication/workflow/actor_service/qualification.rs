//! Activate original held-out evidence before live actor intake, never from it.
use super::{Config, FileOversight, debug};
use std::path::Path;

pub(super) fn activate(host: &mut FileOversight, config: &Config, path: &Path)
    -> Result<(), String>
{
    let request = crate::qualification::read(path)?;
    let epoch = host.inspect().control.ledger.epoch.checked_add(1)
        .ok_or("authority epoch exhausted")?;
    // No actor proposal exists yet. The native activation checks all governance
    // predecessors and evidence. The original actor gateway will independently
    // check the later, unchanged proposal against the resulting epoch/target.
    host.activate_credibility(host.revision(), request, &config.profile.committee).map_err(debug)?;
    host.check_credibility().map_err(debug)?;
    if host.inspect().control.ledger.epoch != epoch {
        return Err("historical activation cannot admit a new live request".into());
    }
    Ok(())
}
