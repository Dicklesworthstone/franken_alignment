//! Lossless interchange for reference identity observations, not authentication.
//! Passport construction remains the original registration authority. Decoding a
//! frame constructs caller-supplied SourceFrame data, never a verified host tap.

use super::{IdentityAnchor, ModelManifest, ModelPassport, MAX_ANCHORS, MAX_STIMULUS_TOKENS};
use crate::action::consequence::activation::{CaptureProfile, FrameIdentity, SourceFrame, HEADER_BYTES, MAX_BLOCK_BYTES, MAX_VALUES};
use crate::Error;

const DOMAIN: &[u8; 8] = b"FAIDP\0\0\x01";
pub const MANIFEST_BYTES: usize = 200;
pub const MAX_PASSPORT_BYTES: usize = 8 + 16 + MANIFEST_BYTES + 4
    + MAX_ANCHORS * 64 + MAX_STIMULUS_TOKENS * 4 + MAX_VALUES * 8;

/// Exact registration bytes, including binary32 interval endpoints and stimuli.
/// No hash, tolerance rounding, model execution or signature is introduced.
pub fn encode_passport(passport: &ModelPassport) -> Result<Vec<u8>, Error> {
    let size = 8 + 16 + MANIFEST_BYTES + 4 + passport.anchors().values().map(|anchor|
        64 + anchor.stimulus().len() * 4 + anchor.dimensions() * 8).sum::<usize>();
    if size > MAX_PASSPORT_BYTES { return Err(Error::Limit); }
    let mut out = Vec::new();
    out.try_reserve_exact(size).map_err(|_| Error::Limit)?;
    out.extend_from_slice(DOMAIN);
    put64(&mut out, passport.id()); put64(&mut out, passport.generation());
    write_manifest(&mut out, passport.manifest());
    put32(&mut out, passport.anchors().len() as u32);
    for anchor in passport.anchors().values() {
        put64(&mut out, anchor.id());
        write_profile(&mut out, anchor.profile()); put64(&mut out, anchor.stream());
        put32(&mut out, anchor.stimulus().len() as u32);
        for token in anchor.stimulus() { put32(&mut out, *token); }
        put32(&mut out, anchor.dimensions() as u32);
        for [lo, hi] in anchor.bound_bits() { put32(&mut out, *lo); put32(&mut out, *hi); }
    }
    Ok(out)
}

/// Bound aggregate tokens/coordinates BEFORE allocation, then use the original
/// constructors. Noncanonical anchor order and duplicate identities refuse.
pub fn decode_passport(bytes: &[u8]) -> Result<ModelPassport, Error> {
    if bytes.len() > MAX_PASSPORT_BYTES { return Err(Error::Limit); }
    let mut r = Reader { bytes, offset: 0 };
    if r.take(8)? != DOMAIN { return Err(Error::InvalidInput); }
    let id = r.u64()?; let generation = r.u64()?;
    let manifest = r.manifest()?;
    let count = r.count(MAX_ANCHORS)?;
    let mut anchors = Vec::new();
    anchors.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    let mut tokens_left = MAX_STIMULUS_TOKENS;
    let mut coordinates_left = MAX_VALUES;
    let mut previous = 0;
    for _ in 0..count {
        let id = r.u64()?;
        if id <= previous { return Err(Error::Binding); }
        previous = id;
        let profile = r.profile()?; let stream = r.u64()?;
        let count = r.count(tokens_left)?;
        tokens_left -= count;
        let mut stimulus = Vec::new();
        stimulus.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        for _ in 0..count { stimulus.push(r.u32()?); }
        let count = r.count(coordinates_left)?;
        coordinates_left -= count;
        let mut bounds = Vec::new();
        bounds.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        for _ in 0..count { bounds.push([f32::from_bits(r.u32()?), f32::from_bits(r.u32()?)]); }
        anchors.push(IdentityAnchor::new(id, profile, stream, stimulus, &bounds)?);
    }
    r.end()?;
    let passport = ModelPassport::new(id, generation, manifest, anchors)?;
    if encode_passport(&passport)?.as_slice() != bytes { return Err(Error::Binding); }
    Ok(passport)
}

