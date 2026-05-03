# KV-7000 Live Validation 2026-05-03

Target PLC:

- User-selected target: KEYENCE KV-7000 class
- Runtime model query: code `55`, model `KV-7500`
- Resolved range catalog: `KV-7000`
- Host: `192.168.250.100`
- TCP port: `8501`
- Protocol: KEYENCE Host Link

## Rust HostLink CLI

Commands:

```bash
target/debug/hostlink_verify_client 192.168.250.100 8501 query-model
target/debug/hostlink_verify_client 192.168.250.100 8501 range-catalog
```

Observed:

- `query-model`: OK, model code `55`, model `KV-7500`
- `range-catalog`: OK, resolved embedded table `KV-7000`

## Device Range Sample Compare

Command:

```bash
KV_SAMPLE_POINTS=10 cargo run --features cli --example kv_device_range_sample_compare -- 192.168.250.100 8501
```

Behavior:

- Uses the live range catalog resolved from `?M` (`KV-7000`, model code `55`).
- Samples up to 10 addresses per device: first, second, middle/quarter points, and last.
- Performs read, write A, readback A, write B, readback B, and restore.
- `R` write samples start at `R200` to avoid low real-I/O relay addresses.
- The command exits non-zero when NG is present. This is intentional; NG is not hidden.

Summary:

- `passed=149`
- `read_failed=20`
- `write_failed=8`
- `readback_failed=1`
- `restore_failed=0`
- `skipped=2`
- `unsupported=0`

OK devices:

- Bit: `R`, `B`, `MR`, `LR`, `VB`
- Word: `CM`, `DM`, `EM`, `FM`, `ZF`, `W`, `TM`, `VM`, `Z`

NG / untested devices:

- `CR`: `CR2000` `readback_failed`; remaining 9 samples passed.
- `T`: 10 samples all `read_failed`, `E0: Abnormal device No.`
- `C`: 10 samples all `read_failed`, `E0: Abnormal device No.`
- `AT`: 8 samples all `write_failed`, `E1: Abnormal command`.
- `CTH`, `CTC`: unsupported by the resolved `KV-7000` catalog.

Human review accepted these remaining NG/unsupported points as expected target behavior or catalog capability behavior.

No restore failure was observed.
