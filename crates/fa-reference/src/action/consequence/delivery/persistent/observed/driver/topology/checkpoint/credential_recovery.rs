//! Pre-replay source custody for one exact credential-maintenance operation.
//! Semantic validation still belongs to the original locked learned recovery.
use super::{TopologyCheckpoint, check_work};
use crate::Error;
use crate::action::consequence::delivery::persistent::observed::{
    BaseEvent, Event, FileOversightProfile,
    guarded::mediated::credential::FileCredentialMaintenanceRequest,
};
use std::path::Path;

impl TopologyCheckpoint {
    pub(in super::super::super::super) fn credential_successor_available(
        &self,
        profile: &FileOversightProfile,
        identity: &Path,
        events: &[Event],
        request: FileCredentialMaintenanceRequest,
    ) -> Result<bool, Error> {
        if request.operation() == 0 {
            return Err(Error::InvalidInput);
        }
        check_work(&self.expected, self.work, self.last_operation)?;
        self.history.check(profile, identity, events)?;
        let after = usize::try_from(self.history.revision()).map_err(|_| Error::Limit)?;
        let (graph, mut available) = super::terminal::topology_at(&events[..after])?;
        if graph != self.expected.graph() || available != self.available {
            return Err(Error::Binding);
        }
        // A maintenance retry may explain a lost successor export, not borrow
        // an unrelated operation's checkpoint or erase new source expenditure.
        // Duplicate matching events still fail the original semantic replay.
        for event in &events[after..] {
            match (event, request) {
                (Event::Core(BaseEvent::Fence), _) => available = false,
                (
                    Event::CredentialRotate(actual),
                    FileCredentialMaintenanceRequest::Rotate(expected),
                ) if *actual == expected => {}
                (
                    Event::CredentialRevoke(actual),
                    FileCredentialMaintenanceRequest::Revoke(expected),
                ) if *actual == expected => {}
                _ => return Err(Error::Stale),
            }
        }
        Ok(available)
    }
}
