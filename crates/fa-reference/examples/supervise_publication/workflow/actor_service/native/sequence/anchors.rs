//! Independently selected custody for native resident recovery. Archive bytes
//! are frozen before recipes; acknowledged output never replaces an old anchor.
use super::super::{Config, FileOversight, FileSupervisedDriver, PeerProfile, Profile, debug, exact_precheck, operator};
use super::Mode;
use crate::config::read_regular;
use fa_reference::action::consequence::delivery::persistent::observed::guarded::anchored::{
    FileHistoryAnchor, MAX_HISTORY_ANCHOR_BYTES,
};
use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{self, Cursor},
    path::{Component, Path, PathBuf},
};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

const USAGE: &str = "native sequence history requires --history-anchor INPUT only with --after-message or explicitly selected --continue-generation, and/or --retain-history-anchor NEW_OUTPUT";

#[derive(Default)]
pub(super) struct Options {
    input: Option<PathBuf>,
    output: Option<PathBuf>,
}

/// Remove only explicitly selected history options after validating every pair.
/// Other native sequence option parsers retain their original closed grammars.
pub(super) fn take_options(args: &[String]) -> Result<(Vec<String>, Options), String> {
    if !args.iter().any(|value| matches!(
        value.as_str(), "--history-anchor" | "--retain-history-anchor"
    )) {
        return Ok((args.to_vec(), Options::default()));
    }
    if args.len() < 6
        || args.len() > 16
        || args.len() % 2 != 0
        || args.first().map(String::as_str) != Some("serve-native-generated")
        || args[1..4].iter().any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut remaining = args[..4].to_vec();
    let mut options = Options::default();
    let mut seen = BTreeSet::new();
    for pair in args[4..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") || !seen.insert(&pair[0]) {
            return Err(USAGE.into());
        }
        match pair[0].as_str() {
            "--history-anchor" => options.input = Some(path(&pair[1])?),
            "--retain-history-anchor" => options.output = Some(path(&pair[1])?),
            "--publication-profile" | "--after-message" | "--operator-profile"
                | "--tower-generation" => remaining.extend_from_slice(pair),
            _ => return Err(USAGE.into()),
        }
    }
    Ok((remaining, options))
}

fn path(value: &str) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.as_bytes().contains(&0)
        || path.file_name().is_none()
        || path.components().any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err("native history paths must be normalized nonempty paths".into());
    }
    Ok(path.to_path_buf())
}

#[derive(Default)]
pub(in super::super) struct Selection {
    input: Option<(PathBuf, FileHistoryAnchor)>,
    output: Option<PathBuf>,
}

impl Options {
    pub(super) fn is_selected(&self) -> bool {
        self.input.is_some() || self.output.is_some()
    }

    pub(super) fn is_recovery_pair(&self) -> bool {
        self.input.is_some() && self.output.is_some()
    }

    /// Archive custody cannot consume or later replace any explicitly selected
    /// configuration or manifest input, including a file not yet created.
    pub(super) fn check_files(&self, files: &[PathBuf]) -> Result<(), String> {
        if !self.is_selected() {
            return Ok(());
        }
        let selected = self.input.iter().chain(self.output.iter())
            .map(|path| operator::resolve(path))
            .collect::<Result<BTreeSet<_>, _>>()?;
        for path in files {
            let path = operator::resolve(path)?;
            if selected.iter().any(|reserved| overlaps(reserved, &path)) {
                return Err("native history custody aliases a selected input file".into());
            }
        }
        Ok(())
    }

    pub(super) fn check_mode(&self, after: Option<u64>) -> Result<(), String> {
        if self.input.is_some() && after.is_none_or(|id| id == 0) {
            return Err(USAGE.into());
        }
        Ok(())
    }

    pub(super) fn check_selected_mode(&self, mode: Mode) -> Result<(), String> {
        match mode {
            Mode::ContinueGeneration => Ok(()),
            Mode::Create | Mode::AfterMessage(_) => self.check_mode(mode.after()),
        }
    }

    #[cfg(test)]
    pub(super) fn admit(
        self,
        config: &Config,
        entries: &[(&Profile, Option<&exact_precheck::Selection>)],
        peers: &PeerProfile,
        operator: Option<&operator::Selection>,
        after: Option<u64>,
        credibility: Option<&Path>,
    ) -> Result<Selection, String> {
        self.admit_mode(
            config,
            entries,
            peers,
            operator,
            Mode::from_after(after),
            credibility,
        )
    }

