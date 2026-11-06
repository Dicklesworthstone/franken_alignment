//! Uses the exact production Journal codec and Rights reducer.
//! This does not exercise FileJournal durability or any external authority.
#![forbid(unsafe_code)]

use fa_journal::replay::{HistoryAnchor, ReplayBudget, ReplayStatus};
use fa_journal::{Event, Identity, Journal, Limits, RecoveryReserve};
use fa_reference::{Effect, State};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Clone, Copy)]
struct Context {
    identity: Identity,
    limits: Limits,
    reserve: Option<RecoveryReserve>,
}

impl Context {
    fn ordinary(version: u8) -> Self {
        Self {
            identity: Identity { run: [1; 32], policy: [2; 32], total_units: 100 },
            limits: Limits { max_frame_bytes: 4096, max_records: 64, max_total_bytes: 16384 },
            reserve: (version == 2).then_some(RecoveryReserve { records: 8, bytes: 512 }),
        }
    }

    fn journal(self) -> Journal {
        match self.reserve {
            Some(reserve) => Journal::new_with_recovery_reserve(self.identity, self.limits, reserve),
            None => Journal::new(self.identity, self.limits),
        }.unwrap()
    }

    fn replay(self, bytes: &[u8], anchor: &[u8]) -> Result<Journal, String> {
        let anchor = match self.reserve {
            Some(reserve) => HistoryAnchor::from_bytes_with_recovery_reserve(
                anchor.to_vec(), self.identity, self.limits, reserve,
            ),
            None => HistoryAnchor::from_bytes(anchor.to_vec(), self.identity, self.limits),
        }.map_err(|error| error.to_string())?;
        let mut replay = match self.reserve {
            Some(reserve) => Journal::begin_replay_anchored_with_recovery_reserve(
                bytes, self.identity, anchor, self.limits, reserve,
            ),
            None => Journal::begin_replay_anchored(bytes, self.identity, anchor, self.limits),
        }.map_err(|error| error.to_string())?;
        while replay.progress().status == ReplayStatus::Reading {
            replay.advance(ReplayBudget::UNLIMITED).map_err(|error| error.to_string())?;
        }
        replay.finish().map_err(|error| error.to_string())
    }
}

fn effect(units: u64, payload: &[u8]) -> Effect {
    Effect {
        principal: "actor".into(),
        resolved_target: "publication".into(),
        payload: payload.to_vec(),
        units,
    }
}

fn append(journal: &mut Journal, bytes: &mut Vec<u8>, event: Event) -> Vec<u8> {
    let frame = journal.append(&event).unwrap();
    bytes.extend_from_slice(&frame);
    frame[16..frame.len() - 4].to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct Suite<'a> {
    directory: &'a Path,
    catalog: String,
}

