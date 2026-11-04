//! Read-only browser projection of the original publication reader.
//! No live owner, fence, recovery, helper, prompt, payload or credential escapes.
use crate::config::Config;
use fa_reference::action::consequence::delivery::persistent::observed::{
    FileOversight, FileOversightProfile,
};
use fa_reference::action::consequence::mediation::CutCheck;
use fa_reference::perimeter_inventory::{MAX_INVENTORY_BYTES, topology::{LoadedAuthorityTopology, MAX_TOPOLOGY_BYTES}};
use fa_reference::strict_json::Json;
use std::{collections::BTreeMap, fmt::Write, path::{Path, PathBuf}};

pub const SCHEMA: &str = "fa.observatory/1";
const MAX_GRAPH_NODES: usize = 128;
const MAX_GRAPH_EDGES: usize = 512;

pub struct Source {
    store: PathBuf,
    profile: FileOversightProfile,
    graph: Option<(PathBuf, PathBuf)>,
}
impl Source {
    pub fn read_config(path: &Path, graph: Option<(PathBuf, PathBuf)>) -> Result<Self, String> {
        let config = Config::read(path)?;
        // Discard program launch data and the evidence reader. The dashboard
        // cannot launch helpers or acquire fresh policy evidence.
        Ok(Self { store: config.store, profile: config.profile, graph })
    }

    pub fn sample(&self) -> Result<Json, String> {
        let state = FileOversight::read_publication(&self.store, &self.profile)
            .map_err(|error| format!("Native publication replay unavailable: {error}"))?;
        let ledger = &state.control.ledger;
        let scope = self.profile.delivery.scope;
        let conserved = ledger.available.checked_add(ledger.reserved)
            .and_then(|value| value.checked_add(ledger.charged)) == Some(self.profile.delivery.total);
        let attempts = ledger.stages.iter().map(|(id, stage)| object([
            ("id", text(id)), ("stage", text(format!("{stage:?}"))),
            ("decision", state.control.decisions.get(id)
                .map_or(Json::Null, |value| text(format!("{value:?}")))),
        ])).collect();
        let helpers = self.profile.delivery.congress.members.iter().map(|(name, member)| object([
            ("name", text(name)), ("cohort", text(&member.cohort)),
            ("weight", text(member.weight)),
        ])).collect();
        Ok(object([
            ("scope", object([("tenant", text(scope.tenant)), ("principal", text(scope.principal)),
                ("run", text(scope.run)), ("branch", text(scope.branch)), ("authority", text(scope.authority))])),
            ("revision", text(state.revision)), ("control_sequence", text(state.control.sequence)),
            ("authority_epoch", text(ledger.epoch)), ("dispatcher_epoch", text(state.dispatcher_epoch)),
            ("elapsed_tick", ledger.elapsed.map_or(Json::Null, |tick| text(tick.0))),
            ("clock_domain", text(self.profile.delivery.clock_domain)),
            ("rights", object([("total", text(self.profile.delivery.total)),
                ("available", text(ledger.available)), ("reserved", text(ledger.reserved)),
                ("charged", text(ledger.charged)), ("conserved", Json::Bool(conserved))])),
            ("executions", text(state.executions)), ("payload_bytes", text(state.payload.len())),
            ("suspended", Json::Bool(state.control.suspended)),
            ("narrowed", Json::Bool(state.control.ceiling.is_some())),
            ("stopped", Json::Bool(state.stop.is_some())),
            ("journal_event_limit", text(self.profile.delivery.limits.events)),
            ("attempts", Json::Array(attempts)), ("helpers", Json::Array(helpers)),
            ("topology", self.graph.as_ref().map_or(Json::Null, |(topology, inventory)|
                match read_graph(topology, inventory) {
                    Ok(graph) => graph,
                    Err(_) => object([("kind", text("unknown")),
                        ("reason", text("Declared topology input is unavailable or invalid; no cached certification is substituted."))]),
                })),
        ]))
    }
}

