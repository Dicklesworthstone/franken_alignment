//! Explicit independent custody for the original native publication history.
//! Archives use the existing private format and exclusive durable writer. They
//! are not actor references, fresh observations, permits or authenticated heads.

use super::{Config, FileOversight, FileSupervisedDriver, PeerProfile, Profile, exact_precheck, operator};
use super::super::learned::anchors as archive;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::FileHistoryAnchor;
use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::{fs, io};

const USAGE: &str = "native history custody requires --history-anchor INPUT only on explicit recovery and/or --retain-history-anchor NEW_OUTPUT; archives must remain outside the authority store and separate from all selected inputs and endpoints";

#[derive(Default)]
pub(super) struct Options {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
}

/// Validate the entire opted-in grammar before removing either custody option.
/// Existing mode parsers still validate their own values and supported pairs.
pub(super) fn take_options(args: &[String]) -> Result<(Vec<String>, Options), String> {
    if !args.iter().any(|arg| matches!(arg.as_str(), "--history-anchor" | "--retain-history-anchor")) {
        return Ok((args.to_vec(), Options::default()));
    }
    let (open, positional) = match args.first().map(String::as_str) {
        Some("serve-create") => (false, 4),
        Some("serve-open") => (true, 4),
        Some("serve-create-checked") => (false, 5),
        Some("serve-open-checked") => (true, 5),
        _ => return Err(USAGE.into()),
    };
    if args.len() < positional + 4
        || (args.len() - positional) % 2 != 0
        || args[1..positional].iter().any(|arg| arg.is_empty() || arg.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut input = None;
    let mut output = None;
    let mut seen = BTreeSet::new();
    let mut native = None;
    let mut continuation = None;
    let mut others = Vec::new();
    for pair in args[positional..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") || !seen.insert(pair[0].as_str()) {
            return Err(USAGE.into());
        }
        match pair[0].as_str() {
            "--history-anchor" if open => input = Some(pair[1].clone()),
            "--retain-history-anchor" => output = Some(pair[1].clone()),
            "--native-text" => native = Some(pair[1].clone()),
            "--operator-profile" | "--tower-generation" | "--precheck-profile" => {
                others.extend_from_slice(pair);
            }
            "--after-message" | "--finish-after-message" if open && continuation.is_none() => {
                continuation = Some(pair.to_vec());
            }
            _ => return Err(USAGE.into()),
        }
    }
    let mut remaining = args[..positional].to_vec();
    remaining.extend(["--native-text".to_owned(), native.ok_or(USAGE)?]);
    remaining.extend(others);
    if let Some(continuation) = continuation {
        remaining.extend(continuation);
    }
    Ok((remaining, Options::from_paths(input, output)?))
}

fn path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.contains('\0')
        || path.components().any(|part| matches!(part, Component::ParentDir))
        || path.file_name().is_none()
    {
        return Err(USAGE.into());
    }
    Ok(path.to_path_buf())
}

pub(super) struct Selection {
    input: Option<FileHistoryAnchor>,
    input_path: Option<PathBuf>,
    output: Option<PathBuf>,
}

impl Options {
    /// Resident manifests use the same strict paths and admission machinery.
    pub(super) fn from_paths(input: Option<String>, output: Option<String>) -> Result<Self, String> {
        let selected = |value: String| {
            if value.starts_with("--") { return Err(USAGE.into()); }
            path(&value)
        };
        Ok(Self {
            input: input.map(selected).transpose()?,
            output: output.map(selected).transpose()?,
        })
    }

    pub(super) fn admit(self, config: &Config, open: bool) -> Result<Option<Selection>, String> {
        if self.input.is_none() && self.output.is_none() {
            return Ok(None);
        }
        if self.input.is_some() && !open {
            return Err(USAGE.into());
        }
        let store = operator::resolve(&config.store)?;
        let input_path = self.input.as_deref().map(operator::resolve).transpose()?;
        let output = self.output.as_deref().map(new_output).transpose()?;
        for path in input_path.iter().chain(output.iter()) {
            if path.starts_with(&store) {
                return Err(USAGE.into());
            }
        }
        // Import exactly the independently selected archive, never today's
        // authority journal as a replacement for a missing or malformed input.
        let input = self.input.as_deref().map(archive::load).transpose()?;
        Ok(Some(Selection { input, input_path, output }))
    }
}

