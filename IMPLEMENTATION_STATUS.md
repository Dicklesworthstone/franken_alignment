# Implementation status · design revision 0.3

## 2026-10-25: validate charged client requests and the stream finish slot

This continuation changes reference tests, fixtures and contract wording; it
activates no production runtime behavior. The original P0 remains in progress.
Six learned-stream fixtures now admit seven messages within the existing eight
delivery slots, preserving the original separate finish slot. Client fixtures
now declare the full 9,000-byte or 25-byte payload charge; historical-investigation
fixtures likewise declare their actual payload cost. No production ceiling,
clock, timeout, wire format or source/authority check is relaxed.

The generated-source peer and named-listener cases now require the original
`Denied` judgment, exact control-ledger/request/generation basis and retained
request stage, rather than misclassifying a complete false policy as a
proposal-construction refusal. The native policy reducer and request book are
unchanged. Existing exact retry, producer-file repair, private-context redaction,
no-source-read and no-effect assertions remain.

At frozen `93660bf54711efca2565d7d36c88ce21fb569b16` (tree
`6963ffe370a06f6073ec136b4958db665067102d`), standard workspace/all-target
compilation and all 359 workspace doc tests pass. The public `stream_release`
target passes all nine tests, including two real reference message publications
and finish within exactly three deliveries, beside rejection at two deliveries.
These reference ballots and memory endpoints do not qualify a real actor model
or deployment. The complete 2,325-function library run finishes **2,244 passed/81 failed**,
zero ignored/filtered, in 1,352.85 seconds. Every prior 2,324 ID remains: **32
previously failing IDs now pass and zero originally passing ID newly fails**.
The added undercharged-client negative also passes with unchanged canonical
bytes, live state and revision and zero peer I/O. All seven batched-client,
five peer-client, thirteen generated-source API and seven named-listener
functions pass. Ten of thirteen anchored-investigation functions pass; three
retain their original failure on a real staging-file collision during a later
writer operation. Their guards and assertions remain unchanged. Overlapping
selections are not added to the full inventory as extra evidence.

The first dispatch used an unresolvable revision and stopped before compilation.
The first new library build at `089eda23` caught two `E0599` calls in the new
regression: read-only `FileClientInspection` has no recovery encoder. The explicit
correction compares actual canonical bytes and uses its existing inspection
fields. Both failed attempts and the corrective commit remain retained; they are
not test passes. Fifteen continuation Rust paths pass qualified formatting.
The original plan, license, dependency policy and Bead acceptance remain intact.

Raw continuation logs and normalized source diffs are retained outside frozen
source under the existing operator evidence directory:
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-stop-76bf4d57/`.
Current Clippy again fails with 25 library errors (14 argument counts, five
large enum variants, six large error results), before later Clippy targets.
The canonical command actually executes and refuses the unchanged initial
admission guard. All 81 remaining library failures, original Bead acceptance,
full dependency/source admission and the separate older workspace/operator
failures remain open. This is self-reexecution of bounded reference evidence,
not independent certification or a complete project gate.

## 2026-10-25: propagate intake journal faults and reduce reference lint debt

The reference inbox now propagates a newly raised source-preparation journal
fault to the existing peer pool. The pool stops before another peer and charges
the complete failed allowance; the original inbox can still drain its queued
redacted refusal. The real collision negative, matched two-peer success and
queued-reply regression all pass.

At frozen `66b2ae6a1b3ed2177ea8e1ea21c8a41398d22e90` (tree
`f6287cb90d28c942edd97c635e7d6a1a3b4c3552`), the complete reference library
finished **2,211 passed/113 failed**, zero ignored/filtered, out of **2,324**, in
807.28 seconds. The original 2,323 IDs are all retained: **73 original failed IDs
now pass and zero originally passing ID newly fails**; one new matched positive
also passes. These are reference-library outcomes, not a complete workspace or
production pass. All-target compilation, all 359 workspace doc tests and
formatting of all 58 owned Rust paths pass at this same source. Clippy still
fails with 25 library diagnostics and the canonical driver still refuses the
unchanged initial dependency-admission guard. Self-reexecution is not independent
certification, and no Bead, roadmap packet or production gate is closed.

The original P0 workspace-qualification Bead remains **in progress**. The full
runtime command at `2f540485` exited 101 with **36 failed Cargo targets**. Its
reference library finished **2,137 passed/186 failed, zero ignored/filtered, out
of 2,323**. The complete pre-cleanup `24062cbf` library control has identical
counts and the same 186 failed IDs; this compares library outcomes, not all
workspace targets or independent certification. The 744-function operator
suite finished **742 passed/two failed**, zero ignored/filtered, in 1,621.69
seconds. The learned checked-publication resident sequence and native
continuation after second dispatch remain unresolved after peer timeouts or
BrokenPipe. No deadline or peer budget was widened. The separate xtask target
passed 248 tests; filtered child helper invocations are not added to parent
inventories, and no whole-workspace test count is inferred from interleaved logs.

At frozen
`2f54048572d9d0af36092985e488754c5929c1cc` (tree
`46e3101f49d21e2f345510bcf0d739ff4323bb54`), all 48 changed Rust files pass
qualified formatting and the standard workspace/all-target compile check passes.
Clippy with warnings denied still **fails** with 25 library errors: 14 argument
counts, five large public enum variants and six large error results. This removes
55 of the original 80 library diagnostics. Later Clippy targets remain unrun;
the passing compile check retained their warnings rather than suppressing them.

The source changes preserve report bytes, fixed-width codecs, binding/length
checks, lazy clock calls and short-circuit delivery/recovery checks. Three
private receipt-transition variants use a short-lived box; feed ingestion reuses
the existing capture `Rc` earlier. Public APIs, journal/event formats, authority
checks and all runtime test behavior are unchanged. The receipt boxes also
allocate during replay; no allocation, latency, stack-limit or RSS improvement
has been measured. This is build-enabling reference work, not a new production
activation or an alignment result.

The canonical driver again exits 1 at the unchanged initial two-package lockfile
guard. Full dependency admission remains required before admitting `fa-journal`
there; no guard, package policy, snapshot, golden, compiler/backend, timeout,
production bound or assertion was relaxed. Six operative README compiler pins
now match the already-qualified `nightly-2026-09-08` registry.

The complete workspace doc gate at `2f540485` exposed two private-field
forgery examples whose empty literals produced an uncoded privacy diagnostic,
so their original `E0451` requirements failed. `ff1f90943195a3a87eab65eddb50ee14dea285f4`
(tree `b5cb7d449866766a47b9f2d701b45333c902392f`) keeps both `E0451`
requirements and supplies complete private-field literals attempting to replace
accepted revision or expiry, beside permitted clone/accessor examples. Dropping
only doc comments from qualified-formatted old/new source leaves identical
runtime text in both files. These positives are compile-only API checks, not
runtime evaluator proof.

At `ff1f9094`, all-target compilation and the full workspace doc gate pass:
13 journal, three merged reference examples and 343 separate reference tests;
359 passed, zero failed/ignored/filtered. The earlier baseline was 357 tests,
355 passed/two failed; its separate one-positive merged harness is included
here, correcting preliminary per-crate shorthand. The failed full runtime
workspace command remains attributed to `2f540485`, not relabeled as
`ff1f9094`. The four older intermittent native investigations remain open,
alongside the native-continuation failure observed in the new full run.

A separately reviewed inherited oracle repair at
`4281d2022dfe2007ad22e43c66f040183d0a80a9` keeps production version-six bytes
unchanged. The manually authored wrapper oracle used 64-bit words where the
original shared journal uses 32-bit blob/count words. Exact equality remains;
precision/cap offsets now target those fields, and widening either/both words
is explicitly rejected. The same three parent configuration functions changed
from baseline one passed/two failed to three passed/zero failed (2,320 filtered,
zero ignored). No generated snapshot/golden file was refreshed.

The repository DSR fragment at `054a7ce5` also pins its three Cargo checks to the
existing qualified compiler, preserving all four checks. Concordance and system
map attempts still fail on unlisted files/digest mismatches in the old source
snapshot; the canonical command still refuses lockfile admission. No global DSR
registry, production profile or manifest refresh was installed. Current source
requires the original admission/source-closure work before a complete gate can
pass.

The inherited hosted-likelihood test failed at an assertion that proposal
recording must refuse on crossing. Stronger diagnostics verified the actual
positive probe band, configured pair, consumed prediction, true payload event,
exact alpha and crossing. The original contract gates permitting review and
authorization instead. At `45829a6c` (tree
`bb5125c405685e80d6105361e16b6cadb23789c6`), the same parent function passes:
a crossing records the proposal, refuses an actual unanimous Allow review with
`Incomplete`, refuses authorization without a journal write and executes zero;
the matched quiet control obtains both keys and publishes once. Exact likelihood
replay/reopen assertions remain. This is a separately reviewed test-contract
repair, not a new production enforcement mechanism.

Three shared test fixtures are corrected at
`26ddac0862f83e4b8e98e5c9cc39c5d479659e22` (tree
`1c1afd90d6b736e6dac2673152ba80ceab5abcdb`): the native toy model now uses
4,096 context tokens, within the unchanged 4,096 hard cap, and two actor-source
policies explicitly conjoin both original leaves. The original 8,192-token
fixture and unreachable policy nodes failed before their intended boundaries.
All 55 owned Rust paths pass qualified formatting and match this frozen source.
The same 112 affected parent functions improved from baseline 24 passed/88
failed to 84 passed/28 failed, zero ignored and 2,211 filtered, in 212.27 seconds;
60 original failures pass and no original passing ID newly fails. The production
limits, policy graph rules, formats and authority remain unchanged. This selected
result is separate from the later full current run.

At `ca4ab945`, an actual ingress propagation gap is repaired: failed source
preparation can queue a redacted actor refusal without adding a ready hint. The
inbox now returns the original newly raised journal I/O fault to its coordinator,
so the existing pool stops before another peer and charges the whole issued
allowance. Subsequent draining of that queued reply from the same faulted inbox
remains possible. The original collision, second-peer, budget and canonical-byte
assertions pass, including an added exact `Stage`/`AlreadyExists` check; a new
matched no-collision positive admits both peers and charges actual socket work.
The existing generated-inbox storage test also passes its unavailable-reply drain
and prior-hint checks. The selected run finished 28 passed/seven failed, zero
ignored, 2,289 filtered out of the new 2,324-parent inventory. All seven failures
still occurred at named Unix socket bind; they are not credited as passes.

Shortening the shared data-directory spelling was insufficient for `SUN_LEN`.
`66b2ae6a1b3ed2177ea8e1ea21c8a41398d22e90` (tree
`f6287cb90d28c942edd97c635e7d6a1a3b4c3552`) restores that spelling and gives
only the seven named-socket tests their own short, atomically created private
0700 directory under `/tmp`. Source evidence and journals keep `TMPDIR`; kernel
credentials, framing, quotas, actual two-key publication and every original
assertion remain. At that current source, all 58 owned Rust paths pass qualified formatting and
match the frozen tree; standard workspace/all-target compilation and all 359
workspace doc tests pass. Clippy still fails with exactly the same 25 library
diagnostics; later Clippy targets remain unreached. The full 2,324-parent library
result is reported above, including the new matched positive test. Within it,
the fixed inbox/pool/socket selection is 34 passed/one failed out of 35, with no
ignored test. Six named listener functions now pass their original checks,
including actual two-key publication. The remaining named backpressure test
reaches its original `ActorOutcome::NotAdmitted` assertion and still fails;
that assertion was not weakened. All 113 remaining library failures stay open.
The complete workspace/operator inventory has not been re-executed at this later
source; its older failures remain separately attributed.

The preceding test-only `34013f95` full library control finished 2,202 passed/121
failed, zero ignored/filtered, in 754.30 seconds. Its exact 2,323 IDs match the
original `24062cbf` baseline: 65 original failed IDs now pass and zero passing ID
newly fails. It does not qualify the later inbox runtime change or socket fix.

Raw logs are outside frozen source at
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-stop-76bf4d57/`:
`clippy-2f54-all-target.log`, `clippy-2f54-final-hz2.jsonl`,
`clippy-2f54-canonical.log` and `clippy-2f54-workspace-tests.log`. The later
cut retains `clippy-ff1f-all-target.log`, `clippy-ff1f-workspace-doc-tests.log`,
`clippy-ff1f-final-hz2.jsonl` and `clippy-ff1f-canonical.log`; its Clippy and
canonical refresh still fail on the same 25 library diagnostics and initial
admission guard. All 50 owned Rust paths pass formatting and match `ff1f9094`.
These are self-verification, not independent certification. Earlier attempts remain
retained: the `f38ab05a` E0596 introduced by the closure cleanup, its explicit
reborrow correction, the second-pass formatter mismatch, queued cancellations,
`hz4` source-initialization refusal, and the superseded `c29f9a06` compile's
SIGTERM/143 terminal acknowledgment. None is a passing test result. The tracker
contains the original acceptance plus 25 exact remaining diagnostic TODOs.

## 2026-10-24: isolate guarded startup frames without changing authority

The full `a985f9cd` operator attempt aborted on the integrated credential-history
function with stack overflow/SIGABRT before a complete 744-function summary. A
fresh exact-function run reproduced that abort on standard `vmi1149989`; it was
not merely a failure following earlier tests. No stack-size, compiler/backend,
deadline or assertion change was used to repair it.

`83cd4443116dbccbeccf7bbb75fc1bfc5c8a13aa` (tree
`094954254ac7e87499977b9eb582de28900b31fc`) keeps the mutually exclusive plain,
raw-predictive and owned-predictive guarded native constructors in separate
private noninlined frames. The same complete guards, credential registration,
terminal reserve, generation and original constructor are selected, and the
same human/consistency/identity roles are returned. No public interface, source
or credential rule, journal format, numerical algorithm or outcome changed.

The unchanged history function passed on `ts1`: one passed, zero failed/ignored,
743 filtered, 3.06 seconds. More decisively, the original failing worker
`vmi1149989` passed the complete fixed checked-publication and computed-identity
module filters: **42 passed, zero failed/ignored, 702 filtered**, 262.72 seconds.
This covers all nine credential functions and their actual publication, both-key,
rotation, missing/foreign credential, source-free history, composed guard and
resident-sequence cases. No credential test body was changed. Preliminary 17 and
33 estimates missed nested modules and are withdrawn; the filters never changed
and only the actual harness count of 42 is measured evidence. Overlapping runs
are not added together.

Owned formatting and standard workspace/all-target compilation passed at this
exact source. Clippy actually failed on 80 library errors before later targets;
the canonical driver actually refused the unchanged dependency-admission guard.
The complete unfiltered operator run at `83cd4443` passed **744 functions, zero
failed/ignored/filtered**, 510.04 seconds, terminal exit 0 on standard `ts1`.
The clean-overlay receipt binds the exact source and tree above. All original
743 functions from the `bfaea731` base remain registered, alongside the one new
pure diagnostic. The earlier aborted run and the 734-function 726/8 execution
remain separate historical results. Four older intermittent-case investigations
still lack causal repairs and remain open despite this passing full run; all
original P0 acceptance remains open for dependency admission, lint and complete
workspace qualification.
These results are bounded reference execution, not independent certification,
provider authentication, a general stack/RSS bound or complete project qualification.

## 2026-10-24: test forecast recovery and retained Unknown against native contracts

The original predictive namespace at frozen `bfaea731` reproduced **6 passed,
1 failed, zero ignored, 736 filtered**, 35.14 seconds. Its negative called a test
helper that now selects the permitting predictive recovery path. The corrected
original function calls the native nonpredictive guarded API with the same
complete requirements and generation, requires exact `Binding`, and checks both
canonical and staging bytes. Selected raw/owned recovery must append one fence
and preserve numerical state, forecast identity, coverage loss and paid work;
the original no-resume assertion remains. At `da8077c7`, all seven predictive
functions plus the original plain continuation/publication passed **8/0**, zero
ignored, 735 filtered, 44.43 seconds on qualified standard RCH.

The four original retention functions independently reproduced **0 passed,
4 failed, zero ignored, 739 filtered**, 1.41 seconds at `bfaea731`. Their expiry
expectations contradicted the existing native contract: `RetentionExpired`
retains `Unknown` and its charge. The corrected fixtures assert that exact stage,
zero reserved units, unchanged execution/payload/stop receipt and retained charges
through repeated recovery. Valid selected receipts and the actual published-tail
positive still execute. No expiry reducer, outcome, refund or abandonment is
manufactured.

A separate explicitly synthetic unit checks empty-sweep report inputs containing
Unknown or IrrecoverablyUnknown against an otherwise identical completed report
with the same nonzero charge. It tests the renderer only. The durable hosts still
have no journaled explicit-abandonment API; that format/epoch/storage feature and
FA-014 prerequisites remain open. Pure report input does not prove native
irrecoverable history or satisfy the complete packet.

Combined source `a985f9cdf1c1b196fa5aa649007b456992277aac`, tree
`20cbe485895be86c581f4af5afd51f8c4f585eae`, passed all **63 selected functions,
zero failed/ignored, 681 filtered**, 50.20 seconds on standard `vmi1149989`:
all 50 stop functions, seven predictive, one plain continuation, four original
stream-completion functions and one pure diagnostic. All five addressed failure
IDs pass and no previously passing selected ID failed. Overlapping runs and
child-helper repeats are not added to this denominator. These are fixture and
verification repairs, not new production capability or independent certification.

Owned formatting and workspace/all-target compilation passed at this exact source.
Fresh Clippy with warnings denied failed on **80 library errors** before later
targets. The canonical driver executed and refused the unchanged initial
dependency-admission guard. Its admission compiler explicitly supports the two
original reviewed local rows; it is not a finished general-admission path that
can simply admit the third package. No policy, snapshot hash or lock guard was
relaxed. The unfiltered **744-function** operator attempt at this source aborted
with a stack overflow/SIGABRT in the integrated
`credential_history_requires_exact_policy_and_rotated_epoch_before_cleanup_without_live_secrets`
function, exit 101, before a complete summary. Later functions were unrun; this
is not a 743/1 result. This inventory includes nine integrated
credential-publication functions from the `bfaea731` base and the one new pure
diagnostic unit. Four prior intermittent-case investigations and original P0
acceptance remain open. The fatal credential-history case triggered the separate
`83cd4443` startup-frame repair and subsequent evidence recorded above; neither
the passing 63-function selection nor the aborted full attempt is rewritten as
a complete operator pass.

The first `da8077c7` dispatch on `hz4` refused source-namespace initialization
before Cargo. Same-ID recovery confirmed terminal exit 1 and acknowledgment;
the refusal was not testing success and no source fence was removed. Commands,
raw failures, passing output and receipts remain outside source under
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-stop-76bf4d57/`.

## 2026-10-24: exercise pending stop/drain and every clock boundary

At frozen `76bf4d57`, the complete original stop namespace reproduced **42 passed,
8 failed, zero ignored, 684 filtered** on standard RCH `vmi1149989`. Four fixture
files now count the existing before/after clock checks, retain the original Ready
and before-clock cuts, and test the exact post-clock one-short/sufficient budget
neighbors without releasing the writer lock. Post-acquisition expiry may read
time once; it still cannot acknowledge a stop or mutate canonical/staged files.

The phase fixtures now use an actual captured policy source, native congress
review, separate human approval and original dispatch. They retain an empty
stopped-domain control that performs no clock call or new journal event. A real
pending counterpart needs the final event of the original three-event reserve
and a native nonexecution receipt. Failed, panicking and slow clocks preserve
its stopped `Unknown` state, sixteen charged units, zero executions and all
original files until an explicit permitted drain. No native reducer, production
deadline, journal format, authority rule or retention behavior changed.

Exact source `81365a31f59a936c830396b301be83910b4c7dc5`, tree
`9958d28331a2e4a73ae27176fba14ca9b6491cbe`, passed the two decisive phase parents
on standard `hz4`: **2 passed, zero failed/ignored, 732 filtered**, 0.19 seconds.
The complete original stop namespace then finished **48 passed, 2 failed, zero
ignored, 684 filtered**, 4.99 seconds. Six original failure IDs now pass; zero
previously passing stop IDs failed. All 50 registrations remain. The two
retention-state disagreements remain red, with their original assertions intact.
These are fixture repairs and native reference evidence, not new production
capability or independent certification.

Qualified `nightly-2026-09-08`, default stack and standard compiler settings were
used. Owned formatting and workspace/all-target compilation passed at this
source. The canonical driver executed and refused the unchanged initial
dependency-admission guard before downstream gates. The first Clippy refresh
was refused before execution with RCH `missing_runtime`; its qualified-component
setup was refused by slot admission (four estimated cores versus two available).
A subsequent admitted `ts1` run actually executed Clippy with warnings denied
at this exact source and failed on **80 library errors**, exit 101. That is a
fresh measurement, not an all-target lint inventory: later targets were not
reached. The complete unfiltered
734-function operator run at this source finished **726 passed, 8 failed, zero
ignored/filtered**, 1115.89 seconds, exit 101. All six addressed stop failures
passed, but three previously passing functions failed: resident native sequence,
native checked create/recovery, and recovered qualification. Their actor-response,
source-reference timeout/broken-pipe and helper-join failure chains remain open;
no host-timing cause or regression-free result is inferred. Together with the
five other current failures and one historical intermittent sequence case, nine
per-ID investigations remain. The original P0 and all acceptance criteria remain
open.

An unchanged-source control of exactly those three newly failing functions
passed on admitted `vmi1149989`: **3 passed, zero failed/ignored, 731 filtered**,
19.75 seconds. This demonstrates that the full-run failures did not reproduce in
that selected context. It is not a repair or proof of a host cause; all three
investigations remain open and the failed 726/8 result remains authoritative for
that full execution. The attempted same-worker control refused slot admission
under critical disk pressure before tests, without a guard override.

The original explicit reviewer-peer target also passed at this source on
`vmi1149989`: **7 passed, zero failed/ignored/filtered**, 2.36 seconds, including
the actual pinned child and both-key publication. Child-helper repeats are not
added to that denominator. Its independent transport/binding refusals remain
covered; this does not settle the separate operator or dependency/lint failures.

The complete `fa-journal` package subsequently passed at this same source on
`vmi1149989`, including all 13 compile-fail doctests; no failures, ignored tests
or filters were reported. The first attempt remained queued and was cancelled
before start with terminal exit 130 before this admitted reroute. Package and
reviewer results are separate inventories, not additions to the 734-function
operator count or evidence for all workspace compile-fail targets.

Retain the first missing-toolchain refusal, invalid installer-option attempts,
source-owner-fenced installer refusal, missing-runtime refusal and original
42/8 runtime failure under
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-stop-76bf4d57/`.
Same-ID recovery confirmed installer exit 1; installation did not reach its
command. No source fence or shared Git lock was removed, and no local compilation
fallback occurred. Shared main independently incorporated these four fixture
files and later credential-publication work. The latter is outside this frozen
source evidence; the combined working tree is not qualified here.

## 2026-10-24: verify complete receipt, topology and checked-Andon groups

The combined final selection at `9a8cf8f07b63903394500543e8e688014dad626a`
(tree `1d76b8603a2b7d1ee51ca919bc96a00fef650527`) passed **38 functions,
zero failed/ignored, 696 filtered**, 99.77 seconds on qualified standard RCH
`ts1` and the default stack. This includes the 27 receipt functions below,
all seven topology-service functions and all four checked-Andon functions;
the overlapping runs are not added together. Actual actor/helper/human
publication, human rejection, post-dispatch witness/topology loss, unrelated
changes and unchanged plain checked-bootstrap positives all executed.

Two additional original fixtures were wrong: the waiting profile omitted its
required feed, and the outer-routing test expected topology usage for a learned
command that owns its topology grammar. Both original functions reproduced as
failed at `73c7822b` before correction. The valid waiting profile now reaches
the specific no-downgrade refusal without reading the absent feed or creating
a store. The direct topology parser still rejects every original malformed
argument with its exact diagnostic; learned-owned arguments must instead
return the learned grammar diagnostic before reading the nonexistent paths.
No production parser, profile, guard or authority rule changed.

All nine owned Rust files passed qualified formatting and workspace/all-target
compilation at this final source, warnings retained. The canonical driver again
executed and refused the unchanged initial dependency-admission guard. Thirteen
old failure IDs have scoped resolutions in this block. The complete unfiltered
operator inventory at this source finished **723 passed, 11 failed, zero
ignored/filtered**, 483.23 seconds, exit 101. Compared with the retained complete
732-function run's 702/30 result, 19 original failed IDs passed and no previously
passing ID failed. Two added tests account for the inventory increase; repeated
or child-helper runs are not added to these counts.

The granular TODO retains the 11 current failures plus one previously failing
checked learned sequence case that passed this run without a specific repair in
this block. Its earlier failure remains an intermittent-case investigation, not
a claimed fix. Complete qualification and original P0 acceptance remain open;
this is fixture verification, not independent certification. The frozen branch
excludes concurrent credential-policy work on shared main; this evidence does
not qualify their combined working tree. Shared staging and the pre-existing
index lock were preserved; owned changes are committed on the isolated train branch.

## 2026-10-24: distinguish retained nonadmission from admitted policy denial

