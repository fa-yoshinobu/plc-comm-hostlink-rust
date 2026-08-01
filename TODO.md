# TODO: Host Link Communication Rust

Current active TODOs only.

## Current Status

The approved implementation items and the six cross-library overhaul items are
complete in the working tree. The Rust portion of `HL-EVAL-TODO-006` is also
implemented and verified with explicit UTF-8 or CP932 selection and an exact
raw-byte path.

### Verification evidence — 2026-08-01

- Current-worktree formatting, Clippy, warning-denied rustdoc, and test gates
  passed: 44 library tests and 69 integration tests, with all executable
  examples compiled. Rust 1.85 all-target/all-feature compilation also passed.
- The crates.io package-content guard and `cargo package --allow-dirty` passed;
  the 28-file package kept tests and repository-only tooling out of the registry
  crate, retained all 7 declared examples, generated warning-free rustdoc, and
  compiled through an isolated consumer that imports both encoding variants.
- Codex self-review removed the queued wrapper and bit-in-word write helper,
  checked the FIFO/generation/error/deadline/read-plan contracts, fixed the
  package-only Tokio feature finding, and reran every affected gate.
- The current-worktree source archive contained 47 files, 10 sample files, and
  2 test files; its isolated format/check/Clippy/rustdoc/test gate passed.
- The deterministic `HL-EVAL-TODO-006` decoder, raw-byte, validation-order, and
  packaging checks do not require additional live PLC communication.

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

The target contract was approved by the user on 2026-08-01. An `RDC` comment encoding must not be fixed by the library or PLC profile and must not be guessed by UTF-8-first/Shift_JIS-fallback decoding. Text decoding requires an explicit caller-selected encoding, and exact raw comment payload bytes remain available. The approved cross-language public selections are UTF-8 and CP932/Windows-31J; KEYENCE documentation that calls the compatible encoding `Shift_JIS` maps to the CP932 selection.

### Implementation scope

- Rust `RDC` device-comment decoding and its public helper/client APIs
- Cross-language API consistency with the Python, Node-RED, and .NET Host Link implementations
- Shared Host Link user documentation where the resulting behavior is common

### Target state

An `RDC` response is first treated as an exact byte payload. A caller that requests text explicitly selects the supported encoding used for that decode. The Rust implementation performs no heuristic UTF-8-first fallback, PLC-profile selection, write-source inference, or silent replacement of malformed bytes. A public raw-byte path exposes the undecoded comment payload.

The Rust public enum is `HostLinkCommentEncoding` with exactly `Utf8` and `Cp932`. `Cp932` uses the `encoding_rs` WHATWG `Shift_JIS` decoder, whose canonical name is `windows-31j`. It is the CP932/Windows-31J selection used for KEYENCE documentation that says `Shift_JIS`; there is no separate strict-Shift_JIS selection, alias, automatic value, or default.

The existing `read_named` and `poll` APIs remain available for non-comment values and reject every `:COMMENT` plan before transport. Explicitly named `read_named_with_comment_encoding` and `poll_with_comment_encoding` APIs accept comments only with a required `HostLinkCommentEncoding` value and reject an unused encoding when the list contains no `:COMMENT` entry.

### Compatibility impact

This is an intentional breaking change. Existing string APIs that silently try UTF-8 and then Shift_JIS must require an explicit encoding selection, while callers that cannot assert an encoding use the raw-byte API. Migration notes must identify the required selection and the removal of heuristic decoding.

### Acceptance criteria

1. Every public `RDC` text-decoding path requires an explicit supported encoding and has no automatic or profile-selected codec.
2. A public raw-byte path returns the undecoded `RDC` comment payload.
3. The exact codec mapping is UTF-8 plus one CP932/Windows-31J selection documented as KEYENCE `Shift_JIS` compatibility; a separate strict-Shift_JIS selection is not exposed.
4. Ambiguous byte sequences valid under multiple codecs decode only according to the caller's selection; malformed sequences fail without fallback or replacement.
5. Decoder failure and connection-state behavior are explicit and consistent with the library's protocol-error contract.
6. Rust user documentation, tests, generated API reference, changelog, and migration notes agree with the approved contract; other implementations retain independent acceptance evidence.

### Evidence and completion checklist

