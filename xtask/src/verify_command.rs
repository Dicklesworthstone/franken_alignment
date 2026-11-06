//! Read-only operator entry point for the independent FARJ accounting profile.
//! The command owns neither authority nor a journal writer. Its files and
//! expected context are explicit inputs, independent of a workspace checkout.

mod context;
mod report;

use std::ffi::OsString;
use std::fs::{self, File, Metadata};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xtask::independent_verify::{ExpectedContext, VerificationReport, VerifyError, verify};

const MAX_CONTEXT_BYTES: usize = 4096;
const MAX_CONTEXT_READ_CALLS: usize = MAX_CONTEXT_BYTES + 1 + 1024;
const USAGE: &str =
    "Usage: xtask verify rights-journal --context PATH --journal PATH --anchor PATH";

struct Arguments {
    context: PathBuf,
    journal: PathBuf,
    anchor: PathBuf,
}

struct Verified {
    expected: ExpectedContext,
    report: VerificationReport,
}

struct Failure {
    reason_code: &'static str,
    input: &'static str,
    detail: String,
    field: Option<&'static str>,
    byte_offset: Option<u64>,
    sequence: Option<u64>,
    io_kind: Option<io::ErrorKind>,
}

impl Failure {
    fn new(reason_code: &'static str, input: &'static str, detail: impl Into<String>) -> Self {
        Self {
            reason_code,
            input,
            detail: detail.into(),
            field: None,
            byte_offset: None,
            sequence: None,
            io_kind: None,
        }
    }

    fn context(field: &'static str, detail: &'static str) -> Self {
        let mut failure = Self::new("invalid_context", "context", detail);
        failure.field = Some(field);
        failure
    }

    fn io(input: &'static str, error: io::Error) -> Self {
        let mut failure = Self::new(
            "input_io",
            input,
            "input could not be inspected, opened or read",
        );
        failure.io_kind = Some(error.kind());
        failure
    }

    fn too_large(input: &'static str, maximum: u64) -> Self {
        let mut failure = Self::new(
            "input_limit_exceeded",
            input,
            "input exceeds its independently selected byte limit",
        );
        failure.byte_offset = Some(maximum);
        failure
    }
}

impl From<VerifyError> for Failure {
    fn from(error: VerifyError) -> Self {
        Self {
            reason_code: error.kind_name(),
            input: error.location.input.as_str(),
            detail: error.explanation().into(),
            field: None,
            byte_offset: Some(error.location.offset),
            sequence: error.location.sequence,
            io_kind: error.io_kind,
        }
    }
}

fn parse_arguments(arguments: &[OsString]) -> Result<Arguments, Failure> {
    if !arguments
        .first()
        .is_some_and(|argument| argument == "rights-journal")
    {
        return Err(Failure::new("unsupported_profile", "arguments", USAGE));
    }
    if arguments.len() != 7 {
        return Err(Failure::new("invalid_arguments", "arguments", USAGE));
    }
    let mut context = None;
    let mut journal = None;
    let mut anchor = None;
    for pair in arguments[1..].chunks_exact(2) {
        let slot = match pair[0].to_str() {
            Some("--context") => &mut context,
            Some("--journal") => &mut journal,
            Some("--anchor") => &mut anchor,
            _ => return Err(Failure::new("invalid_arguments", "arguments", USAGE)),
        };
        if pair[1].is_empty() || slot.replace(PathBuf::from(&pair[1])).is_some() {
            return Err(Failure::new("invalid_arguments", "arguments", USAGE));
        }
    }
    match (context, journal, anchor) {
        (Some(context), Some(journal), Some(anchor)) => Ok(Arguments {
            context,
            journal,
            anchor,
        }),
        _ => Err(Failure::new("invalid_arguments", "arguments", USAGE)),
    }
}

struct InputFile {
    file: File,
    #[cfg(unix)]
    metadata: Metadata,
    #[cfg(not(unix))]
    canonical_path: PathBuf,
}

fn check_metadata(metadata: &Metadata, input: &'static str, maximum: u64) -> Result<(), Failure> {
    if !metadata.is_file() {
        return Err(Failure::new(
            "non_regular_input",
            input,
            "verification accepts regular files only",
        ));
    }
    if metadata.len() > maximum {
        return Err(Failure::too_large(input, maximum));
    }
    Ok(())
}

