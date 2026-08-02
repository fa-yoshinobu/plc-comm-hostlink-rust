# HostLink Rust quality-overhaul decision and acceptance record

This record maps the approved workspace decisions to the Rust implementation.
Breaking compatibility is intentional where it conflicts with an explicit,
single-request, profile-safe contract.

## 2026-08-01 target-state migration

- Float32 writes to direct-bit devices now fail before transport. Use a word
  device for `F`, or use `BIT`/bit operations for a direct-bit device.
- Direct-bit response decoding no longer trims or case-folds tokens. PLC
  responses must be exactly `0`, `1`, `OFF`, or `ON`; malformed responses
  close the connection generation.
- Timer/counter composite reads validate status, current, and preset fields.
- Host Link endpoints are IPv4-only. Replace IPv6 literals with the PLC IPv4
  endpoint or a hostname that has an IPv4 result.
- Empty named reads/polls and zero poll durations are caller errors.
- The former `QueuedHostLinkClient`, its inner-client escape hatch, and its
  clone-capable callback are removed. Construct or connect `HostLinkClient`;
  the ordinary client now owns FIFO admission and the complete logical turn.
- GitHub source archives now include tests and fixtures. This does not expand
  the crates.io package, whose explicit include list remains minimal.

## Acceptance matrix

| Decision | Implementation | Tests | Checks | Codex review | Claude review | Findings | Live/disposition | Docs | Final |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| D-052 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-053 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-054 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-055 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-056 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-057 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-058 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-059 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-060 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-061 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-062 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-063 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |
| D-064 | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] | [x] |

`Live/disposition` is checked because these Rust changes are constructor,
pre-transport validation, deterministic frame, loopback transport-state, and
documentation contracts. They do not create new PLC/profile support claims.

The user ran the authorized HostLink Claude review batch outside Codex on
2026-07-12. Its Rust findings and Codex disposition are recorded below; Codex
did not invoke Claude.

## Verification evidence

- `run_ci.bat`: format, Clippy with `-D warnings`, and all-target/all-feature tests passed.
- Tests: 37 library tests and 45 high-level integration tests passed; all executable example targets built, with zero skip/failure. Library-local cross-implementation vectors were removed by workspace policy.
- `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --all-features` passed.
- `cargo package --allow-dirty` builds and verifies the packaged crate; cross-implementation vectors are not library package content.
- Multi-PLC and JSON-config examples passed no-network dry-run validation with explicit port, transport, and profile values.
- `git diff --check` passed and stale LF/chunk/error-message/default-constructor references were absent.
- Codex reviewed the actual diff, public exports, validation order, raw/semantic separation, TCP/UDP state, cancellation, response caps/counts, Dword frames, compound bit updates, examples, docs, and package contents.

## D-052 — Transport is required

- Scope: connection options, executable examples, verification binary.
- Target: callers choose TCP or UDP; no missing-value TCP fallback exists.
- Compatibility: old constructors and endpoint configs must add transport.
- Acceptance: compile-time required enum and executable missing-value rejection.

## D-053 — Timeout defaults to 3 seconds

- Scope: connect, send, receive, and runtime timeout setter.
- Target: omission is 3 seconds; zero is rejected and explicit durations remain unchanged.
- Compatibility: zero or implicit unlimited waits are invalid.
- Acceptance: constructor and transport-state tests verify the timeout contract.

## D-054 — Normal command frames are CR-only

- Scope: frame builder and connection surface.
- Target: one trailing `0x0D`; no LF append field, setter, or ignored alias.
- Compatibility: CRLF customization is removed.
- Acceptance: deterministic repository-local command and frame-builder tests compare exact request bodies.

## D-055 — Receive buffers are internal

- Scope: TCP/UDP receive paths and semantic response validation.
- Target: 65,536-byte body cap, terminated full UDP datagram handling, command/registration-derived token counts, and transport invalidation on overflow/mismatch.
- Compatibility: no caller-controlled receive size or cap bypass.
- Acceptance: maximum, one-byte-over, EOF, timeout, UDP, and count-mismatch tests.

## D-056 — Trace is maintainer-only and optional

- Scope: internal send/receive observation.
- Target: no hook or logging by default; an internal hook observes one send and one receive without changing exchange behavior.
- Compatibility: trace types/settings are absent from crate-root user exports.
- Acceptance: public symbol inspection and transport tests show no default output path.

## D-057 — Construction performs no communication

- Scope: `HostLinkClient::new` and factories.
- Target: `new` validates/localizes state only; explicitly named connect factories may open.
- Compatibility: callers relying on construction-time I/O must call `open` or a connect factory.
- Acceptance: disconnected-command tests use an unreachable endpoint without network access.

## D-058 — Commands never connect implicitly

- Scope: raw, semantic, queued, read, write, clock, mode, and comment paths.
- Target: disconnected commands return typed `NotConnected`; TCP failure closes
  its connection and requires explicit reopen. The 2026-08-02 PERF-002 override
  makes UDP failure discard only the affected socket; a later command replaces
  it from the resolved logical endpoint without retrying the failed command.
- Compatibility: callers must own initial connection and recovery.
- Acceptance: disconnected/open/failure/explicit-reopen tests for TCP and
  logical-open/failure/socket-replacement tests for UDP.

## D-059 — PLC time is required and validated

- Scope: `set_time` and `HostLinkClock`.
- Target: explicit clock only; real date/time and weekday agreement; no timezone fallback.
- Compatibility: `Option<HostLinkClock>` and no-argument calls are removed.
- Acceptance: fixed frame, nonexistent date, weekday mismatch, and explicit `now_local` behavior.

## D-060 — Raw exchange returns body bytes

- Scope: maintainer raw direct and queued methods.
- Target: terminator-free bytes with no encoding, PLC error, or comment interpretation.
- Compatibility: string return and public decoder selection are removed.
- Acceptance: ASCII, PLC error, non-ASCII, and CR/LF-boundary fixtures preserve bytes.

## D-061 — Comment padding policy is fixed

- Scope: direct, queued, named, and helper comment reads.
- Target: remove only trailing ASCII `0x20`, then decode UTF-8 or Shift_JIS.
- Compatibility: caller-selected padding preservation is removed.
- Acceptance: spaces, tabs, full-width space, Shift_JIS, and all-padding fixtures.

The padding portion remains current. Its implicit decoder selection was
superseded by the user-approved `HL-EVAL-TODO-006` explicit UTF-8/CP932 contract;
the historical UTF-8-or-Shift_JIS target is not a callable compatibility mode.

## D-062 — Expansion-buffer format is required

- Scope: URD/UWR direct methods and verification tool.
- Target: explicit U/S/D/L/H, strict value/token validation, count and word-span validation.
- Compatibility: `None`/empty-to-U fallback is removed.
- Acceptance: deterministic command tests, invalid formats, boundaries, and 32-bit end-crossing tests.

## D-063 — Chunked APIs are removed

