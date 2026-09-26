//! Explicit held-out activation before ORIGINAL numerical continuation.
//! No source, label, quorum or score is supplied by the actor connection.
use super::*;

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
            // Reuse the SAME strict file reader, independently bound committee,
            // native activation and historical-receipt rejection as actor intake.
            Some(source) => super::super::qualification::activate(&mut host, config, source)?,
            None => host.check_credibility().map_err(debug)?,
        }
    }
    // A slow file read or accepted governance transition cannot lend its earlier
    // tick to inference. The original generation/resume loop refreshes policy
    // evidence and samples time again; stop also gets its own checkpoint here.
    deadline.check(time())?;
    if control.checkpoint(driver, reviewer, deadline, time)? { return Ok(false); }
    Ok(true)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod routing_tests {
    use super::*;

    #[test]
    fn native_joint_activation_option_routes_open_modes_without_expanding_create_or_actor_roles() {
        for checked in [false, true] {
            let mode = if checked { "serve-open-checked" } else { "serve-open" };
            let mut args = [mode, "config", "actor", "reviewer"].map(str::to_owned).to_vec();
            if checked { args.push("witness".into()); }
            args.extend(["--native-text", "recipe", "--credibility-activation", "capsule"].map(str::to_owned));
            let source = crate::qualification::take_option(&mut args).unwrap();
            assert_eq!(source.as_deref(), Some(Path::new("capsule")));
            assert_eq!(args.last().map(String::as_str), Some("recipe"));
            assert_eq!(args.len(), if checked { 7 } else { 6 });
        }
        for mode in ["serve-create", "serve-create-checked", "actor-submit", "review-peer"] {
            let mut args = [mode, "config", "actor", "reviewer", "--native-text", "recipe",
                "--credibility-activation", "capsule"].map(str::to_owned).to_vec();
            let before = args.clone();
            assert!(crate::qualification::take_option(&mut args).is_err());
            assert_eq!(args, before);
        }
    }
}