The learned receipt fixtures now exercise three native outcomes in their original
message, finish, read-only and settlement functions. A supplied snapshot with one
mismatched semantic epoch produces retained `NotAdmitted(Binding)` with no action;
a complete known-false policy creates an admitted `Denied` action; actual native
review and independent human approval still produce confirmed publication. These
are fixture corrections, not a new production authorization mechanism. Original
deadlines, byte/store checks, wrong-selection/floor/anchor controls, interrupted
clocks, unknown liabilities and spent-work assertions remain in place.

At exact source `73c7822bcfdaf22b17b696c7bb56e0833f420ce8`, all 26 original
receipt-selection functions plus the new captured-input boundary passed:
**27 passed, zero failed/ignored, 707 filtered**, 96.37 seconds on standard RCH
`ts1`, qualified `nightly-2026-09-08`, default stack. The new boundary separately
checks that already-expired source-only intake performs no source read, clock
callback or request retention. Historical receipt lookup and recovery retain
all three outcomes after live assets disappear; no action or outcome is injected.
Standard workspace/all-target compilation passed with warnings. The canonical
driver executed and refused the unchanged dependency-admission guard before
formatting, Clippy or workspace tests.

Retain the original 26-function **15 passed/11 failed** baseline, the incorrect
expiry probe **0/1**, and the first expanded candidate **15/12**, including its
new helper regression. Expired learned-stream construction cannot be assumed to
reach request retention; the corrected test changes only the supplied snapshot
binding and preserves the expired-intake refusal. The separate `trj` attempt has
recovered terminal acknowledgment with transport exit 255 and no test result.
The queued, unstarted candidate was cancelled with exit 130 before redispatch.
None of those attempts counts as passing evidence.

Eleven original failure-ID TODOs now have scoped passing evidence, leaving 14 at
this source cut. This is not a full 734-function result or independent
certification. The original P0 and roadmap acceptances remain open, including
dependency/source/target/runtime admission, Clippy and complete qualification.
Raw commands, failed attempts and source receipts are retained outside source in
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-receipts-24c632f8/`.

## 2026-10-24: run learned publication with the original credential guard

The learned operator now accepts the closed outer credential envelope around all
six original recipe versions, including checked publication, computed identity,
joint qualification and forecasting. Its independently loaded perimeter must
resolve the exact immutable policy before model loading. Original guarded
constructors install that registration in the first canonical image, including
mediated topology, before numerical startup.

Live acquisition reads separate bounded broker/provider files under the original
aggregate asset allowance and deadline. Missing, unequal, empty, oversized or
aliased inputs refuse before service sockets, owner creation or recovery. The
actual owner then binds the native non-cloneable permit. One live session carries
it through native/process review, the original first-publication driver, finite
publication waiting and resident Message/Message/Finish. Provisioning files need
not survive binding; neither waiting nor a successor request reopens them. Every
request still needs its original helper review, automatic key, human approval
and selected publication witnesses.

Recovery, receipt and Stop/Drain retain the exact credential policy and current
generation/revoked expectation before cleanup or fencing. Historical loading
needs no live inventory or credential files. Anchored review audit retains its
original immutable-policy/history-anchor meaning: epoch-only expectations leave
the past audit unchanged and grant no present effect right. A changed or omitted
policy refuses. Secrets and permits never enter the recipe, journal or packets.

All 77 selected operator functions pass on the feature source based on
`a14a9e5f`: nine new credential functions and 68
existing publication, waiting, identity/joint, cooperative generation/prediction,
recovery, resident and native-publication regressions. The new combined test
publishes through credential, identity, joint, owned-forecast, topology and
witness guards together; a relevant witness change then refuses despite the same
valid credential and original reviews. Separate positive cases retain one permit
after provisioning files disappear during a wait or resident sequence. The
denominator excludes the two registered subprocess helper entry points, whose
actual child invocations are not counted again.

Concurrent main fixture fixes through `341b7d45` were then preserved with every
feature source byte unchanged. All five affected native checked-publication
functions pass again, and standard Cargo check passes the complete combined
operator test source. Runtime
uses the standard LLVM harness with the official Cranelift native library and
32 MiB test-thread stacks. A verification mirror removes only unselected test
registration attributes; production and every selected test file/body remain
unchanged, with original source hashes checked before and after execution. The
required xtask gate was run again and still refuses the unchanged dependency
admission check before formatting, Clippy or full-suite execution. These results
do not establish full workspace or FA-011/014 qualification.

This increment integrates the existing canonical journal publication sink. The
independent external file endpoint still needs durable send/receipt and
whole-process recovery integration; no remote provider or hostile-process
containment is claimed. See [operator selection and usage](docs/LEARNED_CREDENTIAL_PUBLICATION.md).

## 2026-10-24: retain native reviewer deadlines while waiting for readiness

The shared native and generated reviewer fixtures now use the existing verified
readiness helper after the pathname appears, with only the **remaining** original
ten-second setup allowance. Their original twelve-second protocol limit still
runs from the same start. This addresses the broad run's actual
`ConnectionRefused` race; production connection behavior and authentication,
source, accounting and outcome assertions are unchanged.

The complete original native/generated inventory plus its three existing controls
and seven peer entries passed: **121 passed, zero failed/ignored, 612 filtered**
on standard RCH `ts1`, default stack and qualified `nightly-2026-09-08`.
The exact cut is `3b38f717` plus two-path overlay
`e03882634c9457cf09adf050c28928fa414c5e47f214f5cad70df9198be5f04d`, committed in
`bf6b085e41e126359e484f94e0ad2963d590ddb3`. Owned formatting and standard
workspace/all-target compilation passed at that commit, warnings retained.
The canonical driver executed there and failed the unchanged initial dependency
guard before formatting, Clippy or workspace tests. RCH reported the same
`j-30078706395382216` for these three commands; their separate retained logs,
commands and start times distinguish them. Exit statuses are respectively 0/0/1.

The complete **702/30 failed** operator result below remains retained. Four of
those failures have the later checked-capture pass and the native-sharded failure
now has repaired-source passing evidence. The granular original P0 TODO contains
exactly 25 remaining unchecked failure IDs. This is not an unfiltered 733-entry
pass or independent certification. Dependency/source/target/runtime admission,
Clippy and complete project qualification remain open; no Bead or packet closes.

## 2026-10-24: execute checked continuation with exact capture attempts

The checked-continuation fixture had labeled every capture as native attempt 1,
including second-action captures and later input changes. The native gate binds
both attempt and the complete action frame, so it correctly refused those
captures. Parameterizing the fixture's actual attempt restores the existing
second-publication workflow without altering production validation.

All eight original checked parents, a new wrong-attempt/permitted-attempt pair
and the existing unchecked two-publication control passed: **10 passed, zero
failed/ignored, 723 filtered**, on standard RCH `ts1`, qualified
`nightly-2026-09-08` and default stack (`j-30078706395382212`). The new pair checks
exact Binding/cancellation and first-publication retention versus freshly reviewed
publication, exact second-attempt stages, five/eleven-unit charges and zero
reservations. Original phantom, missing-witness, missing-topology, both-loss,
human-rejection, profile, replay-expiry, receipt and CLI assertions remain.

Exact Rust source is `3b38f71721c59c1e1547082d625d698df846e3e2`, the sole fixture
path added to `d218b0b5`; passing overlay fingerprint
`699865740dd234c5e6008ba9990026b1dc433eb2395f7af1ad790ce8c7386554`.
Owned formatting and standard workspace/all-target compilation passed at this
committed source on `ts1`, warnings retained. The preliminary diagnostic dispatch
queue-timed-out with `RCH-I001`/exit 103 before execution; its temporary source
line was byte-restored before repair. It provides no runtime evidence.

Four additional original failure IDs have scoped passing evidence. The full
732-entry run at the preceding `d218b0b5` completed **702 passed, 30 failed,
zero ignored/filtered** (`j-30078706395382190`, `vmi1149989`, 1071.38 seconds).
Comparing its exact outer failure set with the preceding complete 728-entry
run finds 34 original failures now passing and zero previously passing IDs
newly failing. That broad run predates this one-path repair/new test (latest
inventory 733). Its 30 failures remain a failed result, including the four
checked parents subsequently passing in the separate ten-entry selection.

The broad run also reproduced a real `ConnectionRefused` in the shared native
reviewer fixture, followed by actor/reviewer deadlines. That sharded case had
passed in the earlier scoped 114-entry run; its TODO was reopened. The union
inventory at that point had 26 old failure IDs without a passing latest
resolution. The later two-path readiness repair and complete 121-entry selection
above address that race while preserving both original deadline bounds. No red
result is erased by retrying.
No production feature, authority rule, dependency, source contract, format or
deadline changed. Complete qualification and original P0 acceptance remain open.

## 2026-10-24: pin original credential policy in learned recipes

The original learned configuration now binds an exact nonsecret credential
policy: effect family, route, credential name and adapter-profile generation.
The pure public resolver uses the existing perimeter validator. Selection grants
no authority; learned Enable requires the genuinely installed original guard to
match before a numerical owner exists. Plain, raw-predictive, owned-predictive
and mediated guarded constructors retain their original independent registration
and first-image publication. Unselected recipes keep their exact bytes.

The disjoint bounded recipe wrapper makes omitted or changed policy refuse in
every learned receipt, review replay, anchored audit and recovery reader before
numerical replay, staging cleanup or a recovery fence. Dynamic credential
generation and revocation stay in the original independent recovery requirements.
Rotation preserves the numerical recipe and the charged action while requiring
a new original live permit; a recovered or revoked permit cannot regain effect
rights. No secret, credential-equality assertion or permit enters the recipe.

All 39 public regression functions pass: seven new learned-policy functions,
seventeen original credential guard/driver/lifecycle/source-completion functions
and fifteen learned-publication functions. The new tests cover six real guarded
bootstrap lanes, actual generated OK output after sidecar review and both keys,
missing/mismatched credentials, policy-field and audit binding, unchanged
numerical expenditure, rotation, revocation and original nonexecution accounting.
Standard Cargo check passes all six complete public targets. Independent source
review found no blocker, and source hashes remained unchanged during execution.

Runtime uses standard LLVM harnesses linked to the qualified nightly's official
Cranelift library with 32 MiB test-thread stacks. The required full xtask driver
was executed and still refuses the existing dependency-admission guard before
formatting, Clippy or full-suite execution. This is scoped native integration
for the original journal publication sink, not external-provider containment or
full FA-011/014 qualification. The operator increment above consumes this policy
binding. See [learned credential policy](docs/FILE_LEARNED_CREDENTIAL_POLICY.md).

## 2026-10-24: execute the original sequential publication integration

The original 18 service/parser/client entries first completed ten passed/eight
failed at `e5b141b0` (`j-30078706395382143`, `vmi1149989`). Corrections to fixture
frames and intermediate-outcome polling reached sixteen passed/two failed
(`j-30078706395382150`). The final declared selection adds seven peer entries:
**25 passed, zero failed/ignored, 707 filtered** on the qualified standard
compiler/default stack (`j-30078706395382170`, `vmi1149989`). Nested child-helper
runs are not added to that denominator. Retain both failed selections.

This block repairs tests, not the production authorization reducer. It polls
the existing `Unknown(OutcomeUnknown)` intermediate under the original deadline,
funds complete fixture payloads without changing their seven-unit charge, and
waits for actual reviewer listen readiness before the original credential and
audience checks. The test-only wait retries only `NotFound`/`ConnectionRefused`;
it times out without removing a stale socket and also admits a permitted peer.
The immediate production connector retains its original behavior.

The cumulative-budget oracle now follows plan §7.3: reservation occurs at
Authorize after review. The original ten-unit case remains with seven charged,
three available and zero reserved; thirteen versus fourteen adds an exact
one-short/permitted pair. Both requests receive independent reviews. Underfunded
authorization returns the native Limit error and retains a cancelled second
request rather than the test's former incorrect `NotAdmitted` expectation.
The stale second target also receives its independent review, then must return
exact `ConfirmedNotExecuted`, with original actor bytes and first charge intact.

The four Rust files are committed in `d218b0b5d0bd2440b693e08a4480dcdc89dd1d32`,
a descendant of shared main's `ad4a2646`, on
`train/navyheron/native-source-leases-20261024`. Their passing selection used
base `e5b141b0` plus overlay
`517572c0fd12069b86fc3626083ac9c15f2ebdf0c376410d33a785536312f02b`;
the only Rust differences from that base are these four paths. Owned-file
formatting and standard workspace/all-target compilation passed at the committed
source (`j-30078706395382180`, `vmi1149989`, warnings retained). Two initial
source-only dispatches refused invalid option combinations before execution;
both are retained. The canonical driver at the same commit executed and failed
the unchanged dependency guard before formatting, Clippy or tests
(`j-30078706395382184`, `vmi1152480`). No local compilation fallback occurred.

Shared main incorporated the earlier native source-lease commits and three
current files through `e6e21f09`, `20234ef4` and `ad4a2646`. NavyHeron preserved
shared HEAD/index and the pre-existing ambiguous index lock; the remaining
service-fixture commit remains on the isolated branch. Eight additional original
failure IDs have scoped passing evidence, leaving 29 unresolved IDs in the old
inventory. That union count is not a complete-suite result. The full 732-entry
operator inventory at `d218b0b5` completed 702 passed/30 failed after a
queue-timeout refusal (`RCH-I001`, exit 103, no local fallback); see the later
failure-set and readiness record above. Dependency
admission, Clippy, full runtime qualification and original P0 acceptance remain
open. Raw attempts and fresh source/test audit are retained outside source at
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-sequence-e5b141b0/`.

## 2026-10-24: check native source leases before numerical work

The generated operator now calls the existing `capture_file_policy_state` gate
after acknowledging its post-read clock. The native service calls the same gate
at its final stamped clock before generation, resume or finish preparation.
Previously those paths observed time without checking the registered source's
read-start lease. The learned operator already made this explicit check. This
repair connects that original gate; it adds no journal tag, model kernel, source
reader, dependency, authority reducer or historical receipt requirement.

A causal test now delays clock samples only after the first intent is durable,
instead of jumping at an unrelated startup-call ordinal. Before the production
repair, the complete 113-entry native/generated cut finished **112 passed, one
failed**, with the expired case still advancing all four numerical positions
(`j-30078706395382071`). After the repair, **114 passed, zero failed/ignored,
617 filtered**, on the qualified standard compiler/default stack at `vmi1149989`
(`j-30078706395382079`). The denominator is all 111 original native/generated
entries plus three new controls, including registered helper entrypoints. The
new native pair reads at tick 1000: tick 1009 admits an actual first step, while
1010 refuses with unchanged numerical state and generation revision.

The same change corrects test oracles for audience text versus reviewed frames,
whole-frame charges, independently bound helper epochs after a recovery fence,
and exact `Knowledge::Withheld { authority_required: "own_request" }` responses.
Full one-byte stream finish cases retain their publication limits while funding
the tokenizer's existing worst-case generation allocation. Two new actual
operator controls retain the original undersized allocation and require refusal,
no text intent, zero publication, charges and reservations. The earlier complete
111-entry result, **108 passed/three failed**, and all other failed attempts remain
retained. No production bound, deadline, assertion registration or native source
contract was relaxed.

The verified Rust source is base `7228d44ad4bf02d9111d9702412cfc27ac128374` plus
12 owned paths, overlay fingerprint
`f11b1a91644b45b49a980630e9d6102a061e4e902fac484c65369a16ffa175a2`.
All 12 pass qualified individual-file formatting. Standard workspace/all-target
compilation passed at that exact cut (`j-30078706395382085`, `vmi1149989`), with
warnings retained. The canonical driver executed at the same Rust cut
on `vmi1152480` (`j-30078706395382086`) and failed the unchanged initial dependency
admission guard before formatting, Clippy or tests. Its first dispatch refused
`RCH-I003` before execution; the admitted retry used one Cargo job. No local
fallback occurred.

The twelve qualified Rust files are committed byte-for-byte in
`cafce41da3dc909bb93de9d98991158d467c8a86` on
`train/navyheron/native-source-leases-20261024`. A pre-existing shared Git index
lock prevented ordinary staging. A private index recorded only the seventeen
owned code/test/contract/status/tracker paths on that isolated branch; shared
HEAD, staging and the ambiguous lock are preserved. Integration into shared HEAD
remains pending resolution of that lock's ownership.

Fourteen original failure IDs now have scoped passing evidence, leaving 37 of
the preceding 51-ID unresolved inventory. This is not a new complete operator
suite count. Complete source/package admission, Clippy, operator/workspace runtime
qualification and the observed transient reviewer startup race remain open on
the original P0. Raw attempts, normalized source/test review and exact owned-file
hashes are outside frozen source at
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-native-7228d44a/`.

## 2026-10-24: recover learned owners on the standard test-thread stack

The learned recovery selector now finishes its five original cursor variants in
separate private leaf adapters. Each adapter calls the same native finalizer and
returns the same owner and human, identity, consistency and topology roles. The
unchanged exact-reopen regression previously aborted on the default test-thread
stack; debugger evidence located a 993,968-byte selector frame. The new dispatcher
frame is 161,264 bytes, with separate leaf frames; this is not a measurement of
whole-stack reduction. No stack setting, allocation, schema or authority contract
was changed.

At base `5df1aeb4f352e11a81bd0413e0007fbe2e8c9d25`, the original regression aborted
on qualified standard-backend RCH worker `vmi1264463`. The same worker passed the
unchanged test with only the selector repair (overlay `e19d083abbb3a10c`): one
passed, 726 unrelated functions filtered. Its changed tokenizer, prompt, sampling,
monitor and weights controls retain the original unchanged-journal assertions,
followed by a valid reopen. A complete unfiltered run at that repaired source
then finished without abort: **368 passed, 359 failed, zero ignored/filtered**
across all 727 original harness entries. Helper entrypoints are included in that
denominator and are not independent capability claims.

Of those 359 failed parent functions, 306 explicitly reported a Unix socket path
length refusal. The shared operator fixtures now create private short directories
while preserving their unique names and refusing occupied paths. The production
100-byte limit is unchanged. A new real-socket boundary regression passed on
`vmi1149989` (RCH `j-30078269650895234`, base `5df1aeb4` plus overlay
`321ae0d5e40f2eb1f`): a 100-byte socket binds and is secured, while a 101-byte
profile refuses before journal creation. That run executed one test with 727
filtered. The follow-on inventory is 728 entries, comprising the original 727
plus this boundary. That invocation compiled and reached previously blocked
checked learned publication, then aborted on a second default-stack overflow in
`checked_learned_publication_rejects_late_phantoms_but_keeps_disjoint_changes_in_all_forecast_modes`.
That aborted attempt has no completed suite result. Its partial failures remain
retained alongside the subsequent startup repair and complete result below.

The second debugger trace found 792,112-byte `create_selected` and 644,752-byte
`topology::start` frames during native learned stream startup. Separating the
original identity constructor and topology create/recover arms into leaf paths
preserves their calls, guards, reserves, checkpoint ordering and returned roles.
The unchanged original checked regression then passed on the same standard
worker and default stack (overlay `226e8000f19d4a5b`): one function containing
eight original permitted/disjoint and forbidden/phantom cases, 727 unrelated
functions filtered. Complete qualification remains open. Additional native
Stop/deadline and durable raw/cancel/stream fixtures use local short private
roots without changing the shared disk-only fixtures or their accounting and
deadline assertions.

Coupled publication preparation also retains its selected whole-input or
structured context beside the native failure. A missing opaque lane still
quarantines the owner with `Contract(Incomplete)`; no second acquisition,
fallback input or changed binding order was introduced. The original early
missing-input regression retains its assertions and additionally requires that
native cause. That original regression passes in the follow-on run, alongside
the original real Stop/deadline and durable raw/cancel/stream cases.

The complete 42-path operator cut finished on the default stack with the
qualified standard compiler: **664 passed, 64 failed, zero ignored/filtered**, all
728 registered entries (RCH `j-30078269650895280`, `vmi1149989`). Its Rust bytes
were subsequently committed through `ac1575e9b9b2e927ff1f9081780de853403f5089`;
this precedes the one-file shared-option routing fix below. Comparing complete
failure sets against the preceding 727-entry run finds 295 formerly failing
original entries now passing and zero formerly passing entries newly failing.
The extra passed entry is the new socket boundary. Helper entrypoints and
same-origin retries are not additional independent capabilities. This is still
a failed operator suite; the 64 original failures remain retained.

The intermediate 43-path Rust cut passed qualified standard-backend workspace/all-target
compilation on `hz3` (RCH `j-30078269650895290`, base `5df1aeb4` plus overlay
`0d1b4e62a35b74d1ddb3ac33cb120cc746e274fc0c4ab47467f498b27b7442fb`).
Warnings remain. All 43 owned Rust files also pass the qualified formatter's
individual-file check; this does not establish workspace-wide formatting.

The original guarded receipt CLI case exposed a shared-option routing mismatch:
its implemented receipt-only handler was unreachable with the otherwise unused
qualification option. The parser now accepts that option only after the exact
guarded-resume positionals. The handler still never reads or activates it;
unguarded resume, query and create refusals remain. Missing-profile/file negatives
were added beside the permitted parse. The complete parser tests and original
actual CLI regression pass on the subsequent 43-path source cut: four passed, zero
failed/ignored, 724 filtered, RCH `j-30078269650895300` on `vmi1149989`, overlay
`0d1b4e62a35b74d1`. This is scoped evidence, not a complete rerun. The first
substring selection also matched five native qualification functions and
finished eight passed/one failed, 719 filtered; its original native `Incomplete`
failure remains open. The subsequent exact-name dispatch on `hz3` was refused
with `RCH-I006` before execution; no local fallback occurred. The admitted worker
then executed the four declared exact IDs. No result from those attempts is
discarded or counted twice.

The canonical driver on that same 43-path cut executed on `vmi1152480`
(`j-30078269650895295`) and failed at the unchanged initial dependency-admission
guard before formatting, Clippy or workspace tests. No package, manifest digest,
production bound, deadline, test-stack size or original assertion was relaxed.
Three remaining receipt-service fixture constructors now use local private short
roots. All 18 original functions executed at the 46-path cut: **16 passed, two
failed, zero ignored, 710 filtered** (`j-30078269650895304`, `vmi1149989`). The
two failures expect `IrrecoverablyUnknown` after retention expiry, while the
native broker's `reconcile_pending`/`reconcile_status` contract from `399e87a0`
explicitly retains `Unknown` and charged liability. No implicit abandonment,
refund, native transition or expected outcome was changed to manufacture green.
That contract disagreement remains on the original P0.

The final 46-path Rust cut passed qualified standard workspace/all-target
compilation (`j-30078269650895312`, `vmi1149989`, base `5df1aeb4` plus overlay
`0a70ae622cd53623329d40b53556b5129e0dfd339130c7162049ef482c936a0e`). Its changed
Rust bytes match committed `8e8d77dc4f8da6decb186c51d4bbfe58ffeae54f`. All 46 owned
files pass the qualified formatter's individual-file check. The canonical driver
also ran without an overlay at that committed revision (`j-30078269650895313`,
`vmi1152480`) and failed the unchanged admission guard before formatting, Clippy
or workspace tests. Warnings, the 80-error library Clippy baseline and complete
source/package admission remain open.

Thirteen of the 64 failure IDs from the completed 42-path run have subsequent
scoped passes: the guarded CLI case and twelve receipt cases. The original Bead
retains those exact IDs and 51 unresolved IDs with the remaining qualification
work. This is inventory tracking; the complete latest operator suite has not
rerun, and the selected results cannot be added into a new aggregate suite count.

Raw attempts, normalized original/test diffs and debugger observations are retained
outside frozen source at
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-stack-5df1aeb4/`.
These repairs do not qualify the full operator, dependency admission, Clippy or
the canonical workspace gate, and the original P0 Bead remains in progress.

## 2026-10-24: restore executable reference checks without granting admission

The Native Andon peer now resolves its existing transport, receipt and nested
test modules by their original explicit paths. This fixes the workspace's
all-target compilation failure and makes the original real peer tests reachable.
A guarded-resume fixture's unmatched format delimiter is corrected. Private,
short socket fixture directories preserve real Unix and pinned-child transport
under RCH's deep temporary roots; production still rejects an overlong address.
The reviewer negative now creates the automatic reservation it asserts survives,
retaining its original 16-unit assertion and checking that no units were charged.

Source-snapshot discovery now covers reference examples/fixtures and journal
source/tests/manifest, alongside the original roots. Independent sandbox hashes
and positive/negative controls reproduce the former omissions and bind changed
bytes after discovery. No manifest was regenerated and no package was admitted.
Actual Cargo metadata retains all three workspace packages and their resolved
journal-to-reference edge; the original two-package policy still refuses it.
Registry test lookup accepts only sole conditional language-feature payloads,
while conditional ignore/cfg, nested and mixed payloads still refuse. Negative
sandboxes now copy the actual retained contracts and example source before their
single causal mutation; none of their finding assertions were relaxed.

Qualified standard-backend RCH execution at base `4a9fb80e8d34b1cdac65960dcfccc6e43295a9c1`
plus the corresponding owned-path overlays passed all 248 xtask tests and all 46
Native Andon tests. The original reviewer target passed six parent regressions
and its registered helper entrypoint; the pinned child also executed the helper
with actual credentials. The unchanged journal source passed 81 unit tests,
143 integration tests and 13 compile-fail doctests. Each target ran unfiltered;
the child intentionally selects its own helper, not an additional independent
parent regression. Raw attempts, including earlier failures, are retained at
`/Users/jemanuel/Documents/Codex/2026-10-24/franken-alignment-implementation-0761ea9b/`.

