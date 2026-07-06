# Usage guide

## Recommended entry points

| Entry point | Signature or type | Use |
| --- | --- | --- |
| `HostLinkConnectionOptions` | `pub struct HostLinkConnectionOptions` | Configure host, port, timeout, transport, and frame suffix behavior. |
| `HostLinkClient::connect` | `pub async fn connect(options: HostLinkConnectionOptions) -> Result<Self, HostLinkError>` | Open a direct async client. |
| `open_and_connect` | `pub async fn open_and_connect(options: HostLinkConnectionOptions) -> Result<QueuedHostLinkClient, HostLinkError>` | Open a queued client that serializes operations. |
| `available_plc_profiles` | `pub fn available_plc_profiles() -> Vec<String>` | List the exact canonical profile strings accepted by the embedded range catalog. |
| `device_range_catalog_for_plc_profile` | `pub fn device_range_catalog_for_plc_profile(plc_profile: impl AsRef<str>) -> Result<KvDeviceRangeCatalog, HostLinkError>` | Select the profile-specific device range catalog. |
| `read_typed` | `pub async fn read_typed(client: &HostLinkClient, device: &str, dtype: &str) -> Result<HostLinkValue, HostLinkError>` | Read one typed value through the helper function. |
| `HostLinkClient::read_typed` | `pub async fn read_typed(&self, device: &str, dtype: &str) -> Result<HostLinkValue, HostLinkError>` | Read one typed value through the client method. |
| `write_typed` | `pub async fn write_typed<T: HostLinkPayloadValue>(client: &HostLinkClient, device: &str, dtype: &str, value: &T) -> Result<(), HostLinkError>` | Write one typed value through the helper function. |
| `HostLinkClient::write_typed` | `pub async fn write_typed<T: HostLinkPayloadValue>(&self, device: &str, dtype: &str, value: T) -> Result<(), HostLinkError>` | Write one typed value through the client method. |
| `read_named` | `pub async fn read_named<S: AsRef<str>>(client: &HostLinkClient, addresses: &[S]) -> Result<NamedSnapshot, HostLinkError>` | Read a named snapshot from several addresses, including `:COMMENT` strings. |
| `poll` | `pub fn poll<'a, S: AsRef<str> + 'a>(client: &'a HostLinkClient, addresses: &'a [S], interval: Duration) -> impl Stream<Item = Result<NamedSnapshot, HostLinkError>> + 'a` | Poll a named snapshot on an interval. |

## Connection

| Field | Type | Default from `HostLinkConnectionOptions::new` |
| --- | --- | --- |
| `host` | `String` | The host you pass to `new`. |
| `port` | `u16` | `8501` |
| `timeout` | `std::time::Duration` | 3 seconds |
| `transport` | `HostLinkTransportMode` | `HostLinkTransportMode::Tcp` |
| `append_lf_on_send` | `bool` | `false` |

```rust
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions, HostLinkTransportMode};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut options = HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?;
    options.port = 8501;
    options.timeout = Duration::from_secs(3);
    options.transport = HostLinkTransportMode::Tcp;

    let client = HostLinkClient::connect(options).await?;
    println!("{:?}", client.is_open().await);

    client.close().await?;
    Ok(())
}
```

## Performance notes

For stable local networks, UDP usually has the lowest latency. TCP is the safer
default for remote or less predictable networks because the OS handles
retransmission. The TCP transport disables Nagle buffering for small Host Link
command frames.

Reuse one connected client for repeated reads and writes. Prefer
`read_words_single_request`, `read_dwords_single_request`, or `read_named` over
many individual `read_typed` calls when one application snapshot can be read as
one request.

## Connection reuse and concurrent requests

Use `open_and_connect` when a client will be shared by multiple tasks. It returns
a `QueuedHostLinkClient`, which serializes operations so Host Link frames do not
interleave on one PLC connection.

If you use `HostLinkClient::connect` directly, keep access to that client under
your own operation boundary. Use `close` followed by `open` for an intentional
reconnect; after a persistent connection failure, create a new client from the
same `HostLinkConnectionOptions`.

## Read a single value

| Suffix | Value returned by `read_typed` | Words consumed |
| --- | --- | --- |
| `U` | `HostLinkValue::U16` | 1 |
| `S` | `HostLinkValue::I16` | 1 |
| `D` | `HostLinkValue::U32` | 2 for word devices; 1 native timer/counter point. |
| `L` | `HostLinkValue::I32` | 2 for word devices; 1 native timer/counter point. |
| `F` | `HostLinkValue::F32` | 2 |
| `H` | `HostLinkValue::Text` | 1 |
| Empty string | `HostLinkValue::Bool` | 1 direct bit point. |

```rust
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let value = client.read_typed("DM0", "U").await?;
    println!("{:?}", value);

    client.close().await?;
    Ok(())
}
```

## Write a single value

```rust
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let original = client.read_typed("DM120", "U").await?;
    client.write_typed("DM120", "U", 1234_u16).await?;
    let value = client.read_typed("DM120", "U").await?;
    println!("{:?}", value);
    client.write_typed("DM120", "U", original).await?;

    client.close().await?;
    Ok(())
}
```

