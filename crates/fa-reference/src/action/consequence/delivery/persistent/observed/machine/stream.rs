//! Select the existing stream endpoint only at the journal's first event.
use super::{Machine, PublicationEndpoint, Transition};
use super::super::stream::FileStreamSnapshot;
use crate::action::consequence::delivery::stream::StreamProfile;
use crate::Error;

impl Machine {
    pub(super) fn bootstrap_stream(&mut self, stream: StreamProfile) -> Result<Transition, Error> {
        let profile = self.bootstrap.as_ref().ok_or(Error::WrongState)?;
        let delivery = &profile.delivery;
        if !delivery.initial_payload.is_empty() { return Err(Error::Binding); }
        let endpoint = PublicationEndpoint::new_stream(delivery.target, stream,
            delivery.retention_ticks, delivery.max_deliveries)?;
        // This is the SAME native broker bootstrap, before any other accepted
        // event. Never copy grants, reviews or a live endpoint into another owner.
        let mut next = Self::with_endpoint(profile, endpoint)?;
        next.enable_publication_guard()?;
        *self = next;
        Ok(Transition::Unit)
    }

    pub(in super::super) fn stream_snapshot(&self, revision: usize) -> Result<FileStreamSnapshot, Error> {
        let (confirmed_target, confirmed) = self.broker.stream_state().ok_or(Error::WrongState)?;
        let published = self.endpoint.stream_view().ok_or(Error::Binding)?;
        Ok(FileStreamSnapshot {
            publication: self.snapshot(revision), confirmed_target,
            confirmed: confirmed.clone(), published: published.clone(),
            pending: self.broker.stream_pending(),
        })
    }
}
