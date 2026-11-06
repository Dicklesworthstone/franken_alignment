//! Strict caller-selected verification parameters, never learned from FARJ.

use std::collections::BTreeMap;

use crate::json::{self, Json};
use xtask::independent_verify::{ExpectedContext, Limits, RecoveryReserve};

use super::{Failure, MAX_CONTEXT_BYTES};

pub(super) const SCHEMA: &str = "fa.rights_verification_context/1";

fn fields<'a>(
    value: &'a Json,
    expected: &[&str],
    name: &'static str,
) -> Result<&'a BTreeMap<String, Json>, Failure> {
    let object = value.as_object().ok_or_else(|| {
        Failure::context(
            name,
            "context fields must be objects with exactly the required members",
        )
    })?;
    if object.len() != expected.len() || expected.iter().any(|key| !object.contains_key(*key)) {
        return Err(Failure::context(
            name,
            "every context member is required and unknown members are rejected",
        ));
    }
    Ok(object)
}

fn unsigned(value: &Json, name: &'static str) -> Result<u64, Failure> {
    // The syntax parser retains lexical spelling. Do not coerce signed,
    // fractional, exponential, string or boolean representations into integers.
    let Json::Number(number) = value else {
        return Err(Failure::context(
            name,
            "context integer must be an unsigned decimal JSON number",
        ));
    };
    if !number.lexeme().bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Failure::context(
            name,
            "context integer must be an unsigned decimal JSON number",
        ));
    }
    number
        .as_u64()
        .ok_or_else(|| Failure::context(name, "context integer exceeds u64"))
}

fn identity(value: &Json, name: &'static str) -> Result<[u8; 32], Failure> {
    let text = value.as_str().ok_or_else(|| {
        Failure::context(
            name,
            "identity must contain exactly 64 hexadecimal characters",
        )
    })?;
    if text.len() != 64 || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Failure::context(
            name,
            "identity must contain exactly 64 hexadecimal characters",
        ));
    }
    let digit = |byte| match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => unreachable!("all identity bytes were checked before conversion"),
    };
    let mut bytes = [0; 32];
    for (output, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        *output = (digit(pair[0]) << 4) | digit(pair[1]);
    }
    Ok(bytes)
}

pub(super) fn parse(bytes: &[u8]) -> Result<ExpectedContext, Failure> {
    let document = json::parse(
        bytes,
        json::Limits {
            max_bytes: MAX_CONTEXT_BYTES,
            max_depth: 3,
            max_items: 32,
            max_string_bytes: 128,
        },
    )
    .map_err(|error| {
        let mut failure = Failure::new("invalid_context_json", "context", error.to_string());
        failure.byte_offset = Some(error.offset as u64);
        failure
    })?;
    let context = fields(
        &document,
        &[
            "schema_version",
            "run",
            "policy",
            "total_units",
            "limits",
            "recovery",
        ],
        "context",
    )?;
    if context["schema_version"].as_str() != Some(SCHEMA) {
        return Err(Failure::context(
            "schema_version",
            "unsupported context schema version",
        ));
    }
    let limits = fields(
        &context["limits"],
        &["max_frame_bytes", "max_records", "max_total_bytes"],
        "limits",
    )?;
    let recovery = match &context["recovery"] {
        Json::Null => None,
        value => {
            let recovery = fields(value, &["records", "bytes"], "recovery")?;
            Some(RecoveryReserve {
                records: unsigned(&recovery["records"], "recovery.records")?,
                bytes: unsigned(&recovery["bytes"], "recovery.bytes")?,
            })
        }
    };
    let expected = ExpectedContext {
        run: identity(&context["run"], "run")?,
        policy: identity(&context["policy"], "policy")?,
        total_units: unsigned(&context["total_units"], "total_units")?,
        limits: Limits {
            max_frame_bytes: unsigned(&limits["max_frame_bytes"], "limits.max_frame_bytes")?
                .try_into()
                .map_err(|_| {
                    Failure::context("limits.max_frame_bytes", "frame limit exceeds u32")
                })?,
            max_records: unsigned(&limits["max_records"], "limits.max_records")?,
            max_total_bytes: unsigned(&limits["max_total_bytes"], "limits.max_total_bytes")?,
        },
        recovery,
    };
    expected.validate().map_err(Failure::from)?;
    Ok(expected)
}
