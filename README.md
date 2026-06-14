[![CI](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/plc-comm-hostlink-rust.svg)](https://crates.io/crates/plc-comm-hostlink-rust)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

# KEYENCE KV Host Link for Rust

Rust library for KEYENCE KV Host Link PLC communication.

## Supported PLC profiles

The maintained profile table is in [PLC profiles](docs/PROFILES.md). Choose one exact canonical PLC profile from that table.

## Supported device types

The maintained device and range tables are in [Supported registers](docs/SUPPORTED_REGISTERS.md). Use that page for supported device families, address syntax, and profile-specific notes.

## Installation

```bash
cargo add plc-comm-hostlink-rust
```

The package name is `plc-comm-hostlink-rust`; the Rust import path is `plc_comm_hostlink`.

## Quick example

```rust
use plc_comm_hostlink::{
    HostLinkClient, HostLinkConnectionOptions, device_range_catalog_for_plc_profile,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = device_range_catalog_for_plc_profile("keyence:kv-7000")?;

    let mut options = HostLinkConnectionOptions::new("192.168.250.100");
    options.port = 8501;
    let client = HostLinkClient::connect(options).await?;

    let dm0 = client.read_typed("DM0", "U").await?;
    println!("{} DM0 = {:?}", catalog.plc_profile, dm0);

    client.close().await?;
    Ok(())
}
```

## Documentation

| Page | Use it for |
| --- | --- |
| [Full documentation site](https://fa-yoshinobu.github.io/plc-comm-docs-site/) | Unified docs for all PLC communication libraries. |
| [Getting started](docs/GETTING_STARTED.md) | Install the crate, connect to your PLC, and run your first read/write. |
| [Usage guide](docs/USAGE_GUIDE.md) | Use the high-level API and common Host Link workflows. |
| [Supported registers](docs/SUPPORTED_REGISTERS.md) | Check supported device families and address forms. |
| [PLC profiles](docs/PROFILES.md) | Choose the canonical KEYENCE KV profile for the target PLC. |
| [Gotchas](docs/GOTCHAS.md) | Troubleshoot common profile, address, timer/counter, and transport issues. |
| [Examples](examples/README.md) | Run maintained Rust examples. |

## Hardware verified

Live-device verification is maintained in [Latest communication verification](docs/LATEST_COMMUNICATION_VERIFICATION.md).
See that page for verified PLC models, transports, dates, limitations, and retained validation notes.

## License and registry

| Item | Value |
| --- | --- |
| License | [MIT](LICENSE) |
| Registry | [crates.io](https://crates.io/crates/plc-comm-hostlink-rust) |
| Package | `plc-comm-hostlink-rust` |
