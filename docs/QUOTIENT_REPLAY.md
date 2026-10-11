# Bounded quotient replay oracle

This reference implementation addresses FA-093, plan section 11.8 and invariant
FA-INV-028. It serves the structural-sharing and lawful-trace-quotient engineering
addition linked to FI-I13 and FI-I15. It compares complete finite unreduced
schedules against reductions admitted by the original FA-092 independence
relation. The module is `fa_reference::trace_independence::quotient`.

## What is actually replayed

`ReplayOccurrence::new` binds a stable, nonzero occurrence identity to an
immutable `RegisteredTraceOperation` and one instruction from a closed logical
machine. It rejects instructions whose actual accesses or ordering barriers are
not covered by that registration. Extra dependencies remain conservative.

The instructions read/write whole-domain integer cells, copy with checked
addition, check positive or closed-absence evidence, consume a finite resource,
advance a deterministic RNG state, change a fixture authority flag, or record
ordered/external fixture observations. Reads, writes, resources and RNG state
share the original whole-domain namespace. The existing relation treats any
mutation colliding with another read or mutation as dependent, including
cross-category resource/RNG/write collisions.

Evidence checks call the original consequence `decide` function. Missing
required evidence holds the effect; a present wrong value denies it. These are
computed decisions. There is no caller callback that can report a fabricated
successful comparison. The authority flag and external observations are fixture
state, never a live permit, production executor or external side effect.

Each operation yields a result, including Missing, Overflow or Limit. A failed
operation does not abort the schedule or suppress later independent operations.
This deliberate per-occurrence failure semantics keeps every admitted schedule
and all failed work in the population. It is not a globally aborting executor.

## Canonicalization and exhaustive comparison

Each call admits at most eight distinct occurrences: at most 40,320 permutations.
The empty input has one empty schedule. Identity validation rejects duplicate,
missing and unknown occurrences rather than quietly changing the population.

`canonicalize` uses the original `independent` predicate over registered operator
pairs and complete dependency metadata. For a given order it preserves the
relative order of every dependent pair, then selects the lexicographically
smallest topological order by stable occurrence identity. This is the canonical
representative of the class reachable by lawful independent adjacent swaps.
Unregistered pairs, repeated operator identities, unknown dependencies and
authority/ordered/external barriers remain ordered.

`verify` enumerates every permutation, including every dependent order. It
replays each unreduced order from the original initial state and also replays
each distinct representative from that same initial state. It compares:

- Complete final cells and the fixture authority state.
- Every occurrence's result, keyed by stable occurrence identity.
- The exact order and content of ordered and external observations, including
  held attempts.

Equal final state alone is insufficient: a read, decision, RNG draw or attempted
effect may differ even when final cells match. A mismatch returns an error with
the exact failing order, proposed representative, both computed results,
progress and the original full admission denominator. A partial population
cannot be returned as a successful `QuotientReport`.

The internal causal test deliberately corrupts a dependency matrix and checks
that this comparison detects a changed original decision even when final state
is unchanged. Public callers cannot supply that internal matrix.

## Admission, work and trace storage

`estimate` computes factorial schedule count and worst-case logical work and
retained payload before execution. `ReplayBudget` independently bounds schedule
count, replay instructions, canonicalization/relation checks, compared logical
entries and retained payload bytes. Every dimension must fit before replay.
At most 32 distinct cells may be present or touched; retained modeled payload
is capped at 64 MiB. No runtime dependency or source ceiling is increased.

The admission assumes the worst case in which every schedule has a separate
representative. Reports retain actual unreduced and representative replay
counts, successful comparisons and conservatively charged logical work.
These quantities are not CPU cycles, resident memory or latency measurements.
The payload bound includes fixed trace keys and complete worst-case logical
results; it excludes borrowed declarations, allocator/tree overhead, stack
footprint and process RSS.

`TraceSchedule::encode` emits `FAQTRC01`, a big-endian occurrence count and
big-endian occurrence IDs. Its size is `16 + 8 * occurrences` bytes. This is an
order-only trace: replay still needs the original program, registrations and
initial state. `stored_trace_bytes` and `unreduced_trace_bytes` compare these
order encodings, not self-contained archives or total memory.

## Authored controls and qualification

The fourteen public tests in
`crates/fa-reference/tests/quotient_replay.rs` cover complete populations,
independent recursive permutation enumeration, independently specified adjacent
swap reachability, real policy differences despite equal final state, positive
and negative reads, shared/cross-domain RNG and resources, authority and effect
barriers, witness-derived unknown dependencies, registration coverage, malformed
identities, all admission limits, retained failed operations and empty/singleton
populations. The private mismatch test exercises a deliberately unsound reduction.

Examples asserted by the tests are 120 schedules sharing one representative,
all 40,320 schedules at the eight-occurrence limit, and a mixed policy case with
six unreduced schedules and two representatives. These are authored expectations,
not measured results from this session. The timing fixture would print its
complete denominator, actual logical work and a single first-call duration;
it establishes no operating-system cold-cache or production latency claim.

Compilation and execution remain unverified. The prescribed remote-only RCH
gate could not start compilation because there were no configured workers and
the daemon was unavailable. The workspace executor subsequently disconnected.
No syntax, formatting, Rust test, benchmark or complete workspace pass is
claimed; the FA-093 bead and roadmap packet remain in progress. Even successful
bounded execution would not prove the independence of arbitrary production
callbacks or concurrent schedules.
