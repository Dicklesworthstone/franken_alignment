# Bounded durable generation batches

## Positive capability

`FileOversight::advance_decoder_generation_batch(journal_revision, id,
generation_revision, max_steps)` advances an existing durable generation by a
bounded number of its original incremental transitions. The bound is between
1 and `MAX_FILE_GENERATION_BATCH_STEPS` (64). It provides a scheduling boundary
between one-token publication and an entire synchronous generation request.
A supervisor can use smaller batches for more cancellation opportunities or
larger batches to amortize canonical replacement and historical replay work.
This implements the grouped-durability direction of plan 18.3/18.6 over the
FA-025 original-token path; it is not a production gate or a benchmark result.

The batch executes one `Machine::replay` of the acknowledged prefix, then invokes
the original `prepare_decoder_generation_progress` for each admitted transition.
Every transition retains its original request/cursor binding, monitored token,
full state witness, work counters, stop handling and logical generation revision.
The first native termination stops the loop, including a hold, a natural stop,
an admission refusal or budget exhaustion without another token. The batch
never speculates beyond that result and never switches to a new request.

All new events remain ordinary `AdvanceGeneration` records with their existing
encoding. The canonical bytes at the batch endpoint equal those from successive
single-step operations at the same storage identity. No new event tag, cursor
format, finish variant, model implementation, dependency or authority is added.
Whole-request, single-step, cancellation and recovery APIs remain unchanged.

## Publication, retries and recovery

Exactly one invocation of the original `Store::replace` acknowledges a nonempty
batch. No intermediate result, token callback, effect receipt or canonical write
escapes. The live owner is made unavailable before new-request computation; a
failed witness, allocation, encoding or storage operation cannot restore its
older quiet state for live use. After storage acknowledgment, installing the
already prepared events, machine and progress requires no fallible operation.

A crash or injected storage fault leaves the old canonical image or the entire
batch image, not a canonically published first token of the batch. An ambiguous
replacement still returns no candidate output. Existing recovery replays the
ordinary records and appends its original fence; fresh time and explicit resume
are required before continuing. The cache, sampler, requested-history capacity,
monitor budgets and work remain those of the original reconstructed request.
Unacknowledged physical computation may repeat: batching does not provide
exactly-once CPU execution or a work refund.

Older generation revisions return the current retained progress without work;
future revisions refuse. New work requires both current revisions. `max_steps`
is a scheduling allowance, not a new semantic input or an idempotency key.
Changing it on an old-revision retry cannot compute additional tokens. Bounds
are checked even on terminal retries. Old completed requests cannot interfere
with a newer active request, clear a source interruption or refresh time.

## Resource and enforcement boundaries

Before the first transition, the original full-request event-slot check remains
in force. Later batches preflight their worst-case step slots, capped by the
remaining declared request transitions. A possible early stop does not waive
that check. These slots are not reserved against intervening supervisor work.
All original per-step numerical admission and full-prompt checks still run.

The original encoder validates every logical prefix against journal byte/event
limits, recovery reserves and retained-evidence budgets before the sole write.
A necessary aggregate bound also limits retained new witness bytes to the
journal byte ceiling, with at most one additional bounded witness transient on
refusal. This does not replace exact framing/reserve accounting. Late capacity
failure poisons the owner and publishes no smaller, opportunistic prefix.

The path still retains a full witness per original step and clones old event
metadata for final encoding. It reduces the number of historical replays and
physical replacements, NOT the final journal size. Temporary decoded state,
event metadata and encoding buffers still have costs. No throughput, memory-RSS
or constant-time result has been measured. Steps are synchronous and cannot be
preempted: cancellation and new clock observations occur between calls, not
inside a batch. An external watchdog remains necessary for hard time limits.

Only the existing numerical owner runs. Released tokens are supervisor evidence,
not permission to stream them to a user or dispatch a tool. Source interruption,
paused recovery, mandatory monitoring, held state and automatic-stop rules are
not bypassed. No input source or clock callback is invoked or renewed here.

## Regression and execution status

Eight `generation_batch_` tests are authored using the unchanged synthetic-weight
fixture and real journal files. They compare every original event and full
numerical state with the unchanged single-step path for multiple batch sizes;
exercise stale/terminal retries, source interruption, recovery pause, native
holds/stops/refusals, spent budgets, initial/later slot limits, middle-witness
corruption, all five Store failure barriers, and a late exact-byte-limit refusal.
The directory-sync fault must expose ALL three requested transitions; a naive
loop around single-step publication would expose only the first and fail.

The targeted RCH attempt failed BEFORE compilation: `rch: command not found`,
exit 127. Rust compilation, all tests, rustfmt, Clippy and the complete repository
gate remain UNEXECUTED. Source/blob and whitespace checks are not execution
qualification; synthetic weights and Store fault seams do not establish trained
model behavior, physical power-loss durability or production deployment safety.

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference generation_batch_
```
