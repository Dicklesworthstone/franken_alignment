# Atomic terminal recovery for the full-input, two-key host

## Consumer and scope

The privileged `FileOversight` owner can now stop/drain a live publication or
recover a crashed publication directly into a stopped state. These operations
serve the existing L4/L5 authority/effect boundaries (plan sections 8.3, 8.4,
8.11 and 16.4; FA-006 recovery ordering). They compose the existing transitions,
not a new authority, effect adapter, journal format or generic transaction API.
The simpler `FileDelivery` equivalents remain unchanged.

`FileOversight::stop_and_drain(revision, request, observed_tick)` appends the
original Stop and StopProgress records in one canonical replacement.
`FileOversight::open_stopped(directory, profile, request, observed_tick)` acquires
exclusive ownership and appends Stop, Fence and StopProgress in one replacement.
It returns the stopped owner and original sweep, **not a new reviewer role**.

## Contract

Stop preconditions bind the original recovered control sequence and authority
epoch, before the fence changes them. An exact already-recorded stop is reused;
a conflicting operation is rejected. Every successful reopening advances the
current dispatcher fence and creates a new process-local owner identity.
Old automatic, human and reviewer handles cannot authorize new work.

Every transition still passes the original source/decoder/consistency gates and
canonical event/byte admission, including each intermediate encoded prefix.
The original stop withdraws native sessions and keys. The original endpoint
supplies all settlement evidence. An unpublished dispatch can be sealed; an
executed effect stays charged; expired retention remains unresolved and charged.
Nothing is resent, no missing observation becomes a receipt, and successful
startup is not a promise that all liabilities were resolvable.

An installed `RecoveryReserve::terminal()` supplies three events and fifty
logical encoded bytes. Terminal operations need no ordinary Time or Sweep record,
so ordinary-capacity exhaustion does not consume their tail. Bootstrap and prior
recovery records remain counted. The reserve is finite, does not replenish, and
is not reserved disk capacity. Previously full unreserved journals are not
silently enlarged or compacted.

Both APIs require an independently supplied current tick in the profile clock
domain. Saved time is history. Validation/capacity errors do not change the
canonical history, including its admission state. Use the existing separate
`request_stop` when immediate local stopping cannot depend on a clock sample.
I/O failure returns no candidate sweep/refund and poisons the live owner before
replacement, including caught unwinds. Inspection may then lag the file.
Exclusive recovery reads the actual old-or-complete canonical image.

Recovery retains stored optional guards and source floors. It needs no current
helper, evidence file or human approval to stop. It neither authenticates storage
nor detects an operator restoring an entire old journal without an independent
floor. It is not a process kill, distributed simultaneous-stop guarantee,
remote-provider transaction or a production runtime qualification.

## Status and change log

2026-09-20 UTC: added both source implementations and eleven Rust regression
tests. The tests use real canonical files and the original complete-input,
commit/reveal, human-review and endpoint reducers. Their helper verdicts are
synthetic fixtures, not native inference evidence. Coverage includes reserved,
sent and executed work; event and byte neighbors; ordinary-tail exhaustion;
stale preconditions; all five storage barriers; lost acknowledgments; expired
liabilities; old keys; exclusive ownership; stop identity; and guard retention.
The source-interruption case injects the native latch and is not a producer I/O
test. An unchanged permitting two-key control accompanies validation negatives.

Required verification command:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

This environment's attempt exited 127 (`rch: command not found`); Rust, Cargo and
rustfmt are unavailable. Compilation, formatting, Clippy and Rust tests remain
**unexecuted**. Local lexical/delimiter and whitespace screens are not substitutes.
No dependency, production activation, historical gate count or Bead closure is
changed by this work.
