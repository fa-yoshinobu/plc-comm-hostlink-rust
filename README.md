[![CI](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/plc-comm-kv-hostlink.svg)](https://crates.io/crates/plc-comm-kv-hostlink)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/blob/main/LICENSE)

# KEYENCE KV Host Link for Rust

Rust library for KEYENCE KV Host Link PLC communication.

## PLC Comm Family

This library is part of the plc-comm family. See the [package matrix](https://fa-yoshinobu.github.io/plc-comm-docs-site/package-matrix/) for protocol, language, registry, and install-command mapping.

## Supported PLC profiles

The maintained profile table is in [PLC profiles](https://fa-yoshinobu.github.io/plc-comm-docs-site/hostlink/rust/PROFILES/). Choose one exact canonical PLC profile from that table.

## Supported device types

The shared device and range tables are in the [KV Host Link Device Ranges](https://fa-yoshinobu.github.io/plc-comm-docs-site/plc-setup/kv/device-ranges/) page. Use that page for supported device families, address syntax, and profile-specific notes.

## Installation

```bash
cargo add plc-comm-kv-hostlink
```

The package name is `plc-comm-kv-hostlink`; the Rust import path is `plc_comm_kv_hostlink`.

## Quick example

```rust
use plc_comm_kv_hostlink::{
    HostLinkClient, HostLinkConnectionOptions, HostLinkTransportMode,
    device_range_catalog_for_plc_profile,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = device_range_catalog_for_plc_profile("keyence:kv-8000")?;

    let options = HostLinkConnectionOptions::new(
        "192.168.250.100",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )?;
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
| [Getting started](https://fa-yoshinobu.github.io/plc-comm-docs-site/hostlink/rust/GETTING_STARTED/) | Install the crate, connect to your PLC, and run your first read/write. |
| [Usage guide](https://fa-yoshinobu.github.io/plc-comm-docs-site/hostlink/rust/USAGE_GUIDE/) | Use the high-level API and common Host Link workflows. |
| [API reference](https://fa-yoshinobu.github.io/plc-comm-docs-site/hostlink/rust/API_REFERENCE/) | Find public client methods, helpers, profile APIs, and error types. |
| [PLC profiles](https://fa-yoshinobu.github.io/plc-comm-docs-site/hostlink/rust/PROFILES/) | Choose the canonical KEYENCE KV profile for the target PLC. |
| [KV Host Link Device Ranges](https://fa-yoshinobu.github.io/plc-comm-docs-site/plc-setup/kv/device-ranges/) | Check shared device families, address notation, and range tables. |
| [KV Host Link Troubleshooting & Codes](https://fa-yoshinobu.github.io/plc-comm-docs-site/plc-setup/kv/troubleshooting-codes/) | Troubleshoot common port, profile, address, write-permission, and PLC error-code symptoms. |
| [Gotchas](https://fa-yoshinobu.github.io/plc-comm-docs-site/hostlink/rust/GOTCHAS/) | Check whether this library has any current library-specific caveats. |
| [Examples](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/blob/main/examples/README.md) | Run maintained Rust examples. |

## License and registry

| Item | Value |
| --- | --- |
| License | [MIT](https://github.com/fa-yoshinobu/plc-comm-hostlink-rust/blob/main/LICENSE) |
| Registry | [crates.io](https://crates.io/crates/plc-comm-kv-hostlink) |
| Package | `plc-comm-kv-hostlink` |

## Commercial support

If you plan to embed this library in a paid or commercial product, please consider a separate support agreement or supporting the project as a sponsor.

Contact: <https://fa-labo.com/contact.html>
