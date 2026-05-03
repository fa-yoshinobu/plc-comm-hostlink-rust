# TODO: Host Link Communication Rust

This file tracks active follow-up items for the Rust Host Link library.

## 1. Active Follow-Up

- [x] **No blocking protocol issues**: No blocking protocol issues are open right now.

## 2. Validation Notes

- [x] **KV-5000 all-device sample compare**: On 2026-05-03, the live KEYENCE
  KV-5000 at `192.168.250.100:8501` was checked with
  `kv_device_range_sample_compare`. The harness performs read, write A,
  readback A, write B, readback B, and restore for up to 10 sampled addresses
  per catalog device. Summary: `passed=148`, `read_failed=20`,
  `write_failed=8`, `readback_failed=2`, `restore_failed=0`,
  `unsupported=2`. Human review accepted the remaining NG/unsupported points as
  expected target behavior or intentionally unsupported parser/client coverage:
  `T/C` read `E0`, `VB0/VB1` readback NG, `AT` write `E1`, and `CTH/CTC`
  parser/client unsupported. See
  `docs/KV5000_LIVE_VALIDATION_2026-05-03.md`.
