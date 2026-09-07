# Draft interoperability schemas

The three original JSON Schema examples retain their `0.1` wire labels because the design revision number is not an automatic wire-format change. They describe action proposals, capability manifests and evidence claims; they are not production authenticated encodings.

The schemas use bounded JSON integers within the original interoperable range. The Rust reference has separate exact typed integers and is not a serializer for these schemas. Production canonical encoding, migrations, authenticated transcripts, duplicate-field handling and numeric profiles remain contract/compiler work.

The Python schema checker from the first draft is removed. Preparation may use external standard tools to check these static documents, but no Python runtime is shipped or needed by the Rust reference. The current xtask is not a general JSON Schema validator. Full in-family format/registry compilation and conformance must precede live protocol use.
