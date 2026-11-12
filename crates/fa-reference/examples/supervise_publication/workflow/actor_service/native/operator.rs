//! Explicit operator selection for the original native-text service.
//! The actor keeps its source-only wire; this endpoint owns separate finite
//! observation/control grants and never carries caller-supplied message bytes.

use super::super::operator as shared;
use super::{Config, Deadline, FileGeneratedTextActorPort, FileSupervisedDriver, PeerProfile, Profile};
use std::path::{Path, PathBuf};

const USAGE: &str = "native text operator selection requires paired --operator-profile PATH --tower-generation NONZERO alongside --native-text RECIPE; only native create/open, checked forms and explicit open continuation/finish are supported";

/// Validate the entire selected native grammar before other option parsers can
/// remove a conflicting mode. Preserve the original grammar when unselected.
pub(super) fn take_option(
    args: &[String],
) -> Result<(Vec<String>, Option<(PathBuf, u64)>), String> {
    if !args.iter().any(|value| {
        matches!(value.as_str(), "--operator-profile" | "--tower-generation")
    }) {
        return Ok((args.to_vec(), None));
    }
    let (open, checked) = match args.first().map(String::as_str) {
        Some("serve-create") => (false, false),
        Some("serve-open") => (true, false),
        Some("serve-create-checked") => (false, true),
        Some("serve-open-checked") => (true, true),
        _ => return Err(USAGE.into()),
    };
    let positional = if checked { 5 } else { 4 };
    if args.len() < positional + 6
        || (args.len() - positional) % 2 != 0
        || args[1..positional]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut native = None;
    let mut profile = None;
    let mut generation = None;
    let mut continuation = None;
    for pair in args[positional..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") {
            return Err(USAGE.into());
        }
        match pair[0].as_str() {
            "--native-text" if native.is_none() => native = Some(pair[1].clone()),
            "--operator-profile" if profile.is_none() => profile = Some(PathBuf::from(&pair[1])),
            "--tower-generation" if generation.is_none() => {
                let value = &pair[1];
                if value.len() > 20
                    || value.starts_with('0')
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(USAGE.into());
                }
                generation = Some(value.parse::<u64>().map_err(|_| USAGE.to_owned())?);
            }
            "--after-message" | "--finish-after-message" if open && continuation.is_none() => {
                continuation = Some(pair.to_vec());
            }
            _ => return Err(USAGE.into()),
        }
    }
    let mut remaining = args[..positional].to_vec();
    remaining.extend(["--native-text".to_owned(), native.ok_or(USAGE)?]);
    if let Some(continuation) = continuation {
        remaining.extend(continuation);
    }
    Ok((remaining, Some((profile.ok_or(USAGE)?, generation.ok_or(USAGE)?))))
}

pub(super) struct Selection {
    pub(super) settings: shared::Settings,
    pub(super) generation: u64,
}

impl Selection {
    pub(super) fn new(settings: shared::Settings, generation: u64) -> Result<Self, String> {
        if generation == 0 {
            return Err(USAGE.into());
        }
        settings.check_generated_operator()?;
        Ok(Self { settings, generation })
    }

    pub(super) fn read(path: &Path, generation: u64) -> Result<Self, String> {
        Self::new(shared::Settings::read(path)?, generation)
    }

    /// Check audience and all endpoint/source/store aliases before the first
    /// clock, numerical asset loading, socket claim or durable owner startup.
    pub(super) fn check(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
    ) -> Result<(), String> {
        if self.generation == 0 {
            return Err(USAGE.into());
        }
        self.settings.check_generated_operator()?;
        self.settings.check_host(config, actor, reviewer, &[])?;
        let socket = resolve(&self.settings.socket)?;
        if socket.starts_with(resolve(&config.store)?) {
            return Err("native operator socket must be outside the original authority store".into());
        }
        let review = reviewer.socket(actor.request);
        let stop = crate::workflow::control::socket_path(reviewer, actor.request);
        for path in [actor.socket.as_path(), review.as_path(), stop.as_path(), config.source.path()] {
            if socket == resolve(path)? {
                return Err("native operator socket conflicts with original transport or source custody".into());
            }
        }
        Ok(())
    }

    /// The existing exact recovery may already have fenced historical keys.
    /// This selection never turns a recorded receipt workflow into a live
    /// mutation service or silently drops requested authority.
    pub(super) fn check_recorded(&self) -> Result<(), String> {
        if self.settings.resolution.is_some()
            || self.settings.cancellation.is_some()
            || self.settings.abandonment.is_some()
            || self.settings.fence.is_some()
            || self.settings.narrowing.is_some()
        {
            return Err("recorded native receipt recovery accepts only observation operator grants".into());
        }
        Ok(())
    }

    pub(super) fn start(
        &self,
        actor: &Profile,
        port: FileGeneratedTextActorPort,
        driver: &FileSupervisedDriver,
        deadline: &Deadline,
    ) -> Result<shared::Service, String> {
        shared::Service::start_generated(
            &self.settings, actor, port, driver, self.generation, deadline,
        )
    }
}

/// Resolve existing ancestors without opening a source or authority store.
/// Missing output/socket suffixes retain their exact filesystem identity.
fn resolve(path: &Path) -> Result<PathBuf, String> {
    let mut ancestor = std::path::absolute(path).map_err(super::debug)?;
    let mut suffix = Vec::new();
    loop {
        match std::fs::symlink_metadata(&ancestor) {
            Ok(_) => {
                let mut canonical = std::fs::canonicalize(&ancestor).map_err(super::debug)?;
                for component in suffix.into_iter().rev() {
                    canonical.push(component);
                }
                return Ok(canonical);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(ancestor.file_name().ok_or("missing native path ancestor")?.to_os_string());
                if !ancestor.pop() {
                    return Err("missing native path ancestor".into());
                }
            }
            Err(error) => return Err(super::debug(error)),
        }
    }
}
