# Defensive library contract changes

These alpha changes repair public library helpers. They do not establish that
CLI execution routes through every helper, or certify a combined release.

## Path containment

`validate_path_within_base` now resolves existing filesystem components before
checking containment. Existing symlinks to locations outside the base are
rejected. Relative bases work, and successful results are absolute, with existing
components canonicalized. A missing base or child can be resolved from an
existing ancestor. Dangling symlinks, non-directory ancestors, and other
filesystem errors fail instead of falling back to a lexical prefix check.

Callers comparing the returned path text may observe canonical names instead of
symlink aliases. This is a point-in-time check: another actor can replace path
components before a later open. Use directory handles and appropriate open flags
when concurrent replacement is part of the threat model. This helper is not a
filesystem sandbox or an atomic authorization-and-open operation.

## Signature identity and trust

Verification now requires bundle key ID and algorithm to match the supplied key.
The result identity and trust decision come from that key, including the identity
reported for failed verification. Modifying unsigned bundle metadata cannot
substitute a trusted ID while verifying with a different identified key.

The caller must select the key and its identifier from a controlled key store.
`SigningKeyPair` still has a caller-assigned, mutable ID; this change does not
authenticate untrusted key objects. The current primitive remains symmetric
keyed BLAKE3, not asymmetric public-key signing. Bundle timestamps remain unsigned.
Cryptographic validity and trust remain separate: a revoked but correctly signed
artifact returns `valid = true` and `TrustLevel::Revoked`. Callers must apply
their acceptance policy rather than checking only `valid`.

## Audit state and verification

`ImmutableAuditStore::record` now commits its in-memory chain state only after
the storage backend acknowledges an append. A backend failure before writing
therefore leaves the next successful record contiguous. `open` checks stored
chain consistency before resuming; modified event or chain hashes fail. Only a
missing file is treated as an empty log; other file-read errors propagate.

The new `ImmutableAuditError::SequenceExhausted` variant requires an added arm in
exhaustive downstream matches. The store returns this error instead of wrapping
or panicking when a next sequence cannot be represented. Hash-chain and offline
verification reject wrapped sequences. The additive `HashChainState::try_append`
returns `None` without changing state on exhaustion; the existing `append`
signature is retained and explicitly panics on exhaustion without changing state.

This is not a transactional storage protocol. A file/backend error can occur
after a partial write, and cancellation may leave an uncertain append outcome.
Inspect and reopen such storage before retrying. The file backend's flush is not
a new crash-durability guarantee; concurrent writers also need coordination.
Verification retains the existing slice contract: sequences may start above
zero, and the first predecessor cannot be checked without an external anchor.
Timestamps are not hashed, original event bytes are not stored, and an unkeyed
chain cannot prevent whole-log rewriting or prove completeness. Use `open` to
resume existing storage; `new` still starts a fresh in-memory chain.

## One-time password retrieval

`clear_on_retrieve`, including the high-security configuration, now retrieves and
removes a valid cached entry under one lock. Concurrent readers can obtain that
entry at most once. Ordinary caches retain valid entries, track successful uses,
and evict expired entries encountered during retrieval. Hit, miss and expiration
counts reflect these outcomes.

Removing an entry zeroes its stored bytes. The existing API returns an owned
`String`; the caller remains responsible for that copy's lifetime. This does not
promise removal of every copy from process memory.
