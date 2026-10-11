//! Actual checkpoint tokens -> original codec owner -> original fitting inputs.
//! No supplied K/V, fitted coefficients, detector labels or authority are accepted.
#![forbid(unsafe_code)]

#[path = "train_learned_kv_codec_from_checkpoint/assets.rs"]
mod assets;
#[path = "train_learned_kv_codec_from_checkpoint/plan.rs"]
mod plan;
#[path = "train_learned_kv_codec_from_checkpoint/report.rs"]
mod report;
#[path = "train_learned_kv_codec_from_checkpoint/binding.rs"]
mod binding;
// The same closed JSON fields and unchanged independently retained binding
// parser used by both existing learned operator consumers.
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

use assets::{Inputs, OutputPaths, SavedReport};
use fa_reference::action::consequence::activation::tensor::kv::model::learned::replay::training::{
    CodecTraining, CodecTrainingStatus as Status,
};
use fa_reference::Error;
use plan::Plan;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

type Failure = Box<dyn std::error::Error>;
const USAGE: &str = "train_learned_kv_codec_from_checkpoint CONFIG_JSON WEIGHTS_SAFETENSORS TRAINING_JSON NEW_BINDING_JSON NEW_CODEC_ARCHIVE NEW_REPORT_NDJSON";
fn invalid(message: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::InvalidInput, message.into()) }
fn contract(error: Error) -> io::Error { invalid(format!("codec training contract: {error:?}")) }

fn execute(plan: &Plan, inputs: Inputs, destinations: &OutputPaths, output: &mut impl Write)
    -> Result<(), Failure>
{
    destinations.require_absent()?;
    let mut saved = SavedReport::new(&destinations.report, plan.outputs.report_bytes)?;
    report::start(&mut saved, plan, &inputs)?;
    saved.sync()?;
    // There is exactly one original owner. It latches failed calls and releases
    // partial numerical state on cancellation; this runner never restarts it.
    let mut run = plan.begin(&inputs.model).map_err(contract)?;
    let mut stage = "capture_and_fit";
    let result = (|| -> Result<(), Failure> {
        drive(&mut run, &mut saved, output)?;
        report::outcome(&mut saved, run.report())?;
        saved.sync()?;
        let (codec, checkpoint, original) = run.finish().map_err(contract)?;
        plan.check_completed(&inputs.estimate, &original).map_err(contract)?;
        stage = "artifact_serialization";
        let intended = checkpoint.binding();
        let encoded_binding = binding::encode(&intended, plan.outputs.binding_bytes)?;
        // The unchanged consumer parser is also the final writer compatibility
        // check; no alternate source inventory or JSON codec is introduced.
        if fit::binding(&encoded_binding).map_err(invalid)? != intended {
            return Err(contract(Error::Binding).into());
        }
        let archive = checkpoint.encode_archive(plan.outputs.archive_bytes).map_err(contract)?;
        report::artifacts(&mut saved, encoded_binding.len(), archive.len(), false)?;
        saved.sync()?;
        // A broken status consumer must not cause any new artifact writes. A
        // later exclusive-write failure retains completed work and diagnostics.
        writeln!(output, "{{\"kind\":\"codec_ready\",\"sources\":{},\"groups\":{},\"binding_bytes\":{},\"archive_bytes\":{},\"permission\":\"not_issued\"}}",
            intended.sources.len(), codec.groups().len(), encoded_binding.len(), archive.len())?;
        output.flush()?;
        stage = "binding_write";
        assets::write_new(&destinations.binding, &encoded_binding)?;
        stage = "archive_write";
        assets::write_new(&destinations.archive, &archive)?;
        report::artifacts(&mut saved, encoded_binding.len(), archive.len(), true)?;
        saved.sync()?;
        writeln!(output, "{{\"kind\":\"codec_saved\",\"binding_bytes\":{},\"archive_bytes\":{},\"permission\":\"not_issued\"}}",
            encoded_binding.len(), archive.len())?;
        output.flush()?;
        Ok(())
    })();
    if let Err(error) = &result {
        if let Err(report_error) = report::failure(&mut saved, stage, error.as_ref()).and_then(|()| saved.sync()) {
            return Err(invalid(format!("{error}; diagnostic persistence failed: {report_error}")).into());
        }
    }
    result
}

/// The original owner advances one token per call, then performs one original
/// bounded fit. Closed progress output cancels an active owner before further
/// numerical work; the complete original partial report remains diagnostic data.
fn drive(run: &mut CodecTraining, saved: &mut impl Write, output: &mut impl Write)
    -> Result<(), Failure>
{
    let result = (|| -> Result<(), Failure> {
        loop {
            match run.status() {
                Status::Fitted => return Ok(()),
                Status::Failed(error) => return Err(contract(error).into()),
                Status::Cancelled => return Err(contract(Error::WrongState).into()),
                Status::Capturing | Status::ReadyToFit => {},
            }
            let prior = run.current_origin();
            let fitting = run.status() == Status::ReadyToFit;
            run.advance(run.revision()).map_err(contract)?;
            if fitting {
                writeln!(output, "{{\"kind\":\"fit_completed\",\"permission\":\"not_issued\"}}")?;
                output.flush()?;
            } else if prior != run.current_origin() {
                let origin = prior.ok_or_else(|| contract(Error::Binding))?;
                report::document(saved, "document_captured", origin, &run.report().documents[&origin])?;
                saved.flush()?;
                writeln!(output, "{{\"kind\":\"document_captured\",\"origin\":{origin},\"permission\":\"not_issued\"}}")?;
                output.flush()?;
            }
        }
    })();
    if result.is_err() {
        if matches!(run.status(), Status::Capturing | Status::ReadyToFit) {
            run.cancel(run.revision()).map_err(contract)?;
        }
        // Failed inference may have incomplete physical work accounting. Keep
        // that original flag and all unstarted documents instead of claiming 0.
        if let Err(report_error) = report::outcome(saved, run.report()).and_then(|()| saved.flush()) {
            let primary = result.unwrap_err();
            return Err(invalid(format!("{primary}; partial outcome persistence failed: {report_error}")).into());
        }
    }
    result
}

fn run(args: &[OsString], output: &mut impl Write) -> Result<(), Failure> {
    if args.len() != 6 { return Err(invalid(USAGE).into()); }
    let destinations = OutputPaths::new(Path::new(&args[3]), Path::new(&args[4]), Path::new(&args[5]))?;
    destinations.require_absent()?;
    let (plan, inputs) = assets::load(args)?;
    execute(&plan, inputs, &destinations, output)
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).take(7).collect();
    match run(&args, &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => { eprintln!("{error}"); ExitCode::FAILURE }
    }
}

#[cfg(test)]
#[path = "train_learned_kv_codec_from_checkpoint/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "train_learned_kv_probes_from_checkpoint.rs"]
#[allow(dead_code)]
mod probe_command;
