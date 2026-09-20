# Coherent producer acquisition (FA-061 / FA-062)

`FileOversight::refresh_publication_from_producer` acquires the coupled producer
bundle once and acknowledges its feed and witness projections together. It takes
the existing explicit `PublicationInputFile` and `PublicationFeedFile` readers;
they must pin the same path and full producer profile. The source reader must
also pin this exact attempt/action, whose original source is already cut-bound.
No alias resolution, file I/O or profile strengthening occurs during preflight.

The owner durably withdraws feed eligibility and then witness eligibility before
reading. These are two separately acknowledged events. A single bounded regular-
file read and full bundle decode yields both original-format projections. After
the trusted clock sample, the original ingester checks retained overlap and stages
the unseen notice suffix plus heartbeat. Only then does it obtain the resulting
witness revision and stage the original capture event. One canonical replacement
acknowledges both projections; no new journal tag, permission or effect owner exists.

This removes the second producer-file open that could see a snapshot ahead of the
just-read feed. It does not lock the producer during committee capture or effect
execution. Changes after the read remain governed by the declared observation
contract and lease; every subsequent boundary must acquire again. Separate policy
sources and different producer paths are not magically made one snapshot.

The return value is an acknowledged `(FileCaptureIdentity, PublicationFeedReport)`,
not a permit. A missing/malformed file returns the original inner read error with
both withdrawals retained. Successful decode followed by clock unwind, conflict,
missing coverage, capacity, replay or storage failure quarantines the live owner.
No installed report escapes a failed canonical replacement. Original recovery
fences old keys; original receipts alone settle effects. Strict cut/producer floors,
exact whole-input/witness validation, credentials, source leases and both approval
keys remain independent requirements. A fresh read does not renew producer time.

Native buffer, packet, notification, journal and recovery-reserve bounds remain.
Bundle access still requires permission to see the full input, not just its feed.
This is coherent local observation, not authenticated origin, remote completeness,
or an atomic transaction across producer and effect stores.

Eight real-file regression tests cover catch-up before input revision selection,
a producer replacement after the read, negative dependency/positive controls,
read-loss retry with original keys, foreign and raw reader refusal, expiry,
failed final replacement and caught clock unwind. They are authored, not executed.
RCH, cargo and rustc are unavailable here; compilation, formatting, Clippy and the
exact-revision xtask gate remain outstanding. FA-061/062 remain open.
