# HostLink Rust quality-overhaul decision and acceptance record

This record maps the approved workspace decisions to the Rust implementation.
Breaking compatibility is intentional where it conflicts with an explicit,
single-request, profile-safe contract.

## Acceptance matrix

| Decision | Implementation | Tests | Checks | Codex review | Claude review | Findings | Live/disposition | Docs | Final |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| D-052 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-053 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-054 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-055 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-056 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-057 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-058 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-059 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-060 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-061 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-062 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-063 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |
| D-064 | [x] | [x] | [x] | [x] | [ ] | [ ] | [x] | [x] | [ ] |

`Live/disposition` is checked because these Rust changes are constructor,
pre-transport validation, deterministic frame, loopback transport-state, and
documentation contracts. They do not create new PLC/profile support claims.

Claude review remains pending explicit user authorization. No Claude command
has been run.

## Verification evidence

- `run_ci.bat`: format, Clippy with `-D warnings`, and all-target/all-feature tests passed.
- Tests: 37 library tests, 1 frame-vector integration test, and 37 high-level integration tests passed; all executable example targets also built and ran their zero-test harnesses.
- `RUSTDOCFLAGS=-D warnings cargo doc --no-deps --all-features` passed.
- `cargo package --allow-dirty` built and verified the packaged crate; the archive contained the five standard user pages, examples, source, vectors, and tests.
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
- Acceptance: golden frame vectors compare exact request bodies.

## D-055 — Receive buffers are internal

- Scope: TCP/UDP receive paths and semantic response validation.
- Target: 65,536-byte body cap, full UDP datagram handling, expected numeric token counts, and transport invalidation on overflow/mismatch.
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
- Acceptance: golden frames, invalid formats, boundaries, and 32-bit end-crossing tests.

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
