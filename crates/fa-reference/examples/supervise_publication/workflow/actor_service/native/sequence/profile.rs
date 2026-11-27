//! Closed selection for one resident native-text owner. Every recipe is read
//! under its original per-recipe asset ceiling; the fixed entry count bounds
//! aggregate intake by sixteen such allowances. Matching immutable model and
//! tokenizer inputs share the first recipe's retained allocations afterward.

use super::super::{PublicationProfile, debug, exact_precheck, operator, recipe};
use super::{
    Config, Entry, MAX_ENTRIES, Mode, PeerProfile, Profile, Release, anchors, andon, evaluation,
    governor,
};
use crate::config::read_regular;
use crate::workflow::actor_service::{andon_settings, evaluator};
use fa_reference::action::ElapsedTick;
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

const MAX_PROFILE_BYTES: usize = 64 * 1024;
const USAGE: &str = "serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE [--publication-profile PROFILE] [--continue-generation | --after-message NONZERO] [--operator-profile PROFILE --tower-generation NONZERO] [--governor-profile PROFILE with governed operator] [--recovery-requirements PROFILE with governed --after-message and --history-anchor] [--credibility-activation CAPSULE with --after-message] [--history-anchor INPUT with explicit recovery] [--retain-history-anchor NEW_OUTPUT]; creation requires 2..=16 entries; explicit recovery accepts 1..=16 remaining Message/Finish entries; ordinary --continue-generation requires both --history-anchor INPUT and --retain-history-anchor NEW_OUTPUT and excludes governor/activation selections; plain/simple-witness services also accept paired --andon ANDON_PROFILE --evaluator-profile EVALUATOR_PROFILE with --retain-history-anchor and optional per-release exact prechecks; evaluated --continue-generation or --after-message recovery requires --history-anchor and --evaluation-recovery REQUIREMENTS; singleton Finish requires --after-message; governor/topology/activation/joint compositions remain separate";

struct Input {
    recipe: PathBuf,
    actor: PathBuf,
    release: Release,
    precheck: Option<PathBuf>,
}

/// Own the complete grammar before opening any input. Recovery requires an
/// explicit original message predecessor; other native lifecycle and
/// disclosure selections cannot be silently consumed by this mode.
fn options(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Option<PathBuf>, Option<u64>), String> {
    if !matches!(args.len(), 4 | 6 | 8)
        || args.first().map(String::as_str) != Some("serve-native-generated")
        || args[1..4]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut publication = None;
    let mut after = None;
    for pair in args[4..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") {
            return Err(USAGE.into());
        }
        match pair[0].as_str() {
            "--publication-profile" if publication.is_none() => {
                publication = Some(PathBuf::from(&pair[1]));
            }
            "--after-message" if after.is_none() => {
                let value = &pair[1];
                if value.len() > 20
                    || value.starts_with('0')
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(USAGE.into());
                }
                after = Some(value.parse::<u64>().map_err(|_| USAGE.to_owned())?);
            }
            _ => return Err(USAGE.into()),
        }
    }
    if credibility.is_some() && after.is_none() {
        return Err(USAGE.into());
    }
    Ok((publication, after))
}

/// Admit the complete selected operator grammar before opening any file. The
/// remaining original sequence options retain their own value validation;
/// duplicates and unsupported modes cannot disappear during option removal.
fn take_operator(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Vec<String>, Option<(PathBuf, u64)>), String> {
    if !args
        .iter()
        .any(|value| matches!(value.as_str(), "--operator-profile" | "--tower-generation"))
    {
        return Ok((args.to_vec(), None));
    }
    if args.len() < 8
        || args.len() > 12
        || args.len() % 2 != 0
        || args.first().map(String::as_str) != Some("serve-native-generated")
        || args[1..4]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut remaining = args[..4].to_vec();
    let mut profile = None;
    let mut generation = None;
    let mut seen = BTreeSet::new();
    for pair in args[4..].chunks_exact(2) {
        if pair[1].is_empty() || pair[1].starts_with("--") || !seen.insert(&pair[0]) {
            return Err(USAGE.into());
        }
        match pair[0].as_str() {
            "--operator-profile" => profile = Some(PathBuf::from(&pair[1])),
            "--tower-generation" => {
                let value = &pair[1];
                if value.len() > 20
                    || value.starts_with('0')
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(USAGE.into());
                }
                generation = Some(value.parse::<u64>().map_err(|_| USAGE.to_owned())?);
            }
            "--publication-profile" | "--after-message" => remaining.extend_from_slice(pair),
            _ => return Err(USAGE.into()),
        }
    }
    // A selected capsule still requires explicit suffix recovery. Validate
    // the retained native grammar before any configuration file is read.
    options(&remaining, credibility)?;
    Ok((
        remaining,
        Some((profile.ok_or(USAGE)?, generation.ok_or(USAGE)?)),
    ))
}

