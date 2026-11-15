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
const USAGE: &str = "serve-native-generated CONFIG SEQUENCE_PROFILE REVIEWER_PROFILE [--publication-profile PROFILE]; creation only, with 2..=16 explicit Message/Finish entries";

struct Input {
    recipe: PathBuf,
    actor: PathBuf,
    release: Release,
}

/// Own the complete grammar before opening any input. Existing native recovery,
/// operator and disclosure selections cannot be silently consumed by this mode.
fn options(args: &[String], credibility: Option<&Path>) -> Result<Option<PathBuf>, String> {
    if credibility.is_some()
        || !matches!(args.len(), 4 | 6)
        || args.first().map(String::as_str) != Some("serve-native-generated")
        || args[1..4]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    match args.len() {
        4 => Ok(None),
        6 if args[4] == "--publication-profile"
            && !args[5].is_empty()
            && !args[5].starts_with("--") =>
        {
            Ok(Some(PathBuf::from(&args[5])))
        }
        _ => Err(USAGE.into()),
    }
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
    let publication_path = options(args, credibility)?;
    let config = Config::read(Path::new(&args[1]))?;
    let peers = PeerProfile::read(Path::new(&args[3]))?;
    let publication = publication_path
        .as_deref()
        .map(PublicationProfile::read)
        .transpose()?;
    let entries = load(Path::new(&args[2]), &config, &peers)?;
    super::run(config, entries, &peers, publication.as_ref(), time, output)
}

fn decode(bytes: &[u8], base: &Path) -> Result<Vec<Input>, String> {
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
    if !(2..=MAX_ENTRIES).contains(&values.len()) {
        return Err("a native sequence requires 2..=16 independently selected entries".into());
    }
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
                "finish" if index != 0 && index + 1 == count => Release::Finish,
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
    let base = path
        .parent()
        .ok_or("native sequence profile has no parent")?;
    let plan = decode(&read_regular(path, MAX_PROFILE_BYTES)?, base)?;
    let actors = plan
        .iter()
        .map(|entry| Profile::read(&entry.actor))
        .collect::<Result<Vec<_>, _>>()?;
    validate_actors(config, &actors.iter().collect::<Vec<_>>(), peers)?;
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
    validate(config, &entries, peers)?;
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
    if !(2..=MAX_ENTRIES).contains(&entries.len()) {
        return Err("a native sequence requires 2..=16 independently selected entries".into());
    }
    validate_actors(
        config,
        &entries.iter().map(|entry| &entry.actor).collect::<Vec<_>>(),
        peers,
    )?;
    let original = &entries[0].inputs;
    if original.decoder.profile().identity().tenant != config.profile.delivery.scope.tenant {
        return Err("native sequence model belongs to a different tenant".into());
    }
    let tokenizer = original.tokenizer.to_bytes().map_err(debug)?;
    let mut generations = BTreeSet::new();
    for (index, entry) in entries.iter().enumerate() {
        let inputs = &entry.inputs;
        if inputs.decoder != original.decoder
            || inputs.tokenizer.to_bytes().map_err(debug)? != tokenizer
            || inputs.stream != original.stream
        {
            return Err("native sequence recipes must retain the exact original model, tokenizer and stream".into());
        }
        match entry.release {
            Release::Message => {
                if inputs.generation == 0 || !generations.insert(inputs.generation) {
                    return Err(
                        "native sequence Messages require distinct nonzero generation IDs".into(),
                    );
                }
            }
            Release::Finish => {
                if index == 0 || index + 1 != entries.len() {
                    return Err(
                        "native sequence Finish must follow a Message and be the final entry"
                            .into(),
                    );
                }
                let previous = &entries[index - 1];
                if !matches!(previous.release, Release::Message)
                    || inputs.generation != previous.inputs.generation
                    || inputs.request != previous.inputs.request
                {
                    return Err("native sequence Finish requires the preceding Message's exact original recipe".into());
                }
            }
        }
    }
    if generations.len() > original.stream.max_messages() {
        return Err("native sequence exceeds the original stream message allowance".into());
    }
    Ok(())
}

fn validate_actors(
    config: &Config,
    actors: &[&Profile],
    peers: &PeerProfile,
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
