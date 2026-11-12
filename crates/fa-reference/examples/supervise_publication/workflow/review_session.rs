//! One bounded reviewer endpoint across original ordinary authority restrictions.
//! A retained helper result may survive NarrowAuthority; its old human key never
//! does. Reapproval needs a fresh authenticated connection, nonce and explicit
//! decision, under the SAME listener, candidate allowance and workflow deadline.

use super::{
    Admission, BoundSocket, Config, Deadline, Duration, ElapsedTick, FileHumanPermit,
    FileHumanReviewer, FileOversight, FileSupervisedDriver, Instant, LiveControl,
    PeerProfile, ReviewDecision, ReviewerPhase, ReviewerProgress, debug, io, nonce,
    pause, plus,
};
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::JournalError;

pub(super) struct Session {
    socket: BoundSocket,
    admission: Admission,
    request: u64,
    ordinary_continuation: bool,
}

impl Session {
    pub(super) fn new(
        config: &Config,
        request: u64,
        peers: Option<&PeerProfile>,
        ordinary_continuation: bool,
    ) -> Result<Self, String> {
        let path = peers.map_or_else(|| config.socket(request), |profile| profile.socket(request));
        let admission = Admission::new(peers)?;
        let socket = BoundSocket::bind(&path, peers)?;
        if peers.is_none() {
            eprintln!("Reviewer peer credentials are unchecked: legacy namespace-isolated transport");
        }
        eprintln!("Independent reviewer endpoint ready: {path:?}");
        Ok(Self {
            socket,
            admission,
            request,
            ordinary_continuation,
        })
    }

    /// Discover and explicitly consume only the native acknowledged narrowing
    /// chain for this original Ready job. There is no generic stale refresh and
    /// no reviewer, source, clock, helper, permit or journal work in this step.
    pub(super) fn continue_after_narrowing(
        &self,
        driver: &mut FileSupervisedDriver,
    ) -> Result<bool, String> {
        if !self.ordinary_continuation {
            return Ok(false);
        }
        let Some(operation) = driver
            .pending_narrowing_continuation(self.request)
            .map_err(debug)?
        else {
            return Ok(false);
        };
        driver
            .continue_after_narrowing(self.request, operation)
            .map_err(debug)?;
        Ok(true)
    }

    pub(super) fn review<F>(
        &mut self,
        driver: &mut FileSupervisedDriver,
        reviewer: &FileHumanReviewer,
        config: &mut Config,
        deadline: &Deadline,
        control: &mut LiveControl<'_>,
        time: &mut F,
    ) -> Result<Option<FileHumanPermit>, String>
    where
        F: FnMut() -> ElapsedTick,
    {
        'offer: loop {
            let stream = loop {
                deadline.check(time())?;
                if control.checkpoint(driver, reviewer, deadline, time)? {
                    return Ok(None);
                }
                self.continue_after_narrowing(driver)?;
                match self.socket.listener.accept() {
                    Ok((stream, _)) => match self.admission.admit(stream)? {
                        Some(stream) => break stream,
                        None => pause(config.timing.poll_ms),
                    },
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) =>
                    {
                        pause(config.timing.poll_ms)
                    }
                    Err(error) => return Err(debug(error)),
                }
            };
            // Authentication still precedes every source capture and new human
            // request. Rejected candidates cannot spend that work. An accepted
            // socket with no offer has no stale decision to reuse.
            if control.checkpoint(driver, reviewer, deadline, time)? {
                return Ok(None);
            }
            self.continue_after_narrowing(driver)?;
            let expires = plus(time(), config.profile.human.max_validity_ticks)?
                .min(deadline.logical);
            let policy_only = driver
                .supervisor()
                .host()
                .map_err(debug)?
                .policy_only_file_source_required();
            let key = if self.ordinary_continuation {
                let host = driver.supervisor().host().map_err(debug)?;
                unused_request_key(&host, self.request, config.profile.human.max_requests)?
            } else {
                // The original learned one-shot contract is unchanged.
                self.request
            };
            let request = if policy_only {
                driver.request_learned_human_approval_from_policy_file(
                    &mut config.source,
                    key,
                    expires,
                    &mut *time,
                )
            } else {
                driver.request_human_approval_from_file(
                    &mut config.source,
                    key,
                    expires,
                    &mut *time,
                )
            }
            .result
            .map_err(debug)?;
            let mut connection = {
                let host = driver.supervisor().host().map_err(debug)?;
                stream.into_connection(&host, reviewer, request, nonce()?)?
            };
            let application = loop {
                deadline.check(time())?;
                if control.checkpoint(driver, reviewer, deadline, time)? {
                    return Ok(None);
                }
                if self.continue_after_narrowing(driver)? {
                    // The native restriction withdrew this pending/approved
                    // request. A decision buffered on this old connection must
                    // never be applied to the new predecessor.
                    drop(connection);
                    continue 'offer;
                }
                let progress = {
                    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
                    connection
                        .step(&mut host, reviewer, &mut *time)
                        .map_err(debug)?
                };
                match progress {
                    ReviewerProgress::Applied(application) => break application,
                    ReviewerProgress::Complete => {
                        return Err("reviewer completed without an application".into());
                    }
                    ReviewerProgress::Progress | ReviewerProgress::Blocked => {
                        pause(config.timing.poll_ms)
                    }
                }
            };
            if application.receipt.decision != ReviewDecision::Approve
                && application.approval.is_some()
            {
                return Err("unexpected key for a restrictive reviewer decision".into());
            }