/// Own the unary ordinary continuation selector before the pair-only parsers.
/// Accounting has its own complete grammar and removes its selector first.
/// Require independently retained input and a new output at selection time;
/// mixed modes and unsupported guards cannot disappear during option removal.
fn take_unaccounted_continuation(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Vec<String>, bool), String> {
    if !args.iter().any(|arg| arg == "--continue-generation") {
        return Ok((args.to_vec(), false));
    }
    if credibility.is_some()
        || args.len() < 9
        || args.len() > 15
        || args.first().map(String::as_str) != Some("serve-native-generated")
        || args[1..4]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let mut remaining = args[..4].to_vec();
    let mut seen = BTreeSet::new();
    let mut index = 4;
    while index < args.len() {
        let flag = args[index].as_str();
        if !seen.insert(flag) {
            return Err(USAGE.into());
        }
        if flag == "--continue-generation" {
            index += 1;
            continue;
        }
        let value = args.get(index + 1).ok_or(USAGE)?;
        if value.is_empty() || value.starts_with("--") {
            return Err(USAGE.into());
        }
        match flag {
            "--history-anchor"
            | "--retain-history-anchor"
            | "--publication-profile"
            | "--operator-profile"
            | "--tower-generation" => remaining.extend_from_slice(&args[index..index + 2]),
            _ => return Err(USAGE.into()),
        }
        index += 2;
    }
    if !seen.contains("--history-anchor")
        || !seen.contains("--retain-history-anchor")
        || seen.contains("--operator-profile") != seen.contains("--tower-generation")
    {
        return Err(USAGE.into());
    }
    Ok((remaining, true))
}

pub(super) fn command(args: &[String], credibility: Option<&Path>) -> Result<(), String> {
    command_with_clock(
        args,
        credibility,
        crate::workflow::clock,
        &mut std::io::stdout().lock(),
    )
}