These are bounded reference/enabler results, not production alignment evidence.
The frozen workspace/all-targets check also passed at `b688a37bc7baea0efce2d53fc1b0bb769e40af86`
on the qualified standard compiler. The driver at that exact source failed at
its unchanged lock guard. The complete 727-function publication operator target
compiled at the earlier source cut, recorded failed functions and then aborted
on a test-thread stack overflow in
`learned_command_exact_reopen_refuses_changed_independent_recipe_without_journal_writes`.
It has no completed suite result. A real child fixture also refused its overlong
Unix socket address. No registrations or production bounds were changed to
avoid those failures.

The follow-on diagnosis repair funds payload bytes before original policy
evaluation, supplies valid permitted controls for each unchanged policy, and
retains the occupied test-staging bytes before the separate positive writer
phase. All 21 `diagnosis::` functions pass on the standard compiler at base
`b688a37b` plus the three owned diagnosis paths (overlay `69126149d57c429e`):
18 behavioral cases and three renderer cases, zero failed/ignored, 706 unrelated
functions filtered. Complete search masks, all original policy/denial and
zero-write assertions, and captured-image versus later-writer checks remain.
The preliminary policy-only repair and 19/21 intermediate run are retained as
failures; neither counts as qualification. This proves the selected original
operator paths, not an admitted effect or complete operator suite.

The full qualification bridge remains in progress: reviewed source/package
admission is incomplete, the library Clippy baseline reports 80 errors, and
complete operator/workspace runtime qualification remains open. No roadmap
packet or Bead is closed by this repair.

## 2026-10-24: finish finite producer catch-up in the learned operator

The checked learned operator now accepts the original finite-wait witness
profile, `fa.supervised-witnesses/4`. Native and process helper review retain
their original inputs and paid progress while a valid producer capture lags a
complete feed. After both original approvals, the same prepared request retains
its frozen action, review, human key and one finite retry allowance. Each yield
returns to the unchanged deadline, authenticated Stop and actor-cancellation
checks before another acquisition. Review acquisitions do not spend final retries.

The existing native waiting diagnostic alone permits that yield. Its Ready,
human-key, expiry, control-basis and acknowledged-deferral conditions are not
replaced with an operator guess. Persistent lag exhausts the original bounded
allowance even with a frozen clock. Relevant changes, missing history and
post-dispatch lag retain strict refusal or reconciliation, including unknown
charges. Waiting adds no native grant, journal format or recovery permission.

All 68 selected operator regression functions pass: eight new finite-wait
functions and sixty existing checked-publication, generation, forecast, joint,
identity, recovery, resident, bootstrap and native-checked functions. The new
cases execute actual native/process helpers, real source catch-up and both
original keys; they cover changed empty-range membership, exact finite retry
exhaustion, Stop, cancellation, the original accepted deadline, strict
post-dispatch behavior and historical recovery without live sources. Both
original child entrypoints are registered and used by real processes; their
standalone no-input invocations are excluded from the 68-function denominator.

The complete unmodified operator test source and all three public deferral
integration targets pass standard Cargo check. The preceding 31 native/public
regressions remain source-identical and passed in this increment. Independent
source review found no blocker. Runtime uses standard LLVM harnesses linked to
the qualified nightly's official Cranelift library with 32 MiB stacks; the scoped
mirror changes only unselected test registrations, with selected files byte
identical and original source hashes unchanged throughout execution.

Earlier failed attempts remain retained, including shared-storage exhaustion and
one corrected deadline fixture. That fixture now observes authenticated Pending
within the allowed lifetime, lets one acknowledged wait complete, then checks
the authoritative cancellation receipt after expiry without demanding transport
service beyond the original deadline. No production lifetime or assertion about
charge, effect, cancellation or numerical expenditure was weakened.

The required full xtask driver was rerun and still stops at the existing
unchanged dependency-admission guard before formatting, Clippy or full-suite
execution. Full FA-061/062, deployment and release qualification remain open.
See [checked learned publication](docs/LEARNED_CHECKED_PUBLICATION.md).

## 2026-10-23: retain learned review while publication producers catch up

The original computed, native-model and process learned drivers now expose
explicit capture-or-defer acquisition around their original policy-only learned
input provider. A valid snapshot behind a complete change feed remains an
acknowledged nonpermitting observation, allowing native/process review to
advance under its original schedule and budgets. Only the original ready-job,
matching-human-key and Incomplete checks can recommend later bounded waiting.
Malformed history, unavailable evidence, changed helper input and post-dispatch
lag retain the original refusal and cleanup behavior. No capture replaces the
actual learned sidecar, and no review or effect key is synthesized.

The durable capture-or-defer transition also uses the existing same-owner
control replay cache. It derives Installed/Deferred from the original candidate
after semantic replay and returns that outcome only after storage acknowledgment.
Warm owners reuse their acknowledged prefix; cold owners still replay normally,
and every unverified suffix event remains subject to the original checks.

All 31 selected public regression functions pass: fifteen learned-publication
cases, including five new learned deferral functions, seven original durable
deferral cases and nine original driver/socket cases. The new tests exercise
actual generated output and both original keys, warm/cold historical equality,
unchanged numerical expenditure, negative key/range and opaque-input changes,
missing history, equivocation, rollback, recovery fencing, failed withdrawal and
strict post-dispatch charge preservation. The three complete public test targets
pass standard Cargo check, and independent source review found no blocker.

Runtime evidence uses standard LLVM harnesses linked to the qualified nightly's
official Cranelift native library with 32 MiB thread stacks; source hashes remain
unchanged throughout execution. This is scoped reference evidence, not a full
LLVM native-library build, performance SLO, deployment or release qualification.
The required full xtask driver executes and stops at the unchanged existing
dependency-admission guard before formatting, Clippy or full-suite execution.
No dependency, journal format, native gate or work allowance changes.

The runnable learned finite-wait profile is the next operator integration slice;
these native APIs do not themselves add a polling loop. FA-061/062 remain open.
See [supervised capture deferral](docs/SUPERVISED_CAPTURE_DEFERRAL.md).

## 2026-10-24: run checked learned publication through real operator boundaries

The closed `fa.learned-checked-publication/1` envelope selects original publication
witnesses around version-1 through version-6 learned recipes. Canonical optional
composition is checked(identity(joint(original))). The profile consumes the
existing aggregate asset allowance; unsupported finite-wait and witness-joint
selections refuse before startup. Immutable validation, feed, freshness and
fallback policy enter the independently retained numerical recipe.

Every admitted Message and Finish binds its own original evidence before native
or process helper launch. The three original learned driver families reacquire
publication sources around the policy-only learned-input provider, preserving
actual sidecars, both effect keys, topology, identity and joint qualification.
Resident entries retain one exact native policy and reacquire each attempt's
basis. Historical receipt, audit and Stop reconstruct the selected recipe without
current producer, capture, feed or helper files. Plain capture preparation now
matches an explicitly absent feed without calling a feed-only fallback getter.

The operator returns its original cooperative replay custody at completed prompt
and output boundaries instead of dropping it. This uses the preceding native
cache repair; all source reads, numerical quanta, deadlines and work allowances
remain unchanged. The earlier resident positive case reached its 60-second wall
deadline after two confirmed messages. The final positive case completes Message,
Message and separately reviewed Finish under the original limits. This is a
bounded regression result, not a latency SLO or statistical benchmark.

An invalid original witness can quarantine the owner. Final reply grace now
drives the original authenticated observation transport without requiring an
unavailable live cancellation-status lookup. The actor receives the existing
`ControllerUnavailable` knowledge; quarantine and failed Stop/drain remain in
force. The negative resident test requires that exact peer/operator response,
preserved first effect and charge, no second review and no third actor. It still
fails on broken connections, timeouts or missing responses.

All 53 scoped operator functions pass, including twelve new checked-publication
functions and original generation, prediction, joint, identity-recipe, recovery,
resident, producer-bootstrap and native-checked regressions. Both original child
entrypoints are registered and exercised by actual processes; their standalone
no-input invocations are excluded from the 53-function denominator. All 31 public
publication/custody functions also pass. Standard Cargo checks pass for the
production operator, the complete unmodified operator test source and the public
target. Independent source reviews found no blocker.

Runtime uses standard LLVM harnesses linked to the qualified nightly's official
Cranelift library with 32 MiB stacks. The scoped operator mirror changes only
unselected test registrations; all bodies, assertions and fixtures remain, with
source hashes unchanged throughout the successful execution. Earlier failures
remain retained: producer bootstrap could not see the nested learned selection;
repeated cold replay exhausted the resident deadline; final response flushing
queried a quarantined ledger. Existing helper text/epoch and frame-versus-sink
fixture mismatches were corrected against the original contracts. A fixture name
collision was caught before execution; no timeout, production budget or gate was
relaxed to obtain success.

The required full xtask still refuses the existing dependency graph before
formatting, Clippy or full-suite execution. Six witness-profile forms are admitted;
the new operator effect tests cover direct captures, coupled strict/fallback
producers and fallback whole-input mode. Separate-feed and strict whole-input
selection have parser/configuration coverage here, and the original native
fallback tests cover evicted history. Full FA-062 packet, deployment and release
qualification remain open. See [checked learned publication](docs/LEARNED_CHECKED_PUBLICATION.md).

## 2026-10-24: retain acknowledged replay custody through publication acquisition

Producer catch-up now recognizes the original feed cut inside an independently
bound learned Enable configuration. It uses the exact runtime registration and
keeps the original source, conflicting-overlap and missing-history checks. This
repairs the live coupled-producer path for the preceding learned registration.

Publication input withdrawal, freshness persistence, direct capture installation
and coupled feed ingestion now reuse the existing control replay continuation.
They retain original preflights, fresh external acquisition, canonical encoding
and acknowledgment. A successful feed batch retains the actual retired machine
at its pre-batch event count; every notice, heartbeat and capture in its suffix
must still replay. Cold owners retain their original behavior.

The consuming `FileLearnedReplayContinuation::retain_for_control` returns an
already acknowledged same-owner predecessor to the existing cache without a
write, clock observation, numerical operation or effect key. It rejects foreign,
stale, recovered and storage-faulted owners, validates existing custody and keeps
a newer verified prefix. Cooperative raw and owned forecast steppers return their
terminal carry while preserving the original result and interruption latch.

All 31 public integration functions pass: ten learned publication functions,
including four new cache/custody regressions, and 21 unchanged witness, snapshot
fallback and producer-driver functions. Warm and explicitly cold owners retain
equal publication outcomes, numerical work and independently replayed historical
state. The producer pair exercises a two-notice/heartbeat/capture batch, lost
input and contradictory already-acknowledged history. Handback itself leaves
canonical bytes and live inspection unchanged; actual storage failure remains
unavailable. The initial twin fixture omitted the required fresh acquisition
after authorization consumed its capture. That failure is retained; the fixture
now asserts consumption and reacquires through the original readers before
dispatch, without weakening the native gate.

Standard compiler checking and independent source review pass. Runtime uses the
qualified nightly's official Cranelift library and standard LLVM test harnesses
with 32 MiB stacks. The required full xtask still refuses the unchanged dependency
graph before formatting, Clippy or full-suite execution. This is a native
execution improvement, not a performance SLO, full FA-062 packet or release
qualification. See [learned publication replay](docs/LEARNED_PUBLICATION_REPLAY.md).

## 2026-10-23: bind final-publication witnesses to learned ownership

`FileLearnedConfig::with_required_publication_validation` freezes the original
validation limits and optional complete change-feed, freshness and snapshot
fallback policies into the independent numerical recipe. The original witness
codec supplies the disjoint wrapper; existing unselected bytes remain unchanged.
The same learned Enable transition installs the original guards, including the
producer clock-domain check, before a numerical owner or first canonical image
can escape. Plain, raw-predictive, owned-predictive and topology-mediated owners
retain their separate identity, joint-credibility and recovery-reserve contracts.

All six new public integration functions pass. They exercise every selected
field, wrapper composition, all six predictor/topology bootstrap lanes, invalid
limits and clock domains before storage, exact recovery and historical selection
before cleanup, and actual generated text through original sidecar review and
both effect keys. Disjoint changes publish; late phantom insertion, opaque epoch
drift, closure loss and unavailable evidence refuse the obsolete publication.
Old receipts and numerical expenditure survive recovery. A validation-work
refusal cannot grant rights or publish an unacknowledged validation report.

All twenty-one unchanged witness, snapshot-fallback and coupled-producer driver
regressions also pass. The standard qualified compiler checks the production
library/operator and new public target; runtime uses unchanged LLVM harnesses
against the qualified nightly's official Cranelift library, with 32 MiB stacks.
Independent source review found no blocker. The initial new-target run retained
two fixture failures: an oversized count refused in the original codec before
bootstrap, and a refused transaction correctly exposed no acknowledged report.
The fixtures now test the exact native one-over bound and unchanged failed-write
state; no production validator or assertion was weakened.

The standard full xtask driver executes and refuses the unchanged dependency
graph before formatting, Clippy or full-suite execution. No admission guard,
dependency or lockfile was changed to manufacture a passing gate.

This is the native composition slice of FA-062, not full packet or release
qualification. See [checked learned publication](docs/LEARNED_CHECKED_PUBLICATION.md).

## 2026-10-24: enforce computed identity in the learned operator and audit native reviews

The learned operator accepts the closed `fa.learned-identity-publication/1`
envelope around an original version-1 through version-6 recipe, optionally with
joint credibility inside it. The complete independent passport and identity
policy are bound to the exact learned configuration in live and historical modes.
Ordinary, raw-predictive, owned-predictive and topology-mediated paths retain the
original unique identity observer alongside all existing roles.

Each request executes the original cooperative stimulus measurement before
generation. Stop and deadline interruption cancel before journal mutation while
retaining completed work and observer custody. A committed measurement refusal
closes only its still-current challenge; successful computation observes fresh
native time before separate matching installation. Recovery requires fresh
evidence and preserves numerical draws, forecast expenditure and old effect
liability. Resident sequences bind one registration and require distinct check
IDs and increasing measurement sequences. Helper Congress and human approval
remain separate prerequisites for publication.

The first runtime pass exposed an older functional audit gap: native learned
reviews finish through the original learned-sidecar event, but historical audit
selected only ordinary Finish. Both read-only extraction and effect attribution
now consume the actual native archive and application receipt. Exact anchors,
original numerical witnesses, independent recipe binding and whole-suffix replay
remain required. A refinement-only round has no application and explicitly
refuses this export; a later actual application remains auditable after its
refinement prefix and without current evidence or helper inputs.

All 49 selected operator regression functions pass, including all thirteen new
identity cases, six original native audit cases, the causal refinement/application
case, and existing generation, prediction, qualification, recovery and resident
sequences. The original process fixture is registered and exercised by real child
processes; its standalone no-input entry is excluded from that count. All 25
public identity functions pass again with the rebuilt native library. The complete
unmodified operator test source and both public targets pass standard Cargo check.

Runtime uses standard LLVM harnesses against the qualified nightly's official
Cranelift library with 32 MiB stacks. The isolated operator harness retains every
body/assertion/fixture and changes only unselected test registrations; exact source
hashes agree before and after execution. Independent source review found no blocker.
This does not claim full LLVM native-library execution or full-suite qualification.

Earlier failed attempts are retained: the first focused run caught the real audit
gap, the old forecast fixture's incorrect implicit-expiry assumption and the
verifier's omitted process-child registration. The native audit was fixed; the
forecast test now asserts pure-time refusal separately from original acknowledged
expiry, preserving numerical/forecast work and permanent coverage loss. The
verifier retains the original child entrypoint. A setup-cleanup type error and a
verifier-only registration parsing refusal were also corrected before final success.

The full standard xtask driver compiles and executes, then refuses the unchanged
existing dependency graph before formatting, Clippy or full-suite execution. No
admission guard, production limit, journal format, effect authority or lockfile
was weakened. Details and boundaries: [computed identity operator](docs/LEARNED_IDENTITY_OPERATOR.md)
and [anchored native review audit](docs/ANCHORED_REVIEW_AUDIT.md).

This is scoped capability toward FA-118 and FA-020, not complete packet or release
qualification. Passport authentication, randomized audit-floor scheduling, full
evidence-generation propagation, H19 discrimination and a distinct public audit
representation for unattempted refinement applications remain open.

## 2026-10-23: bind complete identity registration to learned ownership

The immutable learned recipe can now require original-model computation with an
exact independently supplied passport and identity policy. The new registration
wrapper reuses the original bounded native codec; unselected and boolean-only
recipes keep their original bytes. Native learned installation checks the actual
bootstrap registration before it can create storage or return roles. Every
existing exact-recipe reader, including historical audit, therefore retains the
complete identity selection rather than only a computed-only flag.

Ordinary, raw-predictive and owned-predictive reserved creation now supports the
complete original guard inventory and returns every separate role. The original
stream, terminal reserve, source, learned owner and selected predictor become
visible together in the first canonical image. Source policies are installed
once, and exact predictor mode/work checks remain intact. Computed observations,
identity installation, native Congress and human approval remain separate.

All five new public registration functions and all twenty existing required
identity functions pass with unchanged LLVM test harnesses linked to the
qualified nightly's official-Cranelift native library. The five new cases also
pass as an ordinary Cranelift Cargo target; they cover actual same-label model
substitution, positive original two-key publication, exact registration and
legacy-byte identity, pre-measurement audit/recovery and all three reserved
source/predictor/identity lanes. The standard library compiler check and
independent source review pass.

The first all-Cranelift existing suite passed eighteen cases and failed two
intentional nested-panic catches. A minimal control reproduced that backend
limitation, and the complete unchanged twenty-function LLVM harness passes.
No assertion, production guard or numerical budget changed. Runtime tests use a
32 MiB thread stack. The failed attempt remains recorded, and this mixed-backend
runtime evidence does not claim complete LLVM code generation of the native library or
release qualification.

This is the native integration slice of FA-118, plan section 7.10 and FS-07.
Passport signing, randomized audit-floor scheduling, full evidence-generation
propagation, H19 discrimination and the complete packet remain open.

## 2026-10-23: enforce joint credibility through the learned operator

The learned operator now accepts `fa.learned-joint-publication/1`, a closed
envelope around the complete original version-1 through version-6 recipe. It
independently selects the native held-out joint policy in both live and
historical loading. The complete policy travels with the original numerical,
source, predictor and continuation configuration; mixed resident selections
refuse before startup. Existing activation still supplies independent campaign
evidence, derives capped weights and requires the original helper and human
approvals. The envelope does not supply qualification or a new authority path.

The outer command parser now forwards `--credibility-activation` to
`serve-learned-generated`, making its existing explicit-recovery path reachable.
The inner parser still refuses fresh-creation activation before opening inputs.
Both command usage surfaces and the existing operator/resident contracts describe
the option and exact joint selection.

All six new operator regression functions pass. The causal lifecycle case covers
ordinary, raw-predictive and owned-predictive publication: marginally qualifying
weights lose two complementary vetoes, joint activation refuses without journal,
epoch, charge, forecast or numerical changes, and a matched nonregressing capsule
then reaches original native review, a fresh human key and confirmed A/A output.
Additional cases cover all six recipe versions, malformed/nested/oversized
envelopes, resident policy equality, full dispatcher routing, all eight policy
fields before recovery cleanup, and offline original receipts and terminal
completion without current helper or capsule intake. Existing test bodies and
fixtures remain intact apart from a shared preparation hook and fixture visibility.

All sixteen selected operator functions pass, including those six new cases,
seven existing learned-qualification cases and three outer-parser cases. The
qualified nightly's official Cranelift backend runs an isolated registration-only
harness: all test bodies, assertions, fixtures and production are retained,
selected files are byte-identical, and source hashes match before and after the
run. The unchanged binary needs an explicit 32 MiB test-thread stack under this
verification build; its earlier default-stack overflow is retained. An earlier
preparation also stopped on an intervening public-test source change before
execution. No production limit or assertion was relaxed.

The complete unmodified operator test source and both public targets also pass
the standard qualified compiler's Cargo check. No test-registration changes
are used for that compiler check.

Independent source review found no blocking defect. Earlier complete-library
builds and a standard check were OOM-killed; those failures remain recorded.
The standard xtask command compiles and executes, then refuses the existing
dependency graph before formatting, Clippy or tests. All eleven new core/operator
functions now have passing scoped runtime evidence. Full FA-105, dependency
admission and release qualification remain open.

## 2026-10-23: bind held-out joint credibility to learned generation

`FileLearnedConfig::with_required_held_out_joint` freezes the native held-out
joint policy into the independently retained numerical recipe. All eight policy
fields, including case/member work limits, are bound alongside the complete
original configuration. Learned Enable installs the original credibility guard
inside the same candidate transition and first canonical image. The ordinary,
raw-predictive, owned-predictive and topology-mediated recovery paths retain
their existing exact-recipe checks, guards, floors and restricted consumers.
Legacy recipe bytes are unchanged; separate outer joint enables retain their
existing profile restrictions. No alternate promotion or weight setter is added.

All five new public regression functions pass for all policy-field mismatches,
omitted selection, wrapper composition, all three mediated replay/terminal
lanes, conflicting evaluation bootstraps and rollback after a later numerical
installation refusal. A causal learned-output case pairs marginally accepted
complementary-veto regression with native joint refusal, then exercises actual
nonregressing publication through original automatic and human keys. Recovery
retains reports while historical capsule retries cannot revive qualification or
old keys. Independent source review found no blocking defect.

All fifteen existing public held-out joint functions also pass. Actual execution
exposed two fixtures that attempted duplicate-root campaigns, which the original
ledger already refuses before labels or scoring. They now directly assert that
admission law and use distinct-root positive/negative controls for insufficient
evidence and per-case regression under permissive aggregate limits. The new
wrapper-size assertion now correctly includes the original four-byte blob prefix:
92 bytes of overhead. No production format, admission or scoring changed.

These twenty public functions execute as unmodified Cargo test targets with the
qualified nightly's official Cranelift backend, one compiler thread and disabled
MIR optimization after the earlier OOM-killed attempts. Failed-attempt logs are
retained. Native and operator source received independent review. The operator
integration and current compiler/gate qualification are recorded above; full
FA-105 and dependency admission remain open.

## 2026-10-23: repair interrupted learned terminal checkpoint exports

The learned operator now exposes `--recover-terminal-checkpoint` on admission
stop, later drain and combined stop. This explicit mode consumes the original
version-2 source checkpoint and matching independently retained history anchor
through the sealed native terminal cursor. It can recover after either or both
outputs failed after native acknowledgement, then export a new exact pair
without manually reconstructing a historical profile. Ordinary live, receipt
and default terminal selection retain their existing exact-cut checks. A
different Stop, mixed cuts, version-1 minima, unretained ordinary/source work
or missing independent requirements cannot select repair.

Four new operator regression functions pass using actual dispatched liabilities
and output collisions across ordinary, raw-predictive and owned-predictive
modes. They verify interrupted admission followed by repair and later drain,
completed clock-free/event-free retries, unchanged source work and documents,
native floors, independent output failures, absent live inputs, closed option
parsing, full command dispatch and staging preservation on refusal. All seven
existing paired-terminal and failed-service checkpoint regressions also pass.
The operator runtime selection changes only unrelated libtest registration in
a disposable copy; every test body, assertion, fixture and production statement
is preserved, selected files are byte-identical and input hashes are checked.
Execution uses the qualified nightly's official Cranelift backend in a private
temporary build namespace. The complete unmodified operator test source also
passes the standard compiler check. A Cargo rerun exhausted shared storage
while producing output; the same standard compiler invocation with a private
output directory completed successfully. The full xtask gate was retried and
still refuses the existing dependency graph before formatting, Clippy or tests.
Full dependency-admission and release qualification remain separate and open.

## 2026-10-23: recover retained source custody after interrupted terminal export

Ordinary and predictive mediated learned cursors can now be consumed before
replay into `FileTerminalTopologyRecovery`, bound to one stop operation. The
adapter checks the retained source checkpoint's exact original prefix against
the same locked canonical image, verifies old graph and availability, requires
the original terminal reserve and preserves or strengthens all independent
floors. Only the same Stop's closed Stop/Fence/StopProgress suffix can explain
an older cut. Ordinary events, later source work, an equal-counter fork or a
different stop refuse. Full numerical replay and original guard checks remain
mandatory; only terminal availability can advance. The resulting type exposes
native admission, drain and combined completion without any live owner,
observer, receipt-reconciliation operation or inner-cursor escape.

Five new public regression functions pass across ordinary, raw-predictive and
owned-predictive modes, including loss of the transient acknowledgement,
missing live files, true Stop-only versus Fence availability, unretained source
and time work, stronger saved and external floors, missing reserve, late cursor
conversion, same-path equal-counter forks and canonical replacement during the
clock. A Time event after a matching Stop is a causal negative, paired with a
newer-checkpoint positive. All six existing public terminal-checkpoint
regressions also pass, and the new public target passes the standard qualified
compiler check. Runtime execution used the qualified nightly's official
Cranelift backend in an isolated temporary build directory because shared disk
storage was exhausted. No test body, source check or production bound was
relaxed. These results do not close full FA-014, dependency admission or release
qualification; operator activation is a separate following change.

