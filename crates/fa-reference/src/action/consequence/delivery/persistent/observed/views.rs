//! Lossless journal encoding of the ORIGINAL validated helper-view types.
//! Submitted bytes are retained, never reconstructed from provenance metadata.
use super::super::codec::shared::{Reader, Writer};
use crate::evidence_view::{
    AuthorizationProjection, EvidencePartView, EvidenceViewManifest, OriginalIdentity,
    RedactionMetadata, WindowMetadata, MAX_EVIDENCE_PART_BINDINGS, MAX_PROJECTED_ORIGINALS,
};
use crate::full_input::{
    ActualHelperInput, ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart,
    MAX_OMISSIONS, MAX_PROFILE_BYTES, MAX_SUBMITTED_BYTES, MAX_SUBMITTED_PARTS,
};
use crate::reducer::{MAX_IDENTIFIER_BYTES, MAX_VOTES};
use crate::Error;
use std::collections::BTreeMap;

pub(super) type Views = BTreeMap<String, EvidenceViewManifest>;

pub(super) fn write_profile(w: &mut Writer, p: &InputProfileBinding) -> Result<(), Error> {
    for value in [p.profile_id, p.tokenizer_epoch, p.policy_epoch, p.model_epoch] { w.u64(value)?; }
    w.blob(&p.profile_bytes)
}
fn read_profile(r: &mut Reader<'_>) -> Result<InputProfileBinding, Error> {
    Ok(InputProfileBinding { profile_id: r.u64()?, tokenizer_epoch: r.u64()?,
        policy_epoch: r.u64()?, model_epoch: r.u64()?, profile_bytes: r.blob(MAX_PROFILE_BYTES)?.to_vec() })
}
pub(super) fn write_name(w: &mut Writer, name: &str) -> Result<(), Error> {
    if name.is_empty() { return Err(Error::InvalidInput); }
    if name.len() > MAX_IDENTIFIER_BYTES { return Err(Error::Limit); }
    w.blob(name.as_bytes())
}
pub(super) fn read_name(r: &mut Reader<'_>) -> Result<String, Error> {
    let name = std::str::from_utf8(r.blob(MAX_IDENTIFIER_BYTES)?).map_err(|_| Error::InvalidInput)?;
    if name.is_empty() { return Err(Error::InvalidInput); }
    Ok(name.to_owned())
}
fn write_original(w: &mut Writer, id: OriginalIdentity) -> Result<(), Error> {
    w.u64(id.tenant_id)?; w.u64(id.object_id)?; w.u64(id.generation)
}
fn read_original(r: &mut Reader<'_>) -> Result<OriginalIdentity, Error> {
    Ok(OriginalIdentity { tenant_id: r.u64()?, object_id: r.u64()?, generation: r.u64()? })
}

pub(super) fn write(w: &mut Writer, views: &Views) -> Result<(), Error> {
    if views.len() > MAX_VOTES { return Err(Error::Limit); }
    w.count(views.len())?;
    for (member, view) in views {
        write_name(w, member)?;
        let input = view.actual_input();
        w.blob(input.submitted_bytes())?;
        write_profile(w, input.input_profile())?;
        w.count(input.ordered_parts().len())?;
        for part in input.ordered_parts() {
            w.count(part.span.start)?; w.count(part.span.end)?;
            match part.kind {
                PartKind::Question => w.u8(0)?,
                PartKind::Prompt => w.u8(1)?,
                PartKind::Instruction => w.u8(2)?,
                PartKind::ToolSchema { schema_id } => { w.u8(3)?; w.u64(schema_id)?; }
                PartKind::Evidence { source_id, transform_id } => { w.u8(4)?; w.u64(source_id)?; w.u64(transform_id)?; }
                PartKind::Delimiter => w.u8(5)?,
                PartKind::Other => w.u8(6)?,
            }
        }
        w.count(input.omissions().len())?;
        for omission in input.omissions() {
            match omission {
                Omission::ClosedAbsent { domain_id, trusted_closure_marker_id } => {
                    w.u8(0)?; w.u64(*domain_id)?; w.u64(*trusted_closure_marker_id)?;
                }
                Omission::Gapped { domain_id, first_missing } => { w.u8(1)?; w.u64(*domain_id)?; w.u64(*first_missing)?; }
                Omission::Unsupported { domain_id } => { w.u8(2)?; w.u64(*domain_id)?; }
                Omission::Redacted { domain_id, transform_id } => { w.u8(3)?; w.u64(*domain_id)?; w.u64(*transform_id)?; }
            }
        }
        let projection = view.authorization();
        w.u64(projection.projection_id)?; w.u64(projection.policy_epoch)?;
        w.count(projection.projected_originals.len())?;
        for id in &projection.projected_originals { write_original(w, *id)?; }
        w.count(view.evidence_parts().len())?;
        for part in view.evidence_parts() {
            w.count(part.input_part_index)?;
            write_original(w, part.original)?;
            w.u64(part.transform_id)?;
            match part.redaction {
                RedactionMetadata::None => w.u8(0)?,
                RedactionMetadata::DeclaredTransform { transform_id } => { w.u8(1)?; w.u64(transform_id)?; }
            }
            w.u64(part.window.original_byte_len)?; w.u64(part.window.window_start)?;
            w.u64(part.window.window_len)?; w.u8(u8::from(part.window.truncated))?;
        }
    }
    Ok(())
}

