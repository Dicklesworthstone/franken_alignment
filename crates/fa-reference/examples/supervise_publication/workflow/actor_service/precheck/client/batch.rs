//! Bounded alternative screening through the ORIGINAL one-shot native client.
//! Every document is validated before the first connection. No member retries,
//! renews a grant, selects another endpoint or becomes an ordinary submission.
use super::{MAX_IO_CALLS, MAX_PRECHECK_FRAME_BYTES, MAX_PRECHECK_RESPONSE_BYTES,
    PrecheckClientPhase, PrecheckPeerClient, PrecheckRequest, PrecheckWireResponse,
    Profile, Settings, Work, check_time, debug, decode_precheck_request,
    drive_client_shared, emit_response, read_regular};
use fa_reference::action::ElapsedTick;
use fa_reference::action::consequence::oversight::actor::Knowledge;
use std::collections::BTreeSet;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

const USAGE: &str = "actor-precheck --batch ACTOR_PROFILE PRECHECK_PROFILE PRECHECK_JSON [PRECHECK_JSON ...]";
const MAX_DOCUMENTS: usize = 16;
// Also bound aggregate retained input, not just the number of documents. Each
// individual document still obeys the original native frame and payload limits.
const MAX_INPUT_BYTES: usize = 4 * MAX_PRECHECK_FRAME_BYTES;

pub(super) fn command(args: &[String], out: &mut impl Write) -> Result<(), String> {
    let documents = arguments(args)?;
    let started = Instant::now();
    let actor = Profile::read(Path::new(&args[2]))?;
    check_time(started, Duration::from_millis(actor.runtime_ms))?;
    let settings = Settings::read(Path::new(&args[3]))?;
    let documents: Vec<_> = documents.iter().map(Path::new).collect();
    query(&actor, &settings, &documents, started, out)
}

fn arguments(args: &[String]) -> Result<&[String], String> {
    if !(5..=MAX_DOCUMENTS + 4).contains(&args.len())
        || args[0] != "actor-precheck" || args[1] != "--batch"
        || args[2..].iter().any(|arg| arg.is_empty() || arg.starts_with("--"))
    { return Err(USAGE.into()); }
    Ok(&args[4..])
}

fn check_count(count: usize, settings: &Settings) -> Result<(), String> {
    let connections = u64::try_from(count).map_err(debug)?;
    if count == 0 || count > MAX_DOCUMENTS || count > settings.query_limit
        || count > settings.exchange_limit || connections > settings.connection_limit
        || connections > settings.candidate_limit
    {
        return Err("precheck batch exceeds document or configured service allowances; no query sent".into());
    }
    Ok(())
}

fn validate_request(request: &PrecheckRequest, settings: &Settings,
    queries: &mut BTreeSet<u64>,
) -> Result<(), String> {
    if request.proposal.expected_policy_epoch != settings.expected_policy_epoch {
        return Err("precheck batch differs from independently configured policy epoch; no query sent".into());
    }
    if !queries.insert(request.query) {
        return Err("precheck batch query labels must be distinct; no query sent".into());
    }
    Ok(())
}

fn prepare(settings: &Settings, documents: &[&Path], started: Instant, wall: Duration)
    -> Result<Vec<PrecheckRequest>, String>
{
    check_count(documents.len(), settings)?;
    let mut remaining = MAX_INPUT_BYTES;
    let mut queries = BTreeSet::new();
    let mut requests = Vec::with_capacity(documents.len());
    for document in documents {
        check_time(started, wall)?;
        let bytes = read_regular(document, remaining.min(MAX_PRECHECK_FRAME_BYTES))?;
        check_time(started, wall)?;
        remaining = remaining.checked_sub(bytes.len())
            .ok_or("precheck batch input allowance exceeded; no query sent")?;
        let request = decode_precheck_request(&bytes).map_err(debug)?;
        validate_request(&request, settings, &mut queries)?;
        requests.push(request);
    }
    check_time(started, wall)?;
    Ok(requests)
}

