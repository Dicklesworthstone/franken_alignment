//! Cumulative, append-only text release (plan 8.7).
//!
//! Each operation is one complete UTF-8 message or an explicit end marker.
//! The reviewed payload contains ALL prior messages with their boundaries.
//! It is not a token transport, tool-call parser, signature or permission.

use crate::Error;

pub const MAX_STREAM_BYTES: usize = 32 * 1_024;
pub const MAX_MESSAGE_BYTES: usize = 4 * 1_024;
pub const MAX_STREAM_MESSAGES: usize = 127;
pub const STREAM_HEADER_BYTES: usize = 49;
pub const MAX_STREAM_FRAME_BYTES: usize = STREAM_HEADER_BYTES + 4 * MAX_STREAM_MESSAGES + MAX_STREAM_BYTES;
const DOMAIN: &[u8; 8] = b"FASOUT\0\x01";

/// Immutable bootstrap limits. A profile identifies complete-message disclosure,
/// not the semantic safety of a sentence or the completeness of a tool call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamProfile {
    id: u64,
    generation: u64,
    max_messages: usize,
    max_message_bytes: usize,
    max_stream_bytes: usize,
}

impl StreamProfile {
    pub fn new(id: u64, generation: u64, max_messages: usize, max_message_bytes: usize, max_stream_bytes: usize) -> Result<Self, Error> {
        if id == 0 || generation == 0 || max_messages == 0 || max_message_bytes == 0 || max_stream_bytes == 0 {
            return Err(Error::InvalidInput);
        }
        if max_messages > MAX_STREAM_MESSAGES || max_message_bytes > MAX_MESSAGE_BYTES
            || max_stream_bytes > MAX_STREAM_BYTES || max_message_bytes > max_stream_bytes
        {
            return Err(Error::Limit);
        }
        Ok(Self { id, generation, max_messages, max_message_bytes, max_stream_bytes })
    }

    pub fn id(self) -> u64 { self.id }
    pub fn generation(self) -> u64 { self.generation }
    pub fn max_messages(self) -> usize { self.max_messages }
    pub fn max_message_bytes(self) -> usize { self.max_message_bytes }
    pub fn max_stream_bytes(self) -> usize { self.max_stream_bytes }
}

/// Immutable audience history. A broker's copy is receipt-confirmed; the endpoint
/// may be ahead while acknowledgment is lost. Copies contain no live authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StreamView {
    profile: StreamProfile,
    visible: Vec<u8>,
    ends: Vec<usize>,
    finished: bool,
}

impl StreamView {
    pub fn empty(profile: StreamProfile) -> Self {
        Self { profile, visible: Vec::new(), ends: Vec::new(), finished: false }
    }

    pub fn profile(&self) -> StreamProfile { self.profile }
    pub fn visible(&self) -> &[u8] { &self.visible }
    pub fn message_count(&self) -> usize { self.ends.len() }
    pub fn finished(&self) -> bool { self.finished }

    pub fn messages(&self) -> impl Iterator<Item = &str> {
        let mut start = 0;
        self.ends.iter().map(move |end| {
            let bytes = &self.visible[start..*end];
            start = *end;
            std::str::from_utf8(bytes).expect("validated complete UTF-8 messages")
        })
    }

    /// Build a proposal payload, not a sendable envelope. Full cumulative context
    /// and every prior message boundary must pass the ordinary policy/congress.
    pub fn encode_message(&self, message: &str) -> Result<Vec<u8>, Error> {
        self.encode(Some(message))
    }

    /// Closing is itself a reviewed effect. An ordinary cancellation is NOT an
    /// end marker, and a timeout cannot prove that no prefix was disclosed.
    pub fn encode_finish(&self) -> Result<Vec<u8>, Error> {
        self.encode(None)
    }

    fn encode(&self, message: Option<&str>) -> Result<Vec<u8>, Error> {
        if self.finished { return Err(Error::WrongState); }
        let added = message.map_or(0, str::len);
        if message.is_some_and(str::is_empty) { return Err(Error::InvalidInput); }
        if added > self.profile.max_message_bytes || self.visible.len() + added > self.profile.max_stream_bytes
            || (message.is_some() && self.ends.len() >= self.profile.max_messages)
        {
            return Err(Error::Limit);
        }
        let length = STREAM_HEADER_BYTES + self.ends.len() * 4 + self.visible.len() + added;
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(DOMAIN);
        for value in [self.profile.id, self.profile.generation] { bytes.extend_from_slice(&value.to_be_bytes()); }
        for value in [self.profile.max_messages, self.profile.max_message_bytes, self.profile.max_stream_bytes,
            self.ends.len(), self.visible.len()]
        {
            bytes.extend_from_slice(&(value as u32).to_be_bytes());
        }
        bytes.push(u8::from(message.is_none()));
        bytes.extend_from_slice(&(added as u32).to_be_bytes());
        for prior in self.messages() {
            bytes.extend_from_slice(&(prior.len() as u32).to_be_bytes());
            bytes.extend_from_slice(prior.as_bytes());
        }
        if let Some(message) = message { bytes.extend_from_slice(message.as_bytes()); }
        Ok(bytes)
    }

