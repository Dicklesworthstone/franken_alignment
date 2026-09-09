//! Offline reference-only payload experiment. Explicit input files only; no
//! controller construction, network, output-file mutation or effect dispatch.
#![forbid(unsafe_code)]

use fa_reference::action::consequence::experiment::{
    Intervention, InterventionScope, PolicyExperiment,
};
use fa_reference::action::consequence::gate::containment::session::policy::controller::replay::{
    DecisionArchive, MAX_ARCHIVE_BYTES, ReviewAnchor,
};
use fa_reference::action::MAX_PAYLOAD_BYTES;
use std::env;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitCode;

fn read_limit(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let cap = u64::try_from(limit).ok().and_then(|value| value.checked_add(1))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid byte limit"))?;
    let mut bytes = Vec::new();
    reader.take(cap).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "input exceeds byte limit"));
    }
    Ok(bytes)
}

fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("open {path:?}: {error}"))?;
    let metadata = file.metadata().map_err(|error| format!("metadata {path:?}: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("not a regular file: {path:?}"));
    }
    read_limit(file, limit).map_err(|error| format!("read {path:?}: {error}"))
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args_os().skip(1).take(4).collect();
    if args.len() != 3 {
        return Err("usage: policy_counterfactual ANCHOR.bin ARCHIVE.bin REPLACEMENT_PAYLOAD".to_owned());
    }
    let anchor_bytes = read_file(Path::new(&args[0]), MAX_ARCHIVE_BYTES)?;
    let archive_bytes = read_file(Path::new(&args[1]), MAX_ARCHIVE_BYTES)?;
    let payload = read_file(Path::new(&args[2]), MAX_PAYLOAD_BYTES)?;
    let anchor = ReviewAnchor::from_bytes(&anchor_bytes)
        .map_err(|error| format!("decode anchor: {error:?}"))?;
    let archive = DecisionArchive::from_bytes(&archive_bytes)
        .map_err(|error| format!("decode archive: {error:?}"))?;
    let scope = InterventionScope::new(1, true, false, false, &[])
        .map_err(|error| format!("experiment scope: {error:?}"))?;
    let experiment = PolicyExperiment::from_archive(&archive, &anchor, scope)
        .map_err(|error| format!("verify independent anchor: {error:?}"))?;
    let report = experiment.run(&[Intervention::Payload(payload)])
        .map_err(|error| format!("counterfactual: {error:?}"))?;
    let mut output = io::stdout().lock();
    writeln!(output, "profile=reference-policy-counterfactual/v1 permission=not_issued")
        .map_err(|error| error.to_string())?;
    writeln!(output, "attempt={} round={} policy_generation={} semantic_epoch={}",
        report.attempt(), report.round(), anchor.policy.generation(), anchor.snapshot_semantic_epoch)
        .map_err(|error| error.to_string())?;
    writeln!(output, "baseline={:?} counterfactual={:?} empirical={:?} next={:?}",
        report.baseline(), report.counterfactual(), report.empirical_status(), report.next_requirement())
        .map_err(|error| error.to_string())?;
    for change in report.changes() {
        writeln!(output, "node={} baseline={:?} counterfactual={:?}",
            change.node, change.baseline, change.counterfactual)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        // Success means the requested comparison completed, NOT approval.
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("policy_counterfactual: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn bounded_reader_accepts_exact_limit_and_refuses_one_more_byte() {
        assert_eq!(read_limit(Cursor::new([1, 2, 3]), 3).unwrap(), vec![1, 2, 3]);
        assert_eq!(read_limit(Cursor::new([1, 2, 3, 4]), 3).unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert_eq!(read_limit(Cursor::new([]), 0).unwrap(), Vec::<u8>::new());
        assert!(read_limit(Cursor::new([1]), 0).is_err());
    }

    #[test]
    fn read_failure_is_not_returned_as_a_complete_short_file() {
        struct Fails;
        impl Read for Fails {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected read refusal"))
            }
        }
        assert_eq!(read_limit(Fails, 20).unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    }
}