pub(super) fn command_with_clock<F>(
    args: &[String],
    credibility: Option<&Path>,
    time: F,
    output: &mut impl Write,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let (args, accounting_paths) = andon::take_options(args, credibility)?;
    let (args, continue_generation) = take_unaccounted_continuation(&args, credibility)?;
    let (args, governor_path, recovery_path) = governor::take_recovery_options(&args, credibility)?;
    let (args, history_options) = anchors::take_options(&args)?;
    let (args, operator_option) = take_operator(&args, credibility)?;
    let (publication_path, after) = options(&args, credibility)?;
    let mode = accounting_paths.as_ref().map_or_else(
        || {
            if continue_generation {
                Mode::ContinueGeneration
            } else {
                Mode::from_after(after)
            }
        },
        |paths| paths.mode,
    );
    if mode.after() != after {
        return Err(andon::INVALID.into());
    }
    history_options.check_selected_mode(mode)?;
    let mut selected_files = args[1..4].iter().map(PathBuf::from).collect::<Vec<_>>();
    selected_files.extend(publication_path.iter().cloned());
    selected_files.extend(operator_option.iter().map(|(path, _)| path.clone()));
    selected_files.extend(credibility.map(Path::to_path_buf));
    selected_files.extend(governor_path.iter().cloned());
    selected_files.extend(recovery_path.iter().cloned());
    let mut history_files = selected_files.clone();
    if let Some(paths) = &accounting_paths {
        history_files.extend([paths.settings.clone(), paths.evaluator.clone()]);
        history_files.extend(paths.recovery.iter().cloned());
    }
    history_options.check_files(&history_files)?;
    let config = Config::read(Path::new(&args[1]))?;
    let peers = PeerProfile::read(Path::new(&args[3]))?;
    let accounting_settings = accounting_paths
        .as_ref()
        .map(|paths| andon_settings::Settings::read(&paths.settings))
        .transpose()?;
    let evaluator_settings = accounting_paths
        .as_ref()
        .map(|paths| evaluator::Settings::read(&paths.evaluator))
        .transpose()?;
    let accounting_recovery = accounting_paths
        .as_ref()
        .and_then(|paths| paths.recovery.as_deref())
        .map(|path| evaluation::Recovery::read(path, &config))
        .transpose()?;
    let accounting = match (accounting_settings.as_ref(), evaluator_settings.as_ref()) {
        (Some(settings), Some(evaluator)) => Some(andon::Selection {
            settings,
            evaluator,
            recovery: accounting_recovery.as_ref(),
            paths: accounting_paths.as_ref(),
        }),
        (None, None) => None,
        _ => return Err(andon::INVALID.into()),
    };
    let selected_governor = governor_path
        .as_deref()
        .map(governor::Settings::read)
        .transpose()?;
    let selected_operator = operator_option
        .map(|(path, generation)| {
            if selected_governor.is_some() {
                operator::Selection::read_governed(&path, generation)
            } else {
                operator::Selection::read(&path, generation)
            }
        })
        .transpose()?;
    governor::check_selected(
        selected_governor.as_ref(),
        selected_operator.as_ref(),
        after,
        credibility,
        recovery_path.is_some(),
    )?;
    let publication = publication_path
        .as_deref()
        .map(PublicationProfile::read)
        .transpose()?;
    if let Some(publication) = &publication {
        // Reserve delayed witness/producer/feed inputs as well as the outer
        // profile, before any recipe can load assets or recovery can begin.
        history_options.check_files(publication.input_paths())?;
    }
    governor::check_publication(selected_governor.as_ref(), publication.as_ref())?;
    let recovery = recovery_path
        .as_deref()
        .map(|path| governor::requirements::Selection::read(path, &config))
        .transpose()?;
    if let Some(accounting) = accounting {
        accounting.check_mode(mode)?;
        accounting.check_configuration(&config, publication.as_ref())?;
        let mut files = selected_files
            .iter()
            .map(PathBuf::as_path)
            .collect::<Vec<_>>();
        files.push(config.source.path());
        if let Some(publication) = &publication {
            files.extend(publication.input_paths().iter().map(PathBuf::as_path));
        }
        accounting.check_files(&files)?;
        history_options.check_files(&[accounting.evaluator.socket_path().to_path_buf()])?;
        let (entries, history) = load_with_complete_services(
            Path::new(&args[2]),
            &config,
            &peers,
            mode,
            selected_operator.as_ref(),
            None,
            Some(history_options),
            None,
            None,
            Some(accounting),
            &files,
        )?;
        return super::run_accounted(
            config, entries, &peers, publication.as_ref(), (mode, selected_operator.as_ref()),
            history.as_ref().ok_or(andon::INVALID)?, accounting, time, output,
        );
    }
    if mode == Mode::ContinueGeneration {
        let (entries, history) = load_with_complete_services(
            Path::new(&args[2]),
            &config,
            &peers,
            mode,
            selected_operator.as_ref(),
            None,
            Some(history_options),
            None,
            None,
            None,
            &[],
        )?;
        return super::run_with_continuation(
            config,
            entries,
            &peers,
            publication.as_ref(),
            selected_operator.as_ref(),
            history.as_ref().ok_or(USAGE)?,
            time,
            output,
        );
    }
    if let Some(governor) = selected_governor.as_ref() {
        governor.check_custody_paths(
            &selected_files
                .iter()
                .map(PathBuf::as_path)
                .collect::<Vec<_>>(),
        )?;
        // A future listener cannot consume an independently retained archive.
        history_options.check_files(&[governor.socket.clone()])?;
        let (entries, history) = load_with_recovery_services(
            Path::new(&args[2]),
            &config,
            &peers,
            after,
            selected_operator.as_ref(),
            credibility,
            Some(history_options),
            Some(governor),
            recovery_path.as_deref(),
        )?;
        let requirements = recovery
            .as_ref()
            .map(|selected| selected.requirements(&config, &entries[0].inputs));
        return super::run_with_governed_recovery(
            config,
            entries,
            &peers,
            publication.as_ref(),
            (after, selected_operator.as_ref()),
            credibility,
            history.as_ref(),
            Some(governor),
            requirements.as_ref(),
            time,
            output,
        );
    }
    if history_options.is_selected() {
        let (entries, history) = load_with_history(
            Path::new(&args[2]),
            &config,
            &peers,
            after,
            selected_operator.as_ref(),
            credibility,
            Some(history_options),
        )?;
        super::run_with_history(
            config,
            entries,
            &peers,
            publication.as_ref(),
            (after, selected_operator.as_ref()),
            credibility,
            history.as_ref(),
            time,
            output,
        )
    } else if let Some(credibility) = credibility {
        let entries = load_with_qualification(
            Path::new(&args[2]),
            &config,
            &peers,
            after,
            selected_operator.as_ref(),
            Some(credibility),
        )?;
        super::run_with_qualification(
            config,
            entries,
            &peers,
            publication.as_ref(),
            (after, selected_operator.as_ref()),
            Some(credibility),
            time,
            output,
        )
    } else if let Some(selected) = selected_operator.as_ref() {
        let entries =
            load_with_operator(Path::new(&args[2]), &config, &peers, after, Some(selected))?;
        super::run_with_operator(
            config,
            entries,
            &peers,
            publication.as_ref(),
            (after, Some(selected)),
            time,
            output,
        )
    } else if after.is_none() {
        let entries = load(Path::new(&args[2]), &config, &peers)?;
        super::run(config, entries, &peers, publication.as_ref(), time, output)
    } else {
        let entries = load_selected(Path::new(&args[2]), &config, &peers, after)?;
        super::run_selected(
            config,
            entries,
            &peers,
            publication.as_ref(),
            after,
            time,
            output,
        )
    }
}

