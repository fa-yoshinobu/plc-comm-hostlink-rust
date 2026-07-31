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
- Target: disconnected commands return typed `NotConnected`; failure closes transport; later commands do not reconnect or retry.
- Compatibility: callers must own initial connection and recovery.
- Acceptance: disconnected/open/failure/explicit-reopen sequence tests for TCP and UDP.

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
3. Unterminated UDP responses fail and close the connection generation; normal
   terminated datagrams continue to pass.
4. Non-composite `H` reads require exactly one 1..4-digit hex token, composite
   reads require exactly three tokens, and non-`U16` conversion fails.
5. Tests prove the 3-second timeout default, zero rejection, TCP timeout state,
   and absence of library-local cross-vector files/runners.
6. RD uses the command-derived token count, including 16/32-point direct-bit
   numeric formats; direct BIT accepts only `0`/`1`/`ON`/`OFF`, and malformed semantic
   responses close the transport generation.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Full format, Clippy `-D warnings`, 82-test, rustdoc `-D warnings`, example, and package checks passed.
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
may have started returns no `HostLinkError`, poisons and retires the transport,
and causes the next operation to return `NotConnected` until explicit reopen.
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
3. Trickled TCP and delayed UDP cross the deadline once, retire transport, and are not reused.
4. Durations that cannot form a deadline fail before transport without panic.

- [x] Implementation completed in this repository.
- [x] Tests added or updated for every acceptance criterion.
- [x] Relevant static checks, unit tests, integration tests, examples, and package/build checks passed.
- [x] Codex self-review completed against the approved contract and cross-language consistency requirements.
- [x] Live PLC checks are not required: timing/state behavior is deterministic with loopback TCP/UDP.
- [x] Documentation, migration notes, changelog, and generated API reference agree with the implementation.
- [x] Final acceptance criteria verified and the item marked complete.

## RS-XOVER-004 — Read-only aggregate planning and no bit RMW helper

Implementation scope: `read_named`, poll, plan compilation, value resolution,
single-request helpers, and bit-in-word public surface.

Target contract: all aggregate inputs are snapshotted and validated before the
first send. Planned requests retain declared wire order, multiword values never
split, the aggregate owns one FIFO turn, and any failure returns no partial
result. Multiple read frames are explicitly non-atomic. Public results use
`NamedReadResult`, not snapshot terminology; no `NamedSnapshot` compatibility
alias remains. No client-side bit-in-word read-modify-write API remains.

Compatibility impact: named-read request order can change to input order;
`write_bit_in_word` is removed without an alias. `NamedSnapshot` is renamed to
`NamedReadResult` without an alias, and the verification CLI poll response key
changes from `snapshots` to `results`.

Acceptance criteria:

1. An invalid later address causes zero sends.
2. Descending/mixed input produces frames in declared order and complete values.
3. Segment-limit rollover moves a complete Dword/Float32 value to the next frame.
4. An aggregate is all-or-error and holds one turn; docs state scan-time non-atomicity.
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

Target contract: request and response bodies accept exactly 65,536 bytes and
reject one byte over before state corruption; caller results are dynamically
owned and have no public receive-capacity setting. Command point limits reject
limit-plus-one without splitting. Direct-bit writes require `bool`. Endpoints
remain IPv4-only.

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