fn work(count: usize) -> Result<Work, String> {
    if count == 0 || count > MAX_DOCUMENTS { return Err(USAGE.into()); }
    Ok(Work {
        read_bytes: count.checked_mul(MAX_PRECHECK_RESPONSE_BYTES + 2)
            .ok_or("precheck batch read allowance overflow")?,
        write_bytes: count.checked_mul(MAX_PRECHECK_FRAME_BYTES + 1)
            .ok_or("precheck batch write allowance overflow")?,
        // One allowance for the entire command, NOT 4096 for each connection.
        io_calls: MAX_IO_CALLS,
    })
}

// A restarted/reprovisioned service must not silently mix visibly different cuts
// in one result set. This checks disclosed provenance, not executable identity or
// an authenticated instance nonce; it cannot detect reuse of identical labels.
fn record_basis(response: &PrecheckWireResponse, observed_at: &mut Option<ElapsedTick>)
    -> Result<bool, String>
{
    let Ok(Knowledge::Known { basis, .. }) = &response.result else { return Ok(false); };
    if observed_at.is_some_and(|previous| previous != basis.observed_at) {
        return Err("precheck batch observation changed; no further query or retry was sent".into());
    }
    *observed_at = Some(basis.observed_at);
    Ok(true)
}

fn query(actor: &Profile, settings: &Settings, documents: &[&Path], started: Instant,
    out: &mut impl Write,
) -> Result<(), String> {
    let wall = Duration::from_millis(actor.runtime_ms.min(settings.runtime_ms));
    check_time(started, wall)?;
    check_count(documents.len(), settings)?;
    settings.check_actor(actor)?;
    let requests = prepare(settings, documents, started, wall)?;
    let mut work = work(requests.len())?;
    let mut observed_at = None;
    let mut any_pass = false;
    for request in requests {
        check_time(started, wall)?;
        // Refuse exhaustion before opening the next socket. Each member has one
        // intentional fresh connection, never a retry of a previous query.
        work.budget(PrecheckClientPhase::Writing)?;
        let socket = UnixStream::connect(&settings.socket).map_err(debug)?;
        check_time(started, wall)?;
        let client = PrecheckPeerClient::connect(socket, actor.supervisor, request,
            settings.expected_policy_epoch).map_err(debug)?;
        let response = drive_client_shared(client, actor.poll_ms, wall, &mut work,
            || started.elapsed())?;
        check_time(started, wall)?;
        let known = record_basis(&response, &mut observed_at)?;
        let passed = emit_response(&response, settings, out)?;
        check_time(started, wall)?;
        if !known {
            return Err("precheck batch incomplete; see original refusal; no further query or retry was sent".into());
        }
        any_pass |= passed;
    }
    if any_pass { Ok(()) } else {
        Err("precheck batch found no passing candidate; no submission was sent".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fa_reference::action::ResolvedTarget;
    use fa_reference::action::consequence::delivery::persistent::requests::actor::{
        FilePrecheckBasis, FilePrecheckOutcome,
    };
    use fa_reference::action::consequence::oversight::actor::{ActorProposal, UnknownReason};
    use fa_reference::action::consequence::oversight::actor_transport::DriveProgress;

    fn settings() -> Settings {
        Settings::decode(br#"{"schema":"fa.actor-precheck-service/1","policy_disclosure":"exact_predicates","socket":"/tmp/fa-batch.sock","expected_policy_epoch":0,"disclosure_id":81,"query_limit":16,"exchange_limit":16,"candidate_limit":16,"connection_limit":16,"runtime_ms":15000}"#).unwrap()
    }
    fn request(query: u64, epoch: u64) -> PrecheckRequest {
        PrecheckRequest { query, proposal: ActorProposal {
            target: ResolvedTarget { adapter: 1, object: 1, contract_version: 1,
                expected_version: 1, generation: 1 },
            payload: b"ok".to_vec(), units: 2, deadline: ElapsedTick(100_000),
            expected_policy_epoch: epoch,
        } }
    }
    fn known(query: u64, tick: u64, value: FilePrecheckOutcome) -> PrecheckWireResponse {
        PrecheckWireResponse { query: Some(query), result: Ok(Knowledge::Known { value,
            basis: FilePrecheckBasis { disclosure: 81, query, policy_epoch: 0,
                observed_at: ElapsedTick(tick) },
        }) }
    }

    #[test]
    fn closed_batch_grammar_rejects_supervisor_options_and_unbounded_inputs() {
        for args in [
            vec![], vec!["actor-precheck", "--batch", "a", "p"],
            vec!["actor-submit", "--batch", "a", "p", "q"],
            vec!["actor-precheck", "--batch", "--durable", "p", "q"],
            vec!["actor-precheck", "--batch", "a", "p", "q", "--andon"],
            vec!["actor-precheck", "--batch", "a", "p", ""],
            vec!["actor-precheck"; MAX_DOCUMENTS + 5],
        ] {
            let args: Vec<_> = args.into_iter().map(str::to_owned).collect();
            assert_eq!(arguments(&args).unwrap_err(), USAGE);
        }
        let args: Vec<_> = ["actor-precheck", "--batch", "a", "p", "q1", "q2"]
            .into_iter().map(str::to_owned).collect();
        assert_eq!(arguments(&args).unwrap(), &args[4..]);
    }

    #[test]
    fn all_independent_service_allowances_bound_the_batch_before_connections() {
        let settings = settings();
        assert!(check_count(1, &settings).is_ok());
        assert!(check_count(MAX_DOCUMENTS, &settings).is_ok());
        assert!(check_count(0, &settings).is_err());
        assert!(check_count(MAX_DOCUMENTS + 1, &settings).is_err());
        for field in 0..4 {
            let mut settings = settings.clone();
            match field {
                0 => settings.query_limit = 1,
                1 => settings.exchange_limit = 1,
                2 => settings.connection_limit = 1,
                _ => settings.candidate_limit = 1,
            }
            assert!(check_count(2, &settings).is_err());
        }
    }

    #[test]
    fn query_labels_are_unique_and_epochs_are_independently_bound() {
        let settings = settings(); let mut queries = BTreeSet::new();
        validate_request(&request(97, 0), &settings, &mut queries).unwrap();
        assert!(validate_request(&request(97, 0), &settings, &mut queries).is_err());
        assert!(validate_request(&request(98, 1), &settings, &mut queries).is_err());
        assert_eq!(queries.len(), 1);
        validate_request(&request(98, 0), &settings, &mut queries).unwrap();
        assert_eq!(queries.len(), 2);
    }

    #[test]
    fn work_is_aggregate_and_io_exhaustion_cannot_be_refreshed_by_a_new_member() {
        let mut work = work(2).unwrap();
        assert_eq!(work.read_bytes, 2 * (MAX_PRECHECK_RESPONSE_BYTES + 2));
        assert_eq!(work.write_bytes, 2 * (MAX_PRECHECK_FRAME_BYTES + 1));
        assert_eq!(work.io_calls, MAX_IO_CALLS);
        let progress = DriveProgress { io_calls: MAX_IO_CALLS,
            read_bytes: 1, written_bytes: 1, ..DriveProgress::default() };
        work.debit(progress).unwrap();
        assert!(work.budget(PrecheckClientPhase::Writing).is_err());
        assert!(work.budget(PrecheckClientPhase::Reading).is_err());
        assert_eq!(work.read_bytes, 2 * (MAX_PRECHECK_RESPONSE_BYTES + 2) - 1);
        assert_eq!(work.write_bytes, 2 * (MAX_PRECHECK_FRAME_BYTES + 1) - 1);
    }

    #[test]
    fn batch_rejects_visible_source_cut_changes_and_does_not_treat_unknown_as_known() {
        let mut at = None;
        assert!(record_basis(&known(97, 1, FilePrecheckOutcome::DeniedByPolicy), &mut at).unwrap());
        assert!(record_basis(&known(98, 1, FilePrecheckOutcome::PrecheckPassed), &mut at).unwrap());
        assert!(record_basis(&known(99, 2, FilePrecheckOutcome::PrecheckPassed), &mut at).is_err());
        assert_eq!(at, Some(ElapsedTick(1)));
        let unknown = PrecheckWireResponse { query: Some(99),
            result: Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }) };
        assert!(!record_basis(&unknown, &mut at).unwrap());
    }
}
