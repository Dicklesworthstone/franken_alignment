//! Strict negotiation of static positional semantics before weight access.
use super::{CheckpointError, ConfigIssue, Json, RotaryScaling, config_error, number};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn configuration(root: &BTreeMap<String, Json>, defaults: &mut BTreeSet<String>,
    trained_context: usize) -> Result<(f64, RotaryScaling), CheckpointError>
{
    let legacy_theta = root.get("rope_theta").map(|value| number("rope_theta", value)).transpose()?;
    let legacy = root.get("rope_scaling").filter(|value| !value.is_null())
        .map(|value| parameters(value, "rope_scaling", false, defaults, trained_context)).transpose()?;
    let modern = root.get("rope_parameters").filter(|value| !value.is_null())
        .map(|value| parameters(value, "rope_parameters", true, defaults, trained_context)).transpose()?;
    if let (Some((_, old)), Some((_, new))) = (&legacy, &modern) {
        if old != new { return Err(config_error("rope_scaling", ConfigIssue::Unsupported)); }
    }
    let theta = match modern.as_ref().and_then(|(theta, _)| *theta) {
        Some(theta) => {
            if legacy_theta.is_some_and(|old| old.to_bits() != theta.to_bits()) {
                return Err(config_error("rope_theta", ConfigIssue::Unsupported));
            }
            theta
        }
        None => legacy_theta.unwrap_or_else(|| { defaults.insert("rope_theta".to_owned()); 10000.0 }),
    };
    let scaling = modern.or(legacy).map_or(RotaryScaling::None, |(_, scaling)| scaling);
    Ok((theta, scaling))
}

fn parameters(value: &Json, container: &str, modern: bool,
    defaults: &mut BTreeSet<String>, trained_context: usize)
    -> Result<(Option<f64>, RotaryScaling), CheckpointError>
{
    let object = value.as_object().ok_or_else(|| config_error(container, ConfigIssue::Type))?;
    let field = |name: &str| format!("{container}.{name}");
    let required = |name: &str| object.get(name)
        .ok_or_else(|| config_error(&field(name), ConfigIssue::Missing));
    let numeric = |name: &str| number(&field(name), required(name)?);
    let text = |name: &str| object.get(name).map(|value| value.as_str()
        .ok_or_else(|| config_error(&field(name), ConfigIssue::Type))).transpose();
    let current = text("rope_type")?;
    let old = if modern { None } else { text("type")? };
    if current.zip(old).is_some_and(|(current, old)| current != old) {
        return Err(config_error(&field("rope_type"), ConfigIssue::Unsupported));
    }
    let kind = match current.or(old) {
        Some(kind) => kind,
        None if modern => { defaults.insert(field("rope_type")); "default" },
        None => return Err(config_error(&field("rope_type"), ConfigIssue::Missing)),
    };
    let fields: &[&str] = match kind {
        "default" => &[],
        "linear" => &["factor"],
        "llama3" => &["factor", "low_freq_factor", "high_freq_factor", "original_max_position_embeddings"],
        _ => return Err(config_error(&field("rope_type"), ConfigIssue::Unsupported)),
    };
    if object.keys().any(|key| key != "rope_type" && !(modern && key == "rope_theta")
        && !(!modern && key == "type") && !fields.contains(&key.as_str()))
    { return Err(config_error(container, ConfigIssue::Unsupported)); }
    let theta = if modern { Some(numeric("rope_theta")?) } else { None };
    let scaling = match kind {
        "default" => RotaryScaling::None,
        "linear" => RotaryScaling::linear(numeric("factor")?)
            .map_err(|_| config_error(&field("factor"), ConfigIssue::Unsupported))?,
        "llama3" => {
            let original = required("original_max_position_embeddings")?.as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .ok_or_else(|| config_error(&field("original_max_position_embeddings"), ConfigIssue::Type))?;
            // The caller may choose a shorter execution window; this compares
            // the checkpoint's two declared context limits, never that window.
            if original == 0 || original > trained_context {
                return Err(config_error(&field("original_max_position_embeddings"), ConfigIssue::Unsupported));
            }
            RotaryScaling::llama3(numeric("factor")?, numeric("low_freq_factor")?,
                numeric("high_freq_factor")?, original)
                .map_err(|_| config_error(container, ConfigIssue::Unsupported))?
        }
        _ => return Err(config_error(&field("rope_type"), ConfigIssue::Unsupported)),
    };
    Ok((theta, scaling))
}
