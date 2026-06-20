# Examples

## What is here

These examples show the high-level Host Link API against a PLC at `192.168.250.100:8501`. Each runnable example requires the canonical PLC profile as an explicit argument; the commands below use `keyence:kv-8000`.

## How to run

```bash
cargo run --example 01_minimal -- 192.168.250.100 8501 keyence:kv-8000
cargo run --example 02_typed_read_write -- 192.168.250.100 8501 keyence:kv-8000
cargo run --features cli --example basic_high_level -- 192.168.250.100 8501 keyence:kv-8000
cargo run --features cli --example kv_device_range_sample_compare -- 192.168.250.100 8501 keyence:kv-8000
```

## Example index

| Example | Run command | Purpose |
| --- | --- | --- |
| `01_minimal.rs` | `cargo run --example 01_minimal -- 192.168.250.100 8501 keyence:kv-8000` | Connect, read `DM0`, print the value, and disconnect. |
| `02_typed_read_write.rs` | `cargo run --example 02_typed_read_write -- 192.168.250.100 8501 keyence:kv-8000` | Demonstrate `read_typed` and `write_typed` with `U`, `S`, `D`, `L`, and `F`. |
| `basic_high_level.rs` | `cargo run --features cli --example basic_high_level -- 192.168.250.100 8501 keyence:kv-8000` | Read and write with the queued high-level client. |
| `kv_device_range_sample_compare.rs` | `cargo run --features cli --example kv_device_range_sample_compare -- 192.168.250.100 8501 keyence:kv-8000` | Resolve the PLC profile, sample supported device ranges, write/readback/restore test values, and report mismatches. |
