//! Explicit original checkpoint at a selected quiet boundary before sampling.
//! The selected numerical recipe supplies the model/stream; no actor input can
//! select this supervisor operation or obtain the saved numerical state.
use super::{Config, FileOversight, GenerationStatus, Loaded, debug};
use super::fields::Fields;
use crate::config::read_regular;
use fa_reference::action::consequence::delivery::persistent::JournalError;
use fa_reference::action::consequence::gate::containment::RestartGrade;
use fa_reference::strict_json::{self, Limits};
use fa_reference::Error;
use std::path::Path;

const OPTION: &str = "--checkpoint-before-generation";
const USAGE: &str = "--checkpoint-before-generation PROFILE requires a live learned Message command and a fresh checkpoint ID; a resident sequence captures its first Message only";
const MAX_BYTES: usize = 4096;

/// A separately selected exact numerical boundary. Native capture binds the
/// actual current authority/actor/control cut under the original owner lock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Selection {
    checkpoint: u64,
    position: u64,
    sampled_draws: u64,
}

/// Own the whole live grammar before historical modes can consume it. The
/// original parsers still validate every remaining mode and option together.
pub(super) fn take_option(args: &[String]) -> Result<(Vec<String>, Option<&Path>), String> {
    let positions = args.iter().enumerate().filter(|(_, arg)| *arg == OPTION)
        .map(|(index, _)| index).collect::<Vec<_>>();
    if positions.is_empty() { return Ok((args.to_vec(), None)); }
    let single = args.first().map(String::as_str) == Some("create-learned-generated");
    let resident = args.first().map(String::as_str) == Some("serve-learned-generated");
    if positions.len() != 1 || args.len() < 6 || (!single && !resident)
        || args[3].starts_with("--") || positions[0] < 4 { return Err(USAGE.into()); }
    let at = positions[0];
    let path = args.get(at + 1).ok_or(USAGE)?;
    if path.is_empty() || path.starts_with("--") { return Err(USAGE.into()); }
    let mut index = 4;
    while index < args.len() {
        match args[index].as_str() {
            "--continue-generation" => index += 1,
            "--actor-profile" | "--precheck-profile" if single => {
                let value = args.get(index + 1).ok_or(USAGE)?;
                if value.is_empty() || value.starts_with("--") { return Err(USAGE.into()); }
                index += 2;
            }
            "--after-message" | "--history-anchor" | "--retain-history-anchor" | "--topology-profile" | "--campaign-profile"
                | OPTION => {
                let value = args.get(index + 1).ok_or(USAGE)?;
                if value.is_empty() || value.starts_with("--") { return Err(USAGE.into()); }
                index += 2;
            }
            _ => return Err(USAGE.into()),
        }
    }
    let mut remaining = args[..at].to_vec();
    remaining.extend_from_slice(&args[at + 2..]);
    Ok((remaining, Some(Path::new(path))))
}

impl Selection {
    pub(super) fn read(path: &Path) -> Result<Self, String> {
        Self::decode(&read_regular(path, MAX_BYTES)?)
    }
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        let json = strict_json::parse(bytes, Limits { max_bytes: MAX_BYTES,
            max_depth: 2, max_items: 16, max_string_bytes: 64 }).map_err(debug)?;
        let mut fields = Fields::new(json)?;
        if fields.text("schema")? != "fa.learned-generation-checkpoint/1" {
            return Err("unsupported learned generation-checkpoint schema".into());
        }
        let selected = Self { checkpoint: number(&mut fields, "checkpoint")?,
            position: number(&mut fields, "expected_position")?,
            sampled_draws: number(&mut fields, "expected_sampled_draws")? };
        fields.end()?;
        if selected.checkpoint == 0 || selected.position == 0 {
            return Err("checkpoint ID and numerical position must be nonzero".into());
        }
        Ok(selected)
    }

    pub(super) fn check(&self, config: &Config, loaded: &Loaded) -> Result<(), String> {
        if config.profile.delivery.actor.profile().grade != RestartGrade::FunctionalRestart {
            return Err("live learned checkpoint capture requires an explicit functional_restart profile".into());
        }
        if loaded.topology.is_some() || loaded.generation.required_pre_output_forecast().is_some()
            || loaded.generation.required_owned_pre_output_forecast().is_some() {
            return Err("live checkpoint selection currently requires a plain nonpredictive learned stream".into());
        }
        Ok(())
    }

    /// Refuse a reused ID or already crossed cut before starting generation.
    /// This remains read-only; it does not adopt a historical capture as new.
    pub(super) fn check_owner(&self, host: &FileOversight) -> Result<(), String> {
        match host.learned_stream_checkpoint(self.checkpoint) {
            Err(JournalError::Contract(Error::Missing)) => {},
            Ok(_) => return Err("checkpoint ID already exists; continue without a new capture selection".into()),
            Err(error) => return Err(debug(error)),
        }
        if host.action_consistency_required() {
            return Err("installed predictors require their own checkpoint rearming contract".into());
        }
        let current = host.learned_generation_inspection().map_err(debug)?;
        if current.numerical.position > self.position
            || current.numerical.sampled_draws > self.sampled_draws {
            return Err("selected generation checkpoint boundary has already been crossed".into());
        }
        Ok(())
    }

    pub(super) fn capture(&self, host: &mut FileOversight) -> Result<(), String> {
        self.check_owner(host)?;
        let current = host.learned_generation_inspection().map_err(debug)?;
        if current.paused || current.pending.is_some()
            || current.numerical.status != GenerationStatus::Generating
            || current.numerical.position != self.position
            || current.numerical.sampled_draws != self.sampled_draws {
            return Err("original quiet generation boundary does not match the selected checkpoint".into());
        }
        let revision = host.revision();
        let epoch = host.inspect().control.ledger.epoch;
        host.capture_learned_stream_checkpoint(revision, self.checkpoint,
            current.numerical.actor_revision, epoch).map_err(debug)?;
        Ok(())
    }
}

fn number(fields: &mut Fields, name: &str) -> Result<u64, String> {
    let value = fields.text(name)?;
    if value.is_empty() || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{name} must be a canonical unsigned decimal string"));
    }
    value.parse().map_err(|_| format!("{name} exceeds the unsigned 64-bit range"))
}
