# Usage guide

## Connection contract

`HostLinkConnectionOptions::new(host, port, transport, plc_profile)` requires
all endpoint and profile choices. Timeout defaults to 3 seconds and must be
greater than zero when changed.

TCP and UDP are IPv4-only. Hostnames are resolved, but only IPv4 results are
used; IPv6 literals are rejected before a socket is created. IPv4 literals
must be written without URI-style brackets (`127.0.0.1`, not `[127.0.0.1]`).

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
absolute body cap of 65,536 bytes. Raw request bodies have a 65,506-byte cap,
so the terminating CR makes the complete TCP/UDP request frame at most 65,507
bytes. Empty raw command bodies and one byte over the request limit are rejected
before client state or traffic counters change.

One non-pipelined TCP request owns exactly one non-empty response. Additional
CR/LF separators are ignored, but a second non-empty response received before
another request owns it is a protocol error and retires the connection. For
TCP as well as UDP, Host Link has no request identifier. Serialization and the
pre-send unowned-data check therefore cannot distinguish a nonconforming late
response that arrives between the final check and the next send. The client
keeps a healthy TCP connection persistent because opening one connection per
request would add a TCP handshake to every normal command without adding a
protocol request identifier. Use a conforming endpoint; every anomaly that can
be observed retires the connection and requires an explicit reopen.

For UDP, `open` creates one connected IPv4 UDP socket for the logical session.
Complete valid exchanges reuse that socket and its local endpoint. A timeout,
cancellation, transport/protocol failure, malformed response, extra response,
or datagram already waiting before a send discards the socket; the next command
creates a replacement from the already resolved remote endpoint without a new
DNS lookup. An explicit `close` discards both the socket and logical session.
Because Host Link has no request identifier, a duplicate datagram that arrives
after the pre-send check but before the current response cannot be distinguished
perfectly; use a conforming endpoint and separate clients when isolation is
required.

`open` is idempotent while the same transport remains healthy. TCP transport
failure, timeout, EOF, response overflow, or a dropped in-flight future retires
the connection; call `open` before the next TCP command. UDP retires only the
affected socket and creates a replacement on the next command. Dropping a
future produces no library `Result`; a possibly transmitted write remains
outcome-unknown, and no failed or abandoned command is retried.

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
Float32 (`F`) parsing, formatting, reads, and writes are defined only for the
canonical ordinary `.U` families `DM`, `EM`, `FM`, `ZF`, `W`, `TM`, `CM`,
`VM`, `D`, `E`, and `F`. Direct-bit, `Z`, and special-response families such as
`R`, `T`, `C`, and `AT` reject `:F` before FIFO admission and frame construction;
they are never reinterpreted as consecutive word or bit operations.

Every numeric semantic `.H` value is the canonical four-character uppercase
form `0000` through `FFFF`. Short or lowercase PLC tokens are accepted only
after 16-bit hexadecimal validation and are then padded, for example `a`
becomes `000A`. Raw response APIs and hexadecimal write-frame spelling are
unchanged.

A low-level `T`/`C` single read returns three fields: structural status,
current value, and preset value. Status is validated from the PLC token as
exact `0` or `1`; the selected `.U`, `.S`, `.H`, `.D`, or `.L` format applies
only to current and preset. For example, a valid `.H` response is returned as
`["0", "270F", "270F"]`, not `["0000", "270F", "270F"]`. Code that
previously compared the first low-level token with `0000` or `0001` must
compare it with `0` or `1`. High-level timer/counter result types are unchanged.

Low-level numeric APIs require a base device and a separate format:

```rust
let original = client
    .read_consecutive("DM200", 2, Some("D"))
    .await?
    .into_iter()
    .map(|value| value.parse::<u32>())
    .collect::<Result<Vec<_>, _>>()?;
client.write_consecutive("DM200", &[1_u32, 2_u32], Some("D")).await?;
let readback_result = client.read_consecutive("DM200", 2, Some("D")).await;
let restore_result = client.write_consecutive("DM200", &original, Some("D")).await;
restore_result?;
let values = readback_result?;
```

Use only a range reserved for controlled testing. Restoration is attempted
after a confirmed write and before a readback error is propagated. If
restoration fails, inspect and reconcile the range explicitly. If the write
outcome is unknown, do not issue an automatic restore or retry.

Custom low-level values implement the fallible formatter contract:

```rust
use plc_comm_kv_hostlink::{HostLinkError, HostLinkPayloadValue};

struct Code(u16);

impl HostLinkPayloadValue for Code {
    fn format_for_suffix(&self, suffix: &str) -> Result<String, HostLinkError> {
        match suffix {
            ".U" => Ok(self.0.to_string()),
            _ => Err(HostLinkError::protocol(format!(
                "Code does not support suffix '{suffix}'"
            ))),
        }
    }
}
```

