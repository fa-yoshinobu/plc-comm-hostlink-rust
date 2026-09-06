# Getting started

## Requirements

Use Rust 1.85 or newer. Rust 1.85 is the crate's declared minimum supported
compiler version.

Choose these values for the actual PLC before connecting:

- host name resolving to IPv4, or an IPv4 address;
- destination port in `1..=65535`;
- `HostLinkTransportMode::Tcp` or `HostLinkTransportMode::Udp`;
- the exact canonical PLC profile from [PROFILES.md](PROFILES.md).

The library does not infer any of those endpoint conditions. Communication
timeout may be omitted and is then 3 seconds.

The endpoint contract is IPv4-only because the target PLC configuration is
IPv4. IPv6 literals fail before socket creation; a hostname with no IPv4
result fails without sending a Host Link command. IPv4 literals must not use
URI-style brackets: use `127.0.0.1`, not `[127.0.0.1]`.

## Add the crate

```bash
cargo add plc-comm-kv-hostlink
```

## Connect and read

```rust
use plc_comm_kv_hostlink::{
    HostLinkClient, HostLinkConnectionOptions, HostLinkTransportMode,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = HostLinkConnectionOptions::new(
        "192.168.250.100",
        8501,
        HostLinkTransportMode::Tcp,
        "keyence:kv-8000",
    )?;

    // Construction performs no network I/O. `connect` explicitly opens the
    // transport before the first command.
    let client = HostLinkClient::connect(options).await?;
    let value = client.read_typed("DM0", "U").await?;
    println!("{value:?}");
    client.close().await?;
    Ok(())
}
```

`HostLinkClient::new` creates a disconnected client. Call `open` before any
command. An unconnected command returns `HostLinkError::NotConnected` without
creating a socket. After a TCP timeout, EOF, transport failure, or dropped
in-flight future, call `open` explicitly again. UDP keeps the resolved logical
endpoint and replaces only the affected socket on the next command. Commands
never retry the failed operation. Dropping a future produces no library
`Result`, and a caller that drops a possibly transmitted write must treat its
PLC outcome as unknown.

## First controlled write

Use only an address reserved by your PLC program for testing.

```rust
let original = client.read_typed("DM120", "U").await?;
client.write_typed("DM120", "U", 1234_u16).await?;
let readback_result = client.read_typed("DM120", "U").await;
let restore_result = client.write_typed("DM120", "U", original).await;
restore_result?;
let readback = readback_result?;
println!("{readback:?}");
```

The readback result is not propagated until after restoration is attempted. If
the test write itself has an unknown outcome, do not automatically restore or
retry; reopen the client, inspect `DM120`, and reconcile it explicitly. If the
restoration attempt fails, also inspect `DM120` and reconcile its value manually
before continuing.

## Common failures

| Symptom | Check |
| --- | --- |
| Constructor rejects the profile | Use an exact canonical profile string; aliases and display names are rejected. |
| Command returns `NotConnected` | Call `open` or use `HostLinkClient::connect`/`open_and_connect`. |
| Numeric read rejects the input | Pass a base device and an explicit format; do not pass `DM100.D` to low-level APIs. |
| A large read is rejected before transport | Reduce it to the one-request limit. The library does not split requests. |
| IPv6 endpoint is rejected | Configure the PLC's IPv4 address or a hostname with an IPv4 result. |

Shared setup and troubleshooting material is published on the
[PLC communication documentation site](https://plc-comm-docs-site.fa-labo.com/).
