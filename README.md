[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

# KV Host Link Protocol for Rust

[![CI](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/actions/workflows/ci.yml)

[![Rust](https://img.shields.io/badge/Rust-000000?logo=rust&logoColor=white)](https://www.rust-lang.org/)

Async Rust implementation of the KEYENCE KV Host Link protocol, aligned with
the public Python, .NET, and Node-RED Host Link libraries.

## Scope

- TCP and UDP Host Link transport
- full low-level Host Link command surface from the `.NET` reference
- queued high-level helper API for typed reads/writes, comment reads, named snapshots, and polling
- `hostlink_verify_client` wrapper binary for diagnostics and compatibility checks

`T` / `C` preset writes use Host Link `WS` / `WSS` only on KV-8000/7000-series
CPU units. Manuals state that other CPU units do not support those commands
and return abnormal response `E1` when they are executed.

`AT` digital trimmer values are treated as 32-bit device points on PLC families
that support them; `AT0` defaults to `AT0:D`, and `AT7:D` is a valid endpoint.
`AT` is not listed in the WR/WRS device table, so write helpers reject AT before
sending.

## Installation

```bash
cargo add plc-comm-hostlink-rust
```

The package name is `plc-comm-hostlink-rust` and the library import path is
`plc_comm_hostlink`.

Examples and the verification wrapper require `--features cli`.

## Quick Start

```rust
use plc_comm_hostlink::{
    open_and_connect, read_named, read_typed, write_typed, HostLinkConnectionOptions,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = open_and_connect(HostLinkConnectionOptions::new("192.168.250.100")).await?;

    let dm0 = client.read_typed("DM0", "U").await?;
    client.write_typed("DM10", "U", dm0).await?;
    let comment = client.read_comments("DM20", true).await?;

    let snapshot = client
        .read_named(&["DM0", "DM1:S", "DM2:D", "DM4:F", "DM10.0", "DM20:COMMENT"])
        .await?;

    println!("{comment}");
    println!("{snapshot:?}");
    Ok(())
}
```

## High-Level API

- `HostLinkConnectionOptions`
- `open_and_connect`
- `read_typed` / `write_typed`
- `read_timer_counter` / `read_timer` / `read_counter`
- `read_comments`
- `device_range_catalog_for_model`
- `write_bit_in_word`
- `read_named`
- `poll`
- `read_words_single_request` / `read_dwords_single_request`
- `read_words_chunked` / `read_dwords_chunked`
- `write_words_single_request` / `write_dwords_single_request`
- `write_words_chunked` / `write_dwords_chunked`

Comment reads also accept XYM aliases such as `D10`, `E20`, `F30`, `M100`, `L200`, `X100`, and `Y100`.

`read_typed("T10", "D")` and `read_named(&["T10"])` return the timer/counter
preset value for compatibility. Use `read_timer_counter(&client, "T10")` or
`client.read_timer_counter("T10")` when the Host Link composite fields are
needed: `status`, `current`, and `preset`.

Device-range catalogs are also available for UI use cases such as device monitors:

```rust
use plc_comm_hostlink::{
    device_range_catalog_for_model, KvDeviceRangeCategory,
};

let catalog = device_range_catalog_for_model("KV-8000")?;
let dm = catalog.entry("DM").unwrap();
assert_eq!(catalog.model, "KV-8000");
assert_eq!(dm.device, "DM");
assert_eq!(dm.category, KvDeviceRangeCategory::Word);
assert_eq!(dm.lower_bound, 0);
assert_eq!(dm.upper_bound, Some(65534));
assert_eq!(dm.point_count, Some(65535));
assert_eq!(dm.address_range.as_deref(), Some("DM00000-DM65534"));
```

The full static range specification is documented in
[`docs/DEVICE_RANGES.md`](docs/DEVICE_RANGES.md).

## Verified Hardware

- CPU: `KV-7500`
- CPU: `KV-X500`
- Transport: `TCP` and `UDP`

## Verification

Run formatting, static analysis, and tests:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Build the diagnostic wrapper binary:

```bash
cargo build --features cli --bin hostlink_verify_client
```