pub(super) fn read(r: &mut Reader<'_>) -> Result<Views, Error> {
    let count = r.count(MAX_VOTES)?;
    let mut views = BTreeMap::new();
    for _ in 0..count {
        let member = read_name(r)?;
        if views.last_key_value().is_some_and(|(last, _)| last >= &member) { return Err(Error::InvalidInput); }
        let submitted = r.blob(MAX_SUBMITTED_BYTES)?.to_vec();
        let profile = read_profile(r)?;
        let count = r.count(MAX_SUBMITTED_PARTS)?;
        let mut parts = Vec::with_capacity(count);
        for _ in 0..count {
            let start = r.count(MAX_SUBMITTED_BYTES)?;
            let end = r.count(MAX_SUBMITTED_BYTES)?;
            let kind = match r.u8()? {
                0 => PartKind::Question, 1 => PartKind::Prompt, 2 => PartKind::Instruction,
                3 => PartKind::ToolSchema { schema_id: r.u64()? },
                4 => PartKind::Evidence { source_id: r.u64()?, transform_id: r.u64()? },
                5 => PartKind::Delimiter, 6 => PartKind::Other,
                _ => return Err(Error::InvalidInput),
            };
            parts.push(SubmittedPart { span: ByteSpan { start, end }, kind });
        }
        let count = r.count(MAX_OMISSIONS)?;
        let mut omissions = Vec::with_capacity(count);
        for _ in 0..count {
            omissions.push(match r.u8()? {
                0 => Omission::ClosedAbsent { domain_id: r.u64()?, trusted_closure_marker_id: r.u64()? },
                1 => Omission::Gapped { domain_id: r.u64()?, first_missing: r.u64()? },
                2 => Omission::Unsupported { domain_id: r.u64()? },
                3 => Omission::Redacted { domain_id: r.u64()?, transform_id: r.u64()? },
                _ => return Err(Error::InvalidInput),
            });
        }
        let actual = ActualHelperInput::new(submitted, profile, parts, omissions)?;
        let projection_id = r.u64()?;
        let policy_epoch = r.u64()?;
        let count = r.count(MAX_PROJECTED_ORIGINALS)?;
        let mut projected_originals = Vec::with_capacity(count);
        for _ in 0..count { projected_originals.push(read_original(r)?); }
        let count = r.count(MAX_EVIDENCE_PART_BINDINGS)?;
        let mut evidence = Vec::with_capacity(count);
        for _ in 0..count {
            let input_part_index = r.count(MAX_SUBMITTED_PARTS)?;
            let original = read_original(r)?;
            let transform_id = r.u64()?;
            let redaction = match r.u8()? {
                0 => RedactionMetadata::None,
                1 => RedactionMetadata::DeclaredTransform { transform_id: r.u64()? },
                _ => return Err(Error::InvalidInput),
            };
            let original_byte_len = r.u64()?;
            let window_start = r.u64()?;
            let window_len = r.u64()?;
            let truncated = match r.u8()? { 0 => false, 1 => true, _ => return Err(Error::InvalidInput) };
            evidence.push(EvidencePartView { input_part_index, original, transform_id, redaction,
                window: WindowMetadata { original_byte_len, window_start, window_len, truncated } });
        }
        let view = EvidenceViewManifest::new(actual, AuthorizationProjection {
            projection_id, policy_epoch, projected_originals,
        }, evidence)?;
        views.insert(member, view);
    }
    Ok(views)
}