impl Suite<'_> {
    fn emit(
        &mut self,
        name: &str,
        context: Context,
        bytes: &[u8],
        anchor: &[u8],
        reservations: &[(u64, u64)],
        accepted: bool,
    ) {
        let checked = context.replay(bytes, anchor);
        assert_eq!(checked.is_ok(), accepted, "native replay disagrees for {name}: {checked:?}");
        fs::write(self.directory.join(format!("{name}.journal")), bytes).unwrap();
        fs::write(self.directory.join(format!("{name}.anchor")), anchor).unwrap();
        let reserve = context.reserve.unwrap_or(RecoveryReserve { records: 0, bytes: 0 });
        fs::write(self.directory.join(format!("{name}.context")), format!(
            "version={}\nrun={}\npolicy={}\ntotal_units={}\nmax_frame_bytes={}\nmax_records={}\nmax_total_bytes={}\nreserve_records={}\nreserve_bytes={}\n",
            if context.reserve.is_some() { 2 } else { 1 },
            hex(&context.identity.run), hex(&context.identity.policy), context.identity.total_units,
            context.limits.max_frame_bytes, context.limits.max_records, context.limits.max_total_bytes,
            reserve.records, reserve.bytes,
        )).unwrap();
        if let Ok(journal) = checked {
            let retained = context.replay(anchor, anchor).unwrap();
            let rights = journal.rights();
            assert!(rights.conserved());
            let mut counts = [0_u64; 5];
            let mut units = [0_u64; 5];
            let mut ids = BTreeSet::new();
            for &(id, amount) in reservations {
                assert!(ids.insert(id), "duplicate fixture catalog ID");
                let index = match rights.state(id).unwrap() {
                    State::Reserved => 0,
                    State::Dispatched => 1,
                    State::Unknown => 2,
                    State::Committed => 3,
                    State::Aborted => 4,
                };
                counts[index] += 1;
                units[index] = units[index].checked_add(amount).unwrap();
            }
            // The native ledger has no spent getter. Every catalog amount was
            // actually reserved; classify it using the ORIGINAL ledger state.
            let fields = [
                ("prefix_sequence", retained.head().sequence),
                ("prefix_checksum", u64::from(retained.head().checksum)),
                ("prefix_bytes", retained.encoded_len()),
                ("final_sequence", journal.head().sequence),
                ("final_checksum", u64::from(journal.head().checksum)),
                ("final_bytes", journal.encoded_len()),
                ("total_units", context.identity.total_units),
                ("available_units", rights.available()),
                ("spent_units", units[3]),
                ("reserved_units", units[0]),
                ("dispatched_units", units[1]),
                ("unknown_units", units[2]),
                ("epoch", rights.epoch()),
                ("incident_count", rights.incident_count()),
                ("reservations", reservations.len() as u64),
                ("reserved_count", counts[0]),
                ("dispatched_count", counts[1]),
                ("unknown_count", counts[2]),
                ("committed_count", counts[3]),
                ("aborted_count", counts[4]),
            ];
            let stats = fields.into_iter()
                .map(|(key, value)| format!("{key}={value}\n"))
                .collect::<String>();
            fs::write(self.directory.join(format!("{name}.stats")), stats).unwrap();
        }
        self.catalog.push_str(&format!("{name}\t{}\n", if accepted { "accept" } else { "reject" }));
    }
}

// Test-only hostile framing. Positive fixtures always use native append.
// Valid CRCs force both replayers to examine the malformed/unlawful event.
fn checksum(bytes: &[u8]) -> u32 {
    let mut value = !0_u32;
    for &byte in bytes {
        value ^= u32::from(byte);
        for _ in 0..8 {
            value = if value & 1 == 0 { value >> 1 } else { (value >> 1) ^ 0xedb8_8320 };
        }
    }
    !value
}

fn hostile_successor(journal: &Journal, bytes: &[u8], payload: &[u8]) -> Vec<u8> {
    let mut output = bytes.to_vec();
    let mut frame = Vec::new();
    frame.extend_from_slice(&(12_u32 + u32::try_from(payload.len()).unwrap()).to_le_bytes());
    frame.extend_from_slice(&(journal.head().sequence + 1).to_le_bytes());
    frame.extend_from_slice(&journal.head().checksum.to_le_bytes());
    frame.extend_from_slice(payload);
    frame.extend_from_slice(&checksum(&frame).to_le_bytes());
    output.extend(frame);
    output
}

