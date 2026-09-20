//! Typed source observations for operators that do not produce Rust wire packets.
//! This is a lossless adapter into the ORIGINAL validated native input types.
//! A declared/admitted close is an operator assertion, never authentication here.
use super::input::{Fields, Observation, debug, read_regular, unhex};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::delivery::persistent::observed::publication::witnesses::{
    FilePublicationInputs, FileWitnessInput, MAX_PUBLICATION_PACKET_BYTES,
};
use fa_reference::full_input::{
    ActualHelperInput, ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart,
    MAX_OMISSIONS, MAX_PROFILE_BYTES, MAX_SUBMITTED_BYTES, MAX_SUBMITTED_PARTS,
};
use fa_reference::product_frontier::{
    FrontierStage, ProductFrontiers, ProjectionKey, TrustedClosingMarker,
};
use fa_reference::strict_json::{self, Json, Limits};
use fa_reference::witness::{
    AdapterDomainInput, DomainClosure, DomainProjection, SnapshotEntry,
    MAX_SNAPSHOT_ENTRIES, MAX_VALUE_BYTES,
};
use std::path::Path;

// Enough for both bounded native lanes, hex expansion, and explicit metadata.
// Array/field limits and each native constructor impose their own tighter bounds.
const SOURCE_BYTES: usize = 2 * MAX_PUBLICATION_PACKET_BYTES + 65_536;

pub(super) fn read(path: &Path) -> Result<Observation, String> {
    decode(&read_regular(path, SOURCE_BYTES)?)
}

pub(super) fn decode(bytes: &[u8]) -> Result<Observation, String> {
    let json = strict_json::parse(bytes, Limits {
        max_bytes: SOURCE_BYTES, max_depth: 12, max_items: 8192,
        max_string_bytes: 2 * MAX_SUBMITTED_BYTES,
    }).map_err(debug)?;
    let mut f = Fields::object(json)?;
    if f.text("schema")? != "fa.publication-source/1" {
        return Err("unsupported publication source document".into());
    }
    let expected_generation = f.number("expected_generation")?;
    expected_generation.checked_add(1).ok_or("producer generation overflow")?;
    let observed_at = ElapsedTick(f.number("observed_at_unix_ms")?);
    // Both keys are REQUIRED. Explicit null means unavailable, never permission
    // to remove a previously bound consumer dependency or infer a closed domain.
    let structured = nullable(f.take("structured")?, structured)?;
    let opaque = nullable(f.take("opaque")?, opaque)?;
    f.end()?;
    let inputs = FilePublicationInputs::new(structured, opaque);
    // Enforce the combined native wire ceiling before any owner is opened.
    inputs.to_bytes().map_err(debug)?;
    Ok(Observation { expected_generation, observed_at, inputs })
}

fn nullable<T>(value: Json, decode: fn(Json) -> Result<T, String>) -> Result<Option<T>, String> {
    match value { Json::Null => Ok(None), value => decode(value).map(Some) }
}

fn key(value: Json) -> Result<ProjectionKey, String> {
    let mut f = Fields::object(value)?;
    let result = ProjectionKey { source: f.number("source")?, branch: f.number("branch")?,
        projection: f.number("projection")?, source_epoch: f.number("source_epoch")? };
    f.end()?;
    Ok(result)
}

fn marker(value: Json) -> Result<TrustedClosingMarker, String> {
    let mut f = Fields::object(value)?;
    let result = TrustedClosingMarker { key: key(f.take("key")?)?,
        final_sequence: f.number("final_sequence")?, marker_generation: f.number("marker_generation")? };
    f.end()?;
    Ok(result)
}

fn closure(value: Json) -> Result<DomainClosure, String> {
    let mut f = Fields::object(value)?;
    let result = match f.text("kind")?.as_str() {
        "unknown" => DomainClosure::Unknown,
        "conservative_summary" => DomainClosure::ConservativeSummary,
        "closed" => DomainClosure::Closed(marker(f.take("marker")?)?),
        _ => return Err("unsupported domain closure".into()),
    };
    f.end()?;
    Ok(result)
}

