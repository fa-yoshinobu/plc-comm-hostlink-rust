# Getting started

## Start here

Use this page to make your first KEYENCE KV Host Link connection from Rust. The examples select the canonical profile `keyence:kv-8000`, use the high-level async API, and read from `DM0` before any write.

## Prerequisites

| Requirement | Value |
| --- | --- |
| Rust | Stable Rust compatible with this crate. |
| Async runtime | `tokio` with `macros` and `rt-multi-thread` enabled for `#[tokio::main]`. |
| PLC host | `192.168.250.100` |
| PLC port | `8501` |
| Protocol | KEYENCE KV Host Link Upper Link over TCP. |

## Add dependency

```bash
cargo add plc-comm-hostlink-rust
```

The package name is `plc-comm-hostlink-rust`; the import path in Rust code is `plc_comm_hostlink`.

## Choose profile

Use the exact canonical profile string that matches your PLC model when you need the device range catalog. The library does not infer the catalog from the PLC model response.

```rust
use plc_comm_hostlink::device_range_catalog_for_plc_profile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = device_range_catalog_for_plc_profile("keyence:kv-8000")?;
    println!("{}", catalog.plc_profile);
    Ok(())
}
```

## Connect

`HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?` uses TCP port `8501` by default.

```rust
use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?;
    let client = HostLinkClient::connect(options).await?;

    println!("{:?}", client.is_open().await);

    client.close().await?;
    Ok(())
}
```

## First read

Start with a `DM` read because it is the simplest word-register path on most KV setups.

```rust
use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let dm0 = client.read_typed("DM0", "U").await?;
    println!("{:?}", dm0);

    client.close().await?;
    Ok(())
}
```

Expected output shape:

```text
U16(1234)
```

The numeric value depends on your PLC state.

## First write

Use a dedicated test address only. The example uses `DM120` because the existing validation notes reserve nearby `DM` addresses for smoke checks on one test setup; choose an address that is safe on your PLC program.

```rust
use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let original = client.read_typed("DM120", "U").await?;
    client.write_typed("DM120", "U", 1234_u16).await?;
    let dm120 = client.read_typed("DM120", "U").await?;
    println!("{:?}", dm120);
    client.write_typed("DM120", "U", original).await?;

    client.close().await?;
    Ok(())
}
```

## Confirm success

1. Confirm your PLC is reachable at `192.168.250.100:8501`.
2. Confirm the connection example prints `true`.
3. Confirm the first read prints a `HostLinkValue`, such as `U16(...)`.
4. Confirm any write uses a test-only `DM` address.
5. Confirm the write example restores the original test-register value.

## If it does not work

| Symptom | Check |
| --- | --- |
| Connection fails immediately | The default port is `8501`, not `1025`. |
| Profile selection fails | Use one of the exact strings in [PROFILES.md](PROFILES.md); aliases and old combined names are rejected. |
| You need the verification binary | Build it with `cargo build --features cli --bin hostlink_verify_client`. |
| Reads fail on special devices | Start with `DM` reads first, then move to timers, counters, aliases, or comments. |

Common connection, profile, address, write-permission, and PLC error-code symptoms are in the shared [KV Host Link Error Codes](https://fa-yoshinobu.github.io/plc-comm-docs-site/plc-setup/kv/error-codes/) page.