## 2026-10-23: preserve source work after failed learned service

Learned source retention no longer returns early merely because the service has
an error. While the original driver still owns a healthy acknowledged host, its
selected checkpoint captures the admitted documents, actual source expenditure,
native availability and exact history. The original service failure remains
first; an export error is appended and independent anchor retention still runs.
The shared path also serves resident learned sequences. Native capture continues
to reject a poisoned owner, without issuing live authority or another effect.

All three new `learned_failed_service_` regressions pass across ordinary,
raw-predictive and owned-predictive modes. They exercise actual initial bypass
refusal and loss of live inventory, retain spent work and native withdrawal,
then remove live inputs and perform clock-free, event-free terminal retries
with the exact saved pair. A late checkpoint collision preserves the competing
bytes, original error and independent anchor. A real stage-file collision
latches native storage failure and prevents either archive from being created.
An initial fixture assertion incorrectly expected bypass coverage to remain
available; source inspection confirmed `refresh_checked` explicitly withdraws
every non-Verified cut before Stop. The regression now requires that native
withdrawal while preserving all expenditure, identity and retry assertions.

The final complete operator test source passes the standard qualified nightly
check. These three runtime functions pass in the same registration-only scoped
Cranelift harness described below, with the final source hashes checked against
the frozen input. Additional existing stop-suite reruns were blocked during
fixture setup by shared storage exhaustion; they are not counted as passing.
The full dependency-admission gate and wider packet qualification remain open.

## 2026-10-23: retain paired topology custody from learned terminal commands

Learned `--stop-admission`, `--drain-stopped` and `--recover-stop` now accept
`--retain-topology-checkpoint NEW_OUTPUT` with a separately selected version-2
source checkpoint and paired `--retain-history-anchor NEW_OUTPUT`. The original
exact-cut requirements are checked before native recovery. Both destinations
must be new, distinct private files outside the authority store; canonical
parent aliases and version-1 manual minima cannot create exact source custody.
The sealed native result advances the retained source checkpoint without live
files or another journal read. Both exports and the native report are attempted
independently, preserving each failure and the acknowledged native revision.
A retained successor pair supports later drain and clock-free completed retries
in ordinary, raw-predictive and owned-predictive modes.

Four new operator regressions pass: real dispatched liability through both
phases and all predictor modes; malformed/manual/aliased output refusal paired
with a permitted command; independent output races; and broken report output.
All eight existing checkpoint creation/continuation regressions and their
synthetic helper entry point also pass against the shared private writer.
The unchanged complete operator test source also passes the standard qualified
`nightly-2026-09-08` check. Full operator code generation was killed by SIGKILL
twice on the shared host. Focused execution uses that same nightly's official
Cranelift backend and a disposable copy changing only unrelated libtest
registration, preserving all test bodies, assertions, fixtures and production
logic. Disk-exhausted attempts remain recorded; the four final cases pass with
private temporary fixtures. The full standard xtask gate was retried and still
refuses the existing dependency graph before formatting, Clippy or tests.
These results do not close the wider packet, RCH or release qualification.

Terminal persistence and paired exports remain separate writes. An earlier v2
source checkpoint becomes stale after an advancing stop even if export fails.
The operator contract now explicitly requires valid independently selected
historical custody for retry; changing output paths alone cannot bypass exact
cut matching, infer requirements or silently downgrade a profile.

## 2026-10-22: retain exact topology custody across native terminal cuts

`TopologyCheckpoint::after_admission_stop` and `after_stopped` carry an
independently retained source checkpoint to a sealed native terminal result.
The original decoder binds the bootstrap and canonical identity; exact prefix
comparison and a Stop/Fence/StopProgress-only suffix prevent later unretained
source work or an equal-counter fork from inheriting old expenditure. Complete
topology/inventory bytes, work and last source operation remain unchanged.
The exact captured anchor and native control/authority floors advance together.
Historical availability follows the real events: a recovery Fence withdraws it,
while a live Stop without that Fence preserves its historical value. No source,
observer, live owner, second semantic replay, filesystem access or clock is used.

The standard qualified `nightly-2026-09-08` compiler passes targeted checks for
the affected operator and the new public `topology_terminal_checkpoint` target.
All six public regression functions pass using that same nightly's official
Cranelift backend, including real source refreshes, missing live files,
binding/floor/graph refusals and a genuine same-path, equal-counter native fork.
The complete library unit harness reached compilation but was killed by
SIGKILL on the shared host; five additional internal cases remain unexecuted.
No assertion or production limit was relaxed. These scoped results do not
establish the full standard-backend, formatting, Clippy, RCH or release gate;
the wider FA-014 packet and compilation bridge remain open.

## 2026-10-23: execute learned terminal and authenticated stop regressions

The operator and both changed integration targets pass targeted `cargo check`
with the qualified `nightly-2026-09-08` and its standard backend. The standard
runtime build was killed by SIGKILL during library compilation on the shared
host. Installing that same nightly's official Cranelift component made bounded
runtime verification possible without changing the repository toolchain,
dependencies, source admission or qualification gates.

The complete 677-test operator harness compiles with that alternate backend.
Selected runtime results are 7/7 authenticated single-peer stop tests, 10/10
authenticated fleet-stop tests, 18/18 learned stop/export/phase tests, and 1/1
topology terminal test covering all three predictor modes. All 4/4 new public
topology phase regressions also pass, including real reserved, sent, executed
and retention-expired effects at complete terminal capacity. Another 4/4 native
forecast/continuation lifecycle tests pass, for 44 selected passing functions.
All runtime builds used `RUSTFLAGS=-Zcodegen-backend=cranelift`. Socket tests ran
with approved native execution because the sandbox rejects Unix socket creation.
One initial 60-second setup timeout passed in isolation and in the final complete
18-test selection; no deadline or production limit was relaxed.

Compilation exposed unsupported equality assertions on bootstrap profiles,
a malformed JSON fixture and an incomplete submission fixture. A test-only
helper now compares every original profile field through exhaustive patterns;
the fixtures retain their original checks. Native stop fixtures use payloads
within their existing 16-unit allowance. The topology expiry fixture derives
receipt retention from the actual original dispatch time and checks the native
`RetentionExpired` outcome directly. Expiry preserves a charged, unresolved
`Unknown` attempt; it does not invent explicit irrecoverable abandonment.
The native forecast fixture also selected an unsupported 8192-token context.
It now uses the existing 4096-token maximum with a monitoring allowance derived
from that fixed context. Its strengthened assertions require original native
model evidence, every submitted byte and prompt token, both sampled draws and
monitoring of every decoded token. No input is truncated or production bound
increased to make the fixture pass.

The required `cargo run --locked -p xtask -- check` was attempted with the
standard qualified compiler and stops at the existing lock admission refusal:
the gate admits only `fa-reference` and `xtask`, while the workspace lock also
contains `fa-journal`. That gate is unchanged. Cranelift runtime results do not
constitute the full standard-backend, formatting, Clippy, remote RCH or release
qualification. Full FA-014 and the compilation bridge remain open.

## 2026-10-22: expose learned stop and drain as independent operator actions

`create-learned-generated` now accepts explicit `--stop-admission OPERATION`
and `--drain-stopped OPERATION` terminal modes alongside the unchanged atomic
`--recover-stop`. All ordinary, raw-predictive and owned-predictive selections,
with or without topology, consume the original fully verified guarded cursor.
Admission uses only Stop/Fence and no settlement clock; later drain requires the
matching acknowledged Stop and uses only StopProgress when needed. Both retain
the original policy, predictor, topology, availability, reserve, floor and
anchor requirements and return no live custody.

The operator shares one monotonic budget from command entry through input
admission, every replay event and terminal clock acquisition. The combined and
drain consumers check both before and after their fallible clock. Late time
leaves active history unchanged for combined completion and preserves the exact
earlier stop for separate drain; no failure implicitly selects another phase.
Acknowledged anchors and native reports remain distinct from successful export.

Seven operator regression functions cover all predictor modes, actual command
parsing, real dispatched liabilities, unavailable/late clocks, replay deadlines,
missing reserve, independent guards/anchors, conflicting options, missing live
assets, output/export failures and topology source continuity. Four independent
public-library functions cover all three mediated predictor modes and real
reserved/sent/executed/expired effects, complete terminal capacity, exact anchors,
zero-event retries, failed guards and canonical replacement during clock
acquisition. These are enforcement fixtures, not detector calibration.

The untouched baseline workspace check reached an existing private-method call
in `learned_text_stream_durable/continuation/forecast/owned_native.rs`. That test
now obtains checked source shapes and acknowledged input through the existing
public evidence APIs; its native adapter checks and all prior assertions remain,
with an added exact-input equality assertion. No production visibility changed.
The operator check also exposed unresolved child modules beneath explicitly
selected source paths and a fleet-profile parameter shadowing its validator.
Explicit paths now bind all 53 affected declarations to their existing source
files, including the original test modules; the shadowing parameter is renamed.
No module or test was removed to make compilation advance.
The first targeted runtime attempt was killed by SIGKILL while compiling the
library under shared host memory pressure, before any test function executed.
The qualified `nightly-2026-09-08` targeted check now passes for the operator
example and the `mediated_learned_reserve` and `learned_text_stream_durable`
integration targets. This does not execute their tests or compile the example's
test harness. Full RCH/workspace qualification and FA-014 remain open.

## 2026-10-22: preserve the original authenticated stop-peer deadline

The live `stop-peer` operator now starts one elapsed budget before socket
connection and peer authentication, carries it through every native step, and
caps polling waits at the remaining allowance. A slow connection cannot reset
the budget or permit a new stop request after expiry. Public profile-file intake
still precedes this budget; synchronous kernel calls remain nonpreemptible.

A genuine native receipt received across the deadline keeps its original
acknowledgment, drain facts and sequence in the unchanged JSON report, while the
command returns a deadline error. A failed output also preserves the late-receipt
status in its error. No reconnect, replacement request or effect retry occurs.

Three new regression functions use authenticated named Unix sockets and the
original native authority with a real reserved publication. They pair timely
durable success with connection exhaustion before any request and a final
receipt crossing the exact cutoff. They verify canonical history, liabilities,
native receipt identity and single application. Central compilation and runtime
verification are pending at this source commit; no qualification gate or packet
is closed by source review.

## 2026-10-22: guarded learned admission stop and separate drain

The original learned stop phases now reach every guarded cursor: ordinary
learned, raw-predictive, owned-predictive, topology-mediated and predictive
topology-mediated recovery. Each terminal consumer keeps the original
`check_ready()` before delegating to the same verified machine and exclusive
canonical store. Admission-only stop uses the original two-event Stop/Fence
transaction without logical time; later drain consumes only the original
StopProgress event. Both return the exact acknowledged history anchor and no
live authority or reviewer role.

Matching admission retries append zero events, including histories with
unresolved liabilities. Completed drain retries need no clock or spare journal
capacity. Original dispatched uncertainty and executed effects keep their
charges; a stop never resends an effect. Fallible combined completion preserves
the original atomic transaction and does not silently fall back to admission.

Four additional public regression functions cover the original four
non-topology library lanes, including real reserved, sent, executed and expired
effects, acknowledged anchor reuse, failure without implicit admission fallback,
post-clock canonical conflicts and failed independent floors on all consumers.
They are source-reviewed; execution is pending at this commit. Operator phase
selection and topology regression coverage remain in progress.

The exact earlier c09def8 source now passes all 42 shutdown_fleet tests on the
qualified nightly. The next core unit-test compile exposed an opaque
LearnedPrediction equality assertion, repaired here by comparing every exposed
field, exact encoded checked source and refinement bits. Its subsequent
monolithic unit-test build was killed by SIGKILL before tests ran. No complete
core-test, formatting, Clippy, RCH or workspace-gate pass is claimed. Full
qualification and the scoped FA-014 and P0 compilation beads remain open.

## 2026-10-21: recovery/fleet compilation repair (partial verification)

The P0 compilation bridge was checked against the current source and the exact
qualified `nightly-2026-09-08` compiler
(`cea272fa356e94bd2ee2cadf376630aa0683867a`). The baseline workspace check
reported 52 errors: two missing test-module paths caused most follow-on errors,
and a review replay test accessed `FileOversight.actions` instead of its
original `machine.actions`. These are now repaired without dropping tests.
The subsequent scoped check compiled the `fa-reference` and `fa-journal`
library targets, then exposed missing fleet profile archive/custody module
paths. Those paths and the archive's nested test path are also corrected.
A subsequent xtask test compile exposed a missing Debug implementation for its
internal hash failure type; the existing hash positive control now has that
required diagnostic trait, with no change to hash/refusal behavior.

This is partial compilation evidence, not a passing workspace gate. Shared
overlay and host temporary storage exhausted during verification; a later
attempt lost its execution environment with `environment_offline`. The final
fleet path repair has not been compiled or tested. No runtime, Clippy, complete
formatting, RCH or production qualification is claimed. The baseline full gate
also refuses the stale two-package admission inventory, and baseline formatting
emitted extensive differences. Their historical records are unchanged.
The P0 bridge and full roadmap packets remain open.

## 2026-10-19: resident learned publication sequence (unqualified source)

`supervise_publication serve-learned-generated` now admits a fixed sequence of
2..=16 learned Messages and an optional final Finish, each with an independently
selected recipe and authenticated actor profile. One original locked authority,
driver, numerical generation, human reviewer, forecast observer and topology
custodian remain resident. The complete immutable generation and stream must
match; each release still needs its own original native or process congress,
fresh forecast when required, and separate human approval.

A successor requires both original Known(Executed) and native Confirmed
Executed, plus confirmed retirement of direct and process-review children.
Intermediate entries do not reopen the authority, add a recovery Fence or
requalify helpers. Original draws, forecast work, source checks and publication
charges remain cumulative. One global lifetime bounds all entries, each TTL
has a monotonic wall bound, and the initial request owns the same Stop endpoint
even during automatic failure handling in later requests. Final source/history
outputs are retained once from the original owner after the sequence.

Independent regression source covers ordinary, raw-predictive and
owned-predictive A/A/Finish, mixed native/process review, cancellation, original
prediction exhaustion, real post-Dispatch staging failure, live Stop during
request two, full profile admission, on-disk command/topology/export composition
and a short TTL under a stalled logical clock. Source review corrected typed
owned-forecast observation, per-entry wall deadlines and fixed Stop operation
before publication. All 17 changed Rust files in this wave passed nonmutating
Rust 2024 syntax parsing from their exact candidate contents. No compilation,
runtime or Clippy result is claimed: the normal sandbox cannot start on the
full device and the mandatory remote-only RCH gate is unavailable.
FA-027 is in progress; long-horizon and isolated-branch acceptance remain open.
See [resident sequence usage](docs/RESIDENT_LEARNED_PUBLICATION.md).

## 2026-10-19: live authenticated fleet Stop (unqualified source)

`supervise_publication stop-fleet FLEET_JSON` now drives the original native
stop-only protocol for a fixed independently selected roster of 1..=64 active
supervisors. Unlike offline journal recovery, it can reach the owner while its
original journal lock is held. Full manifest/profile admission precedes all
connections. Linux kernel identity, complete audience, original operation and
fresh-session binding remain on the existing checked client path.

Each connected member advances at most one native step per round with its own
work bound and the same overall elapsed budget. The complete roster reports
not-attempted, unconfirmed/may-have-sent, stop acknowledged, drain refused,
pending and drained outcomes. No failure, lost acknowledgement or failed output
triggers a reconnect, effect retry or liability refund. Synchronous file intake
and Unix connect are not preemptible; fairness applies to connected nonblocking
clients. This is a local live Stop consumer, not the complete FA-120 ordered
distributed fence, lease protocol or persistent acknowledgement frontier.

Regression source uses real file-backed live drivers and authenticated named
sockets, plus causal protocol, incomplete-drain, full-roster, time/work and
output controls. The executor was unavailable and the required remote-only RCH
gate has no available worker. No compilation, test execution, Clippy or
production qualification is claimed. FA-120 is in progress; all full packet
acceptance conditions remain open. See [live fleet usage](docs/LIVE_FLEET_STOP.md).

## 2026-10-19: complete actor/topology command admission (unqualified source)

The learned actor option parser now admits the documented `--topology-profile`
pair. Previously its closed whitelist refused combined actor/topology commands
before the topology parser, despite the underlying workflow supporting both.
Both flag orders now reach the original independent profile admission.

A full-command regression drives actual learned generation, native review,
authenticated actor ingress and independent human approval in all three
forecast modes, and checks original publication plus paired source/history
checkpoints. Invalid combinations must refuse before time observation or
durable/socket creation. The production clock is unchanged. Source review is
not type checking or execution: the workspace executor is offline and required
remote-only RCH qualification is still unavailable. FA-073 remains in progress.

## 2026-10-18: topology-mediated learned operator (unqualified source)

The original learned publication command now accepts `--topology-profile` for
creation, continuation, explicit Finish, historical receipt/reconciliation
operations, receipt service and offline stop. It reuses the existing
`fa.mediated-create/1` and `/2`, `fa.mediated-submit/1` and `/2`, original
topology source/checker and independently retained history/source checkpoints.
No new recipe, journal, wire command, dependency or numerical owner is added.

The source enters the same supervised driver before live preparation. Fresh
native certification follows qualification and precedes numerical or forecast
work; the original final publication refresh remains attached through both
native and process review. Ordinary learned, raw-predictive and owned-predictive
recovery preserve complete independently selected guards, topology,
availability, predictor, policy, reserve, anchor, source expenditure and floors.
Historical modes load no next live topology files and cannot export a fresh
source checkpoint. Reconciliation retains its original Fence/Time/Sweep;
passive receipts and direct terminal stop retain their separate original laws.

Four new operator regression functions exercise real authenticated actor and
human peers with native model review, all three forecast modes, multiple
messages and explicit Finish, exact source/history continuity, historical
receipts, offline stop/retry, initial bypass and source loss after dispatch.
Five independent public receipt functions cover all three modes at original
Sent, Published and Confirmed cuts, exact source envelopes, full capacity,
independent requirements, canonical races and unknown-liability conservation.
These synthetic models are enforcement fixtures, not calibration evidence.

All seventeen changed Rust sources passed nonmutating syntax parsing using
nightly-2026-09-21 rustfmt. Production custody and new test source received
independent review. This does not establish Rust type checking, formatting,
Clippy, runtime behavior or deployment qualification. The required RCH gate
remains blocked before compilation by the unavailable remote daemon (exit 103,
local fallback refused under AGENTS.md). The scoped beads and full FA-014/FA-073
packets remain in progress. See [operator usage](docs/LEARNED_OPERATOR_PUBLICATION.md)
and [receipt service](docs/LEARNED_RECEIPT_SERVICE.md).

## 2026-10-18: reserved mediated learned recovery (unqualified source)

The original topology-mediated learned and predictive learned constructors now
have explicit reserved variants. Their existing guarded bootstrap installs the
terminal recovery reserve before guard and numerical work and publishes the
complete configuration in one first canonical image. Ordinary, raw-predictive,
and owned-predictive modes retain their exact original recipe and role custody.
Older constructors and journal formats are unchanged.

Both sealed mediated recovery cursors can require the exact independently
selected reserve before replay. Their original full guard, topology, predictor,
policy, floor, and anchor checks now also protect message/finish receipt
selection, immutable actor receipt ports, explicit reconciliation, and direct
stopped recovery. No ordinary recovery fence consumes terminal capacity before
the original Stop/Fence/StopProgress transaction, and no live role escapes a
receipt or stopped finalizer.

Four independent public regression functions cover all three modes, pairing
useful numerical output and full-capacity shutdown with startup, reserve, and
recovery refusals. These tests and the changed library sources passed
nonmutating syntax parsing with
nightly-2026-09-21 rustfmt. This is not type checking, format qualification, or
test execution. RCH exits 103 because the remote daemon is unavailable and
refuses local fallback as required by AGENTS.md. Compilation, Clippy, runtime
tests, full packet acceptance, and production qualification remain open.
See [ordinary mediated recovery](docs/MEDIATED_LEARNED_RECOVERY.md) and
[predictive mediated recovery](docs/PREDICTIVE_MEDIATED_LEARNED_RECOVERY.md).

## 2026-10-17: explicit BOS tokenizer compatibility (unqualified source)

The original bounded byte-BPE engine now has an explicit Llama-3/Unicode-17 BOS
profile. Its closed importer accepts one precisely bound BOS template and the
converter's inert ByteLevel wrapper. It preserves original token IDs and literal
control occurrences, prepends a synthetic `0..0` token span, and retains the
original source, work, output and decoder limits. The FA-BBPE/6 archive binds the
prefix and original BOS ID; formats /1 through /5 are unchanged.

The same selection reaches native helper bootstrap and checkpoint configuration,
publication input, and learned monitor training. Regression source checks exact
encoding and archive behavior, worker Allow/Deny and malformed intake, monitored
training/artifact binding, and original publication/recovery. This does not
expand the supported architecture or model/vocabulary bounds, provide a general
template interpreter, or establish empirical detector quality. See
[the exact tokenizer contract](docs/UNICODE_PRETOKENIZATION.md).

FA-025 and FA-026 remain in progress. The remote-only gate command using the
qualified `nightly-2026-09-08` was attempted with `RCH_REQUIRE_REMOTE=1`;
RCH exited 103 and refused local fallback because the daemon was unavailable.
After the execution server restarted, all eighteen changed Rust sources passed
nonmutating syntax parsing with rustfmt from the qualified-date toolchain. Test
source has been reviewed, but syntax is not type checking or execution;
compilation, formatting, Clippy, runtime tests and release qualification remain
open.

## 2026-10-17: process-backed learned publication (unqualified source)

The original `FileLearnedProcessReview` now runs through the supervised actor
driver and the explicit `fa.learned-publication/6` operator recipe. External
helpers receive the exact learned input through the existing private-socket
commit/reveal protocol. A witnessed refinement updates the pending actor job;
the next fixed roster starts only after predecessor children have been reaped.
An original permitting receipt returns the same driver to independent human
approval and its existing automatic, current-input and publication checks.

The operator validates complete fixed rosters and aggregate start bounds before
model ingestion, retains live post-launch and post-I/O clocks, polls in bounded
short intervals, and carries actual child custody through stop and cleanup.
Receipt-only recovery can read a recorded effect without executable preflight
or a live helper. Old /1–/5 recipes retain their original semantics; /6 reuses
finite numerical continuation and can select the existing required forecast.

Six substantive public-driver tests and operator regressions are authored,
including real child processes and an original publication, with causal
controls for deny/EOF, cancellation, missing evidence, exact clock cutoffs,
roster limits and missing executables during receipt recovery. Synthetic
ballots exercise protocol and authority behavior, not detector quality.
The twelve changed process Rust sources passed nonmutating syntax parsing with
qualified-date rustfmt. Source review and syntax do not establish runtime
qualification. The required remote
RCH attempt refused local fallback with its daemon unavailable (exit 103);
compilation, Clippy, runtime tests and release qualification remain open.
FA-018, FA-019, FA-107 and FA-113 stay in progress: this does not establish
host containment, judgment independence, residual-channel measurements or
held-out sidecar quality. See [the process publication contract](docs/PROCESS_LEARNED_PUBLICATION.md).

## 2026-10-14: reconcile stream receipts through verified recovery (unqualified source)

The anchored stream receipt service now has an explicit `--reconcile-receipts`
mode. After exact history, guard, floor and reserve replay, it requires the selected
request to exist before any recovery mutation, then uses the original fixed
`Fence -> Time -> Sweep` transaction before exposing the non-admitting stream
receipt port. Missing requests therefore consume no fence or clock. Receipt-only
mode retains its no-clock behavior; reconciliation neither reads fresh evidence
nor launches helpers/reviewers and cannot recreate effects. Published unresolved
messages can become confirmed only through the original endpoint receipt sweep;
unresolved outcomes stay charged and unknown.

Two new regressions exercise confirmed recovery of a published message and the
missing-request/no-clock boundary, while existing stream recovery tests are
strengthened to require no mutation for absent keys. The required RCH build/test
gate remains unavailable, so this is unqualified source and no FA-014/FA-107
closure or production qualification is claimed. See
[the stream receipt contract](docs/STREAM_RECEIPT_SERVICE.md).

## 2026-10-12: recover learned histories directly into stopped state (unqualified source)

All four original learned replay cursor types now expose exact stop selection
and a consuming stopped completion. Fully verified numerical state enters the
existing fixed Stop/Fence/StopProgress transaction without another replay or an
ordinary recovery fence first. Source, event/byte limits and original reducer
checks remain shared with ordinary terminal operations. Guarded variants retain
their complete independent requirements; exact anchors and terminal reserve
remain on the same exclusive canonical history.

The learned operator's offline `--recover-stop OPERATION` uses those cursors with
its one-event elapsed replay checkpoints. It loads the original numerical and
predictor recipe without live native helpers, policy evidence, reviewer sockets
or a credibility capsule. It returns only the existing native stop report and
preserves executed/unknown charges. A matching completed retry needs no clock,
event or replacement. Input anchors remain usable; successor anchor output is
explicitly rejected before startup because no live owner escapes this path.

