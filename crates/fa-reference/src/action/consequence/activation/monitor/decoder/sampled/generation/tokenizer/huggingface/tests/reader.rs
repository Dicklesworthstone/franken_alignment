use super::*;
use std::io::{self, Cursor, Read};

struct Fragmented {
    source: Cursor<Vec<u8>>,
    calls: usize,
    delivered: usize,
}
impl Fragmented {
    fn new(bytes: &[u8]) -> Self {
        Self { source: Cursor::new(bytes.to_vec()), calls: 0, delivered: 0 }
    }
}
impl Read for Fragmented {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if self.calls % 3 == 1 { return Err(io::Error::from(io::ErrorKind::Interrupted)); }
        let length = output.len().min(7);
        let count = self.source.read(&mut output[..length])?;
        self.delivered += count;
        Ok(count)
    }
}

#[test]
fn hf_reader_requires_eof_and_accepts_fragmentation_and_interruptions() {
    let json = document(false);
    let mut reader = Fragmented::new(json.as_bytes());
    let tokenizer = ByteBpe::read_huggingface_json(&profile(), &mut reader, json.len()).unwrap();
    assert_eq!(encode(&tokenizer, b"abc"), vec![258]);
    assert_eq!(reader.delivered, json.len());
    assert!(reader.calls > 2);
    assert_eq!(tokenizer.to_bytes().unwrap(), import(&json).unwrap().to_bytes().unwrap());
}

#[test]
fn hf_reader_never_treats_a_size_limited_or_trailing_prefix_as_complete() {
    let json = document(false);
    let mut short_budget = Fragmented::new(json.as_bytes());
    let error = ByteBpe::read_huggingface_json(&profile(), &mut short_budget, json.len() - 1).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert_eq!(short_budget.delivered, json.len()); // exactly bound + one, never an arbitrary read-ahead
    let mut bad_bound = Fragmented::new(json.as_bytes());
    assert_eq!(ByteBpe::read_huggingface_json(&profile(), &mut bad_bound, MAX_JSON_BYTES + 1).unwrap_err().kind(),
        io::ErrorKind::InvalidInput);
    assert_eq!(bad_bound.calls, 0);
    assert_eq!(ByteBpe::read_huggingface_json(&profile(), &mut bad_bound, 0).unwrap_err().kind(),
        io::ErrorKind::InvalidInput);
    assert_eq!(bad_bound.calls, 0);
    for invalid in [json[..json.len() - 1].to_owned(), format!("{json}{{}}")]
    {
        let mut reader = Cursor::new(invalid.as_bytes());
        assert_eq!(ByteBpe::read_huggingface_json(&profile(), &mut reader, invalid.len()).unwrap_err().kind(),
            io::ErrorKind::InvalidData);
    }
    let mut valid = Cursor::new(json.as_bytes());
    assert_eq!(encode(&ByteBpe::read_huggingface_json(&profile(), &mut valid, json.len()).unwrap(), b"abc"), vec![258]);
}

#[test]
fn hf_io_error_after_complete_json_is_not_eof_or_success() {
    struct FailAtEnd<'a> { bytes: &'a [u8], failure: io::ErrorKind }
    impl Read for FailAtEnd<'_> {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
            if self.bytes.is_empty() { return Err(io::Error::from(self.failure)); }
            self.bytes.read(output)
        }
    }
    let json = document(false);
    assert!(import(&json).is_ok());
    for failure in [io::ErrorKind::PermissionDenied, io::ErrorKind::WouldBlock, io::ErrorKind::TimedOut] {
        let mut reader = FailAtEnd { bytes: json.as_bytes(), failure };
        assert_eq!(ByteBpe::read_huggingface_json(&profile(), &mut reader, json.len()).unwrap_err().kind(), failure);
        assert!(reader.bytes.is_empty());
    }
}

#[test]
fn hf_reader_rejects_a_false_count_without_indexing_outside_the_buffer() {
    struct InvalidReader;
    impl Read for InvalidReader {
        fn read(&mut self, output: &mut [u8]) -> io::Result<usize> { Ok(output.len() + 1) }
    }
    assert_eq!(ByteBpe::read_huggingface_json(&profile(), &mut InvalidReader, 1).unwrap_err().kind(),
        io::ErrorKind::InvalidData);
}
