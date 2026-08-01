# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

**Entry labels**

- `Release`: Package/version metadata and publishing preparation.
- `Library`: Runtime behavior, public API, protocol handling, or validation in the distributed library.
- `Docs`: README, user guides, generated API docs, or other documentation-only changes.
- `Samples`: Examples, sample flows, sample scripts, or sample applications.
- `Tests`: Test suites, test fixtures, golden vectors, or verification data.
- `Tooling`: Developer/operator command-line tools and helper utilities.
- `CI`: Release checks, workflow scripts, or automation-only changes.

## [Unreleased]

- Breaking: Replace the queued/direct split with one `HostLinkClient` that owns FIFO admission and one complete logical wire turn across all clones. Remove `QueuedHostLinkClient` without an alias; `open_and_connect` now returns `HostLinkClient`.
- Breaking: Remove `write_bit_in_word`; bit-in-word notation remains read-only because client-side read-modify-write cannot be PLC-atomic. Direct-bit write inputs are now strictly `bool`.
- Breaking: Rename the public named-read result type from `NamedSnapshot` to `NamedReadResult` without a compatibility alias; the old name incorrectly implied that a multi-request aggregate was one PLC-atomic snapshot. The verification CLI poll response key is likewise renamed from `snapshots` to `results`.
- Breaking: Make every `RDC` text path require `HostLinkCommentEncoding::Utf8` or `Cp932`; remove UTF-8-first/Shift_JIS-fallback decoding without an alias or default. Ordinary `read_named`/`poll` now reject `:COMMENT` before transport, while explicitly named comment-encoding aggregate variants require at least one `:COMMENT` entry and reject an unused encoding before transport.
- Library: Add `read_comment_bytes` for exact terminator-free `RDC` payload bytes, including trailing padding. Strict text decoding never substitutes replacement characters or retries another codec; malformed payloads retire the connection. `Cp932` is the shared strict CP932/Windows-31J mapping used for KEYENCE `Shift_JIS` compatibility, rejects standalone `80`/`A0`/`FD`/`FE`/`FF`, and has no separate Shift-JIS selection.
- Tooling: Require verification CLI comment text reads to pass `--comment-encoding utf-8` or `cp932`, and add `read-comment-bytes` for byte/hex inspection.
- Library: Add dedicated `Timeout`, `Closed`, `Transport`, and `OutcomeUnknown` error categories. State-changing and raw operations report a machine-readable uncertain-outcome reason after a possible send, close the transport, and are never retried automatically. The reason set contains timeout, close, transport, and malformed response; it does not claim to return Rust future-drop cancellation.
- Library: Capture timeout at FIFO admission, use one monotonic absolute deadline through send, complete receive framing, and response decoding, and invalidate active and waiting work by connection generation on `close`.
- Library: Dropping a waiting future sends nothing. Dropping an active future returns no library result, poisons and retires the transport, and requires callers to treat a possibly transmitted state-changing operation as unknown; the next command returns `NotConnected` until explicit reopen.
- Library: Make `read_named` and each poll cycle one all-or-error FIFO turn. Prevalidate the entire address set, preserve declared wire order, keep multiword values whole at segment boundaries, and document that multi-frame reads are not PLC-atomic; coherent reads require one request or a PLC-side snapshot/handshake.
- Library: Enforce a 65,536-byte request-body cap in addition to the existing response-body cap; limit-plus-one fails before state or traffic changes. IPv4-only endpoint behavior remains the supported contract.
- Tests: Add FIFO aggregate, close/reopen generation, future-drop retirement, uncertain write outcome, named-read preflight/order, multiword boundary, bool-only input, and exact capacity boundary coverage.
- Tests: Add ambiguous UTF-8/CP932 and UTF-8-BOM selection vectors, CP932 control/singleton/malformed/unassigned/extension boundaries, strict malformed-byte/no-fallback coverage, exact raw padding checks, PLC-error connection-retention checks for raw/text comments, and zero-send rejection for implicit aggregate comment reads.
- Release: Aligned artifact roles so the registry package contains consumer runtime, native API metadata, license, README, and ecosystem-native examples where applicable while excluding repository tests and maintainer tooling; the GitHub source archive retains tracked non-hardware validation and maintainer inputs.
- Docs: README documentation links now include the shared Performance and Choosing a Language pages, and package registry metadata was expanded for discoverability. No functional change.
- Library: Reject Float32 writes to direct-bit devices, empty named reads/polls, and zero polling intervals before transport.
- Library: Interpret `R`/`MR`/`LR`/`CR` catalog endpoints as decimal bank plus a final two-digit bit field (`00..15`).
- Library: Accept only exact documented direct-bit response tokens, validate every timer/counter composite field, and close the connection after malformed semantic responses.
- Library: Make Host Link endpoints explicitly IPv4-only. IPv6 literals are rejected before socket creation, hostname resolution selects only IPv4 results, and UDP uses an IPv4 local socket.
- Library: Removed the former queued-client inner-client escape hatch as an intermediate hardening step; the queued wrapper itself is now removed by the breaking FIFO-client change above.
- CI: Include tests and fixtures in GitHub source archives and run the standard Cargo format, check, Clippy, documentation, and test gates from the extracted archive. The crates.io package remains minimal and excludes repository tests.
- CI: Generate the `.crate`, inspect its extracted contents, compile its examples and rustdoc, and build a separate path consumer using only the extracted package; this independently proves that the consumer artifact excludes repository tests and is usable without the checkout.
- CI: Build current-worktree source archives through an isolated temporary Git index so tracked modifications, non-ignored untracked files, and tracked deletions are all validated without changing the maintainer's real index.
- CI: Added an explicit Rust 1.85 all-target/all-feature check so the crate's declared minimum compiler is tested independently from the stable-toolchain gate.
- Docs: Documented Rust 1.85 as the declared minimum supported compiler in the getting-started guide.

