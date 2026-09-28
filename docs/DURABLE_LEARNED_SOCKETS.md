# External learned-sidecar helpers over the original durable protocol

`helpers::learned_sockets::FileLearnedSocketRound` connects the existing live
learned sidecar to provisioned Unix sockets and witnessed durable completion.
It serves plan 9.2 and 10.15: actual external workers receive the exact original
source-bound packet, and their original commits/reveals reach the existing
congress. This is not a new evaluator, reducer, journal format or authority.

`begin_learned_socket_round` consumes an acknowledged `FileLearnedSidecar` and
admits the exact member/socket inventory through `begin_helper_review`. There
is no source substitution, mutable planner, raw port, replacement socket or
caller-verdict interface. All existing member/input/salt limits still apply.
The original worker-custody marker prevents manual commits, reveals and sidecar
completion from taking over the active round, even after its transport is lost.

`pump_with_clock` performs at most one original nonblocking connection step per
member. It observes the independently supplied elapsed clock and checks the
current learned source before each I/O step AND before accepting the resulting
protocol event. A completed commitment passes its original canonical-storage
barrier before a reveal request can be sent. A disconnected or malformed member
remains missing. Commit/reveal deadlines are the original coordinator's rules.
No failed peer is replaced, silently retried, or turned into a synthetic vote.

The pump retains the original I/O results and worker statuses on backend failure,
plus a monotone count of attempted connection steps (not bytes, syscalls or
model work). A stale revision, foreign owner or initially old clock performs no
I/O and preserves the usable round. After admission, ordinary errors close its
transports. A caught unwind leaves a terminal failure marker and stack ownership
closes sockets and ports; recorded progress can then be incomplete. No retry
can rerun a half-completed operation through that owner.

`finish` requires an actually completed original round and invokes the existing
witnessed learned-sidecar completion transaction. A standalone round does NOT
buy an unassigned refinement. Its result is exposed only after acknowledgment
and remains readable through the existing durable outcome reader. Completion
is not authorization: original policy/input checks, mandatory human approval,
one-use dispatch and final publication revalidation still apply.

Cancellation/drop closes transports without cancelling an action, refunding an
unknown effect, undoing retained disclosure costs or issuing a publication key.
Reopening reconstructs original history and fences old sessions and keys; it
never resumes these process-local sockets. A failed journal write returns no
acknowledged result and cannot release a reveal signal based on that failed
commit. Already acknowledged earlier protocol events are not rolled back.

Seven new regression functions use the original history-sensitive decoder,
fitted codec, checked sidecar, HelperClient, actual Unix streams, canonical
files and two-key endpoint. Ballots are synthetic protocol controls, not a claim
about helper reasoning or detector accuracy. Cases include successful reviewed
publication, source advancement before I/O, stale/foreign calls, early finish,
an actual occupied-staging-file failure while accepting a commitment, missing
workers at deadline, cancellation/drop and incomplete socket inventories. One
compile-fail example denies a coordinator verdict-submission API.

Compilation, Rust tests, rustfmt and Clippy remain UNEXECUTED in this environment.
Required remote-only RCH attempts cannot start because rch is absent. Source/hash
checks are not execution evidence; no Bead, gate or production claim is closed.
Peer authentication, process isolation, current-clock provenance and child
shutdown/reaping remain supervisor responsibilities. Journal replay and fsync
are synchronous and not made latency-bounded by nonblocking socket calls.

## Fixed socket sequences and witnessed refinement

`begin_learned_socket_review` consumes the existing first sidecar and a complete
socket inventory for the original `LearnedWorkerSchedule`. Every round ID,
root, member roster, deadline, per-round limit and total poll allowance is fixed
before first Begin. All future sockets are made nonblocking before any evidence
is sent. At most the original 32 rounds and 65,536 polls can be selected. This
bounds retained socket/record counts, not kernel buffering or physical memory.

Each revision-checked advance invokes the same single-round owner above. If the
original completed result permits refinement and a provisioned future window
remains usable, the existing witnessed finish transaction buys one original
residual and invalidates old input approval. Only then does a fresh original
round acquire its own sockets over the richer input. No bytes are sent to this
next round in the same invocation. A terminal/missing result or the end of the
schedule does not purchase unassigned disclosure. Expired windows do not slide.
No inference or codec fitting is explicitly repeated by this orchestration;
original whole-journal replay retains its existing numerical reconstruction cost.

Acknowledged completion archives remain ordered in `history`, and the existing
durable outcome index remains their recovery consumer. Per-round records retain
attempted connection steps, original worker states, first wire failures and
completion status. Every scheduled round has a record, including unstarted ones.
These are logical/process-local work records, not authenticated helper-model
performance, persistent network accounting or independent scientific trials.

Source failure, storage failure, cancellation and total-poll exhaustion close
active AND all unused sockets without clearing earlier acknowledged results.
Caught unwinds also drop both inventories and latch failure; their last returned
progress may omit interrupted work. If refinement is acknowledged but the next
richer input fails its original per-round admission, that refinement and its
costs remain recorded. No earlier vote becomes permission for its new bytes.
Future IDs are checked at admission and before refinement, not globally leased;
concurrent use refuses rather than renaming or adopting a competing review.

Nine additional regressions cover actual socket-driven abstention/refinement and
two-key publication, preservation of both durable outcomes, no next-round bytes
in the transition call, disconnected-member missingness, retained first failures,
all-descriptor cancellation/poll exhaustion, full inventory admission, richer
input budget refusal after committed refinement, later source loss, clock
unwinding, single-round abstention and stale/foreign preflight with a successful
control. The first seven tests are unchanged. A second compile-fail example
denies socket replacement. All sixteen tests and both compile-fail examples
remain UNEXECUTED pending a successful fresh remote-only RCH gate.

Sockets can come from the existing authenticated/provisioned helper-process
workflow, but this adapter does not launch or reap children or add CLI commands.
It adds the durable review/refinement transport path those consumers can use,
not a new service, executor, cryptographic commitment or authority mechanism.
