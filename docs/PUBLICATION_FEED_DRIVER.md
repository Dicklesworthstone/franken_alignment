# Supervised change-feed ingestion

`FileSupervisedDriver::step_with_publication_feed` runs concrete change-window
catch-up, committee/policy capture and witness-file capture through the original
supervised review/authorize/two-key dispatch/publication/reconciliation machine.
`step_from_files_with_publication_feed` uses the original concrete native
policy-source adapter instead of the callback. Neither introduces a second
rights ledger, executor, helper protocol or external effect sink.

Configure the existing publication validation, change source and heartbeat
freshness policies before proposals. Bind the original witness-file source and
reviewed recipe to each attempt. Supply a `PublicationFeedFile` matching the
configured change source and a `PublicationInputFile` matching that attempt's
witness producer. Feed batches use the contract in PUBLICATION_FEED_TRANSPORT.md.

## Why acquisition order matters

The feed first durably withdraws global feed eligibility, reads the complete
bounded window, checks overlap and commits its new suffix and heartbeat. These
notices can withdraw the active attempt and increment its publication-input
revision. ONLY THEN does the original PublicationProvider begin the attempt's
witness capture, obtaining the new revision and withdrawing witness eligibility
before external committee code. Putting catch-up inside an already started
witness capture would leave its saved revision stale after relevant changes.

There are at most two feed reads per step: authorization and dispatch. First
publication acquires again; settled/expired/original query-only phases skip all
providers. Feed reads occur even when the window is unchanged, but identical
covered notifications are not applied again and rereads cannot extend expiry.
Every effect check independently revalidates the current lease after captures.

`FileFeedDriverReport` separates acknowledged feed reports/read errors from
committee observations, native policy-source updates, witness reads and the
actual driver result. Missing or expired feed coverage is not fabricated
committee-input drift. The native gate refuses it, retaining an unspent automatic
permit for a genuine retry where applicable. Installation failures stop before
committee/witness capture, with the original owner quarantined.

After dispatch, missing coverage or changed witnesses use original sealing;
only receipt reconciliation refunds confirmed nonexecution. A caught committee
unwind occurs after witness withdrawal and the driver has already retired its
publication send phase. Neither catch-up nor recovery resurrects effect keys.

The three-reader API preserves independently configured native policy-source
leases. A fresh feed cannot compensate for an expired policy source. Reads are
sequential, not an atomic snapshot across producers. Source authenticity,
real-world notification completeness, filesystem control and the agreed elapsed
clock remain host assumptions. This is not a network transport or a background
polling service; the embedding scheduler invokes the original driver steps.
The older source-completion API does not automatically acquire this new feed.

## Verification

Eight new helper-socket integration tests cover active-revision catch-up,
post-dispatch phantoms, retained-permit retry, gap repair, lease expiry during
capture, caught provider unwind, terminal precedence and native policy leases.
Two missing `fn` keywords in earlier heartbeat regression declarations were
also repaired without changing their assertions. Lexical/delimiter, declaration
and authored whitespace screens passed; new-file blob hashes match local files.
These are not compiler or runtime results. RCH is unavailable (exit 127), so
Rust compilation, formatting, Clippy and all new tests remain UNEXECUTED.
FA-061/062 remain open pending the exact integrated revision's required verifier.
