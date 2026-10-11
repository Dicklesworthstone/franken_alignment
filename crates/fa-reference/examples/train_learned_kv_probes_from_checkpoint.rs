//! Actual checkpoint K/V -> complete fitted/evaluated roster -> bound monitor.
//! The diagnostic report survives statistical rejection; no output grants rights.
#![forbid(unsafe_code)]

#[path = "train_learned_kv_probes_from_checkpoint/assets.rs"]
mod assets;
#[path = "train_learned_kv_probes_from_checkpoint/plan.rs"]
mod plan;
#[path = "train_learned_kv_probes_from_checkpoint/report.rs"]
mod report;
// Reuse the learned operator's exact binding schema/parser, not a second codec
// interchange. Its unused recipe helpers remain private to the included module.
#[path = "supervise_publication/workflow/actor_service/learned/fields.rs"]
#[allow(dead_code)]
mod fields;
#[path = "supervise_publication/workflow/actor_service/learned/fit.rs"]
#[allow(dead_code)]
mod fit;
mod config {
    pub(crate) use super::assets::read_regular;
    pub(crate) fn debug(error: impl std::fmt::Debug) -> String { format!("{error:?}") }
}

use assets::{Inputs, SavedReport};
use fa_reference::action::consequence::activation::probe::training::decoder::kv::KvDecoderCampaign;
use fa_reference::Error;
use plan::Plan;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

type Failure = Box<dyn std::error::Error>;
const USAGE: &str = "train_learned_kv_probes_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS TOKENIZER CAMPAIGN_JSON CODEC_BINDING_JSON CODEC_ARCHIVE NEW_REPORT_NDJSON NEW_MONITOR_ARTIFACT";
fn invalid(message: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, message.into()) }
fn contract(error: Error) -> io::Error { invalid(format!("campaign contract: {error:?}")) }

/// One run keeps its original mutable capture/fit allowances. Neither reporting
/// nor an export error constructs a fresh allowance or retries numerical work.
fn execute(plan: &mut Plan, inputs: Inputs, report_path: &Path, monitor_path: &Path,
    output: &mut impl Write) -> Result<bool, Failure>
{
    if report_path == monitor_path { return Err(invalid("report and monitor need distinct new paths").into()); }
    assets::require_absent(report_path)?;
    assets::require_absent(monitor_path)?;
    let mut saved = SavedReport::new(report_path, plan.report_bytes)?;
    report::start(&mut saved, plan, &inputs)?;
    saved.sync()?;
    let mut stage = "codec_replay";
    let result = (|| -> Result<bool, Failure> {
        let (codec, _checkpoint) = inputs.archive.replay(inputs.binding.budget).map_err(contract)?;
        report::codec(&mut saved, &codec)?;
        stage = "capture_and_training";
        let campaign = plan.run(&inputs.model).map_err(contract)?;
        report::capture_completed(&mut saved, plan)?;
        report::campaign(&mut saved, &campaign)?;
        saved.sync()?;
        if !campaign.accepted() {
            publish_status(output, &campaign, saved.bytes(), false)?;
            return Ok(false);
        }
        stage = "monitor_serialization";
        let artifact = campaign.monitor_artifact(&inputs.tokenizer, &codec, &plan.settings,
            plan.monitor_bytes).map_err(contract)?;
        writeln!(saved, "{{\"kind\":\"monitor_ready\",\"format\":\"FAKVMON2\",\"bytes\":{},\"permission\":\"not_issued\"}}", artifact.len())?;
        saved.sync()?;
        // Persisted results and a working output sink precede monitor creation.
        // Later I/O failure may retain the new artifact, never delete/refit it.
        publish_status(output, &campaign, saved.bytes(), true)?;
        stage = "monitor_write";
        assets::write_new(monitor_path, &artifact)?;
        writeln!(saved, "{{\"kind\":\"monitor_saved\",\"format\":\"FAKVMON2\",\"bytes\":{},\"permission\":\"not_issued\"}}", artifact.len())?;
        saved.sync()?;
        writeln!(output, "{{\"kind\":\"monitor_saved\",\"bytes\":{},\"permission\":\"not_issued\"}}", artifact.len())?;
        output.flush()?;
        Ok(true)
    })();
    if let Err(error) = &result {
        // A broken report file cannot be claimed durable. Preserve the original
        // failure and explicitly surface any additional diagnostic-write error.
        if let Err(report_error) = report::failure(&mut saved, stage, error.as_ref(), plan).and_then(|()| saved.sync()) {
            return Err(invalid(format!("{error}; diagnostic persistence failed: {report_error}")).into());
        }
    }
    result
}

fn publish_status(output: &mut impl Write, campaign: &KvDecoderCampaign, bytes: usize,
    export_ready: bool) -> io::Result<()>
{
    writeln!(output, "{{\"kind\":\"campaign_complete\",\"taps\":{},\"accepted\":{},\"report_bytes\":{bytes},\"export_ready\":{export_ready},\"permission\":\"not_issued\"}}",
        campaign.taps().len(), campaign.accepted())?;
    output.flush()
}

fn run(args: &[OsString], output: &mut impl Write) -> Result<bool, Failure> {
    if args.len() != 8 { return Err(invalid(USAGE).into()); }
    let (mut plan, inputs) = assets::load(args)?;
    execute(&mut plan, inputs, Path::new(&args[6]), Path::new(&args[7]), output)
}

/// Test-only bridge for the independent codec command's actual file pipeline.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn run_for_codec_test(args: &[OsString], output: &mut impl Write) -> Result<bool, Failure> {
    run(args, output)
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).take(9).collect();
    match run(&args, &mut io::stdout().lock()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(2),
        Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
    }
}

#[cfg(test)]
#[path = "train_learned_kv_probes_from_checkpoint/tests.rs"]
mod tests;
