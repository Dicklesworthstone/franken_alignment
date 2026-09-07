//! Exact, bounded qualification of `rustc -Vv` output.
//!
//! This checks that the process output names the release commit and host the
//! operator qualified for a gate run. It does not authenticate the executable
//! selected through `PATH`, the Rust distribution, or the surrounding host.

/// The largest verbose-version output this bounded parser accepts.
///
/// `rustc -Vv` is a short, line-oriented header. A larger input is not useful
/// evidence and must be refused before UTF-8 decoding or allocating field
/// values.
const MAX_VERBOSE_VERSION_BYTES: usize = 16 * 1024;

const ZERO_COMMIT: &str = "0000000000000000000000000000000000000000";

/// Check a captured `rustc -Vv` header against a qualified commit and host.
///
/// The three fields that establish this limited identity -- `release`,
/// `commit-hash`, and `host` -- must each occur exactly once. The release must
/// be a nightly release, the reported commit must be an exact, non-placeholder
/// 40-hex match for `expected_commit`, and the host must exactly match
/// `expected_host`.
///
/// This is intentionally a parser over captured process output. Capturing the
/// output and retaining its raw log are responsibilities of the caller.
pub fn check_verbose_version(
    bytes: &[u8],
    expected_commit: &str,
    expected_host: &str,
) -> Result<(), String> {
    if bytes.len() > MAX_VERBOSE_VERSION_BYTES {
        return Err(format!(
            "rustc -Vv output is {} bytes; maximum is {MAX_VERBOSE_VERSION_BYTES}",
            bytes.len()
        ));
    }

    if !is_non_placeholder_commit(expected_commit) {
        return Err("qualified rustc commit must be a non-placeholder 40-hex value".to_string());
    }
    if expected_host.is_empty() || expected_host.trim() != expected_host {
        return Err("qualified rustc host must be a non-empty exact value".to_string());
    }

    let output = std::str::from_utf8(bytes)
        .map_err(|_| "rustc -Vv output is not valid UTF-8".to_string())?;
    let mut release = None;
    let mut commit = None;
    let mut host = None;

    for line in output.lines() {
        if let Some(value) = line.strip_prefix("release:") {
            record_field("release", &mut release, value)?;
        } else if let Some(value) = line.strip_prefix("commit-hash:") {
            record_field("commit-hash", &mut commit, value)?;
        } else if let Some(value) = line.strip_prefix("host:") {
            record_field("host", &mut host, value)?;
        }
    }

    let release = required_field("release", release)?;
    let commit = required_field("commit-hash", commit)?;
    let host = required_field("host", host)?;

    if !is_nightly_release(&release) {
        return Err(format!(
            "rustc release `{release}` is not a nightly release"
        ));
    }
    if !is_non_placeholder_commit(&commit) {
        return Err(format!(
            "rustc commit-hash `{commit}` is not a non-placeholder 40-hex value"
        ));
    }
    if commit != expected_commit {
        return Err(format!(
            "rustc commit-hash `{commit}` does not equal qualified commit `{expected_commit}`"
        ));
    }
    if host != expected_host {
        return Err(format!(
            "rustc host `{host}` does not equal qualified host `{expected_host}`"
        ));
    }

    Ok(())
}

fn record_field(field: &str, slot: &mut Option<String>, value: &str) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("rustc -Vv contains duplicate `{field}` field"));
    }
    *slot = Some(value.trim().to_string());
    Ok(())
}

fn required_field(field: &str, value: Option<String>) -> Result<String, String> {
    value.ok_or_else(|| format!("rustc -Vv is missing required `{field}` field"))
}

fn is_nightly_release(release: &str) -> bool {
    release
        .strip_suffix("-nightly")
        .is_some_and(|version| !version.is_empty())
}

fn is_non_placeholder_commit(commit: &str) -> bool {
    commit.len() == 40
        && commit != ZERO_COMMIT
        && commit.as_bytes().iter().all(u8::is_ascii_hexdigit)
}

#[cfg(test)]
mod tests {
    use super::{MAX_VERBOSE_VERSION_BYTES, check_verbose_version};

    const COMMIT: &str = "5a2be9f5f075d31e3ca5526b5b029881ce441253";
    const HOST: &str = "x86_64-unknown-linux-gnu";

    // Exact header retained in artifacts/execution/2026-09-07-epoch2-attempt1.log
    // (lines 44-50), the qualified Linux epoch-2 execution. This fixture is a
    // captured process header, not a test that executes an ambient `rustc`.
    const EPOCH2_RUSTC_VV: &str = "rustc 1.100.0-nightly (5a2be9f5f 2026-09-06)\n\
binary: rustc\n\
commit-hash: 5a2be9f5f075d31e3ca5526b5b029881ce441253\n\
commit-date: 2026-09-06\n\
host: x86_64-unknown-linux-gnu\n\
release: 1.100.0-nightly\n\
LLVM version: 23.1.1\n";

