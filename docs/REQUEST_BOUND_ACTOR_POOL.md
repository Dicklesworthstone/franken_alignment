# Request-bound multi-peer endpoints

## Source addition and change log

`FileActorPool::for_requests` binds each existing peer name to that exact request
ID for every future Submit. This serves the live multi-listener publication
consumer (FA-107; plan 9.3, 15, 17.1), using the original L5 ingress and L4 authority.
It is not a new principal, actor verb, credential, runtime or authority domain.
The unrestricted `FileActorPool::new` keeps its previous behavior.

The binding is checked inside the original shared scheduler before either source
preparation or recorded-request lookup. Thus an authenticated peer cannot submit
another endpoint's request, reacquire its ticket by exact retry, or consume a
preinstalled snapshot for it. Source-acquiring and observation-only drives share
this check, for both ordinary and generated ports. Reconnect retains the binding,
original ticket session, quotas and shared socket accounting.

The original inbox now creates a scheduling hint only after its sealed preparation
adapter accepts the scope/intent. A refused key cannot enqueue another request
just because that key happens to exist in the journal. Successful preparation is
still not admission: the queue subsequently checks the original durable status.
Recorded exact/conflicting requests still use the original final binding check.
No source result, response write or work hint can mint an effect permit.

Construction rejects pre-existing unrelated work hints and returns the original
inboxes intact. Existing tickets are not revoked by construction: integrations
requiring fresh endpoint custody must supply fresh inboxes, as the live consumer
will do. Poll/cancel remains governed by original ticket possession. Extracting
inboxes is an explicit trusted-owner handoff, not an actor operation or a durable
persistence of this ingress configuration.

## Validation status

Four `scoped_pool_` tests are authored with real Unix socket pairs, source files
and the original durable supervisor. They pair cross-key refusal with both matching
endpoints admitting; exercise cross-peer recorded retry/poll/cancel refusal versus
owner retry/cancellation; retain an unsent reply's ticket and full-width request
binding across reconnect; and preserve an incompatible pending hint on failed
construction while demonstrating unchanged unrestricted-pool behavior.

The targeted command was attempted in this session:

```
RCH_REQUIRE_REMOTE=1 rch exec -- cargo test --locked -p fa-reference scoped_pool_
```

It failed before compilation: `rch: command not found`, exit 127. The four tests,
compilation, rustfmt, Clippy and the complete gate remain UNEXECUTED. Source hash
checks are not execution evidence. No production qualification or Beads closure
is claimed. Existing historical receipts do not validate these changed files.
