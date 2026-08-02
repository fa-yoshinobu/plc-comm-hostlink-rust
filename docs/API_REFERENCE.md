# KV Host Link Rust API reference

This page indexes the supported user-facing surface. Maintainer raw frame and
trace facilities are intentionally omitted from ordinary user documentation.

## Connection and lifecycle

| Purpose | API |
| --- | --- |
| Validated options | `HostLinkConnectionOptions::new(host, port, transport, plc_profile)` |
| Disconnected client | `HostLinkClient::new` |
| Connected client | `HostLinkClient::connect`, `open_and_connect`, `HostLinkClientFactory::open_and_connect` |
| Lifecycle | `open`, `close`, `is_open` |
| Session values | `timeout`, `set_timeout`, `plc_profile` |
| Transport selection | `HostLinkTransportMode::{Tcp, Udp}` |

Endpoints are IPv4-only. IPv6 literals are caller errors; hostnames without an
IPv4 result fail as connection errors before protocol communication. IPv4
literals use the unbracketed form; `[127.0.0.1]` is rejected during option
validation. Raw command bodies must be non-empty ASCII without CR/LF and are
limited to 65,506 bytes, making the CR-terminated TCP/UDP request frame at most
65,507 bytes. Response bodies retain their internal 65,536-byte cap. There is
no caller-controlled receive capacity; public results own their dynamic storage.

## PLC operations

| Purpose | API |
| --- | --- |
| PLC mode | `change_mode`, `confirm_operating_mode`, `KvPlcMode` |
| Error operation | `clear_error`, `check_error_no` |
| PLC model | `query_model`, `KvModelInfo` |
| PLC clock | `set_time`, `HostLinkClock` |
| Forced bit control | `forced_set`, `forced_reset`, `forced_set_consecutive`, `forced_reset_consecutive` |
| Bank selection | `switch_bank` |

## Device operations

| Purpose | API |
| --- | --- |
| Low-level single read/write | `read`, `write` |
| Low-level consecutive read/write | `read_consecutive`, `write_consecutive` |
| Legacy command variants | `read_consecutive_legacy`, `write_consecutive_legacy` |
| Timer/counter set value | `write_set_value`, `write_set_value_consecutive` |
| Monitor registration | `register_monitor_bits`, `register_monitor_words`, `HostLinkMonitorWord` |
| Monitor read | `read_monitor_bits`, `read_monitor_words` |
| Expansion-unit buffer | `read_expansion_unit_buffer`, `write_expansion_unit_buffer` |
| Comment encoding | `HostLinkCommentEncoding::{Utf8, Cp932}` |
| Comment text | `read_comments(device, encoding)` |
| Comment bytes | `read_comment_bytes` |

Numeric low-level methods require a base device plus an explicit format.
Direct bit methods use an unsuffixed device. Suffix-bearing low-level device
strings are rejected. Direct-bit writes require `bool`; numeric and textual
aliases are rejected before transport.

Custom low-level write values implement `HostLinkPayloadValue`.
`format_for_suffix` returns `Result<String, HostLinkError>` and must return
`Err` for an unsupported suffix or invalid value. The default
`append_to_payload` appends only a complete successful token; formatter errors
and an empty returned token are rejected without changing the output. Failures
propagate through direct, consecutive, set-value, expansion-buffer, and typed
writes before transport. There is no infallible compatibility formatter.

