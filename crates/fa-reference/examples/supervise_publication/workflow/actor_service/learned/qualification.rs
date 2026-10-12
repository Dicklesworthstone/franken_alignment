//! Explicit held-out requalification before ORIGINAL learned continuation.
//! The capsule is independently supplied data, never actor input or a new key.
use super::{Config, Control, Deadline, ElapsedTick, FileHumanReviewer,
    FileSupervisedDriver, debug};
use std::path::Path;

pub(super) fn prepare<F>(driver: &mut FileSupervisedDriver, config: &Config,
    source: Option<&Path>, controls: (&mut Control, &FileHumanReviewer, &Deadline),
    time: &mut F) -> Result<bool, String>
where F: FnMut() -> ElapsedTick {
    let (control, reviewer, deadline) = controls;
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    {
        let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
        match source {
            // Reuse the original bounded reader, exact committee comparison,
            // native activation and rejection of historical permission retries.
            Some(source) => super::super::qualification::activate(&mut host, config, source)?,
            None => host.check_credibility().map_err(debug)?,
        }
    }
    // Acquisition and governance cannot lend their earlier tick to inference.
    // The following original generation/continuation/finish path must acquire
    // fresh policy evidence too. This helper does not resume the numerical owner,
    // renew a pending forecast, clear lost coverage or replenish spent work.
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    Ok(true)
}
