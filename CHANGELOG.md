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

## [3.0.0] - 2026-07-10

### Changed
- Release: Bumped crate metadata to `3.0.0`.
- Packaging: Included LICENSE in the published crate.
- Docs: Replaced relative README links with absolute URLs so they resolve on package registry pages.

### BREAKING
- Library: Breaking: Moved PLC profile lookup APIs out of `device_ranges` and re-exported them from the crate root; `device_ranges` now owns only the device-range catalog entry point.
- Migration: Import the profile functions from the crate root, for example `use plc_comm_kv_hostlink::{available_plc_profiles, display_name, normalize_plc_profile, profile_from_name};`.

### Changed
- Docs: Updated Getting Started to use the queued `open_and_connect` entry point consistently with the recommended application API.
- Docs: Updated PLC profile documentation and API reference entries for the profile API re-export.
- Tests: Updated PLC profile display-name coverage to use the crate-root profile API.

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