fn lifecycle(suite: &mut Suite<'_>, version: u8) {
    let context = Context::ordinary(version);
    let mut journal = context.journal();
    let mut bytes = journal.header();
    let amounts = [(0, 10), (1, 20), (2, 15), (3, 12), (4, 8), (5, 5), (6, 4)];
    for &(id, units) in &amounts {
        append(&mut journal, &mut bytes, Event::Reserve {
            id, effect: effect(units, &[id as u8, 0, 255]),
        });
    }
    for &(id, units) in &amounts[..4] {
        append(&mut journal, &mut bytes, Event::Dispatch {
            id, effect: effect(units, &[id as u8, 0, 255]),
        });
    }
    let anchor = bytes.clone();
    append(&mut journal, &mut bytes, Event::MarkUnknown { id: 0 });
    append(&mut journal, &mut bytes, Event::MarkUnknown { id: 2 });
    append(&mut journal, &mut bytes, Event::Reconcile { id: 2, occurred: true });
    append(&mut journal, &mut bytes, Event::Reconcile { id: 3, occurred: false });
    append(&mut journal, &mut bytes, Event::RevokeEpoch);
    append(&mut journal, &mut bytes, Event::AbortBeforeDispatch { id: 4 });
    append(&mut journal, &mut bytes, Event::ResetToCheckpoint {
        keep: BTreeSet::from([2, 4, 5]),
    });
    assert_eq!(journal.rights().available(), 50);
    assert_eq!(journal.rights().state(0), Ok(State::Unknown));
    assert_eq!(journal.rights().state(1), Ok(State::Dispatched));
    assert_eq!(journal.rights().state(5), Ok(State::Reserved));
    assert_eq!(journal.rights().state(6), Ok(State::Aborted));
    suite.emit(&format!("lifecycle_v{version}"), context, &bytes, &anchor, &amounts, true);
    suite.emit(&format!("full_anchor_v{version}"), context, &bytes, &bytes, &amounts, true);
    // Establish each native outcome used by the truncation integration test.
    for end in 0..bytes.len() {
        assert!(context.replay(&bytes[..end], &bytes).is_err(), "native truncation {end}");
    }
    suite.emit(&format!("partial_anchor_v{version}"), context,
        &bytes, &anchor[..anchor.len() - 1], &amounts, false);
    let mut bad_suffix = bytes.clone();
    bad_suffix.push(0);
    suite.emit(&format!("partial_suffix_v{version}"), context,
        &bad_suffix, &anchor, &amounts, false);
}

fn collision(suite: &mut Suite<'_>) {
    let context = Context::ordinary(1);
    let mut left = context.journal();
    let mut right = context.journal();
    let mut left_bytes = left.header();
    let mut right_bytes = right.header();
    append(&mut left, &mut left_bytes, Event::Reserve { id: 1, effect: effect(4, b"plumless") });
    append(&mut right, &mut right_bytes, Event::Reserve { id: 1, effect: effect(4, b"buckeroo") });
    assert_eq!(left.head(), right.head());
    assert_ne!(left_bytes, right_bytes);
    Journal::replay(right_bytes.as_slice(), context.identity, left.head(), context.limits).unwrap();
    suite.emit("collision_left", context, &left_bytes, &left_bytes, &[(1, 4)], true);
    suite.emit("collision_right", context, &right_bytes, &right_bytes, &[(1, 4)], true);
    suite.emit("collision_fork", context, &right_bytes, &left_bytes, &[(1, 4)], false);
}