    fn assert_refused(bytes: &[u8], expected_commit: &str, expected_host: &str, cause: &str) {
        let error = check_verbose_version(bytes, expected_commit, expected_host)
            .expect_err("the planted identity defect must refuse");
        assert!(error.contains(cause), "expected {cause:?} in {error:?}");
    }

    fn replace_field(header: &str, field: &str, replacement: &str) -> String {
        let needle = format!("{field}:");
        let mut replaced = false;
        let mut output = String::new();
        for line in header.lines() {
            if line.starts_with(&needle) {
                assert!(!replaced, "fixture has duplicate {field} field");
                output.push_str(&needle);
                output.push(' ');
                output.push_str(replacement);
                output.push('\n');
                replaced = true;
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }
        assert!(replaced, "fixture lacks {field} field");
        output
    }

    fn without_field(header: &str, field: &str) -> String {
        let needle = format!("{field}:");
        let mut removed = false;
        let mut output = String::new();
        for line in header.lines() {
            if line.starts_with(&needle) {
                assert!(!removed, "fixture has duplicate {field} field");
                removed = true;
            } else {
                output.push_str(line);
                output.push('\n');
            }
        }
        assert!(removed, "fixture lacks {field} field");
        output
    }

    #[test]
    fn retained_qualified_epoch2_header_passes_exactly() {
        assert_eq!(
            check_verbose_version(EPOCH2_RUSTC_VV.as_bytes(), COMMIT, HOST),
            Ok(())
        );
    }

    #[test]
    fn every_required_identity_field_is_required_and_unique() {
        for field in ["release", "commit-hash", "host"] {
            let absent = without_field(EPOCH2_RUSTC_VV, field);
            assert_refused(absent.as_bytes(), COMMIT, HOST, field);

            let duplicate = format!("{EPOCH2_RUSTC_VV}{field}: planted-duplicate\n");
            assert_refused(duplicate.as_bytes(), COMMIT, HOST, "duplicate");
        }
    }

    #[test]
    fn stable_release_cannot_launder_the_qualified_commit_and_host() {
        let stable = replace_field(EPOCH2_RUSTC_VV, "release", "1.100.0");
        assert_refused(stable.as_bytes(), COMMIT, HOST, "not a nightly");
    }

    #[test]
    fn commit_must_be_exact_40_hex_and_not_a_placeholder() {
        let wrong_but_well_formed = replace_field(
            EPOCH2_RUSTC_VV,
            "commit-hash",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );
        assert_refused(
            wrong_but_well_formed.as_bytes(),
            COMMIT,
            HOST,
            "does not equal qualified",
        );

        let malformed = replace_field(EPOCH2_RUSTC_VV, "commit-hash", "not-40-hex");
        assert_refused(malformed.as_bytes(), COMMIT, HOST, "non-placeholder 40-hex");

        let placeholder = replace_field(
            EPOCH2_RUSTC_VV,
            "commit-hash",
            "0000000000000000000000000000000000000000",
        );
        assert_refused(
            placeholder.as_bytes(),
            COMMIT,
            HOST,
            "non-placeholder 40-hex",
        );
    }

    #[test]
    fn host_must_match_the_qualified_execution_host() {
        let wrong_host = replace_field(EPOCH2_RUSTC_VV, "host", "aarch64-apple-darwin");
        assert_refused(
            wrong_host.as_bytes(),
            COMMIT,
            HOST,
            "does not equal qualified host",
        );
    }

    #[test]
    fn malformed_expectations_invalid_utf8_and_oversized_output_refuse() {
        assert_refused(
            EPOCH2_RUSTC_VV.as_bytes(),
            "not-a-qualified-commit",
            HOST,
            "qualified rustc commit",
        );
        assert_refused(
            EPOCH2_RUSTC_VV.as_bytes(),
            COMMIT,
            "",
            "qualified rustc host",
        );

        let mut invalid_utf8 = EPOCH2_RUSTC_VV.as_bytes().to_vec();
        invalid_utf8.push(0xff);
        assert_refused(&invalid_utf8, COMMIT, HOST, "UTF-8");

        let oversized = vec![b'x'; MAX_VERBOSE_VERSION_BYTES + 1];
        assert_refused(&oversized, COMMIT, HOST, "maximum");
    }
}