- Scope: crate-root helpers and documentation.
- Target: every word/Dword helper call sends at most one command; overflow fails before transport.
- Compatibility: applications must implement and own any intentional multi-request loop.
- Acceptance: no chunk exports/references and limit/limit-plus-one command-count tests.

## D-064 — Numeric device and format are separate

- Scope: RD/RDS/WR/WRS/RDE/WRE/WS/WSS/MWS and high-level address parsing.
- Target: low-level numeric calls use a base device plus explicit format; suffix input is rejected. High-level `.0`-`.F` remains bit-in-word and colon remains dtype.
- Compatibility: suffix-only and conflicting dual-format calls are rejected.
- Acceptance: suffix, missing/empty format, direct bit, `DM100.D`, `DM100:D`, numeric range, and response-token tests.

## HL-EVAL-TODO-006 — Explicit RDC encoding and raw bytes

- Scope: direct/helper `RDC` reads, named reads, polling, verification CLI,
  public exports, tests, package consumer, and user/maintainer documentation.
- Target: successful `RDC` payloads are bytes first. Text APIs require exactly
  `HostLinkCommentEncoding::Utf8` or `Cp932`; `Cp932` is CP932/Windows-31J for
  KEYENCE `Shift_JIS` compatibility. No automatic, profile-selected, fallback,
  replacement, default, alias, or separate strict-Shift-JIS mode exists.
- Compatibility: the former UTF-8-first/Shift_JIS-fallback call shape is
  removed. Ordinary `read_named`/`poll` reject comments before transport;
  explicit encoding variants require at least one comment and reject an unused
  encoding before transport. Applications that cannot assert an encoding use
  `read_comment_bytes`.

Acceptance criteria:

1. Every public text path requires one explicit enum value, and public rustdoc
   and package-consumer compilation prove the new surface is exported.
2. Raw reads preserve successful terminator-free bytes and padding, but still
   classify exact Host Link PLC error replies before returning a payload.
3. Ambiguous bytes follow only the selected codec; malformed bytes produce a
   protocol error without fallback or replacement and retire the connection.
   UTF-8 `EF BB BF 41` remains `U+FEFF` plus `A`, while CP932 rejects it.
4. Ordinary aggregate APIs reject all comment plans with zero sends, while
   their explicitly named encoding variants require at least one comment, read
   comments in declared order, and reject an unused encoding with zero sends.
5. CLI, API reference, usage guidance, gotchas, migration notes, changelog, and
   executable tests describe one consistent contract.

Evidence: 44 library tests and 69 integration tests passed with formatting,
Clippy, warning-denied rustdoc, Rust 1.85 compilation, the 28-file generated
crate/isolated consumer gate, and the 47-file current-worktree source-archive
gate. No additional live PLC test is required for this deterministic decoding,
raw-payload, and preflight-validation contract. Codex self-review disposition:
accepted and corrected `5` (`HL-RDC-RS-F-001` PLC-error regression coverage,
`HL-RDC-RS-F-002` isolated consumer enum coverage, and `HL-RDC-RS-F-003`
connection retention after a correctly framed PLC NG response, and
`HL-RDC-RS-F-004` strict cross-runtime CP932 boundary validation); rejected
`0`; duplicate `0`; deferred `0`. `HL-RDC-RS-F-005` additionally rejects an
unused aggregate comment encoding before transport.

## RS-HL-CLAUDE-20260712 — Independent-review corrections

Scope: Claude HostLink findings 8, 9, 10, 11, 12, 13, 20, and 21 for the
Rust repository.

Target contract: the clock year fits the two-digit field; every non-format
command rejects suffix-bearing devices; UDP requires a terminator and discards
malformed transport; `H` reads have exact token shape and digits; conversion
from `HostLinkValue` is never silently lossy; timeout behavior has executable
coverage. Cross-language vectors live only in the separate cross-verification
repository.

Compatibility impact: year values 100..255, suffixes previously erased by
ST/RS/STS/RSS/MBS/RDC/timer helpers, malformed hexadecimal replies, and lossy
`u16::from` calls are rejected or no longer compile. Callers use
`u16::try_from` and handle the result.

Acceptance criteria:

1. Clock year 99 passes and 100 fails before send.
2. ST/RS/STS/RSS/MBS/RDC and timer/counter helpers reject any suffix before
   transport.
3. Unterminated UDP responses fail and discard that socket; the logical endpoint
   remains available for a replacement. Normal terminated datagrams continue to pass.
4. Non-composite `H` reads require exactly one 1..4-digit hex token, composite
   reads require exactly three tokens, and non-`U16` conversion fails.
5. Tests prove the 3-second timeout default, zero rejection, TCP timeout state,
   and absence of library-local cross-vector files/runners.
6. RD uses the command-derived token count. Direct-bit numeric formats accept
   exactly one packed scalar token whose `.U`/`.S`/`.H` view spans 16 bits and
   whose `.D`/`.L` view spans 32 bits; direct BIT accepts only
   `0`/`1`/`ON`/`OFF`, and malformed semantic responses close the transport
   generation. This supersedes the former 16/32-token assumption using the
   KV-X500 live response vectors recorded by `LIVE-HL-001`.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Full format, Clippy `-D warnings`, 153 tests, rustdoc `-D warnings`, example, and package checks passed.
- [x] Codex self-review completed against public API, validation order, response shape, transport state, timeout/cancellation, docs, and package contents.
- [x] Claude source review completed; the user ran the authorized batch and its result is preserved in the workspace.
- [x] Codex dispositioned all Rust findings and reran affected checks.
- [x] No additional live-PLC check is required for these validation and local transport corrections.
- [x] Documentation, migration notes, and changelog agree with the implementation.
- [x] Final acceptance criteria verified for this repository; HostLink family-level acceptance remains separate.

## 2026-07-12 KV-X500 live smoke evidence

- [x] The public `HostLinkClient` connected to `keyence:kv-x500` at `192.168.250.100:8501` over TCP and read `DM0:U` once; the result was `5878`.
- [x] No write, retry, or profile／transport fallback was performed.
- [x] This evidence is limited to that endpoint, profile, device, transport, and operation; it does not verify other device families or the complete profile.

## NR-007: Lifetime traffic statistics

Approved next-release contract: `traffic_stats().await` returns immutable lifetime counters; only
complete sends and complete response lines/datagrams count, pre-send and partial failures do not,
and close/reconnect does not reset. Deterministic tests are required; live PLC verification is
unnecessary. Final packaging and publication acceptance completed with `v3.2.0`.

- [x] Public API and transport-boundary implementation completed.
- [x] Deterministic tests, documentation, changelog, and package gate completed.
- [x] Codex final self-review completed.
- [x] Next-release package acceptance completed. Evidence: the `v3.2.0` tag equals repository HEAD,
  the GitHub Release and crates.io `plc-comm-kv-hostlink` `3.2.0` crate are public, tag-commit checks
  passed, and the final six-runtime family source/API comparison was completed on 2026-07-18.

## RS-XOVER-001 — Ordinary client FIFO and connection generations

Implementation scope: `HostLinkClient`, its clones, constructors/factories,
logical helper boundaries, close, and reopen.

