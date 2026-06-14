# Examples

## What is here

These examples show the high-level Host Link API against a PLC at `192.168.250.100:8501`.

## How to run

```bash
cargo run --example 01_minimal
cargo run --example 02_typed_read_write
cargo run --features cli --example basic_high_level
cargo run --features cli --example kv_device_range_sample_compare -- 192.168.250.100 8501
```

## Example index

| Example | Run command | Purpose |
| --- | --- | --- |
| `01_minimal.rs` | `cargo run --example 01_minimal` | Connect, read `DM0`, print the value, and disconnect. |
| `02_typed_read_write.rs` | `cargo run --example 02_typed_read_write` | Demonstrate `read_typed` and `write_typed` with `U`, `S`, `D`, `L`, and `F`. |
| `basic_high_level.rs` | `cargo run --features cli --example basic_high_level` | Read and write with the queued high-level client. |
| `kv_device_range_sample_compare.rs` | `cargo run --features cli --example kv_device_range_sample_compare -- 192.168.250.100 8501` | Resolve the PLC profile, sample supported device ranges, write/readback/restore test values, and report mismatches. |
