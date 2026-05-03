# KV-5000 Live Validation 2026-05-03

Target PLC:

- Model: KEYENCE KV-5000
- Host: `192.168.250.100`
- TCP port: `8501`
- Protocol: KEYENCE Host Link

## Rust HostLink CLI

Commands:

```bash
cargo build --features cli --bin hostlink_verify_client
target/debug/hostlink_verify_client 192.168.250.100 8501 query-model
target/debug/hostlink_verify_client 192.168.250.100 8501 range-catalog
target/debug/hostlink_verify_client 192.168.250.100 8501 read-named DM0 DM1:S DM2:D DM4:F DM50.0 CR2006 CM705
```

Observed:

- `query-model`: OK, model code `52`, model `KV-5000`
- `range-catalog`: OK, resolved embedded table `KV-3000/5000`
- Named read: OK
  - `DM0=64959`
  - `DM1:S=32`
  - `DM2:D=0`
  - `DM4:F=0`
  - `DM50.0=false`
  - `CR2006=true`
  - `CM705=41`

## Write / Readback / Restore

Command path:

- `read-typed DM121 --dtype U`
- `write-typed DM121 --dtype U <candidate>`
- `read-typed DM121 --dtype U`
- `write-typed DM121 --dtype U <original>`
- `read-typed DM121 --dtype U`

Observed:

- `DM121`: `15070 -> 15087 -> 15070`
- Result: OK

## App Bridge Notes

- Android Rust bridge `kvLiveTest` passed when using `R200` for bit write/restore and `DM120` for word write/restore.
- `R0` is backed by real I/O on the current PLC and can be overwritten by PLC scan. Keep it as read-only live I/O in validation; do not use it as a write/readback target.
- `DM100` can also be controlled by the active PLC program on this target. Use `DM120` or nearby dedicated validation addresses for write/readback smoke checks.
- iOS C ABI Keyence connect and snapshot passed:
  - Connected as `KV-5000`
  - `DM120=26801`
  - CPU state `Run`

## Device Range Sample Compare

Command:

```bash
KV_SAMPLE_POINTS=10 cargo run --features cli --example kv_device_range_sample_compare -- 192.168.250.100 8501
```

Behavior:

- Uses the live range catalog resolved from `?M` (`KV-3000/5000`, model code `52`).
- Samples up to 10 addresses per device: first, second, middle/quarter points, and last.
- Performs read, write A, readback A, write B, readback B, and restore.
- `R` write samples start at `R200` to avoid the current PLC's real I/O at `R0`.
- The command exits non-zero when NG is present. This is intentional; NG is not hidden.

Summary:

- `passed=148`
- `read_failed=20`
- `write_failed=8`
- `readback_failed=2`
- `restore_failed=0`
- `skipped=0`
- `unsupported=2`

OK devices:

- Bit: `R`, `B`, `MR`, `LR`, `CR`
- Word: `CM`, `DM`, `EM`, `FM`, `ZF`, `W`, `TM`, `VM`, `Z`

NG / untested devices:

- `T`: 10 samples all `read_failed`, `E0: Abnormal device No.`
- `C`: 10 samples all `read_failed`, `E0: Abnormal device No.`
- `VB`: `VB0` and `VB1` `readback_failed`; remaining 8 samples passed.
- `AT`: 8 samples all `write_failed`, `E1: Abnormal command`.
- `CTH`, `CTC`: catalog entries exist, but the current Rust parser/client does not support these device types yet.

No restore failure was observed.
