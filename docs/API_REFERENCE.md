# KV Host Link Rust API reference

This page indexes the supported user-facing surface. Maintainer raw frame and
trace facilities are intentionally omitted from ordinary user documentation.

## Connection and lifecycle

| Purpose | API |
| --- | --- |
| Validated options | `HostLinkConnectionOptions::new(host, port, transport, plc_profile)` |
| Disconnected client | `HostLinkClient::new` |
| Connected direct client | `HostLinkClient::connect` |
| Connected queued client | `open_and_connect`, `HostLinkClientFactory::open_and_connect` |
| Lifecycle | `open`, `close`, `is_open` |
| Session values | `timeout`, `set_timeout`, `plc_profile` |
| Transport selection | `HostLinkTransportMode::{Tcp, Udp}` |

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
| Comments | `read_comments` |

Numeric low-level methods require a base device plus an explicit format.
Direct bit methods use an unsuffixed device. Suffix-bearing low-level device
strings are rejected.

`HostLinkClock.year` is the explicit two-digit PLC year and must be `0..=99`.
Semantic reads validate command-derived response counts. Direct-bit responses
accept only `0`, `1`, `OFF`, or `ON`, while numeric reads of direct-bit devices require 16 or
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
| Named snapshot | `NamedSnapshot`, `read_named` |
| Polling | `poll` |
| Timer/counter composite | `TimerCounterValue`, `read_timer_counter`, `read_timer`, `read_counter` |
| Word reads | `read_words`, `read_words_single_request` |
| Dword reads | `read_dwords`, `read_dwords_single_request` |
| Word writes | `write_words_single_request` |
| Dword writes | `write_dwords_single_request` |
| Bit in word | `write_bit_in_word` |

All word/Dword helpers are single-request operations. There are no chunked
exports.

Hexadecimal typed reads require exactly one token containing 1..4 hexadecimal
digits (timer/counter composite reads require their exact three-token shape).
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

`HostLinkError` distinguishes protocol validation, `NotConnected`, transport
connection failure, and PLC rejection. PLC errors retain the returned code and
response; the crate does not embed copied manual error descriptions.

The complete generated Rust API for a release is available through docs.rs.

## Traffic statistics

`HostLinkClient::traffic_stats().await` and the queued equivalent return `HostLinkTrafficStats`.
TCP receive bytes count the body plus the first CR/LF terminator, independent of separator
segmentation; UDP receive bytes count the complete datagram.