fn new_output(path: &Path) -> Result<PathBuf, String> {
    let canonical = operator::resolve(path)?;
    let parent = canonical.parent().ok_or(USAGE)?;
    if !fs::metadata(parent).map_err(super::debug)?.is_dir() {
        return Err("native history output parent must be an existing directory".into());
    }
    require_new(&canonical)?;
    Ok(canonical)
}

fn require_new(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(super::debug(error)),
        Ok(_) => Err("retained native history requires a NEW file; an existing archive is never replaced".into()),
    }
}

impl Selection {
    pub(super) fn input(&self) -> Option<&FileHistoryAnchor> { self.input.as_ref() }
    pub(super) fn output(&self) -> Option<&Path> { self.output.as_deref() }

    /// Recheck direct callers and every future sequence endpoint before clocks
    /// or live custody. The imported bytes and selected output are immutable.
    pub(super) fn check(
        &self,
        config: &Config,
        actors: &[(&Profile, Option<&exact_precheck::Selection>)],
        reviewer: &PeerProfile,
        selected_operator: Option<&operator::Selection>,
        open: bool,
    ) -> Result<(), String> {
        if self.input.is_some() && !open {
            return Err(USAGE.into());
        }
        let store = operator::resolve(&config.store)?;
        let mut forbidden = vec![config.source.path().to_path_buf()];
        for (actor, precheck) in actors {
            forbidden.extend([
                actor.socket.clone(),
                reviewer.socket(actor.request),
                crate::workflow::control::socket_path(reviewer, actor.request),
            ]);
            if let Some(precheck) = precheck {
                forbidden.push(precheck.settings.socket_path().to_path_buf());
            }
        }
        if let Some(operator) = selected_operator {
            forbidden.push(operator.settings.socket.clone());
        }
        for path in self.input_path.iter().chain(self.output.iter()) {
            if path.starts_with(&store) {
                return Err(USAGE.into());
            }
        }
        self.check_files(&forbidden)?;
        if let Some(output) = &self.output {
            require_new(output)?;
        }
        Ok(())
    }

    pub(super) fn check_files(&self, forbidden: &[PathBuf]) -> Result<(), String> {
        for forbidden in forbidden {
            let forbidden = operator::resolve(forbidden)?;
            if self.input_path.as_ref() == Some(&forbidden) || self.output.as_ref() == Some(&forbidden) {
                return Err("native history archive aliases a selected input or transport endpoint".into());
            }
        }
        Ok(())
    }

    /// Capture only the exact acknowledged healthy owner. Advancing an old
    /// selected prefix never permits replacing a stronger independent archive.
    pub(super) fn capture(&self, host: &FileOversight) -> Result<Option<FileHistoryAnchor>, String> {
        if self.output.is_none() {
            return Ok(None);
        }
        match &self.input {
            Some(previous) => host.history_anchor_after(previous),
            None => host.history_anchor(),
        }.map(Some).map_err(super::debug)
    }

    pub(super) fn retain<T>(&self, host: &FileOversight, result: Result<T, String>) -> Result<T, String> {
        self.finish_capture(self.capture(host), result)
    }

    pub(super) fn retain_driver<T>(
        &self,
        driver: &FileSupervisedDriver,
        result: Result<T, String>,
    ) -> Result<T, String> {
        if self.output.is_none() {
            return result;
        }
        let capture = driver.supervisor().host().map_err(super::debug)
            .and_then(|host| self.capture(&host));
        self.finish_capture(capture, result)
    }

    /// Called only at a no-work ownership transfer, or with the original sealed
    /// terminal recovery's successful acknowledgment. No disk reread/replay.
    pub(super) fn finish_capture<T>(
        &self,
        captured: Result<Option<FileHistoryAnchor>, String>,
        result: Result<T, String>,
    ) -> Result<T, String> {
        let retained = captured.and_then(|captured| match (&self.output, captured) {
            (Some(path), Some(anchor)) => archive::export_acknowledged(&anchor, path),
            (None, _) => Ok(()),
            (Some(_), None) => Err("native history acknowledgment is unavailable".into()),
        });
        match retained {
            Ok(()) => result,
            Err(error) => {
                let error = format!("native history retention failed: {error}; no effect retry was sent; retain the previous archive and select a new output path");
                Err(match result {
                    Ok(_) => error,
                    Err(original) => format!("{original}; {error}"),
                })
            }
        }
    }
}