Target contract: the ordinary client owns FIFO admission and one wire turn.
There is no queued wrapper or bypass alias. Admission snapshots caller inputs
and timeout. Dropping a waiting future sends nothing and returns no library
`Result`. `close` rejects active and waiting work from the old generation; only
newly admitted work can use a subsequent explicit reopen.

Compatibility impact: `QueuedHostLinkClient` and its members are removed.
Callers use `HostLinkClient` directly.

Acceptance criteria:

1. Crate exports, examples, CLI, docs, and tests contain no queued type or alias.
2. Concurrent ordinary-client operations use FIFO wire order; `read_named` and
   one poll cycle retain one turn across every planned request and decode.
3. Dropping a waiting future produces no request or library `Result`; close
   rejects active/waiting old-generation operations and those operations cannot
   send after reopen.
4. Timeout is captured before waiting and used for the admitted operation.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required: FIFO/generation behavior is deterministic client-side state and loopback transport behavior.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## RS-XOVER-002 — Dedicated errors and uncertain state-changing outcomes

Implementation scope: public error API, read/write/raw classification, malformed
acknowledgements, transport retirement, and retry behavior.

Target contract: callers can distinguish `Protocol`, `Timeout`, `Closed`,
`NotConnected`, `Transport`, `Plc`, and `OutcomeUnknown`. An operation that can
change PLC state and may have been sent reports the uncertain reason as timeout,
close, transport, or malformed response when its future returns a library
`Result`. Raw commands are treated conservatively as state-changing. The
library never retries automatically.

Rust cancellation is caller-observed future drop, not a returned error path.
Dropping a waiting future sends nothing. Dropping an active future after a send
may have started returns no `HostLinkError`, poisons the exchange, and retires
its socket. TCP causes the next operation to return `NotConnected` until
explicit reopen. The PERF-002 UDP override retains the resolved logical
endpoint and creates a replacement socket for the next operation without
retrying the abandoned request.
The caller must treat a possibly transmitted state-changing operation as
unknown. `HostLinkOutcomeUnknownReason` therefore has no `Cancellation` variant,
and caller-observed drop is distinct from the library's `Timeout` result.

Compatibility impact: the former generic connection error is replaced by
dedicated variants and exhaustive matches must be updated.

Acceptance criteria:

1. Pre-send validation and disconnected use never report uncertain outcome.
2. Post-send timeout/close/transport/malformed failures of writes or raw calls
   report `OutcomeUnknown` with the matching machine-readable reason.
3. PLC-declared rejection remains `Plc`; reads retain their non-write error class.
4. Every failed exchange retires transport and sends at most once.
5. Pairwise error classification covers only states for which the operation
   future returns a library `Result`; future drop is separately observable by
   the caller and cannot be represented as a returned cancellation variant.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required: classification is covered by deterministic fault injection and loopback transport.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## RS-XOVER-003 — One monotonic transaction deadline

Implementation scope: endpoint resolution, connect, TCP/UDP write, receive,
terminator assembly, decoding, timeout snapshot, and transport state.

Target contract: one checked monotonic absolute deadline covers each admitted
connect/exchange through complete framing and decoding. Progress never restarts
the deadline. Timeout retires the transport and does not retry.

Compatibility impact: trickled or repeatedly partial responses can now time out
even while progress continues; timeouts use the dedicated error category.

Acceptance criteria:

1. Connect resolution/socket work shares one absolute deadline.
2. Write and every receive fragment use the same deadline through terminator and decode.
3. Trickled TCP and delayed UDP cross the deadline once; the affected TCP
   connection or UDP socket is not reused. UDP may create a replacement socket
   for a later command without retrying the timed-out request.
4. Durations that cannot form a deadline fail before transport without panic.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required: timing/state behavior is deterministic with loopback TCP/UDP.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

### HL-KVX500-02B — Controlled UDP timeout and socket replacement evidence

The approved read-only Rust anomaly batch passed against `keyence:kv-x500` at
`192.168.250.100:8501`. The controlled setup used one cable unplug and replug;
it performed no PLC writes. Phase A read `DM120.U` as `00000` on the retained
healthy UDP socket. Phase B made exactly one request while communication was
interrupted and returned `HostLinkError::Timeout` after 2001 ms: the request
count increased by one, received bytes increased by zero, no retry occurred,
and the affected socket was retired. Phase C used exactly one replacement
socket from the retained numeric endpoint, performed no DNS lookup, read
`DM120.U` as `00000`, and closed cleanly with no active UDP endpoint remaining.
The complete batch used 3 requests, 33 transmitted bytes, and 14 received
bytes.

Evidence:
`D:\APP\live-kvx500-20260802\hl-kvx500-02b-runs\f610eebe-7842-4f61-9894-835912f5575c\Rust\result.json`,
SHA-256
`df11dd1f79a64f4f419272258182346b89397a2f94ed620d6f4f517b3fe824ef`.

- [x] Rust `HL-KVX500-02B` live row passed: healthy reuse, one timeout with no
  retry, failed-socket retirement, one DNS-free replacement, successful read,
  and close cleanup all matched the approved UDP recovery contract.

## RS-XOVER-004 — Read-only aggregate planning and no bit RMW helper

Implementation scope: `read_named`, poll, plan compilation, value resolution,
single-request helpers, and bit-in-word public surface.

Target contract: all aggregate inputs are snapshotted and validated before FIFO
admission or the first send. The 2026-08-02 PERF-001 decision supersedes the
earlier declared-wire-order rule: wire-compatible device types are grouped by
first appearance, sorted by address, and merged to the minimum request count.
Multiword values never split, the aggregate and each poll cycle own one FIFO
turn through final decode/staging, and pure result materialization and poll
intervals occur outside that turn. Any failure returns no partial result.
Multiple read frames are explicitly non-atomic. Public results use
`NamedReadResult`, not snapshot terminology; no `NamedSnapshot` compatibility
alias remains. No client-side bit-in-word read-modify-write API remains.

Compatibility impact: wire request order is optimized rather than preserving
input order, while public result order remains input order;
`write_bit_in_word` is removed without an alias. `NamedSnapshot` is renamed to
`NamedReadResult` without an alias, and the verification CLI poll response key
changes from `snapshots` to `results`.

Acceptance criteria:

1. An invalid later address causes zero sends.
2. Descending/mixed input produces the minimum grouped/sorted frames and complete input-ordered values.
3. Segment-limit rollover moves a complete Dword/Float32 value to the next frame.
4. An aggregate or poll cycle is all-or-error, holds one turn through staging,
   and releases it before pure result materialization or interval waiting; docs
   state scan-time non-atomicity.
5. Public source/API/docs contain no bit-in-word write helper.
6. Public source/API/docs/examples/CLI use `NamedReadResult` and result
   terminology; documentation directs coherent reads to one request or a
   PLC-side snapshot/handshake.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required: planning, ordering, and API removal are deterministic and do not add PLC support claims.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## RS-XOVER-005 — Capacity, Boolean input, and IPv4 boundaries