fn read_graph(topology: &Path, inventory: &Path) -> Result<Json, String> {
    let graph = crate::config::read_regular(topology, MAX_TOPOLOGY_BYTES)?;
    let inventory = crate::config::read_regular(inventory, MAX_INVENTORY_BYTES)?;
    let loaded = LoadedAuthorityTopology::from_json_bytes(&graph, &inventory).map_err(crate::config::debug)?;
    let spec = loaded.graph().spec();
    let check = match loaded.check() {
        Ok(CutCheck::Verified(_)) => "verified_offline",
        Ok(_) => "not_certified",
        Err(_) => "not_certified",
    };
    // The graph is separately read declaration data, NOT an installed native
    // certificate and NOT an atomic snapshot with the authority journal.
    let nodes: Vec<_> = spec.nodes.iter().take(MAX_GRAPH_NODES).map(|node| object([
        ("id", text(node.id)), ("kind", text(format!("{:?}", node.kind))),
    ])).collect();
    let retained: std::collections::BTreeSet<_> = spec.nodes.iter().take(MAX_GRAPH_NODES).map(|node| node.id).collect();
    let edges: Vec<_> = spec.edges.iter().filter(|edge| retained.contains(&edge.from) && retained.contains(&edge.to))
        .take(MAX_GRAPH_EDGES).map(|edge| object([
            ("id", text(edge.id)), ("from", text(edge.from)), ("to", text(edge.to)),
            ("channel", text(format!("{:?}", edge.channel))),
        ])).collect();
    let truncated = nodes.len() != spec.nodes.len() || edges.len() != spec.edges.len();
    Ok(object([
        ("kind", text("known")), ("basis", text("independent_declaration_not_live_authority")),
        ("check", text(check)), ("generation", text(spec.generation)),
        ("inventory_generation", text(spec.inventory_generation)),
        ("nodes", Json::Array(nodes)), ("edges", Json::Array(edges)),
        ("total_nodes", text(spec.nodes.len())), ("total_edges", text(spec.edges.len())),
        ("truncated", Json::Bool(truncated)),
    ]))
}

pub fn envelope(mode: &str, sequence: u64, sampled_at: u64, result: Result<Json, String>) -> Json {
    let (kind, value, reason) = match result {
        Ok(value) => ("known", value, Json::Null),
        Err(_) => ("unknown", Json::Null, text("The original reader could not validate this history. Check the local terminal, bootstrap profile, and journal. Loaded-model histories may require a specialized replay reader.")),
    };
    object([
        ("schema", text(SCHEMA)), ("mode", text(mode)), ("kind", text(kind)),
        ("sample_id", text(sequence)), ("observed_at_ms", text(sampled_at)),
        ("basis", text("canonical_replay; historical_observation; owner_acknowledgment_not_attested")),
        ("value", value), ("reason", reason),
    ])
}

pub fn text(value: impl ToString) -> Json { Json::String(value.to_string()) }
pub fn object<const N: usize>(fields: [(&str, Json); N]) -> Json {
    Json::Object(fields.into_iter().map(|(key, value)| (key.to_owned(), value)).collect::<BTreeMap<_, _>>())
}

/// All u64 identities/counters are decimal STRINGS, never rounded browser numbers.
/// Strings are escaped as JSON, not injected into HTML or executable script.
pub fn encode(value: &Json) -> String {
    fn emit(value: &Json, out: &mut String) {
        match value {
            Json::Null => out.push_str("null"),
            Json::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Json::Number(value) => out.push_str(value.lexeme()),
            Json::String(value) => {
                out.push('"');
                for c in value.chars() {
                    match c {
                        '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"),
                        '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"), '\t' => out.push_str("\\t"),
                        c if c < '\u{20}' => { let _ = write!(out, "\\u{:04x}", c as u32); }
                        c => out.push(c),
                    }
                }
                out.push('"');
            }
            Json::Array(values) => {
                out.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index > 0 { out.push(','); } emit(value, out);
                }
                out.push(']');
            }
            Json::Object(values) => {
                out.push('{');
                for (index, (key, value)) in values.iter().enumerate() {
                    if index > 0 { out.push(','); }
                    emit(&Json::String(key.clone()), out); out.push(':'); emit(value, out);
                }
                out.push('}');
            }
        }
    }
    let mut out = String::new(); emit(value, &mut out); out
}