## Named snapshot

```rust
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions, HostLinkValue};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let snapshot = client
        .read_named(&["DM0:U", "DM1:S", "DM2:D", "DM4:F", "DM120.0", "DM0:COMMENT"])
        .await?;
    println!("{:?}", snapshot);

    client.close().await?;
    Ok(())
}
```

## Block reads

| Function | Signature | Use |
| --- | --- | --- |
| `read_words_single_request` | `pub async fn read_words_single_request(client: &HostLinkClient, device: &str, count: usize) -> Result<Vec<u16>, HostLinkError>` | Read one contiguous word block with one Host Link request. |
| `read_dwords_single_request` | `pub async fn read_dwords_single_request(client: &HostLinkClient, device: &str, count: usize) -> Result<Vec<u32>, HostLinkError>` | Read one contiguous double-word block with one Host Link request. |
| `read_words_chunked` | `pub async fn read_words_chunked(client: &HostLinkClient, device: &str, count: usize, max_words_per_request: usize) -> Result<Vec<u16>, HostLinkError>` | Read many words split into smaller requests. |
| `read_dwords_chunked` | `pub async fn read_dwords_chunked(client: &HostLinkClient, device: &str, count: usize, max_dwords_per_request: usize) -> Result<Vec<u32>, HostLinkError>` | Read many double words split into smaller requests. |

```rust
use plc_comm_kv_hostlink::{
    HostLinkClient, HostLinkConnectionOptions, read_dwords_chunked, read_words_single_request,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let words = read_words_single_request(&client, "DM0", 4).await?;
    let dwords = read_dwords_chunked(&client, "DM100", 8, 2).await?;
    println!("{:?}", words);
    println!("{:?}", dwords);

    client.close().await?;
    Ok(())
}
```

## Bit in word

Use `.n` notation for bit-in-word reads and `write_bit_in_word` for masked writes.

```rust
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions, HostLinkValue};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let bit = client.read_named(&["DM120.0"]).await?;
    println!("{:?}", bit);
    let original_bit = matches!(bit.get("DM120.0"), Some(HostLinkValue::Bool(true)));

    client.write_bit_in_word("DM120", 0, true).await?;
    client.write_bit_in_word("DM120", 0, original_bit).await?;

    client.close().await?;
    Ok(())
}
```

## Polling

`poll` returns a stream. Add `futures-util` to your application when you want the `.next()` helper shown here.

```rust
use futures_util::StreamExt;
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions, poll};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;
    let addresses = ["DM0:U", "DM1:S", "DM2:D"];
    let mut stream = Box::pin(poll(&client, &addresses, Duration::from_millis(500)));

    if let Some(snapshot) = stream.next().await {
        println!("{:?}", snapshot?);
    }

    client.close().await?;
    Ok(())
}
```

## Operational recipes

The `multi_plc_monitor` example is a read-only multi-PLC monitor. It polls each
PLC in an independent reconnect loop, logs `connected`, `lost`, `reconnecting`,
and `recovered` transitions, and uses the same 1 second to 30 second backoff as
the reconnect polling sample.

The `config_polling` example is read-only JSON-driven polling. Its CSV output
uses the shared long form `timestamp,plc,tag,value`. YAML config is only
available in the Python recipes; the Rust recipe intentionally accepts JSON.

## Timer/counter helpers

`read_timer_counter` returns `TimerCounterValue { status, current, preset }`. Use `read_timer` for `T` devices and `read_counter` for `C` devices when you want the same shape with device-type validation.

```rust
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let timer = client.read_timer_counter("T0").await?;
    let timer_checked = client.read_timer("T0").await?;
    let counter_checked = client.read_counter("C0").await?;
    println!("{:?}", timer);
    println!("{:?}", timer_checked);
    println!("{:?}", counter_checked);

    client.close().await?;
    Ok(())
}
```

> **Caution:** Timer/Counter preset writes only on KV-8000/7000-series.

## Device comments

```rust
use plc_comm_kv_hostlink::{HostLinkClient, HostLinkConnectionOptions};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client =
        HostLinkClient::connect(HostLinkConnectionOptions::new("192.168.250.100", "keyence:kv-8000")?).await?;

    let comment = client.read_comments("DM20", true).await?;
    println!("{comment}");

    client.close().await?;
    Ok(())
}
```

## Address reference table

| Form | Meaning | Example |
| --- | --- | --- |
| `:U` | Unsigned 16-bit word. | `DM0:U` |
| `:S` | Signed 16-bit word. | `DM1:S` |
| `:D` | Unsigned 32-bit value. | `DM2:D` |
| `:L` | Signed 32-bit value. | `DM2:L` |
| `:F` | 32-bit floating point value. | `DM4:F` |
| `:BIT` | Direct bit device value. | `R200:BIT`, `CR000:BIT` |
| `:COMMENT` | PLC device comment text as `HostLinkValue::Text`. | `DM0:COMMENT` |
| `.n` | Bit-in-word index `0` through `F`. | `DM120.0`, `DM120.F` |

For `read_named` and `poll`, include the intended type. Use `DM0:U` instead of plain `DM0`.