- [x] Evidence sufficient to reject a universal or profile-fixed `RDC` codec is recorded.
- [x] Shift_JIS versus Windows-31J/CP932 public mapping resolved for all four language runtimes.
- [x] Ambiguous and malformed byte vectors defined with evidence-backed expected results.
- [x] Further profile-by-profile live verification is not required to select the explicit-codec/raw-byte contract.
- [x] Target contract and compatibility impact explicitly approved by the user.
- [x] Implementation completed in this repository; other language repositories are tracked independently.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Additional live-PLC verification is recorded as not required for this deterministic decoder and raw-payload contract.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final Rust acceptance criteria verified and the item marked complete in this repository.
- [x] Independent Python, .NET, Node-RED, and Rust evidence confirms final family completion.

### Rust implementation and review evidence

- `HostLinkCommentEncoding` exposes exactly `Utf8` and `Cp932`; no default,
  automatic, profile-derived, compatibility alias, or separate strict
  Shift-JIS selection exists.
- Direct and helper text reads require the enum. `read_comment_bytes` preserves
  successful terminator-free payload bytes including trailing ASCII spaces,
  while exact `E0` through `E9` responses remain PLC errors and leave the
  correctly framed connection reusable.
- Ordinary `read_named` and `poll` reject `:COMMENT` during complete-plan
  validation with no request; explicit encoding variants retain intentional
  comment aggregates and reject non-comment-only lists with no request.
- Ambiguous UTF-8/CP932, codec-specific, malformed, padding, raw-payload,
  UTF-8-BOM selection, aggregate, PLC-error, connection-retirement, CP932
  control-byte, forbidden singleton, unassigned-pair, and valid-extension cases
  are executable tests.
- Codex self-review inspected the actual diff, public exports, validation order,
  error classification, connection retirement, deadline coverage, aggregate
  state, CLI, tests, rustdoc, package consumer, changelog, and user/maintainer
  documentation. Accepted and corrected findings: `5`; rejected: `0`;
  duplicate: `0`; deferred: `0`.
  - `HL-RDC-RS-F-001`: added explicit regression evidence that the raw comment
    API classifies a two-byte PLC error before returning payload bytes.
  - `HL-RDC-RS-F-002`: added the new public encoding enum and both variants to
    the isolated generated-crate consumer contract.
  - `HL-RDC-RS-F-003`: corrected the comment-response error branch so a
    syntactically valid PLC `E0` through `E9` reply does not retire the
    connection; raw and text tests prove that the next command succeeds on the
    same TCP connection.
  - `HL-RDC-RS-F-004`: `encoding_rs` WHATWG `SHIFT_JIS` accepts standalone
    `0x80` as `U+0080`, unlike the shared strict CP932 subset. Added structural
    validation that rejects standalone `80`/`A0`/`FD`/`FE`/`FF` without
    rejecting a valid two-byte trail `0x80`, plus exact control, malformed,
    unassigned, and extension vectors.
  - `HL-RDC-RS-F-005`: explicit aggregate APIs previously accepted an encoding
    that no entry used. They now require at least one `:COMMENT` entry and
    reject non-comment-only lists as `Protocol` before FIFO admission or send.

### Current evidence boundary

The pre-overhaul implementations tried UTF-8 first and fell back to Shift_JIS. The located KEYENCE material says that KV-8000 strings use Shift_JIS in a specific EtherNet/IP connection-guide context, but it does not define the Host Link `RDC` response encoding: <https://www.keyence.co.jp/support/user/controls/plc/connection_guide/kv_iv4/>.

On 2026-08-01, after the user's explicit `OK`, a read-only live check used KEYENCE KV-X500 / `keyence:kv-x500` at `192.168.250.100:8501`. `RDC R000` returned `E38182E38184E38186E38188E3818A` (UTF-8 `あいうえお`) and `RDC R001` returned `E3818BE3818DE3818FE38191E38193` (UTF-8 `かきくけこ`). Both payloads fail strict Shift_JIS and CP932 decoding. This proves that a universal Shift_JIS assumption is unsafe; it does not prove that all `RDC` comments are UTF-8 or identify how the comment-writing path determines stored bytes. The approved explicit-selection/raw-byte contract therefore does not depend on resolving that mechanism.

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
