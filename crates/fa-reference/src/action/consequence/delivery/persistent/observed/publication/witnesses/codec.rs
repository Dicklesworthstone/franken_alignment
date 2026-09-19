//! Bounded, versioned canonical packets using the original journal primitives.
use super::*;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::full_input::{ByteSpan, InputProfileBinding, Omission, PartKind, SubmittedPart,
    MAX_OMISSIONS, MAX_PROFILE_BYTES, MAX_SUBMITTED_BYTES, MAX_SUBMITTED_PARTS};
use crate::product_frontier::ProjectionKey;
use crate::witness::{DomainClosure, DomainProjection, QueryRole, MAX_SNAPSHOT_ENTRIES, MAX_VALUE_BYTES};

const INPUT: &[u8; 8] = b"FAPWIN01";
const EVIDENCE: &[u8; 8] = b"FAPWEV01";

pub(super) fn encode_inputs(input: &FilePublicationInputs) -> Result<Vec<u8>, Error> {
    let mut w = Writer::new(MAX_PUBLICATION_PACKET_BYTES);
    w.raw(INPUT)?; write_inputs(&mut w, input)?; Ok(w.finish())
}
pub(super) fn decode_inputs(bytes: &[u8]) -> Result<FilePublicationInputs, Error> {
    if bytes.len() > MAX_PUBLICATION_PACKET_BYTES { return Err(Error::Limit); }
    let mut r = Reader::new(bytes);
    if r.take(INPUT.len())? != INPUT { return Err(Error::Binding); }
    let input = read_inputs(&mut r)?; r.end()?;
    if encode_inputs(&input)?.as_slice() != bytes { return Err(Error::Binding); }
    Ok(input)
}
pub(super) fn encode_evidence(evidence: &FilePublicationEvidence) -> Result<Vec<u8>, Error> {
    let mut w = Writer::new(MAX_PUBLICATION_PACKET_BYTES);
    w.raw(EVIDENCE)?; write_inputs(&mut w, &evidence.original)?;
    w.count(evidence.requests.len())?;
    for request in &evidence.requests {
        match request {
            WitnessRequest::ExactValue { key, role } => {
                w.u8(0)?; w.u64(*key)?;
                w.u8(match role { QueryRole::Subject => 0, QueryRole::PolicyInput => 1, QueryRole::PredicateInput => 2 })?;
            }
            WitnessRequest::AbsentKey { key } => { w.u8(1)?; w.u64(*key)?; }
            WitnessRequest::EmptyRange { start, end } => { w.u8(2)?; w.u64(*start)?; w.u64(*end)?; }
            WitnessRequest::RangeMembers { start, end } => { w.u8(3)?; w.u64(*start)?; w.u64(*end)?; }
        }
    }
    Ok(w.finish())
}
pub(super) fn decode_evidence(bytes: &[u8]) -> Result<FilePublicationEvidence, Error> {
    if bytes.len() > MAX_PUBLICATION_PACKET_BYTES { return Err(Error::Limit); }
    let mut r = Reader::new(bytes);
    if r.take(EVIDENCE.len())? != EVIDENCE { return Err(Error::Binding); }
    let original = read_inputs(&mut r)?;
    let count = r.count(MAX_WITNESSES)?;
    let mut requests = Vec::with_capacity(count);
    for _ in 0..count {
        requests.push(match r.u8()? {
            0 => { let key = r.u64()?; let role = match r.u8()? {
                0 => QueryRole::Subject, 1 => QueryRole::PolicyInput, 2 => QueryRole::PredicateInput,
                _ => return Err(Error::InvalidInput),
            }; WitnessRequest::ExactValue { key, role } }
            1 => WitnessRequest::AbsentKey { key: r.u64()? },
            2 => WitnessRequest::EmptyRange { start: r.u64()?, end: r.u64()? },
            3 => WitnessRequest::RangeMembers { start: r.u64()?, end: r.u64()? },
            _ => return Err(Error::InvalidInput),
        });
    }
    r.end()?;
    let evidence = FilePublicationEvidence::new(original, requests)?;
    if encode_evidence(&evidence)?.as_slice() != bytes { return Err(Error::Binding); }
    Ok(evidence)
}

fn write_key(w: &mut Writer, key: ProjectionKey) -> Result<(), Error> {
    for value in [key.source, key.branch, key.projection, key.source_epoch] { w.u64(value)?; }
    Ok(())
}
fn read_key(r: &mut Reader<'_>) -> Result<ProjectionKey, Error> {
    Ok(ProjectionKey { source: r.u64()?, branch: r.u64()?, projection: r.u64()?, source_epoch: r.u64()? })
}
fn write_marker(w: &mut Writer, marker: TrustedClosingMarker) -> Result<(), Error> {
    w.u64(marker.final_sequence)?; w.u64(marker.marker_generation)
}
fn read_marker(r: &mut Reader<'_>, key: ProjectionKey) -> Result<TrustedClosingMarker, Error> {
    Ok(TrustedClosingMarker { key, final_sequence: r.u64()?, marker_generation: r.u64()? })
}

