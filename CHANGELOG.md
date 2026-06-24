# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-06-24

### Changed
- Bumped crate metadata and lockfile metadata to `1.0.0` for the first stable release line.

### Fixed
- Aligned Host Link frame-vector entries that share IDs with the .NET/Python vector sets, including `wre_dm100_vals` and `check_error_no`.

## [0.9.0] - 2026-06-21

### Changed
- Require an explicit canonical PLC profile when creating Host Link connection options, aligning standard connection behavior with the cross-language libraries.
- Updated examples to pass the PLC profile argument explicitly.

## [0.8.0] - 2026-06-14

### Changed
- Bumped release metadata to 0.8.0 for the unified PLC communication library release.

## [0.1.4] - 2026-06-12

### Changed
- Bumped the crate revision for release alignment after the resolved
  live-validation pass.

## [0.1.3] - 2026-05-14

### Added
- Added `TimerCounterValue` plus `read_timer_counter`, `read_timer`, and
  `read_counter` helpers for full `T` / `C` composite values.
- Added `read-timer-counter`, `read-timer`, and `read-counter` commands to the
  verification wrapper.

### Changed
- Documented that `WS` / `WSS` timer/counter preset writes are supported only
  by KV-8000/7000-series CPU units; other CPU units return abnormal response
  `E1` when those commands are executed.

### Fixed
- Corrected `URD` / `UWR` expansion unit buffer command framing so the data
  suffix is attached directly to the buffer address, for example `100.U`.
- Parse comma-separated timer/counter composite responses and make
  `read_typed()` return the preset value for `T` / `C` `.D` / `.L` reads.
- Added `write-set-value` and `write-set-values` to the verification wrapper.

## [0.1.2] - 2026-05-14

### Changed
- Extended the `hostlink_verify_client` wrapper for operator-run live validation commands and TCP/UDP selection.
- Added `KV-X500` to the README verified hardware list.

## [0.1.1] - 2026-05-02

### Changed
- Refreshed package metadata for the public crates.io release.

## [0.1.0] - 2026-05-02

### Added
- Initial async KEYENCE KV Host Link Rust client.
- Added high-level typed read/write helpers, named reads, polling, comment reads, and device-range catalog helpers.
- Added `hostlink_verify_client` and example coverage.
