# Durable policy replacement without recreating authority

Consumer: the trusted supervisor of FileDelivery or FileOversight changing the
active predicate program while retaining the same publication identity and
original rights ledger. This implements the existing policy replacement operation
across the plan's policy/dispatch serial boundary (§7.6–7.8, §8.3, §8.5). It adds no
new predicate evaluator, actor command, reviewer role, runtime or effect sink.

## Exact identity and original transition

PolicyUpdate binds an operation ID, expected control sequence, expected authority
epoch, and validated Policy. replace_policy also checks the journal predecessor.
The original controller requires a strictly newer policy generation, advances its
revocation floor, cancels every undispatched attempt and refunds only its
reservations. Even an otherwise identical policy cannot rebase old opaque
judgments or permits. The original lifetime 64-change limit remains in force.

FileOversight additionally withdraws pending/approved human keys and discards live
review sessions. Consumed keys stay consumed. Policy replacement, cancellation,
reservation refunds and human withdrawal are acknowledged together in one
canonical replacement. Governance requires no new helper evidence, current clock,
or human effect-approval key. Operator authentication and policy promotion remain
outside this trusted reference operation.

Replacement cannot widen the independently registered scope/resource/target
ceilings, mint resources, disable mandatory human review or the publication guard,
resume suspension, or clear terminal stopping. New permitted work requires a new
action under the advanced epoch, exact current evidence and the original congress
and key requirements.

## Dispatched effects and the optional first-publication guard

Already admitted dispatches are not rewritten, cancelled or refunded. Their exact
envelopes, deadlines, outstanding charges and endpoint history remain intact.
In the original unguarded execution profile they retain their dispatch-before-
policy-change ordering. Endpoint stop/fence is a separate revocation operation.

The concurrently implemented optional publication guard remains enabled across
updates. It independently revalidates current evidence at first execution and can
seal an old-policy request instead of executing it. Sealing still does not refund
until the original reconciler accepts the outcome. Historical execution receipts
always take precedence; new policy cannot relabel an executed effect unexecuted.

## Idempotence and recovery

An exact healthy-owner retry compares both predecessors and every policy node and
literal, returning its original PolicyUpdateReceipt without another event, epoch,
cancellation or refund. This works after later updates, reopening, missing time or
journal exhaustion. Conflicting operation reuse refuses. A new update needs the
current journal/control predecessors and a strictly newer policy generation.

Receipts are historical. current_policy returns the effective acknowledged policy,
not whichever receipt was last retrieved. Storage-faulted owners refuse this read
and retries because disk may be newer than their acknowledged in-memory state.

Opening still requires the ORIGINAL independently supplied bootstrap and exact
location. Replay invokes the original policy transition; it never deserializes
balances, permits or asserted cancellations as authority. Replacing bootstrap
with the latest policy is not recovery. The original recovery fence still
withdraws old permissions and requires fresh time for effects. An update absent
from the recovered canonical cut is not silently rebased onto the recovery epoch.

## Encoding and pending verification

Base journal tag 14 stores the complete bounded update. The full-input journal
explicitly admits that nested tag while still refusing one-key operations. It is
distinct from its outer PublishChecked tag 14. Old bootstrap and event bytes are
unchanged; old readers reject the new tag. Decoding bounds total literals/edges
before cloning and uses the original Policy constructor for semantic validation.

Six base-owner and four full-input-owner integration tests plus two codec tests
cover permitted fresh work, exact denials, invalidated old keys, earlier dispatches,
exact retries, stale/conflicting operations, retained actor identities, suspension,
resource ceilings, lifetime bounds, and all predicate forms. One compile-fail
example excludes interpreting an update receipt as endpoint evidence.

Rust compilation, formatting, Clippy and tests have NOT run. The required command
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check failed before
compilation with command-not-found (127). Source checks are not revision-bound
execution qualification. No original assertion, dependency, Beads status,
historical result or production gate is changed. The operator-controlled Unix
storage, anti-rollback and hostile-host limitations remain in force.