## [3.2.1] - 2026-07-29

- Release: Bumped crate and lockfile metadata to `3.2.1`.
- Release: GitHub Release drafts now prepend this version's changelog section to generated notes and repair a missing section on workflow reruns.

### Changed
- Library: Removed an unreachable maintainer-only trace-hook implementation and its test-only model types. The hook was crate-private, had no production caller, and was not part of the public API.

### Fixed
- Library: TCP and UDP exchanges now share one checked absolute deadline across write and complete response assembly, so repeated partial data cannot restart the timeout. Timeout values too large to form a runtime deadline are rejected before transport use instead of risking a panic.
- Library: Direct-bit numeric and bit-in-word operations now preserve complete 16-/32-bit values, sequential typed reads pack direct-bit tokens, and RDS requests split at command limits.
- Library: Hexadecimal VB parsing preserves the `F` digit through `F9FF`, and profile/device catalog upper bounds no longer reject transport sends.

### Tests
- Tests: Added segmented-response deadline, delayed-write, UDP deadline, and oversized-timeout regression coverage.

## [3.2.0] - 2026-07-17

- Release: Bumped crate and lockfile metadata to `3.2.0`.
- CI: Excluded maintainer-only files, tests, and release tooling from generated source archives while retaining the complete example set, and added source-archive contract checks to local, CI, and release gates.

- Library: Added immutable client-lifetime traffic snapshots through `traffic_stats()` on direct and queued clients.
- Library: Made TCP receive-byte accounting independent of CR/LF segmentation by counting the response body and first terminator only; UDP datagram accounting is unchanged.

## [3.1.0] - 2026-07-13

