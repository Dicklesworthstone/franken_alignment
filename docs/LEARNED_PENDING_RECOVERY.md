# Learned pending-step recovery and admission

Status: unqualified source, advancing FA-014 without closing its full packet.
This serves the existing effect-ledger and replay contracts in plan sections 8,
11 and 16. It changes no founding semantics, dependency or journal format.

## Ordinary capacity is distinct from terminal recovery space

A new learned inference intent now requires two ordinary event slots before it
can be persisted. The old physical-limit check counted the reserved terminal
Fence/Stop/StopProgress tail, although the original encoder correctly refused
using that tail for a numerical completion. With only one ordinary slot left,
that admitted an intent that the next completion could never append.

Completion also checks its one ordinary slot before replay or new numerical
work. An intervening transaction can still consume previously available space;
this check is not an escrow or a promise of eventual completion. The durable
intent remains a barrier when completion refuses. No older quiet prefix becomes
eligible, and the terminal recovery reserve is not enlarged or spent.

The same check applies to paired learned checkpoint reset intents and outcomes.
A new reset requires two ordinary slots and its completion requires one before
any new audit or restoration work. Exact retries of an already acknowledged
reset retain the original historical-result path and need no new slots, even
at exhausted capacity or with the original stale revision. This does not make
an interrupted reset resumable or upgrade its frozen authority predecessor.

These checks cover event slots, not future witness bytes, physical disk space,
future restart attempts or hardware durability. Original canonical prefix byte
limits, independent recipe matching, numerical algorithms, authority fencing,
mandatory automatic/human keys and storage-failure poisoning remain unchanged.

## Atomic completion of a recovered pending step

After independently configured learned recovery has durably fenced old effect
keys, `FileOversight::resume_pending_learned_step_at` accepts the exact journal
revision, pending actor revision and position, plus a fresh trusted elapsed tick.
It requires a paused owner and an existing matching intent. It cannot create a
new intent, abandon one, import numerical state or resume a held/failed run.

The method applies the original Time and Resume transitions, then computes the
original Step and exact witness. All three original records become visible in
one canonical replacement, and the returned revision advances by three. The
original history is reconstructed once rather than once per public operation.
There is no new wire tag, public event importer, helper vote, approval, external
send or alternate numerical engine. Storage and numerical work are synchronous;
no measured speedup, preemption or constant-time recovery is claimed.

Stale or mismatched requests, refused clocks/resumes, source interruption and
insufficient ordinary event capacity leave the acknowledged state and journal
unchanged. The original byte encoder checks every prefix before reconstruction;
a final witness can still exceed the remaining bytes after inference. Once new
computation starts, a failed witness or storage replacement poisons the owner.
No partial Time/Resume, candidate token or candidate result is returned. Reopen
must determine whether the original pending intent or the complete new outcome
is actually durable. A lost directory-sync acknowledgment does not mean the
replacement failed to become visible.

An outer journal error is distinct from a durably acknowledged inner numerical
error. Alarm samples remain withheld, completed costs are retained, old effect
keys remain withdrawn and unknown-effect charges remain outstanding. The method
does not refill budgets, reset authority epochs or clear interrupted-source
latches. It uses the existing terminal reserve and original publication guards.

## Verification and change record

Six admission regression functions pair one-slot refusal with exactly-two-slot
original inference or checkpoint reset, both with and without a terminal reserve.
They also exercise intervening clock work, retained pending state, unchanged
canonical bytes, stale/poisoned owners and historical reset retries at exhausted
capacity without repeated audits or incidents.

Eight atomic-recovery regression functions compare the three original public
operations and their complete encoded records against atomic recovery; continued
sampling; stale/invalid requests; exact/one-less ordinary capacity; all five
original storage fault barriers and actual old-or-new reopening; held samples;
recorded numerical failures; and charged unknown effects with withdrawn keys.
They use the existing original-model fixture and real journal files; synthetic
probe parameters are not detector-quality evidence.

Required targeted and full RCH commands were attempted and failed before
compilation because `rch` is absent (exit 127). Rust compilation, test execution,
rustfmt and Clippy are UNEXECUTED. The unchanged original learned event and reset
intent codecs were compared byte-for-byte in the preparation environment; that
is not runtime qualification. No historical receipt qualifies these changes.
Beads remain unchanged because their required `br` CLI is unavailable; no gate
is promoted.
