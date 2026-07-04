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

### Changed
- Docs: Removed the per-library Error Codes page; shared KV Host Link error-code guidance now lives in the PLC Setup Guide.
- Docs: Removed the per-library latest communication verification page and links so user docs stay focused on usage, not verification logs.

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