`HostLinkClock.year` is the explicit two-digit PLC year and must be `0..=99`.
Semantic reads validate command-derived response counts. Bare direct-bit
responses accept only the exact tokens `0`, `1`, `OFF`, or `ON` without
trimming or case folding. Formatted direct-bit single reads return one packed
numeric token: `.U`, `.S`, and `.H` represent 16 bits, while `.D` and `.L`
represent 32 bits. Signed `.S` and `.L` tokens may include an explicit leading
`+`. Malformed semantic responses close the connection generation.
UDP responses require a CR/LF terminator. A valid completed exchange reuses the
connected UDP socket and local endpoint. Timeout, cancellation, I/O/protocol or
framing failure, an extra response, or a pre-send unowned datagram discards that
socket; the next operation creates a replacement from the resolved endpoint.
Host Link has no request identifier, so a duplicate arriving between the
pre-send check and current response remains inherently indistinguishable. TCP
accepts one non-empty response per request and retires the connection when it
receives an additional unowned non-empty response. The same request-identifier
limitation leaves a TCP race between the final pre-send check and send. Healthy
TCP connections remain persistent because a connection per request would add a
handshake without adding a protocol identifier; every observable anomaly still
retires the connection and requires explicit reopen.
All non-format commands, including forced control, monitor-bit registration,
comment reads, and timer/counter helpers, reject suffix-bearing devices.
Monitor reads require a successful registration in the current connection
generation and enforce the exact registered token count. Word-monitor
registration also preserves each entry's ordered format, and `MWR` validates
each token against its corresponding `.U`, `.S`, `.H`, `.D`, `.L`, or packed
direct-bit unsigned 16-bit format before returning any values.

`HostLinkMonitorWord::numeric(device, format)` emits the explicit numeric suffix
in `MWS`. `HostLinkMonitorWord::packed_direct_bits_u16(device)` accepts only an
unsuffixed direct-bit device, emits that exact bare device in `MWS`, and validates
the corresponding `MWR` field as exactly 1-5 ASCII decimal digits whose numeric
value is from `0` through `65535`. Leading zeros are optional and the returned
`String` preserves them. Empty fields, signs, whitespace, nondecimal text, six
or more digits, and overflow are rejected. It does not return one Boolean. For
individual bit values, use `register_monitor_bits` followed by
`read_monitor_bits`; those `MBR` fields remain strict `0`/`1`/`ON`/`OFF`. The
former `DirectBit` enum variant and `direct_bit` constructor were removed
without an alias.

## High-level helpers

| Purpose | API |
| --- | --- |
| Typed value | `HostLinkValue`, `read_typed`, `write_typed` |
| Named read result | `NamedReadResult`, `read_named`, `read_named_with_comment_encoding` |
| Polling | `poll`, `poll_with_comment_encoding` |
| Timer/counter composite | `TimerCounterValue`, `read_timer_counter`, `read_timer`, `read_counter` |
| Word reads | `read_words`, `read_words_single_request` |
| Dword reads | `read_dwords`, `read_dwords_single_request` |
| Word writes | `write_words_single_request` |
| Dword writes | `write_dwords_single_request` |

All word/Dword helpers are single-request operations. There are no chunked
exports. `read_named` is the only automatic multi-request read aggregate. It
groups wire-compatible device types by first appearance, sorts each group by
address, merges contiguous ranges up to request limits, keeps multiword values
whole, owns one FIFO turn through final decode/staging, and returns no partial
result. Public result order remains input order; wire order is optimized. Pure
result materialization occurs after releasing the turn. A multi-frame named read is not PLC-atomic;
coherent readers must use one request or a PLC-side snapshot/handshake.
Named keys must be semantically unique by device family, numeric address,
dtype, bit index, and scalar count. Spelling-only variants are rejected before
FIFO admission, while distinct dtype views, bit indices, and overlapping spans
remain valid. Result keys preserve the original input strings.

Numeric semantic `.H` values validate 1..4 hexadecimal digits and return
exactly four uppercase digits (`0000` through `FFFF`); raw reads and write
spelling are not normalized. Timer/counter composite reads require exactly
three semantic tokens. The first is a structural status field validated as the
exact raw PLC token `0` or `1`; `.U`, `.S`, `.H`, `.D`, or `.L` applies only to
the current and preset fields. Consequently, low-level `.H` reads expose status
as `0`/`1`, not the former incorrect `0000`/`0001`, while current and preset are
canonical four-digit uppercase hexadecimal values. Public signatures and
high-level return types are unchanged.
Malformed semantic responses close the connection. Float32 parsing,
formatting, reads, and writes use canonical family metadata and accept only the
ordinary `.U` families `DM`, `EM`, `FM`, `ZF`, `W`, `TM`, `CM`, `VM`,
`D`, `E`, and `F`. Direct-bit, `Z`, and special-response families such as `R`,
`T`, `C`, and `AT` reject `:F` before FIFO admission and transport. Named reads
and polls require a non-empty address set, and poll intervals must be greater
than zero.

