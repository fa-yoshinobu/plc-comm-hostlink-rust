[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

# KV Host Link Protocol for Rust

Async Rust implementation of the KEYENCE KV Host Link protocol, aligned with
`plc-comm-hostlink-dotnet` and the shared `plc-comm-hostlink-cross-verify`
harness.

## Scope

- TCP and UDP Host Link transport
- full low-level Host Link command surface from the `.NET` reference
- queued high-level helper API for typed reads/writes, comment reads, named snapshots, and polling
- `hostlink_verify_client` wrapper binary for cross-language verification

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

## Verification

Run formatting, static analysis, and tests:

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Run the shared cross-verify harness after building the Rust wrapper:

```bash
cargo build --features cli --bin hostlink_verify_client
cd ../plc-comm-hostlink-cross-verify
python verify.py
```
