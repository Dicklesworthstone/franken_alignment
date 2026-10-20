# Checked publication and native evaluation continuity

Plan §9.9 and §16–18; scoped continuation of credibility-ledger-dc0.4.
This is source implementation, not execution qualification or bead closure.

## Bootstrap without an inferred reserve

The native checked constructors now support both explicitly reserved live
publication and unreserved offline accounting. `create_checked_andon_guarded`
and `create_checked_joint_andon_guarded` use the same private bootstrap and
canonical publication as their reserved variants. The witness gate, evaluator
protocol, optional joint policy, and Andon configuration enter the first image
before any owner or role escapes. Invalid witness limits refuse before creating
a store. Existing reserved encodings and their placement remain unchanged.

The `PublicationLimits` import and `RecoveryReserve` path in the previously added
checked constructors are corrected. No new authority, witness reducer, evaluator,
journal encoding or dependency is introduced.

Recovery still requires the exact witness limits, complete native protocol and
window inventory, original guards, independent floors and exact history anchor.
A missing or changed witness requirement does not permit a weaker recovery.
Applied cases and censored labels survive; resolving a benign label after restart
requires a fresh native clock and fresh evaluator custody. Accounting never
changes the original authority state or substitutes for publication witnesses.

Four regression functions use the original native file owner and review fixtures:
all four joint/reserve combinations, real hold/censor/recovery/benign accounting
with exact retry and old-role rejection, each witness-limit mismatch versus a
permitted recovery, and invalid bootstrap versus an otherwise identical permitted
bootstrap. Existing tests and assertions are unchanged.

Required compilation and tests must run through the repository's remote RCH gate.
The authoring environment has no RCH launcher; source/blob/patch checks are not
Rust compilation or runtime evidence. No production or statistical claim follows.
