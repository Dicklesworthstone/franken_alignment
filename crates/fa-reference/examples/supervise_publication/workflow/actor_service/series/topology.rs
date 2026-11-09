//! Schedule-wide endpoint custody and the original selected topology source.
use super::super::super::continuation::checkpoint::AcknowledgedOutput;
use super::*;
use std::{fs, path::PathBuf};

/// Every future listener is admitted before the first source read or canonical
/// image. Canonical parents prevent an output alias from replacing a retired
/// later reviewer, Stop or operator endpoint.
pub(super) fn check_destinations(
    config: &Config,
    requests: &[u64],
    actor: &Profile,
    reviewer: &PeerProfile,
    operator: Option<&operator::Settings>,
    output: Option<&Path>,
) -> Result<(), String> {
    let mut sockets = vec![canonical_destination(&actor.socket)?];
    for &request in requests {
        sockets.push(canonical_destination(&reviewer.socket(request))?);
        sockets.push(canonical_destination(
            &super::super::super::control::socket_path(reviewer, request),
        )?);
    }
    if let Some(settings) = operator {
        sockets.push(canonical_destination(&settings.socket)?);
    }
    if sockets
        .iter()
        .enumerate()
        .any(|(index, path)| sockets[..index].contains(path))
    {
        return Err(
            "actor, every scheduled reviewer and Stop, and operator sockets must be distinct"
                .into(),
        );
    }
    if let Some(path) = output {
        let archive = AcknowledgedOutput::admit(config, path)?;
        if sockets
            .iter()
            .any(|socket| socket.as_path() == archive.path())
        {
            return Err(
                "topology checkpoint and all scheduled service sockets must be distinct".into(),
            );
        }
    }
    Ok(())
}

fn canonical_destination(path: &Path) -> Result<PathBuf, String> {
    let parent =
        fs::canonicalize(path.parent().ok_or("service socket needs a parent")?).map_err(debug)?;
    Ok(parent.join(path.file_name().ok_or("service socket needs a filename")?))
}

/// Installing the original noncloneable source once conserves its limits and
/// spent work across every handoff. The driver's ordinary first-publication
/// path performs each later final check; scheduling never certifies a new cut.
pub(super) fn install<F>(
    driver: &mut FileSupervisedDriver,
    source: Option<FileDriverTopology>,
    deadline: &Deadline,
    time: &mut F,
) -> Result<(), String>
where
    F: FnMut() -> ElapsedTick,
{
    let Some(source) = source else {
        return Ok(());
    };
    let revision = driver.supervisor().host().map_err(debug)?.revision();
    driver.require_topology(revision, source).map_err(debug)?;
    deadline.check(time())?;
    let revision = driver.supervisor().host().map_err(debug)?.revision();
    let refreshed = driver.refresh_topology(revision).map_err(debug)?;
    if refreshed.refusal.is_some()
        || !matches!(
            refreshed.checked,
            Some(Ok(
                fa_reference::action::consequence::mediation::CutCheck::Verified(_)
            ))
        )
    {
        return Err("initial live-service topology was not certified by the native checker".into());
    }
    let now = time();
    deadline.check(now)?;
    let mut host = driver.supervisor_mut().host_mut().map_err(debug)?;
    let revision = host.revision();
    host.observe_time(revision, now).map_err(debug)?;
    Ok(())
}
