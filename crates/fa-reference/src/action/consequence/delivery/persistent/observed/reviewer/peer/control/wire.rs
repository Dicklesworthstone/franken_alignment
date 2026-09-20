//! Fixed-size stop-only interchange. A network report is not a native receipt.
use super::{CapacityDrain, CapacityStop, Error, JournalError, ReviewerExpectation};
use crate::action::Purpose;

pub const OFFER_BYTES: usize = 104;
pub const RECEIPT_BYTES: usize = OFFER_BYTES + 25;
const OFFER: &[u8; 8] = b"FASTOFF1";
const REQUEST: &[u8; 8] = b"FASTREQ1";
const RECEIPT: &[u8; 8] = b"FASTRCP1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopBinding {
    pub expected: ReviewerExpectation,
    pub operation: u64,
    pub session: [u8; 32],
}
impl StopBinding {
    pub fn check(self) -> Result<(), Error> {
        self.expected.scope.validate()?;
        if self.expected.scope.purpose != Purpose::Effect { return Err(Error::Binding); }
        if self.expected.reviewer == 0 || self.expected.clock_domain == 0 || self.operation == 0
            || self.session == [0; 32] { return Err(Error::InvalidInput); }
        Ok(())
    }
    pub(super) fn offer(self) -> Result<[u8; OFFER_BYTES], Error> {
        self.check()?;
        let scope = self.expected.scope;
        let mut bytes = [0; OFFER_BYTES];
        bytes[..8].copy_from_slice(OFFER);
        for (index, value) in [scope.tenant, scope.principal, scope.run, scope.branch, scope.authority,
            self.expected.clock_domain, self.expected.reviewer, self.operation].into_iter().enumerate() {
            bytes[8 + index * 8..16 + index * 8].copy_from_slice(&value.to_be_bytes());
        }
        bytes[72..].copy_from_slice(&self.session);
        Ok(bytes)
    }
    pub(super) fn request(self) -> Result<[u8; OFFER_BYTES], Error> {
        let mut bytes = self.offer()?;
        bytes[..8].copy_from_slice(REQUEST);
        Ok(bytes)
    }
    pub(super) fn decode_offer(bytes: &[u8; OFFER_BYTES], expected: ReviewerExpectation,
        operation: u64) -> Result<Self, Error>
    {
        let mut session = [0; 32]; session.copy_from_slice(&bytes[72..]);
        let binding = Self { expected, operation, session };
        if bytes != &binding.offer()? { return Err(Error::Binding); }
        Ok(binding)
    }
}

/// Unconfirmed means no native stop receipt was returned, NOT proof that the
/// domain remained live. Drain refusal never erases an acknowledged local stop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopStatus { Unconfirmed, StoppedDrainRefused, StoppedPending, StoppedDrained }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StopControlReceipt {
    pub binding: StopBinding,
    pub status: StopStatus,
    pub control_sequence: u64,
    pub revocation_floor: u64,
    pub dispatcher_epoch: u64,
}
impl StopControlReceipt {
    pub fn acknowledged(self) -> bool { self.status != StopStatus::Unconfirmed }
    pub fn drained(self) -> bool { self.status == StopStatus::StoppedDrained }
    pub(super) fn project(binding: StopBinding, result: &Result<CapacityStop, JournalError>) -> Self {
        match result {
            Err(_) => Self { binding, status: StopStatus::Unconfirmed, control_sequence: 0,
                revocation_floor: 0, dispatcher_epoch: 0 },
            Ok(stopped) => {
                let status = match &stopped.drain {
                    Err(_) => StopStatus::StoppedDrainRefused,
                    Ok(CapacityDrain::AlreadyDrained(progress)) => {
                        if progress.drained() { StopStatus::StoppedDrained } else { StopStatus::StoppedPending }
                    }
                    Ok(CapacityDrain::Advanced(sweep)) => {
                        if sweep.progress.drained() { StopStatus::StoppedDrained } else { StopStatus::StoppedPending }
                    }
                };
                Self { binding, status, control_sequence: stopped.stop.control_sequence(),
                    revocation_floor: stopped.stop.revocation_floor(), dispatcher_epoch: stopped.stop.dispatcher_epoch() }
            }
        }
    }
    pub(super) fn encode(self) -> Result<[u8; RECEIPT_BYTES], Error> {
        let mut bytes = [0; RECEIPT_BYTES];
        bytes[..OFFER_BYTES].copy_from_slice(&self.binding.offer()?);
        bytes[..8].copy_from_slice(RECEIPT);
        bytes[104] = match self.status { StopStatus::Unconfirmed => 0, StopStatus::StoppedDrainRefused => 1,
            StopStatus::StoppedPending => 2, StopStatus::StoppedDrained => 3 };
        for (index, value) in [self.control_sequence, self.revocation_floor, self.dispatcher_epoch].into_iter().enumerate() {
            bytes[105 + index * 8..113 + index * 8].copy_from_slice(&value.to_be_bytes());
        }
        Ok(bytes)
    }
    pub(super) fn decode(bytes: &[u8; RECEIPT_BYTES], binding: StopBinding) -> Result<Self, Error> {
        let mut header = binding.offer()?; header[..8].copy_from_slice(RECEIPT);
        if bytes[..OFFER_BYTES] != header { return Err(Error::Binding); }
        let status = match bytes[104] { 0 => StopStatus::Unconfirmed, 1 => StopStatus::StoppedDrainRefused,
            2 => StopStatus::StoppedPending, 3 => StopStatus::StoppedDrained, _ => return Err(Error::InvalidInput) };
        let mut counters = [0; 3];
        for (index, value) in counters.iter_mut().enumerate() {
            let mut raw = [0; 8]; raw.copy_from_slice(&bytes[105 + index * 8..113 + index * 8]);
            *value = u64::from_be_bytes(raw);
        }
        if status == StopStatus::Unconfirmed && counters != [0; 3] { return Err(Error::Binding); }
        Ok(Self { binding, status, control_sequence: counters[0], revocation_floor: counters[1], dispatcher_epoch: counters[2] })
    }
}