#[cfg(test)]
mod tests {
    use super::*;
    use fa_reference::{Snapshot, strict_json::{self, Limits}};
    use fa_reference::action::{ActionSpec, ElapsedTick, VERSION};
    use std::{fs, sync::atomic::{AtomicU64, Ordering}};

    struct Root(PathBuf);
    impl Root {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            let path = std::env::temp_dir().join(format!("fa-observatory-{}-{stamp}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&path).unwrap(); Self(path)
        }
    }
    impl Drop for Root {
        fn drop(&mut self) { if let Err(error) = fs::remove_dir_all(&self.0) { eprintln!("observatory cleanup: {error}"); } }
    }
    fn fixture(root: &Root) -> Source {
        let config = Config::decode(include_bytes!("../../fixtures/supervised_publication.json")).unwrap();
        Source { store: root.0.join("store"), profile: config.profile, graph: None }
    }

    #[test]
    fn reads_locked_original_owner_without_fencing_or_exposing_payloads() {
        let root = Root::new(); let source = fixture(&root);
        let (mut host, _) = FileOversight::create(&source.store, source.profile.clone()).unwrap();
        host.observe_time(host.revision(), ElapsedTick(1)).unwrap();
        let secret = b"payload-never-goes-to-the-dashboard";
        host.propose(host.revision(), 9, ActionSpec { version: VERSION, scope: source.profile.delivery.scope,
            target: Some(source.profile.delivery.target), payload: secret.to_vec(), required_witnesses: Vec::new(),
            policy_epoch: 0, deadline: ElapsedTick(5000), units: 64 },
            Snapshot { semantic_epoch: 1, complete: true, values: BTreeMap::from([(7, b"ok".to_vec())]) }).unwrap();
        let before = fs::read(source.store.join("delivery.bin")).unwrap();
        let data = source.sample().unwrap();
        assert_eq!(data.get("revision").and_then(Json::as_str), Some(host.revision().to_string().as_str()));
        assert_eq!(data.get("attempts").unwrap().as_array().unwrap().len(), 1);
        let json = encode(&data);
        assert!(!json.contains(std::str::from_utf8(secret).unwrap()));
        assert!(!json.contains("question_hex")); assert!(!json.contains("environment"));
        assert_eq!(fs::read(source.store.join("delivery.bin")).unwrap(), before);
        host.cancel(host.revision(), 9).unwrap();
        assert_eq!(source.sample().unwrap().get("attempts").unwrap().as_array().unwrap()[0]
            .get("stage").and_then(Json::as_str), Some("Cancelled"));
    }

    #[test]
    fn missing_or_invalid_journal_is_unknown_not_a_demo_or_a_fresh_owner() {
        let root = Root::new(); let source = fixture(&root);
        assert!(source.sample().is_err()); assert!(!source.store.exists());
        let (host, _) = FileOversight::create(&source.store, source.profile.clone()).unwrap();
        fs::write(source.store.join("delivery.bin"), b"invalid").unwrap();
        let result = envelope("live", 1, 2, source.sample());
        assert_eq!(result.get("kind").and_then(Json::as_str), Some("unknown"));
        assert!(result.get("value").unwrap().is_null());
        assert_eq!(fs::read(source.store.join("delivery.bin")).unwrap(), b"invalid");
        drop(host);
    }

    #[test]
    fn json_preserves_full_width_counters_unicode_and_untrusted_strings() {
        let value = object([("id", text(u64::MAX)), ("text", text("\"\\\n\r\t\0</script>🧭"))]);
        assert_eq!(strict_json::parse(encode(&value).as_bytes(), Limits::default()).unwrap(), value);
        assert_eq!(value.get("id").and_then(Json::as_str), Some("18446744073709551615"));
    }
}
