# Getting started

## Start here

Use this page to make your first KEYENCE KV Host Link connection from Rust. The examples use the high-level async API and read from `DM0` before any write.

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

## Connect

`HostLinkConnectionOptions::new("192.168.250.100")` uses TCP port `8501` by default.

```rust
use plc_comm_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = HostLinkConnectionOptions::new("192.168.250.100");
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
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100")).await?;

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
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100")).await?;

    client.write_typed("DM120", "U", 1234_u16).await?;
    let dm120 = client.read_typed("DM120", "U").await?;
    println!("{:?}", dm120);

    client.close().await?;
    Ok(())
}
```

## Confirm success

1. Confirm your PLC is reachable at `192.168.250.100:8501`.
2. Confirm the connection example prints `true`.
3. Confirm the first read prints a `HostLinkValue`, such as `U16(...)`.
4. Confirm any write uses a test-only `DM` address.
5. Restore the test register if your procedure requires a known value.

## If it does not work

| Symptom | Check |
| --- | --- |
| Connection fails immediately | The default port is `8501`, not `1025`. |
| You need the verification binary | Build it with `cargo build --features cli --bin hostlink_verify_client`. |
| Reads fail on special devices | Start with `DM` reads first, then move to timers, counters, aliases, or comments. |

Detailed edge cases live in [GOTCHAS.md](GOTCHAS.md).

## Next pages

| Page | Link |
| --- | --- |
| Usage guide | [USAGE_GUIDE.md](USAGE_GUIDE.md) |
| Supported registers | [SUPPORTED_REGISTERS.md](SUPPORTED_REGISTERS.md) |