    /// Pure preview used independently by the broker and endpoint. Publication
    /// of this next state is ordered with their existing effect/receipt records.
    pub(crate) fn advance(&self, bytes: &[u8]) -> Result<Self, Error> {
        if self.finished { return Err(Error::WrongState); }
        let frame = ReleaseFrame::decode(bytes)?;
        if frame.profile != self.profile || !frame.prior.iter().copied().eq(self.messages()) {
            return Err(Error::Binding);
        }
        let mut next = self.clone();
        match frame.message {
            Some(message) => {
                next.visible.extend_from_slice(message.as_bytes());
                next.ends.push(next.visible.len());
            }
            None => next.finished = true,
        }
        Ok(next)
    }

    pub(crate) fn logical_bytes(&self) -> usize {
        self.visible.len() + self.ends.len() * std::mem::size_of::<usize>()
    }
}

/// Borrowed, bounded inspection of the exact review payload. Decoding proves
/// shape only; the live endpoint separately checks its actual prior history.
#[derive(Debug)]
pub struct ReleaseFrame<'a> {
    profile: StreamProfile,
    prior: Vec<&'a str>,
    message: Option<&'a str>,
}

impl<'a> ReleaseFrame<'a> {
    pub fn decode(bytes: &'a [u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_STREAM_FRAME_BYTES { return Err(Error::Limit); }
        let mut reader = Reader { bytes, offset: 0 };
        if reader.take(8)? != DOMAIN { return Err(Error::InvalidInput); }
        let id = reader.u64()?;
        let generation = reader.u64()?;
        let profile = StreamProfile::new(id, generation, reader.length()?, reader.length()?, reader.length()?)?;
        let count = reader.length()?;
        let prior_bytes = reader.length()?;
        let kind = reader.take(1)?[0];
        let added = reader.length()?;
        if count > profile.max_messages || prior_bytes > profile.max_stream_bytes || added > profile.max_message_bytes {
            return Err(Error::Limit);
        }
        if kind > 1 || (kind == 1 && added != 0) || (kind == 0 && added == 0) {
            return Err(Error::InvalidInput);
        }
        if prior_bytes.checked_add(added).ok_or(Error::Limit)? > profile.max_stream_bytes
            || (kind == 0 && count >= profile.max_messages)
        {
            return Err(Error::Limit);
        }
        let mut prior = Vec::with_capacity(count);
        let mut observed_bytes = 0_usize;
        for _ in 0..count {
            let length = reader.length()?;
            if length == 0 { return Err(Error::InvalidInput); }
            if length > profile.max_message_bytes { return Err(Error::Limit); }
            observed_bytes = observed_bytes.checked_add(length).ok_or(Error::Limit)?;
            if observed_bytes > prior_bytes { return Err(Error::Binding); }
            prior.push(reader.text(length)?);
        }
        if observed_bytes != prior_bytes { return Err(Error::Binding); }
        let message = if kind == 0 { Some(reader.text(added)?) } else { None };
        if reader.offset != bytes.len() { return Err(Error::InvalidInput); }
        Ok(Self { profile, prior, message })
    }

    pub fn profile(&self) -> StreamProfile { self.profile }
    pub fn prior_messages(&self) -> &[&'a str] { &self.prior }
    pub fn message(&self) -> Option<&'a str> { self.message }
    pub fn is_finish(&self) -> bool { self.message.is_none() }
}

struct Reader<'a> { bytes: &'a [u8], offset: usize }

impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(length).ok_or(Error::Limit)?;
        let part = self.bytes.get(self.offset..end).ok_or(Error::Incomplete)?;
        self.offset = end;
        Ok(part)
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| Error::Incomplete)?))
    }
    fn length(&mut self) -> Result<usize, Error> {
        let value = u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Incomplete)?);
        usize::try_from(value).map_err(|_| Error::Limit)
    }
    fn text(&mut self, length: usize) -> Result<&'a str, Error> {
        std::str::from_utf8(self.take(length)?).map_err(|_| Error::InvalidInput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> StreamProfile { StreamProfile::new(7, 1, 4, 8, 16).unwrap() }
    fn append(view: &StreamView, message: &str) -> StreamView {
        view.advance(&view.encode_message(message).unwrap()).unwrap()
    }

    #[test]
    fn golden_first_message_has_exact_lengths_and_no_implicit_history() {
        let view = StreamView::empty(profile());
        let actual = view.encode_message("Hi").unwrap();
        let mut expected = b"FASOUT\0\x01".to_vec();
        expected.extend_from_slice(&7_u64.to_be_bytes());
        expected.extend_from_slice(&1_u64.to_be_bytes());
        for value in [4_u32, 8, 16, 0, 0] { expected.extend_from_slice(&value.to_be_bytes()); }
        expected.extend_from_slice(&[0, 0, 0, 0, 2, b'H', b'i']);
        assert_eq!(actual, expected);
        assert_eq!(actual.len(), STREAM_HEADER_BYTES + 2);
        assert_eq!(ReleaseFrame::decode(&actual).unwrap().message(), Some("Hi"));
    }

    #[test]
    fn whole_utf8_messages_and_terminal_finish_preserve_visible_bytes() {
        let first = append(&StreamView::empty(profile()), "é");
        let second = append(&first, "世界");
        assert_eq!(second.visible(), "é世界".as_bytes());
        assert_eq!(second.messages().collect::<Vec<_>>(), vec!["é", "世界"]);
        let finish = second.encode_finish().unwrap();
        let parsed = ReleaseFrame::decode(&finish).unwrap();
        assert_eq!(parsed.prior_messages(), &["é", "世界"]);
        assert!(parsed.is_finish());
        let closed = second.advance(&finish).unwrap();
        assert!(closed.finished());
        assert_eq!(closed.visible(), second.visible());
        assert_eq!(closed.encode_message("x"), Err(Error::WrongState));
        assert_eq!(closed.encode_finish(), Err(Error::WrongState));
    }

    #[test]
    fn identical_concatenation_cannot_hide_changed_prior_message_boundaries() {
        let empty = StreamView::empty(profile());
        let left = append(&append(&empty, "ab"), "c");
        let right = append(&append(&empty, "a"), "bc");
        assert_eq!(left.visible(), right.visible());
        assert_eq!(left.message_count(), right.message_count());
        let substitution = right.encode_message("d").unwrap();
        assert_eq!(left.advance(&substitution), Err(Error::Binding));
        assert_ne!(left.encode_message("d").unwrap(), substitution);
    }

    #[test]
    fn truncation_trailing_data_and_partial_unicode_never_release_a_prefix() {
        let view = append(&StreamView::empty(profile()), "old");
        let bytes = view.encode_message("é").unwrap();
        for end in 0..bytes.len() { assert!(ReleaseFrame::decode(&bytes[..end]).is_err()); }
        let mut extra = bytes.clone(); extra.push(0);
        assert!(ReleaseFrame::decode(&extra).is_err());
        let mut invalid = bytes.clone(); *invalid.last_mut().unwrap() = 0xff;
        assert!(view.advance(&invalid).is_err());
        let mut invalid = bytes; invalid[44] = 2;
        assert!(view.advance(&invalid).is_err());
        assert_eq!(view.visible(), b"old");
    }

    #[test]
    fn message_and_total_caps_still_allow_an_explicit_finish() {
        let profile = StreamProfile::new(1, 1, 2, 4, 5).unwrap();
        let empty = StreamView::empty(profile);
        assert_eq!(empty.encode_message(""), Err(Error::InvalidInput));
        assert_eq!(empty.encode_message("12345"), Err(Error::Limit));
        let full = append(&append(&empty, "123"), "45");
        assert_eq!(full.encode_message("6"), Err(Error::Limit));
        assert!(full.advance(&full.encode_finish().unwrap()).unwrap().finished());
        assert!(empty.advance(&empty.encode_finish().unwrap()).unwrap().finished());
        assert!(StreamProfile::new(0, 1, 1, 1, 1).is_err());
        assert!(StreamProfile::new(1, 1, 128, 1, 1).is_err());
        assert!(StreamProfile::new(1, 1, 1, 4097, 8192).is_err());
        assert!(ReleaseFrame::decode(&vec![0; MAX_STREAM_FRAME_BYTES + 1]).is_err());
    }

    #[test]
    fn stale_prefix_and_changed_profile_refuse_without_rewriting_history() {
        let empty = StreamView::empty(profile());
        let first = empty.encode_message("first").unwrap();
        let advanced = empty.advance(&first).unwrap();
        assert_eq!(advanced.advance(&first), Err(Error::Binding));
        let foreign = StreamView::empty(StreamProfile::new(7, 1, 3, 8, 16).unwrap());
        assert_eq!(empty.advance(&foreign.encode_message("first").unwrap()), Err(Error::Binding));
        assert_eq!(advanced.visible(), b"first");
    }
}