fn invalid_transitions(suite: &mut Suite<'_>) {
    let context = Context::ordinary(1);
    let mut reserved = context.journal();
    let genesis = reserved.header();
    let mut reserved_bytes = genesis.clone();
    let reserved_payload = append(&mut reserved, &mut reserved_bytes,
        Event::Reserve { id: 1, effect: effect(4, b"payload") });
    let mut dispatched = reserved.clone();
    let mut dispatched_bytes = reserved_bytes.clone();
    let dispatch_payload = append(&mut dispatched, &mut dispatched_bytes,
        Event::Dispatch { id: 1, effect: effect(4, b"payload") });
    let mut unknown = dispatched.clone();
    let mut unknown_bytes = dispatched_bytes.clone();
    let unknown_payload = append(&mut unknown, &mut unknown_bytes, Event::MarkUnknown { id: 1 });
    let mut settled = unknown.clone();
    let mut settled_bytes = unknown_bytes.clone();
    let settlement_payload = append(&mut settled, &mut settled_bytes,
        Event::Reconcile { id: 1, occurred: true });
    let mut aborted = reserved.clone();
    let mut aborted_bytes = reserved_bytes.clone();
    let abort_payload = append(&mut aborted, &mut aborted_bytes, Event::AbortBeforeDispatch { id: 1 });
    let mut stale = reserved.clone();
    let mut stale_bytes = reserved_bytes.clone();
    append(&mut stale, &mut stale_bytes, Event::RevokeEpoch);
    let mut changed = dispatch_payload.clone();
    *changed.last_mut().unwrap() ^= 1;
    let mut changed_principal = dispatch_payload.clone();
    changed_principal[21] ^= 1; // first principal byte, preserving valid UTF-8
    let mut changed_target = dispatch_payload.clone();
    changed_target[30] ^= 1; // first target byte after the five-byte principal
    let mut changed_units = dispatch_payload.clone();
    changed_units[9..17].copy_from_slice(&5_u64.to_le_bytes());
    let mut missing_keep = vec![7];
    missing_keep.extend_from_slice(&1_u32.to_le_bytes());
    missing_keep.extend_from_slice(&999_u64.to_le_bytes());
    for (name, journal, bytes, payload) in [
        ("dispatch_after_revoke", &stale, &stale_bytes, dispatch_payload.as_slice()),
        ("dispatch_twice", &dispatched, &dispatched_bytes, dispatch_payload.as_slice()),
        ("changed_effect", &reserved, &reserved_bytes, changed.as_slice()),
        ("changed_principal", &reserved, &reserved_bytes, changed_principal.as_slice()),
        ("changed_target", &reserved, &reserved_bytes, changed_target.as_slice()),
        ("changed_units", &reserved, &reserved_bytes, changed_units.as_slice()),
        ("unknown_refund", &unknown, &unknown_bytes, abort_payload.as_slice()),
        ("unknown_without_dispatch", &reserved, &reserved_bytes, unknown_payload.as_slice()),
        ("duplicate_unknown", &unknown, &unknown_bytes, unknown_payload.as_slice()),
        ("settle_undispatched", &reserved, &reserved_bytes, settlement_payload.as_slice()),
        ("settle_twice", &settled, &settled_bytes, settlement_payload.as_slice()),
        ("reuse_terminal_id", &aborted, &aborted_bytes, reserved_payload.as_slice()),
        ("missing_checkpoint_id", &reserved, &reserved_bytes, missing_keep.as_slice()),
    ] {
        suite.emit(name, context, &hostile_successor(journal, bytes, payload), &genesis, &[], false);
    }
    // Mutate an actually emitted native successor. The first two retain valid
    // record CRCs, so their refusal must concern sequence/chain continuity.
    let mut framing_source = reserved.clone();
    let mut framing_bytes = reserved_bytes.clone();
    append(&mut framing_source, &mut framing_bytes, Event::RevokeEpoch);
    context.replay(&framing_bytes, &genesis).unwrap();
    let record_offset = reserved_bytes.len();
    let trailer = framing_bytes.len() - 4;
    let mut wrong_sequence = framing_bytes.clone();
    wrong_sequence[record_offset + 4..record_offset + 12]
        .copy_from_slice(&(reserved.head().sequence + 2).to_le_bytes());
    let repaired = checksum(&wrong_sequence[record_offset..trailer]);
    wrong_sequence[trailer..].copy_from_slice(&repaired.to_le_bytes());
    let mut wrong_previous = framing_bytes.clone();
    wrong_previous[record_offset + 12] ^= 1;
    let repaired = checksum(&wrong_previous[record_offset..trailer]);
    wrong_previous[trailer..].copy_from_slice(&repaired.to_le_bytes());
    let mut wrong_checksum = framing_bytes;
    wrong_checksum[trailer] ^= 1;
    for (name, bytes) in [
        ("wrong_sequence", wrong_sequence),
        ("wrong_previous", wrong_previous),
        ("wrong_checksum", wrong_checksum),
    ] {
        suite.emit(name, context, &bytes, &genesis, &[], false);
    }

    let mut invalid_bool = settlement_payload.clone();
    *invalid_bool.last_mut().unwrap() = 2;
    let mut invalid_utf8 = reserved_payload.clone();
    invalid_utf8[21] = 0xff; // tag + id + units + principal length
    let mut zero_units = reserved_payload.clone();
    zero_units[9..17].fill(0);
    let mut duplicate_keep = vec![7];
    duplicate_keep.extend_from_slice(&2_u32.to_le_bytes());
    duplicate_keep.extend_from_slice(&1_u64.to_le_bytes());
    duplicate_keep.extend_from_slice(&1_u64.to_le_bytes());
    let mut oversized_set = vec![7];
    oversized_set.extend_from_slice(&u32::MAX.to_le_bytes());
    let empty = context.journal();
    for (name, journal, bytes, payload) in [
        ("noncanonical_boolean", &unknown, &unknown_bytes, invalid_bool.as_slice()),
        ("invalid_utf8", &empty, &genesis, invalid_utf8.as_slice()),
        ("zero_units", &empty, &genesis, zero_units.as_slice()),
        ("duplicate_checkpoint", &reserved, &reserved_bytes, duplicate_keep.as_slice()),
        ("oversized_checkpoint", &reserved, &reserved_bytes, oversized_set.as_slice()),
        ("trailing_payload", &empty, &genesis, &[6, 0][..]),
        ("unknown_tag", &empty, &genesis, &[255][..]),
    ] {
        suite.emit(name, context, &hostile_successor(journal, bytes, payload), &genesis, &[], false);
    }
}

