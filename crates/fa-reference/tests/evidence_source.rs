use fa_reference::action::consequence::oversight::evidence_source::*;
use fa_reference::action::consequence::oversight::{CommitteeContract, HelperContract};
use fa_reference::action::{ActionSpec, ElapsedTick, FrozenAction, Purpose, ResolvedTarget, Scope, VERSION};
use fa_reference::full_input::InputProfileBinding;
use fa_reference::{Error, ReadWitness, Snapshot};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn scope() -> Scope {
    Scope { tenant: 1, principal: 2, run: 3, branch: 4, authority: 5, purpose: Purpose::Effect }
}
fn snapshot(generation: u64) -> EvidenceSnapshot {
    EvidenceSnapshot::new(EvidenceIdentity { source: 9, generation, scope: scope() }, Snapshot {
        semantic_epoch: 2, complete: true,
        values: BTreeMap::from([(7, b"private-policy-sentinel".to_vec())]),
    }, BTreeMap::from([
        ("alice".to_owned(), b"alice-only-context".to_vec()),
        ("bob".to_owned(), b"bob-only-context".to_vec()),
    ])).unwrap()
}
fn action() -> FrozenAction {
    FrozenAction::freeze(ActionSpec {
        version: VERSION, scope: scope(),
        target: Some(ResolvedTarget { adapter: 1, object: 1, contract_version: 1, expected_version: 1, generation: 1 }),
        payload: b"publish".to_vec(), units: 7, deadline: ElapsedTick(100), policy_epoch: 3,
        required_witnesses: vec![ReadWitness::Exact { key: 7, value: Some(b"private-policy-sentinel".to_vec()) }],
    }).unwrap()
}
fn contracts() -> CommitteeContract {
    CommitteeContract::new(["alice", "bob"].into_iter().map(|name| (name.to_owned(), HelperContract::new(
        InputProfileBinding { profile_id: 1, profile_bytes: b"file-context-v1".to_vec(), model_epoch: 1, tokenizer_epoch: 1, policy_epoch: 0 },
        2, b"Review the proposed publication".to_vec(),
    ).unwrap())).collect()).unwrap()
}
fn includes(haystack: &[u8], needle: &[u8]) -> bool { haystack.windows(needle.len()).any(|part| part == needle) }

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("fa-evidence-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap(); Self(path)
    }
    fn file(&self) -> PathBuf { self.0.join("evidence.json") }
    fn publish(&self, bytes: &[u8]) {
        let staged = self.0.join("next.json");
        fs::write(&staged, bytes).unwrap();
        fs::rename(staged, self.file()).unwrap();
    }
    fn source(&self) -> FileEvidenceSource { FileEvidenceSource::new(self.file(), 9, scope(), MAX_EVIDENCE_FILE_BYTES).unwrap() }
}
impl Drop for Directory { fn drop(&mut self) { fs::remove_dir_all(&self.0).unwrap(); } }

#[test]
fn exact_interchange_retains_private_state_and_arbitrary_context_bytes() {
    let mut contexts = snapshot(1).contexts().clone();
    contexts.insert("quoted\"name\\\n🦀".to_owned(), (0..=255).collect());
    let original = EvidenceSnapshot::new(snapshot(1).identity(), snapshot(1).snapshot().clone(), contexts).unwrap();
    let encoded = original.encode();
    assert_eq!(EvidenceSnapshot::decode(&encoded).unwrap(), original);
    assert_eq!(EvidenceSnapshot::decode(&encoded).unwrap().encode(), encoded);
    for end in 0..encoded.len() { assert!(EvidenceSnapshot::decode(&encoded[..end]).is_err()); }
}

