# Durable credibility in the original delivery owner

FA-105 increment; plan sections 9.9, 8.3 and 16.4; FI-A08/FI-A13/FI-A18.
The L8 evidence produces L3 weights consumed by the existing L4 authority and
L5 delivery endpoint. No alternate rights ledger, runtime or actor command exists.

`FileDelivery::activate_credibility` and `withdraw_credibility` commit the original
native transitions through the same locked canonical journal as publication.
The candidate applies the new dispatcher fence to its RAM-only endpoint before
one canonical replacement publishes the whole state. Old sendable handles are
discarded; original delivery records and terminal receipts remain queryable.
Qualification, cancellation refunds and fence acknowledgment are returned only
after that replacement succeeds. An ambiguous I/O failure latches the owner.

New base-event tags 16 and 17 leave every existing bootstrap and event encoding
unchanged. The activation payload has domain `FACRED\0\x01`: exact operation and
predecessor, scope, policy, actor profile, deployment binding, requirements and
original sealed campaign inputs. No score, admitted weight, receipt, balance or
freshness deadline is trusted from disk. The decoder replays the original
`CredibilityLedger` and native activation derives the weights again. Missing
reports, explicit Missing/Abstain, pending labels and censoring stay distinct.
Every manifest case is retained, including nonconsecutive and extreme IDs.

Decoding bounds each string, collection and case/helper product before retaining
case data. A request is capped at 4 MiB and 65,536 logical observations; the
encoder and decoder also enforce the native 64-activation / 65,536-observation
lifetime bound. One decoded request may exist transiently while checking the
aggregate bound. This is logical resource accounting, not measured heap usage.
Credibility changes are ordinary work and cannot spend reserved recovery space.

Reopening still runs the original authority/dispatcher fence and discards old
keys before returning an owner. Saved elapsed time is not current. Qualification
retains its original control-sequence age; old operation retries neither append
nor reinstall policy, including after withdrawal or a newer activation. Only
original endpoint receipts settle charged effects. Mere age expiry does not
retroactively revoke an already dispatched independent envelope; explicit
activation/withdrawal installs the new endpoint fence. Read-only journal inspection
never replays an external effect. Protected-directory integrity, independent
labels and the mapping into the control-sequence domain remain operator trust
assumptions; this is not rollback-resistant authenticated storage.

Fourteen tests are authored: ten durable/native compositions and four codec
boundary tests. They cover real file publication and reopening, before/after
publication withdrawal, unknown charges, exact retries and conflicts, refresh,
expiry through real reviews, all five replacement barriers, recovery reservation,
duplicate imported frames, missing/censored data, forged label fields, all
truncated activation prefixes and exact/one-over product and string limits.
Compilation, Rust tests, formatting and Clippy remain UNEXECUTED: the required
`RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check` cannot
start because `rch`, `cargo`, `rustc` and `rustfmt` are absent. Source identity and
lexical checks are not substitutes. FA-105 remains open. FileOversight integration
and supervisor command exposure are separate increments; no production activation
or evaluator-authentication claim follows.