Implementation scope: request construction, TCP/UDP response storage, semantic
caller-owned results, point limits, direct-bit writes, and endpoint selection.

Historical target contract: request and response bodies accepted exactly
65,536 bytes and rejected one byte over before state corruption; caller results
were dynamically owned and had no public receive-capacity setting. Command
point limits rejected limit-plus-one without splitting. Direct-bit writes
required `bool`. Endpoints remained IPv4-only. RS-REAUDIT-008 supersedes only
the request boundary: the current raw request body maximum is 65,506 bytes and
the CR-terminated frame maximum is 65,507 bytes. The 65,536-byte response-body
boundary is unchanged.

Compatibility impact: oversized raw requests and numeric/text Boolean aliases
are rejected; no IPv6 support is added.

Acceptance criteria:

1. Request/response exact maximum and maximum-plus-one are tested; rejection
   preserves pre-operation state and traffic counts.
2. Word/Dword command limits accept the maximum and reject one over with zero send.
3. Direct-bit numeric/text inputs fail before transport and Boolean inputs retain exact frames.
4. IPv6 literals/IPv6-only resolution fail before protocol send; IPv4 TCP/UDP remain supported.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required: these are pre-transport, buffer, and loopback boundary contracts.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## RS-XOVER-006 — Rust 1.85 and distributable-source gates

Implementation scope: Cargo metadata, CI, user/maintainer documentation,
examples, registry package contents, and GitHub source-archive checks.

Target contract: Rust 1.85 is the declared and tested MSRV. Format, check,
Clippy, tests, rustdoc, examples, package content, generated-crate consumer, and
current-worktree source archive all pass from their intended inputs. The crate
gate extracts the generated `.crate`, proves separate repository tests are
absent, checks README/license/rustdoc source/examples, builds all packaged
targets, builds rustdoc, and checks an independent path consumer using only the
extracted package. Registry publication remains manual and is not part of this
implementation.

Compatibility impact: consumers require Rust 1.85 or later; source archives
include repository tests while the crate package remains minimal.

Acceptance criteria:

1. `cargo +1.85.0 check --all-targets --all-features` passes.
2. Format, Clippy with warnings denied, all-feature tests, and rustdoc pass.
3. Every executable example builds and deterministic dry-run examples pass.
4. The generated-crate content/consumer gate and current-worktree synthetic-
   source-archive gate pass independently.
5. Changelog, user docs, generated API reference, and this record agree.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required: this item concerns compiler, documentation, packaging, and deterministic archive evidence.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

### RS-XOVER verification evidence and self-review disposition

- Rust 1.85: `cargo +1.85.0 check --all-targets --all-features` passed.
- Local gate: format, Clippy with warnings denied, and all-target/all-feature
  tests passed. The suite contained 41 library tests and 67 integration tests;
  every executable example target also built.
- Rustdoc with warnings denied passed. The multi-PLC and JSON-config examples
  passed their no-network dry-run validation.
- The generated `.crate` contained 28 files and 7 package-manager examples,
  with no separate repository test tree. Its library, binary, examples, and
  rustdoc built from the extracted artifact, and an independent path consumer
  compiled using only that extracted package.
- The current-worktree synthetic source archive contained 47 files, 10 sample
  files, and 2 test files; format, check, Clippy, rustdoc, and all-target tests
  passed from the fresh extracted archive.
- Accepted finding `RS-XOVER-F-001`: package verification initially found that
  Tokio's `select!` macro was supplied only by dev-dependency feature unification.
  The normal dependency now enables `macros`; package verification and all
  affected gates passed afterward.
- Accepted finding `RS-XOVER-F-002`: `NamedSnapshot` falsely implied that the
  potentially multi-request aggregate was PLC-atomic. It was renamed to
  `NamedReadResult` across the public API, CLI, tests, examples, and docs with
  no compatibility alias.
- Accepted finding `RS-XOVER-F-003`: the public
  `HostLinkOutcomeUnknownReason::Cancellation` variant had no reachable return
  path. It was removed, and future-drop behavior is now documented and tested
  separately from every error state that returns a library `Result`.
- Accepted finding `RS-XOVER-F-004`: package validation inspected only
  `cargo package --list`. The gate now extracts the generated `.crate` and
  checks package contents, examples, rustdoc, and an isolated consumer.
- Accepted finding `RS-XOVER-F-005`: the first isolated gate used the
  unavailable Windows PowerShell 5 `Path.GetRelativePath` API. Relative paths
  now use a prefix-validated substring and the gate passes under Windows
  PowerShell 5.
- Accepted finding `RS-XOVER-F-006`: the first isolated gate requested Cargo
  test targets, which correctly exposed references to deliberately excluded
  repository fixtures. Consumer validation now compiles the packaged library,
  binary, and examples without enabling `cfg(test)`, while separately proving
  that the repository test tree is absent.
- Accepted finding `RS-XOVER-F-007`: the renamed public result and uncertain-
  outcome reason types needed source-level rustdoc, not only hand-written API
  pages. Their rustdoc now states the non-atomic aggregate and future-drop
  boundaries directly on the exported types.
- Accepted finding `RS-XOVER-F-008`: current-worktree source validation used
  `git stash create`, which captured tracked modifications and deletions but not
  non-ignored untracked files. The gate now writes a synthetic tree through a
  temporary Git index using `git add -A`; modifications, untracked files, and
  deletions are all represented without changing the maintainer's real index.
- Current correction-pass finding disposition: accepted and corrected `7`;
  rejected `0`; duplicate `0`; deferred `0`. `RS-XOVER-F-001` is prior evidence
  and is not included in that count. `HL-EVAL-TODO-006` remains the separately
  approved deferred device-comment encoding investigation and its decoder was
  not changed.

## QREV-20260714-004: Segmentation-independent TCP receive accounting

Scope: direct and queued TCP receive framing and `HostLinkTrafficStats.rx_bytes`.

Family equivalence: all four HostLink implementations count TCP `OK\r`, `OK\n`, coalesced `OK\r\n`, and either split CR/LF ordering as 3 bytes; UDP `OK\r\n` remains 4 bytes. Incomplete oversize/EOF/timeout/future-drop data contributes zero, while a complete PLC error line is counted before semantic decoding. The family comparison is preserved in the archived workspace record `communication_library_quality_review_20260714.md`.

Target contract: one completed TCP response counts its body through the first CR or LF. Additional
CR/LF separator bytes are consumed without changing the counter, whether they arrive together or
in a later TCP read. UDP continues to count the complete accepted response datagram.

Compatibility impact: a coalesced CRLF response previously could count both terminators and now
counts only the first; split CRLF already counted one. The corrected value is independent of TCP chunking.

Acceptance criteria:

1. Equivalent CRLF responses produce the same `rx_bytes` when CR and LF are coalesced or split.
2. The separator left after a completed line cannot become an empty or misassociated next response.
3. Complete PLC errors are counted; incomplete oversize, EOF, and timeout paths are not counted. Complete UDP datagram accounting is unchanged.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Profile drift, format, Clippy, 85 tests, rustdoc, examples, and package checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Claude source review completed; findings are preserved in the archived workspace record `claude_review_findings_20260714.md`.
- [x] Codex resolved or dispositioned every applicable Claude finding and reran affected checks.
- [x] Live PLC verification is not required for this deterministic local framing and counter contract.
- [x] Documentation, migration notes, and changelog agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## GOAL-CROSS-OS-CI-001 — Required Windows representative contract smoke

Implementation scope: the repository CI workflow and existing deterministic
Tokio loopback tests. Runtime code, public API, packaging, release workflows,
the Ubuntu MSRV job, and the Ubuntu full Rust gate are unchanged.

Target contract: the primary Ubuntu gates remain authoritative. One additional
non-optional Windows stable-Rust job runs only representative localhost
contracts for fragmented receive accounting, one deadline across a trickled
response, refused TCP connection classification, close retirement of
active/queued work followed by reopen, and UDP late-response retirement before
socket replacement. The job has a ten-minute bound and does not run the complete test,
feature, documentation, or package matrices.

Compatibility impact: none; this adds CI evidence only.

Machine-verifiable acceptance criteria:

1. `.github/workflows/ci.yml` contains exactly one `windows-latest` contract-
   smoke job in addition to the unchanged Ubuntu MSRV and full gates.
2. The Windows job is required by workflow semantics: it has no conditional,
   failure suppression, or `continue-on-error` path.
3. Its five explicit test filters cover fragmented receive, connection failure,
   bounded timeout, close/waiter retirement, reopen, and delayed-response rejection/replacement.
4. The Windows job installs only the stable minimal toolchain, runs the bounded
   integration-test subset, and does not package, publish, or contact a PLC.

- [x] Implementation completed in this repository.
- [x] Existing deterministic tests explicitly selected for every acceptance criterion.
- [x] The new Windows CI job passed on GitHub for the final source state.
- [x] The equivalent local Windows contract and complete non-hardware gates passed with Rust 1.85.0 and stable 1.95.0.
- [x] Codex self-review completed after the requested local verification run.
- [x] Live PLC checks are not required; all selected behavior uses localhost loopback.
- [x] Maintainer CI documentation agrees with the workflow; no user migration note or changelog entry is required.
- [x] Final acceptance criteria verified and the item marked complete.

Verification evidence: the local Windows run passed
`cargo +1.85.0 check --all-targets --all-features`, stable format, Clippy with
warnings denied, rustdoc with warnings denied, and all-target/all-feature
tests. The `high_level` integration target passed 70 tests. The generated
crate contained 28 files and 7 examples and passed extracted-artifact and
independent-consumer checks. The synthetic current-worktree source archive
contained 47 files, 10 sample files, and 2 test files and passed its extracted
full gate. This includes every test selected by the new Windows representative
job. The required GitHub-hosted Windows job and Ubuntu MSRV/full jobs passed on
final merged source commit `c0e3b63f4140acc8a1bd944877ecc41cbec6683d`
in [CI run 30705296574](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/actions/runs/30705296574).
This follow-up changes only the maintainer evidence record. No live PLC
communication was performed.

Self-review disposition:

- Accepted and corrected: the initial subset had no connection-refusal case. A
  bounded loopback refusal test was added and selected without changing the
  Ubuntu MSRV or full gate.
- Rejected: duplicating the four integration cases into a Windows-only test
  target would add maintenance without a distinct contract. Exact filters keep
  the existing tests authoritative.
- Duplicate findings: none. Deferred findings: none.

## GOAL-HOSTLINK-RUST-FALLIBLE-FORMATTER-001 — Make payload formatting explicitly fallible

Stable identifier: `HOSTLINK-RUST-FALLIBLE-FORMATTER-001`.

Decision status: implemented and verified on 2026-08-02.

Implementation scope: the exported `HostLinkPayloadValue` trait, every built-in
and blanket implementation, `HostLinkValue`, the default `append_to_payload`
method, typed-write and payload-construction helpers, normal write APIs, tests,
rustdoc, user documentation, migration notes, changelog, and generated API
reference.

Target contract: `HostLinkPayloadValue::format_for_suffix` returns
`Result<String, HostLinkError>`. Formatting an out-of-range value, a value whose
type is incompatible with the requested format, or an unsupported suffix returns
`Err`; it must never return an empty string or another fallback token to hide the
failure. The default `append_to_payload` calls the fallible formatter, propagates
its error, and appends only a successfully formatted complete token. Every
internal formatter call and helper propagates the error without parsing or
transmitting a fallback value.

Built-in boundaries retain their existing intended semantics: integer formats
use the current direct-bit, unsigned/signed 16-bit, hexadecimal 16-bit, unsigned
32-bit, and signed 32-bit ranges; `bool` is valid only for direct-bit formatting;
floating-point and text values remain unavailable to low-level numeric payload
append paths and are accepted by the explicitly typed helpers only where those
helpers currently support them. Unknown suffixes fail for every built-in type.
Valid built-in inputs retain their current wire tokens and normal write behavior.

Compatibility impact: this is a source-breaking public-trait change. External
implementations must change `format_for_suffix(&self, suffix) -> String` to
`format_for_suffix(&self, suffix) -> Result<String, HostLinkError>`, return
`Ok(token)` only after validating their supported suffix and value domain, and
return `Err` for unsupported or invalid input. Direct callers must handle or
propagate the `Result`. Custom implementations that rely on the default
`append_to_payload` gain mandatory error propagation; implementations that
override it must preserve the same no-fallback contract. No compatibility trait,
infallible alias, empty-string fallback, or deprecated callable surface remains.

Machine-verifiable acceptance criteria:

1. The exported trait signature and rustdoc expose
   `Result<String, HostLinkError>`, and a compile-time migration fixture proves
   that an implementation using the former `String` signature no longer builds.
2. Every built-in integer type accepts exact minimum/maximum values for each
   supported suffix and rejects the immediately out-of-range values without
   producing a token or transport request.
3. `bool`, `f32`, `f64`, `String`, `&str`, references, and `HostLinkValue`
   return `Err` for type-incompatible or unsupported suffixes and preserve their
   documented valid typed-helper behavior.
4. The default `append_to_payload`, every overriding implementation, reference
   forwarding, joined-payload builders, and typed-write helpers propagate the
   original `HostLinkError`; no ignored `Result`, empty token, partial token, or
   post-error payload mutation remains.
5. Representative direct, consecutive, set-value, expansion-buffer, and typed
   writes retain byte-identical commands for valid built-in boundary values.
6. Equivalent invalid calls through those normal write APIs fail before request
   counters or transport activity and identify the value, type, or suffix error.
7. A custom-trait implementation fixture demonstrates both a successful token
   and an explicit unsupported-suffix error through direct formatting, default
   append, and a normal write caller.
8. Public API documentation, examples, migration notes, and generated API
   reference show `Result` handling and contain no infallible formatter example.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Required live-PLC checks passed, or each unavailable check has an explicit release disposition.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

### Verification evidence and self-review disposition (2026-08-02)