`append_to_payload` and all normal write helpers propagate this `Result` and
send nothing on error. An empty successful token is also rejected without
changing the output. Do not return an empty string or another fallback token.

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
All addresses are copied and validated before FIFO admission or the first send.
Wire-compatible device types are grouped in their first-appearance order. Each
group is sorted by address and contiguous ranges are merged up to the request
limit, so alternating device types or descending input do not add avoidable PLC
round trips. A new segment starts only at a declared value boundary, so a Dword
or Float32 value is never split. Wire order is the optimized group/address
order; returned keys and values retain declared input order.

Named keys must be semantically unique by device family, numeric address,
dtype, bit index, and scalar count. Case and leading zeros do not make a second
key distinct. Different dtype views of the same word, different bit indices,
and overlapping multiword spans are valid. Result keys preserve the original
input strings.

The complete named read (or one `poll` cycle) owns one FIFO wire turn and
returns all requested values or an error, never a partial result. Multiple
request frames are not a PLC-atomic observation: PLC scan timing can differ
between segments. Applications that need one coherent PLC snapshot must use a
single request or an explicit PLC-side snapshot/handshake design. Named reads
and polls require at least one address, and poll intervals must be greater than
zero; invalid input fails before FIFO admission or communication. `poll` reuses
the validated minimum-request plan for each cycle. The FIFO turn is released
after the cycle's final response is decoded and staged; result-object assembly
and the completion-to-next-cycle interval do not block another wire operation.

The ordinary `read_named` and `poll` APIs reject `:COMMENT` entries during
complete-plan validation and send no request. When an aggregate intentionally
contains comments, use `read_named_with_comment_encoding` or
`poll_with_comment_encoding` and pass one explicit `HostLinkCommentEncoding`.
Those explicit variants require at least one `:COMMENT` entry; a non-comment-only
list rejects the unused encoding before FIFO admission or communication.

## Bit-in-word access

Bit-in-word notation uses hexadecimal indexes (`DM120.0` through `DM120.F`).
Use the explicit operation only when a client-side read-modify-write is the
intended policy:

```rust
client.write_bit_in_word("DM120", 10, true).await?;
// Equivalent helper:
plc_comm_kv_hostlink::write_bit_in_word(&client, "DM120", 10, true).await?;
```

The value is a Rust `bool`, the index is `0..=15`, and the target must be an
ordinary 16-bit word device. The complete plan is rejected before FIFO
admission if invalid. After activation, one absolute transaction deadline
covers exactly one word read and one word write in the same client FIFO turn;
queue wait is outside that deadline. The write is sent even when the bit is
already in the requested state. There is no fallback, resend, success readback,
or implicit named-write behavior.

The operation is not PLC-atomic. PLC logic or another connection can update
the word between requests and that update can be lost. Use PLC-side logic, a
handshake, or exclusive complete-word ownership for stronger guarantees.
Dropping before the write starts sends no write. A drop after write transmission
may have started has unknown PLC outcome and retires the transport; reopen and
reconcile instead of retrying automatically. A returned complete PLC error is
definitive and does not by itself retire a healthy connection.

Expansion-unit buffer memory uses its own route-specific operation:

```rust
client
    .write_bit_in_expansion_unit_buffer(1, 100, 3, true)
    .await?;
// Equivalent helper:
plc_comm_kv_hostlink::write_bit_in_expansion_unit_buffer(
    &client, 1, 100, 3, true,
)
.await?;
```

The unit/address and one `.U` word remain immutable across exactly one `URD`
point and one `UWR` point. Ordinary-device and expansion-unit routes never fall
back to one another. The shared-deadline, future-drop/outcome reconciliation,
no-readback, and non-PLC-atomic rules are otherwise identical.

## Word monitor registration

`MWS` entries beginning at a direct-bit device are packed word views, not
individual Boolean entries. Use the explicit packed constructor so the API
matches the PLC response contract:

```rust
use plc_comm_kv_hostlink::HostLinkMonitorWord;

client
    .register_monitor_words(&[
        HostLinkMonitorWord::numeric("DM120", "U"),
        HostLinkMonitorWord::packed_direct_bits_u16("R5000"),
    ])
    .await?;
let values = client.read_monitor_words().await?;
```

The packed constructor contributes the bare `R5000` token to `MWS`; registered
by itself it sends `MWS R5000`. Its `MWR` field must be exactly 1-5 ASCII decimal
digits with a numeric value from `0` through `65535`. Leading zeros are optional,
so `0`, `2`, `13`, `00000`, `00002`, `00013`, and `65535` are valid and retain
their original `String` spelling. Empty fields, signs, whitespace, nondecimal
text, six or more digits, and overflow are rejected. It is not valid to treat
this field as a Boolean. Use `register_monitor_bits` and `read_monitor_bits`
when each registered entry must remain an exact `0`/`1`/`ON`/`OFF` bit.

