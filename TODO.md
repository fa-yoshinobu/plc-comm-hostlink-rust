# TODO: Host Link Communication Rust

Current active TODOs only.

## Current Status

The approved implementation items and the six cross-library overhaul items are
complete in the working tree. The evidence-dependent comment-encoding decision
remains open, and no comment-decoder implementation change is authorized until
`HL-EVAL-TODO-006` is approved.

### Verification evidence — 2026-08-01

- Current-worktree formatting, Clippy, and test gates passed: 41 library tests
  and 67 integration tests, with all executable examples compiled.
- A synthetic current-worktree Git tree produced a self-contained source
  archive; clean extracted format/check/Clippy/rustdoc/test gates passed.
- The crates.io package-content guard and `cargo package --allow-dirty` passed;
  the package kept tests and repository-only tooling out of the registry crate.
- Codex self-review removed the queued wrapper and bit-in-word write helper,
  checked the FIFO/generation/error/deadline/read-plan contracts, fixed the
  package-only Tokio feature finding, and reran every affected gate.
- These deterministic validation and packaging corrections do not require
  live PLC communication. `HL-EVAL-TODO-006` is intentionally still open.

## HL-EVAL-001 — Reject Float32 writes to direct bit devices before transport

### Implementation scope

- Rust high-level Float32 write planning in the ordinary FIFO client API
- Every direct bit device family accepted by the address parser, including `Y`, `R`, `B`, `MR`, `LR`, `CR`, `VB`, `X`, `M`, and `L`

### Target contract

Float32 (`F`) writes are supported only for word devices. A direct bit target is rejected as caller input before frame construction or transport; the implementation must not reinterpret, split, retry, or send the Float32 bit pattern as consecutive bit writes.

### Compatibility impact

Calls that previously could emit unintended multi-bit writes now fail before communication. This is an intentional safety correction; no compatibility alias or fallback is retained.

### Acceptance criteria

1. `Y0:F` and `R0:F` writes fail with the documented Rust argument-error variant before any transport call.
2. Every supported direct bit family follows the same rejection path, while valid word-device Float32 writes retain their defined two-word encoding.
3. Client, named, and helper write paths cannot bypass the validation.
4. Regression tests prove zero sends for rejected writes; live PLC writes are not required for this safety guard.

### Completion checklist

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live-PLC verification is recorded as not required, or each required check has evidence or an explicit release disposition.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## HL-EVAL-005 — Normalize banked bit ranges before calculating bounds and point counts

### Implementation scope

- Rust profile/device-range metadata for `R`, `MR`, `LR`, and `CR`
- Public lower-bound, upper-bound, point-count, and display-range properties

### Target contract

Banked bit addresses are parsed as a decimal bank plus a final bit field `00..15`, and their logical index is `bank * 16 + bit`. Numeric bounds and point counts use the logical index, while the public display range preserves PLC notation. Profile catalog ranges remain descriptive metadata and are not communication-library pre-send address guards.

### Compatibility impact

Incorrect numeric bounds and point counts change to their logical values. Display addresses remain in PLC notation, and no new transport-side range rejection is introduced.

### Acceptance criteria

1. All catalog rows for `R`, `MR`, `LR`, and `CR` produce logical lower/upper indices and exact point counts from `bank * 16 + bit`.
2. KV-8000 `R00000..R199915` reports 32,000 points and `MR00000..MR399915` reports 64,000 points.
3. Invalid final bit fields outside `00..15` are rejected by catalog parsing/tests.
4. Address-range display text remains unchanged and transport APIs do not enforce profile catalog bounds.
5. Equivalent vectors agree with the Python and .NET implementations.

### Completion checklist

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live-PLC verification is recorded as not required, or each required check has evidence or an explicit release disposition.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## HL-EVAL-TODO-006 — Determine the Host Link device-comment encoding contract

### User disposition

Deferred by the user on 2026-08-01 for evidence investigation followed by implementation in the next Host Link implementation cycle. The current UTF-8-first/Shift_JIS-fallback behavior is not approved as the final contract. Do not change the decoder in the current implementation batch; investigate the exact profile-specific byte contract first, present the resulting target contract one item at a time, and implement only after explicit approval.

### Implementation scope

- Rust `RDC` device-comment decoding and its public helper/client APIs
- Cross-language comparison with the Python, Node-RED, and .NET Host Link implementations
- Shared Host Link user documentation where the resulting behavior is common

### Target state

The encoding of `RDC` device-comment response bytes is defined from direct KEYENCE Host Link evidence for every affected PLC profile. The Rust implementation does not infer a target contract merely from successful decoding, a general KV string-encoding statement, or existing UTF-8-first/Shift_JIS-fallback behavior.

