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
IPv4 result fail as connection errors before protocol communication.
Request and response bodies have an internal 65,536-byte cap. There is no
caller-controlled receive capacity; public results own their dynamic storage.

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

`HostLinkClock.year` is the explicit two-digit PLC year and must be `0..=99`.
Semantic reads validate command-derived response counts. Direct-bit responses
accept only the exact tokens `0`, `1`, `OFF`, or `ON` without trimming or case
folding, while numeric reads of direct-bit devices require 16 or
32 response points according to the explicit format. Malformed semantic
responses close the connection generation.
UDP responses require a CR/LF terminator; missing framing closes the transport.
All non-format commands, including forced control, monitor-bit registration,
comment reads, and timer/counter helpers, reject suffix-bearing devices.
Monitor reads require a successful registration in the current connection
generation and enforce the exact registered token count.

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
exports. `read_named` is the only automatic multi-request read aggregate: it
preserves input wire order, keeps multiword values whole, owns one FIFO turn,
and returns no partial result. A multi-frame named read is not PLC-atomic;
coherent readers must use one request or a PLC-side snapshot/handshake.
Named keys must be semantically unique by device family, numeric address,
dtype, bit index, and scalar count. Spelling-only variants are rejected before
FIFO admission, while distinct dtype views, bit indices, and overlapping spans
remain valid. Result keys preserve the original input strings.

Hexadecimal typed reads require exactly one token containing 1..4 hexadecimal
digits. Timer/counter composite reads require exactly three semantic tokens,
status `0` or `1`, and valid current and preset fields for the requested type.
Malformed semantic responses close the connection. Float32 parsing,
formatting, reads, and writes use canonical family metadata and accept only the
ordinary `.U` families `DM`, `EM`, `FM`, `ZF`, `W`, `TM`, `Z`, `CM`, `VM`,
`D`, `E`, and `F`. Direct-bit and special-response families such as `R`, `T`,
`C`, and `AT` reject `:F` before FIFO admission and transport. Named reads and
polls require a non-empty address set, and poll intervals must be greater than
zero.

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
without retiring the connection; malformed framing or payload decoding retires
it.

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
have started poisons and retires the transport, so the next operation returns
`NotConnected` until an explicit `open`; the caller must treat a state-changing
outcome as unknown. `HostLinkOutcomeUnknownReason` therefore has no cancellation
variant, and a dropped future is distinct from a returned library `Timeout`.

The complete generated Rust API for a release is available through docs.rs.

Every `HostLinkClient` owns a FIFO admission queue and one wire turn shared by
its clones. `close` invalidates active and waiting work from the old generation;
only work admitted after an explicit reopen may use the new transport.

## Traffic statistics

`HostLinkClient::traffic_stats().await` returns `HostLinkTrafficStats`.
TCP receive bytes count the body plus the first CR/LF terminator, independent of separator
segmentation; UDP receive bytes count the complete datagram.