- `run_ci.bat`: PASS. Formatting, clippy for all targets and features with
  `-D warnings`, rustdoc with `-D warnings`, all-target/all-feature Cargo tests,
  generated-crate validation, all seven examples, and an isolated generated-
  crate consumer completed successfully. The rustdoc compile-fail fixture
  proves the former infallible trait signature no longer compiles.
- Boundary tests cover every built-in integer type, exact supported limits and
  immediately invalid values, incompatible built-in and `HostLinkValue` types,
  reference forwarding, custom success/error formatters, joined payloads,
  unchanged output on failure, and empty-success-token rejection. Normal direct,
  consecutive, set-value, expansion-buffer, and typed write paths prove invalid
  formatting fails with zero transport requests.
- Codex self-review inspected the actual diff, exported trait and blanket
  implementations, validation order, token construction and mutation boundary,
  normal and typed helper propagation, tests, examples, rustdoc, user docs,
  generated API, and package output. Accepted findings: the initial default
  append path accepted custom `Ok("")`, so explicit empty-token rejection and
  no-mutation/no-send tests were added; and typed F/H behavior required separate
  internal float/text projections after low-level formatting became strictly
  fallible, so `as_float`/`as_text` projections and regression coverage were
  added. Rejected findings: none. Duplicate findings: none. Deferred findings:
  none.
- Live PLC verification is not required for this item: formatter return values,
  payload mutation, and pre-send request counters are deterministic local
  behavior, and no PLC/profile compatibility claim changed. No live PLC
  communication was performed.

## RS-REAUDIT-001 — Persistent TCP response ownership

Implementation scope: serialized TCP exchange ownership, anomaly retirement,
connection-scoped monitor registration, direct regression tests, and user
transport guidance.

Target contract: one healthy persistent TCP connection carries serialized
requests with at most one unfinished request. Observable unowned, surplus,
timed-out, cancelled, malformed, or transport-failed exchanges retire that
connection. Host Link has no request identifier, so input arriving between the
final pre-send check and the send cannot be associated perfectly. A connection
per request is not adopted because it adds a TCP handshake to every normal
command without creating a protocol request identifier.

Machine-verifiable acceptance criteria:

1. Healthy requests reuse one TCP connection and execute serially.
2. Surplus and delayed pre-send unowned responses prevent a later send and
   retire the connection.
3. State-changing failures after a possible send remain outcome-unknown and
   are never retried automatically.
4. Monitor registration and read share one connection; close/reopen clears the
   registration and a later monitor read sends nothing until re-registration.
5. User documentation states the remaining request-ID limitation and the
   normal-latency decision.

- [x] Implementation completed in this repository.
- [x] Direct response-ownership and connection-scoped monitor tests completed.
- [x] The post-correction repository gate passed formatting, Clippy and rustdoc
  with warnings denied, 153 tests, all examples, crate packaging, and the
  isolated generated-crate consumer.
- [x] Codex self-review identified and corrected the missing reconnect monitor
  regression and user-facing residual-risk explanation.
- [x] Live PLC verification is not required because ownership, serialization,
  pre-send rejection, and registration reset are deterministic local transport
  and state-machine behavior.
- [x] User guidance and changelog agree with the implementation.
- [x] Final acceptance criteria reverified after the accepted corrections.

## LIVE-HL-003 — Timer/counter structural status formatting

Implementation scope: low-level formatted `RD` responses for the `T` and `C`
families, their high-level timer/counter projections, malformed-response
retirement, deterministic regressions, user documentation, and migration
records in this repository.

Target contract: a timer/counter response has exactly three semantic fields.
The first field is structural status and is validated as the exact raw PLC token
`0` or `1`; it is never parsed or normalized with `.U`, `.S`, `.H`, `.D`, or
`.L`. The selected numeric format applies only to current and preset. Preserve
the existing low-level `Vec<String>` container and all high-level public return
types.

Compatibility impact: public signatures and high-level results do not change.
The erroneous low-level `.H` first value changes from synthesized `0000` or
`0001` to the PLC-semantic `0` or `1`. Callers comparing the first token must
use `0`/`1`; reliance on the synthesized hexadecimal status is not retained.

Machine-verifiable acceptance criteria:

1. Low-level `T`/`C` status accepts only exact `0` or `1` before numeric-format
   validation.
2. Current and preset alone use the requested `.U`, `.S`, `.H`, `.D`, or `.L`
   parser and range.
3. The real `RD T0.H` vector `0,270F,270F` returns
   `["0", "270F", "270F"]`; high-level typed projection remains unchanged.
4. Formatted or otherwise malformed status, missing or extra fields, invalid
   current/preset values, and overflow are protocol errors that retire the
   supplying transport.
5. User guidance, API reference, changelog, and this migration record state the
   low-level compatibility change and required comparison migration.

Evidence:

- The configuration-corrected KV-X500 live evidence returned
  `RD T0.H -> 0,270F,270F`; this is the authoritative PLC response vector that
  exposed the former status normalization defect.
- The separately approved read-only Rust `HL-KVX500-01` runner passed against
  `keyence:kv-x500` at `192.168.250.100:8501` with `writes: false`. It started
  at `2026-08-02T11:12:01.4085217Z` and finished at
  `2026-08-02T11:12:01.4556450Z`. The evidence repository state was HEAD
  `ed008ff9c18c7ba275f852274f861f8ad08635e9` with pre-evidence-record
  `git diff --binary` SHA-256
  `a8a545c60d24e8d102aff1519215355f4d260516ac427c48a6db10c4e7c47015`.
  The guarded runner source SHA-256 was
  `84a19b9feee800207fd2f11eaae849f7d06a39c66b7b87d463d4ce4554c13197`,
  and the executed release binary SHA-256 was
  `8363077b5b5d3371cfe8c28192a361fa61bf31951e369b697b7ffe4093c39b79`.
  The result file SHA-256 is
  `a8d9939d0fc4269685c89d2f72c02f1dab33de9e3358a44703c7cee4a44f015f`.
  Its 12 requests used 163 transmitted and 139 received bytes;
  `R000.H` was `0000`, and `T0.H` was exactly
  `["0", "270F", "270F"]`. The direct-read and `MWR` arrays were identical:
  `00000,+00000,0000,0000000000,+0000000000,00013`.
- Deterministic targeted tests cover the real `.H` vector, exact status
  rejection, all five numeric formats and boundaries, missing/extra fields,
  invalid current/preset values, overflow, and transport retirement.
- The post-correction local gate passed formatting, Clippy and rustdoc with
  warnings denied, all 153 tests, all examples, crate packaging, and the
  isolated generated-crate consumer. No live PLC communication was performed
  during this Rust verification.
- Codex self-review inspected the response split, command-derived count,
  structural-status validation before current/preset normalization, low- and
  high-level result shapes, error retirement, tests, docs, and package output.
  No accepted runtime finding remained; repeated empty separators retain the
  existing shared tokenization behavior and are outside this approved change.