Until the evidence is complete and the resulting target contract is explicitly approved, the comment-encoding behavior remains undecided and no implementation change is authorized.

### Compatibility impact

Undecided. The investigation must identify whether the approved result preserves the current UTF-8-first/Shift_JIS-fallback behavior, fixes one encoding, selects encoding by PLC profile, or introduces an explicit API setting. Any public API, default, decoding, error, or migration impact must be recorded before implementation.

### Acceptance criteria

1. Official KEYENCE communication documentation is checked for the `RDC` response encoding for KV-NANO, KV-3000/KV-5000, KV-7000/KV-8000, and KV-X500 families; evidence is recorded per profile rather than inferred across families.
2. The exact codec contract is identified, including whether “Shift_JIS” means strict Shift_JIS, Windows-31J/CP932-compatible decoding, or another defined mapping.
3. Ambiguous byte sequences that are valid under both UTF-8 and Shift_JIS are included in deterministic decoder vectors, and the expected result follows the approved evidence rather than decoder ordering.
4. If official documentation does not settle a profile, that profile remains `unverified` until an exact live-PLC evidence plan is written with the PLC/profile, endpoint, address, read intent, registered comment value, purpose, expected raw-byte evidence, and restoration requirement, then separately approved by the user with `OK` before communication.
5. A maintainer decision record defines the encoding selection mechanism, malformed-byte behavior, connection invalidation behavior, public API impact, compatibility impact, and cross-language mapping before source implementation begins.
6. User documentation, tests, generated API reference, and migration notes agree with the approved contract in every affected implementation.

### Evidence and completion checklist

- [ ] Official `RDC` encoding evidence recorded for every affected PLC family/profile.
- [ ] Shift_JIS versus Windows-31J/CP932 mapping resolved for all four language runtimes.
- [ ] Ambiguous and malformed byte vectors defined with evidence-backed expected results.
- [ ] Need for live PLC verification decided; any required exact live batch is separately documented and approved.
- [ ] Target contract and compatibility impact explicitly approved by the user.
- [ ] Implementation completed in every affected repository.
- [ ] Tests added or updated for every acceptance criterion.
- [ ] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [ ] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [ ] Required live-PLC checks passed, or each unavailable check has an explicit release disposition.
- [ ] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [ ] Final acceptance criteria verified and the item marked complete.

### Current evidence boundary

The current implementations try UTF-8 first and fall back to Shift_JIS. KEYENCE material stating that KV-series strings use Shift_JIS is relevant but does not by itself establish the byte contract of every Host Link `RDC` response. It is supporting evidence only, not approval of a Shift_JIS-only implementation.

## HL-EVAL-012 — Separate Rust bit-write input parsing from PLC bit-response parsing

### Implementation scope

- Direct bit, bit-in-word, named, and typed read paths
- Strict Boolean caller input for direct-bit writes

### Target contract

Caller write input and PLC-response parsing are separate. Direct-bit caller input is a Rust `bool` only. A PLC bit response token is accepted only when it is exactly `0`, `1`, `OFF`, or `ON`; `TRUE`, `FALSE`, lowercase forms, surrounding whitespace, and all other tokens are protocol errors that invalidate the connection. Client-side bit-in-word read-modify-write is not exposed.

### Compatibility impact

Permissive PLC-response spellings and numeric/text caller-side Boolean aliases are removed. The former bit-in-word read-modify-write helper is removed without an alias.

### Acceptance criteria

1. Exact `0`, `1`, `OFF`, and `ON` map to the documented Boolean results in every response-consuming path.
2. `TRUE`, `FALSE`, lowercase variants, whitespace variants, empty, and arbitrary tokens return the Rust protocol-error category and close the receiving connection.
3. Direct bit, bit-in-word, named, and typed paths use the same response parser.
4. Public API/source inspection contains no bit-in-word write or read-modify-write helper.

### Completion checklist

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live-PLC verification is recorded as not required, or each required check has evidence or an explicit release disposition.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## HL-EVAL-013 — Validate every Rust timer/counter response field

### Implementation scope

- Timer/counter typed reads and composite helpers in the FIFO client API
- Unsigned, signed, double-word, long, and hexadecimal response formats

### Target contract

A timer/counter response contains exactly three tokens: status, current value, and preset value. Status is exactly `0` or `1`; current and preset are both validated according to the requested format: `U` as decimal `u16`, `S` as decimal `i16`, `D` as decimal `u32`, `L` as decimal `i32`, and `H` as hexadecimal `u16`. A helper returning only one field still validates all three first.

### Compatibility impact

Responses with ignored garbage fields, extra fields, or loosely parsed values now fail as protocol errors and invalidate the connection.

### Acceptance criteria