Fourteen new regression functions cover original stopped completion at full
capacity; reserved, sent, executed and expired liabilities; stale/foreign stop
intent; before/during-clock canonical changes; all five Store barriers; exact
retries; every operator replay checkpoint; missing live assets; original guarded
refusal; and output loss after acknowledgment. Root and an independent agent
reviewed the implementation and tests. The required remote-only RCH gate and
runtime tests remain pending, so this is unqualified source. The scoped work and
full FA-014 prerequisites remain open. See [the terminal contract](docs/LEARNED_STOP_RECOVERY.md).

## 2026-10-12: requalify learned helpers for resumed publication (unqualified source)

The learned operator now accepts the original independent held-out credibility
capsule for explicit generation continuation, a next message or stream finish.
Activation uses the original bounded decoder, exact committee/evidence binding
and native epoch transition after guarded recovery and before new inference or
request admission. An old activation cannot clear recovery invalidation. Missing
qualification and invalid message predecessors preserve their early refusal and
corrected retry behavior. Initial creation refuses a capsule before I/O; receipt
recovery ignores it without reading new evidence.

The original stop/deadline service surrounds activation, and subsequent work
still obtains fresh policy evidence and time, native review and a human key.
Paused numerical state, pending intent, sampling/work counters, old charges and
forecast history remain intact. Requalification cannot renew a lost pending
forecast. Errors after explicit live preparation use the existing stop/drain and
cleanup path rather than starting a replacement numerical generation.

