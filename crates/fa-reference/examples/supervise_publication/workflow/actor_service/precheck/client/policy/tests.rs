//! Closed operator grammar, selected-disclosure binding and cumulative work.
use super::*;
use fa_reference::action::consequence::delivery::persistent::requests::actor::precheck_wire::{
    PrecheckWireError, policy::decode_policy_response,
};
use fa_reference::action::consequence::oversight::{actor::UnknownReason,
    actor_peer::{PeerCredentials, PeerPolicy}};
use std::io::Read;

#[test]
fn full_read_grammar_is_owned_before_any_shared_flags_or_paths_are_opened() {
    let base: Vec<String> = ["actor-precheck", "--policy", "missing-actor", "missing-profile", "7"]
        .into_iter().map(str::to_owned).collect();
    for label in ["", "0", "01", "+1", "-1", "1.0", "18446744073709551616", "missing-query.json"] {
        let mut args = base.clone(); args[4] = label.into();
        assert_eq!(crate::command(args), Err(USAGE.into()));
    }
    for option in ["--andon", "--credibility-activation", "--durable", "--precheck-profile", "--batch"] {
        let mut args = base.clone(); args.extend([option.into(), "missing".into()]);
        assert_eq!(crate::command(args), Err(USAGE.into()));
    }
    let mut args = base; args[4] = u64::MAX.to_string();
    assert_eq!(arguments(&args).unwrap(), u64::MAX);
    args.pop(); assert_eq!(arguments(&args), Err(USAGE.into()));
}

#[test]
fn refusals_render_unchanged_and_never_become_success_or_a_definition() {
    for result in [
        Ok(Knowledge::Unknown { reason: UnknownReason::ControllerUnavailable }),
        Ok(Knowledge::Withheld { authority_required: "exact_policy_disclosure" }),
        Ok(Knowledge::Stale { generation: 0, current: 1 }),
        Err(PrecheckWireError::Capacity),
    ] {
        let response = PolicyResponse { query: Some(7), result };
        let mut output = Vec::new(); assert!(emit(&response, 81, &mut output).is_err());
        assert_eq!(output, [response.encode().unwrap(), b"\n".to_vec()].concat());
        assert!(!std::str::from_utf8(&output).unwrap().contains("definition"));
    }
}

#[test]
fn selected_disclosure_is_checked_before_any_native_policy_bytes_are_printed() {
    let bytes = br#"{"schema":"fa.actor-policy-response/1","informational":true,"query":"7","status":"ok","knowledge":{"state":"known","value":{"policy_disclosure":"full","scope":"exact_policy_definition","source_values":"withheld","helper_feedback":"withheld","consequences":"withheld","effect_authority":false,"review_required":true,"definition":{"generation":"1","nodes":[{"op":"payload_at_most","maximum":"4"}]}},"basis":{"disclosure":"81","query":"7","policy_epoch":"0","observed_at":"1"}}}"#;
    let response = decode_policy_response(bytes, 7, 0).unwrap();
    let mut output = Vec::new();
    assert!(emit(&response, 82, &mut output).unwrap_err().contains("another disclosure"));
    assert!(output.is_empty());
    emit(&response, 81, &mut output).unwrap();
    assert_eq!(output, [response.encode().unwrap(), b"\n".to_vec()].concat());
}

#[test]
fn cumulative_call_or_elapsed_exhaustion_never_retransmits_the_policy_request() {
    for exhausted_calls in [false, true] {
        let (socket, mut server) = UnixStream::pair().unwrap(); let id = PeerCredentials::observe(&socket).unwrap();
        let policy = PeerPolicy::new(id.uid(), id.gid(), Some(id.pid())).unwrap();
        let request = PolicyRequest { query: 7, expected_policy_epoch: 0 };
        let client = PrecheckPolicyClient::connect_policy(socket, policy, request, 0).unwrap();
        let mut work = work();
        assert_eq!((work.read_bytes, work.write_bytes, work.io_calls), (196_610, 257, 4_096));
        if exhausted_calls { work.io_calls = 3; }
        let wall = Duration::from_secs(15); let mut checks = 0;
        let result = drive_client_shared(client, 1, wall, &mut work, || {
            checks += 1;
            if !exhausted_calls && checks > 2 { wall } else { Duration::ZERO }
        });
        assert!(result.unwrap_err().contains(if exhausted_calls { "I/O allowance" } else { "elapsed allowance" }));
        let mut received = Vec::new(); server.read_to_end(&mut received).unwrap();
        assert_eq!(received, [request.encode().unwrap(), b"\n".to_vec()].concat());
    }
}