    pub(super) fn admit_mode(
        self,
        config: &Config,
        entries: &[(&Profile, Option<&exact_precheck::Selection>)],
        peers: &PeerProfile,
        operator: Option<&operator::Selection>,
        mode: Mode,
        credibility: Option<&Path>,
    ) -> Result<Selection, String> {
        self.check_selected_mode(mode)?;
        if self.input.is_none() && self.output.is_none() {
            return Ok(Selection::default());
        }
        let input = self.input.map(|path| {
            let canonical = operator::resolve(&path)?;
            Ok::<_, String>((path, canonical))
        }).transpose()?;
        let output = self.output.map(|path| resolve_new(&path)).transpose()?;
        check_paths(
            config, entries, peers, operator, credibility,
            input.as_ref().map(|(_, path)| path.as_path()), output.as_deref(),
        )?;
        if let Some(path) = &output {
            require_new(path)?;
        }
        // Bind the selected file to one private regular archive. A pathname,
        // current journal or later edit can never replace these frozen bytes.
        let input = input.map(|(path, canonical)| {
            let metadata = fs::symlink_metadata(&path).map_err(debug)?;
            if !metadata.is_file()
                || metadata.file_type().is_symlink()
                || metadata.permissions().mode() & 0o077 != 0
            {
                return Err("native history input must be a private regular file".into());
            }
            let bytes = read_regular(&path, MAX_HISTORY_ANCHOR_BYTES)?;
            let anchor = FileHistoryAnchor::read_trusted(
                &mut Cursor::new(bytes), MAX_HISTORY_ANCHOR_BYTES,
            ).map_err(debug)?;
            Ok::<_, String>((canonical, anchor))
        }).transpose()?;
        Ok(Selection { input, output })
    }
}

impl Selection {
    pub(super) fn output_path(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub(super) fn input(&self) -> Option<&FileHistoryAnchor> {
        self.input.as_ref().map(|(_, anchor)| anchor)
    }

    pub(super) fn check_endpoint(&self, endpoint: &Path) -> Result<(), String> {
        let endpoint = operator::resolve(endpoint)?;
        if self
            .input
            .as_ref()
            .is_some_and(|(path, _)| overlaps(path, &endpoint))
            || self.output.as_ref().is_some_and(|path| overlaps(path, &endpoint))
        {
            return Err(
                "native history custody overlaps a selected input or service endpoint".into(),
            );
        }
        Ok(())
    }

    pub(super) fn check_files(&self, files: &[&Path]) -> Result<(), String> {
        for path in files {
            self.check_endpoint(path)?;
        }
        Ok(())
    }

    pub(super) fn check_selected(
        &self,
        config: &Config,
        entries: &[(&Profile, Option<&exact_precheck::Selection>)],
        peers: &PeerProfile,
        operator: Option<&operator::Selection>,
        mode: Mode,
        credibility: Option<&Path>,
    ) -> Result<(), String> {
        if self.input.is_some() && (!mode.is_recovery() || mode.after() == Some(0)) {
            return Err(USAGE.into());
        }
        if self.input.is_none() && self.output.is_none() {
            return Ok(());
        }
        check_paths(
            config, entries, peers, operator, credibility,
            self.input.as_ref().map(|(path, _)| path.as_path()), self.output.as_deref(),
        )?;
        if let Some(path) = &self.output {
            require_new(path)?;
        }
        Ok(())
    }
}

fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

fn check_paths(
    config: &Config,
    entries: &[(&Profile, Option<&exact_precheck::Selection>)],
    peers: &PeerProfile,
    selected_operator: Option<&operator::Selection>,
    credibility: Option<&Path>,
    input: Option<&Path>,
    output: Option<&Path>,
) -> Result<(), String> {
    let store = operator::resolve(&config.store)?;
    let mut reserved = BTreeSet::from([operator::resolve(config.source.path())?]);
    if let Some(path) = credibility {
        reserved.insert(operator::resolve(path)?);
    }
    for (actor, precheck) in entries {
        for endpoint in [
            actor.socket.clone(),
            peers.socket(actor.request),
            crate::workflow::control::socket_path(peers, actor.request),
        ] {
            reserved.insert(operator::resolve(&endpoint)?);
        }
        if let Some(selected) = precheck {
            reserved.insert(operator::resolve(selected.settings.socket_path())?);
        }
    }
    if let Some(selected) = selected_operator {
        reserved.insert(operator::resolve(&selected.settings.socket)?);
    }
    let mut selected: BTreeSet<PathBuf> = BTreeSet::new();
    for path in [input, output].into_iter().flatten() {
        let path = operator::resolve(path)?;
        if overlaps(&path, &store)
            || reserved.iter().any(|reserved| overlaps(reserved, &path))
            || selected.iter().any(|previous| overlaps(previous, &path))
        {
            return Err("native history custody must be distinct from source, store, selected inputs and every scheduled endpoint".into());
        }
        selected.insert(path);
    }
    Ok(())
}

fn resolve_new(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(debug)?;
    let parent = fs::canonicalize(absolute.parent().ok_or(USAGE)?).map_err(debug)?;
    if !fs::metadata(&parent).map_err(debug)?.is_dir() {
        return Err("native history output parent must be an existing directory".into());
    }
    Ok(parent.join(absolute.file_name().ok_or(USAGE)?))
}

fn require_new(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(debug(error)),
        Ok(_) => Err("native history output must be a new file; previous custody is never replaced".into()),
    }
}