Seven new regression functions exercise actual native/human second-message
publication in all three modes, historical/malformed/wrong capsules, missing
source, deadline and real authenticated stop ordering, pending forecast/intent
preservation, the actual caller's omission retry, and receipt-only CLI recovery
with capsule and helper files absent. Synthetic campaigns establish enforcement
cases, not evaluator authentication or statistical calibration. The required
RCH gate and runtime execution remain pending; full FA-105 and its prerequisites
remain open. See [the operator contract](docs/LEARNED_OPERATOR_PUBLICATION.md#requalify-helpers-after-recovery).

## 2026-10-12: enforce deadline and authenticated stop during learned replay (unqualified source)

The actual learned continuation, next-message and finish command now connects
the original one-event recovery checkpoints to its existing deadline and stop
transport. Checks occur before guarded open, before every original event and
at Ready before the single fence. An authenticated pending peer interrupts
replay without canonical/staged-file changes or role issuance. It does not apply
or acknowledge a durable Stop: the recovered owner needed for that protocol
does not yet exist. Rejected peers retain the original admission-quota behavior.

Message and finish receipt recovery instead enforce a single monotonic elapsed
budget, with no supplied logical-clock calls, live evidence, new forecast or
renewed request deadline. Exact anchors, guard inventory, reserve, retained
numerical work and final anchor retention remain on the original owner path.
One event and initial/final synchronous storage work can exceed a time budget;
these are cooperative event boundaries, not an interruptible-kernel guarantee.

Seven new regression functions cover cancellation at every boundary, actual Unix
credential admission, the real command entry and original whole-batch numerical/
fence parity. Six changed Rust files passed older, unqualified syntax parsing
only. Target and full-gate RCH dispatches both stopped before compilation because
the launcher is absent (exit 127); tests remain unexecuted. Scoped work remains in progress;
the full FA-014 packet and its prerequisites remain open. See
[the recovery contract](docs/COOPERATIVE_LEARNED_OPERATOR_RECOVERY.md).

## 2026-10-12: retain and require independent learned operator history (unqualified source)

The existing learned publication command now accepts an independently retained
exact prefix through `--history-anchor` and can export a checked successor to a
new private file through `--retain-history-anchor`. Legacy, raw-forecast and
owned-forecast recovery use their original anchored guarded cursors with the
same model/predictor, complete guards, policy, numeric floors and terminal
reserve. Genuine successors recover; valid equal-counter divergent histories
and older prefixes refuse before fencing or staged-file cleanup.

Final retention captures only the healthy acknowledged original owner, preserves
the selected old archive and requires exclusive output creation plus file and
parent-directory sync. Retention failure keeps the original actor receipt and
effect state, adds an explicit failure and still cleans up; it sends no retry.
Receipt-only recovery can advance the retained prefix without live inference,
source or review. Early failures or a crash can leave no new archive: retention
is not atomic with the original publication or its recovery fence.

Eleven new regression functions cover original loaded owners and native publication, exact
successors versus valid forks/rollback, unchanged staging on refusal, closed
file intake and output races after publication. Seven changed Rust files passed
older, unqualified rustfmt syntax parsing only. Both targeted and full-gate RCH
dispatches stopped before compilation (missing launcher, exit 127), so no
compile/test or release qualification is claimed. The scoped implementation does not establish custodian
authentication or protect a jointly rolled-back journal and anchor. The scoped
work remains in progress; full FA-014 acceptance and prerequisites stay open. See
[the operator contract](docs/LEARNED_OPERATOR_HISTORY_ANCHORS.md).

## 2026-10-12: drive required forecasts through learned publication (unqualified source)

The executable's new `fa.learned-publication/5` recipe requires an independently
supplied `fa.learned-forecast/1` artifact. Closed parsing and original predictor
validation precede weight intake; the actual imported model then checks full
decoder identity, selected residual/K/V profile and dimensions, source stream
and publication stream before durable startup. Exact finite binary32 words and
the original prediction, owned-work and aggregate asset allowances are retained.

The supervisor holds separate observer custody and drives original raw or owned
forecast handles at the complete-prompt and receipt-bound successor boundaries.
Forecast acquisition yields another ordinary deadline, stop and fresh source
quantum before sampling. Finish uses a separate original forecast with no new
inference. Every release still requires original native review, fresh human
approval and confirmed publication. No observer or forecast input enters the
source-only actor wire.

Selected guarded recovery matches the complete independent predictor/generation,
guards, policy, external floors and terminal reserve. It preserves unanswered
forecasts and spent work, refusing renewed generation after lost coverage.
Original terminal message/finish receipt recovery performs no new forecast,
inference or review. Earlier recipe meanings and immutable history bytes remain
unchanged.

Eight loader tests and seven loaded lifecycle tests are authored and independently
source-reviewed. The positive uses actual original numerical and native reviewer
owners for two messages and separate finish in both modes. Causal controls cover
missing custody, expiry or missing policy evidence before the first draw, owned
forecast budget exhaustion, interrupted forecasts and changed configuration.
The prescribed remote launcher is absent. After the complete operator source
was frozen, the workspace executor disconnected before this batch's syntax and
remote-dispatch attempts could start. No operator parse, Rust compilation,
new-test or workspace pass is claimed. Reviewed source was recovered from
retained reads and exact edits for direct GitHub publication. The fixture
probabilities do not qualify calibration or detection quality, and FA-111
remains in progress. See [required operator forecasts](docs/LEARNED_OPERATOR_FORECASTS.md).

## 2026-10-12: preserve terminal recovery capacity for predictive streams (unqualified source)

Predictive learned STREAM startup now accepts an explicit recovery reserve in
the original first canonical image, returning the host, human reviewer and
separate consistency observer only after persistence. Required raw-residual and
owned-K/V predictors use the original numerical and forecasting paths.

Both predictive guarded recovery wrappers can require that exact reserve before
replay or staged-file cleanup. They share the original ordinary learned recovery
check; full recipe, guard, policy, floor and canonical-cut checks still apply.
Recovery retains numerical work and forecast evidence, rotates observer custody
and does not repair unanswered forecasts or replenish any allowance.

Five new public test functions cover both modes, original-output parity,
bootstrap refusal, changed/missing/late reserve requirements, unmet floors,
old-observer refusal and actual terminal drain after ordinary capacity is full.
They are authored and source-reviewed, but unexecuted: the prescribed remote
build launcher is absent in the restored workspace. No compiler, target-test or
workspace pass is claimed. FA-111 and the scoped bead remain in progress.
See [the predictive reserve contract](docs/PREDICTIVE_RECOVERY_RESERVE.md).

## 2026-10-11: bounded exhaustive quotient replay (unqualified source)

FA-093 now has a public closed logical replay oracle over the original FA-092
independence relation. It validates declared access coverage, canonicalizes only
lawful independent swaps and compares every bounded unreduced permutation with
a cold replay of its representative. The comparison includes original
consequence decisions and ordered observations as well as final state. Failures
retain per-occurrence results and the complete population denominator.

Admission bounds all schedules, replay/canonicalization/comparison work and
modeled retained payload before execution: at most eight occurrences, 40,320
orders and 32 cells. Reports expose original denominators, representative counts,
logical work and order-only trace bytes; they make no process-memory, latency,
production-executor or arbitrary-concurrency claim.

Fourteen public integration tests and one private causal mismatch test are
authored, including independent permutation/swap controls, cross reads, closed
absence, resource/RNG mutations, ordering barriers, malformed bindings and every
budget dimension. None has executed in this session. The unchanged remote-only
RCH gate could not start compilation, and the workspace executor is unavailable.
No source-format, compile, test or benchmark qualification is claimed. FA-093
remains in progress. See [the replay contract](docs/QUOTIENT_REPLAY.md).

## 2026-10-11: require fresh forecasts across finite learned messages (unqualified source)

The explicit `with_forecast_text_stream_continuation()` library recipe composes
the original required raw or owned learned predictor with finite continuation.
Every later message must first record its original receipt-bound request and
numerical cut, then forecast that exact cut before another sampled token.
Original source audits, cumulative numerical work, sampler history and lifetime
sequential evidence remain shared. The actual release frame distinguishes later
messages from an independently forecast finish; a pending finish obligation
cannot be converted into a continuation.

Old configuration bytes, prompt-only refusals and executable recipe `/4` retain
their meaning. This adds no prompt, context extension, token budget, permission,
predictor provisioning for the executable, or recovery of unanswered forecasts.
Seven new public integration tests cover both predictor representations, real
A/B numerical output and separate receipt paths, missing/wrong/late obligations,
finish substitution, unconfirmed publication, expiry and interrupted recovery.
Their forecast tables and committee ballots are controlled fixtures.

Qualification is pending. The unchanged remote-only RCH gate failed before
compilation with no configured workers and no daemon; the workspace executor
then disconnected. No formatter, compilation, new-test or workspace pass is
claimed. The scoped bead and FA-111 remain in progress, including the original
research acceptance. See [the exact contract](docs/FORECAST_TEXT_STREAM_CONTINUATION.md).

## 2026-10-11: adapted checkpoint publication and receipt-bound finite continuation

Learned publication recipe `/3` and native roster `/2` accept an explicitly
selected `fa.checkpoint-weights/1` manifest. Primary generation and actual native
reviewers now use the same original single/sharded readers and static LoRA
merger as the training commands. Complete metadata admission precedes tensors;
all base and adapter reads share the original byte/read-call allowances and
consume the operator's remaining aggregate asset allowance, including on failure.
An ordinary path remains a single weight file, with no schema or filename fallback.

Primary startup compares the final effective model with independently supplied
actor model/tokenizer generations, including the audit-only restart profile.
Recovery compares the complete original effective model, tokenizer and learned
configuration. Equivalent repacking from shards to a single checkpoint retains
that identity. A changed parameter refuses even if the saved prompt's sampled
text would be identical. Live native assets are not needed for terminal receipt
recovery, which does not rerun generation or review.

Recipe `/4` additionally admits finite message continuation. After a confirmed
Executed message receipt, `--after-message PREVIOUS_REQUEST` can use the remaining
part of the same originally admitted generation. The durable owner records the
exact predecessor, successor request and numerical position before more inference.
It preserves the full accepted-token history, K/V state, sampler draws and spent
work, advances only the current text offset and confirmed audience basis, and
requires the successor's own native and human approvals. Source-only, generic
request and direct proposal admission enforce the same retained request binding.
An explicit finish adds no inference but remains a separately reviewed effect.

Old configuration bytes and journal tags retain their interpretation. The new
configuration wrapper and continuation event are explicit. Exact retained retries
write nothing; semantic recovery revalidates the original receipt boundary.
Held or failed work, terminal token limits, pending sends, stale predecessors,
changed configuration and exhausted original allowances cannot create another
message. Required pre-output forecast modes do not define later message forecasts
and are refused in both configuration orders and independent installation.

Qualified `nightly-2026-09-08` passes all 24 public durable learned-stream tests,
including nine new continuation controls, and all 22 selected forecast/reset
and mediated-recovery compatibility tests. The final learned-operator selection
passes 39 of 41 tests, including all four new checkpoint-source and three new
continuation functions. Its two existing Unix-listener tests fail at listener
creation with environment EPERM; neither is skipped or replaced. The loaded
continuation positive executes both messages and finish with actual native
review, paid progressive residuals, fresh human keys, exact numerical recovery
and receipt charges. All source, sampling and journal ceilings remain intact.

The fresh unchanged `xtask check` passes the exact compiler identity and initial
source/dependency checks, then refuses the reviewed source snapshot at its
512-file discovery ceiling, depth-eight limit, unlisted files and stale recorded digests. Its
enforcement and reviewed records remain unchanged. Formatting, Clippy and the
complete workspace test gate are not reached or claimed to pass.

This is bounded continuation within one generation. It appends no new prompt,
creates no new funding, resets no policy-source history and leaves stream
checkpoint/reset unsupported. The lifetime observation allowance includes all
native token-level review polls and the final stream closure. Numerical fixtures
establish contract behavior, not alignment-detector quality or deployment
qualification. See the [publication contract](docs/LEARNED_OPERATOR_PUBLICATION.md),
[checkpoint sources](docs/CHECKPOINT_SOURCE_MANIFEST.md) and
[finite continuation](docs/LEARNED_TEXT_STREAM_CONTINUATION.md).

## 2026-10-11: sharded and adapted checkpoints reach both learned training commands

Both executable training examples now accept an optional leading
`--checkpoint-source` switch. Their existing weights operand then names the
closed `fa.checkpoint-weights/1` manifest: an explicit single or indexed base
and an optional saved static LoRA adapter. Their existing positional operands,
training/campaign schemas, numerical owners and output formats retain their
meaning. Ordinary file operands are never sniffed or retried as manifests.

One example support module composes the original configuration, shard reader and
adapter merger. It checks complete index labels, adapter base/final identity,
shape and merge-work admission before opening tensor paths. All explicit handles
pass regular-file and aggregate-size admission before base scalars. Base and
adapter consume one original read-byte/read-call budget conjoined with the
caller's limits. Manifest, index and adapter configuration consume the same
existing auxiliary allowance; failures retain original work and return no model.
No decoder, adapter or resource ceiling is increased.

Actual original codec output files feed the actual probe-training command and
publication monitor parser. The effective model must match the independently
negotiated decoder/cache profiles and the complete original trained-monitor
binding. Reports retain actual source work, including original adapter merge
reservations without treating partial failure as successful parameter updates.

Qualified `nightly-2026-09-08` passes all 32 codec-example tests, including the
same 16 probe-example functions. The separately executed probe target also
passes all 16: 32 distinct regressions, nine new for this source route. Tests
exercise all single/sharded and absent/nonzero-adapter combinations against
independently specified effective tensors and original sampled quiet/hold
oracles. Changing only the last value adapter preserves the other three mandatory
tap results while refusing monitor export and old binding. Exact/one-short
caller and manifest byte/read-call limits, auxiliary accounting, merge work,
pre-tensor metadata refusal and old invocation semantics also pass.

These synthetic controls establish executable composition and complete binding,
not learned detector quality, input authenticity or deployment qualification.
The original complete project gate and its reviewed source snapshot remain
unchanged and separate from targeted execution. See the [checkpoint source
contract](docs/CHECKPOINT_SOURCE_MANIFEST.md) and both linked command documents.

## 2026-10-11: sharded base checkpoints with static LoRA reach native workers

The actual `fa.native-worker/4` path now accepts its existing explicit sharded
weights declaration together with the mandatory static adapter. The new
`NativeEvaluator::from_llama_lora_shard_files` composes the original complete
index reader and immutable LoRA merger, retaining their actual receipts. No
unadapted helper, partial model or supplied verdict can escape this startup.

Base/adapted identity, configuration, final tokenizer/policy, adapter shape and
merge-work checks precede weight reads. The original index checks the complete
label set before opening any tensor path. Every base handle and adapter handle
passes file/size admission before the original all-base-header reader starts
scalar ingestion. All base shards and the adapter consume one supplied weight
read allowance; the original merge budget remains spent after numerical or
subsequent monitor failure. Adapter headers retain their original later position
after loading the base; this is not a cross-file transaction or atomic snapshot.

Qualified `nightly-2026-09-08` passes all nine new regression functions, 15
existing spawned-worker LoRA/checkpoint/static-RoPE tests and all 110 native
helper unit tests: 134 selected tests. Rank-one/rank-two, native/raw-HF and
zero/nonzero adapter pairs reach actual Allow/Deny and commit/reveal. Removing
the monolithic base prevents an accidental single-file fallback. Tests retain
causal controls for missing later shards, late malformed directories before
earlier bodies, exact/one-short bounds, adapter overflow, post-merge monitor
failure, complete-input mismatch, terminal holds and process-step exhaustion.

The existing single-file path, older schemas, complete input ownership and
cooperative deadline remain unchanged. No model or resource maximum increases.
Targeted execution establishes numerical/protocol composition on synthetic
fixtures, not trained judgment quality, asset authenticity, process isolation
or complete project qualification. See [sharded native LoRA
startup](docs/SHARDED_LORA_NATIVE_WORKERS.md).

## 2026-10-11: exact Unicode tokenization reaches native workers and publication

The original bounded `ByteBpe` now admits explicitly named GPT-2 and Llama-3
Unicode-17 grammars. A finite scanner freezes boundaries in the original byte
nodes before the existing ranked merge engine runs. It preserves original IDs,
complete byte spans, literal named-control isolation, independent BPE budgets
and cooperative cursor ownership. Old constructors and raw-byte archives retain
their existing behavior, including arbitrary non-UTF-8 bytes. Unicode requests
require complete UTF-8 and never normalize input or insert prefix spaces.

Official Unicode 17 category, White_Space and default simple case-fold tables
are frozen with exact source hashes and the complete Unicode License V3. An
offline, hash-pinned development generator adds no runtime or build dependency.
An independent exhaustive comparison validates all 1,114,112 code points and
every admitted fold mapping. These are explicitly versioned input semantics,
not an assertion of parity with every upstream regex engine's Unicode version.

Native single/sharded checkpoint startup, `fa-native-helper` manifests, the
monitor-training command and native publication now select either exact named
Hugging Face pipeline explicitly. No parser fallback or filename inference is
added. `FABBPE05` extends the original archive with six fixed interpretation
bytes; complete trained-monitor and durable-generation bindings already compare
that same canonical archive. A grammar-only substitution therefore refuses even
when a particular saved prompt would happen to produce identical token IDs.

Qualified `nightly-2026-09-08` passes 14 Unicode integration tests, all 61
tokenizer unit tests, all 13 native worker configuration tests, all 11 monitor
training command tests and all 23 codec command tests (including those same 11
monitor tests). A new actual publication test runs both grammars with both human
decisions, pauses after an original sampled token, rejects a changed grammar
without changing journal bytes, then resumes exact numerical work and records
the original reviewed outcome. The independent numerical fixture holds weights
and prompt fixed: raw BPE merges `a1` and computes Allow, while either Unicode
grammar separates the original IDs and computes Deny. Cooperative admission
preserves the synchronous verdict and work while yielding before numerical work.

The final integrated public selection passes 46 tests across Unicode input,
cooperative native preparation, trained-monitor binding and durable learned
streams; all 110 native-helper unit tests pass on the integrated source. The
broader learned-publication selection passes 32 of 34 tests. Its two existing
real Unix-listener tests fail at listener creation with environment EPERM.
The unchanged original project gate passes compiler identity and then refuses
the reviewed source snapshot: the 512-file discovery ceiling, unlisted files
and stale existing digests prevent later workspace qualification. Formatting,
Clippy and the complete workspace gate are not claimed to pass. Snapshot
enforcement and its reviewed records remain unchanged.

The initial bounded UTF-8/control/segmentation scans remain synchronous. These
input grammars do not add a GPT-2 numerical architecture, increase the current
decoder's model/vocabulary limits, or implement normalization, arbitrary regex,
chat templates or SentencePiece. Synthetic controls establish executable
composition and complete binding, not trained detector quality, asset
authenticity, latency guarantees or complete workspace qualification.
See [exact Unicode pretokenization](docs/UNICODE_PRETOKENIZATION.md).

## 2026-10-11: native worker deadlines cover cooperative prompt preparation

The actual `fa-native-helper` process now uses `step_cooperative`: at most 256
advances of the original tokenizer per peer call, charged to its existing step
allowance and checked against its original absolute deadline. The evaluator's
explicit `Admitting` state freezes the complete original `WorkerInput`, including
binary bytes, ordered parts and omissions. It moves the completed encoding
through the original text-admission handoff and yields before the first native
token. No tokenizer, numerical engine, worker protocol or manifest is replaced.

Progress retains original tokenization work and cooperative advance attempts
separately from unchanged numerical work. Cancellation and caught interruption
close the original observation source and keep spent work without rerunning the
input. An expired deadline, exhausted allowance or incomplete monitored answer
cannot issue a vote. Already sent commitment bytes remain sent after a later
step limit. Fresh synchronous evaluator/peer entry points and the learned
sidecar's original source polling sequence retain their existing behavior.

All nine new public tests, 110 native-helper unit tests and 15 executable-worker
regressions pass on qualified `nightly-2026-09-08`: 134 selected tests, including
the five new private deadline/interruption tests. An independently framed
403-byte binary input requires 320 real merges and 83 prompt tokens; changing
its final byte drives actual Allow/Deny. Exact and one-short input, lookup,
heap, context and process budgets, expiry mid-BPE and at handoff, terminal holds,
unwinds, observer closure and retained sent commitments are exercised without
weakening the original assertions. Initial bounded copies/scans, final admission,
individual numerical tokens and filesystem operations remain nonpreemptive.
These results establish bounded reference behavior, not detector quality,
process independence, latency guarantees or complete project qualification.
See [cooperative native input](docs/COOPERATIVE_NATIVE_INPUT.md).

## 2026-10-11: checkpoint token documents to usable learned codec files

`train_learned_kv_codec_from_checkpoint` fills the missing first asset-producing
step of the learned operator workflow. It imports actual Llama configuration and
Safetensors, captures every document in its own empty original decoder cache,
and drives the original cooperative codec-training owner through its complete
centered-covariance/Jacobi fit. It saves the unchanged independent
`fa.learned-fit-binding/1` and original `FAKVFIT/1` archive consumed by both the
K/V monitor trainer and learned publication operator.

The complete token corpus, original inference and fitting costs, retained image
size and output reservations are admitted before weight loading. The original
fit admission now exposes its same arithmetic as a read-only estimate; decoder
cache contracts can likewise be obtained from the independently negotiated
profile without allocating model parameters. Actual fitting still validates
the real complete images. No supplied K/V arrays or learned coefficients replace
the original capture or fitter.

Exclusive new files preserve completed artifacts and bounded diagnostic reports
on later failure. Reports retain exact original work, unfinished documents and
numeric failures. A closed progress sink cancels active capture before further
numerical work; no retry, refit, overwritten output or refreshed budget occurs.
Separate file writes are not an atomic multi-file transaction.

All 12 new command tests, ten existing monitor-command tests and 39 original
codec-training, fit archive/replay, decoder and static-RoPE regressions pass under
the qualified `nightly-2026-09-08`. They consume the actual newly saved files through
the existing trainer and operator, with independent sampled quiet/hold oracles,
per-document empty-cache checks, changed source/witness controls, every original
work dimension one unit short, numeric failure, cancellation, output races,
wrong identity/origin and complete decoder profile checks. Synthetic fixtures
establish executable composition, not corpus authenticity or detector quality.
Full project qualification remains subject to the unchanged original gate.
See the [codec training command](docs/TRAIN_LEARNED_KV_CODEC_COMMAND.md).

## 2026-10-11: checkpoint files to complete evaluated K/V monitors

The new `train_learned_kv_probes_from_checkpoint` command joins actual Llama
configuration/Safetensors, an explicit tokenizer, fixed labelled token cases
and independently bound codec fit inputs to the original complete K/V campaign.
It negotiates the model shape and admits all capture, fitting, calibration,
final-scoring and report allowances before opening weights. The original decoder
estimator now also lives on `DecoderProfile`; model execution delegates to that
same unchanged formula. Original bounded readers, codec refit, corpus, optimizer,
calibrator and exact scorer retain ownership of every numerical transition.

The command saves the fixed population and policies before numerical work, then
retains actual capture work, coefficients, exact scores, threshold trials,
failed/unevaluated final denominators and admitted/completed/remaining budgets.
A failed mandatory tap blocks the complete monitor export while preserving the
other results. A completely accepted campaign exports the original `FAKVMON2`
artifact for the learned operator. Reports and artifacts use distinct exclusive
new paths; output failures never overwrite an existing file, retry numerical
work or refund its consumed allowance. Two synced files are not a transaction.

All ten new file-backed command tests pass on qualified `nightly-2026-09-08`.
They execute original sampled quiet/hold through the actual operator loader and
include causal final-label, last-value-projection, every-budget-dimension,
wrong-asset, corrupted-fit-witness, numerical-overflow and output-race controls.
The final selected public decoder, static RoPE, configured-reader, campaign,
codec/replay and durable-stream integration suites pass all 91 tests.
Final targeted execution totals 133 passing runtime tests: those 91, ten training
command tests and 32 learned-operator tests. Two existing real Unix-listener
tests fail at listener creation with environment EPERM. The unchanged original
project gate passes qualified compiler identity, then refuses its reviewed
source snapshot: the 512-file discovery ceiling, numerous unlisted files and
stale existing digests prevent later workspace qualification. Formatting, Clippy
and the complete workspace gate are not claimed to pass; snapshot enforcement
and its records remain unchanged.
These synthetic fixtures establish executable composition, not detector quality
or label authenticity. Complete artifact size remains capped at 16 MiB; regex
pretokenization and full pretrained/native text semantics remain unfinished.
See [checkpoint K/V training](docs/TRAIN_LEARNED_KV_PROBES_COMMAND.md).

## 2026-10-11: complete model and tokenizer binding for trained monitors

Accepted K/V campaigns can now export `FAKVMON2`: the unchanged canonical v1
monitor JSON, the exact original tokenizer archive and complete original
model/codec/policy comparison bytes. The public verifier checks every parameter,
including unused weights and signed zeros, complete tokenizer semantics, codec
basis and fit provenance, all probes and every operational cap. Matching model
labels or matching logits on the campaign cases cannot hide a substituted asset.

The actual learned operator consumes this artifact with its independently
imported model, tokenizer and codec before generation or store creation. Its
original parser and monitor constructors remain the numerical owners. V1 JSON
retains its existing provisioning behavior and 2 MiB ceiling; the complete
artifact is bounded at 16 MiB and charged to the existing aggregate asset cap.
The shared original model/policy writers retain generation replay byte order.
The artifact compares intended assets; it authenticates neither its provenance
nor the campaign's labels or acceptance. Oversized full bindings refuse.

All eight new public artifact tests and 44 existing campaign, codec and replay
regressions pass on qualified `nightly-2026-09-08`. All three new actual operator
loader tests pass, covering complete binding and real sampled quiet/hold behavior.
The full learned-command selection passes 32 of 34 tests; the same two existing
Unix-listener tests remain blocked at listener creation by environment EPERM.
See [bound trained monitors](docs/BOUND_TRAINED_KV_MONITORS.md).

## 2026-10-11: separately reviewed learned stream finish

The learned publication operator now accepts an explicit
`--finish-after-message PREVIOUS_REQUEST` with a distinct finish request and
fresh native review schedule. It reopens the exact original numerical recipe,
requires the predecessor's accepted Executed receipt and complete confirmed
stream boundaries, then acquires current policy and resumes the same paused
owner without computing another token. The finish traverses the original
source-only gateway, complete learned sidecar, native congress, independent
human key and current-policy publication gate. It closes a full message stream
without needing another message slot, while charging the complete finish frame.

Receipt-only finish recovery retrieves the original pre-policy request, including
an admission-capacity refusal with no FrozenAction. It pins the exact preceding
message receipt, target version, cumulative boundaries and original deadline;
changed TTL cannot renew the request. Terminal recovery needs no live evidence,
helper assets or clock. Original numerical work, sampler state, source history,
recovery reserve and unknown publication obligations remain with their owners.
No journal format, permission type or numerical engine changes.

All eight new finish tests pass on qualified `nightly-2026-09-08`, including the
two actual native reviews, separate human decisions and the full one-message
capacity case under the unchanged 1024-event source ceiling. The complete
learned-command selection passes 29 of 31 tests; the two existing real
Unix-listener tests still fail at listener creation with environment EPERM.
All 15 durable learned text stream integration tests also pass.
See [learned stream finish](docs/LEARNED_STREAM_FINISH_OPERATOR.md).

## 2026-10-10: complete trained K/V probe campaigns and operator export

`probe::training::decoder::kv` now executes a fixed labelled population through
the original immutable decoder and captures the actual final-token key and value
SourceFrames for every layer. The new complete K/V corpus reuses the original
fitter, threshold calibrator and exact held-out evaluator. GQA taps use actual
cache widths. All three splits retain both labels, with exact-history and declared
task/lineage reuse refused before capture. The complete capture and fit/scoring
allowances are charged before work and remain spent on later numerical failure.

A campaign exports only when every mandatory layer/side passes its independent
final evaluation. Failed taps and their denominators remain in the report; callers
cannot drop a failed side or supply replacement coefficients. The accepted roster
constructs the original LearnedModelMonitor/LearnedDecoderPolicy and emits the
unchanged `fa.learned-kv-monitor/1` input consumed by the learned publication
operator. Exact finite coefficient bits, selected thresholds and explicit runtime
budgets survive that real parser/binder. This closes the K/V training-to-operator
provisioning gap while leaving the original residual-stream trainer intact.

All twelve new public campaign tests and all three actual operator-loader tests
pass on qualified nightly-2026-09-08. Real two-layer GQA captures train four
nonconstant probes; benign sampled output reaches an audited stop, while a real
violating sample is held without exposing its candidate cache/logits/token.
Zeroing only layer two's V signal blocks the entire export while preserving the
other three tap results. Changing only final cases preserves fit/calibration and
records failed final counts. A final-tap resource refusal cannot convert three
quiet rows into a released token. Whole-cohort charges and refused retries are
also exercised, including actual arithmetic failures.

The final selected run passes 48 public integration tests across trained K/V,
numerical LoRA, actual native LoRA helpers, learned stream/reserve recovery and
predictive/mediated recovery. The learned-command example passes 21 of 23 tests;
the two original Unix-listener paths still fail with environment EPERM. Three
compile-fail checks pass: two sealed predictive-recovery custody cases and the
new campaign-is-not-a-permit case. That is 69 passing runtime tests plus three
compile-fail checks, with no skipped transport assertions. The original full
gate still refuses the reviewed source snapshot before later workspace checks;
these targeted results are not a complete workspace qualification.

The campaign retains its full immutable model and declared case metadata. The
original codec binds its existing cache profile, which omits tokenizer generation;
the v1 JSON contains no model, tokenizer or lineage authentication fields. It is
provisioning data. These synthetic controls demonstrate executable learned-signal
and guard behavior, not real-world detector quality or an alignment research
result. See [trained K/V monitors](docs/TRAINED_KV_MONITORS.md).

## 2026-10-10: progressive retained-residual review in the learned operator

`fa.learned-publication/2` connects the original retained-residual planner to the
publication operator. Operators declare an ordered exact layer/side/position/head
priority and the complete fixed native round-ID schedule. All future evaluators,
probes, salts, windows and aggregate native reservations are admitted before the
first Begin. Both configured runtime and recipe TTL bound the schedule before
asset intake; remaining lifetime is checked again after generation. Version 1
retains its one-round coarse-only behavior and existing refusal contract.

The original sequence advances only after an explicit uncertainty/abstention
result purchases the next retained residual. The purchase charges cumulative
residual and replicated-input bytes, creates a new input revision and invalidates
the old review. Fresh original probes and actual full-input native inference
must complete on that richer input. Terminal applied reviews do not rerun an
unused roster. One disclosure allowance, one native reservation and one poll
budget cover the entire sequence, including cancelled or unstarted future work.
The original two-key and current-policy publication gates remain unchanged.

All twenty learned-command example tests compile under the qualified compiler;
eighteen pass, including seven new progressive cases and all eleven original
direct/admission cases. The positive buys a genuinely needed exact residual,
runs native Allow, publishes only after independent human approval and recovers
the original receipt once. Negative controls retain work on an underfunded
residual, native Deny, changed policy, exhausted polls, invalid complete schedules,
unretained groups and insufficient future deadlines. The unchanged 1024-event
source ceiling is respected by the complete synthetic positive fixture.

The two actual Unix-listener tests fail with environment EPERM at listener
creation, including authenticated stop during partial successor inference. They
remain unskipped; direct cancellation success does not validate that transport.
The original full gate still refuses the reviewed source snapshot before later
workspace qualification, as recorded in the preceding increment. No process
independence, trained detector quality or matched-cost research claim follows.
See [learned operator publication](docs/LEARNED_OPERATOR_PUBLICATION.md).

## 2026-10-10: atomic learned startup and post-read source freshness

The learned publication command previously created its learned stream and then
attempted to install a terminal reserve. The original journal correctly treats
learned Enable as work, so every otherwise valid command failed with WrongState.
`create_with_learned_text_stream_with_reserve` now writes StreamBootstrap,
ReserveRecovery and the exact learned Enable in one first canonical image.
The original unreserved constructor keeps its two-event sequence, and late
reserve installation remains forbidden by the original admission law.

`FileGuardedLearnedRecovery::require_recovery_reserve` pins the independently
selected reserve from the same exclusively locked canonical history before the
first replay step. The command applies that requirement before numerical replay,
cleanup or fencing. Original complete guard checks, independent floors, exact
recipe matching and the final canonical-cut check remain in force.

The command also now checks the original policy-state capture after acknowledging
post-read time. Merely advancing the clock did not validate the read-start lease;
previously the exact-expiry case could begin stale numerical work. The new
read-only `capture_file_policy_state` delegates the original source frontier and
half-open freshness check. No learned event replay semantics or freshness limits
change. The boundary test now requires Stale, zero admitted tokens and no pending
intent at expiry, while the immediately preceding tick completes normally.

All eleven direct/admission command tests now pass, including actual imported
native review, original two-key publication, pending-step recovery, retained
cancellation work and receipt-only reopen. The twelfth test still fails at the
actual Unix listener bind with EPERM; it remains present and unskipped. The
nonzero actor fixture uses a constant V training population to avoid an unintended
ordered-f32 uncertainty envelope; rotating K remains genuinely compressed, and
all complete-input, native-work and publication assertions remain intact.

All fifteen learned text-stream integration tests pass on qualified
nightly-2026-09-08, including four new boundary tests: atomic installation with
zero inference, malformed/unfunded reserve refusal, no-write missing/changed/late
reserve requirements, and actual learned work exhausting ordinary journal space
before the original fence, stop and drain consume the terminal tail. Six actual
native LoRA helper tests also pass after correcting their expected refusal stage.
Earlier tests in this session passed all seven numerical LoRA cases and all eight
predictive/mediated learned recovery cases. Native fixtures now respect the
original 4096-position limit, and stream proposal comparisons include the exact
policy-derived frozen witness rather than an unfrozen input specification.

The original project gate now passes the qualified compiler-identity check but
refuses the reviewed source snapshot: numerous existing and new source files
are unlisted, and recorded digests for existing files are stale. It stops before
formatting, Clippy and the full workspace test gate. No snapshot record or gate
was rewritten to manufacture qualification. The direct command tests do not establish the unavailable listener transport.
See [learned stream recovery reserve](docs/LEARNED_STREAM_RECOVERY_RESERVE.md).

## 2026-10-10: learned generation through the publication operator

`supervise_publication create-learned-generated` now loads an explicit learned
publication recipe and drives the original text-stream gateway, native sidecar
review, independent human approval and durable publication. The command admits
a real SafeTensors actor, selected tokenizer format, sampling policy and prompt;
replays an imported fit archive against a separately supplied expected binding;
and installs explicit probes for every layer and both K/V sides. The exact
learned recipe requires its original sidecar and policy-only evidence source.
All recipe-referenced asset admission precedes durable startup, with bounded regular-file reads
and one aggregate asset allowance.

A complete independently configured native roster supplies actual monitored
models for the original one-round sidecar sequence. Native inputs come from the original learned sidecar; actor-supplied output,
file-supplied helper contexts and supplied ballots cannot replace them. The sequence owns
the original supervised driver until a terminal handoff, and the same peer
review protocol supplies the human key. Source checks at generation, request,
review, human-request and publication boundaries preserve current policy.
Idle protocol phases wait against the original deadline in short intervals;
active numerical work retains its original poll and compute allowances.

Explicit `--continue-generation` restores the exact original recipe, numerical
state, pending intent and spent work, then requires fresh source/time admission
and original resume. It refuses an already recorded publication request.
`--resume` retrieves or reconciles that original request with its retained
deadline, rebuilding only the bound primary recipe and reading neither current
policy evidence nor native helper assets. The authenticated stop endpoint stays
available across generation/review/publication. During native review it retains
the admitted connection, cancels numerical ownership, takes the original
terminal handoff and completes the existing nonce-bound stop protocol.

The first command profile is deliberately one message and one native round.
It uses in-process original native model owners; model/process independence and
training lineage are not inferred from this orchestration. Existing decoder,
fit, journal, source and effect limits remain intact. The shared v1 supervisor
configuration and the exact native roster remain independently supplied.

Twelve example tests were authored: five direct original-owner flows, six
admission/recovery cases and one authenticated peer-transport flow. They cover
an acknowledged partial generation and retained pending step, complete native
model review, independent human approval/rejection, original publication and
receipt-only reopen, native denial, cancelled work retention, changed policy,
exact no-write recipe mismatches, malformed external assets, the 4096/4097
capture boundary, and read-start lease expiry. A real-clock test preserves
poll/read/journal counters while waiting for a fixed protocol deadline. Complete
native-input, numerical-token and per-poll source accounting assertions bound
the recovered positive fixture below the original 1024-event source limit.

These tests and the complete implementation were source-reviewed, but this
increment has not been compiled or executed because the local execution service
remains unavailable. Existing Unix-socket and qualified-toolchain restrictions
have not been bypassed; earlier passing component tests do not validate this
command. See [learned operator publication](docs/LEARNED_OPERATOR_PUBLICATION.md).

## 2026-10-10: immutable plain-LoRA model and native-helper ingestion

The original decoder now imports one explicitly supplied, complete plain PEFT
LoRA adapter into a new immutable dense model. The bounded profile covers
Q/K/V/O and gate/up/down projections across all layers, including GQA widths,
through the original F32/F16/BF16 SafeTensors reader. Each merged parameter uses
ascending-rank f64 products, an f64 base addition and one final finite f32
rounding. The caller must supply strictly newer model and profile generations;
tenant, model, tokenizer, numerical shape and static RoPE remain exact. Original
sessions stay with their original model, and complete learned recipe bindings
retain every merged parameter, including parameters unused by the prompt.

The public APIs accept byte slices or an authorized bounded reader. Independent
identity/configuration/work preflight precedes adapter reads. Actual reads stay
charged on refusal; after scalar and true-EOF admission, the full planned merge
work is reserved before construction and cannot be refunded by late overflow or
consumer refusal. The original model ceilings and dependencies are unchanged.

`fa.native-worker/4` and `NativeEvaluator::from_llama_lora_files` consume the
adapted model through the original monitored evaluator and commit/reveal worker.
Version 4 requires an explicit base identity and a single base checkpoint; the
prior three manifest versions retain their contracts. Auxiliary assets share
one allowance, base/adapter weights share the original counted reader budget,
and the original worker lifetime includes startup and merge. Tokenizer, monitor
and evaluation policy must bind to the complete adapted decoder. Input-profile
epochs remain their independently provisioned namespace.

Thirteen public test functions were authored across `decoder_lora` and
`native_worker_lora`. They cover independent rank-one/rank-two matrix and query
oracles, all targets/layers and mixed storage, original inference/replay,
zero-delta identity refusal, full unused-parameter recipe binding, early no-read
refusal, retained spending, and actual-helper socketpair commit/reveal with
causal verdict changes and legacy controls. Fixtures are synthetic. Production
and tests have been independently source-reviewed, but this increment has not
been compiled or executed: the local execution service remains unavailable.
No new passing-test, trained-model-quality or deployment claim is made.
See [static LoRA ingestion](docs/STATIC_LORA_INGESTION.md).

## 2026-10-10: required learned forecasts with mediated recovery

The explicit predictive-mediated learned profile now composes a recipe-owned
required pre-output predictor, either raw residual or owned learned K/V, with
the original topology guard and optional marginal evaluator. The independent
predictor must equal the sole pin in the exact learned recipe. Atomic creation
installs the complete original guard inventory before the first canonical image;
the ordinary mediated learned profile keeps its existing predictor exclusions.

Cooperative and anchored recovery retain the original exclusive learned cursor.
Before numerical replay, the profile checks the original/current graph, exact
recipe, predictor source mode and evaluator inventory. Before Ready and around
pending-reset completion, it checks every guard, effective policy, credential
epoch, external floor, graph availability, raw registration and complete owned
predictor fingerprint. Reset completion and fencing use the original transaction;
only acknowledged completion releases separate forecast, topology, evaluator and
oversight roles. Pending forecast loss, abandoned sampled output, statistical
evidence, spent work and unknown publication liability are never reset. Fresh
topology certification and both original publication keys remain required.

The new public harness `mediated_predictive_learned_recovery` supplies positive
and causal negative cases for both predictor representations, actual native
review/publication, reset successors and exact retry, anchored fork/truncation
refusal, changed requirements, retained lost coverage, exhausted work, old-role
refusal and graph bypasses. Two compile-fail examples specify the sealed recovery
boundary. These tests have been authored and independently source-reviewed,
but have not been compiled or executed in this increment: the local execution
service stopped responding. Earlier passing tests do not validate these changes.
The original qualified compiler gate, dependencies and numerical/statistical
contracts are unchanged; no execution or deployment qualification is claimed.
See [predictive mediated learned recovery](docs/PREDICTIVE_MEDIATED_LEARNED_RECOVERY.md).

## 2026-10-10: operator tokenizer JSON through durable generation

`supervise_publication --native-text` and `create-generated` now consume explicitly
selected Hugging Face raw ByteLevel tokenizer JSON through the existing native
importer. Native service schemas /3 and /4 cover single and sharded checkpoints;
generated publication schema /2 adds the same format/path selection. Legacy
schemas keep their original native-archive paths and exact durable semantics.
Complete schema admission precedes referred-file I/O, and the tokenizer's
semantics are checked against the complete model profile before weight/index
reads. Existing file and aggregate limits, paths, controls and stop IDs remain.

The original canonical tokenizer archive remains the durable binding. Recovery
rejects changed IDs, merges or control spellings even when the selected prompt
would tokenize identically. Equivalent JSON whitespace or an explicitly selected
native archive does not create a different effective tokenizer identity.

All eleven new example tests compiled on `nightly-2026-09-21`; seven passed in
the final selected run. Five verify strict format/schema/limit compatibility and
refusal before weight reads. Two exercise the actual recipe loaders and original
file owners across six single/sharded and approve/reject combinations: compute a
partial output, reopen the exact acknowledged numerical state and spent work,
require fresh evidence/resume, finish the original monitored generation, then
perform original commit/reveal review and independent human approval or rejection.
They also reject an old reviewer and changed tokenizer without journal mutation,
and retrieve exact retained receipts with the evidence file removed.

Four complete service tests fail at transport startup in this environment:
`socket(AF_UNIX, SOCK_STREAM)` is denied with `EPERM`, independently reproduced
outside the application; one hold test consequently reports the absent store.
The full service paths and their assertions remain present without skips or a
production fallback. The successful direct tests do not establish socket-service
execution. Actual helper process tests in the RoPE increment use the permitted
inherited socket-pair path and did execute successfully.

Across these three increments, 108 targeted tests and two compile-fail examples
passed. The four transport failures remain outstanding. A fresh original
`cargo run --locked -p xtask -- check` passed reference-lock and source-inventory
checks, then rejected installed compiler `bba531001d4de6d7f49693e0836a2668ca063282`
against qualified commit `cea272fa356e94bd2ee2cadf376630aa0683867a`. No compiler
policy was changed, and no full workspace, formatting, Clippy or deployment
qualification pass is claimed. See
[operator tokenizer intake](docs/OPERATOR_TOKENIZER_INTAKE.md).

## 2026-10-10: learned generation with topology-observer custody

The original mediated guard profile now composes with learned numerical startup,
cooperative recovery, exact retained-history anchors and pending-reset completion.
Creation checks the exact learned recipe, every declared guard, original graph
and optional marginal evaluator before its single first image. A recipe-owned
policy source must match the independent declaration and is installed once.

Recovery retains the original exclusive learned cursor and checks original/current
graphs, availability, evaluator, policy, credentials and external floors before
readiness or writes. Completion uses the original reset/fence transaction and
only then releases separate role custody. The fence withdraws the old topology
cut, identity eligibility and approvals; publication requires a newer graph and
inventory, fresh certification, resumed numerical work and both original keys.
Unknown effects and spent work remain charged. Predictor-pinned and joint
evaluation profiles are explicitly excluded rather than losing observer custody.

All 41 public tests passed locally on `nightly-2026-09-21`: eight new
`mediated_learned_recovery` tests and 33 surrounding tests in
`file_mediated_bootstrap`, `file_mediated_recovery`, `file_mediated_inspection`
and `learned_guarded_recovery`. They execute real generation, anchored pending
reset, exact sampled-token retry, retained unknown liability, independent
evaluations, fresh identity, original congress and two-key publication. Negative
cases preserve journal/staging bytes under graph, recipe, evaluator, guard,
floor, reset-intent, equal-counter fork and truncation mismatches. Two old
bootstrap fixtures were corrected to respect their unchanged 16-byte effect
budget and the existing pre-clock identity refusal; no production check changed.
Both new compile-fail examples passed, confirming that the public recovery type
offers neither early role access nor an escape to the underlying owner.

This completes this bounded learned/mediation composition, not operating-system
containment or deployment qualification. See
[mediated learned recovery](docs/MEDIATED_LEARNED_RECOVERY.md).

## 2026-10-10: static RoPE through numerical execution and helper processes

The original Llama/SafeTensors decoder now admits explicit linear and Llama3
rotary scaling as immutable numerical profile identity. Strict legacy and modern
checkpoint declarations agree exactly or refuse before weight access. The
existing half-split Q/K kernel consumes the selected frequencies; every sampled,
learned, tokenizer and durable decoder binding retains the complete declaration.
Unscaled profiles keep their original bytes and all resource ceilings remain.

The actual `fa-native-helper` executable accepts `fa.native-worker/3` with a
required independent `decoder.rotary` declaration. It supports the existing
single/sharded checkpoint and native/JSON tokenizer selections, checks imported
profiles before weight/index access, and preserves the original worker lifetime,
monitoring and commit/reveal protocol. /1 and /2 remain unchanged.

Local validation on `nightly-2026-09-21` passed all 60 tests across
`decoder_rotary_scaling`, `decoder_pretrained`, `decoder_sampled_archive`,
`learned_generation_archive`, `file_decoder_oversight`, `native_worker_rotary`,
`native_worker_v2` and `native_helper_process`. Twelve new tests cover independent
scalar Q-capture oracles, changed later attention, exact stochastic continuation,
named controls, recipe/profile substitution, durable publication and the actual
spawned helper's input-dependent votes. The other 48 exercise compatibility.
An older ReLU rejection fixture needed its missing JSON quote restored; its
Unsupported assertion and production admission were not relaxed. An initial
library build was killed under shared resource pressure; serializing compiler
frontend/backend work allowed the completed builds and tests above.

This advances the bounded FA-025/FA-026 reference path, not production-sized
Llama3 admission, dynamic scaling, empirical calibration or deployment
qualification. The qualified compiler policy and historical gate receipts are
unchanged. See [static RoPE](docs/STATIC_ROTARY_SCALING.md).

## 2026-10-10: atomic evaluated learned startup

`create_evaluated_guarded_with_learned_generation` now publishes the exact learned
recipe, independently supplied evaluation protocol and every declared guard in
one original first image. The prepared owner remains private until canonical
acknowledgment. Recipe-owned policy-only sources must exactly match the declared
guard and are installed once, followed by complete native contract validation.
Predictor-pinned recipes retain their separate required role profile.

All 24 public `learned_guarded_recovery` tests passed locally on
`nightly-2026-09-21`, including four new first-image cases. Positive cases execute
real numerical generation, fresh identity, original congress, independent labels,
both publication keys and a real policy-file refresh. Historical gate receipts
and production-runtime admission remain unchanged. See
[evaluated learned startup](docs/EVALUATED_GUARDED_RECOVERY.md#atomic-learned-startup-and-recovery).

The final surrounding regression run passed all 51 tests across
`action_consistency`, `file_action_consistency`, `file_owned_forecasts`,
`hosted_action_forecasts`, `file_keyed_forecasts`, `file_consistency_loss`,
`file_predictive_guarded` and `file_evaluated_guarded`. Together with the 24 above
and six reset-successor tests below, 81 public integration tests passed. Repairs
to older positive fixtures retain existing payload budgets and check the actual
crossing boundary: staging remains observable while positive review, automatic
permission, human approval and effects remain refused.

The original `xtask check` passed lock and source inventory checks, then refused
the installed compiler commit `bba531001d4de6d7f49693e0836a2668ca063282`, because
the qualification policy requires `cea272fa356e94bd2ee2cadf376630aa0683867a`.
The policy was not changed. The separate workspace/all-targets check and
monolithic unit-test build were killed with SIGKILL under shared resource
pressure. The eight new predictor reset/fault unit tests have not executed;
no complete workspace, formatting, Clippy or qualification pass is claimed.

## 2026-10-10: forecast continuation on original reset successors

New learned text recipes can explicitly select
`with_forecast_reset_successors` after pinning their required raw-residual or
owned-K/V predictor. The original broker follows only its own restored learned
source and retains one lifetime forecast process. A fresh accepted prefill token
must establish the new source; the original pending/lost-coverage, sampling,
acquisition-budget and authority restrictions continue to apply. Existing
recipes retain their original exact-stream behavior and historical refusals.

All six public `forecast_reset_successors` tests passed locally on
`nightly-2026-09-21`. They exercise actual original numerical execution, both
forecast source modes, pending-reset recovery, exact sampled-token/RNG receipts,
generated-request admission, original-history replay and causal refusal cases.
This requires a checkpoint before prompt completion and does not enable a fresh
output attempt after sampled work is abandoned. It does not qualify empirical
forecast calibration, dependencies or deployment. See the
[reset-successor forecast contract](docs/LEARNED_FORECAST_RESET_SUCCESSORS.md).

## 2026-10-10: composed learned-reset recovery

Evaluated learned generation now supports cooperative and anchored recovery while
retaining the independently supplied evaluator protocol, every guard, effective
policy, credential epoch and external floor. Evaluated recovery and both existing
predictive learned recovery profiles can finish an acknowledged pending reset
through the original completion-and-fence transaction. All role bundles remain
sealed until that replacement acknowledges; old tickets, unknown charges,
unanswered forecasts and spent numerical work retain their original semantics.

Public integration coverage exercises real numerical replay, exact contract
rejection, anchored reset, fresh evaluator and identity custody, and two-key
publication. Eight new unit regressions additionally target predictive lifetime
retention, exact retries and all five storage barriers. Local validation uses the
installed dated nightly and does not replace the historical qualified gate;
the full monolithic unit build encountered shared-environment resource limits.
No production-runtime admission, bead closure or deployment qualification is
claimed. See the [composed reset contract](docs/PENDING_LEARNED_RESET_RECOVERY.md#evaluated-and-predictive-learned-profiles).

## Unqualified source addition: supervised learned recovery and containment

Actor supervision and connected/offline drivers now invoke the same paired
learned checkpoint/reset, retain actor tickets, and clean up abandoned reviews
and permits according to the original ledger. Detached reset preserves unknown
effect charges and the original fenced reconnection path. Typed learned
advancement and the ordinary driver loop now service actual learned automatic
stops through endpoint fencing and reconciliation.

Regression sources use original inference, helper sockets and endpoint recovery,
with positive publication and causal refusal controls. Required remote RCH is
unavailable, so compilation, tests, rustfmt and Clippy remain UNEXECUTED. See
[learned supervision](docs/HOSTED_LEARNED_SUPERVISION.md). This is in-process
recovery integration, not durable broker restart or deployment qualification.

## Unqualified source addition: paired learned containment reset

The broker now pairs an original learned-generation KV checkpoint with the
existing authority checkpoint. A fresh complete original audit and exact KV/RNG
restore precede the original narrowing/reset transition. Abandoned work remains
charged across repeated rewinds, incident counts and spent effects do not rewind,
and the resumed source requires a newly accepted token before effect admission.
Fixed stops remain terminal; cumulative audience-stream owners are excluded.

Eleven regression functions cover actual continuation and renewed two-key
publication, budget exhaustion, unknown effects, incidents, and causal refusal
paths. Source review does not qualify the feature: required remote RCH cannot
launch, so compilation, tests, rustfmt and Clippy are UNEXECUTED. See the
[paired learned reset contract](docs/HOSTED_LEARNED_RESET.md). Durable reset and
independent restart/incident-frontier qualification remain outside this increment.

## Unqualified source addition: computed durable learned congress

The durable sidecar path now drives complete original numerical helper rosters
through journaled commit/reveal and original refinement. Coefficients, salts,
round identities and limits freeze before execution. Missing members do not
purchase residuals; richer input requires a fresh original review. Completed
round archives and result comparisons survive replay, while publication still
requires the existing automatic and human keys.

The driver retains completed probe work through source loss, cancellation,
deadlines and storage failures. Regression sources exercise actual uncertainty
through residual refinement to both keys and canonical publication, plus causal
negative controls. Compilation, tests, rustfmt and Clippy are UNEXECUTED: the
required RCH command cannot launch. See the
[computed durable sidecar contract](docs/DURABLE_LEARNED_SIDECARS.md#computed-votes-and-durable-refinement).
No empirical detector quality, process independence or qualification is claimed.

## Unqualified source addition: current durable learned sidecar plans

The existing durable learned sidecar path now has an opaque current plan handle
and a distinct witnessed creation record. Original replay reconstructs the source,
planner and provenance marker, then compares exact saved payload expectations.
Acknowledged handles track the durable owner and reject stale or foreign sources.
The existing snapshot constructor, historical getter, independent mandatory
recipe and request-only journal format remain compatible.

Regression sources cover original publication and replay, recipe/payload mismatch,
copied inputs, source loss and storage barriers. Rust compilation, tests, rustfmt
and Clippy remain UNEXECUTED because required RCH is unavailable. See the
[durable sidecar contract](docs/DURABLE_LEARNED_SIDECARS.md). This advances FA-113
without closing its remaining verification or empirical obligations.

## Unqualified source addition: cooperative durable learned recovery

`FileOversight::begin_open_with_learned_generation` now holds the original
exclusive lock while the host advances bounded original journal events. It
exposes progress only, retains first failures and leaves storage untouched when
unfinished recovery is abandoned. Completion compares the exact canonical cut
and persists the original recovery fence before releasing an owner or reviewer.
The existing synchronous learned open consumes this implementation; finishing
no longer repeats the entire learned numerical history a second time.

Seven unit tests, two text integration tests and two compile-fail cases are
authored for continuation, pending work, nonresurrection, cancellation, capacity
and original storage faults. They are UNEXECUTED: targeted and full checks cannot
launch because `rch` and Rust tools are absent. See the
[durable learned contract](docs/DURABLE_LEARNED_GENERATION.md#cooperative-recovery).
This advances the bounded FA-014 recovery path without a qualification claim.

## Unqualified source addition: cooperative learned congress execution

The actual `LearnedProbeReview` driver now admits each member once, scores one
original probe per member per poll, and yields after complete judgment before
queuing its commitment. The original source/deadline checks precede each poll;
automatic residual refinement still starts a fresh original congress round.
Cancellation, expiry and source/poll failures retain completed numerical work
without supplying a partial or substitute vote.

Regression sources exercise interleaved member progress, cancellation and expiry
between probes, retained disclosure costs and the original successful two-key
publication path. Compilation, Rust tests, formatting and Clippy remain
UNEXECUTED: remote-only RCH cannot launch because `rch` is absent. See the
[computed review contract](docs/COMPUTED_PROBE_REVIEW.md). No historical receipt
or bead closure qualifies this addition.

## Unqualified source addition: cooperative original-protocol probe transport

`ProbeHelperClient` now drives the numerical evaluator through admission and
one-probe steps. Every numerical step yields even with the maximum drive
allowance. The original commit/reveal pair is frozen only after complete-roster
judgment; reveal still requires the original coordinator signal. Cancellation
and failure release unfinished evaluator ownership and preserve completed work
and reports without manufacturing a vote.

Two added transport regressions exercise cancellation between probes and after
judgment before any commitment bytes. Existing transport controls now assert
zero-score admission and one-probe drive boundaries. All nine tests added by
the cooperative evaluator/transport increments are authored but UNEXECUTED;
workspace execution and the required RCH/Rust tools are unavailable. See
[the bounded contract](docs/SIDECAR_PROBE_HELPERS.md). No gate is closed.

## Unqualified source addition: cooperative sidecar probe evaluation

The original numerical sidecar evaluator now admits an input once and exposes
one registered probe per revision-checked advance. A final verdict still
requires the complete captured K/V roster, including probes after an early
alarm. Cancellation releases unfinished source ownership while retaining
completed disclosed observations, work and terminal reports. Existing
synchronous entry points consume the same implementation.

Seven additional integration tests cover step parity, full-roster completion,
stale calls, cancellation and exact request binding. They are authored but
UNEXECUTED because workspace execution and the required RCH/Rust tools are
unavailable. See [the bounded contract](docs/SIDECAR_PROBE_HELPERS.md).
No bead or qualification status is changed; historical receipts do not validate
this addition.

## Unqualified source addition: learned-generation checkpoint replay

The typed checkpoint/reconstruction path in `sampling/replay.rs` reruns the
original learned generator, preserves sampler and spent aggregate budgets, and
compares exact cache/logit words before exposing a reconstructed owner. This is
the in-memory recomputation baseline, not durable/live recovery. Eight Rust
tests and four compile-fail cases are authored but UNEXECUTED; RCH and Rust
tools are unavailable. See [the bounded contract](docs/LEARNED_GENERATION_REPLAY.md).
Historical qualification evidence below does not validate this addition.

Current execution evidence: [receipt](artifacts/execution/2026-09-08-range-witness-receipt.json).

## 2026-09-08 UTC: exact range membership and phantom invalidation

The complete frozen RCH gate passed **511 tests: 105 reference unit, 156 integration, 242 xtask and 8 doctests**, with zero failed, ignored or filtered tests, on `vmi1149989` under qualified `nightly-2026-09-08`. [The receipt](artifacts/execution/2026-09-08-range-witness-receipt.json) binds 175 source/build inputs and job `j-30004650170124459`.

FA-059 adds exact EmptyRange and RangeMembers witnesses over finite half-open intervals `[start, end)`. Ordered scans detect inserted/deleted members, versions and same-version byte changes; unrelated keys remain reusable. Both capture and reuse require the complete caller-trusted authenticated terminal marker. Same-snapshot exact closure controls succeed while unknown, unrecorded-tail and declared conservative-summary completeness evidence refuse. The latter is evidence-kind admission, not an approximate-summary algorithm. This range profile cannot include `u64::MAX`; that limitation is declared and tested without an overflowing successor calculation.

Independent public scans compare empty/member results across bounded intervals, endpoint mutations, closure loss and extreme keys. Retention counts and sums all requested member occurrences before cloning, admitting exactly 256 members and 2 MiB of logical values. Overlapping requests can reach the one-over member refusal. The separate byte guard is redundant under the entry/value bounds and is not claimed independently exercised. Reuse reports logical visited members/bytes, including early mismatch; an `Err` exposes no partial cost or effect accounting.

Two planted defects fail after controls: ignoring an interior empty-range insertion and ignoring same-version member bytes. Nine fixed release cases produced 54 samples across actual capture/reuse, mutations, incomplete closure, maximum retention and independent recomputation. Printed durations sum six samples per case; they are not a distribution. A separate sole-process first capture and an exact-binary heap profile are retained. Whole-process allocation calls were 2,552 versus a 185-call zero-test baseline; inclusive witness-frame calls were 1,683 versus zero. No per-case allocation, allocated-byte, peak-heap, RSS, OS/cache-cold or SLO claim follows.

Independent original-contract review qualified FA-059 at the declared adapter-evidence boundary; final documentation qualification remains separate. No adapter authentication, database integration, physical planner, runtime admission, production cache/dispatch or release is qualified by this batch.

## 2026-09-08 UTC: observation transfer laws and bounded inventory loading

The complete frozen RCH gate passed **499 tests: 105 reference unit, 144 integration, 242 xtask and 8 doctests**, with zero failed, ignored or filtered tests, on `vmi1149989` under qualified `nightly-2026-09-08`. [The receipt](artifacts/execution/2026-09-08-algebra-inventory-receipt.json) binds 172 source/build inputs and job `j-30004650170124437`.

FA-068 implements the thirteen pure observation operator transfer laws. Private transfers retain their origin, exact supplied capture/encoding/decoding/helper bytes, declared uncertainty, scoped proof predicates and cancellation obligations. Fallible transforms borrow active inputs; successful composition unions identical obligations, frontiers and privacy labels without erasing distinct requirements. Exact closure cannot be supplied by approximate discovery, joins require compatible uncertainty and origins, and emitted opaque judgments retain the actual input basis and calibration. Cancellation was repaired to retain captured bytes. Supplied decoded values and caller-registered laws remain assumptions: no capture provider, decoder, helper inference or physical executor runs through this IR.

FA-002 now reads actual caller-selected regular files through a bounded 1 MiB reader and strict JSON/schema validation. The declaration includes all eleven effect classes, profile generations, trust paths/domains, credential dispositions and residual nonclaims. Unknown effects/fields and unmodeled bypasses refuse; any brokered route forbids direct actor credentials throughout that family. Independent controls exercise exact and one-over file, family, route, string and aggregate text bounds. The parser enforces the individual string cap before schema validation. Metadata/open replacement races and file authenticity remain outside this reference loader; the real disposable-endpoint mediation gate is still missing, so FA-002 remains open.

Three separately executed mutations fail after valid controls: dropping captured bytes during cancellation, admitting approximate closed scope, and accepting an unknown effect. The closure mutation tests the admission boundary, not end-to-end absence promotion through the separate exact-lookup defense. Five fixed algebra cases produced eight timed samples and eight inventory cases produced 32; two expected Binding responses are labeled `observed_errors=1` in raw algebra output and asserted as expected refusals. Separate fresh release processes measured one first operation each. No statistical, OS/cache-cold, physical decoding or SLO claim follows.

Exact-binary heap profiles recorded 707 algebra and 3,811 inventory whole-process allocation calls, with same-binary zero-test baselines of 180 and 185. Inclusive library-frame counts were 395 and 3,384, with zero in either baseline. They are not per-case allocations, allocated bytes, peak heap or RSS. All seven failed gate attempts remain retained; fixture type/epoch/budget mistakes and a parser-versus-schema error expectation were corrected without relaxing production guards. Fresh full-contract review reopened FA-058 for missing conservative fact-equivalent invalidation classification in its cost harness. The [repair receipt](artifacts/execution/2026-09-08-algebra-inventory-witness-review-receipt.json) now records a new 499-test full gate and seven release cases with 84 samples: 24 necessary fact invalidations, 12 conservative fact-equivalent invalidations and 24 typed refusals. Sixty selected retained-judgment comparisons had zero stale reuse inferred from completed oracle/verdict assertions; this is not an independently instrumented counter or a helper/production-permission metric. The new marker supports fresh capture/reuse while the old marker witness invalidates. A new sole-process first capture and exact-binary heap profile (1,780 whole-process calls, 185 baseline, 444 inclusive witness-frame calls, zero baseline attribution) bind this repaired harness. Earlier evidence remains historical; it does not qualify the changed harness. Independent original-contract review then requalified FA-058 and qualified FA-068 within their reference boundaries. Final documentation and registry changes require a separate gate. Range witnesses, foundation admission, runtime integration and production release remain unqualified by this batch.

## 2026-09-08 UTC: exact-value and absent-key witness reuse

The complete frozen RCH gate passed **458 tests: 100 reference unit, 108 integration, 242 xtask and 8 doctests**, with zero failed, ignored or filtered tests, on `vmi1149989` under qualified `nightly-2026-09-08`. [The receipt](artifacts/execution/2026-09-08-witness-receipt.json) binds 164 source/build inputs and job `j-30004650170124415`.

FA-058 adds private L2 witnesses over exact key/value bytes, versions and query roles, plus absent keys tied to the complete caller-trusted authenticated domain marker and product frontier. Reuse checks snapshot revision/control cut, semantic epoch and domain/projection identity. Unrelated newer changes remain valid; changed bytes, versions or membership invalidate. Older cuts and unknown/incomplete absence closure refuse. Positive exact reads do not require a globally closed domain. The adapter's trust declaration remains an assumption, not implemented authentication or a production cache.

Independent public controls exercise semantic-only invalidation, absent-key insertion, stale snapshots, exact 256-entry/8 KiB-value/64-request limits and their one-over refusals. A bounded store scan independently recomputes observed facts. Two planted guard omissions fail after valid controls: ignoring same-version byte changes and accepting insertion of an absent key. Six fixed release cases produced 72 descriptive samples; a separate fresh process measured its first capture once, excluding setup. No OS/cache-cold, statistical, helper-call or SLO claim follows.

The exact release binary's heap profile records 1,431 whole-process allocation calls versus 185 in its same-binary zero-test baseline; 336 calls have an inclusive witness-library frame, versus zero in the baseline. These are neither per-case allocations nor allocated-byte, peak-heap or RSS measurements. The earlier cost harness incorrectly labeled eight bytes as captured when unknown closure returned no judgment; the label now counts successfully returned judgment bytes (zero for that refusal), and the full gate, release cases, fresh-process observation and profiles were rerun. Raw earlier results and the initial strict-Clippy failure remain retained with those limits.

Observation algebra, range witnesses and complete perimeter/endpoint integration remain separate work. No production permission, provider capture, runtime admission, persistence or release is qualified by this batch.

## 2026-09-08 UTC: exact declared views and conservative trace independence

The complete frozen RCH gate passed **446 tests: 95 reference unit, 101 integration, 242 xtask and 8 doctests**, with zero failed, ignored or filtered tests, on `vmi1149989` under qualified `nightly-2026-09-08`. [The receipt](artifacts/execution/2026-09-08-view-trace-receipt.json) binds 160 source/build inputs and job `j-30004650170124390`.

FA-081 adds an immutable L2 evidence-view manifest over original identities, authorization projection, transform/redaction, order, source windows and the exact supplied helper input. Every evidence part has one matching binding, the projection is exact, and truncation means a proper subset of the declared source range. Valid alternate manifests test complete equality; separate raw constructor negatives change one representation to expose mismatches. No caller bytes are re-encoded, and this does not authenticate a provider or prove a transform executed.

FA-092 adds an immutable L7 registered trace relation over retained input-derived positive and negative reads, writes, RNG state, consuming resources and authority/ordered/external/unknown barriers. Review found and repaired cross-category collisions in the shared domain namespace: mutation in any category conflicts with every access to that domain. The independent 25-category matrix retains four read-only cases and orders 21 mutation cases. Thirteen operations yield 78 distinct unordered pairs (31 independent, 47 ordered); three small fixtures enumerate six permutations and retain 6, 1 and 3 original-order-compatible schedules. These bounded counts are not general DPOR coverage or executor authorization.

Three planted mutations fail at the intended public assertions after controls: neutralized redaction, omitted RNG mutation and omitted negative-domain conflict. Nine fixed release cases produced 108 timed samples. Separate sole-test fresh processes measured first view construction and first relation evaluation once each; no OS/cache-cold or statistical claim follows. Exact release binaries were copied unchanged to the existing heap profiler: view/trace whole-process allocation calls were 934/1,897, each with a 185-call same-binary zero-test baseline. Inclusive library-frame attribution was 411/180 calls, with zero in either baseline; it is neither per-case allocation nor a complete attribution of inlined work. Raw logs, profiles, failed setup attempts and exact byte identities are retained.

FA-081 and FA-092 were independently closed after source and raw-artifact verification; final documentation and registry changes require their own gate.

The initial prospective 438-test pass preceded the cross-category repair and receives no credit for that fix. The witness and observation-algebra work remains separate and unqualified by this batch. Production capture, authentication, scheduling, effects, persistence and release remain unimplemented.

## 2026-09-08 UTC: property, first-parse and compiler qualification

The complete frozen RCH gate passed **412 tests: 80 reference unit, 82 integration, 242 xtask and 8 doctests**, with zero failures, ignored or filtered tests, on `vmi1149989`. The actual rolling compiler `cea272fa356e94bd2ee2cadf376630aa0683867a` was resolved with all declared components, passed the full gate, then was frozen as `nightly-2026-09-08`. [The qualification receipt](artifacts/execution/2026-09-08-history-format-qualification-receipt.json) binds 154 source/build inputs and job `j-30004650170124379`; the earlier compiler evidence below remains historical.

Three fixed generators executed 128 instances per schema, checking original syntax meaning, independent canonical byte ordering/whitespace, and malformed twins. Separate release processes measured the first decode of each manual schema fixture (one sample each, no warmup); these are not OS/page-cache/CPU-cold or statistical results. Eight descriptive release cases again completed 96 samples on the new compiler. [Heaptrack evidence](artifacts/execution/2026-09-08-history-format-allocation-receipt.json) separately binds the earlier qualified release binary: 8,544 whole-process allocation calls, a 185-call zero-test baseline, and 5,352 inclusive calls under 60 roundtrips. The derived mixed-case mean is 89.2 calls per roundtrip; inclusive parser totals overlap. This older-compiler allocation run does not certify the new compiler's allocation behavior. Compiler, Clippy and RCH admission failures are retained, with no runtime credit for refused jobs.

FA-004 and FA-005 were independently closed against their original contracts after reviewers checked raw execution and artifact hashes. Final documentation and roadmap changes require a separate gate; the qualification receipt binds its own frozen overlay.

## Qualified seeded histories and canonical draft bytes

The complete frozen RCH gate passed **406 tests: 80 reference unit, 76 integration, 242 xtask and 8 doctests**, with zero failures, ignored tests or filtered tests, on `vmi1227854` under `nightly-2026-09-07`. [The receipt](artifacts/execution/2026-09-08-history-format-receipt.json) binds job `j-30004650170124362` and 152 reviewed source/build inputs.

FA-004 now replays bounded typed event lanes through the existing action authority, preserving each lane's order and retaining seed, selected prefix, exact refusal and state inspection. Completed replay means every supplied event was accepted, not that every action reached a terminal state. The reference profile bounds lanes, event count, snapshot entries/value bytes and aggregate retained payload/witness/snapshot bytes. Independent public tests exercise successful multilane lifecycles, exact accounting, expiry, unknown-effect cancellation/reconciliation and revocation. Restoring the raw low-bit PRNG output failed the fixed-seed schedule-diversity regression after all six histories replayed; this is a bounded counterexample, not a uniformity or complete scheduling proof.

FA-005 encodes and decodes the three original draft JSON shapes under the explicit [FA-CJSON/0.1 profile](schemas/README.md), preserving their wire labels. Opaque validated payloads prevent unchecked construction or mutation. Independent manual golden bytes and public negatives cover canonical spelling/order, duplicate/missing/unknown fields, numeric ranges, Unicode characters without normalization, empty and duplicate array rules, truncation and resource bounds. Removing only the canonical byte-comparison refusal made the manual-golden test fail after its unchanged and different-valid-document controls passed. No golden was regenerated from the encoder; original newline-bearing fixture bytes were backed up before the exact trailing-newline edit.

The release harness executed eight fixed cases, twelve samples each: **96 samples with 84 asserted successes and 24 asserted refusals** (one case has both a positive and negative observation). It distinguishes fresh construction/replay from setup-excluded parsing and includes malformed bytes and a large Unicode document. These are descriptive timings, not an SLO, cold OS/process measurements or a distribution estimate. Logical input bytes are not allocation or byte movement. Whole-Cargo-command peak RSS was 291,596 KiB including compiler and test children, not isolated model memory; allocation was unmeasured at this earlier checkpoint (the later stack-attributed measurement is above).

The first gate refused a missing static lifetime annotation on field-name diagnostics. Two scratch mutation preparations failed before RCH submission and receive no execution credit. The shared strict syntax parser retains independent operator admission decisions and adds no Cargo edge; invalid UTF-8 now has valid-prefix coordinates, and oversized input explicitly reports unavailable coordinates rather than a fabricated position. Existing operator negatives pass. This batch implements no serialized history parser, general JSON Schema engine, authenticated transcript, production differential, broker, persistence, effect or DSR rerun.

## Qualified full-input epochs and public frontier boundaries

The complete frozen RCH gate passed **373 tests: 68 reference unit, 57 integration, 242 xtask and 6 doctests**, with zero failures, ignored tests or filtered tests, on `vmi1227854` under `nightly-2026-09-07`. [The receipt](artifacts/execution/2026-09-08-input-frontier-receipt.json) binds job `j-30004650170124353` and 141 reviewed source/build inputs.

FA-057 now requires explicit tokenizer, policy and model epochs in every input profile; zero remains a valid explicitly supplied initial epoch. Public constructor/capture/validation tests bind each epoch, every submitted byte, profile and part identities, omission presence and metadata, and a removed context section with a valid rebuilt partition. Exact configured caps and one-over refusals execute. A separate planted mutation ignoring only the tokenizer epoch failed at the intended public assertion after unchanged-input and byte-change controls passed. It does not independently neutralize the other two epochs.

FA-063 public tests cover nonempty missing-middle sequences, duplicate idempotence, source/branch/projection/epoch isolation, exact closing generations, independent stage completion and atomic post-close refusals. A named prefix plus closure does not assert whole-stream stage coverage; that requires the requested prefix to reach the caller-retained marker's terminal sequence. Zero terminal sequence and zero marker generation are different: the former can declare an empty projection, while the latter refuses.

The first gate stopped on formatting after root's local dated toolchain lacked rustfmt and the shell continued. The formatter was installed, source formatted and identities rebound before the passing run. These are bounded reference results, with no authenticated provider capture, marker-origin authentication, production receipt, persistence or effect. This batch measures no module-specific performance or allocation costs and does not rerun DSR. The first final-documentation gate refused stale current-count rows that root had missed; those rows were corrected while dated earlier results were preserved. An independent verifier reconciled the original FA-057/063 acceptance with the exact source, executed test names, raw results and hashes before closing both reference-definition packets.

## Qualified transition table and declared perimeter lookup

The complete frozen RCH gate passed **362 tests: 67 reference unit, 48 integration, 242 xtask and 5 doctests**, with zero failures, ignored tests or filtered tests, on `hz3` under `nightly-2026-09-07`. [The receipt](artifacts/execution/2026-09-08-semantics-receipt.json) binds job `j-30004650170124336`, 139 reviewed source/build inputs, the failed attempts and a separate release cost run.

FA-001's public transition table exercises 107 callable cells across all eleven states and ten lifecycle methods. Three preauthorization dispatch cells are explicitly unavailable because the same attempt has no permit. Allowed edges assert exact state and budget accounting; refused edges assert the entire inspection is unchanged. The release harness executed eleven fixed cases and 102 samples covering fresh-object initialization, successful transitions, validation, expiry, revocation and unknown-outcome reconciliation. Setup-excluded authorization/dispatch timings are distinguished from whole-case timings. These small descriptive samples establish neither an SLO nor cold operating-system caches. Logical input bytes are not measured allocation or byte movement; witness-count-cap and empty-range timing remain unmeasured. Isolated model memory is unavailable under the admitted in-memory test profile. GNU time separately measured the whole Cargo command, including compiler and test children, at 301,932 KiB peak RSS; that is not model memory.

An independent verifier checked the original FA-001 acceptance, immutable base and overlaid source hashes, retained gate/cost log hashes, actual test counts and RCH exit results before closing that full packet. Closure is limited to its original semantics-only contract. Canonical transport, Asupersync purpose contexts, broker dispatch, durability and real endpoint E2E remain later work.

FA-002 now has an immutable, bounded declaration inventory and exact scope/family/route lookup. Brokered declarations require broker credentials and refuse direct actor credentials across the entire family. All unmodelled bypasses refuse, while modeled residuals remain explicit for observation/cooperative declarations. Removing only the new all-mode guard made the independent regression fail after valid controls succeeded; the corrected source passes the full gate. FA-002 remains in progress: complete effect/profile/threat/credential-surface declarations and actual bounded inventory loading, cold and malformed/unreadable-input evidence are still missing. A checked declaration neither creates enforcement nor proves a deployment perimeter.

Two full-gate failures are retained: root initially missed the admission-policy digest after updating the reviewed source manifest, then an integration assertion attempted equality on an intentionally opaque permit result. The binding and assertion were corrected without relaxing admission or error/state assertions. Two cost-command flag combinations were refused before execution; the supported RCH job ran the same frozen checkout directly. No foundation, production broker, canonical transport, authenticated issuer, persistence, DSR rerun or release is qualified by this batch.

## Earlier action/permit reference qualification

The complete frozen RCH gate passed **335 tests: 58 reference unit, 30 integration, 242 xtask and 5 doctests**, with zero failures, ignored tests or filtered tests, on `hz3` under `nightly-2026-09-07`. [The receipt](artifacts/execution/2026-09-08-action-receipt.json) binds job `j-30004650170124324`, 135 reviewed source/build inputs and all four gate attempts. Final documentation and tracker updates are checked separately; the earlier receipts retain their original source boundaries.

FA-001 now freezes exact scope, target versions, payload, declared logical witnesses, policy epoch, deadline and units. Opaque instance-bound permits consume the existing rights ledger once; a permit from another authority refuses even with identical authorized state. A missing clock observation refuses preparation; an explicitly observed tick zero is valid. The initial unqualified implementation reproduced that clock defect with ten passing tests and one failing regression; [the complete original source/test patch](artifacts/execution/2026-09-08-action-clock-red-baseline-tests.patch) and raw run are retained. The final twelve public action tests cover binding changes, witness limits, expiry, revocation, lifecycle, accounting and unknown-effect liability, alongside eighteen existing integration tests.

Qualification retained three failed gate attempts: a test-helper Clippy warning, a worker missing its nightly formatter, and eighteen registry negatives with incomplete sandbox inputs after new roadmap artifact links. The warning and worker setup were repaired; sandboxes now copy the referenced real files, with exact negative assertions unchanged. At that checkpoint FA-001 remained in progress with transition-table and cost obligations unfinished. Admitted Asupersync purpose contexts, canonical wire compatibility, an authenticated issuer, a broker and a durable ledger belong to later packets and were not supplied by that reference batch.

FA-052's existing five-file provenance results were independently reconciled and its missing changelog entry added. It is complete only as source intake. FA-053 is blocked: the pinned Asupersync native source includes an unconditional build script, mandatory unadmitted dependencies and native FFI paths even with default features disabled. No foundation has been admitted and no donor source was changed.

## Fresh review: corrected reference and operator boundaries

The complete frozen RCH gate passed **319 tests: 57 reference unit, 18 integration, 242 xtask and 2 doctests**, with zero failures, ignored tests or filtered tests, on `vmi1227854` under `nightly-2026-09-07`. [The receipt](artifacts/execution/2026-09-08-fresh-review-receipt.json) binds the base, exact overlay, 133 source/build inputs and all retained attempts. The initial regression batch on unchanged `3001c52` implementation reproduced eleven failures with one passing control; [its exact test patch](artifacts/execution/2026-09-08-fresh-review-baseline-tests.patch) is retained separately from later added controls.

Empty reference domains can now close at sequence zero only with an explicit nonzero-generation caller-trusted marker; capacity, scope and post-close checks still apply. The admission gate refuses duplicate package IDs before identity collapse, and observation projection refuses duplicate resolve nodes or partially unreadable dependency kinds/targets. Document scanners exclude indented code and HTML comments while preserving fence boundaries. Named reference checks reject ignored tests and unqualified conditional declarations; this remains bounded source inspection, not proof that a declared test ran. The operator digest rejects non-regular paths before invoking the hash utility; its test covers a regular file and directory, not an executed FIFO hang experiment.

Two full-gate failures remain visible: a late diagnostic edit needed formatting, then an added test's local variable shadowed its helper. Both were repaired without relaxing assertions. The malformed README performance table was corrected and the old package digest/report explicitly labeled historical. Production behavior, full foundation admission, signing and release remain unimplemented; the previous DSR result has not been rerun on these changes.

At that fresh-review checkpoint, eight affected leaves or epics had been reopened; three scoped reference/operator leaves were reclosed, leaving five checker leaves/epics open. Its 336 live beads comprised 12 closed and 324 open. The later action batch independently reconciled the original checker criteria after FA-052 provenance completion and reclosed those five in dependency order. These are checker and source-intake results, with no production increment.

## Roadmap admission ordering verified

The reviewed roadmap now makes complete admission an explicit prerequisite of native types (FA-007) and the ATP adapter (FA-064). The source binding was updated only for that reviewed roadmap change. A fresh frozen RCH gate on `hz3`, job `j-30004650170124271`, passed all 298 tests under `nightly-2026-09-07`; [the inputs and raw log](artifacts/execution/2026-09-08-bridge-gate-inputs.json) identify the snapshot. Its tracker input precedes the final dependency import, which is checked separately. No native package was added or admitted.

## Ninth qualified batch: system-map and vocabulary checks

The complete isolated RCH gate passed **298 tests: 56 reference unit, 16 integration, 224 xtask and 2 doctests**, with zero failures, on `hz3` under `nightly-2026-09-07`. [The receipt](artifacts/execution/2026-09-08-epoch9-receipt.json) binds 133 source/build inputs and the explicit overlay; [the passing log](artifacts/execution/2026-09-08-epoch9-attempt2.log) follows a retained Clippy refusal.

The checker validates nine layers, 35 registered verbs, typed links, exact Knowledge variant sets, bounded command/address syntax and the checker-epic bead link. It distinguishes payload types and domain constructors from explicit Knowledge variants. Its 28 new negative/control tests do not implement a production command or a layer conformance suite. A full-gate injected retired command failed at the intended system-map check after earlier gates passed; the exact input and raw log are retained. The updated four-step DSR integration subsequently passed on unchanged clean commit `3d8fd5a7cb1bd25ddac1bbb69069ed6d9997039f`, including all 298 tests through RCH; [the integration record](artifacts/execution/2026-09-08-epoch9-gate-integration.json) retains the exact source binding and raw-log hashes. Later tracker and reality-check edits are outside that frozen-source result.

## Eighth qualified batch: runtime checkout and current declarations

The complete isolated RCH gate passed on `vmi1227854` under `nightly-2026-09-07`: **56 reference unit tests, 16 integration tests, 196 xtask tests and 2 doctests; 270 passed, zero failed**. [The receipt](artifacts/execution/2026-09-07-epoch8-receipt.json) binds 47 overlay files and 99 verified source/build inputs. [The raw log](artifacts/execution/2026-09-07-epoch8-code-attempt1.log) includes clean concordance and current-prose reports.

The gate now resolves the invocation's actual checkout; a cached binary cannot keep using a removed compile-time archive path. Current declarations are checked against registries and a named retained execution receipt, with bounded real-file reads, canonical receipt paths, symlink refusal and shared Markdown fence semantics. Historical counts remain historical. A receipt's fields are comparison inputs, not proof of execution by themselves.

The planted-concordance experiment stopped on the intended unknown invariant after earlier checks passed. The first DSR integration attempt is retained as failed: its second Cargo invocation exposed the stale archive path that this batch repairs. On 2026-09-08 UTC, the repaired committed revision `2e60924598f5283b8eeab5482087db87f154f106` passed all three required DSR checks, including the complete 270-test gate, with identical clean source identities before and after. A fresh injected-defect run on that same code refused the unknown invariant after source, admission and registry checks passed. [The integration record](artifacts/execution/2026-09-08-epoch8-gate-integration.json) links the exact raw logs and inputs. Production remains unimplemented, and mandatory Asupersync dependency/build/runtime admission remains incomplete.

## Seventh qualified batch: owned concordance and admission discrimination

The complete isolated RCH gate passed on `hz4` under `nightly-2026-09-07`: **56 reference unit tests, 16 integration tests, 174 xtask tests and 2 doctests; 248 passed, zero failed**. [The receipt](artifacts/execution/2026-09-07-epoch7-receipt.json) binds the 40-file overlay and 67 verified source/build inputs. [The passing log](artifacts/execution/2026-09-07-epoch7-attempt4.log) contains the actual concordance report: zero missing/dangling findings for 231 headings, 40 invariants, 21 hypotheses and 143 packets. The four canonical-reference corrections from the sixth batch are now checked by execution.

The admission compiler compares full source/name/version identities and exact target, feature and dependency-edge scopes. Real Cargo observations feed the same evaluator as the unit twins. The prior three failures are retained: test compilation, Clippy, then three unchanged regressions that caught a real source/path bypass missed by source review. Root restored external-first/path checks and made every evaluator refusal visible; no negative test was weakened. Both required target inventories and actual child-process failure paths executed. No external package is admitted, FA-053/054 remain incomplete, and this is no production qualification.

The five founding-source immutable blob identities were independently revalidated; [the scoped result](artifacts/execution/2026-09-07-founding-source-results.tsv) does not promote donor/runtime evidence. DSR wiring and an injected-defect full-gate experiment remain outstanding.


## Sixth qualified batch: explicit founding concordance

The sixth isolated RCH gate passed all **217 tests** on `ovh-a` with the reviewed concordance and its source-manifest binding. [The receipt](artifacts/execution/2026-09-07-epoch6-receipt.json) retains the exact six-file overlay and separates execution from the independent source audit: 231 required heading keys, 40 invariants, 21 hypotheses and 143 packets have direct typed mappings. The 67 engineering additions remain labeled additions serving the founding ideas. The full owned concordance checker is the next task; these coverage counts are not yet its output. **Correction at 22:58 UTC:** the manual audit normalized four shorthand section references, so its zero-unknown claim was too broad under the canonical-key contract. The rules bead was reopened and those references corrected; the owned checker must execute before reclosure.

## Fifth qualified batch: closed initial admission contract

The fifth isolated RCH gate passed on `ovh-a` under `nightly-2026-09-07`: **56 reference unit tests, 16 integration tests, 143 xtask tests and 2 doctests; 217 passed, zero failed**. [The receipt](artifacts/execution/2026-09-07-epoch5-receipt.json) binds the nine selected overlay files and [raw execution log](artifacts/execution/2026-09-07-epoch5-attempt1.log). It verified 40 source/build inputs and recollected both required target inventories. Apple metadata collection remains distinct from Linux compilation and execution.

The two local rows now carry checked source identity, version, target coverage, reason, ownership, decision, build closure and scoped runtime review. The policy binds the source-manifest digest; missing requirements, silent target removal, permissive external flags and false closure declarations refuse. Dependency-edge diagnostics preserve declared kinds, targets and malformed evidence independently of input order. The schema leaf is qualified; the general compiler's target/source discrimination remains unfinished, and no external foundation or production feature is admitted. Concurrent concordance edits were excluded from this batch.

## Fourth qualified batch: reviewed source snapshots

The fourth isolated RCH gate passed on `hz4` under `nightly-2026-09-07`: **56 reference unit tests, 16 integration tests, 125 xtask tests and 2 doctests; 199 passed, zero failed**. [The receipt](artifacts/execution/2026-09-07-epoch4-receipt.json) identifies the exact base and four overlay files; [the raw log](artifacts/execution/2026-09-07-epoch4-attempt1.log) records verification of all 39 reviewed source/build inputs before the remaining gate commands. The selected snapshot used committed concordance bytes, excluding concurrent concordance work.

The verifier requires exact file-set and SHA-256 equality, includes the six embedded registries, refuses symlink ancestors and leaves, bounds directory traversal, and permits a second hash utility only when the first is absent. Real filesystem and subprocess regressions passed. It relies on trusted operator hash tools and an externally frozen checkout; it does not provide an atomic snapshot or authenticate those tools. Full admission rows, runtime closure and external admission remain unfinished. No bead was closed for this intermediate prerequisite.

## Third qualified batch: capped reduction and owned registry checks

The third isolated batch passed on `hz4` under `nightly-2026-09-07`: **56 reference unit tests, 16 reference integration tests, 102 xtask tests and 2 doctests passed; zero failed**. Qualified source is committed in `84eeca9`; [the receipt](artifacts/execution/2026-09-07-epoch3-receipt.json) identifies the exact base, nine overlay files and passing [remote log](artifacts/execution/2026-09-07-epoch3-attempt3.log). The preceding Clippy failure and real Cargo ancestor-workspace discovery failure are retained. The latter was fixed by explicitly binding metadata collection to the requested `Cargo.toml`; its test was not weakened.

The reference reducer clips empirical weights per member and cohort using name-independent floor-proportional allocation, discards fractional remainders, rejects actor statements and preserves exact disqualifier dominance. It grants no authority and does not implement credibility or helper selection. The owned xtask now checks registry IDs, DAGs, declared links, retained files and scoped Rust test declarations. It rejects lexical lookalikes and escaping symlinks, and checks the actual compiler-reported identity against the qualified Linux commit. Those checks do not authenticate operator binaries or prove the referenced invariants. At that batch, full founding-concordance validation, source-snapshot verification and general dependency admission remained unfinished.

## Second qualified batch: full-input witnesses, frontiers and initial admission

The second isolated batch passed the complete gate remotely on `hz4` under `nightly-2026-09-07` (rustc `5a2be9f5f075d31e3ca5526b5b029881ce441253`): **44 reference unit tests, 16 reference integration tests, 69 xtask tests, one positive doctest and one private-field compile-fail doctest passed; zero failed**. The driver actually collected `cargo metadata --locked --offline --filter-platform` for `aarch64-apple-darwin` and `x86_64-unknown-linux-gnu`, checked their exact initial inventory, and resolved workspace/manifest/target paths against the real filesystem. Compilation and tests ran on Linux; collecting an Apple target graph is not Apple execution.

[Exact source hashes, command and scope](artifacts/execution/2026-09-07-epoch2-receipt.json) bind [the passing log](artifacts/execution/2026-09-07-epoch2-attempt2.log). [The preceding Clippy failure](artifacts/execution/2026-09-07-epoch2-attempt1.log) is retained; the fix preserved the same refusal semantics and suppressed no warning.

Opaque witnesses now retain all supplied bytes, profile, spans and omission semantics behind validated private construction. Product frontiers keep source/branch/projection/epoch and stages separate, requiring an explicit caller-trusted closing marker for closure. These remain reference models: no provider capture, authentication, durable frontier or production authority. The new bounded JSON reader and metadata checks strengthen the initial two-package gate; full admission-row/source/build/runtime closure and external positive cases remain unfinished. The exact lockfile guard remains unchanged. The reducer and registry compiler were excluded from this batch.

## First reference batch on 2026-09-07

The first code batch adds a frozen commit–reveal transcript model and a containment-reset rights model. The final complete gate passed remotely on `ovh-a` (`x86_64-unknown-linux-gnu`) using explicit `nightly-2026-09-07`, rustc `5a2be9f5f075d31e3ca5526b5b029881ce441253`: **34 reference tests passed, zero failed**, plus formatting, workspace compilation and Clippy with warnings denied. Eight new tests cover the round; six cover reset. Source was formatted before the isolated RCH transfer; exact base, overlay hashes, command and boundaries are in [the execution receipt](artifacts/execution/2026-09-07-reference-epoch-1-receipt.json), with [the raw gate log](artifacts/execution/2026-09-07-reference-epoch-1-final.log). An earlier pass on an older nightly and a worker-pressure refusal are retained alongside them.

The round uses a deliberately non-cryptographic FNV comparison and proves no cryptographic property. Reset preserves spent rights, epochs and dispatched/unknown liabilities while incrementing an in-memory incident counter; it does not restore an actor, persist the counter or enforce escalation. FA-056 remains incomplete, and production FA-108 remains planned. The separate admission/parser work in progress was excluded from this frozen batch and receives no execution credit from it. The 2026-09-06 and revision 0.2 history below remains unchanged.

| Surface | Actual status | Claim boundary |
|---|---|---|
| Integrated comprehensive architecture | Revision 0.3 draft, founding essays as the spine | Requirements and research hypotheses, not deployed features |
| Founding-ideas concordance | `docs/FOUNDING_IDEAS.md` and `registry/founding_concordance.json`: 38 founding ideas, 9 syntheses, 8 labeled non-literal imports, 67 engineering additions | Traceability of the design, not evidence that any mechanism works |
| Agent-facing surface (tower, Knowledge wrapper, addresses, vocabulary, journal, situation, explain, affordances, rehearsal, annotations, handoffs, precheck) | Plan §6.7, §6.8, §17.2, §17.3, §17.8, §17.11, §17.12; `docs/SYSTEM_MAP.md`, `docs/AGENT_GUIDE.md`; packets FA-132 through FA-143; beads | No command exists; the only executable is the local gate driver |
| Founding essays | Both read in full again for revision 0.3; head commits recorded in `registry/sources.json` | Complete premise; no missing post |
| Ten donor/build project deep dives | Targeted code/manifest/plan review at fixed refs (revision 0.2) | Exact files/scopes recorded; not full repository audits or builds |
| Pure-Rust reference workspace | Source present, zero external packages, formatted with `cargo fmt --all` on 2026-09-06 | Selected logical semantics only; not a broker |
| Rust test functions | **105 unit + 156 integration reference tests, 242 xtask tests and 8 doctests passed remotely on 2026-09-08 UTC**; earlier runs remain dated below | Exact frozen batch above; not production evidence or credit for other work in progress |
| Rust local gate driver | **Complete reference gate PASS remotely on 2026-09-08 UTC** under rolling `nightly`, frozen identically as `nightly-2026-09-08`; earlier local 2026-09-06 gate retained below | Initial inventory, registry-core and full concordance checks execute; external admission remains incomplete; new source requires a new run |
| Production release gate | Explicitly refuses release | No qualified broker/toolchain/target/signing closure |
| Machine-readable registries | 40 invariants, 21 hypotheses, 143 packets, 5 SLO targets, founding concordance (38 ideas, 9 syntheses, 67 engineering additions), preregistration ledger with no preregistered protocol yet, system map (9 layers) and vocabulary (26 nouns, 35 verbs, 12 reason codes) | Static structural checks are not proofs of their claims |
| Beads task graph | All 143 roadmap packets have full owners; 336 live beads at this checkpoint: 28 closed, 305 open, 2 in progress and 1 blocked | FA-001, FA-004, FA-005, FA-057, FA-058, FA-059, FA-063, FA-068, FA-081 and FA-092 are closed within their reference contracts; FA-002 remains in progress and FA-053 is blocked. Reference, provenance and checker closures do not qualify production. Earlier graph counts describe their dated checkpoints |
| Production control broker / persistence / containment | Not implemented | Reference code performs no external effects |
| Native foundation adapters | Planned, admission blockers recorded | A reviewed source file is not an integration |
| Trained helpers / codecs / signatures / surprise residual / rewind | Research and implementation plans | No measured safety, compression, detection or containment result |
| Receipts, assurance profiles, passports, autonomy ledger, canaries, risk-theater detector, formal anchors | Plan subsections, invariants FA-INV-035 through FA-INV-038, packets FA-117 through FA-131, beads | No verifier, no proof, no profile exists yet |
| Local DSR quality integration | All four required checks executed and passed on unchanged clean revision `3d8fd5a`, 2026-09-08 UTC; Cargo checks ran through RCH | Reference/operator gate only; no production build, signing or release profile |
| Public repository mutation | The operator committed and pushed earlier revision 0.3 batches during the working session; later changes remain in the local working tree until committed | This document does not track remote state |

## Execution facts recorded on 2026-09-06

All logs are under [`artifacts/execution/`](artifacts/execution/). Each records host, date, toolchain identity and the exact command.

| Log | Command | Toolchain | Result |
|---|---|---|---|
| `2026-09-06-cargo-test.log` | `cargo test --workspace --all-targets --frozen` | rolling `nightly` (rustc c54751567 2026-08-22) | 20 passed, 0 failed |
| `2026-09-06-xtask-check.log` | `cargo run --locked -p xtask -- check` | rolling `nightly` | FAIL at `cargo fmt --all --check`: this host's rolling nightly directory lacks the `rustfmt` and `clippy` binaries although rustup reports the components installed |
| `2026-09-06-xtask-check-nightly-2026-08-31.log` | `cargo fmt --all --check` (18 diffs), then `cargo fmt --all`, then the gate | `nightly-2026-08-31` | Formatting applied; gate reached `cargo check` and was refused by the host's remote-build offload wrapper (no admissible workers) |
| `2026-09-06-xtask-check-nightly-2026-08-31-local.log` | `cargo run --locked -p xtask -- check` with the offload wrapper bypassed | `nightly-2026-08-31` (rustc 908501772 2026-08-30) | **PASS local_reference_gate**: lockfile inventory, source inventory (required files and the crate-level `unsafe` prohibition), `rustc -Vv`, `fmt --check`, `check`, `clippy -D warnings`, `test` (20 passed) |

What these facts do and do not establish:

- The reference source compiles, is Clippy-clean under `-D warnings`, is formatted, and its 20 tests pass on one aarch64 macOS host with one dated nightly. The formatting change is the only source mutation and is the documented preparation step.
- The gate did **not** pass under the rolling `nightly` alias on this host, because that toolchain directory is missing components; that is a host-environment defect, recorded rather than hidden. `rust-toolchain.toml` still tracks `nightly`; a release campaign freezes a dated identity anyway (plan §2.2).
- The historical fact stands: the revision 0.2 preparation environment did not compile this workspace. That prose is retained in the validation report and is not rewritten.
- Nothing here is production, release or safety evidence. Packets FA-003 and FA-004 remain open until the gate runs under the rolling nightly with its declared components and the registry checks move into the owned `xtask`.

## Founding-idea coverage

| Founding idea family | Mechanism in plan | Reference-model check | Production status |
|---|---|---|---|
| Effect gate, one-shot permits, conserved rights (FI-A01, FI-A02) | §8 | `effect_binding_and_one_shot_dispatch`, `rights_unknown_cannot_be_refunded_by_cancel`, `revocation_fences_reserved_not_history`, `reserve_budget_and_duplicate_fail_closed` and `resolved_non_effect_returns_rights_once`; executed 2026-09-06 | planned |
| Congress: independent votes, commit–reveal, consequences, credibility, escalation reports (FI-A06 through FI-A09, FI-A13 through FI-A16) | §9.2 through §9.10 | Nine bounded commit–reveal unit tests plus public-API framing/replay tests executed 2026-09-07; non-cryptographic comparison only. Twelve capped empirical reducer tests also passed in epoch3. Consequence lattice and credibility remain unimplemented | planned |
| Activation channel, sidecar-to-congress, certified margins (FI-A05, FI-I04, FI-I06) | §10 | `integer_probe_requests_refinement_at_margin`, `integer_probe_bound_matches_exhaustive_small_errors`; executed 2026-09-06 | planned |
| Elicitation, signatures, honeypots, surprise (FI-A10 through FI-A12) | §12.6, §12.7 | none; research packets FA-109 through FA-111 | planned |
| Rewind as containment (FI-A09, FI-A16, FI-I03) | §11.10 | Six bounded reset tests executed 2026-09-07: rights, spent units, epochs, dispatched/unknown effects, counter monotonicity and overflow. No host restore or persistent escalation | planned |
| Thought graph, practice, strategies (FI-I09, FI-I15 through FI-I20) | §11.4, §11.5, §11.9 | none; experiment-plane packet FA-045 | planned |
| Risk-theater governance (FI-A18) | §9.6, §9.9, §22.4 | none; FA-INV-032 has no checker yet | planned |

The previous Python reference and hosted-runner workflow remain removed. Their historical results do not validate the Rust source. Some original reference coverage, including complete congress/statistical utilities, must be ported and revalidated under packet FA-056; the Rust reference is not labeled feature-equivalent.

Preparation results for revision 0.2 are in [artifacts/VALIDATION_REPORT.md](artifacts/VALIDATION_REPORT.md), with a revision 0.3 addendum. Production invariant statuses remain planned and safety waivers forbidden.
