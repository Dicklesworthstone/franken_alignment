//! Render only the original selected client image, never a synthesized outcome.
use fa_reference::action::consequence::delivery::persistent::observed::stream::actor_wire::client::FileClientInspection;
use fa_reference::action::consequence::oversight::actor_wire::client::ClientIoWork;
use std::io::Write;
fn work(w: ClientIoWork) -> String {
    format!("{{\"exchanges\":\"{}\",\"calls\":\"{}\",\"written_bytes\":\"{}\",\"read_bytes\":\"{}\"}}",
        w.exchanges, w.calls, w.written_bytes, w.read_bytes)
}
pub(super) fn write(cut: &FileClientInspection, request: u64, status: &str, out: &mut impl Write) -> Result<(), String> {
    // Only fixed status strings are passed by this module's caller. Request bytes
    // and diagnostics never enter JSON through string interpolation.
    if !matches!(status, "inspection" | "observed" | "interrupted") { return Err("invalid client report status".into()); }
    let response = cut.last_response(request).map(|r| String::from_utf8(r.encode()))
        .transpose().map_err(|_| "original response was not UTF-8")?.unwrap_or_else(|| "null".into());
    let reserved = cut.reserved_work().map(work).unwrap_or_else(|| "null".into());
    let steps = cut.reserved_drive().map(|b| b.steps().to_string()).unwrap_or_else(|| "null".into());
    let bytes = format!(concat!("{{\"schema_version\":\"fa.durable-actor-result/1\",\"status\":\"{}\",",
        "\"historical\":true,\"request\":\"{}\",\"client_revision\":\"{}\",",
        "\"request_retained\":{},\"recorded_response\":{},\"accounted_work\":{},",
        "\"reserved_work\":{},\"reserved_drive_steps\":{}}}\n"),
        status, request, cut.revision(), cut.original_proposal(request).is_some(), response,
        work(cut.work()), reserved, steps);
    out.write_all(bytes.as_bytes()).and_then(|()| out.flush()).map_err(|error|
        format!("client history retained at revision {}, but report output failed: {error}; no reconnect or resend", cut.revision()))
}