fn structured(value: Json) -> Result<FileWitnessInput, String> {
    let mut f = Fields::object(value)?;
    let revision = f.number("revision")?;
    let control_cut = f.number("control_cut")?;
    let semantic_epoch = f.number("semantic_epoch")?;
    let mut d = Fields::object(f.take("domain")?)?;
    let domain = DomainProjection::new(d.number("domain_id")?, d.number("domain_epoch")?,
        key(d.take("projection")?)?);
    d.end()?;
    let declared = closure(f.take("closure")?)?;
    let admitted = nullable(f.take("admitted_close")?, marker)?;
    let raw = f.array("entries", MAX_SNAPSHOT_ENTRIES)?;
    f.end()?;
    let mut entries = Vec::new();
    entries.try_reserve_exact(raw.len()).map_err(debug)?;
    for item in raw {
        let mut e = Fields::object(item)?;
        let key = e.number("key")?;
        let version = e.number("version")?;
        let value = unhex(&e.text("value_hex")?, MAX_VALUE_BYTES)?;
        e.end()?;
        entries.push(SnapshotEntry::new(key, version, value).map_err(debug)?);
    }
    // Reconstruct only an EXPLICITLY supplied independently admitted close, as
    // native file replay does. Never use the snapshot's own closure assertion as
    // its admitted frontier. Unknown/mismatching observations remain restrictive.
    let mut frontiers = ProductFrontiers::new(1, 1).map_err(debug)?;
    if let Some(admitted) = admitted {
        if admitted.key != domain.projection() {
            return Err("admitted close belongs to another source projection".into());
        }
        if admitted.final_sequence != 0 {
            frontiers.accept_contiguous(admitted.key, FrontierStage::Authenticated,
                1, admitted.final_sequence).map_err(debug)?;
        }
        frontiers.record_close(admitted).map_err(debug)?;
    }
    FileWitnessInput::new(revision, control_cut, semantic_epoch,
        AdapterDomainInput::new(domain, declared), entries, &frontiers).map_err(debug)
}

fn opaque(value: Json) -> Result<ActualHelperInput, String> {
    let mut f = Fields::object(value)?;
    let bytes = unhex(&f.text("submitted_hex")?, MAX_SUBMITTED_BYTES)?;
    let mut p = Fields::object(f.take("input_profile")?)?;
    let profile = InputProfileBinding {
        profile_id: p.number("profile_id")?, profile_bytes: unhex(&p.text("profile_hex")?, MAX_PROFILE_BYTES)?,
        tokenizer_epoch: p.number("tokenizer_epoch")?, policy_epoch: p.number("policy_epoch")?,
        model_epoch: p.number("model_epoch")?,
    };
    p.end()?;
    let raw = f.array("parts", MAX_SUBMITTED_PARTS)?;
    let mut parts = Vec::new();
    parts.try_reserve_exact(raw.len()).map_err(debug)?;
    for item in raw {
        let mut p = Fields::object(item)?;
        let start = usize::try_from(p.number("start")?).map_err(debug)?;
        let end = usize::try_from(p.number("end")?).map_err(debug)?;
        let kind = match p.text("kind")?.as_str() {
            "question" => PartKind::Question,
            "prompt" => PartKind::Prompt,
            "instruction" => PartKind::Instruction,
            "tool_schema" => PartKind::ToolSchema { schema_id: p.number("schema_id")? },
            "evidence" => PartKind::Evidence { source_id: p.number("source_id")?, transform_id: p.number("transform_id")? },
            "delimiter" => PartKind::Delimiter,
            "other" => PartKind::Other,
            _ => return Err("unsupported submitted part kind".into()),
        };
        p.end()?;
        parts.push(SubmittedPart { span: ByteSpan { start, end }, kind });
    }
    let raw = f.array("omissions", MAX_OMISSIONS)?;
    f.end()?;
    let mut omissions = Vec::new();
    omissions.try_reserve_exact(raw.len()).map_err(debug)?;
    for item in raw {
        let mut o = Fields::object(item)?;
        let domain_id = o.number("domain_id")?;
        let omission = match o.text("kind")?.as_str() {
            "closed_absent" => Omission::ClosedAbsent { domain_id,
                trusted_closure_marker_id: o.number("trusted_closure_marker_id")? },
            "gapped" => Omission::Gapped { domain_id, first_missing: o.number("first_missing")? },
            "unsupported" => Omission::Unsupported { domain_id },
            "redacted" => Omission::Redacted { domain_id, transform_id: o.number("transform_id")? },
            _ => return Err("unsupported omission kind".into()),
        };
        o.end()?;
        omissions.push(omission);
    }
    // The native constructor checks the COMPLETE byte partition, exactly one
    // question, unique omitted domains and all original resource bounds.
    ActualHelperInput::new(bytes, profile, parts, omissions).map_err(debug)
}
