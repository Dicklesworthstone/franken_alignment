//! Bounded exact-byte sinks. Comparing a recipe never constructs one from disk.
use crate::Error;

enum Output<'a> { Count, Bytes(Vec<u8>), Compare(&'a [u8]) }
pub(super) struct Writer<'a> { output: Output<'a>, at: usize, limit: usize }
impl<'a> Writer<'a> {
    pub(super) fn count(limit: usize) -> Self { Self { output: Output::Count, at: 0, limit } }
    pub(super) fn compare(bytes: &'a [u8]) -> Self {
        Self { output: Output::Compare(bytes), at: 0, limit: bytes.len() }
    }
    pub(super) fn collect(limit: usize) -> Result<Self, Error> {
        let mut bytes = Vec::new(); bytes.try_reserve_exact(limit).map_err(|_| Error::Limit)?;
        Ok(Self { output: Output::Bytes(bytes), at: 0, limit })
    }
    pub(super) fn len(&self) -> usize { self.at }
    fn end(&self, length: usize) -> Result<usize, Error> {
        let end = self.at.checked_add(length).ok_or(Error::Limit)?;
        if end > self.limit { Err(Error::Limit) } else { Ok(end) }
    }
    pub(super) fn bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self.end(bytes.len())?;
        match &mut self.output {
            Output::Count => {}
            Output::Bytes(out) => out.extend_from_slice(bytes),
            Output::Compare(expected) => {
                if expected[self.at..end] != *bytes { return Err(Error::Binding); }
            }
        }
        self.at = end; Ok(())
    }
    pub(super) fn u32(&mut self, value: u32) -> Result<(), Error> { self.bytes(&value.to_be_bytes()) }
    pub(super) fn u64(&mut self, value: u64) -> Result<(), Error> { self.bytes(&value.to_be_bytes()) }
    pub(super) fn size(&mut self, value: usize) -> Result<(), Error> {
        self.u64(u64::try_from(value).map_err(|_| Error::Limit)?)
    }
    pub(super) fn blob(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.size(bytes.len())?; self.bytes(bytes)
    }
    /// Reject complete vector size before scanning or emitting its first value.
    pub(super) fn words(&mut self, words: &[u32]) -> Result<(), Error> {
        self.size(words.len())?;
        let end = self.end(words.len().checked_mul(4).ok_or(Error::Limit)?)?;
        if matches!(&self.output, Output::Count) { self.at = end; return Ok(()); }
        for word in words { self.u32(*word)?; }
        Ok(())
    }
    pub(super) fn floats(&mut self, values: &[f32]) -> Result<(), Error> {
        self.size(values.len())?;
        let end = self.end(values.len().checked_mul(4).ok_or(Error::Limit)?)?;
        if matches!(&self.output, Output::Count) { self.at = end; return Ok(()); }
        for value in values { self.u32(value.to_bits())?; }
        Ok(())
    }
    pub(super) fn finish(self) -> Result<Vec<u8>, Error> {
        if self.at != self.limit { return Err(Error::Binding); }
        match self.output { Output::Bytes(bytes) => Ok(bytes), _ => Err(Error::WrongState) }
    }
}

pub(super) struct Reader<'a> { bytes: &'a [u8], at: usize }
impl<'a> Reader<'a> {
    pub(super) fn new(bytes: &'a [u8]) -> Self { Self { bytes, at: 0 } }
    pub(super) fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(length).ok_or(Error::Limit)?;
        let part = self.bytes.get(self.at..end).ok_or(Error::Incomplete)?;
        self.at = end; Ok(part)
    }
    pub(super) fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    pub(super) fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    pub(super) fn count(&mut self, maximum: usize) -> Result<usize, Error> {
        let count = usize::try_from(self.u64()?).map_err(|_| Error::Limit)?;
        if count > maximum { Err(Error::Limit) } else { Ok(count) }
    }
    pub(super) fn end(&self) -> Result<(), Error> {
        if self.at == self.bytes.len() { Ok(()) } else { Err(Error::InvalidInput) }
    }
}
