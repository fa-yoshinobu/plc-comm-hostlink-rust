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
  `T/C` read `E0`, `VB0/VB1` readback NG, raw `AT` write `E1` with current
  helpers rejecting AT before WR/WRS send, and `CTH/CTC` parser/client
  unsupported. See
  `docs/KV5000_LIVE_VALIDATION_2026-05-03.md`.

- [x] **KV-7000 class all-device sample compare**: On 2026-05-03, the live
  KEYENCE KV-7000 class target at `192.168.250.100:8501` reported model code
  `55` / `KV-7500` and resolved to the `KV-7000` embedded range catalog.
  `kv_device_range_sample_compare` completed with `passed=149`,
  `read_failed=20`, `write_failed=8`, `readback_failed=1`,
  `restore_failed=0`, `skipped=2`, and `unsupported=0`. Human review accepted
  the remaining NG/unsupported points as expected target behavior or catalog
  capability behavior: `CR2000` readback NG, `T/C` read `E0`, raw `AT` write
  `E1` with current helpers rejecting AT before WR/WRS send, and
  catalog-unsupported `CTH/CTC`. See
  `docs/KV7000_LIVE_VALIDATION_2026-05-03.md`.

## 3. Cross-Stack API Alignment

- [x] **Unify PLC profile naming across libraries**: Public Host Link catalog selectors now use canonical lowercase `PlcProfile` values such as `keyence:kv-x500` or `keyence:kv-7000`. Legacy KEYENCE model labels such as `KV-X500` are intentionally rejected as public profile input; runtime `?K` model labels remain separate query results used only for internal catalog resolution.
