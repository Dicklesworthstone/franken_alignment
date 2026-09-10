# Two-key human authorization at the effect boundary

## Consumer and status

`OversightBroker` now supports the two-key requirement in plan section 9.11.
The consumer is its existing dispatcher: with this profile enabled, no sendable
publication envelope is returned unless both the automatic effect permit and
the separately held reviewer's exact human key pass their checks.

This is safe, std-only reference code in `oversight/broker/human.rs`, with no
additional Cargo dependencies. The separate role models a human reviewer; it
is not a login, signature, remote approver service or proof that a person read
the evidence. The embedding operator must authenticate and isolate that role.
No real human member was added to a deployed congress by this implementation.

Compilation, formatting, Clippy, Rust tests and the repository gate have not
been executed in this session: no configured Rust/RCH runner was available.
Existing receipts cover different source. Beads and production qualification
remain open. The new tests are source, not passing execution evidence.

## Mandatory mode, not a permissive wrapper

`enable_human_review` freezes the reviewer identity, maximum key lifetime and
request cap once, before any proposal or control transition. In enabled mode,
the original one-key `dispatch` entry point refuses. There is no disable API,
reviewer getter or actor-selectable effect threshold. Unconfigured brokers retain
their previous one-key behavior; this is an explicit bootstrap profile, not a
claim that every existing deployment now requires a person.

The profile covers all effects of the broker's existing single-resource scope.
Per-family selection, multiple human signatories, reviewer replacement and
cross-process credential provisioning remain separate work.

## Bound request and independent key

A request can be created only for an undispatched congress-approved action with
matching current helper inputs. It captures the actual action and complete
committee input basis from the owning broker, not a replacement action supplied
after voting. It also binds the input revision, current control sequence,
revocation epoch, exact-policy generation, reviewer identity and expiry.

The reviewer receives immutable `HumanRequest` evidence. `HumanReviewer::approve`
issues a non-cloneable `HumanPermit`; reject and revoke are terminal for that
request. The broker cannot recover the reviewer role from a request or key.
`human_request` retrieves the retained evidence for inspection, not a lost key.
A numeric reviewer/request identifier alone is never authority: matching uses
private process-local issuer and request identities.

One request is admitted per action/input-revision/control-sequence/epoch context.
A different request number cannot silently reroll a rejection, withdrawal or
lost key in that same context. A new independently applied congress review can
create a new context, but it still needs an explicit new human approval. This
profile conservatively invalidates keys on any control-sequence change, even
an unrelated review; it does not claim optimal concurrency or review reuse.

## Dispatch and refusal behavior

`dispatch_with_human` checks the complete current input approval, exact action,
request identity and status, input revision, control sequence, policy generation,
revocation epoch and human deadline. It then invokes the existing delivery
broker, which still enforces the automatic permit's issuer, witnesses, exact
policy, expiry, target, byte budget, endpoint fence and one-use state.

The human key is marked Consumed only after that dispatch succeeds. No caller
callback runs between the checks and publication of the model transition, and
an exclusive borrow orders reviewer revocation against dispatch. A refused
operation leaves both keys unspent unless they were already spent; it does not
restore a revoked or expired key. Shared clock observations may advance on a
refusal, but effect accounting cannot be repaired by rolling them back.

The clocks are supplied in one declared monotone logical domain. The broker must
independently observe at least the key's issuance time; a future approval cannot
advance its clock. A key expires at `now >= expires_at`. Its expiry may not
exceed either the frozen action deadline or the bootstrap maximum lifetime.
`HumanStatus::Approved` records issuance, not a currentness certificate; every
use still checks expiry and the bound context.

Human authorization does not reclassify missing capture or override an exact
policy failure. Nor does it make the effect safe: the second key is an additional
authorization prerequisite, not a factual judgment about the world.

## Withdrawal, reset and interrupted delivery

`revoke_all` withdraws all currently Pending or Approved requests in one model
step and reports their identities. Repeating it is idempotent for unchanged
state. Rejected, revoked and consumed records keep their real dispositions.
This does not suspend future reviewer issuance or cancel already sent messages.

Policy replacement, credibility-driven congress changes and actor reset cannot
restore an old key: the existing control/epoch fencing still applies. Input
outage and identical recapture also require fresh independent and human review.
Reset never disables the configured second-key requirement or rewinds human
request history. Human rejection/revocation does not itself refund an automatic
reservation; the owner can cancel undispatched work through the existing gate.

A dispatcher restart preserves an unspent key while requiring its ordinary
endpoint fence acknowledgment. Once dispatch returned an envelope, the human
key remains Consumed even if acknowledgment is lost or the endpoint later
establishes nonexecution. Revocation does not undo a sent effect. Reconciliation,
nonexecution sealing, cancellation and endpoint receipts do not depend on human
availability or remaining human request capacity. A legitimate refund never
makes the old human key spendable again.

## Bounds, evidence and verification scope

At most 256 requests are admitted, further bounded by the configured cap. Each
retained complete input basis is charged against an 8 MiB lifetime logical-byte
limit before insertion. Shared identical bases are conservatively charged again
per request; these figures are neither allocated bytes nor peak memory. Request
identities and terminal states are not recycled. Capacity exhaustion blocks new
human requests, not settlement of existing obligations.

The tests include eight direct two-key cases and eleven recovery/lifecycle cases,
plus two compile-fail type controls. They pair successful actual reference
publication with one-key bypass, wrong role/key, stale evidence, altered action,
revocation, expiry, future/rolled-back clocks, identity substitution, forgotten
inputs, reset, policy replacement, both dispatcher recovery paths, request and
byte caps, and retained reconciliation. They have not been executed.

Human request/status/withdrawal records are in-memory reference data. The older
DecisionArchive format verifies its own policy/congress contract, not this new
human authorization. No signed co-authorization receipt, authenticated human
interface, durable two-key transaction, OS-crash atomicity, statistically measured
human performance or production adapter qualification is claimed. Production
must include both consumptions in its authoritative durability boundary rather
than relying on these in-memory model steps.