- [x] Implementation completed in this repository.
- [x] Tests cover every local acceptance criterion.
- [x] Static, unit, integration, example, documentation, and package gates passed.
- [x] Codex self-review completed against the approved contract.
- [x] The Rust portion of the corrected representative live batch passed after
  separate explicit approval (`HL-KVX500-01`).
- [x] The Node.js, .NET, and Python portions of the corrected representative
  cross-language live batch have passed after separate explicit approval.
- [x] Documentation, migration notes, changelog, and API reference agree.
- [x] Family-level final acceptance is verified and `LIVE-HL-003` marked complete.

## LIVE-HL-004 / LIVE-HL-004-RUST-API / LIVE-HL-004-WIRE-GRAMMAR — Packed direct-bit word monitoring

Implementation scope: the public `HostLinkMonitorWord` type, `MWS` command
construction, connection-scoped `MWR` response metadata and validation, the
verification binary, compile-checked examples, deterministic transport tests,
user documentation, and migration guidance in this repository.

Target contract: a bare direct-bit-family `MWS` entry registers one packed
unsigned 16-bit word. `HostLinkMonitorWord::packed_direct_bits_u16(device)`
accepts only an unsuffixed direct-bit device and emits the exact bare device
token on the wire. Distinct internal packed metadata validates the corresponding
`MWR` field as exactly 1-5 ASCII decimal digits whose numeric value is from `0`
through `65535`. Leading zeros are optional and retained in the existing public
`String`; empty fields, signs, whitespace, nondecimal text, six or more digits,
and overflow fail. It is not validated as an individual bit. Bare scalar `RD`
and `MBS`/`MBR` retain their exact `0`/`1`/`ON`/`OFF` contract.

Compatibility impact: the public `HostLinkMonitorWord::DirectBit` variant and
`direct_bit` constructor are removed completely. No alias, deprecated member,
or Boolean compatibility interpretation remains. Callers that intended bare
packed `MWS` migrate to `PackedDirectBitsU16` or
`packed_direct_bits_u16`; callers that intended an individual bit migrate to
`register_monitor_bits`/`read_monitor_bits`. The verification binary replaces
the word-monitor `BIT` selector with `PACKED_U16` and rejects `BIT` rather than
silently changing its meaning.

Wire grammar impact: packed fields that the general `.U` parser could formerly
accept despite noncanonical spelling, including a leading sign or more than
five zero-padded digits, now fail as protocol errors and retire the supplying
transport. Numeric `HostLinkMonitorWord::numeric(..., "U")` parsing is unchanged.

Machine-verifiable acceptance criteria:

1. Mixed numeric and packed registrations preserve order and emit explicit
   suffixes only for numeric entries; a packed `R0` entry emits exact bare
   `MWS R000`.
2. Packed `MWR` values `0`, `2`, `13`, `00000`, `00002`, `00013`, and `65535`
   succeed and preserve their existing public `String` spelling.
3. Empty, signed, whitespace-bearing, non-ASCII/nondecimal, longer-than-five-
   digit, overflow, wrong-token-count, and other invalid packed response shapes
   fail as protocol errors and retire the supplying transport.
4. Retirement, explicit reopen, and ordinary close clear packed monitor
   registration metadata; `MWR` cannot be sent on the new connection before a
   new successful registration.
5. Packed construction rejects suffix-bearing or non-direct-bit devices before
   transport. Scalar direct-bit `RD` and `MBS`/`MBR` validation is unchanged.
6. The old public names have no source, test, example, or binary use; public
   docs, changelog, compile-checked examples, and this migration record identify
   the breaking migration.

Evidence:

- Existing approved KV-X500 raw evidence returned `MWS R5000 -> OK` followed by
  `MWR -> 00002` while adjacent `R5001` was ON. Explicit `.U` returned the same
  packed value, proving that bare `MWS` is an implicit packed unsigned 16-bit
  word rather than a Boolean projection.
- Local deterministic tests cover exact wire spelling, mixed registration,
  approved accepted spellings, every rejected grammar class, transport
  retirement, reconnect metadata, preflight rejection, and unchanged
  scalar/bit-monitor strictness.
- After the exact guarded Rust program was completed, compiled, reviewed, and
  separately approved, the public API read `R5000`–`R5015`, calculated `13`,
  sent bare `MWS R5000`, and returned preserved monitor string `00013`.
  Evidence: `D:\APP\live-kvx500-20260802\rust_mwr_semantic_acceptance_result.json`.
- The read-only Rust `HL-KVX500-01` evidence recorded under `LIVE-HL-003`
  independently reconfirmed the mixed six-target `MWS`/`MWR` contract: direct
  reads and monitor values were identical and the packed `R5000` field retained
  the exact PLC spelling `00013`. Evidence:
  `D:\APP\live-kvx500-20260802\rust_hl_kvx500_01_result.json`.
- The final read-only Rust `HL-KVX500-02` UDP batch passed two cycles of the
  fixed 11-request plan against `keyence:kv-x500` at
  `192.168.250.100:8501`, with all 22 request/response frames accepted and
  `writes: false`. Traffic totals were 22 requests, 316 transmitted bytes, and
  246 received bytes. One socket was created, bound/connected, reused for both
  cycles, and closed once; no active UDP endpoint remained after close. The
  post-close read was rejected without a send or counter change. Both cycles
  preserved packed `R5000` as `00013`, returned direct bits `1,0,1`, and had
  identical direct/monitor word and bit arrays. The runner `src/main.rs`
  SHA-256 was
  `355a862694ff69275bc7c3cf35cacd0023d954090487c0f7e0d2d890385db65a`.
  Evidence:
  `D:\APP\live-kvx500-20260802\rust_hl_kvx500_02_udp_result.json`, SHA-256
  `1a6f3de70ac89bcc802419bbbe2abf6c1c8965ac331fd7aa9fd9a2a5509f6ec1`.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Formatting, Clippy and rustdoc with warnings denied, 163 tests, all seven examples, crate packaging, and the isolated generated-crate consumer passed.
- [x] Codex self-review completed against the approved contract and cross-language evidence. Accepted findings corrected and reverified: the original packed metadata incorrectly reused general `.U` parsing, packed responses needed empty-field-preserving tokenization, and the ordinary-close packed-metadata regression was initially missing. Rejected, duplicate, and deferred findings: none.
- [x] Required wire semantics, corrected Rust public-API mapping, two-cycle UDP
  connection reuse, close cleanup, and post-close no-send behavior passed
  KV-X500 live acceptance in `HL-KVX500-02`.
- [x] Documentation, migration notes, changelog, binary, and compile-checked example agree with the implementation and identify the no-alias migration.
- [x] Final acceptance criteria verified and the Rust item marked complete.

## HL-RUST-001 / HL-RUST-002 — KV-X500 error continuity and read-plan segmentation

Implementation scope: live evidence for the existing typed PLC-error continuity
contract and the descending-boundary named-read/poll planner. This item changes
no runtime source, public API, profile catalog, or supported-device claim.