fn decode(bytes: &[u8], base: &Path) -> Result<Vec<Input>, String> {
    decode_selected(bytes, base, None)
}

fn decode_selected(bytes: &[u8], base: &Path, after: Option<u64>) -> Result<Vec<Input>, String> {
    decode_mode(bytes, base, Mode::from_after(after))
}

fn decode_mode(bytes: &[u8], base: &Path, mode: Mode) -> Result<Vec<Input>, String> {
    let after = mode.after();
    let mut root = object(
        strict_json::parse(
            bytes,
            Limits {
                max_bytes: MAX_PROFILE_BYTES,
                max_depth: 4,
                max_items: 256,
                max_string_bytes: 4096,
            },
        )
        .map_err(debug)?,
    )?;
    let with_prechecks = match text(&mut root, "schema")?.as_str() {
        "fa.native-publication-sequence/1" => false,
        "fa.native-publication-sequence/2" => true,
        _ => return Err("unsupported native publication sequence schema".into()),
    };
    let Json::Array(values) = take(&mut root, "entries")? else {
        return Err("native sequence entries must be an array".into());
    };
    end(root)?;
    check_count(values.len(), mode)?;
    let count = values.len();
    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            let mut fields = object(value)?;
            let recipe = selected_path(&text(&mut fields, "recipe")?, base)?;
            let actor = selected_path(&text(&mut fields, "actor_profile")?, base)?;
            let release = match text(&mut fields, "release")?.as_str() {
                "message" => Release::Message,
                "finish" if index + 1 == count && (index != 0 || after.is_some()) => Release::Finish,
                _ => {
                    return Err(
                        "native sequence entries must be Messages followed by an optional final Finish"
                            .into(),
                    );
                }
            };
            // Version one remains closed. Every version-two entry must
            // explicitly select its own disclosure or decline it with null.
            let precheck = if with_prechecks {
                match take(&mut fields, "precheck_profile")? {
                    Json::Null => None,
                    Json::String(path) => Some(selected_path(&path, base)?),
                    _ => return Err("native sequence precheck_profile must be a path or null".into()),
                }
            } else {
                None
            };
            end(fields)?;
            Ok(Input { recipe, actor, release, precheck })
        })
        .collect()
}

/// Freeze every actor selection before any model asset is read, then use the
/// original native recipe loader. No journal, socket, source observation or
/// logical clock is acquired by this admission path.
pub(super) fn load(
    path: &Path,
    config: &Config,
    peers: &PeerProfile,
) -> Result<Vec<Entry>, String> {
    load_selected(path, config, peers, None)
}

/// Recovery changes only the independently selected suffix bounds. Original
/// historical request/generation availability and the exact confirmed message
/// predecessor are checked by the native owner after cooperative reconstruction.
pub(super) fn load_selected(
    path: &Path,
    config: &Config,
    peers: &PeerProfile,
    after: Option<u64>,
) -> Result<Vec<Entry>, String> {
    load_with_operator(path, config, peers, after, None)
}

