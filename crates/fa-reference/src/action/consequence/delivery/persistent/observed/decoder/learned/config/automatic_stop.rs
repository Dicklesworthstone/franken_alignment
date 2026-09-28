//! Freeze automatic learned containment into the exact durable bootstrap recipe.
//! The ORIGINAL learned stop policy, trigger detector and stop reducer do the work.
use super::{FileLearnedConfig, HostedStopPolicy, Writer, DOMAIN, MAX_CONFIG_BYTES};
use crate::action::consequence::delivery::persistent::{JournalError, observed::FileOversight};
use crate::action::consequence::oversight::learned_host::LearnedHostStopIncident;
use crate::Error;

impl FileLearnedConfig {
    /// Select the original terminal-stop profile before installation or inference.
    /// All nonquiet monitors and admitted numerical failures use its original
    /// cause classification. No caller verdict or live retuning path is added.
    ///
    /// Keep this exact recipe independently: recovery rejects missing or changed
    /// policy identities before numerical replay or cleanup. Existing recipes
    /// without this wrapper retain their exact bytes and manual-containment mode.
    /// Text, stream and mandatory-sidecar choices are preserved. Wrapper order is
    /// part of the byte-exact recipe, just as its other configuration choices are.
    pub fn with_automatic_stop(mut self, policy: HostedStopPolicy) -> Result<Self, Error> {
        if self.automatic_stop.is_some() { return Err(Error::Duplicate); }
        let mut writer = Writer::new(MAX_CONFIG_BYTES);
        writer.raw(DOMAIN)?;
        // Legacy numeric recipes require nonzero monitor generation. Existing
        // wrappers have different domains; no old valid recipe has this prefix.
        writer.u64(0)?;
        writer.raw(b"FALSTOP\x01")?;
        writer.blob(&self.bytes)?;
        for value in [policy.id(), policy.generation(), policy.operation()] { writer.u64(value)?; }
        self.bytes = writer.finish().into();
        self.automatic_stop = Some(policy);
        Ok(self)
    }

    pub fn automatic_stop_policy(&self) -> Option<HostedStopPolicy> { self.automatic_stop }
}

impl FileOversight {
    /// Configured mode only; None means manual containment, not safety evidence.
    pub fn learned_host_stop_policy(&self) -> Option<HostedStopPolicy> {
        self.machine.broker.learned_host_stop_policy()
    }

    /// The first ORIGINAL trigger at this acknowledged cut. A returned local
    /// stop is not proof of endpoint settlement. Use original progress_stop,
    /// reconciliation and sealing for outstanding dispatches. Reopening retains
    /// the incident as history, never a fresh source observation or an effect key.
    /// An unavailable owner must not expose its older RAM incident after a failed
    /// canonical replacement. This method does no I/O or numerical work.
    ///
    /// ```compile_fail,E0599
    /// use fa_reference::action::consequence::delivery::persistent::observed::FileOversight;
    /// fn relax(host: &mut FileOversight) { host.disable_learned_host_stop(); }
    /// ```
    pub fn learned_host_stop_incident(&self) -> Result<Option<LearnedHostStopIncident>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.learned_host_stop_incident().cloned())
    }
}
