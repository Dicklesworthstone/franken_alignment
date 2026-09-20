# Submitting subsequent publications to a retained journal

The Unix `supervise_publication` reference example now has three distinct paths:
`create` initializes a store, `resume` only reconciles a previously recorded request,
and `submit` admits a new request against the existing store. There is no
create-on-missing fallback. This is synchronous operator orchestration over the
original owner, not a production daemon or a new actor capability.

After a first successful publication, construct the next frozen actor document:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  proposal-next supervisor.json 2 next-payload.bin 15000 > next-submit.json
```

The request ID must be nonzero and unused for new work. TTL is in milliseconds
and must fit the configured one-shot runtime. `proposal-next` reads the current
target/version and retained policy epoch without opening or modifying the store.
It anticipates the owner's next recovery fence (`retained epoch + 1`), rather
than using the bootstrap epoch zero. Construction does not reserve authority or
guarantee admission: another owner opening the journal or changing its state can
make this document stale. Submit promptly; the command never rewrites actor bytes
to make a stale document pass.

For the legacy reference publication profile:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  submit supervisor.json next-submit.json
```

For a store initialized with checked publication witnesses:

```sh
cargo run --locked -p fa-reference --example supervise_publication -- \
  submit-checked supervisor.json next-submit.json witness-profile.json
```

Both submit commands accept `--reviewer-profile reviewer.json` for the existing
independent peer-reviewer path. The same configured helper and human approval
requirements as `create` still apply. The separate reviewer runs `review CONFIG
REQUEST_ID` or `review-peer REVIEWER_JSON REQUEST_ID` as appropriate. Checked
stores require their matching explicit witness profile; a failure never retries
legacy mode. Fresh input capture must name the new request/action and its current
policy epoch; do not reuse an earlier action's approvals or capture identity.

Opening an existing journal deliberately performs the original owner's recovery
fence. It retains spent rights, endpoint state, request identities and history;
it does not refill the configured budget. It may fence/cancel unfinished old
work, so this is not concurrent multi-owner admission. After an interrupted or
uncertain attempt, keep the exact original document and use `resume` (or
`resume-checked`) to reconcile it. An exact recorded retry through `submit` is
also query/reconciliation-only, even after its original deadline: no helper
relaunch, evidence refresh, new human offer or repeated publication. Changed
bytes under the same request ID conflict. A stored refusal is not a new request.

An owner open can advance the epoch even when subsequent admission fails. For
truly new work, take another read-only proposal after reconciliation and use a
new request ID. Never treat unknown publication status as permission to resend
or erase the journal to recover budget. Actor-visible results still come only
from the original gateway; historical `inspect` output is not fresh permission.

## Validation status

Regression tests cover sequential retained-budget publications, exact expired
retries and conflicts, stale epochs, absent stores, profile substitution,
read-only proposal construction, binary payloads and input/arithmetic bounds.
They are authored, not a claim of executed validation. Rust compilation,
formatting, Clippy and tests were unavailable in the implementation environment
because cargo/rustc/rustfmt/RCH were absent. Run the required repository gate in
a configured environment before relying on these changes:

```sh
RCH_REQUIRE_REMOTE=1 rch exec -- cargo run --locked -p xtask -- check
```

Synthetic helper tests do not establish model accuracy, reviewer authentication,
production suitability or closure of any larger readiness Bead.