### BREAKING
- Library: Require destination port, transport, and canonical PLC profile when constructing connection options. Constructors remain local-only; commands require an explicit successful `open` and return `HostLinkError::NotConnected` instead of opening or reopening a transport.
- Library: Fix the communication timeout default at 3 seconds, normal command termination at CR, and the internal response-body cap at 65,536 bytes. Public LF and buffer-size controls are removed.
- Library: Make `set_time` require an explicit, calendar-valid `HostLinkClock`; local-time creation remains a separate caller-invoked helper and never falls back to UTC.
- Library: Change maintainer `send_raw` to return undecoded response-body bytes without converting PLC error responses. It is hidden from ordinary generated user documentation.
- Library: Remove comment padding options. Comment decoding removes only trailing ASCII space bytes before UTF-8/Shift_JIS decoding.
- Library: Require expansion-buffer and low-level numeric formats, reject suffix-bearing low-level device inputs, and validate response tokens, numeric ranges, counts, and 32-bit spans without fallback conversion.
- Library: Remove all public chunked helpers. Word and native Dword helpers send at most one request and reject counts over their one-request limit.
- Library: Keep bit-in-word read-modify-write under one client lock so concurrent clones cannot interleave the read and write portions.
- Library: Remove the embedded PLC manual error-message lookup and retain only the returned PLC code and response.
- Library: Require UDP response terminators, reject suffixes on every non-format command, validate hexadecimal reads as exactly one 1..4-digit token, and limit clock years to `00..99`.
- Library: Validate command-derived response counts, including 16/32-point direct-bit numeric reads; accept only documented `0`/`1`/`ON`/`OFF` direct-bit tokens and close the session after malformed semantic responses.
- Library: Replace the lossy `From<HostLinkValue> for u16` conversion with fallible `TryFrom`, so non-`U16` variants cannot silently become zero.
- Tests: Add explicit 3-second/zero/TCP-timeout recovery coverage and remove library-local cross-implementation vectors; cross-language verification is a separate repository concern.
- Tooling: Allow the local release gate to package the reviewed working-tree diff before it is committed.
- Samples: Require port, transport, and canonical PLC profile in every runnable endpoint definition.

### Added
- Library: Added `KvHostLinkPlcProfileDescriptor` and crate-root `plc_profile_descriptors()` for canonical Host Link profile metadata.

### Changed
- Docs: Rebuilt getting-started, usage, API, profile, README, examples, and maintainer migration material around the explicit quality-overhaul contract.
- Tests: Added coverage for disconnected commands, raw bytes, response caps and expected counts, native Dword requests, calendar validation, comment padding, and concurrent bit-in-word updates.


- Release: Bumped crate and lockfile metadata to `3.1.0`.

### Fixed
- Library: Corrected ten KV device range cells against live PLC hardware and the KEYENCE simulator, and pinned the canonical profile source to `plc-comm-hostlink-profiles` `v1.2.0`. `VM` widens to `VM0-9999` on KV-NANO and `VM0-59999` on KV-3000/KV-5000; `Z` widens to `Z1-23` on KV-8000. `CTH` narrows to `CTH0-1` on the KV-3000 and KV-5000 XYM profiles, matching their base profiles: `CTH2` and `CTH3` were previously accepted there and are now rejected.
- Library: Poison and replace TCP/UDP transports after timeout, I/O failure, or a dropped request future so delayed responses cannot satisfy a later request.
- Tests: Cover UDP timeout and cancellation with delayed loopback responses.
- Docs: Remove hand-maintained next-page navigation from Getting Started.

## [3.0.0] - 2026-07-10

### Changed
- Release: Bumped crate metadata to `3.0.0`.
- Packaging: Included LICENSE in the published crate.
- Docs: Replaced relative README links with absolute URLs so they resolve on package registry pages.
- Docs: Updated Getting Started to use the queued `open_and_connect` entry point consistently with the recommended application API.
- Docs: Updated PLC profile documentation and API reference entries for the profile API re-export.
- Tests: Updated PLC profile display-name coverage to use the crate-root profile API.

### BREAKING
- Library: Breaking: Moved PLC profile lookup APIs out of `device_ranges` and re-exported them from the crate root; `device_ranges` now owns only the device-range catalog entry point.
- Migration: Import the profile functions from the crate root, for example `use plc_comm_kv_hostlink::{available_plc_profiles, display_name, normalize_plc_profile, profile_from_name};`.

## [2.0.0] - 2026-07-06

### BREAKING
- Release: Renamed the crates.io package and Rust import path.

| Old crate/use | New crate/use |
| --- | --- |
| `plc-comm-hostlink-rust` | `plc-comm-kv-hostlink` |
| `use plc_comm_hostlink::...` | `use plc_comm_kv_hostlink::...` |

### Added
- Docs: Added `docs/API_REFERENCE.md` as the standard user-facing API index and linked it from the README.

### Changed
- Release: Bumped package metadata to `2.0.0`.
- Docs: Updated README, Getting Started, docs.rs links, examples, tests, and release duplicate checks for `plc-comm-kv-hostlink` / `plc_comm_kv_hostlink`.
- CI: Kept the tag-driven release workflow for the renamed crate package.

## [1.3.0] - 2026-07-06

