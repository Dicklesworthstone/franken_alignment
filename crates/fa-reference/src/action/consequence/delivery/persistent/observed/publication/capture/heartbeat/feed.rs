//! Bounded, immutable change-window transport for the original publication gate.
//! A producer-owned file contains BOTH the records and their closed heartbeat.
//! It is not an authenticated source, a permission or a missing-tail substitute.
pub(in crate::action::consequence::delivery::persistent::observed) mod ingest;
use super::{FileCaptureError, PublicationHeartbeat};
use super::super::super::witness_gate::changes::{read_change, write_change};
use super::super::super::witness_gate::freshness::{read_heartbeat, write_heartbeat};
use super::super::super::witnesses::producer::{PublicationProducerImage, PublicationProducerProfile, MAX_PRODUCER_BYTES};
use crate::action::consequence::delivery::publication_gate::changes::PublicationChange;
use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};
use crate::Error;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
pub use ingest::PublicationFeedReport;

const DOMAIN: &[u8; 8] = b"FAPFEED1";
pub const MAX_FEED_RECORDS: usize = 256;
pub const MAX_FEED_BYTES: usize = 32 * 1024;

/// A complete retained window (after, heartbeat.through]. The window may overlap
/// acknowledged records but cannot contain gaps, duplicates or another source.
/// Sequences are immutable across generations; changing retention is not rewriting
/// a notice. Completeness of real-world capture remains a producer assumption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicationFeedBatch {
    heartbeat: PublicationHeartbeat,
    after: u64,
    records: Vec<PublicationChange>,
}
impl PublicationFeedBatch {
    pub fn new(heartbeat: PublicationHeartbeat, after: u64, records: Vec<PublicationChange>) -> Result<Self, Error> {
        if records.len() > MAX_FEED_RECORDS { return Err(Error::Limit); }
        if heartbeat.source == 0 || heartbeat.clock_domain == 0 || heartbeat.generation == 0 {
            return Err(Error::InvalidInput);
        }
        if heartbeat.through.checked_sub(after) != Some(records.len() as u64) { return Err(Error::Incomplete); }
        let mut sequence = after;
        for record in &records {
            sequence = sequence.checked_add(1).ok_or(Error::Overflow)?;
            if record.source != heartbeat.source { return Err(Error::Binding); }
            if record.sequence != sequence { return Err(Error::Incomplete); }
        }
        Ok(Self { heartbeat, after, records })
    }
    pub fn heartbeat(&self) -> PublicationHeartbeat { self.heartbeat }
    pub fn after(&self) -> u64 { self.after }
    pub fn records(&self) -> &[PublicationChange] { &self.records }
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut w = Writer::new(MAX_FEED_BYTES);
        w.raw(DOMAIN)?; write_heartbeat(&mut w, self.heartbeat)?;
        w.u64(self.after)?; w.count(self.records.len())?;
        for record in &self.records { write_change(&mut w, *record)?; }
        Ok(w.finish())
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_FEED_BYTES { return Err(Error::Limit); }
        let mut r = Reader::new(bytes);
        if r.take(DOMAIN.len())? != DOMAIN { return Err(Error::Binding); }
        let heartbeat = read_heartbeat(&mut r)?;
        let after = r.u64()?;
        let count = r.count(MAX_FEED_RECORDS)?;
        let mut records = Vec::new();
        records.try_reserve_exact(count).map_err(|_| Error::Limit)?;
        for _ in 0..count { records.push(read_change(&mut r)?); }
        r.end()?;
        let batch = Self::new(heartbeat, after, records)?;
        if batch.to_bytes()?.as_slice() != bytes { return Err(Error::Binding); }
        Ok(batch)
    }
}

/// Fixed concrete reader with no positive cache or reader override. Each call
/// reopens an operator-owned regular file. Producers replace it by atomic rename.
/// Metadata checks detect ordinary mutation, not hostile filesystem races.
#[derive(Debug)]
pub struct PublicationFeedFile {
    path: PathBuf,
    source: u64,
    producer: Option<PublicationProducerProfile>,
}
impl PublicationFeedFile {
    pub fn new(path: impl AsRef<Path>, source: u64) -> Result<Self, Error> {
        let path = path.as_ref();
        if source == 0 || path.as_os_str().is_empty() { return Err(Error::InvalidInput); }
        if path.as_os_str().as_bytes().len() > 4_096 { return Err(Error::Limit); }
        Ok(Self { path: path.to_owned(), source, producer: None })
    }
    /// Extract the bounded feed from a freshly opened coupled producer bundle.
    /// Explicit selection pins the full profile; legacy readers remain strict.
    pub fn from_producer(path: impl AsRef<Path>, profile: PublicationProducerProfile) -> Result<Self, Error> {
        profile.check()?;
        let mut reader = Self::new(path, profile.feed)?;
        reader.producer = Some(profile);
        Ok(reader)
    }
    pub fn source(&self) -> u64 { self.source }
    // Pure configuration equality: do not resolve aliases or touch files before
    // the owner durably withdraws both eligibility lanes.
    pub(in crate::action::consequence::delivery::persistent::observed) fn pairs_with(
        &self, path: &Path, profile: PublicationProducerProfile) -> bool
    {
        self.path == path && self.producer == Some(profile)
    }
    pub fn read_batch(&self) -> Result<PublicationFeedBatch, FileCaptureError> {
        let max_bytes = if self.producer.is_some() { MAX_PRODUCER_BYTES } else { MAX_FEED_BYTES };
        let before = regular(&self.path, max_bytes)?;
        let file = File::open(&self.path)?;
        let opened = file.metadata()?;
        if !opened.is_file() || super::stamp(&before) != super::stamp(&opened) {
            return Err(Error::Stale.into());
        }
        let mut bytes = Vec::new();
        // Reserve the whole bounded read plus its overflow sentinel before I/O.
        bytes.try_reserve_exact(max_bytes + 1).map_err(|_| Error::Limit)?;
        (&file).take(max_bytes as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > max_bytes { return Err(Error::Limit.into()); }
        if super::stamp(&opened) != super::stamp(&file.metadata()?)
            || super::stamp(&opened) != super::stamp(&regular(&self.path, max_bytes)?) {
            return Err(Error::Stale.into());
        }
        let batch = match self.producer {
            None => PublicationFeedBatch::from_bytes(&bytes)?,
            Some(profile) => {
                let image = PublicationProducerImage::from_bytes(&bytes)?;
                if image.profile() != profile { return Err(Error::Binding.into()); }
                image.batch().clone()
            }
        };
        if batch.heartbeat.source != self.source { return Err(Error::Binding.into()); }
        Ok(batch)
    }
}
fn regular(path: &Path, max_bytes: usize) -> Result<fs::Metadata, FileCaptureError> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() { return Err(Error::Binding.into()); }
    if metadata.len() > max_bytes as u64 { return Err(Error::Limit.into()); }
    Ok(metadata)
}