Target contract: a complete `E0` through `E9` PLC response is returned as
`HostLinkError::Plc` without retiring the healthy TCP connection. The next read
uses the same TCP tuple. For caller order `DM100:U`, `DM0:U`, `DM1:U`, both a
named read and one polling cycle use exactly two planned requests, return values
in caller order, and agree with direct reads of the same devices.

Runner-finding disposition: the first fixed runner selected `VM0.U` as the
error-producing request, but the KV-X500 returned the normal value `00000`.
That run is preserved as NG in
`D:\APP\live-kvx500-20260802\rust_hl_kvx500_03_result.json`, SHA-256
`6f3a75adc50d260e381adb8775da167a4f34883ce67295443488f290bcb883c5`.
This is an accepted runner probe-selection finding, not a PLC or library
failure. Maintainer support for `VM` remains limited to KV-7000 and KV-8000;
the observation does not add KV-X500 support and does not authorize a catalog
change. The corrected fixed runner used the public comment-read API for
`RDC DM120`, for which the same PLC/project had prior evidence that comment data
was not registered, and received exact typed PLC error `E6`.

Final live evidence: the corrected read-only `HL-KVX500-03` batch passed against
`keyence:kv-x500` at `192.168.250.100:8501` with `writes: false`. The PID-owned
TCP tuple remained
`192.168.250.110:54990 -> 192.168.250.100:8501` throughout the batch and was
removed on close. The post-error `DM0.U` continuity read returned `5878`.
Named, corresponding direct, and one-cycle polling results all agreed:
`DM100:U = 24243`, `DM0:U = 5878`, and `DM1:U = 0`; named read and polling each
used exactly two planned requests. The complete batch used exactly 9 requests,
100 transmitted bytes, and 63 received bytes. Evidence:
`D:\APP\live-kvx500-20260802\rust_hl_kvx500_03_corrected_result.json`,
SHA-256
`f9cb13cf2be887f3a17f2b0b0a781c32c59cabb3bdae352369c5c0fedf441cd8`.
The frozen corrected runner source SHA-256 was
`0c5cdcb0b0ded761285d836029cf6ae67521d398c2c5cd7fc2208f56ed0faced`,
and the executed release binary SHA-256 was
`fa8617e875662860d53d6abf38f1de9c402b455f4a4831df19dd75c0a19a319e`.

- [x] `HL-RUST-001` live evidence passed: a complete `E6` remained a typed PLC
  error and the following successful read used the same TCP tuple.
- [x] `HL-RUST-002` live evidence passed: named read and one polling cycle each
  used exactly two requests, preserved caller order, and matched direct values.
- [x] The original NG and corrected passing result are both preserved, and the
  runner finding has an explicit no-KV-X500-VM-support/catalog-change
  disposition.

## RS-REAUDIT-004 — Reject bracketed IPv4 literals

Implementation scope: public connection-option validation and endpoint
resolution preflight.

Target contract: an IPv4 literal is accepted only in unbracketed form. A value
such as `[127.0.0.1]` fails as a protocol input error before DNS resolution,
socket creation, connection, or protocol traffic. Existing hostname and IPv6
behavior is unchanged.

Compatibility impact: callers that supplied bracketed IPv4 literals must
remove the brackets.

Machine-verifiable acceptance criteria:

1. `[127.0.0.1]` is rejected by `HostLinkConnectionOptions::new`.
2. `127.0.0.1` remains valid.
3. Rejection cannot reach DNS, socket creation, connection, or send.
4. Documentation and the changelog state the unbracketed migration.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required because this is deterministic pre-transport validation.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## RS-REAUDIT-005 — Reject an empty raw command

Implementation scope: frame construction and the maintainer raw-send preflight.

Target contract: an empty raw command body is a protocol input error and must
not enter FIFO admission, inspect lifecycle state, mutate client state, create
a socket, connect, or send.

Compatibility impact: a former CR-only raw frame is no longer constructible.

Machine-verifiable acceptance criteria:

1. `build_frame("")` returns `HostLinkError::Protocol`.
2. `send_raw("")` returns the same category with unchanged traffic counters.
3. Every non-empty otherwise-valid raw command retains one terminating CR.
4. Documentation and the changelog state the empty-input rejection.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required because this is deterministic pre-transport validation.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## RS-REAUDIT-008 — Bound every raw request frame to 65,507 bytes

Implementation scope: shared TCP/UDP raw frame construction and boundary tests.

Target contract: a non-empty ASCII raw command body is at most 65,506 bytes;
the appended CR makes the complete frame at most 65,507 bytes. A body of 65,507
bytes or more fails before FIFO admission, lifecycle state, DNS, socket work,
connection, traffic counters, or send. Smaller command-specific limits remain
unchanged.

Compatibility impact: the former 65,536-byte Rust raw body limit is reduced by
30 bytes and applies identically to TCP and UDP.

Machine-verifiable acceptance criteria:

1. A 65,506-byte body constructs a 65,507-byte CR-terminated frame.
2. A 65,507-byte body fails as `HostLinkError::Protocol`.
3. Limit failure leaves traffic counters and transport activity unchanged.
4. TCP and UDP use the same frame builder and therefore the same boundary.
5. Documentation and the changelog use the body/frame units consistently.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required because this is deterministic frame validation.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

### RS-REAUDIT verification evidence and self-review disposition (2026-08-02)

- `run_ci.bat`: PASS. Formatting, Clippy with warnings denied, rustdoc with
  warnings denied, 143 library/integration/documentation tests, all seven
  examples, crate packaging, and the isolated generated-crate consumer passed.
- Accepted findings corrected during self-review: the public options fields
  required request-time bracketed-IPv4 validation in addition to constructor
  validation; TCP and UDP needed an explicit shared pre-state limit test; and
  the earlier RS-XOVER-005 65,536-byte request record needed an explicit
  supersession note. Rejected findings: none. Duplicate findings: none.
  Deferred findings: none.
- Live PLC verification is not required for RS-REAUDIT-004, -005, or -008.
  These changes reject input before DNS/socket/protocol traffic or define a
  deterministic local frame boundary; no PLC/profile support result changed.
  No live PLC communication was performed.

## Final non-live disposition recheck — `HL-001` and `HL-003`

Final source-state targeted checks passed on 2026-08-02 without PLC
communication.

- `HL-001`: `cargo test --test reaudit_contract
  tcp_rejects_a_second_nonempty_response_and_never_reuses_it -- --exact` and
  the same command for
  `state_changing_tcp_surplus_response_is_never_false_success` each passed
  1/1. The deterministic peer proves that the surplus line cannot become a
  later response, the transport retires, and a state-changing request cannot
  report false success.
- `HL-003`: exact tests
  `special_family_float32_rejects_before_fifo_and_transport` and
  `float32_parser_normalizer_and_hand_built_formatter_share_family_validation`
  in `tests/high_level.rs` each passed 1/1. Direct, named, polling, parser,
  normalizer, and hand-built forms reject `Z:F` before FIFO/transport.

- [x] `HL-001` deterministic non-live disposition reverified on the final source state.
- [x] `HL-003` deterministic non-live disposition reverified on the final source state.