### Added
- Release: Bumped package metadata to `1.3.0` and synced the embedded profile fixture to `plc-comm-hostlink-profiles` `v1.1.0`.
- Library: Added `CTH`/`CTC` (high-speed counter / comparator, codes 04H/05H) device support to the address parser and command device-type sets, treated like the counter (`C`) device. Availability is model/unit dependent (governed by the canonical catalog).
- Library: Synced the embedded KV Host Link device-range catalog with the canonical `TC`/`TS`/`CC`/`CS` (timer/counter current and set value) rows and official `device_name` labels.

### Fixed
- Library: Corrected the misspelled `KvDeviceRangeCategory::FileRefresh` variant to `FileRegister`. The category is a descriptive label only; device identification uses `device_type`/device code and bit/word width uses `is_bit_device`.

### Changed
- CI: Added a tag-driven release workflow that re-runs checks and attaches the packaged crate to the GitHub release.

## [1.2.0] - 2026-07-05

### Changed
- Release: Bumped package metadata to `1.2.0`.
- Tooling: Normalized line-ending handling in the canonical profile JSON update script so `-SourceRoot` runs no longer report false changes.
- Library: `available_plc_profiles()` now fails loudly if the embedded device range table cannot be parsed instead of returning an empty list.
- Library: Synced the embedded KV Host Link device-range fixture to `plc-comm-hostlink-profiles` `v1.0.1`, including `display_name` labels for KEYENCE model families and XYM variants.
- Library: Added `display_name(plc_profile)` as the public UI-label helper while keeping stored PLC profile values canonical.
- Docs: Documented the profile display-name helper and canonical-ID storage guidance.
- Tests: Added canonical fixture parity coverage for profile `display_name` values.
- Samples: Added read-only Rust `multi_plc_monitor` and `config_polling` operational recipes with dry-run validation, reconnect backoff, and JSON config.
- Docs: Removed the per-library troubleshooting/code page; shared KV Host Link troubleshooting and code guidance now lives in the PLC Setup Guide.
- Docs: Removed the per-library latest communication verification page and links so user docs stay focused on usage, not verification logs.
- Docs: Removed the manual page-navigation block from Getting Started and rely on site navigation instead.
- Docs: Removed the thin per-library Troubleshooting page after moving common KV Host Link troubleshooting to the PLC Setup Guide.
- Docs: Moved shared KV Host Link gotcha and troubleshooting items to the common PLC Setup Guide and standardized the Gotchas page structure with SLMP.
- Docs: Moved shared supported-register and device-range guidance to the common KV Host Link Device Ranges page and kept the user docs to Getting Started, Usage Guide, PLC Profiles, and Gotchas.

## [1.1.1] - 2026-06-29

### Changed
- Release: Bumped crate metadata to `1.1.1`.
- Docs: Documented explicit Host Link value-format requirements in existing user docs.
- Samples: Updated the high-level example to use explicit value-format suffixes.

## [1.1.0] - 2026-06-29

### Changed
- Release: Bumped crate metadata to `1.1.0`.
- Library: Made Host Link device parsing require explicit device areas and value-format suffixes; numeric-only devices no longer default to `R`, and suffixless named addresses no longer infer a default format.
- Library: Removed the unused public `resolve_effective_format` helper so suffixless devices are not exposed through an implicit-format API.
- Docs: Refreshed Host Link getting-started, gotchas, supported-register, and usage guidance.
- Samples: Updated Host Link examples to use safer write/restore patterns.

### Fixed
- Library: Reject malformed embedded device-range segments while building the KV range catalog instead of silently defaulting invalid lower bounds to `0`.
- Library: `BIT_IN_WORD` now requires an explicit `.0` through `.F` bit index instead of treating a missing bit index as bit 0.

### Tests
- Tests: Added coverage for invalid embedded device-range segment parsing.
- Tests: Added coverage for rejecting bit-in-word logical addresses without an explicit bit index.
- Tests: Updated high-level and shared frame-vector coverage for explicit device/value-format requirements.

## [1.0.0] - 2026-06-24

### Changed
- Release: Bumped crate metadata and lockfile metadata to `1.0.0` for the first stable release line.

### Fixed
- Tests: Aligned Host Link frame-vector entries that share IDs with the .NET/Python vector sets, including `wre_dm100_vals` and `check_error_no`.
