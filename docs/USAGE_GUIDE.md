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
absolute body cap of 65,536 bytes. Request bodies use the same 65,536-byte cap;
one byte over is rejected before client state or traffic counters change.

`open` is idempotent while the same transport remains healthy. Transport
failure, timeout, EOF, or response overflow closes that transport. If the
caller drops an in-flight command future, the future returns no library
`Result`; the abandoned exchange poisons and retires the transport. A later
command returns `HostLinkError::NotConnected`; only an explicit `open` creates
the next transport, and the failed or abandoned command is not retried.

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
not use `None` as a default. Direct-bit writes accept Rust `bool` values only;
numeric and textual Boolean aliases are rejected before transport.

## Single-request block helpers

`read_words`, `read_dwords`, `write_words_single_request`, and
`write_dwords_single_request` each send at most one Host Link command.
Unsigned Dword helpers use native `.D` commands. Word requests accept at most
1,000 values and Dword requests at most 500 values, subject to stricter
device-family limits.

The library has no general chunked helper. A low-level read or write is never
divided automatically. If an application intentionally uses several requests,
it must own the address progression, timing difference, retry policy, and
partial-write handling.

## Named read results and polling

```rust
let values = client
    .read_named(&["DM0:U", "DM1:S", "DM2:D", "DM4:F", "DM120.D"])
    .await?;
```

`read_named` is the one read-only aggregate allowed to plan multiple requests.
All addresses are copied and validated before the first send. Compatible
adjacent values may share a request; when a request limit is reached, a new
request starts only at a declared value boundary, so a Dword or Float32 value
is never split. Requests retain declared input order.

The complete named read (or one `poll` cycle) owns one FIFO wire turn and
returns all requested values or an error, never a partial result. Multiple
request frames are not a PLC-atomic observation: PLC scan timing can differ
between segments. Applications that need one coherent PLC snapshot must use a
single request or an explicit PLC-side snapshot/handshake design. Named reads
and polls require at least one address, and poll intervals must be greater than
zero; invalid input fails before FIFO admission or communication. `poll` reuses
the validated plan for each cycle.

## Bit-in-word access

Bit-in-word notation (`DM120.0` through `DM120.F`) is read-only. The former
client-side read-modify-write helper was removed because it could overwrite a
concurrent PLC or external-client update. Use a PLC-native atomic bit operation
when available, or make the application explicitly own any non-atomic whole-
word read/write sequence.

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

## Shared clients and lifecycle

`HostLinkClient`, `HostLinkClient::connect`, and `open_and_connect` all use the
same client type. Clones share one FIFO admission queue and one wire turn, so
there is no separate queued wrapper or bypass alias. Arguments and timeout are
snapshotted at admission. Dropping a future while it is still waiting removes
it without a send and produces no library `Result`.

`close` rejects both the active operation and all operations waiting in the old
connection generation. A later explicit `open` creates a new generation; work
admitted before `close` cannot send on that reopened transport.

## Errors and uncertain write outcomes

`HostLinkError` distinguishes `Protocol`, `Timeout`, `Closed`, `NotConnected`,
`Transport`, `Plc`, and `OutcomeUnknown`. A state-changing command that may
already have been sent returns `OutcomeUnknown` when timeout, close, transport
failure, or malformed acknowledgement prevents a definite result. Raw commands
are conservatively treated as state-changing. The client closes the affected
transport and never retries automatically.

Rust cancellation is future drop rather than a returned library error. Dropping
a state-changing future after transmission may have started gives the caller no
`HostLinkError`; the caller must treat the PLC outcome as unknown. The transport
is poisoned and retired, so the next command returns `NotConnected` until
`open` succeeds. This caller-observed cancellation is distinct from the
library's `Timeout` result and is deliberately not a
`HostLinkOutcomeUnknownReason` variant.

## Traffic statistics

Call `client.traffic_stats().await` for cumulative request, transmitted-byte, and received-byte counts.
For TCP, a received line counts its body plus the first CR/LF terminator; extra CR/LF separators
are consumed but not counted. For UDP, the complete response datagram is counted.
