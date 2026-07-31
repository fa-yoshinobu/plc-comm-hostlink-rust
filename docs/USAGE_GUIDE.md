# Usage guide

## Connection contract

`HostLinkConnectionOptions::new(host, port, transport, plc_profile)` requires
all endpoint and profile choices. Timeout defaults to 3 seconds and must be
greater than zero when changed.

TCP and UDP are IPv4-only. Hostnames are resolved, but only IPv4 results are
used; IPv6 literals are rejected before a socket is created.

```rust
use plc_comm_kv_hostlink::{
    HostLinkClient, HostLinkConnectionOptions, HostLinkTransportMode,
};

let options = HostLinkConnectionOptions::new(
    "192.168.250.100",
    8501,
    HostLinkTransportMode::Tcp,
    "keyence:kv-8000",
)?;
let client = HostLinkClient::new(options);
client.open().await?;
```

Normal command frames always end in CR (`0x0D`). There is no public LF append
option or receive-buffer-size option. TCP and UDP responses have an internal
absolute body cap of 65,536 bytes.

`open` is idempotent while the same transport remains healthy. Transport
failure, timeout, cancellation, EOF, or response overflow closes that
transport. A later command returns `HostLinkError::NotConnected`; only an
explicit `open` creates the next transport, and the failed command is not
retried.

## Typed values and address grammar

High-level addresses use a colon for the value type:

| Form | Meaning |
| --- | --- |
| `DM100:U` | unsigned 16-bit word |
| `DM100:S` | signed 16-bit word |
| `DM100:D` | unsigned 32-bit value |
| `DM100:L` | signed 32-bit value |
| `DM100:F` | 32-bit floating point value |
| `R100:BIT` | direct bit device |
| `DM100:COMMENT` | device comment |
| `DM100.0` through `DM100.F` | bit 0 through bit 15 in one word |

Therefore, `DM100.D` means bit 13, while `DM100:D` means an unsigned Dword.
Float32 (`F`) writes are defined only for word devices. Supplying a direct-bit
device is rejected before frame construction and is never reinterpreted as
consecutive bit writes.

Low-level numeric APIs require a base device and a separate format:

```rust
let values = client.read_consecutive("DM100", 2, Some("D")).await?;
client.write_consecutive("DM200", &[1_u32, 2_u32], Some("D")).await?;
```

Passing `DM100.D` to a low-level numeric API is rejected even when a matching
format argument is also present. Direct bit devices use `None` because the
device family and command already determine bit semantics; numeric devices do
not use `None` as a default.

## Single-request block helpers

`read_words`, `read_dwords`, `write_words_single_request`, and
`write_dwords_single_request` each send at most one Host Link command.
Unsigned Dword helpers use native `.D` commands. Word requests accept at most
1,000 values and Dword requests at most 500 values, subject to stricter
device-family limits.

The library has no chunked helper and never combines multiple PLC scan times
into one returned snapshot. If an application intentionally uses several
requests, it must own the address progression, timing difference, retry
policy, and partial-write handling.

## Named snapshots and polling

```rust
let snapshot = client
    .read_named(&["DM0:U", "DM1:S", "DM2:D", "DM4:F", "DM120.D"])
    .await?;
```

`read_named` may combine compatible adjacent values into one request, but it
does not exceed one-request protocol limits. Named reads and polls require at
least one address, and poll intervals must be greater than zero; invalid input
fails before queue execution or communication. `poll` reuses the compiled plan
for each cycle.

## Bit-in-word writes

`write_bit_in_word` performs its read-modify-write sequence under the client
lock. Concurrent updates through clones of the same client cannot interleave
between the read and write portions.

```rust
client.write_bit_in_word("DM120", 0, true).await?;
```

This is a compound operation, not a PLC-atomic instruction. Other PLC logic or
another client can still change the same word.

## Expansion-unit buffer access

Both URD and UWR require one of `U`, `S`, `D`, `L`, or `H`:

```rust
let values = client
    .read_expansion_unit_buffer(1, 100, 2, "U")
    .await?;
client
    .write_expansion_unit_buffer(2, 200, &[7_i16, 8_i16], "S")
    .await?;
```

The format controls signedness, width, point limits, and buffer span. Missing
or empty formats, invalid tokens, out-of-range values, and 32-bit end crossing
are rejected rather than converted.

## Comments

```rust
let comment = client.read_comments("DM20").await?;
```

Comment decoding removes only trailing ASCII space bytes (`0x20`). Tabs,
full-width spaces, other Unicode whitespace, and spaces inside the comment are
preserved. UTF-8 is attempted first, then Shift_JIS.

## PLC clock

`set_time` requires a `HostLinkClock`. It never substitutes the host clock.
The `year` field is the PLC's two-digit year and must be in `0..=99`.
Calendar fields, real date existence, and weekday agreement are validated
before transport.

```rust
let clock = plc_comm_kv_hostlink::HostLinkClock::now_local()?;
client.set_time(clock).await?;
```

Calling `now_local` is an explicit application choice. Failure to obtain the
local offset is returned and is not replaced with UTC.

## Shared clients

`open_and_connect` returns `QueuedHostLinkClient`, which serializes public
operations. Direct `HostLinkClient` requests are also serialized per client
instance; the queued wrapper additionally provides an application operation
boundary for helper workflows. The queued client does not expose its inner
direct client. Applications that require direct-client semantics construct or
connect a `HostLinkClient` explicitly.

## Traffic statistics

Call `client.traffic_stats().await` for cumulative request, transmitted-byte, and received-byte counts.
For TCP, a received line counts its body plus the first CR/LF terminator; extra CR/LF separators
are consumed but not counted. For UDP, the complete response datagram is counted.
