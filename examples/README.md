# Examples

## What is here

These examples show the high-level Host Link API against a PLC at `192.168.250.100:8501`. Each runnable example requires the canonical PLC profile as an explicit argument; the commands below use `keyence:kv-8000`.

Use only test addresses that are safe for your PLC program before you run any write example.

## How to run

```bash
cargo run --example 01_minimal -- 192.168.250.100 8501 keyence:kv-8000
cargo run --example 02_typed_read_write -- 192.168.250.100 8501 keyence:kv-8000
cargo run --features cli --example basic_high_level -- 192.168.250.100 8501 keyence:kv-8000
cargo run --features cli --example polling_reconnect -- 192.168.250.100 8501 keyence:kv-8000 DM100 U 1
cargo run --features cli --example multi_plc_monitor -- --plc line-a=192.168.250.100,keyence:kv-8000,8501,tcp --tag dm100=DM100:U --cycles 1 --dry-run
cargo run --features cli --example config_polling -- --config examples/config_polling.example.json --dry-run
cargo run --features cli --example kv_device_range_sample_compare -- 192.168.250.100 8501 keyence:kv-8000
```

## Example index

| Example | Run command | Purpose |
| --- | --- | --- |
| `01_minimal.rs` | `cargo run --example 01_minimal -- 192.168.250.100 8501 keyence:kv-8000` | Connect, read `DM0`, print the value, and disconnect. |
| `02_typed_read_write.rs` | `cargo run --example 02_typed_read_write -- 192.168.250.100 8501 keyence:kv-8000` | Demonstrate `read_typed` and `write_typed` with `U`, `S`, `D`, `L`, and `F`. |
| `basic_high_level.rs` | `cargo run --features cli --example basic_high_level -- 192.168.250.100 8501 keyence:kv-8000` | Read and write with the queued high-level client. |
| `polling_reconnect.rs` | `cargo run --features cli --example polling_reconnect -- 192.168.250.100 8501 keyence:kv-8000 DM100 U 1` | Read-only polling loop with automatic reconnect and backoff after transport loss. |
| `multi_plc_monitor.rs` | `cargo run --features cli --example multi_plc_monitor -- --plc line-a=192.168.250.100,keyence:kv-8000,8501,tcp --tag dm100=DM100:U --cycles 1 --dry-run` | Read-only multi-PLC polling with `connected`/`lost`/`reconnecting`/`recovered` states. |
| `config_polling.rs` | `cargo run --features cli --example config_polling -- --config examples/config_polling.example.json --dry-run` | Read-only polling from JSON config, with long-form `timestamp,plc,tag,value` CSV output. |
| `kv_device_range_sample_compare.rs` | `cargo run --features cli --example kv_device_range_sample_compare -- 192.168.250.100 8501 keyence:kv-8000` | Resolve the PLC profile, sample supported device ranges, write/readback/restore test values, and report mismatches. |
