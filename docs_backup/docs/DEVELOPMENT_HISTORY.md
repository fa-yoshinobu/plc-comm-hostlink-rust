# Development History

Last consolidated: 2026-06-11

This document preserves the useful content that used to live in temporary
refactor memo files. Keep this file as the durable engineering record for the
Host Link communication crate.

## Library Contracts

- Preserve the public API unless an API migration is explicitly approved.
- Preserve existing frame strings and protocol behavior.
- Keep Android and iOS FFI consumers compatible.
- Prefer golden frame vectors before moving protocol logic.

## Refactoring History

### Golden Frame Expansion

Completed work:

- Expanded golden frame vectors from 20 to 36.
- Expanded `tests/frame_vectors.rs` dispatcher coverage.

Added frame vectors:

- `check_error_no ?E`
- `query_model ?K`
- `read_device_range_catalog ?K`
- `confirm_operating_mode ?M`
- `forced_set ST R000`
- `forced_reset RS R000`
- `read_monitor_bits MBR`
- `read_monitor_words MWR`
- `forced_set_consecutive STS R000 3`
- `forced_reset_consecutive RSS R000 3`
- `write_consecutive_legacy WRE DM100.U 3 100 200 300`
- `write_set_value_consecutive WSS T0.D 2 1000 2000`
- `switch_bank BE 3`
- `read_expansion_unit_buffer URD 01 100.U 2`
- `write_expansion_unit_buffer UWR 02 200.S 2 7 8`
- `read_comments RDC DM20`

Effect:

- The crate has stronger protection against frame-format regressions before
  moving builder logic.

### Read Plan Extraction

Completed work:

- Moved the private read-plan mechanism from `src/helpers.rs` to
  `src/read_plan.rs`.
- Added `mod read_plan;` privately from `src/lib.rs`.

Preserved:

- Public API.
- Existing vectors.
- Frame strings.
- `Cargo.toml`.
- Existing docs.
- Changelog.

Observed verification:

- `cargo fmt` passed.
- `cargo clippy --all-features` passed.
- `cargo test --all-targets --all-features` passed with 50 tests.
- CLI build passed.
- Rust doc build passed.
- Android rust-core checks passed.
- iOS FFI check passed.

Resolved during the work:

- Rustfmt wrapping differences.
- Compile reference path issue after extraction.

## Work Intentionally Not Done

- No live PLC validation in that refactor pass.
- No `set_time(None)` golden vector because it depends on current time and is
  not stable as a golden frame.
- No public API change.
- No Cargo metadata change.

## Future Notes

- Add new golden vectors before future protocol refactors.
- Keep volatile commands out of golden tests unless the volatile value can be
  injected or frozen.
- Live KEYENCE validation remains valuable after deeper protocol changes.
