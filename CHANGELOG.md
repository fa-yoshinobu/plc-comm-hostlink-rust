# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Fixed
- Corrected `URD` / `UWR` expansion unit buffer command framing so the data
  suffix is attached directly to the buffer address, for example `100.U`.

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
