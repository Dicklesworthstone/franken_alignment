# Guarded first publication in the durable supervised driver

Consumer: `FileSupervisedDriver` on an owner that enabled the original
`FileOversight::enable_publication_guard` before admitting any proposal.
This connects FILE_PUBLICATION_GUARD.md to FILE_SUPERVISED_DRIVER.md at the
existing FA-012/014/062 boundaries. It adds no new executor or authority ledger.

The driver's publication phase now uses the original guarded transaction instead
of the legacy `publish` method. For an unresolved, unexpired, live envelope it
calls the evidence provider once more, after dispatch, and samples trusted time
AFTER capture. The existing guarded transition rechecks the full helper inputs,
original policy witnesses and consumed human context, then atomically publishes
or seals nonexecution in the canonical journal. A missing or malformed provider
result is passed as unavailable evidence, not treated as a weaker publication
mode. The returned `PublicationChecked` diagnostic includes the original outcome,
its validation basis and any provider error. It carries no effect capability.

An internal read-only endpoint query avoids contacting providers for already
resolved, expired or unsendable obligations. It cannot grant permission and the
original guarded transition independently checks endpoint state again. Previous
execution/nonexecution wins over later source loss. Query-only recovery remains
provider-free, as does explicitly unguarded legacy publication.

The driver's send path is retired BEFORE the publication provider is invoked.
A caught provider unwind, rejected post-capture clock, capacity refusal or storage
failure leaves only reconciliation, never an automatic source or publication
retry. A source-induced nonexecution seal does not itself refund the charged
attempt; the original reconciliation transaction must acknowledge that receipt.
The original publication guard, reducers, journal event formats and legacy
behavior are unchanged. Only the driver's connection to them is added.

Nine new public test functions use the existing actor/driver fixture and actual
helper socket protocol. They cover successful guarded publication, six last-
moment source/policy failures, unrelated-state success, expiry before/during
capture, prior endpoint outcomes, provider unwind, clock rollback, failed storage
with reopen, and the unchanged legacy mode. These are source scenarios, not
trained-model evaluation, production qualification or executed test results.

Attempted in the editing container:

```text
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference --test file_supervised_publication
```

It failed before compilation with `rch: command not found` (127). No local
compiler fallback was used. Compilation, formatting, Clippy, tests and revision-
bound qualification remain unexecuted. No Beads packet or release gate is closed.

Clock correctness, source authentication and completeness, actor isolation,
plaintext trusted storage, cooperative locking, finite journal capacity and
anti-rollback limitations remain the original host assumptions. A synchronous
provider read or filesystem commit is not a bounded physical-latency operation.
