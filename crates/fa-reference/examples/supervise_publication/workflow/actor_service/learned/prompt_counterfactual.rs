//! Asset-only source intervention through the original sealed numerical campaign.
//! No authority configuration, journal, live role or publication owner is opened.
#[path = "prompt_counterfactual/assets.rs"]
mod assets;
#[path = "prompt_counterfactual/plan.rs"]
mod plan;
#[path = "prompt_counterfactual/report.rs"]
mod report;

use super::debug;
use crate::config::read_regular;
use fa_reference::action::consequence::activation::tensor::kv::decoder::experiment::prompt::campaign::PromptCampaignStatus;
use std::{io::Write, path::Path, time::{Duration, Instant}};

const USAGE: &str = "compare-learned-prompt PLAN";
const DEADLINE: &str = "original prompt experiment deadline reached";

pub(super) fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    command_with_elapsed(args, output, &mut || started.elapsed())
}

/// The caller's single origin starts before plan intake. Each advance retains
/// the original campaign revision and spent work; no trial is replaced/retried.
pub(super) fn command_with_elapsed(args: &[String], output: &mut impl Write,
    elapsed: &mut impl FnMut() -> Duration) -> Result<(), String>
{
    if args.len() != 2 || args[0] != "compare-learned-prompt" {
        return Err(USAGE.into());
    }
    super::fields::path(&args[1])?;
    let bytes = read_regular(Path::new(&args[1]), plan::MAX_PLAN_BYTES)?;
    let plan = plan::Plan::decode(&bytes)?;
    let wall = Duration::from_millis(plan.runtime_ms);
    let mut checkpoint = || if elapsed() < wall { Ok(()) } else { Err(DEADLINE.to_owned()) };
    checkpoint()?;
    let mut loaded = assets::load(&plan, bytes.len(), &mut checkpoint)?;
    // No inference has started until the native fixed schedule exists. Expiry
    // here is still reported as a cancelled schedule with all seeds unstarted.
    let mut failure = None;
    let mut native_error = None;
    let mut late = false;
    let mut cancelled_at = None;
    while loaded.campaign.progress().status == PromptCampaignStatus::Active {
        let observed = elapsed();
        if observed >= wall {
            late = true;
            cancelled_at = Some(observed);
            let revision = loaded.campaign.progress().revision;
            if let Err(error) = loaded.campaign.cancel(revision) {
                native_error = Some(error);
                failure = Some(format!("original campaign cancellation failed: {error:?}"));
            }
            break;
        }
        let revision = loaded.campaign.progress().revision;
        if let Err(error) = loaded.campaign.advance(revision) {
            native_error = Some(error);
            failure = Some(format!("original prompt campaign failed: {error:?}"));
            break;
        }
    }
    let mut observed = elapsed();
    late |= observed >= wall;
    let mut bytes = report::encode(&plan, &loaded, report::Output {
        elapsed: observed, cancelled_at, deadline_reached: late, error: native_error })?;
    // Encoding is bounded but synchronous. Include an observed late return in
    // the emitted observation without dropping any already computed trial.
    observed = elapsed();
    if !late && observed >= wall {
        late = true;
        bytes = report::encode(&plan, &loaded, report::Output {
            elapsed: observed, cancelled_at, deadline_reached: true, error: native_error })?;
    }
    if let Err(error) = output.write_all(&bytes).and_then(|()| output.flush()) {
        let original = failure.as_deref().unwrap_or(if late { DEADLINE } else { "campaign result retained" });
        return Err(format!("{original}; prompt experiment output failed: {error}; no inference retry was sent"));
    }
    late |= elapsed() >= wall;
    match (failure, late) {
        (Some(error), true) => Err(format!("{error}; {DEADLINE}; partial numerical report emitted")),
        (Some(error), false) => Err(format!("{error}; partial numerical report emitted")),
        (None, true) => Err(format!("{DEADLINE}; retained numerical report emitted without retry")),
        (None, false) => Ok(()),
    }
}
