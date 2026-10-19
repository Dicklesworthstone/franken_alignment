//! Closed, bounded independent intake for a resident learned sequence.
//! At most 16 recipes each retain the original 64 MiB asset ceiling: aggregate
//! recipe asset intake is at most 1 GiB, plus the separately bounded profile
//! documents. These admission reads never replenish live numerical, source,
//! predictor, topology or publication allowances.
use super::{Config, Entry, LearnedTextRelease, MAX_ENTRIES};
use super::super::{fields::Fields, recipe};
use super::super::super::Profile;
use crate::config::read_regular;
use std::path::{Path, PathBuf};

const MAX_PROFILE_BYTES: usize = 64 * 1024;
struct Input { recipe: PathBuf, actor: PathBuf, release: LearnedTextRelease }

fn decode(bytes: &[u8]) -> Result<Vec<Input>, String> {
    let mut fields = Fields::parse(bytes, MAX_PROFILE_BYTES)?;
    if fields.text("schema")? != "fa.learned-publication-sequence/1" {
        return Err("unsupported learned publication sequence schema".into());
    }
    let values = fields.array("entries", MAX_ENTRIES)?;
    fields.end()?;
    if values.len() < 2 { return Err("a learned sequence requires 2..=16 entries".into()); }
    let count = values.len();
    let mut entries = Vec::with_capacity(count);
    for (index, value) in values.into_iter().enumerate() {
        let mut fields = Fields::new(value)?;
        let recipe = fields.path("recipe")?;
        let actor = fields.path("actor_profile")?;
        let release = match fields.text("release")?.as_str() {
            "message" => LearnedTextRelease::Message,
            "finish" if index != 0 && index + 1 == count => LearnedTextRelease::Finish,
            _ => return Err("sequence entries must be Messages followed by an optional final Finish".into()),
        };
        fields.end()?;
        entries.push(Input { recipe, actor, release });
    }
    Ok(entries)
}

pub(super) fn load(path: &Path, config: &Config) -> Result<Vec<Entry>, String> {
    let plan = decode(&read_regular(path, MAX_PROFILE_BYTES)?)?;
    let mut entries: Vec<Entry> = Vec::with_capacity(plan.len());
    for input in plan {
        // All generation and helper artifacts use their original strict
        // loaders, before a store is opened or an actor can submit a request.
        let mut loaded = recipe::load(&input.recipe, config, true)?;
        let actor = Profile::read(&input.actor)?;
        if let Some(original) = entries.first() {
            if loaded.generation != original.loaded.generation || loaded.stream != original.loaded.stream {
                return Err("sequence recipes disagree on the complete original generation or stream".into());
            }
            // The equal later configuration is compatibility evidence only.
            // Share the exact immutable recipe; create no second live engine.
            loaded.generation = original.loaded.generation.clone();
        }
        entries.push(Entry { loaded, actor, release: input.release });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, release: &str) -> String {
        format!(r#"{{"recipe":"/operator/recipe-{id}.json","actor_profile":"/operator/actor-{id}.json","release":"{release}"}}"#)
    }

    fn document(entries: &[String]) -> String {
        format!(r#"{{"schema":"fa.learned-publication-sequence/1","entries":[{}]}}"#, entries.join(","))
    }

    #[test]
    fn sequence_manifest_selects_bounded_messages_and_explicit_last_finish() {
        for count in [2, MAX_ENTRIES] {
            let messages: Vec<_> = (0..count).map(|id| entry(id as u64 + 1, "message")).collect();
            let parsed = decode(document(&messages).as_bytes()).unwrap();
            assert_eq!(parsed.len(), count);
            assert!(parsed.iter().all(|entry| matches!(entry.release, LearnedTextRelease::Message)));
            let mut finished = messages;
            finished[count - 1] = entry(count as u64, "finish");
            let parsed = decode(document(&finished).as_bytes()).unwrap();
            assert!(matches!(parsed[count - 1].release, LearnedTextRelease::Finish));
            assert_eq!(parsed[0].recipe, Path::new("/operator/recipe-1.json"));
        }
    }

    #[test]
    fn sequence_manifest_refuses_missing_actor_selection_extra_grants_and_unbounded_or_early_finish() {
        for entries in [vec![], vec![entry(1, "message")],
            (0..=MAX_ENTRIES).map(|id| entry(id as u64 + 1, "message")).collect(),
            vec![entry(1, "finish"), entry(2, "message")],
            vec![entry(1, "message"), entry(2, "finish"), entry(3, "message")],
            vec![entry(1, "message"), entry(2, "continue")]] {
            assert!(decode(document(&entries).as_bytes()).is_err());
        }
        let valid = document(&[entry(1, "message"), entry(2, "finish")]);
        for invalid in [
            valid.replace("/operator/actor-1.json", "../actor.json"),
            valid.replacen(r#""actor_profile":"/operator/actor-1.json","#, "", 1),
            valid.replacen(r#""release":"message""#, r#""release":"message","automatic_human_approval":true"#, 1),
            valid.replacen(r#""entries":["#, r#""requests":1000,"entries":["#, 1),
            valid.replace("fa.learned-publication-sequence/1", "fa.learned-publication-sequence/2"),
        ] {
            assert!(decode(invalid.as_bytes()).is_err());
        }
    }
}