#[test]
fn helper_inputs_preserve_exact_context_and_do_not_disclose_private_policy_or_peers() {
    let observed = snapshot(1);
    let action = action();
    let contract = contracts();
    let inputs = observed.inputs_for(&action, &contract).unwrap();
    inputs.validate_for(&action, &contract).unwrap();
    for (member, peer) in [("alice", "bob"), ("bob", "alice")] {
        let input = inputs.views()[member].actual_input();
        assert!(input.part_bytes(2).unwrap().ends_with(&observed.contexts()[member]));
        assert!(!includes(input.submitted_bytes(), b"private-policy-sentinel"));
        assert!(!includes(input.submitted_bytes(), &observed.contexts()[peer]));
        assert_eq!(input.input_profile().policy_epoch, action.spec().policy_epoch);
    }
}

#[test]
fn input_identity_changes_even_when_the_new_context_bytes_are_identical() {
    let first = snapshot(1);
    let next = snapshot(2);
    assert_ne!(first.reference_root(), next.reference_root());
    assert_ne!(first.inputs_for(&action(), &contracts()).unwrap(), next.inputs_for(&action(), &contracts()).unwrap());
    let mut spec = action().spec().clone(); spec.scope.run += 1;
    assert_eq!(first.inputs_for(&FrozenAction::freeze(spec).unwrap(), &contracts()), Err(Error::Binding));
    let mut contexts = first.contexts().clone(); contexts.remove("bob");
    let missing = EvidenceSnapshot::new(first.identity(), first.snapshot().clone(), contexts).unwrap();
    assert_eq!(missing.inputs_for(&action(), &contracts()), Err(Error::Binding));
}

#[test]
fn incomplete_is_not_upgraded_by_a_valid_file_or_an_empty_value_map() {
    let original = snapshot(1);
    let mut state = original.snapshot().clone(); state.complete = false; state.values.clear();
    let incomplete = EvidenceSnapshot::new(original.identity(), state, original.contexts().clone()).unwrap();
    let decoded = EvidenceSnapshot::decode(&incomplete.encode()).unwrap();
    assert!(!decoded.snapshot().complete);
    assert!(decoded.snapshot().values.is_empty());
}

#[test]
fn duplicate_unknown_fields_and_inexact_identifier_spellings_refuse() {
    let valid = String::from_utf8(snapshot(1).encode()).unwrap();
    for changed in [
        valid.replacen("\"version\":1", "\"version\":1,\"version\":1", 1),
        valid.replacen("\"version\":1", "\"version\":2", 1),
        valid.replacen("\"version\":1", "\"version\":1.0", 1),
        valid.replacen("\"version\":1", "\"version\":1,\"permit\":true", 1),
        valid.replacen("\"source\":\"9\"", "\"source\":9", 1),
        valid.replacen("\"source\":\"9\"", "\"source\":\"09\"", 1),
        valid.replacen("\"source\":\"9\"", "\"source\":\"18446744073709551616\"", 1),
        valid.replacen("\"purpose\":\"effect\"", "\"purpose\":\"experiment\"", 1),
        format!("{valid} {{}}"),
    ] { assert!(EvidenceSnapshot::decode(changed.as_bytes()).is_err(), "{changed}"); }
}

#[test]
fn every_decoded_scalar_and_aggregate_limit_has_a_positive_boundary() {
    let original = snapshot(1);
    let mut state = original.snapshot().clone();
    state.values = (0..8).map(|id| (id, vec![7; MAX_EVIDENCE_VALUE_BYTES])).collect();
    let bounded = EvidenceSnapshot::new(original.identity(), state.clone(), original.contexts().clone()).unwrap();
    assert_eq!(EvidenceSnapshot::decode(&bounded.encode()).unwrap(), bounded);
    state.values.insert(9, vec![1]);
    assert_eq!(EvidenceSnapshot::new(original.identity(), state, original.contexts().clone()), Err(Error::Limit));
    let context = BTreeMap::from([("alice".to_owned(), vec![1; MAX_HELPER_CONTEXT_BYTES + 1])]);
    assert_eq!(EvidenceSnapshot::new(original.identity(), original.snapshot().clone(), context), Err(Error::Limit));
    let mut many = original.snapshot().clone(); many.values = (0..=MAX_EVIDENCE_VALUES as u64).map(|k| (k, Vec::new())).collect();
    assert_eq!(EvidenceSnapshot::new(original.identity(), many, original.contexts().clone()), Err(Error::Limit));
}

