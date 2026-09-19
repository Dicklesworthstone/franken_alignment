# Durable monitored generation and interrupted-request recovery

## Implementation status

This extends the original `FileOversight`, hosted residual-monitor owner, stochastic sampler and bounded generation driver. It adds no alternative decoder, authority ledger, tokenizer, provider adapter or external publication path. The separate learned-KV generator is unchanged. This is development source: the new Rust, tests, doctests, formatting and Clippy have not been compiled or executed in this implementation environment, where RCH is unavailable. No Beads acceptance item or deployment gate is closed.

## Entry points

`FileGenerationCommand::new(id, actor_revision, position, request)` binds a nonzero caller-selected request ID to the exact `GenerationRequest`: original prompt IDs, maximum continuation length, ordered unique stop IDs and cumulative numerical/sampling budget. The decoder, monitor roster, model bytes, sampler policy, stream and seed remain those of the already installed `FileDecoderConfig`.

`FileOversight::generate_decoder(journal_revision, command)` now makes two canonical journal cuts for a new request. First it acknowledges `BeginGeneration`, reserving the command and its logical history capacity. Only then does it compute the new request through the original generation driver and hosted single-token methods. It acknowledges the complete result before returning a `FileGenerationReceipt`. An already recorded, exactly matching ID returns the historical receipt with no new inference or journal write, even while recovery-paused or with an old journal revision. A changed command under that ID is rejected.

`begin_decoder_generation` exposes the first cut separately for a trusted controller. It computes no new-request token, although ordinary journal replay can recompute earlier history. An exact repeat of that intent is a read-only acknowledgment of the same frozen command. `pending_decoder_generation` returns only this original input. `decoder_generation(id)` returns a completed historical receipt, `Incomplete` for the pending ID, or `Missing` for an absent ID. None of these observations resumes inference or authorizes effects.

For a new execution or continuation of a pending command, the current journal revision, a fresh trusted clock and an explicitly active decoder are required. Actor revision and numerical position are checked against the original owner. Initial intent admission checks room for two event slots; the original journal byte limits and configured recovery reserves continue to apply. Intervening supervisor operations can consume remaining storage, so this is not a guarantee that a later result write will fit.

## Preserve the request across crashes

At most one generation intent is outstanding. Its prompt, stop IDs, budgets, ID and numerical predecessor cannot be replaced by another request. The common durable admission gate blocks new proposals and other permitting transitions while it is pending. The existing single-token APIs and checkpoint capture/reset preparation also reject bypass attempts before new numerical computation. Historical result and checkpoint lookups remain observations, not new transitions.

Recovery fences old effect authority and pauses the decoder without clearing the intent. A fresh clock plus explicit `resume_decoder` permits only completion of the same pending command, not an alternate forced token or a new request. The original restrictive operations, manual suspension, evidence withdrawal and reconciliation of existing obligations remain available. Suspension can leave the unresolved intent retained permanently; there is no API to cancel it into a new draw, reset around it or refund its history reservation.

If the result write fails after computation, the acknowledged intent remains the older canonical state, unless the complete result replacement has become visible. The live object stays unavailable and returns neither speculative output nor an older quiet fallback. An interrupted request may require physically recomputing the same deterministic input after explicit recovery. This does **not** claim exactly-once CPU execution, account for all abandoned physical work, authenticate a runtime version or prove that no unrecorded alarm occurred. It prevents the controller API from silently changing the frozen request or sampling seed after such an interruption.

## Original computation and exact comparison

The durable recorder implements only the crate-private `GenerationOwner` composition point. It calls the existing driver and the same hosted forced/sample methods, including original consistency checks, actor/cache/sampler synchronization and automatic containment. There is no public callback able to assert that invented logits or reports were reviewed.

Each actual attempted token contributes its original result, query/residual captures and, for a released token, exact logits, loop counts and probe-interval review. Sampled results retain original sampling diagnostics in private journal comparison material. Held results retain their actual review but no released candidate. The final witness also contains the original numerical owner, actor-state projection, output report, work counters and stop evidence. The complete command is included, so even changing an unused budget or request ID cannot reuse another command's witness.

The growing full cache is not copied once per token into the transcript. Synchronous generation has no reset operation: its final original cache covers the appended K/V history, while per-token evidence covers intermediate logits and reviews. Existing single-step witness bytes are unchanged. Replay re-executes the original operations and compares the entire witness; it never imports stored cache bytes, approvals, logits or probe verdicts as live state.

Native admission refusals are recorded as `receipt.result() == Err(error)`. A successful `result()` returns a `GenerationReport`, whose finish may still be `Held`, `Failed`, `BudgetExhausted`, `StopToken` or `TokenLimit`. Invalid or underfunded complete prompts retain the original precomputation refusal behavior. A monitoring hold retains its computed draw and invokes the installed native stop policy. A comparison-buffer, encoding, cleanup or storage failure is an unacknowledged journal error instead of a fabricated native result.

## Canonical-file inspection

`read_decoder_generation(directory, profile, expected_decoder_config, id)` is a read-only recovery route, including while a failed writer still holds its lock. It pins the exact independently supplied model/configuration bytes before numerical replay, validates the entire canonical journal and returns `FileGenerationSnapshot`. `open_with_decoder` now also pins those bytes before executing historical tokens.

The snapshot distinguishes `FileGenerationState::Pending(command)` from `Recorded(receipt)` and includes the enclosing publication and numerical inspection. The enclosing image can be later than the requested result. Reading does not create a writer, clean temporary files, append a fence, acquire a fresh-clock claim, resume a decoder or issue human/effect keys. A visible result after a lost directory-sync acknowledgment is evidence about the currently readable canonical image, not a new claim that directory synchronization succeeded.

An absent ID means absent in this image, not proof that no computation happened. A pending command means no completed result in this image, not a benign verdict. File authenticity, independently retained rollback floors, runtime qualification and custody of supervisor data remain operator responsibilities.

## Bounds and compatibility

At most 128 generation records and 65,536 conservatively declared prompt-plus-continuation steps are retained. A pending intent consumes its reservation once; completion does not charge it again. Refused, held or early-stopped commands do not refund their declared history footprint. These are bounded logical history and admission counts, not measurements of physical CPU time, resident memory or network latency. The driver retains its existing 4,096-step request and 256-stop-ID limits, original context admission and lifetime monitor budget.

Existing numerical event tags 0 through 5 and their witnesses are unchanged. Tag 6 retains the first durable generation result format; tag 7 adds the write-ahead intent. Existing completed tag-6 histories can replay without a preceding intent for backward compatibility. New live generation preparation requires its exact pending intent, and an outstanding intent cannot be completed by a different tag-6 command. This compatibility is not authentication of a modified journal.

## Verification targets

The implementation adds 18 unit-test functions and one compile-fail authority-separation example. The first eight cover original-driver parity, receipt idempotency, conflicting IDs, codec truncation, early/final witness corruption, recovery pause/resume, native alarm containment, complete-prompt admission and bounded history. Ten additional tests cover intent persistence, single-step/proposal/reset bypass prevention, explicit recovery of the frozen request, all five original Store failure barriers at intent and result publication, read-only whole-image validation, nonoperative command substitutions, conservative capacity, old-format compatibility and event-slot admission. The result-write barrier test runs both quiet and alarm-producing configurations.

These tests are source coverage, not passing execution evidence. The required remote verification remains:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

The bounded in-memory residual contract is documented in `HOSTED_RESIDUAL_GENERATION.md`; this document adds the persistent request/receipt layer rather than changing the separate learned-KV publication semantics in `MONITORED_GENERATION.md`.