/// Actor and operator scopes and ALL future endpoint aliases are admitted
/// before loading any numerical recipe. This never starts a live service.
pub(super) fn load_with_operator(
    path: &Path,
    config: &Config,
    peers: &PeerProfile,
    after: Option<u64>,
    selected_operator: Option<&operator::Selection>,
) -> Result<Vec<Entry>, String> {
    load_with_qualification(path, config, peers, after, selected_operator, None)
}

/// The independently selected capsule is not read during model admission.
/// Reserve its canonical name before any future listener or authority owner.
pub(super) fn load_with_qualification(
    path: &Path,
    config: &Config,
    peers: &PeerProfile,
    after: Option<u64>,
    selected_operator: Option<&operator::Selection>,
    credibility: Option<&Path>,
) -> Result<Vec<Entry>, String> {
    load_with_history(
        path,
        config,
        peers,
        after,
        selected_operator,
        credibility,
        None,
    )
    .map(|(entries, _)| entries)
}

fn load_with_history(
    path: &Path,
    config: &Config,
    peers: &PeerProfile,
    after: Option<u64>,
    selected_operator: Option<&operator::Selection>,
    credibility: Option<&Path>,
    history_options: Option<anchors::Options>,
) -> Result<(Vec<Entry>, Option<anchors::Selection>), String> {
    load_with_services(
        path,
        config,
        peers,
        after,
        selected_operator,
        credibility,
        history_options,
        None,
    )
}

fn load_with_services(
    path: &Path,
    config: &Config,
    peers: &PeerProfile,
    after: Option<u64>,
    selected_operator: Option<&operator::Selection>,
    credibility: Option<&Path>,
    history_options: Option<anchors::Options>,
    selected_governor: Option<&governor::Settings>,
) -> Result<(Vec<Entry>, Option<anchors::Selection>), String> {
    load_with_recovery_services(
        path,
        config,
        peers,
        after,
        selected_operator,
        credibility,
        history_options,
        selected_governor,
        None,
    )
}

fn load_with_recovery_services(
    path: &Path,
    config: &Config,
    peers: &PeerProfile,
    after: Option<u64>,
    selected_operator: Option<&operator::Selection>,
    credibility: Option<&Path>,
    history_options: Option<anchors::Options>,
    selected_governor: Option<&governor::Settings>,
    recovery_path: Option<&Path>,
) -> Result<(Vec<Entry>, Option<anchors::Selection>), String> {
    load_with_complete_services(
        path,
        config,
        peers,
        Mode::from_after(after),
        selected_operator,
        credibility,
        history_options,
        selected_governor,
        recovery_path,
        None,
        &[],
    )
}