Every comment text read requires `HostLinkCommentEncoding::Utf8` or
`HostLinkCommentEncoding::Cp932`. `Cp932` is CP932/Windows-31J and is the
selection for KEYENCE documentation that calls the compatible encoding
`Shift_JIS`; there is no separate strict-Shift-JIS, automatic, default, or
profile-selected mode. Decoding is strict and never retries another codec or
inserts replacement characters. The shared CP932 subset preserves ASCII
controls, rejects standalone `80`/`A0`/`FD`/`FE`/`FF`, and accepts defined NEC,
IBM, and duplicate extension mappings. `read_comment_bytes` returns the exact
terminator-free `RDC` payload, including trailing ASCII-space padding.
UTF-8 decoding preserves an initial `EF BB BF` as `U+FEFF`; it is comment data,
not a removable transport signature. The same bytes are invalid under `Cp932`.
Syntactically valid PLC `E0` through `E9` replies return `HostLinkError::Plc`
without retiring the connection for every semantic command, including writes;
malformed framing or payload decoding retires it.

The ordinary `read_named` and `poll` APIs reject `:COMMENT` entries before
transport. Use `read_named_with_comment_encoding` or
`poll_with_comment_encoding` when an aggregate intentionally includes comments;
both require one explicit comment encoding and at least one `:COMMENT` entry.
Providing an unused comment encoding for a non-comment-only list is a
pre-transport protocol error.
Converting `HostLinkValue` to `u16` is fallible through `TryFrom`; variants
other than `HostLinkValue::U16` return an error instead of producing zero.

## Address and profile APIs

| Purpose | API |
| --- | --- |
| Address models | `HostLinkAddress`, `KvDeviceAddress`, `KvLogicalAddress` |
| Address parsing | `parse_device`, `parse_logical_address` |
| Address validation | `validate_device_type`, `validate_device_count`, `validate_device_span` |
| Expansion validation | `validate_expansion_buffer_count`, `validate_expansion_buffer_span` |
| Profile enumeration | `available_plc_profiles`, `plc_profile_descriptors` |
| Profile selection | `normalize_plc_profile`, `profile_from_name`, `display_name` |
| Range catalog | `device_range_catalog_for_plc_profile`, `KvDeviceRangeCatalog` |

## Errors

`HostLinkError` distinguishes `Protocol`, `Timeout`, `Closed`, `NotConnected`,
`Transport`, `Plc`, and `OutcomeUnknown`. PLC errors retain the returned code
and response; the crate does not embed copied manual error descriptions.
`OutcomeUnknown` has a machine-readable reason for timeout, close, transport
failure, or malformed response. It is used when a state-changing request may
have reached the PLC and the future returns a library `Result`; no automatic
retry occurs.

Dropping a Rust future is caller-observed cancellation and returns no
`HostLinkError`. A waiting drop sends nothing. A drop after transmission may
have started poisons the exchange and retires its socket. TCP then returns
`NotConnected` until an explicit `open`; UDP creates a replacement socket from
the resolved logical endpoint when the next operation begins. The caller must
treat a state-changing outcome as unknown. `HostLinkOutcomeUnknownReason`
therefore has no cancellation variant, and a dropped future is distinct from a
returned library `Timeout`.

The complete generated Rust API for a release is available through docs.rs.

Every `HostLinkClient` owns a FIFO admission queue and one wire turn shared by
its clones. `close` invalidates active and waiting work from the old generation;
only work admitted after an explicit reopen may use the new transport.

## Traffic statistics

`HostLinkClient::traffic_stats().await` returns `HostLinkTrafficStats`.
TCP receive bytes count the body plus the first CR/LF terminator, independent of separator
segmentation; UDP receive bytes count the complete datagram.