/// No disk read, replay or capture is hidden in this writer. Failure preserves
/// the caller's original outcome; an incomplete output is never acknowledged.
fn export(anchor: &FileHistoryAnchor, path: &Path) -> Result<(), String> {
    let parent = File::open(path.parent().ok_or(USAGE)?).map_err(debug)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(debug)?;
    anchor.write_to(&mut file, MAX_HISTORY_ANCHOR_BYTES).map_err(debug)?;
    file.sync_all().map_err(debug)?;
    parent.sync_all().map_err(debug)?;
    Ok(())
}

/// One selected output is written once after its original invocation returns.
/// Startup captures the acknowledged create/Fence cut, live completion replaces
/// it before driver cleanup, and sealed Stop supplies its own native archive.
/// A poisoned final owner suppresses export rather than relabeling an old cut.
pub(super) struct Retention<'a> {
    selection: Option<&'a Selection>,
    acknowledged: Option<Result<FileHistoryAnchor, String>>,
}

impl<'a> Retention<'a> {
    pub(super) fn new(selection: Option<&'a Selection>) -> Self {
        Self { selection, acknowledged: None }
    }

    pub(super) fn requested(&self) -> bool {
        self.selection.is_some_and(|selection| selection.output.is_some())
    }

    pub(super) fn observe(&mut self, host: &FileOversight) {
        if !self.requested() {
            return;
        }
        let previous = self.selection.and_then(Selection::input);
        self.acknowledged = Some(match previous {
            Some(anchor) => host.history_anchor_after(anchor),
            None => host.history_anchor(),
        }.map_err(debug));
    }

    pub(super) fn observe_driver(&mut self, driver: &FileSupervisedDriver) {
        if !self.requested() {
            return;
        }
        match driver.supervisor().host() {
            Ok(host) => self.observe(&host),
            Err(error) => self.acknowledged = Some(Err(debug(error))),
        }
    }

    pub(super) fn observe_stop(&mut self, anchor: Option<FileHistoryAnchor>) {
        if self.requested() && let Some(anchor) = anchor {
            self.acknowledged = Some(Ok(anchor));
        }
    }

    pub(super) fn finish(self, result: Result<(), String>) -> Result<(), String> {
        let Some(path) = self.selection.and_then(|selection| selection.output.as_deref()) else {
            return result;
        };
        let Some(acknowledged) = self.acknowledged else {
            // Admission/replay failed without an acknowledged new native cut.
            return result;
        };
        let retained = acknowledged.and_then(|anchor| export(&anchor, path));
        match (result, retained) {
            (result, Ok(())) => result,
            (result, Err(error)) => {
                let error = format!("history anchor retention failed: {error}; keep previous custody; no effect or Stop retry was sent");
                Err(match result {
                    Ok(()) => error,
                    Err(original) => format!("{original}; {error}"),
                })
            }
        }
    }
}