/// A measurement may contain a wrong/zero manifest; retain that observation so
/// the native identity observer can latch Mismatch rather than erase its input.
pub fn encode_manifest(manifest: &ModelManifest) -> Vec<u8> {
    let mut out = Vec::with_capacity(MANIFEST_BYTES);
    write_manifest(&mut out, manifest); out
}
pub fn decode_manifest(bytes: &[u8]) -> Result<ModelManifest, Error> {
    if bytes.len() > MANIFEST_BYTES { return Err(Error::Limit); }
    let mut r = Reader { bytes, offset: 0 };
    let manifest = r.manifest()?; r.end()?; Ok(manifest)
}

/// Read ONLY the original full-precision initial-frame format. Progressive or
/// refinement blocks cannot manufacture missing coordinates. Original capture
/// validates finite values/identity, and verify_block checks exact canonical bits.
/// Call frame.encode_initial(23) to encode. These are supplied observations, not
/// a proof that an actual model produced them or that a capture was authentic.
pub fn decode_frame(bytes: &[u8]) -> Result<SourceFrame, Error> {
    if bytes.len() > MAX_BLOCK_BYTES { return Err(Error::Limit); }
    let mut r = Reader { bytes, offset: 0 };
    if r.take(8)? != b"FAPF32\0\x01" { return Err(Error::InvalidInput); }
    let identity = FrameIdentity { profile: r.profile()?, stream: r.u64()?, sequence: r.u64()?, position: r.u64()? };
    let count = r.count(MAX_VALUES)?;
    if r.take(2)? != [u8::MAX, 23] { return Err(Error::Binding); }
    if bytes.len() != HEADER_BYTES + count * 4 { return Err(Error::InvalidInput); }
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count { values.push(f32::from_bits(r.u32()?)); }
    r.end()?;
    let frame = SourceFrame::capture(identity, &values)?;
    frame.verify_block(bytes)?;
    Ok(frame)
}

fn put32(out: &mut Vec<u8>, value: u32) { out.extend_from_slice(&value.to_be_bytes()); }
fn put64(out: &mut Vec<u8>, value: u64) { out.extend_from_slice(&value.to_be_bytes()); }
fn write_profile(out: &mut Vec<u8>, p: CaptureProfile) {
    for value in [p.tenant, p.model, p.model_generation, p.tap, p.layout_generation] { put64(out, value); }
}
fn write_manifest(out: &mut Vec<u8>, m: &ModelManifest) {
    for value in [m.tenant, m.model, m.model_generation, m.host_generation, m.tokenizer_generation] { put64(out, value); }
    for digest in [&m.weights, &m.adapters, &m.tokenizer, &m.architecture, &m.numeric_profile] { out.extend_from_slice(digest); }
}
struct Reader<'a> { bytes: &'a [u8], offset: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Limit)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Incomplete)?;
        self.offset = end; Ok(bytes)
    }
    fn u32(&mut self) -> Result<u32, Error> { Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Incomplete)?)) }
    fn u64(&mut self) -> Result<u64, Error> { Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?)) }
    fn count(&mut self, limit: usize) -> Result<usize, Error> {
        let count = usize::try_from(self.u32()?).map_err(|_| Error::Limit)?;
        if count > limit { return Err(Error::Limit); } Ok(count)
    }
    fn profile(&mut self) -> Result<CaptureProfile, Error> {
        Ok(CaptureProfile { tenant: self.u64()?, model: self.u64()?, model_generation: self.u64()?, tap: self.u64()?, layout_generation: self.u64()? })
    }
    fn manifest(&mut self) -> Result<ModelManifest, Error> {
        Ok(ModelManifest { tenant: self.u64()?, model: self.u64()?, model_generation: self.u64()?, host_generation: self.u64()?, tokenizer_generation: self.u64()?,
            weights: self.take(32)?.try_into().map_err(|_| Error::Incomplete)?, adapters: self.take(32)?.try_into().map_err(|_| Error::Incomplete)?,
            tokenizer: self.take(32)?.try_into().map_err(|_| Error::Incomplete)?, architecture: self.take(32)?.try_into().map_err(|_| Error::Incomplete)?,
            numeric_profile: self.take(32)?.try_into().map_err(|_| Error::Incomplete)? })
    }
    fn end(&self) -> Result<(), Error> { if self.offset == self.bytes.len() { Ok(()) } else { Err(Error::InvalidInput) } }
}

#[cfg(test)]
mod tests;
