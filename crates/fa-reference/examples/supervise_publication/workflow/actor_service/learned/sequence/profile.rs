//! Closed, bounded independent intake for a resident learned sequence.
//! At most 16 recipes each retain the original 64 MiB asset ceiling: aggregate
//! recipe asset intake is at most 1 GiB, plus the separately bounded profile
//! documents. These admission reads never replenish live numerical, source,
//! predictor, topology or publication allowances.
use super::{Config, Entry, LearnedTextRelease, MAX_ENTRIES};
use super::super::{fields::Fields, recipe};
use super::super::super::Profile;
use crate::config::read_regular;
use fa_reference::strict_json::Json;
use std::path::{Path, PathBuf};

const MAX_PROFILE_BYTES: usize = 64 * 1024;
struct Input { recipe: PathBuf, actor: PathBuf, release: LearnedTextRelease,
    precheck: Option<PathBuf> }

fn decode(bytes: &[u8]) -> Result<Vec<Input>, String> {
    let mut fields = Fields::parse(bytes, MAX_PROFILE_BYTES)?;
    let with_prechecks = match fields.text("schema")?.as_str() {
        "fa.learned-publication-sequence/1" => false,
        "fa.learned-publication-sequence/2" => true,
        _ => return Err("unsupported learned publication sequence schema".into()),
    };
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
        // Version one stays closed and byte-compatible. Version two requires
        // an explicit per-entry choice; absence cannot silently erase intent.
        let precheck = if with_prechecks {
            match fields.take("precheck_profile")? {
                Json::Null => None,
                Json::String(path) => Some(super::super::fields::path(&path)?),
                _ => return Err("sequence precheck_profile must be an absolute path or null".into()),
            }
        } else { None };
        fields.end()?;
        entries.push(Input { recipe, actor, release, precheck });
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
        let precheck = input.precheck.as_deref()
            .map(super::super::precheck::Settings::read).transpose()?;
        if let Some(original) = entries.first() {
            if loaded.generation != original.loaded.generation || loaded.stream != original.loaded.stream {
                return Err("sequence recipes disagree on the complete original generation or stream".into());
            }
            // The equal later configuration is compatibility evidence only.
            // Share the exact immutable recipe; create no second live engine.
            loaded.generation = original.loaded.generation.clone();
        }
        entries.push(Entry { loaded, actor, release: input.release, precheck });
    }
    Ok(entries)
}

/// Admit every original recipe's supported mode before the first model/helper
/// asset is read. The same frozen documents then enter the ordinary loaders.
pub(super) fn load_governed(
    path: &Path,
    config: &Config,
    governor: &super::governor::Settings,
    operator: &super::super::operator::Selection,
    peers: &super::PeerProfile,
    continuing: bool,
) -> Result<Vec<Entry>, String> {
    let plan = decode(&read_regular(path, MAX_PROFILE_BYTES)?)?;
    if !continuing && plan
        .iter()
        .filter(|input| input.release == LearnedTextRelease::Message)
        .count()
        < 2
    {
        return Err("live learned governor requires at least two selected Messages".into());
    }
    let prepared = plan
        .into_iter()
        .map(|input| {
            let recipe = if continuing {
                recipe::GovernedRecipe::read_recovery(
                    &input.recipe,
                    governor.campaign_requirement(),
                )?
            } else {
                recipe::GovernedRecipe::read(
                    &input.recipe,
                    config,
                    governor.campaign_requirement(),
                )?
            };
            let actor = Profile::read(&input.actor)?;
            let precheck = input
                .precheck
                .as_deref()
                .map(super::super::precheck::Settings::read)
                .transpose()?;
            Ok((input.release, recipe, actor, precheck))
        })
        .collect::<Result<Vec<_>, String>>()?;
    if continuing
        && prepared.windows(2).any(|pair| {
            pair[0].1.expected_policy() != pair[1].1.expected_policy()
        })
    {
        return Err("every governed recovery recipe must select the same complete effective policy".into());
    }
    let requests = prepared
        .iter()
        .map(|(_, _, actor, _)| actor.request)
        .collect::<Vec<_>>();
    for (_, _, actor, _) in &prepared {
        governor.check_learned_host(config, actor, peers, &operator.settings, &requests)?;
    }
    let mut entries: Vec<Entry> = Vec::with_capacity(prepared.len());
    for (release, recipe, actor, precheck) in prepared {
        let mut loaded = recipe.load(config)?;
        if let Some(original) = entries.first() {
            if loaded.generation != original.loaded.generation
                || loaded.stream != original.loaded.stream
            {
                return Err(
                    "sequence recipes disagree on the complete original generation or stream"
                        .into(),
                );
            }
            loaded.generation = original.loaded.generation.clone();
        }
        entries.push(Entry {
            loaded,
            actor,
            release,
            precheck,
        });
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

    fn selected(id: u64, release: &str, precheck: &str) -> String {
        let original = entry(id, release);
        format!(r#"{},"precheck_profile":{precheck}}}"#, original.trim_end_matches('}'))
    }

    #[test]
    fn resident_precheck_manifest_keeps_each_explicit_choice_and_the_original_bounds() {
        let entries = [selected(1, "message", r#""/operator/precheck-1.json""#),
            selected(2, "message", "null"), selected(3, "finish", r#""/operator/precheck-3.json""#)];
        let selected = document(&entries).replace("sequence/1", "sequence/2");
        let parsed = decode(selected.as_bytes()).unwrap();
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].precheck.as_deref(), Some(Path::new("/operator/precheck-1.json")));
        assert_eq!(parsed[1].precheck, None);
        assert_eq!(parsed[2].precheck.as_deref(), Some(Path::new("/operator/precheck-3.json")));
        assert!(matches!(parsed[2].release, LearnedTextRelease::Finish));
        assert!(decode(selected.replace("sequence/2", "sequence/1").as_bytes()).is_err(),
            "the original closed grammar cannot silently acquire a disclosure");
        let original = decode(document(&[entry(1, "message"), entry(2, "finish")]).as_bytes()).unwrap();
        assert!(original.iter().all(|entry| entry.precheck.is_none()));
    }

    #[test]
    fn resident_precheck_manifest_refuses_implicit_ambiguous_or_unbounded_selection() {
        let valid = document(&[selected(1, "message", r#""/operator/precheck.json""#),
            selected(2, "finish", "null")]).replace("sequence/1", "sequence/2");
        for invalid in [
            valid.replace(r#","precheck_profile":null"#, ""),
            valid.replace(r#""precheck_profile":null"#, r#""precheck_profile":true"#),
            valid.replace(r#""precheck_profile":null"#, r#""precheck_profile":{}"#),
            valid.replace("/operator/precheck.json", "../precheck.json"),
            valid.replace("/operator/precheck.json", "/operator/../precheck.json"),
            valid.replace(r#""precheck_profile":null"#, r#""precheck_profile":null,"precheck_profile":null"#),
            valid.replace(r#""precheck_profile":null"#, r#""precheck_profile":null,"reuse_previous_disclosure":true"#),
            valid.replace("sequence/2", "sequence/3"),
        ] {
            assert!(decode(invalid.as_bytes()).is_err(), "accepted {invalid}");
        }
        let too_many = (0..=MAX_ENTRIES).map(|id| selected(id as u64 + 1, "message", "null"))
            .collect::<Vec<_>>();
        assert!(decode(document(&too_many).replace("sequence/1", "sequence/2").as_bytes()).is_err());
        let early_finish = [selected(1, "finish", "null"), selected(2, "message", "null")];
        assert!(decode(document(&early_finish).replace("sequence/1", "sequence/2").as_bytes()).is_err());
    }
}
