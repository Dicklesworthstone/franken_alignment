//! Recovered schedules retain the original Fence, source and request history.
//! Admission reserves future listeners before replay; recorded rows never issue
//! a new grant, including in a schedule that also contains genuinely new work.
use super::*;
use crate::workflow::actor_service::FileOversight;
use fa_reference::Error;
use fa_reference::action::consequence::delivery::persistent::JournalError;

impl Schedule {
    /// CLI profile names are independent inputs too. These are compared before
    /// recovery and are never reopened when the schedule changes request keys.
    pub(crate) fn check_recovery_inputs(&self, paths: &[&Path]) -> Result<(), String> {
        let custody = paths
            .iter()
            .map(|path| resolve_path(path))
            .collect::<Result<BTreeSet<_>, _>>()?;
        for path in self.inputs.iter().map(PathBuf::as_path).chain(
            self.entries
                .iter()
                .filter_map(|(_, row)| row.as_ref().map(Settings::socket_path)),
        ) {
            if custody.contains(&resolve_path(path)?) {
                return Err("scheduled precheck inputs and listeners must be distinct from original recovery custody".into());
            }
        }
        Ok(())
    }

    /// Reserve the whole schedule, including later disclosure and profile
    /// paths, against the original store, history/source archives and every
    /// service endpoint. Canonical ancestors cover not-yet-created outputs.
    pub(crate) fn check_recovery_host(
        &self,
        config: &Config,
        actor: &Profile,
        reviewer: &PeerProfile,
        requests: &[u64],
        operator: Option<&operator::Settings>,
        governor: Option<&governor::Settings>,
        paths: &[&Path],
    ) -> Result<(), String> {
        self.check_host(config, actor, reviewer, requests, operator, None)?;
        let mut custody = paths.to_vec();
        custody.push(config.source.path());
        self.check_recovery_inputs(&custody)?;
        let store = resolve_path(&config.store)?;
        let mut endpoints = vec![actor.socket.clone()];
        for &request in requests {
            endpoints.push(reviewer.socket(request));
            endpoints.push(crate::workflow::control::socket_path(reviewer, request));
        }
        endpoints.extend(operator.map(|settings| settings.socket.clone()));
        endpoints.extend(governor.map(|settings| settings.socket.clone()));
        let mut reserved = BTreeSet::new();
        for path in endpoints {
            let path = resolve_path(&path)?;
            if path.starts_with(&store) || !reserved.insert(path) {
                return Err(
                    "recovered service endpoints must be distinct and outside the authority store"
                        .into(),
                );
            }
        }
        let endpoints = reserved.clone();
        for path in custody {
            let path = resolve_path(path)?;
            if endpoints.contains(&path) {
                return Err("recovered service endpoints must be distinct from original history and source custody".into());
            }
            reserved.insert(path);
        }
        // Profile paths can legitimately be shared by schedule rows, but no
        // profile may become a listener or an authority-store entry.
        let inputs = self
            .inputs
            .iter()
            .map(|path| resolve_path(path))
            .collect::<Result<BTreeSet<_>, _>>()?;
        for path in inputs {
            if path.starts_with(&store) || !reserved.insert(path) {
                return Err("scheduled precheck profiles must be distinct from recovered endpoints, custody and the authority store".into());
            }
        }
        for (_, settings) in &self.entries {
            let Some(settings) = settings else { continue };
            let path = resolve_path(settings.socket_path())?;
            if path.starts_with(&store) || !reserved.insert(path) {
                return Err("scheduled precheck listeners must be distinct from all recovered endpoints, profiles, custody and the authority store".into());
            }
        }
        Ok(())
    }

    /// Run only after original guarded replay and reconciliation, before any
    /// next source is opened or rebound. A non-null historical row is an error,
    /// never an ignored disclosure selection or a request to replay the effect.
    pub(crate) fn check_reconciled(&self, owner: &FileOversight) -> Result<(), String> {
        for (request, settings) in &self.entries {
            if settings.is_none() {
                continue;
            }
            match owner.request_status(*request) {
                Ok(_) => return Err("recorded requests are observation-only and cannot acquire a new precheck disclosure".into()),
                Err(JournalError::Contract(Error::Missing)) => {}
                Err(error) => return Err(debug(error)),
            }
        }
        Ok(())
    }
}
