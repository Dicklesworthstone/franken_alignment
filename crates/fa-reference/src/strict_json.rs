//! Bounded strict JSON syntax shared by the reference formats and operator inputs.
//!
//! Parsing preserves lexical number spelling and rejects duplicate keys. This
//! syntax layer does not define canonical encoding or document semantics; the
//! canonical draft profile and operator admission apply their own checks.

use std::collections::{BTreeMap, BTreeSet};

// Recursive descent stays bounded even if a caller accidentally configures an
// unbounded `Limits::max_depth` for untrusted operator input.
const IMPLEMENTATION_MAX_DEPTH: usize = 128;

/// Limits applied before allocating unbounded JSON input structures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_depth: usize,
    pub max_items: usize,
    pub max_string_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 64 * 1024 * 1024,
            max_depth: 128,
            max_items: 4_000_000,
            max_string_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Parsed JSON values. Numbers deliberately retain their exact JSON lexeme.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    pub fn get(&self, key: &str) -> Option<&Json> {
        self.as_object()?.get(key)
    }

    pub fn as_object(&self) -> Option<&BTreeMap<String, Json>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Json]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Number(value) => value.as_u64(),
            _ => None,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Number(_) => "number",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }
}

/// An exact JSON number spelling with opt-in primitive conversions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Number {
    lexeme: String,
}

impl Number {
    fn new(lexeme: String) -> Self {
        Self { lexeme }
    }

    pub fn lexeme(&self) -> &str {
        &self.lexeme
    }

    pub fn as_u64(&self) -> Option<u64> {
        self.lexeme().parse().ok()
    }
}

/// The reason a strict JSON parse failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    InvalidUtf8,
    UnexpectedEof,
    TrailingValue,
    InvalidEscape,
    LoneSurrogate,
    ControlCharacter,
    InvalidNumber,
    DuplicateKey(String),
    DepthLimit,
    SizeLimit,
    ItemLimit,
    StringLimit,
    Unexpected(u8),
    ExpectedValue,
}

/// A parse failure with its byte offset in the supplied input.
/// For `SizeLimit`, the offset is the first byte outside the permitted prefix;
/// line and column are zero (unavailable), since the prefix may cut UTF-8 and
/// oversized input is refused before text validation. Otherwise coordinates
/// are one-based, including the valid prefix before an invalid UTF-8 byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub offset: usize,
    pub line: u32,
    pub column: u32,
}

impl Error {
    fn at(input: &str, offset: usize, kind: ErrorKind) -> Self {
        let prefix = &input[..offset.min(input.len())];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit_once('\n')
            .map_or(prefix.chars().count() + 1, |(_, tail)| {
                tail.chars().count() + 1
            });
        Self {
            kind,
            offset,
            line: u32::try_from(line).unwrap_or(u32::MAX),
            column: u32::try_from(column).unwrap_or(u32::MAX),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "strict JSON parse error {:?} at byte {} (line {}, column {})",
            self.kind, self.offset, self.line, self.column
        )
    }
}

impl std::error::Error for Error {}

/// Parse strict JSON under explicit resource bounds.
pub fn parse(input: &[u8], limits: Limits) -> Result<Json, Error> {
    if input.len() > limits.max_bytes {
        return Err(Error {
            kind: ErrorKind::SizeLimit,
            offset: limits.max_bytes,
            line: 0,
            column: 0,
        });
    }
    let text = std::str::from_utf8(input).map_err(|error| {
        let offset = error.valid_up_to();
        let prefix =
            std::str::from_utf8(&input[..offset]).expect("UTF-8 error identifies a valid prefix");
        Error::at(prefix, offset, ErrorKind::InvalidUtf8)
    })?;
    let mut parser = Parser {
        input: text,
        position: 0,
        limits,
        item_count: 0,
    };
    parser.skip_whitespace();
    let value = parser.parse_value(0)?;
    parser.skip_whitespace();
    if parser.position != parser.input.len() {
        return Err(parser.error(ErrorKind::TrailingValue));
    }
    Ok(value)
}

struct Parser<'a> {
    input: &'a str,
    position: usize,
    limits: Limits,
    item_count: usize,
}