fn open_regular(path: &Path, input: &'static str, maximum: u64) -> Result<InputFile, Failure> {
    // Inspect before open so ordinary FIFO/device inputs never wait for a
    // producer. Inspect the opened handle as well; paths are not file identity.
    // This does not establish an immutable filesystem snapshot against races.
    let metadata = fs::symlink_metadata(path).map_err(|error| Failure::io(input, error))?;
    check_metadata(&metadata, input, maximum)?;
    let file = File::open(path).map_err(|error| Failure::io(input, error))?;
    let metadata = file.metadata().map_err(|error| Failure::io(input, error))?;
    check_metadata(&metadata, input, maximum)?;
    Ok(InputFile {
        file,
        #[cfg(unix)]
        metadata,
        #[cfg(not(unix))]
        canonical_path: fs::canonicalize(path).map_err(|error| Failure::io(input, error))?,
    })
}

fn aliases(journal: &InputFile, anchor: &InputFile) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        journal.metadata.dev() == anchor.metadata.dev()
            && journal.metadata.ino() == anchor.metadata.ino()
    }
    #[cfg(not(unix))]
    {
        // std exposes no portable opened-handle file identifier. This catches
        // identical resolved paths; other alias relationships remain unknown.
        journal.canonical_path == anchor.canonical_path
    }
}

fn read_context(path: &Path) -> Result<ExpectedContext, Failure> {
    let mut input = open_regular(path, "context", MAX_CONTEXT_BYTES as u64)?;
    let mut bytes = Vec::with_capacity(MAX_CONTEXT_BYTES + 1);
    let mut buffer = [0; 512];
    for _ in 0..MAX_CONTEXT_READ_CALLS {
        let width = buffer.len().min(MAX_CONTEXT_BYTES + 1 - bytes.len());
        match input.file.read(&mut buffer[..width]) {
            Ok(0) => return context::parse(&bytes),
            Ok(count) => {
                bytes.extend_from_slice(&buffer[..count]);
                if bytes.len() > MAX_CONTEXT_BYTES {
                    return Err(Failure::too_large("context", MAX_CONTEXT_BYTES as u64));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => {
                let mut failure = Failure::io("context", error);
                failure.byte_offset = Some(bytes.len() as u64);
                return Err(failure);
            }
        }
    }
    let mut failure = Failure::new(
        "read_budget_exhausted",
        "context",
        "context input exceeded its finite read-attempt budget",
    );
    failure.byte_offset = Some(bytes.len() as u64);
    Err(failure)
}

fn execute(arguments: &[OsString]) -> Result<Verified, Failure> {
    let arguments = parse_arguments(arguments)?;
    // The parser also calls ExpectedContext::validate. Never open an untrusted
    // history to discover the context against which that history will be judged.
    let expected = read_context(&arguments.context)?;
    let maximum = expected.limits.max_total_bytes;
    let journal = open_regular(&arguments.journal, "journal", maximum)?;
    let anchor = open_regular(&arguments.anchor, "anchor", maximum)?;
    if aliases(&journal, &anchor) {
        return Err(Failure::new(
            "aliased_inputs",
            "anchor",
            "journal and retained prefix must be distinct file objects; distinct files with identical bytes are allowed",
        ));
    }
    // One extra byte distinguishes exact-limit EOF from a growing input. The
    // independent verifier enforces every frame and cumulative limit itself.
    let journal = BufReader::with_capacity(8192, journal.file.take(maximum + 1));
    let anchor = BufReader::with_capacity(8192, anchor.file.take(maximum + 1));
    let report = verify(journal, anchor, expected).map_err(Failure::from)?;
    Ok(Verified { expected, report })
}

pub(super) fn run(arguments: &[OsString]) -> ExitCode {
    let (output, status) = match execute(arguments) {
        Ok(verified) => (report::success(&verified), ExitCode::SUCCESS),
        Err(error) => (report::failure(&error), ExitCode::FAILURE),
    };
    let mut stdout = io::stdout().lock();
    if stdout
        .write_all(output.as_bytes())
        .and_then(|()| stdout.write_all(b"\n"))
        .is_err()
    {
        // A broken output stream cannot carry a complete JSON response. Do not
        // report success merely because verification finished before that fault.
        eprintln!("rights-journal verification response could not be written");
        return ExitCode::FAILURE;
    }
    status
}
