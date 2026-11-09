//! Follow only the issued pages of one explicitly selected original campaign.
//! No reconnect, retry, new campaign, source acquisition or policy mutation.

use super::{
    Budget, Endpoint, Instant, OperatorSettings, Path, TowerRequest, Verb, Write, connect_endpoint,
    emit_response, request_frame,
};
use crate::config::read_regular;
use fa_reference::action::consequence::delivery::persistent::requests::actor::MAX_POLICY_REPLAY_PAGE_QUERIES;
use fa_reference::tower::policy_replay_client::PolicyReplayCapture;
use fa_reference::tower::{MAX_REQUEST_BYTES, ReadArguments, decode_request};

const USAGE: &str = "tower-policy-replay OPERATOR_PROFILE TOWER_REQUEST_JSON --max-pages N (canonical N is 1..1024; one explicitly paged campaign; no reconnect or retry)";

pub(super) fn command(args: &[String], output: &mut impl Write) -> Result<(), String> {
    let max_pages = selection(args)?;
    let started = Instant::now();
    let initial = decode_request(&read_regular(Path::new(&args[2]), MAX_REQUEST_BYTES)?)
        .map_err(|_| "invalid bounded policy replay request document".to_owned())?;
    let ReadArguments::PolicyReplayPaged { campaign, .. } = &initial.arguments else {
        return Err("Tower policy enumeration requires an explicit initial paged request; no connection was opened".into());
    };
    let campaign = *campaign;
    let settings = OperatorSettings::read(Path::new(&args[1]))?;
    let selected = settings.policy_replay.as_ref().filter(|selected| selected.page_queries != 0)
        .ok_or("Tower policy enumeration requires explicit policy_replay.page_queries; no connection was opened")?;
    let endpoint = Endpoint::operator(&settings);
    let page_limit = max_pages
        .min(selected.page_queries)
        .min(endpoint.exchanges as usize);
    let mut capture = PolicyReplayCapture::new(settings.scope, &initial, page_limit)
        .map_err(|error| format!("{error}; no connection was opened"))?;
    let mut budget = Budget::new(&endpoint, started);
    let mut socket = connect_endpoint(endpoint, &budget, || settings.check_directory())?;
    let mut request = initial.clone();
    for page in 0..page_limit {
        budget.write_frame(&mut socket, &request_frame(&request)?)?;
        let response = budget.read_frame(&mut socket)?;
        let progress = capture.accept(&request, &response).map_err(|error| {
            format!("{error}; no continuation, retry or new campaign was requested")
        })?;
        // Output succeeds before another cursor can enter the original grant.
        // Each emitted line is the original complete correlated response.
        emit_response(&response, output, &budget)?;
        if progress.complete {
            return Ok(());
        }
        let Some(cursor) = progress.next_cursor else {
            return Err("Tower policy replay enumeration is incomplete: native continuation unavailable or grant exhausted; partial pages retained; no retry or new campaign".into());
        };
        if page + 1 == page_limit {
            break;
        }
        request = TowerRequest::new(
            format!("tower-policy-replay-page-{}", page + 2),
            initial.address.clone(),
            Verb::PolicyReplay,
        );
        request.arguments = ReadArguments::PolicyReplayContinue { campaign, cursor };
        request.idempotency_key = initial.idempotency_key.clone();
    }
    Err("Tower policy replay enumeration is incomplete: page or original exchange allowance exhausted; partial pages retained; no reconnect, retry or new campaign".into())
}

fn selection(args: &[String]) -> Result<usize, String> {
    if args.len() != 5
        || args[0] != "tower-policy-replay"
        || args[3] != "--max-pages"
        || args[1..3]
            .iter()
            .any(|value| value.is_empty() || value.starts_with("--"))
    {
        return Err(USAGE.into());
    }
    let max_pages = args[4].parse::<usize>().map_err(|_| USAGE.to_owned())?;
    if args[4] != max_pages.to_string()
        || !(1..=MAX_POLICY_REPLAY_PAGE_QUERIES).contains(&max_pages)
    {
        return Err(USAGE.into());
    }
    Ok(max_pages)
}
