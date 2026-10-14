//! Original guarded creation and historical inspection with independent custody.
#[path = "settings.rs"]
mod settings;
#[path = "mutations.rs"]
mod mutations;
#[cfg(target_os = "linux")]
#[path = "peer.rs"]
mod peer;
use settings::Settings;
use crate::config::{Config, debug};
use crate::profile::{CreationOutput, ResumeProfile};
use fa_reference::action::consequence::delivery::persistent::{FileDeliverySnapshot,
    observed::FileOversight};
use fa_reference::action::consequence::oversight::credibility::CredibilityReport;
use fa_reference::round::review::andon::AndonLedger;
use std::{io::Write, path::Path, time::{Duration, Instant}};

const USAGE: &str = "usage: native_andon create CONFIG ANDON_PROFILE NEW_ANCHOR\n       native_andon inspect CONFIG ANDON_PROFILE RECOVERY_PROFILE\n       native_andon assess CONFIG ANDON_PROFILE RECOVERY_PROFILE ASSESSMENT_JSON\n       native_andon register-window CONFIG ANDON_PROFILE RECOVERY_PROFILE WINDOW_JSON\n       native_andon promote CONFIG ANDON_PROFILE RECOVERY_PROFILE PROMOTION_JSON";

pub(crate) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let started = Instant::now();
    command_with_elapsed(args, &mut || started.elapsed(), out)
}
fn command_with_elapsed(args: &[String], elapsed: &mut impl FnMut() -> Duration,
    out: &mut impl Write) -> Result<(), String>
{
    // Closed routing BEFORE opening configuration or any authority path.
    let mode = args.first().map(String::as_str).unwrap_or("");
    let expected = match mode {
        "create" | "inspect" => 4,
        "assess" | "register-window" | "promote" | "serve-assess" => 5,
        _ => return Err(USAGE.into()),
    };
    if args.len() != expected {
        return Err(USAGE.into());
    }
    #[cfg(target_os = "linux")]
    if mode == "serve-assess" {
        return peer::serve(Path::new(&args[1]), Path::new(&args[2]), Path::new(&args[3]), Path::new(&args[4]), out);
    }
    #[cfg(not(target_os = "linux"))]
    if mode == "serve-assess" { return Err("peer-verified evaluator intake requires Linux".into()); }
    let config = Config::read(Path::new(&args[1]))?;
    let mut checkpoint = || {
        if elapsed() >= Duration::from_millis(config.timing.runtime_ms) {
            Err("native Andon invocation deadline reached".to_owned())
        } else { Ok(()) }
    };
    checkpoint()?;
    let settings = Settings::read(Path::new(&args[2]))?;
    checkpoint()?;
    match args[0].as_str() {
        "create" => create(&config, &settings, Path::new(&args[3]), &mut checkpoint, out),
        "inspect" => inspect(&config, &settings, Path::new(&args[3]), &mut checkpoint, out),
        "assess" | "register-window" | "promote" => {
            let mutation = mutations::Mutation::read(mode, Path::new(&args[4]))?;
            checkpoint()?;
            mutations::run(&config, &settings, Path::new(&args[3]), &mutation,
                &mut checkpoint, &mut mutations::clock, out)
        }
        _ => unreachable!("closed command"),
    }
}

fn create(config: &Config, settings: &Settings, output: &Path,
    checkpoint: &mut impl FnMut() -> Result<(), String>, out: &mut impl Write) -> Result<(), String>
{
    settings.creation()?;
    let custody = CreationOutput::admit(config, output)?;
    checkpoint()?;
    let (host, _roles) = settings.create(config, None)?;
    // Once acknowledged, never skip independent retention merely because a
    // synchronous write overran the deadline. No retry or rollback follows.
    let deadline = checkpoint();
    let retained = custody.retain(&host);
    let reported = report("create", false, retained.is_ok(), &host.inspect(),
        &host.credibility_report().map_err(debug)?, host.andon_ledger().map_err(debug)?, out);
    complete([deadline, retained, reported])
}
fn inspect(config: &Config, settings: &Settings, input: &Path,
    checkpoint: &mut impl FnMut() -> Result<(), String>, out: &mut impl Write) -> Result<(), String>
{
    let request = ResumeProfile::read(input)?;
    let custody = request.admit(config)?;
    if custody.retention_requested() {
        return Err("historical inspection cannot export a successor anchor".into());
    }
    let expected = settings.requirements(config, request.minimum())?;
    checkpoint()?;
    let snapshot = FileOversight::read_andon_guarded(&config.store, &config.profile,
        &expected, &settings.protocol, custody.anchor()).map_err(debug)?;
    checkpoint()?;
    report("inspect", true, false, &snapshot.credibility.journal,
        &snapshot.credibility.report, &snapshot.accounting, out)
}

// Only actual native counts and fixed command names are rendered. No private
// payload, helper question, evaluator evidence bytes or executable path escapes.
fn report(operation: &str, historical: bool, retained: bool, journal: &FileDeliverySnapshot,
    credibility: &CredibilityReport, accounting: &AndonLedger, out: &mut impl Write)
    -> Result<(), String>
{
    let result = (|| -> std::io::Result<()> {
        write!(out, "{{\"schema_version\":\"fa.native-andon-result/1\",\"operation\":\"{operation}\",\"historical\":{historical},\"history_anchor_retained\":{retained},\"journal_revision\":\"{}\",\"control_sequence\":\"{}\",\"authority_epoch\":\"{}\",\"evaluation_revision\":\"{}\",\"pending_cases\":{},\"censored_cases\":{},\"windows\":[",
            journal.revision, journal.control.sequence, journal.control.ledger.epoch,
            credibility.revision, credibility.pending_cases, credibility.censored_cases)?;
        for (index, state) in accounting.windows().values().enumerate() {
            if index != 0 { write!(out, ",")?; }
            write!(out, "{{\"window\":\"{}\",\"false_stops\":\"{}\",\"total_cost\":\"{}\",\"incident_opened_by_round\":",
                state.budget.window, state.false_stops, state.total_cost)?;
            match &state.incident {
                Some(incident) => write!(out, "\"{}\"", incident.opened_by_round)?,
                None => write!(out, "null")?,
            }
            write!(out, "}}")?;
        }
        writeln!(out, "]}}")?;
        out.flush()
    })();
    result.map_err(|error| format!("native accounting result output failed at revision {}: {error}; no operation was retried", journal.revision))
}
fn complete(results: [Result<(), String>; 3]) -> Result<(), String> {
    let errors: Vec<_> = results.into_iter().filter_map(Result::err).collect();
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
