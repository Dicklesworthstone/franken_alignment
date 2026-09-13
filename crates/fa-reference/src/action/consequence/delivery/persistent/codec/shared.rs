//! Share the existing bounded journal primitives, not another storage format.
//! Visibility stops at the persistent host family; none of these are authority.
use super::{Reader as InnerReader, Writer as InnerWriter};
use super::super::{Event, FileDeliveryProfile};
use crate::action::{ResolvedTarget, Scope};
use crate::{Error, Snapshot};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub(in super::super) struct Writer(InnerWriter);
impl Writer {
    pub(in super::super) fn new(maximum: usize) -> Self { Self(InnerWriter::new(maximum)) }
    pub(in super::super) fn finish(self) -> Vec<u8> { self.0.bytes }
    pub(in super::super) fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> { self.0.raw(bytes) }
    pub(in super::super) fn blob(&mut self, bytes: &[u8]) -> Result<(), Error> { self.0.blob(bytes) }
    pub(in super::super) fn count(&mut self, count: usize) -> Result<(), Error> { self.0.count(count) }
    pub(in super::super) fn u8(&mut self, value: u8) -> Result<(), Error> { self.0.u8(value) }
    pub(in super::super) fn u32(&mut self, value: u32) -> Result<(), Error> { self.0.u32(value) }
    pub(in super::super) fn u64(&mut self, value: u64) -> Result<(), Error> { self.0.u64(value) }
    pub(in super::super) fn scope(&mut self, value: Scope) -> Result<(), Error> { self.0.scope(value) }
    pub(in super::super) fn target(&mut self, value: ResolvedTarget) -> Result<(), Error> { self.0.target(value) }
    pub(in super::super) fn snapshot(&mut self, snapshot: &Snapshot) -> Result<(), Error> { self.0.snapshot(snapshot) }
    pub(in super::super) fn event(&mut self, event: &Event) -> Result<(), Error> { super::write_event(&mut self.0, event) }
    pub(in super::super) fn bootstrap(&mut self, profile: &FileDeliveryProfile, path: &Path) -> Result<(), Error> {
        let path = path.as_os_str().as_bytes();
        if path.len() > super::MAX_PATH_BYTES { return Err(Error::Limit); }
        self.0.blob(path)?;
        self.0.blob(&super::profile_bytes(profile)?)
    }
}

pub(in super::super) struct Reader<'a>(InnerReader<'a>);
impl<'a> Reader<'a> {
    pub(in super::super) fn new(bytes: &'a [u8]) -> Self { Self(InnerReader { bytes, offset: 0 }) }
    pub(in super::super) fn take(&mut self, count: usize) -> Result<&'a [u8], Error> { self.0.take(count) }
    pub(in super::super) fn blob(&mut self, maximum: usize) -> Result<&'a [u8], Error> { self.0.blob(maximum) }
    pub(in super::super) fn count(&mut self, maximum: usize) -> Result<usize, Error> { self.0.count(maximum) }
    pub(in super::super) fn u8(&mut self) -> Result<u8, Error> { self.0.u8() }
    pub(in super::super) fn u32(&mut self) -> Result<u32, Error> { self.0.u32() }
    pub(in super::super) fn u64(&mut self) -> Result<u64, Error> { self.0.u64() }
    pub(in super::super) fn scope(&mut self) -> Result<Scope, Error> { self.0.scope() }
    pub(in super::super) fn target(&mut self) -> Result<ResolvedTarget, Error> { self.0.target() }
    pub(in super::super) fn snapshot(&mut self) -> Result<Snapshot, Error> { self.0.snapshot() }
    pub(in super::super) fn event(&mut self) -> Result<Event, Error> { super::read_event(&mut self.0) }
    pub(in super::super) fn end(&self) -> Result<(), Error> { self.0.end() }
    pub(in super::super) fn bootstrap(&mut self, profile: &FileDeliveryProfile, path: &Path) -> Result<(), Error> {
        let expected = super::profile_bytes(profile)?;
        if self.0.blob(super::MAX_PATH_BYTES)? != path.as_os_str().as_bytes()
            || self.0.blob(super::MAX_BOOT_BYTES)? != expected.as_slice()
        { return Err(Error::Binding); }
        Ok(())
    }
}