The former `HostLinkMonitorWord::DirectBit`/`direct_bit` API was removed without
an alias. Replace it with `packed_direct_bits_u16` when the intended operation is
bare-wire packed `MWS`, or move the operation to the bit monitor APIs when the
intended result is an individual bit.

## Expansion-unit buffer access

Both URD and UWR require one of `U`, `S`, `D`, `L`, or `H`:

```rust
let original = client
    .read_expansion_unit_buffer(2, 200, 2, "S")
    .await?
    .into_iter()
    .map(|value| value.parse::<i16>())
    .collect::<Result<Vec<_>, _>>()?;
client
    .write_expansion_unit_buffer(2, 200, &[7_i16, 8_i16], "S")
    .await?;
let readback_result = client
    .read_expansion_unit_buffer(2, 200, 2, "S")
    .await;
let restore_result = client
    .write_expansion_unit_buffer(2, 200, &original, "S")
    .await;
restore_result?;
let values = readback_result?;
```

The format controls signedness, width, point limits, and buffer span. Missing
or empty formats, invalid tokens, out-of-range values, and 32-bit end crossing
are rejected rather than converted. Use only a configured unit and buffer range
reserved for controlled testing. If the write outcome is unknown, inspect and
reconcile the module state explicitly rather than restoring or retrying blindly.
If restoration fails after a confirmed write, inspect the buffer range and
reconcile its values manually before continuing.

## Comments

```rust
use plc_comm_kv_hostlink::HostLinkCommentEncoding;

let utf8_comment = client
    .read_comments("DM20", HostLinkCommentEncoding::Utf8)
    .await?;
let cp932_comment = client
    .read_comments("DM21", HostLinkCommentEncoding::Cp932)
    .await?;
let exact_payload = client.read_comment_bytes("DM22").await?;
```

There is no automatic, default, or profile-selected comment encoding.
`HostLinkCommentEncoding::Cp932` means CP932/Windows-31J and is the selection
for KEYENCE documentation that describes the compatible encoding as
`Shift_JIS`; Rust does not expose a second strict-Shift-JIS variant.
For cross-runtime consistency, this selection preserves ASCII control bytes,
rejects standalone `80`, `A0`, `FD`, `FE`, and `FF`, and accepts defined NEC,
IBM, and duplicate CP932 extension pairs.
Under `Utf8`, an initial `EF BB BF` is preserved as `U+FEFF` comment data rather
than removed as a signature; the same byte sequence is invalid under `Cp932`.

Text decoding is strict. It removes only trailing ASCII space padding bytes
(`0x20`) and fails with `HostLinkError::Protocol` when the selected codec cannot
decode the remaining payload. It never retries the other codec or inserts
replacement characters, and a malformed payload retires the connection.
An exact, correctly framed PLC `E0` through `E9` response returns
`HostLinkError::Plc` without retiring the connection, so a later command may
reuse it. This applies to all semantic commands, including state-changing
commands; malformed framing and malformed non-PLC payloads still retire it.
Tabs, full-width spaces, other Unicode whitespace, and spaces inside the
comment are preserved. `read_comment_bytes` excludes only the transport CR/LF
terminator and returns the exact payload, including trailing ASCII spaces; use
it whenever the application cannot assert the stored encoding.

## PLC clock

`set_time` requires a `HostLinkClock`. It never substitutes the host clock.
The `year` field is the PLC's two-digit year and must be in `0..=99`.
Calendar fields, real date existence, and weekday agreement are validated
before transport. Setting the clock changes PLC state and elapsed time makes an
exact automatic restore impossible. Run this only on a controlled PLC when
replacing its clock with the host's local time is explicitly intended, then
verify the PLC clock through the engineering environment.

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
are conservatively treated as state-changing. The client retires the affected
TCP connection or UDP socket and never retries automatically.

Rust cancellation is future drop rather than a returned library error. Dropping
a state-changing future after transmission may have started gives the caller no
`HostLinkError`; the caller must treat the PLC outcome as unknown. TCP returns
`NotConnected` until `open` succeeds. UDP drops the in-flight socket and creates
a replacement from the resolved endpoint for the next command. This
caller-observed cancellation is distinct from the library's `Timeout` result
and is deliberately not a `HostLinkOutcomeUnknownReason` variant.

## Traffic statistics

Call `client.traffic_stats().await` for cumulative request, transmitted-byte, and received-byte counts.
For TCP, a received line counts its body plus the first CR/LF terminator; extra CR/LF separators
are consumed but not counted. For UDP, the complete response datagram is counted.