impl Parser<'_> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error::at(self.input, self.position, kind)
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.position).copied()
    }

    fn consume(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.position += 1;
        }
    }

    fn charge_item(&mut self) -> Result<(), Error> {
        self.item_count = self
            .item_count
            .checked_add(1)
            .ok_or_else(|| self.error(ErrorKind::ItemLimit))?;
        if self.item_count > self.limits.max_items {
            return Err(self.error(ErrorKind::ItemLimit));
        }
        Ok(())
    }

    fn parse_value(&mut self, depth: usize) -> Result<Json, Error> {
        self.charge_item()?;
        match self.peek() {
            Some(b'n') => {
                self.consume_literal(b"null")?;
                Ok(Json::Null)
            }
            Some(b't') => {
                self.consume_literal(b"true")?;
                Ok(Json::Bool(true))
            }
            Some(b'f') => {
                self.consume_literal(b"false")?;
                Ok(Json::Bool(false))
            }
            Some(b'"') => self.parse_string().map(Json::String),
            Some(b'[') => self.parse_array(depth),
            Some(b'{') => self.parse_object(depth),
            Some(b'-' | b'0'..=b'9') => self.parse_number().map(Number::new).map(Json::Number),
            // These are common non-JSON number spellings. Classifying them as
            // numbers makes the refusal causal rather than a generic token error.
            Some(b'.' | b'+' | b'N' | b'I') => Err(self.error(ErrorKind::InvalidNumber)),
            Some(byte) => Err(self.error(ErrorKind::Unexpected(byte))),
            None => Err(self.error(ErrorKind::UnexpectedEof)),
        }
    }

    fn consume_literal(&mut self, literal: &[u8]) -> Result<(), Error> {
        let end = self
            .position
            .checked_add(literal.len())
            .ok_or_else(|| self.error(ErrorKind::SizeLimit))?;
        if end > self.input.len() {
            return Err(self.error(ErrorKind::UnexpectedEof));
        }
        if self.input.as_bytes().get(self.position..end) == Some(literal) {
            self.position = end;
            Ok(())
        } else {
            Err(self.error(ErrorKind::ExpectedValue))
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Json, Error> {
        if depth >= self.limits.max_depth.min(IMPLEMENTATION_MAX_DEPTH) {
            return Err(self.error(ErrorKind::DepthLimit));
        }
        self.position += 1; // `[` matched by parse_value.
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume(b']') {
            return Ok(Json::Array(values));
        }
        loop {
            values.push(self.parse_value(depth + 1)?);
            self.skip_whitespace();
            if self.consume(b']') {
                return Ok(Json::Array(values));
            }
            if !self.consume(b',') {
                return Err(self.error(if self.peek().is_none() {
                    ErrorKind::UnexpectedEof
                } else {
                    ErrorKind::ExpectedValue
                }));
            }
            self.skip_whitespace();
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Json, Error> {
        if depth >= self.limits.max_depth.min(IMPLEMENTATION_MAX_DEPTH) {
            return Err(self.error(ErrorKind::DepthLimit));
        }
        self.position += 1; // `{` matched by parse_value.
        self.skip_whitespace();
        let mut entries = BTreeMap::new();
        let mut keys = BTreeSet::new();
        if self.consume(b'}') {
            return Ok(Json::Object(entries));
        }
        loop {
            if self.peek() != Some(b'"') {
                return Err(self.error(if self.peek().is_none() {
                    ErrorKind::UnexpectedEof
                } else {
                    ErrorKind::ExpectedValue
                }));
            }
            let key = self.parse_string()?;
            if !keys.insert(key.clone()) {
                return Err(self.error(ErrorKind::DuplicateKey(key)));
            }
            self.charge_item()?;
            self.skip_whitespace();
            if !self.consume(b':') {
                return Err(self.error(if self.peek().is_none() {
                    ErrorKind::UnexpectedEof
                } else {
                    ErrorKind::ExpectedValue
                }));
            }
            self.skip_whitespace();
            let value = self.parse_value(depth + 1)?;
            entries.insert(key, value);
            self.skip_whitespace();
            if self.consume(b'}') {
                return Ok(Json::Object(entries));
            }
            if !self.consume(b',') {
                return Err(self.error(if self.peek().is_none() {
                    ErrorKind::UnexpectedEof
                } else {
                    ErrorKind::ExpectedValue
                }));
            }
            self.skip_whitespace();
        }
    }

    fn parse_string(&mut self) -> Result<String, Error> {
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.position += 1;
        let mut value = String::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.position += 1;
                    return Ok(value);
                }
                Some(b'\\') => {
                    self.position += 1;
                    let escaped = match self.peek() {
                        Some(b'"') => {
                            self.position += 1;
                            '"'
                        }
                        Some(b'\\') => {
                            self.position += 1;
                            '\\'
                        }
                        Some(b'/') => {
                            self.position += 1;
                            '/'
                        }
                        Some(b'b') => {
                            self.position += 1;
                            '\u{0008}'
                        }
                        Some(b'f') => {
                            self.position += 1;
                            '\u{000C}'
                        }
                        Some(b'n') => {
                            self.position += 1;
                            '\n'
                        }
                        Some(b'r') => {
                            self.position += 1;
                            '\r'
                        }
                        Some(b't') => {
                            self.position += 1;
                            '\t'
                        }
                        Some(b'u') => {
                            self.position += 1;
                            self.parse_unicode_escape()?
                        }
                        Some(_) => return Err(self.error(ErrorKind::InvalidEscape)),
                        None => return Err(self.error(ErrorKind::UnexpectedEof)),
                    };
                    value.push(escaped);
                }
                Some(byte) if byte <= 0x1f => return Err(self.error(ErrorKind::ControlCharacter)),
                Some(byte) if byte.is_ascii() => {
                    self.position += 1;
                    value.push(char::from(byte));
                }
                Some(_) => {
                    let character = self.input[self.position..]
                        .chars()
                        .next()
                        .ok_or_else(|| self.error(ErrorKind::UnexpectedEof))?;
                    self.position += character.len_utf8();
                    value.push(character);
                }
                None => return Err(self.error(ErrorKind::UnexpectedEof)),
            }
            if value.len() > self.limits.max_string_bytes {
                return Err(self.error(ErrorKind::StringLimit));
            }
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, Error> {
        let first = self.parse_hex_u16()?;
        match first {
            0xd800..=0xdbff => {
                if !self.consume(b'\\') || !self.consume(b'u') {
                    return Err(self.error(ErrorKind::LoneSurrogate));
                }
                let second = self.parse_hex_u16()?;
                if !(0xdc00..=0xdfff).contains(&second) {
                    return Err(self.error(ErrorKind::LoneSurrogate));
                }
                let scalar =
                    0x1_0000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00);
                char::from_u32(scalar).ok_or_else(|| self.error(ErrorKind::LoneSurrogate))
            }
            0xdc00..=0xdfff => Err(self.error(ErrorKind::LoneSurrogate)),
            scalar => char::from_u32(u32::from(scalar))
                .ok_or_else(|| self.error(ErrorKind::InvalidEscape)),
        }
    }

    fn parse_hex_u16(&mut self) -> Result<u16, Error> {
        let mut value = 0_u16;
        for _ in 0..4 {
            let byte = self
                .peek()
                .ok_or_else(|| self.error(ErrorKind::UnexpectedEof))?;
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a' + 10),
                b'A'..=b'F' => u16::from(byte - b'A' + 10),
                _ => return Err(self.error(ErrorKind::InvalidEscape)),
            };
            self.position += 1;
            value = (value << 4) | digit;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<String, Error> {
        let start = self.position;
        self.consume(b'-');
        match self.peek() {
            Some(b'0') => {
                self.position += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.error(ErrorKind::InvalidNumber));
                }
            }
            Some(b'1'..=b'9') => {
                self.position += 1;
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.position += 1;
                }
            }
            Some(_) | None => return Err(self.error(ErrorKind::InvalidNumber)),
        }
        if self.consume(b'.') {
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error(ErrorKind::InvalidNumber));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.position += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.position += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.position += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error(ErrorKind::InvalidNumber));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.position += 1;
            }
        }
        if matches!(
            self.peek(),
            Some(b'.' | b'+' | b'-' | b'a'..=b'z' | b'A'..=b'Z' | b'_')
        ) {
            return Err(self.error(ErrorKind::InvalidNumber));
        }
        Ok(self.input[start..self.position].to_owned())
    }
}