fn write_inputs(w: &mut Writer, input: &FilePublicationInputs) -> Result<(), Error> {
    match &input.structured {
        None => w.u8(0)?,
        Some(input) => {
            w.u8(1)?;
            let snapshot = &input.snapshot;
            w.u64(snapshot.revision())?; w.u64(snapshot.control_cut())?; w.u64(snapshot.semantic_epoch())?;
            let domain = snapshot.domain_input();
            write_key(w, domain.domain().projection())?;
            w.u64(domain.domain().domain_id())?; w.u64(domain.domain().domain_epoch())?;
            match domain.closure() {
                DomainClosure::Unknown => w.u8(0)?,
                DomainClosure::ConservativeSummary => w.u8(1)?,
                DomainClosure::Closed(marker) => { w.u8(2)?; write_marker(w, marker)?; }
            }
            match input.admitted_close {
                None => w.u8(0)?,
                Some(marker) => { w.u8(1)?; write_marker(w, marker)?; }
            }
            w.count(input.keys.len())?;
            for key in &input.keys {
                let entry = snapshot.entry(*key).ok_or(Error::Binding)?;
                w.u64(entry.key())?; w.u64(entry.version())?; w.blob(entry.value())?;
            }
        }
    }
    match &input.opaque {
        None => w.u8(0)?,
        Some(actual) => { w.u8(1)?; write_actual(w, actual)?; }
    }
    Ok(())
}
fn read_inputs(r: &mut Reader<'_>) -> Result<FilePublicationInputs, Error> {
    let structured = match r.u8()? {
        0 => None,
        1 => {
            let revision = r.u64()?; let control_cut = r.u64()?; let epoch = r.u64()?;
            let key = read_key(r)?;
            let domain = DomainProjection::new(r.u64()?, r.u64()?, key);
            let closure = match r.u8()? {
                0 => DomainClosure::Unknown, 1 => DomainClosure::ConservativeSummary,
                2 => DomainClosure::Closed(read_marker(r, key)?), _ => return Err(Error::InvalidInput),
            };
            let admitted = match r.u8()? {
                0 => None, 1 => Some(read_marker(r, key)?), _ => return Err(Error::InvalidInput),
            };
            let frontiers = replay_frontiers(admitted)?;
            let count = r.count(MAX_SNAPSHOT_ENTRIES)?;
            let mut entries: Vec<SnapshotEntry> = Vec::with_capacity(count);
            for _ in 0..count {
                let key = r.u64()?;
                if entries.last().is_some_and(|last| last.key() >= key) { return Err(Error::InvalidInput); }
                entries.push(SnapshotEntry::new(key, r.u64()?, r.blob(MAX_VALUE_BYTES)?.to_vec())?);
            }
            Some(FileWitnessInput::new(revision, control_cut, epoch,
                AdapterDomainInput::new(domain, closure), entries, &frontiers)?)
        }
        _ => return Err(Error::InvalidInput),
    };
    let opaque = match r.u8()? {
        0 => None, 1 => Some(read_actual(r)?), _ => return Err(Error::InvalidInput),
    };
    Ok(FilePublicationInputs::new(structured, opaque))
}

fn write_actual(w: &mut Writer, input: &ActualHelperInput) -> Result<(), Error> {
    w.blob(input.submitted_bytes())?;
    let p = input.input_profile();
    for value in [p.profile_id, p.tokenizer_epoch, p.policy_epoch, p.model_epoch] { w.u64(value)?; }
    w.blob(&p.profile_bytes)?; w.count(input.ordered_parts().len())?;
    for part in input.ordered_parts() {
        w.count(part.span.start)?; w.count(part.span.end)?;
        match part.kind {
            PartKind::Question => w.u8(0)?, PartKind::Prompt => w.u8(1)?, PartKind::Instruction => w.u8(2)?,
            PartKind::ToolSchema { schema_id } => { w.u8(3)?; w.u64(schema_id)?; }
            PartKind::Evidence { source_id, transform_id } => { w.u8(4)?; w.u64(source_id)?; w.u64(transform_id)?; }
            PartKind::Delimiter => w.u8(5)?, PartKind::Other => w.u8(6)?,
        }
    }
    w.count(input.omissions().len())?;
    for omission in input.omissions() {
        match omission {
            Omission::ClosedAbsent { domain_id, trusted_closure_marker_id } => { w.u8(0)?; w.u64(*domain_id)?; w.u64(*trusted_closure_marker_id)?; }
            Omission::Gapped { domain_id, first_missing } => { w.u8(1)?; w.u64(*domain_id)?; w.u64(*first_missing)?; }
            Omission::Unsupported { domain_id } => { w.u8(2)?; w.u64(*domain_id)?; }
            Omission::Redacted { domain_id, transform_id } => { w.u8(3)?; w.u64(*domain_id)?; w.u64(*transform_id)?; }
        }
    }
    Ok(())
}
fn read_actual(r: &mut Reader<'_>) -> Result<ActualHelperInput, Error> {
    let submitted = r.blob(MAX_SUBMITTED_BYTES)?.to_vec();
    let profile = InputProfileBinding { profile_id: r.u64()?, tokenizer_epoch: r.u64()?,
        policy_epoch: r.u64()?, model_epoch: r.u64()?, profile_bytes: r.blob(MAX_PROFILE_BYTES)?.to_vec() };
    let count = r.count(MAX_SUBMITTED_PARTS)?;
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        let span = ByteSpan { start: r.count(MAX_SUBMITTED_BYTES)?, end: r.count(MAX_SUBMITTED_BYTES)? };
        let kind = match r.u8()? {
            0 => PartKind::Question, 1 => PartKind::Prompt, 2 => PartKind::Instruction,
            3 => PartKind::ToolSchema { schema_id: r.u64()? },
            4 => PartKind::Evidence { source_id: r.u64()?, transform_id: r.u64()? },
            5 => PartKind::Delimiter, 6 => PartKind::Other, _ => return Err(Error::InvalidInput),
        };
        parts.push(SubmittedPart { span, kind });
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
    ActualHelperInput::new(submitted, profile, parts, omissions)
}
