//! Fixed automatic containment over the original numerical owner and stop path.
use super::{DecoderEvent, Event, FileOversight, JournalError};
use crate::action::consequence::oversight::decoder_host::{HostedStopIncident, HostedStopPolicy};

impl FileOversight {
    /// Irreversible opt-in before any numerical token, proposal or request.
    /// The native monitor/decoder supplies the cause; callers cannot assert an
    /// alarm or exempt one category. Existing manual-reset mode remains separate.
    pub fn enable_decoder_stop(&mut self, revision: u64, policy: HostedStopPolicy)
        -> Result<(), JournalError>
    {
        self.transact(revision, Event::Decoder(DecoderEvent::StopPolicy(policy)))?;
        Ok(())
    }

    /// Frozen configuration at the last acknowledged cut, not a watchdog lease.
    pub fn decoder_stop_policy(&self) -> Result<Option<HostedStopPolicy>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.hosted_stop_policy())
    }

    /// The original first trigger and stop-attempt result. A retained local stop
    /// receipt cannot prove external nonexecution; use the original stop/drain.
    /// An absent incident means none at this acknowledged cut, not global health.
    pub fn decoder_stop_incident(&self) -> Result<Option<HostedStopIncident>, JournalError> {
        if self.fault.is_some() { return Err(JournalError::Unavailable); }
        Ok(self.machine.broker.hosted_stop_incident().cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::consequence::delivery::persistent::codec::shared::{Reader, Writer};

    #[test]
    fn stop_policy_has_an_independent_exact_vector_and_rejects_truncation_or_zero_identity() {
        let policy = HostedStopPolicy::new(7, 11, 13).unwrap();
        let event = DecoderEvent::StopPolicy(policy);
        let mut w = Writer::new(64); super::super::write(&mut w, &event).unwrap();
        let actual = w.finish(); let mut expected = vec![5];
        for value in [7_u64, 11, 13] { expected.extend_from_slice(&value.to_be_bytes()); }
        assert_eq!(actual, expected);
        let mut r = Reader::new(&actual);
        let decoded = super::super::read(&mut r).unwrap(); r.end().unwrap();
        assert!(matches!(decoded, DecoderEvent::StopPolicy(value) if value == policy));
        for end in 0..actual.len() {
            assert!(super::super::read(&mut Reader::new(&actual[..end])).is_err());
        }
        for offset in [1, 9, 17] {
            let mut invalid = actual.clone(); invalid[offset..offset + 8].fill(0);
            assert!(super::super::read(&mut Reader::new(&invalid)).is_err());
        }
    }
}