fn load_with_complete_services(
    path: &Path,
    config: &Config,
    peers: &PeerProfile,
    mode: Mode,
    selected_operator: Option<&operator::Selection>,
    credibility: Option<&Path>,
    history_options: Option<anchors::Options>,
    selected_governor: Option<&governor::Settings>,
    recovery_path: Option<&Path>,
    accounting: Option<andon::Selection<'_>>,
    accounting_inputs: &[&Path],
) -> Result<(Vec<Entry>, Option<anchors::Selection>), String> {
    let after = mode.after();
    if mode == Mode::ContinueGeneration
        && accounting.is_none()
        && (credibility.is_some()
            || selected_governor.is_some()
            || recovery_path.is_some()
            || !history_options
                .as_ref()
                .is_some_and(anchors::Options::is_recovery_pair))
    {
        return Err(USAGE.into());
    }
    if let Some(accounting) = accounting {
        accounting.check_mode(mode)?;
        if credibility.is_some() || selected_governor.is_some() || recovery_path.is_some() {
            return Err(andon::INVALID.into());
        }
    }
    governor::check_selected(
        selected_governor,
        selected_operator,
        after,
        credibility,
        recovery_path.is_some(),
    )?;
    if let Some(options) = &history_options {
        options.check_selected_mode(mode)?;
        options.check_files(&[path.to_path_buf()])?;
        if let Some(recovery) = recovery_path {
            options.check_files(&[recovery.to_path_buf()])?;
        }
    }
    super::requalification::check_mode(after, credibility)?;
    if after == Some(0) {
        return Err(
            "native sequence recovery requires a nonzero original message predecessor".into(),
        );
    }
    let base = path
        .parent()
        .ok_or("native sequence profile has no parent")?;
    let bytes = read_regular(path, MAX_PROFILE_BYTES)?;
    let plan = match mode {
        Mode::Create => decode(&bytes, base)?,
        Mode::AfterMessage(_) => decode_selected(&bytes, base, after)?,
        Mode::ContinueGeneration => decode_mode(&bytes, base, mode)?,
    };
    if let Some(accounting) = accounting {
        let mut paths = vec![path];
        for entry in &plan {
            paths.extend([entry.recipe.as_path(), entry.actor.as_path()]);
            paths.extend(entry.precheck.as_deref());
        }
        accounting.check_files(&paths)?;
    }
    if let Some(governor) = selected_governor {
        let mut paths = vec![path];
        for entry in &plan {
            paths.extend([entry.recipe.as_path(), entry.actor.as_path()]);
            paths.extend(entry.precheck.as_deref());
        }
        governor.check_custody_paths(&paths)?;
        if let Some(options) = &history_options {
            options.check_files(&[governor.socket.clone()])?;
        }
    }
    if let Some(options) = &history_options {
        let mut files = Vec::with_capacity(plan.len() * 3);
        for entry in &plan {
            files.extend([entry.recipe.clone(), entry.actor.clone()]);
            files.extend(entry.precheck.iter().cloned());
        }
        options.check_files(&files)?;
    }
    let actors = plan
        .iter()
        .map(|entry| Profile::read(&entry.actor))
        .collect::<Result<Vec<_>, _>>()?;
    validate_actors(config, &actors.iter().collect::<Vec<_>>(), peers, after)?;
    if let Some(accounting) = accounting {
        accounting.check_actors(
            config, &actors.iter().collect::<Vec<_>>(), peers, selected_operator,
        )?;
        let endpoints = accounting.endpoints(
            &actors.iter().collect::<Vec<_>>(),
            peers,
            selected_operator,
        )?;
        let mut files = accounting_inputs.to_vec();
        files.extend([path, config.source.path()]);
        for entry in &plan {
            files.extend([entry.recipe.as_path(), entry.actor.as_path()]);
            files.extend(entry.precheck.as_deref());
        }
        andon::check_endpoint_files(&endpoints, &files)?;
    }
    if let Some(selected) = selected_operator {
        for actor in &actors {
            selected.check(config, actor, peers)?;
        }
    }
    let prechecks = plan
        .iter()
        .map(|entry| {
            entry
                .precheck
                .as_deref()
                .map(exact_precheck::Selection::read)
                .transpose()
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Pending-generation recovery has no confirmed Message predecessor,
    // but still opens an existing owner at its actual post-Fence epoch.
    super::disclosure::check_initial_epoch(prechecks[0].as_ref(), mode.is_recovery())?;
    let accounting_endpoints = accounting.map(|accounting| {
        let endpoints = accounting.endpoints_with_prechecks(
            config,
            &actors.iter().collect::<Vec<_>>(),
            peers,
            selected_operator,
            prechecks.iter().filter_map(Option::as_ref),
        )?;
        let mut files = accounting_inputs.to_vec();
        files.extend([path, config.source.path()]);
        for entry in &plan {
            files.extend([entry.recipe.as_path(), entry.actor.as_path()]);
            files.extend(entry.precheck.as_deref());
        }
        andon::check_endpoint_files(&endpoints, &files)?;
        Ok::<_, String>(endpoints)
    }).transpose()?;
    if let Some(path) = recovery_path {
        governor::requirements::check_path(
            path,
            config,
            &actors
                .iter()
                .zip(&prechecks)
                .map(|(actor, selected)| (actor, selected.as_ref()))
                .collect::<Vec<_>>(),
            peers,
            selected_operator,
            selected_governor,
        )?;
    }
    super::disclosure::check(
        config,
        &actors
            .iter()
            .zip(&prechecks)
            .map(|(actor, selected)| (actor, selected.as_ref()))
            .collect::<Vec<_>>(),
        peers,
        selected_operator,
    )?;
    super::requalification::check_paths(
        config,
        &actors
            .iter()
            .zip(&prechecks)
            .map(|(actor, selected)| (actor, selected.as_ref()))
            .collect::<Vec<_>>(),
        peers,
        selected_operator,
        credibility,
    )?;
    let custody = actors
        .iter()
        .zip(&prechecks)
        .map(|(actor, selected)| (actor, selected.as_ref()))
        .collect::<Vec<_>>();
    if recovery_path.is_some() {
        governor::check_with_recovery(
            selected_governor,
            selected_operator,
            config,
            &custody,
            peers,
            after,
            credibility,
            None,
            true,
        )?;
    } else {
        governor::check(
            selected_governor,
            selected_operator,
            config,
            &custody,
            peers,
            after,
            credibility,
            None,
        )?;
    }
    let history = history_options
        .map(|options| {
            options.admit_mode(
                config,
                &actors
                    .iter()
                    .zip(&prechecks)
                    .map(|(actor, selected)| (actor, selected.as_ref()))
                    .collect::<Vec<_>>(),
                peers,
                selected_operator,
                mode,
                credibility,
            )
        })
        .transpose()?;
    if let Some(accounting) = accounting {
        let history = history.as_ref().ok_or(andon::INVALID)?;
        accounting.check_history(history)?;
        history.check_files(accounting_inputs)?;
    }
    let mut entries: Vec<Entry> = Vec::with_capacity(plan.len());
    let mut tokenizer = None;
    for ((selected, actor), precheck) in plan.into_iter().zip(actors).zip(prechecks) {
        let mut inputs = recipe::load_with_path_check(
            &selected.recipe,
            config.profile.delivery.scope.tenant,
            config.profile.delivery.limits.bytes,
            |path| {
                if let Some(governor) = selected_governor {
                    governor.check_custody_paths(&[path])?;
                }
                if let Some(accounting) = accounting {
                    accounting.check_files(&[path])?;
                }
                if let Some(endpoints) = &accounting_endpoints {
                    andon::check_endpoint_files(endpoints, &[path])?;
                }
                if let Some(history) = &history {
                    history.check_files(&[path])?;
                }
                Ok(())
            },
        )?;
        let canonical = inputs.tokenizer.to_bytes().map_err(debug)?;
        if let Some(original) = entries.first() {
            if inputs.decoder != original.inputs.decoder
                || inputs.stream != original.inputs.stream
                || tokenizer.as_ref() != Some(&canonical)
            {
                return Err("native sequence recipes must retain the exact original model, tokenizer and stream".into());
            }
            // Compatibility evidence does not create another live model. Keep
            // one original immutable allocation after the complete comparison.
            inputs.decoder = original.inputs.decoder.clone();
            inputs.tokenizer = original.inputs.tokenizer.clone();
        } else {
            tokenizer = Some(canonical);
        }
        entries.push(Entry {
            actor,
            inputs,
            release: selected.release,
            precheck,
        });
    }
    match mode {
        Mode::Create => validate(config, &entries, peers)?,
        Mode::AfterMessage(_) => validate_selected(config, &entries, peers, after)?,
        Mode::ContinueGeneration => validate_mode(config, &entries, peers, mode)?,
    }
    Ok((entries, history))
}

/// Direct callers receive the same complete admission checks as the manifest.
/// Requests, generation identities and per-entry policies remain independently
/// selected data, never authority to reuse an earlier publication approval.
pub(super) fn validate(
    config: &Config,
    entries: &[Entry],
    peers: &PeerProfile,
) -> Result<(), String> {
    validate_selected(config, entries, peers, None)
}

pub(super) fn validate_selected(
    config: &Config,
    entries: &[Entry],
    peers: &PeerProfile,
    after: Option<u64>,
) -> Result<(), String> {
    validate_mode(config, entries, peers, Mode::from_after(after))
}

pub(super) fn validate_mode(
    config: &Config,
    entries: &[Entry],
    peers: &PeerProfile,
    mode: Mode,
) -> Result<(), String> {
    let after = mode.after();
    check_count(entries.len(), mode)?;
    super::disclosure::check_initial_epoch(entries[0].precheck.as_ref(), mode.is_recovery())?;
    validate_actors(
        config,
        &entries.iter().map(|entry| &entry.actor).collect::<Vec<_>>(),
        peers,
        after,
    )?;
    super::disclosure::check(
        config,
        &entries
            .iter()
            .map(|entry| (&entry.actor, entry.precheck.as_ref()))
            .collect::<Vec<_>>(),
        peers,
        None,
    )?;
    let original = &entries[0].inputs;
    if original.decoder.profile().identity().tenant != config.profile.delivery.scope.tenant {
        return Err("native sequence model belongs to a different tenant".into());
    }
    let tokenizer = original.tokenizer.to_bytes().map_err(debug)?;
    let mut generations = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let inputs = &entry.inputs;
        if inputs.generation == 0 {
            return Err("native sequence recipes require a nonzero original generation ID".into());
        }
        if inputs.decoder != original.decoder
            || inputs.tokenizer.to_bytes().map_err(debug)? != tokenizer
            || inputs.stream != original.stream
        {
            return Err("native sequence recipes must retain the exact original model, tokenizer and stream".into());
        }
        match entry.release {
            Release::Message => {
                if !generations.insert(inputs.generation) {
                    return Err(
                        "native sequence Messages require distinct nonzero generation IDs".into(),
                    );
                }
            }
            Release::Finish => {
                if index + 1 != entries.len() || (index == 0 && after.is_none()) {
                    return Err(
                        "native sequence Finish must follow a Message and be the final entry"
                            .into(),
                    );
                }
                if index != 0 {
                    let previous = &entries[index - 1];
                    if !matches!(previous.release, Release::Message)
                        || inputs.generation != previous.inputs.generation
                        || inputs.request != previous.inputs.request
                    {
                        return Err("native sequence Finish requires the preceding Message's exact original recipe".into());
                    }
                }
                // A first remaining Finish binds the explicitly named native
                // history in finish::select; this manifest is not its receipt.
            }
        }
    }
    if generations.len() > original.stream.max_messages() {
        return Err("native sequence exceeds the original stream message allowance".into());
    }
    Ok(())
}

fn check_count(count: usize, mode: Mode) -> Result<(), String> {
    if mode.after() == Some(0) {
        return Err(
            "native sequence recovery requires a nonzero original message predecessor".into(),
        );
    }
    let minimum = if mode.is_recovery() { 1 } else { 2 };
    if !(minimum..=MAX_ENTRIES).contains(&count) {
        return Err(if mode.is_recovery() {
            "native sequence recovery requires 1..=16 independently selected remaining entries"
                .into()
        } else {
            "a native sequence requires 2..=16 independently selected entries".into()
        });
    }
    Ok(())
}

fn validate_actors(
    config: &Config,
    actors: &[&Profile],
    peers: &PeerProfile,
    after: Option<u64>,
) -> Result<(), String> {
    if !config.profile.delivery.initial_payload.is_empty() {
        return Err("a native text stream requires an explicitly empty initial payload".into());
    }
    peers.check_host(config)?;
    let store = operator::resolve(&config.store)?;
    let source = operator::resolve(config.source.path())?;
    let mut requests = BTreeSet::new();
    let mut sockets = BTreeSet::new();
    for actor in actors {
        if after == Some(actor.request) {
            return Err(
                "native sequence recovery predecessor cannot be a remaining actor request".into(),
            );
        }
        actor.check_host(config)?;
        if actor.request == 0 || !requests.insert(actor.request) {
            return Err("native sequence actor request IDs must be distinct and nonzero".into());
        }
        for path in [
            actor.socket.clone(),
            peers.socket(actor.request),
            crate::workflow::control::socket_path(peers, actor.request),
        ] {
            let resolved = operator::resolve(&path)?;
            if resolved.starts_with(&store) || resolved == source {
                return Err("native sequence endpoints must be outside the authority store and independent of source custody".into());
            }
            if !sockets.insert(resolved) {
                return Err(
                    "native sequence actor, review and Stop endpoints must be distinct".into(),
                );
            }
        }
    }
    Ok(())
}

fn selected_path(value: &str, base: &Path) -> Result<PathBuf, String> {
    let path = Path::new(value);
    if value.is_empty()
        || value.as_bytes().contains(&0)
        || path.file_name().is_none()
        || path
            .components()
            .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err("native sequence input paths must be normalized nonempty paths".into());
    }
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    })
}

fn object(value: Json) -> Result<BTreeMap<String, Json>, String> {
    match value {
        Json::Object(fields) => Ok(fields),
        _ => Err("expected native sequence object".into()),
    }
}

fn take(fields: &mut BTreeMap<String, Json>, name: &str) -> Result<Json, String> {
    fields
        .remove(name)
        .ok_or_else(|| format!("missing native sequence field {name}"))
}

fn text(fields: &mut BTreeMap<String, Json>, name: &str) -> Result<String, String> {
    match take(fields, name)? {
        Json::String(value) => Ok(value),
        _ => Err(format!("native sequence {name} must be text")),
    }
}

fn end(fields: BTreeMap<String, Json>) -> Result<(), String> {
    if fields.is_empty() {
        Ok(())
    } else {
        Err("unknown native sequence field".into())
    }
}

#[cfg(test)]
#[path = "profile/tests.rs"]
mod tests;
