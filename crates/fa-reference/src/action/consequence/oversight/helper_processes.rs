//! Host-owned executable helpers on private inherited Unix sockets.
//!
//! Launch is synchronous OS setup, not an executor or sandbox. Programs, paths,
//! arguments and the explicit environment come only from the supervising host.
//! Actual evidence travels through HelperPool, never through command arguments.

use super::CommitteeContract;
use super::helper_client::HelperClient;
use super::helper_workers::io::WorkerIoError;
use crate::full_input::InputProfileBinding;
use crate::Error;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};

pub const MAX_PROGRAM_ARGUMENTS: usize = 64;
pub const MAX_PROGRAM_ENVIRONMENT: usize = 64;
pub const MAX_PROGRAM_FIELD_BYTES: usize = 4_096;
pub const MAX_PROGRAM_TEXT_BYTES: usize = 32 * 1_024;
pub const MAX_LAUNCH_TEXT_BYTES: usize = 256 * 1_024;

/// Trusted executable configuration, not actor or helper input. No shell is
/// inserted and there is no PATH lookup. An explicitly configured interpreter
/// remains a host choice. Debug never prints arguments or environment values.
pub struct HelperProgram {
    executable: PathBuf,
    directory: PathBuf,
    arguments: Vec<OsString>,
    environment: BTreeMap<OsString, OsString>,
    text_bytes: usize,
}

impl fmt::Debug for HelperProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HelperProgram").field("argument_count", &self.arguments.len())
            .field("environment_count", &self.environment.len()).finish_non_exhaustive()
    }
}