            // An already applied decision retains its historical receipt. Drain
            // that SAME receipt under the original bounded cleanup grace; a later
            // restriction cannot turn receipt loss into an unapplied decision.
            // If approval was withdrawn meanwhile, require a different request
            // and an independently initiated connection after the drain.
            let mut narrowed = false;
            let drain_started = Instant::now();
            while connection.phase() != ReviewerPhase::Complete
                && drain_started.elapsed() < Duration::from_millis(config.timing.cleanup_ms)
            {
                if control.checkpoint(driver, reviewer, deadline, time)? {
                    return Ok(None);
                }
                narrowed |= self.continue_after_narrowing(driver)?;
                let result = {
                    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
                    connection.step(&mut host, reviewer, &mut *time)
                };
                match result {
                    Ok(ReviewerProgress::Complete) => break,
                    Ok(ReviewerProgress::Applied(_)) => {
                        return Err("duplicate reviewer application".into());
                    }
                    Ok(_) => pause(config.timing.poll_ms),
                    Err(error) => {
                        eprintln!("Committed review receipt was not delivered: {error:?}");
                        break;
                    }
                }
            }
            if narrowed {
                // A rejection/revocation is still terminal for this workflow;
                // narrowing cannot turn it into an invitation to approve again.
                if application.approval.is_none() {
                    return Ok(None);
                }
                drop(application.approval);
                drop(connection);
                continue 'offer;
            }
            return Ok(application.approval);
        }
    }
}

/// Human request identities and actor request identities share no semantics.
/// Preserve the preferred original ID when free, then inspect at most the
/// ORIGINAL finite request capacity plus one. This also avoids a renewal key
/// colliding with a later scheduled actor request. Opaque-ID wrap is explicit;
/// control sequences, epochs and counters never wrap or reset here.
fn unused_request_key(
    host: &FileOversight,
    preferred: u64,
    maximum: usize,
) -> Result<u64, String> {
    if preferred == 0 || maximum == 0 {
        return Err("human request selection requires nonzero identity and capacity".into());
    }
    let mut selected = preferred;
    for _ in 0..=maximum {
        match host.human_request(selected) {
            Err(JournalError::Contract(Error::Missing)) => return Ok(selected),
            Ok(_) => {}
            Err(error) => return Err(debug(error)),
        }
        selected = selected.checked_add(1).unwrap_or(1);
    }
    Err("original human request identity capacity is exhausted".into())
}
