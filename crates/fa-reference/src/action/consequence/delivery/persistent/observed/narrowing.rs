//! Explicit ordinary target-ceiling restriction in the original two-key journal.
//! The original authority intersects sets; no policy, checkpoint or review is invented.
use super::{Event, FileOversight, JournalError, Transition};
use super::super::codec::shared::{Reader, Writer};
use crate::action::consequence::gate::{TargetCeiling, TargetNarrowReceipt};
use crate::action::{MAX_ATTEMPTS, ResolvedTarget};
use crate::Error;

pub const MAX_AUTHORITY_NARROWINGS: usize = 128;

/// Independently selected monotone restriction. A target is its complete exact
/// adapter/object/contract/version/generation identity. Empty retains nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAuthorityNarrowRequest {
    operation: u64,
    expected_control_sequence: u64,
    expected_authority_epoch: u64,
    retained_targets: Vec<ResolvedTarget>,
}

impl FileAuthorityNarrowRequest {
    pub fn new(operation: u64, expected_control_sequence: u64,
        expected_authority_epoch: u64, retained_targets: Vec<ResolvedTarget>) -> Result<Self, Error>
    {
        if operation == 0 { return Err(Error::InvalidInput); }
        let retained_targets = TargetCeiling::new(&retained_targets)?.targets();
        Ok(Self { operation, expected_control_sequence, expected_authority_epoch, retained_targets })
    }
    pub fn operation(&self) -> u64 { self.operation }
    pub fn expected_control_sequence(&self) -> u64 { self.expected_control_sequence }
    pub fn expected_authority_epoch(&self) -> u64 { self.expected_authority_epoch }
    pub fn retained_targets(&self) -> &[ResolvedTarget] { &self.retained_targets }
    pub(super) fn ceiling(&self) -> Result<TargetCeiling, Error> {
        if self.operation == 0 { return Err(Error::InvalidInput); }
        TargetCeiling::new(&self.retained_targets)
    }
}

/// Historical acknowledged restriction, never an effect permit or evidence
/// that a previous human key is current at the new control sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileAuthorityNarrowReceipt {
    pub(super) request: FileAuthorityNarrowRequest,
    pub(super) change: TargetNarrowReceipt,
}
impl FileAuthorityNarrowReceipt {
    pub fn request(&self) -> &FileAuthorityNarrowRequest { &self.request }
    pub fn change(&self) -> &TargetNarrowReceipt { &self.change }
}

impl FileOversight {
    /// Commit one explicit restriction of this original ordinary artifact owner.
    /// Retained automatic reservations and every dispatched/Unknown charge stay
    /// with the original gate. Only outside undispatched reservations refund.
    /// The control sequence advances once; policy and authority epochs do not.
    /// Original human keys withdraw because their predecessor is now historical.
    /// No source/clock observation, helper execution, resend or actor reset runs.
    ///
    /// Decoder/predictor, mediated and ALL stream histories are unsupported by
    /// this ordinary operation and refuse before replay. An exact healthy retry
    /// returns its original receipt, including after reopening, without widening
    /// a later ceiling or restoring a key. Existing event bytes remain unchanged.
    pub fn narrow_authority(&mut self, revision: u64, request: FileAuthorityNarrowRequest)
        -> Result<FileAuthorityNarrowReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        request.ceiling()?;
        if let Some(receipt) = self.machine.narrowing_retry(&request)? { return Ok(receipt); }
        if revision != self.revision() { return Err(Error::Stale.into()); }
        self.machine.check_authority_narrow(&request)?;
        match self.transact(revision, Event::NarrowAuthority(request))? {
            Transition::AuthorityNarrowed(receipt) => Ok(receipt),
            _ => unreachable!("original authority narrowing transition"),
        }
    }

    pub fn authority_narrowing_receipt(&self, operation: u64)
        -> Result<&FileAuthorityNarrowReceipt, JournalError>
    {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        self.machine.narrowings.get(&operation).ok_or_else(|| Error::Missing.into())
    }

    pub(crate) fn authority_narrowing_supported(&self) -> bool {
        self.machine.authority_narrowing_supported()
    }
}

pub(super) fn write(w: &mut Writer, request: &FileAuthorityNarrowRequest) -> Result<(), Error> {
    request.ceiling()?;
    w.u64(request.operation)?;
    w.u64(request.expected_control_sequence)?;
    w.u64(request.expected_authority_epoch)?;
    w.count(request.retained_targets.len())?;
    for target in &request.retained_targets { w.target(*target)?; }
    Ok(())
}

pub(super) fn read(r: &mut Reader<'_>) -> Result<FileAuthorityNarrowRequest, Error> {
    let operation = r.u64()?;
    let sequence = r.u64()?;
    let epoch = r.u64()?;
    let count = r.count(MAX_ATTEMPTS)?;
    let mut targets = Vec::new();
    targets.try_reserve_exact(count).map_err(|_| Error::Limit)?;
    for _ in 0..count { targets.push(r.target()?); }
    let request = FileAuthorityNarrowRequest::new(operation, sequence, epoch, targets.clone())?;
    // Live selection canonicalizes a set. Persisted input has exactly one
    // encoding; alternate ordering cannot create another canonical history.
    if request.retained_targets != targets { return Err(Error::InvalidInput); }
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(version: u64) -> ResolvedTarget {
        ResolvedTarget { adapter: 1, object: 2, contract_version: 3,
            expected_version: version, generation: 4 }
    }

    fn raw(operation: u64, targets: &[ResolvedTarget]) -> Vec<u8> {
        let mut writer = Writer::new(4096);
        writer.u64(operation).unwrap();
        writer.u64(5).unwrap();
        writer.u64(6).unwrap();
        writer.count(targets.len()).unwrap();
        for target in targets { writer.target(*target).unwrap(); }
        writer.finish()
    }

    #[test]
    fn native_recipe_has_one_bounded_canonical_encoding_and_refuses_incomplete_input() {
        let input = FileAuthorityNarrowRequest::new(7, 5, 6, vec![target(2), target(1)]).unwrap();
        assert_eq!(input.retained_targets(), &[target(1), target(2)]);
        let mut writer = Writer::new(4096);
        write(&mut writer, &input).unwrap();
        let bytes = writer.finish();
        assert_eq!(bytes, raw(7, &[target(1), target(2)]));
        let mut reader = Reader::new(&bytes);
        assert_eq!(read(&mut reader).unwrap(), input);
        reader.end().unwrap();
        for end in 0..bytes.len() {
            assert!(read(&mut Reader::new(&bytes[..end])).is_err(), "prefix {end}");
        }
        assert_eq!(read(&mut Reader::new(&raw(7, &[target(2), target(1)]))), Err(Error::InvalidInput));
        assert_eq!(read(&mut Reader::new(&raw(7, &[target(1), target(1)]))), Err(Error::Duplicate));
        assert_eq!(read(&mut Reader::new(&raw(0, &[target(1)]))), Err(Error::InvalidInput));
        let mut writer = Writer::new(4096);
        writer.u64(7).unwrap();
        writer.u64(5).unwrap();
        writer.u64(6).unwrap();
        writer.count(MAX_ATTEMPTS + 1).unwrap();
        assert_eq!(read(&mut Reader::new(&writer.finish())), Err(Error::Limit));
    }
}