fn capacity(suite: &mut Suite<'_>) {
    let context = Context {
        identity: Context::ordinary(2).identity,
        limits: Limits { max_frame_bytes: 1024, max_records: 5, max_total_bytes: 4096 },
        reserve: Some(RecoveryReserve { records: 3, bytes: 90 }),
    };
    let mut journal = context.journal();
    let mut bytes = journal.header();
    let reserve_payload = append(&mut journal, &mut bytes,
        Event::Reserve { id: 1, effect: effect(4, b"tail") });
    append(&mut journal, &mut bytes, Event::Dispatch { id: 1, effect: effect(4, b"tail") });
    let anchor = bytes.clone();
    let mut new_reservation = reserve_payload.clone();
    new_reservation[1..9].copy_from_slice(&2_u64.to_le_bytes());
    suite.emit("ordinary_record_tail", context,
        &hostile_successor(&journal, &bytes, &new_reservation), &anchor, &[], false);
    append(&mut journal, &mut bytes, Event::MarkUnknown { id: 1 });
    append(&mut journal, &mut bytes, Event::RevokeEpoch);
    append(&mut journal, &mut bytes, Event::Reconcile { id: 1, occurred: false });
    suite.emit("recovery_record_boundary", context, &bytes, &anchor, &[(1, 4)], true);
    suite.emit("recovery_record_overflow", context,
        &hostile_successor(&journal, &bytes, &[6]), &anchor, &[], false);

    let ordinary_bytes = anchor.len() as u64 - 120;
    let byte_context = Context {
        limits: Limits {
            max_total_bytes: 120 + ordinary_bytes + 80,
            max_records: 20,
            ..context.limits
        },
        reserve: Some(RecoveryReserve { records: 3, bytes: 80 }),
        ..context
    };
    let mut byte_journal = byte_context.journal();
    let mut byte_bytes = byte_journal.header();
    append(&mut byte_journal, &mut byte_bytes, Event::Reserve { id: 1, effect: effect(4, b"tail") });
    append(&mut byte_journal, &mut byte_bytes, Event::Dispatch { id: 1, effect: effect(4, b"tail") });
    let byte_anchor = byte_bytes.clone();
    suite.emit("ordinary_byte_tail", byte_context,
        &hostile_successor(&byte_journal, &byte_bytes, &new_reservation), &byte_anchor, &[], false);
    append(&mut byte_journal, &mut byte_bytes, Event::MarkUnknown { id: 1 });
    append(&mut byte_journal, &mut byte_bytes, Event::RevokeEpoch);
    append(&mut byte_journal, &mut byte_bytes, Event::Reconcile { id: 1, occurred: false });
    assert_eq!(byte_journal.encoded_len(), byte_context.limits.max_total_bytes);
    suite.emit("recovery_byte_boundary", byte_context, &byte_bytes, &byte_anchor, &[(1, 4)], true);
    suite.emit("recovery_byte_overflow", byte_context,
        &hostile_successor(&byte_journal, &byte_bytes, &[6]), &byte_anchor, &[], false);
    for (name, altered) in [
        ("v2_changed_reserve", Context {
            reserve: Some(RecoveryReserve { records: 2, bytes: 90 }), ..context
        }),
        ("v2_changed_limit", Context {
            limits: Limits { max_records: 6, ..context.limits }, ..context
        }),
        ("v2_wrong_profile", Context { reserve: None, ..context }),
    ] {
        suite.emit(name, altered, &bytes, &anchor, &[], false);
    }
}