1. Exactly three tokens are required, with no missing or additional token accepted.
2. Status, current, and preset boundary and overflow vectors are validated for every supported format.
3. Typed reads that return preset or current validate the unreturned fields before returning.
4. Composite helpers return all three validated values without a second inconsistent parser.
5. Any malformed field returns the protocol-error category, closes the connection, and is not retried.

### Completion checklist

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live-PLC verification is recorded as not required, or each required check has evidence or an explicit release disposition.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## HL-EVAL-014 — Enforce the IPv4-only Host Link endpoint contract in Rust

### Implementation scope

- Rust TCP and UDP endpoint parsing, hostname resolution, socket binding, and error reporting
- User documentation and examples for Host Link network configuration

### Target contract

Host Link endpoints are IPv4-only because the target PLC configuration exposes IPv4, not IPv6. IPv6 literals are rejected before transport with a clear argument error. Hostnames use only resolved IPv4 addresses; a hostname with no IPv4 result fails as a connection error. UDP binds an IPv4 local socket. No IPv6 support or discovery task is created.

### Compatibility impact

Accidental IPv6 attempts fail deterministically instead of reaching an incompatible socket operation. Supported IPv4 literals and IPv4-resolving hostnames retain their behavior.

### Acceptance criteria

1. IPv4 literals and hostnames with an IPv4 result use an IPv4 TCP/UDP transport.
2. IPv6 literals fail before socket creation or send with the documented argument error.
3. A hostname resolving only to IPv6 fails with the documented connection error and sends nothing.
4. Documentation states the IPv4-only endpoint contract without presenting IPv6 as pending support.

### Completion checklist

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live-PLC verification is recorded as not required, or each required check has evidence or an explicit release disposition.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## HL-EVAL-015 — Reject empty Rust named reads and invalid poll intervals

### Implementation scope

- Named-read and polling helpers in the ordinary FIFO client

### Target contract

Named reads and polls require at least one address. Poll durations must be strictly greater than zero. Empty address collections and zero durations are rejected as caller input before communication, FIFO admission, or result production.

### Compatibility impact

No-op reads and tight zero-duration polling loops no longer report success or produce empty results.

### Acceptance criteria

1. Empty named-read and poll address collections fail before any send.
2. A zero `Duration` fails before communication or result production; positive durations remain supported.
3. Rejected operations do not enter or delay the FIFO operation gate and are not retried.

### Completion checklist

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live-PLC verification is recorded as not required, or each required check has evidence or an explicit release disposition.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## HL-EVAL-016 — Remove the Rust queued-client surface

### Implementation scope

- Former queued wrapper, factories, callbacks, and inner-client visibility/API surface
- Validation CLI and repository tooling
- Operation-gate coverage for all public `HostLinkClient` operations

### Target contract

One ordinary `HostLinkClient` owns FIFO admission and one wire turn across all clones. The former queued wrapper, callbacks, and bypasses are removed without aliases. Every public logical operation uses the ordinary client gate for its full logical operation, and repository tooling uses the same supported client surface.

### Compatibility impact

External source using the queued type or its members stops compiling. Callers use `HostLinkClient`; no compatibility alias or second behavior remains.

### Acceptance criteria

1. Public generated API documentation and source inspection contain no queued-client type, factory, callback, or alias.
2. Multi-segment named reads retain one FIFO turn for their complete logical operation.
3. Repository validation CLI/tests use the ordinary supported client without a bypass.
4. `HostLinkClient::new`, `HostLinkClient::connect`, and `open_and_connect` remain explicit supported entry points; no automatic retry is added.

### Completion checklist

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live-PLC verification is recorded as not required, or each required check has evidence or an explicit release disposition.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## HL-EVAL-024 — Make the GitHub source archive self-contained for standard build and test commands

### Implementation scope

- Git attributes/archive rules, Rust tests and fixtures, Cargo configuration, and source-archive release gate

### Target contract

The GitHub source archive includes the repository tests and all fixtures required by them. From a clean extracted archive, the documented standard Cargo build and test commands complete without references to intentionally omitted files. Registry packages remain minimal and follow their separate package-content contract.

### Compatibility impact

GitHub source archives become larger because test assets are included; the published crate content does not expand as a consequence.

### Acceptance criteria

1. An archive produced from repository HEAD contains all tests and every fixture referenced by `include_str!` or other test code.
2. Documented formatting, check, clippy, documentation, and all-target test commands run from the extracted archive with the expected tests.
3. The release gate creates a fresh archive, extracts it, and verifies those commands without checkout-only files.
4. Crate package-content checks independently enforce the approved minimal registry package.

### Completion checklist

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live-PLC verification is recorded as not required, or each required check has evidence or an explicit release disposition.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.
