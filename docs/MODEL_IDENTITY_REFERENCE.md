# Model identity and anchor liveness reference

## Numerical identity baseline

`activation::identity` implements immutable model-space manifests and mandatory
activation anchors for plan 7.10 / FA-118, grounded in FI-A01 and FI-A10. Its
consumer is the oversight identity lane, not an actor-controlled model switch.
This is scoped reference progress; the original native capture, signature,
calibration and adversarial qualification obligations remain open.

A ModelManifest binds supplied commitments for weights, adapters, tokenizer,
architecture and numerical profile, along with tenant/model/host/tokenizer
identities. These are structural commitments, not signatures or hashes computed
from actual model files by this code. A ModelPassport freezes the manifest and
its complete anchor set. Matching dimensions alone cannot substitute for matching
model, tap, layout, stream and stimulus position.

Each IdentityAnchor freezes reference stimulus token IDs and inclusive finite
binary32 bounds for every observed coordinate. Comparison reads the actual
immutable SourceFrame bits. It scans every coordinate, counts outliers and retains
the first outlier's exact bits and bounds. No mean error can hide an outlier;
no norm, subtraction or rounded tolerance calculation widens the acceptance box.
Signed zero, subnormals and extreme finite endpoints are explicit cases.

At most sixteen anchors, 65,536 total coordinates and 8,192 total stimulus tokens
are registered. Malformed bounds, nonfinite values, duplicate anchors and mixed
model spaces refuse. Registration and capture provenance are trusted inputs.
The kernel does not execute stimuli, prove that a host served the claimed model,
or establish that passing anchors distinguish all possible substituted models.

## Verification

Five kernel test functions pair inside-bound drift with outliers, changed capture
identities, invalid registrations and aggregate limits. Rust compilation, tests,
formatting, Clippy and the required RCH gate have not run in this session; no
configured Rust/RCH runner was available. No bead is closed and historical
receipts do not qualify these additions. No Cargo dependency was added.
