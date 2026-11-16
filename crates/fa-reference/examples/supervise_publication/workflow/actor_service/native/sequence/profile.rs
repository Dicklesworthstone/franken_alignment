//! Closed selection for one resident native-text owner. Every recipe is read
//! under its original per-recipe asset ceiling; the fixed entry count bounds
//! aggregate intake by sixteen such allowances. Matching immutable model and
//! tokenizer inputs share the first recipe's retained allocations afterward.

use super::super::{PublicationProfile, debug, operator, recipe};
use super::{Config, Entry, MAX_ENTRIES, PeerProfile, Profile, Release};
use crate::config::read_regular;
use fa_reference::action::ElapsedTick;
use fa_reference::strict_json::{self, Json, Limits};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

const MAX_PROFILE_BYTES: usize = 64 * 1024;
const USAGE: &str = "serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE [--publication-profile PROFILE] [--after-message NONZERO] [--operator-profile PROFILE --tower-generation NONZERO]; creation requires 2..=16 entries; explicit recovery accepts 1..=16 new remaining Message/Finish entries";

struct Input {
    recipe: PathBuf,
    actor: PathBuf,
    release: Release,
}

/// Own the complete grammar before opening any input. Recovery requires an
/// explicit original message predecessor; other native lifecycle and
/// disclosure selections cannot be silently consumed by this mode.
fn options(
    args: &[String],
    credibility: Option<&Path>,
) -> Result<(Option<PathBuf>, Option<u64>), String> {
    if credibility.is_some()
        || !matches!(args.len(), 4 | 6 | 8)
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
    if credibility.is_some()
        || args.len() < 8
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
    Ok((
        remaining,
        Some((profile.ok_or(USAGE)?, generation.ok_or(USAGE)?)),
    ))
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
    let (args, operator_option) = take_operator(args, credibility)?;
    let (publication_path, after) = options(&args, credibility)?;
    let config = Config::read(Path::new(&args[1]))?;
    let peers = PeerProfile::read(Path::new(&args[3]))?;
    let selected_operator = operator_option
        .map(|(path, generation)| operator::Selection::read(&path, generation))
        .transpose()?;
    let publication = publication_path
        .as_deref()
        .map(PublicationProfile::read)
        .transpose()?;
    if let Some(selected) = selected_operator.as_ref() {
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
    if text(&mut root, "schema")? != "fa.native-publication-sequence/1" {
        return Err("unsupported native publication sequence schema".into());
    }
    let Json::Array(values) = take(&mut root, "entries")? else {
        return Err("native sequence entries must be an array".into());
    };
    end(root)?;
    check_count(values.len(), after)?;
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
            end(fields)?;
            Ok(Input { recipe, actor, release })
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
    if after == Some(0) {
        return Err(
            "native sequence recovery requires a nonzero original message predecessor".into(),
        );
    }
    let base = path
        .parent()
        .ok_or("native sequence profile has no parent")?;
    let bytes = read_regular(path, MAX_PROFILE_BYTES)?;
    let plan = match after {
        Some(_) => decode_selected(&bytes, base, after)?,
        None => decode(&bytes, base)?,
    };
    let actors = plan
        .iter()
        .map(|entry| Profile::read(&entry.actor))
        .collect::<Result<Vec<_>, _>>()?;
    validate_actors(config, &actors.iter().collect::<Vec<_>>(), peers, after)?;
    if let Some(selected) = selected_operator {
        for actor in &actors {
            selected.check(config, actor, peers)?;
        }
    }
    let mut entries: Vec<Entry> = Vec::with_capacity(plan.len());
    let mut tokenizer = None;
    for (selected, actor) in plan.into_iter().zip(actors) {
        let mut inputs = recipe::load(
            &selected.recipe,
            config.profile.delivery.scope.tenant,
            config.profile.delivery.limits.bytes,
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
        });
    }
    match after {
        Some(_) => validate_selected(config, &entries, peers, after)?,
        None => validate(config, &entries, peers)?,
    }
    Ok(entries)
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
    check_count(entries.len(), after)?;
    validate_actors(
        config,
        &entries.iter().map(|entry| &entry.actor).collect::<Vec<_>>(),
        peers,
        after,
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

fn check_count(count: usize, after: Option<u64>) -> Result<(), String> {
    if after == Some(0) {
        return Err(
            "native sequence recovery requires a nonzero original message predecessor".into(),
        );
    }
    let minimum = if after.is_some() { 1 } else { 2 };
    if !(minimum..=MAX_ENTRIES).contains(&count) {
        return Err(if after.is_some() {
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
mod tests;