#[test]
fn file_rereads_preserve_floors_and_never_return_a_cached_success_on_error() {
    let directory = Directory::new(); directory.publish(&snapshot(1).encode());
    let mut source = directory.source();
    let old = source.read().unwrap();
    let mut whitespace = snapshot(1).encode(); whitespace.extend_from_slice(b" \n"); directory.publish(&whitespace);
    assert_eq!(source.read().unwrap().as_ref(), old.as_ref());
    directory.publish(b"{broken");
    assert!(source.read().is_err()); assert!(source.current().is_none());
    assert_eq!(source.status().retained_generation, Some(1));
    assert_eq!(old.identity().generation, 1);
    directory.publish(&snapshot(2).encode()); source.read().unwrap();
    directory.publish(&snapshot(1).encode());
    assert_eq!(source.read().unwrap_err(), EvidenceError::Data(Error::Stale));
    assert!(source.current().is_none()); assert_eq!(source.status().retained_generation, Some(2));
}

#[test]
fn same_generation_substitution_and_semantic_epoch_rollback_refuse() {
    let directory = Directory::new(); directory.publish(&snapshot(2).encode());
    let mut source = directory.source(); source.read().unwrap();
    let original = snapshot(2);
    let mut contexts = original.contexts().clone(); contexts.get_mut("alice").unwrap().push(1);
    let substituted = EvidenceSnapshot::new(original.identity(), original.snapshot().clone(), contexts).unwrap();
    directory.publish(&substituted.encode());
    assert_eq!(source.read().unwrap_err(), EvidenceError::Data(Error::Binding));
    let mut state = original.snapshot().clone(); state.semantic_epoch = 1;
    let rollback = EvidenceSnapshot::new(EvidenceIdentity { generation: 3, ..original.identity() }, state, original.contexts().clone()).unwrap();
    directory.publish(&rollback.encode());
    assert_eq!(source.read().unwrap_err(), EvidenceError::Data(Error::Stale));
    directory.publish(&original.encode()); source.read().unwrap();
    assert!(source.status().available);
}

#[test]
fn wrong_namespace_missing_file_and_capacity_refuse_without_disclosing_paths() {
    let directory = Directory::new();
    let mut source = directory.source(); assert!(matches!(source.read(), Err(EvidenceError::Io(_))));
    directory.publish(&snapshot(1).encode()); source.read().unwrap();
    let original = snapshot(2);
    let foreign = EvidenceSnapshot::new(EvidenceIdentity { source: 10, ..original.identity() }, original.snapshot().clone(), original.contexts().clone()).unwrap();
    directory.publish(&foreign.encode()); assert_eq!(source.read().unwrap_err(), EvidenceError::Data(Error::Binding));
    assert!(source.current().is_none());
    assert!(!format!("{source:?}").contains(directory.file().to_str().unwrap()));
    let mut limited = FileEvidenceSource::new(directory.file(), 9, scope(), 8).unwrap();
    assert_eq!(limited.read().unwrap_err(), EvidenceError::Data(Error::Limit));
    assert!(FileEvidenceSource::new(Path::new("relative.json"), 9, scope(), 100).is_err());
}

#[cfg(unix)]
#[test]
fn symlink_sources_are_not_silently_followed() {
    let directory = Directory::new();
    fs::write(directory.0.join("real.json"), snapshot(1).encode()).unwrap();
    std::os::unix::fs::symlink("real.json", directory.file()).unwrap();
    assert_eq!(directory.source().read().unwrap_err(), EvidenceError::Data(Error::Binding));
}
