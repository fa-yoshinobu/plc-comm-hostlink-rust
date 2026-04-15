[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

# KV Host Link Protocol for Rust

Async Rust implementation of the KEYENCE KV Host Link protocol, aligned with
`plc-comm-hostlink-dotnet` and the shared `plc-comm-hostlink-cross-verify`
harness.

## Scope

- TCP and UDP Host Link transport
- full low-level Host Link command surface from the `.NET` reference
- queued high-level helper API for typed reads/writes, named snapshots, and polling
- `hostlink_verify_client` wrapper binary for cross-language verification

## Installation

```bash
cargo add plc-comm-hostlink-rust
```

The package name is `plc-comm-hostlink-rust` and the library import path is
`plc_comm_hostlink`.

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

    let snapshot = client
        .read_named(&["DM0", "DM1:S", "DM2:D", "DM4:F", "DM10.0"])
        .await?;

    println!("{snapshot:?}");
    Ok(())
}
```

## High-Level API

- `HostLinkConnectionOptions`
- `open_and_connect`
- `read_typed` / `write_typed`
- `write_bit_in_word`
- `read_named`
- `poll`
- `read_words_single_request` / `read_dwords_single_request`
- `read_words_chunked` / `read_dwords_chunked`
- `write_words_single_request` / `write_dwords_single_request`
- `write_words_chunked` / `write_dwords_chunked`

## Verification

Run the crate tests:

```bash
cargo test
```

Run the shared cross-verify harness after building the Rust wrapper:

```bash
cargo build --bin hostlink_verify_client
cd ../plc-comm-hostlink-cross-verify
python verify.py
```