impl HelperProgram {
    pub fn new(
        executable: PathBuf, directory: PathBuf, arguments: Vec<OsString>,
        environment: BTreeMap<OsString, OsString>,
    ) -> Result<Self, Error> {
        if !executable.is_absolute() || !directory.is_absolute() { return Err(Error::InvalidInput); }
        if arguments.len() > MAX_PROGRAM_ARGUMENTS || environment.len() > MAX_PROGRAM_ENVIRONMENT {
            return Err(Error::Limit);
        }
        let mut text_bytes = 0_usize;
        for text in [executable.as_os_str(), directory.as_os_str()].into_iter()
            .chain(arguments.iter().map(OsString::as_os_str))
            .chain(environment.iter().flat_map(|(key, value)| [key.as_os_str(), value.as_os_str()]))
        {
            let bytes = text.as_encoded_bytes();
            if bytes.contains(&0) { return Err(Error::InvalidInput); }
            if bytes.len() > MAX_PROGRAM_FIELD_BYTES { return Err(Error::Limit); }
            text_bytes = text_bytes.checked_add(bytes.len()).ok_or(Error::Limit)?;
            if text_bytes > MAX_PROGRAM_TEXT_BYTES { return Err(Error::Limit); }
        }
        if environment.keys().any(|key| key.is_empty() || key.as_encoded_bytes().contains(&b'=')) {
            return Err(Error::InvalidInput);
        }
        Ok(Self { executable, directory, arguments, environment, text_bytes })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessStage { InspectProgram, InspectDirectory, SocketPair, SocketSetup, Spawn, Reap, Terminate }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessFailure {
    Refused(Error),
    Io { stage: ProcessStage, kind: io::ErrorKind },
}

fn io_failure(stage: ProcessStage, error: io::Error) -> ProcessFailure {
    ProcessFailure::Io { stage, kind: error.kind() }
}

/// OS process outcome only. Success never stands in for an accepted reveal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessExit { pub success: bool, pub code: Option<i32>, pub signal: Option<i32> }
impl From<ExitStatus> for ProcessExit {
    fn from(status: ExitStatus) -> Self {
        Self { success: status.success(), code: status.code(), signal: status.signal() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessStatus {
    pub pid: u32,
    pub stop_requested: bool,
    pub termination_sent: bool,
    pub exit: Option<ProcessExit>,
    pub failure: Option<ProcessFailure>,
}

struct OwnedChild { child: Option<Child>, status: ProcessStatus }
impl OwnedChild {
    fn poll(&mut self) {
        let Some(child) = self.child.as_mut() else { return; };
        match child.try_wait() {
            Ok(Some(exit)) => {
                self.status.exit = Some(exit.into());
                self.status.failure = None;
                self.child = None;
                return;
            }
            Ok(None) => self.status.failure = None,
            Err(error) => {
                self.status.failure = Some(io_failure(ProcessStage::Reap, error));
                return;
            }
        }
        if self.status.stop_requested && !self.status.termination_sent {
            match child.kill() {
                Ok(()) => self.status.termination_sent = true,
                Err(error) => self.status.failure = Some(io_failure(ProcessStage::Terminate, error)),
            }
        }
    }
}

/// Retains every successfully spawned direct child until it has been reaped.
/// Polling is bounded to one try_wait and at most one kill per retained child.
/// No worker is restarted, and no exit status is converted into a verdict.
/// Descendants and inherited non-CLOEXEC descriptors are host responsibilities.
#[must_use = "retain the child owner, request shutdown, and poll until all children are reaped"]
pub struct HelperChildren { children: BTreeMap<String, OwnedChild> }

impl fmt::Debug for HelperChildren {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HelperChildren").field("statuses", &self.statuses()).finish()
    }
}

impl HelperChildren {
    fn empty() -> Self { Self { children: BTreeMap::new() } }
    pub fn statuses(&self) -> BTreeMap<String, ProcessStatus> {
        self.children.iter().map(|(member, child)| (member.clone(), child.status)).collect()
    }
    pub fn all_reaped(&self) -> bool { self.children.values().all(|child| child.child.is_none()) }
    pub fn reap(&mut self) -> BTreeMap<String, ProcessStatus> {
        for child in self.children.values_mut() { child.poll(); }
        self.statuses()
    }
    pub fn request_stop(&mut self, member: &str) -> Result<ProcessStatus, Error> {
        let child = self.children.get_mut(member).ok_or(Error::Missing)?;
        child.status.stop_requested = true;
        child.poll();
        Ok(child.status)
    }
    pub fn request_stop_all(&mut self) -> BTreeMap<String, ProcessStatus> {
        for child in self.children.values_mut() {
            child.status.stop_requested = true;
            child.poll();
        }
        self.statuses()
    }
}

impl Drop for HelperChildren {
    fn drop(&mut self) {
        // Never block a runtime thread in Drop or create a hidden reaper thread.
        // Normal hosts keep this owner and poll to completion before dropping.
        for child in self.children.values_mut() {
            child.status.stop_requested = true;
            child.poll();
            if child.child.is_some() { child.poll(); }
        }
        let remaining = self.children.values().filter(|child| child.child.is_some()).count();
        if remaining != 0 {
            eprintln!("franken_alignment: {remaining} direct helper children not reaped on drop; retain and poll HelperChildren during shutdown");
        }
    }
}

/// A partial OS launch is not atomic. Every started child is returned for
/// explicit cleanup, with termination requested; no input bytes have been sent.
#[derive(Debug)]
#[must_use = "a failed partial launch can still own children that must be reaped"]
pub struct HelperLaunchError {
    pub member: Option<String>,
    pub failure: ProcessFailure,
    pub children: HelperChildren,
}

fn rejected(member: Option<String>, failure: ProcessFailure, mut children: HelperChildren) -> HelperLaunchError {
    children.request_stop_all();
    HelperLaunchError { member, failure, children }
}

/// Launch the entire frozen roster. Metadata and aggregate bounds are checked
/// before the first spawn. Later OS failures return all previously started
/// children; no reduced-roster result or automatic retry is possible.
///
/// Each child receives a private full-duplex socket as stdin. stdout and stderr
/// both go to the host's PRIVATE stderr, never into the vote or actor protocol.
/// The host must isolate that diagnostic stream and all other ambient authority.
/// Only explicitly supplied environment entries survive env_clear. This is not
/// an executable-image attestation, descriptor sandbox, process tree supervisor,
/// wall-clock-bounded spawn, or a claim that a model actually used its input.
pub fn launch_helpers(
    contract: &CommitteeContract, programs: &BTreeMap<String, HelperProgram>,
) -> Result<(BTreeMap<String, UnixStream>, HelperChildren), HelperLaunchError> {
    let preflight = (|| -> Result<(), (Option<String>, ProcessFailure)> {
        if !contract.members().keys().eq(programs.keys()) {
            return Err((None, ProcessFailure::Refused(Error::Binding)));
        }
        let mut total = 0_usize;
        for (member, program) in programs {
            total = total.checked_add(program.text_bytes).and_then(|sum| sum.checked_add(member.len()))
                .ok_or((None, ProcessFailure::Refused(Error::Limit)))?;
            if total > MAX_LAUNCH_TEXT_BYTES { return Err((None, ProcessFailure::Refused(Error::Limit))); }
            let metadata = fs::metadata(&program.executable)
                .map_err(|error| (Some(member.clone()), io_failure(ProcessStage::InspectProgram, error)))?;
            if !metadata.is_file() { return Err((Some(member.clone()), ProcessFailure::Refused(Error::Binding))); }
            let metadata = fs::metadata(&program.directory)
                .map_err(|error| (Some(member.clone()), io_failure(ProcessStage::InspectDirectory, error)))?;
            if !metadata.is_dir() { return Err((Some(member.clone()), ProcessFailure::Refused(Error::Binding))); }
        }
        Ok(())
    })();
    if let Err((member, failure)) = preflight {
        return Err(rejected(member, failure, HelperChildren::empty()));
    }
    let mut children = HelperChildren::empty();
    let mut streams = BTreeMap::new();
    for (member, program) in programs {
        let spawned = (|| -> Result<(UnixStream, Child), ProcessFailure> {
            let (parent, worker) = UnixStream::pair().map_err(|error| io_failure(ProcessStage::SocketPair, error))?;
            parent.set_nonblocking(true).map_err(|error| io_failure(ProcessStage::SocketSetup, error))?;
            let input: OwnedFd = worker.into();
            let child = Command::new(&program.executable).args(&program.arguments)
                .current_dir(&program.directory).env_clear().envs(&program.environment)
                .stdin(Stdio::from(input)).stdout(Stdio::from(io::stderr())).stderr(Stdio::inherit())
                .spawn().map_err(|error| io_failure(ProcessStage::Spawn, error))?;
            Ok((parent, child))
        })();
        match spawned {
            Ok((stream, child)) => {
                let status = ProcessStatus { pid: child.id(), stop_requested: false,
                    termination_sent: false, exit: None, failure: None };
                children.children.insert(member.clone(), OwnedChild { child: Some(child), status });
                streams.insert(member.clone(), stream);
            }
            Err(failure) => return Err(rejected(Some(member.clone()), failure, children)),
        }
    }
    Ok((streams, children))
}

impl HelperClient<UnixStream> {
    /// Worker-side attachment to the launcher's inherited full-duplex stdin.
    /// A normal pipe/file stdin is rejected; no raw-fd or unsafe conversion is
    /// required. Cloning preserves the process's ownership of its standard fd.
    pub fn from_process_stdin(expected: InputProfileBinding) -> Result<Self, WorkerIoError> {
        let input = io::stdin();
        let descriptor = input.as_fd().try_clone_to_owned()
            .map_err(|error| WorkerIoError::Io(error.kind()))?;
        let stream = UnixStream::from(descriptor);
        stream.peer_addr().map_err(|error| WorkerIoError::Io(error.kind()))?;
        Self::from_unix(stream, expected)
    }
}
