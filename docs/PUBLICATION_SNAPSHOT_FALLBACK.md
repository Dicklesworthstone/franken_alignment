# Exact snapshot validation after change-feed retention loss

Plan 7.7 permits exact fallback or refusal when an invalidation index lacks a
complete tail. The native `enable_publication_snapshot_fallback` profile selects
the former for the existing snapshot-based witnesses. It is disabled by default,
requires a configured producer heartbeat lease, and can be enabled only before
any proposal. Existing strict profiles and packet encodings retain their meaning.

The fallback does not reset `through`, copy `observed_through` into it, synthesize
notifications, or label an incomplete history complete. Feed status and ordinary
feed freshness continue to report Incomplete. A separate internal acquisition
marker records that a valid producer identity/time observation occurred in the
current dispatcher epoch, despite missing history. Explicit withdrawal clears
that marker; recovery's original epoch fence makes it stale. Malformed, clockless,
future, expired, rolled-back or conflicting observations cannot activate it.

Only a source-bound, cut-bound input at the EXACT latest observed feed head can
use fallback. A legacy input, absent input cut, lagging snapshot, snapshot ahead
of the observed head, or unavailable index cannot. Original source generations,
exact same-generation contents and metadata, snapshot/control/semantic floors,
and one-capture-per-boundary consumption remain mandatory. Initial binding under
this profile still produces no fresh input or permission.

At authorization, dispatch and first publication the original PublicationJudgment
validates from scratch. No notification-derived skip mask is supplied. All captured
exact-value, absence, empty-range and membership dependencies and the entire
opaque helper input are checked under the original shared comparison budget.
Missing closing evidence, content/semantics drift and budget exhaustion refuse.
The existing policy, review, human, credential, target and deadline requirements
are independent. Fallback never edits a judgment or replenishes effect rights.

This is current-state validation, NOT reconstruction of a lost event history.
It applies only to the presently supported snapshot and whole-input witnesses.
It must not be reused for a future witness whose correctness requires every
intervening event. Producer completeness, identity authenticity, clock truth and
filesystem isolation remain host assumptions. A producer bundle supplies a
coherent local image but does not prove that all real-world changes were observed.

The first increment implements the native gate and eight lease/history tests,
including strict-mode controls, exact source/head matching, time boundaries,
generation conflict, withdrawal/recovery, real gap repair and overflow. Durable
configuration and actual file/driver integration follow as a separate increment.
Rust tests, compilation, formatting and Clippy are UNEXECUTED in this environment.
The required RCH xtask command exits 127 because rch is absent; cargo and rustc
are absent as well. FA-061/062 remain open pending exact-revision verification.
