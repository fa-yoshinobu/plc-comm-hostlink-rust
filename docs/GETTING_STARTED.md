# Getting started

## Requirements

Choose these values for the actual PLC before connecting:

- host name or IP address;
- destination port in `1..=65535`;
- `HostLinkTransportMode::Tcp` or `HostLinkTransportMode::Udp`;
- the exact canonical PLC profile from [PROFILES.md](PROFILES.md).

The library does not infer any of those endpoint conditions. Communication
timeout may be omitted and is then 3 seconds.

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
creating a socket. After timeout, cancellation, EOF, or transport failure,
call `open` explicitly again; commands never reconnect or retry themselves.

## First controlled write

Use only an address reserved by your PLC program for testing.

```rust
let original = client.read_typed("DM120", "U").await?;
client.write_typed("DM120", "U", 1234_u16).await?;
let readback = client.read_typed("DM120", "U").await?;
client.write_typed("DM120", "U", original).await?;
println!("{readback:?}");
```

## Common failures

| Symptom | Check |
| --- | --- |
| Constructor rejects the profile | Use an exact canonical profile string; aliases and display names are rejected. |
| Command returns `NotConnected` | Call `open` or use `HostLinkClient::connect`/`open_and_connect`. |
| Numeric read rejects the input | Pass a base device and an explicit format; do not pass `DM100.D` to low-level APIs. |
| A large read is rejected before transport | Reduce it to the one-request limit. The library does not split requests. |

Shared setup and troubleshooting material is published on the
[PLC communication documentation site](https://fa-yoshinobu.github.io/plc-comm-docs-site/).
