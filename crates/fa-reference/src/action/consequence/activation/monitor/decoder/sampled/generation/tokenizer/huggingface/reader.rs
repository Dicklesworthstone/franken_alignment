//! Bounded reader admission and fresh-owner text integration. No implicit file
//! path, network access, execution fallback, or unreviewed numerical output.

use super::{ByteBpe, DecoderProfile, MAX_JSON_BYTES};
use crate::action::consequence::activation::monitor::decoder::MonitoringStatus;
use crate::action::consequence::activation::monitor::decoder::sampled::MonitoredSampledDecoder;
use crate::action::consequence::activation::monitor::decoder::sampled::generation::text::TextDecoder;
use std::io::{self, Read};

impl ByteBpe {
    /// Read one finite JSON document under an explicit total-byte bound and then
    /// invoke the ORIGINAL strict importer. Read at most max_bytes + 1 bytes;
    /// the extra byte proves oversize and is never retained as a valid prefix.
    /// Successful admission requires actual EOF, not a short read, WouldBlock,
    /// timeout or a syntactically complete prefix before a later reader failure.
    ///
    /// The caller owns the reader and its authenticity/deadline/permission policy.
    /// File and fragmented-reader inputs use this same path. Interrupted reads
    /// retry; other I/O errors propagate unchanged. Parser/contract errors become
    /// InvalidData, bad caller bounds InvalidInput, and allocation refusal
    /// OutOfMemory. Neither partial bytes nor a partial tokenizer escape on error.
    pub fn read_huggingface_json<R: Read + ?Sized>(expected: &DecoderProfile,
        reader: &mut R, max_bytes: usize) -> io::Result<Self>
    {
        if max_bytes == 0 || max_bytes > MAX_JSON_BYTES {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "tokenizer JSON byte bound"));
        }
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 8192];
        loop {
            let remaining = max_bytes - bytes.len();
            let window = chunk.len().min(remaining + 1);
            let count = match reader.read(&mut chunk[..window]) {
                Ok(count) => count,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            };
            if count > window {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "reader exceeded supplied buffer"));
            }
            if count == 0 { break; }
            if count > remaining {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "tokenizer JSON exceeds byte bound"));
            }
            // Geometric growth avoids one reallocation per tiny fragmented read.
            // Requested payload capacity never exceeds the caller's hard cap.
            let needed = bytes.len() + count; // count <= remaining above
            if needed > bytes.capacity() {
                let target = bytes.capacity().saturating_mul(2).max(chunk.len())
                    .min(max_bytes).max(needed);
                bytes.try_reserve_exact(target - bytes.len())
                    .map_err(|_| io::Error::new(io::ErrorKind::OutOfMemory, "bounded tokenizer JSON allocation"))?;
            }
            bytes.extend_from_slice(&chunk[..count]);
        }
        Self::from_huggingface_json(expected, &bytes, max_bytes)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData,
                format!("tokenizer JSON admission: {error:?}")))
    }
}

impl TextDecoder {
    /// Import a compatible tokenizer directly into a FRESH monitored numerical
    /// owner. Its actual DecoderProfile supplies the binding; JSON cannot choose
    /// a second profile or replace the tokenizer inside an existing KV history.
    ///
    /// Existing history, consumed draws and non-ready status refuse BEFORE reader
    /// I/O. Original TextDecoder::new still performs final admission. No inference
    /// runs during construction, and every subsequent generated token goes through
    /// the existing mandatory monitor, sampler, cursor and output-byte checks.
    /// This consumes the supplied owner even on refusal, as TextDecoder::new does.
    /// Model/tokenizer authenticity and exact trained semantics remain assumptions.
    pub fn from_huggingface_reader<R: Read + ?Sized>(decoder: MonitoredSampledDecoder,
        reader: &mut R, max_bytes: usize) -> io::Result<Self>
    {
        if decoder.position() != 0 || decoder.sampled_draws() != 0
            || decoder.status() != MonitoringStatus::Ready
        {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "fresh ready monitored decoder required"));
        }
        let tokenizer = ByteBpe::read_huggingface_json(decoder.profile(), reader, max_bytes)?;
        Self::new(decoder, tokenizer).map_err(|error| io::Error::new(io::ErrorKind::InvalidData,
            format!("text decoder admission: {error:?}")))
    }
}