fn edge_cases_and_vectors(suite: &mut Suite<'_>) {
    let mut context = Context::ordinary(1);
    context.identity.total_units = 0;
    let journal = context.journal();
    suite.emit("zero_total", context, &journal.header(), &journal.header(), &[], true);
    context.identity.total_units = u64::MAX;
    let mut journal = context.journal();
    let mut bytes = journal.header();
    let anchor = bytes.clone();
    let exact = Effect {
        principal: "\0".into(), resolved_target: " ".into(), payload: vec![], units: u64::MAX,
    };
    append(&mut journal, &mut bytes, Event::Reserve { id: 0, effect: exact.clone() });
    append(&mut journal, &mut bytes, Event::Dispatch { id: 0, effect: exact });
    append(&mut journal, &mut bytes, Event::MarkUnknown { id: 0 });
    suite.emit("max_units_empty_payload", context, &bytes, &anchor, &[(0, u64::MAX)], true);

    let v1 = Context { limits: Limits::default(), ..Context::ordinary(1) };
    let mut journal = v1.journal();
    let anchor = journal.header();
    let mut bytes = anchor.clone();
    append(&mut journal, &mut bytes, Event::RevokeEpoch);
    suite.emit("v1_wire_vector", v1, &bytes, &anchor, &[], true);
    let v2 = Context {
        limits: Limits { max_frame_bytes: 1024, max_records: 9, max_total_bytes: 1024 },
        reserve: Some(RecoveryReserve { records: 5, bytes: 160 }),
        ..Context::ordinary(2)
    };
    let bytes = v2.journal().header();
    suite.emit("v2_wire_vector", v2, &bytes, &bytes, &[], true);
    for (name, altered) in [
        ("wrong_run", Context { identity: Identity { run: [3; 32], ..v1.identity }, ..v1 }),
        ("wrong_policy", Context { identity: Identity { policy: [3; 32], ..v1.identity }, ..v1 }),
        ("wrong_total", Context { identity: Identity { total_units: 101, ..v1.identity }, ..v1 }),
    ] {
        suite.emit(name, altered, &anchor, &anchor, &[], false);
    }
    let mut oversized = anchor.clone();
    oversized.extend_from_slice(&u32::MAX.to_le_bytes());
    suite.emit("hostile_frame_length", v1, &oversized, &anchor, &[], false);
}

fn main() {
    let arguments: Vec<_> = std::env::args_os().collect();
    assert_eq!(arguments.len(), 2, "expected one fixture directory");
    let directory = Path::new(&arguments[1]);
    let mut suite = Suite { directory, catalog: String::new() };
    lifecycle(&mut suite, 1);
    lifecycle(&mut suite, 2);
    collision(&mut suite);
    invalid_transitions(&mut suite);
    capacity(&mut suite);
    edge_cases_and_vectors(&mut suite);
    fs::write(directory.join("catalog.tsv"), suite.catalog).unwrap();
}
