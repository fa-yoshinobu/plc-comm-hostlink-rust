# KV Host Link Rust API Reference

This page is a user-facing index of the public Rust KV Host Link API surface.
Use the usage guide for examples, and this page when you need to find the
operation name for a specific Host Link workflow.

The main async client type is `HostLinkClient`. For shared application
sessions, use `QueuedHostLinkClient` or `open_and_connect`.

## Connection And PLC Control

| Operation | Public API |
| --- | --- |
| Open a ready-to-use queued connection | `open_and_connect`, `HostLinkClientFactory::open_and_connect`, `HostLinkConnectionOptions` |
| Low-level client | `HostLinkClient`, `HostLinkClient::new`, `HostLinkClient::connect` |
| Queued shared client | `QueuedHostLinkClient`, `QueuedHostLinkClient::new`, `execute_async`, `inner_client` |
| Raw command exchange | `send_raw` |
| PLC mode and error control | `change_mode`, `clear_error`, `check_error_no`, `confirm_operating_mode` |
| PLC model and clock | `query_model`, `set_time`, `KvModelInfo`, `HostLinkClock` |
| Connection lifecycle | `open`, `close`, `is_open` |
| Session settings | `timeout`, `set_timeout`, `plc_profile`, `append_lf_on_send`, `set_append_lf_on_send`, `set_trace_hook` |
| Trace capture | `HostLinkTraceDirection`, `HostLinkTraceFrame`, `TraceHook` |

## Device Operations

| Operation | Public API |
| --- | --- |
| Single device read/write | `read`, `write` |
| Consecutive device read/write | `read_consecutive`, `write_consecutive` |
| Legacy consecutive read/write | `read_consecutive_legacy`, `write_consecutive_legacy` |
| Forced bit/device control | `forced_set`, `forced_reset`, `forced_set_consecutive`, `forced_reset_consecutive` |
| Timer/counter set-value writes | `write_set_value`, `write_set_value_consecutive` |
| Monitor registration/cycle | `register_monitor_bits`, `register_monitor_words`, `read_monitor_bits`, `read_monitor_words` |
| Comment reads | `read_comments` |
| Data bank switching | `switch_bank` |
| Expansion unit buffer access | `read_expansion_unit_buffer`, `write_expansion_unit_buffer` |

## High-Level Helpers

| Operation | Public API |
| --- | --- |
| Address parsing and formatting | `HostLinkAddress`, `KvDeviceAddress`, `KvLogicalAddress`, `parse_device`, `parse_logical_address`, `normalize_suffix` |
| Address validation | `validate_device_type`, `validate_device_count`, `validate_device_span`, `validate_expansion_buffer_count`, `validate_expansion_buffer_span` |
| Typed values | `HostLinkValue`, `read_typed`, `write_typed` |
| Timer/counter composite reads | `TimerCounterValue`, `read_timer_counter`, `read_timer`, `read_counter` |
| Named snapshots and polling | `NamedSnapshot`, `read_named`, `poll` |
| Word/dword reads | `read_words`, `read_dwords` |
| Single-request reads/writes | `read_words_single_request`, `read_dwords_single_request`, `write_words_single_request`, `write_dwords_single_request` |
| Explicit chunked reads/writes | `read_words_chunked`, `read_dwords_chunked`, `write_words_chunked`, `write_dwords_chunked` |
| Bit-in-word write | `write_bit_in_word` |

## Address, Profile, And Diagnostics

| Operation | Public API |
| --- | --- |
| Device range catalog | `KvDeviceRangeCatalog`, `KvDeviceRangeEntry`, `KvDeviceRangeSegment`, `KvDeviceRangeCategory`, `KvDeviceRangeNotation` |
| Profile lookup | `available_plc_profiles`, `device_range_catalog_for_plc_profile`, `display_name` |
| Transport and mode enums | `HostLinkTransportMode`, `KvPlcMode` |
| Value formatting | `HostLinkPayloadValue` |
| Error handling | `HostLinkError`, `decode_error_code` |

## Public Symbol Index

The crate re-exports these public names from `src/lib.rs`:

`HostLinkAddress`, `KvDeviceAddress`, `KvLogicalAddress`,
`normalize_suffix`, `parse_device`, `parse_logical_address`,
`validate_device_count`, `validate_device_span`, `validate_device_type`,
`validate_expansion_buffer_count`, `validate_expansion_buffer_span`,
`HostLinkClient`, `HostLinkClientFactory`, `HostLinkPayloadValue`,
`QueuedHostLinkClient`, `open_and_connect`, `KvDeviceRangeCatalog`,
`KvDeviceRangeCategory`, `KvDeviceRangeEntry`, `KvDeviceRangeNotation`,
`KvDeviceRangeSegment`, `available_plc_profiles`,
`device_range_catalog_for_plc_profile`, `display_name`, `HostLinkError`,
`decode_error_code`, `HostLinkValue`, `NamedSnapshot`, `TimerCounterValue`,
`poll`, `read_comments`, `read_counter`, `read_dwords`,
`read_dwords_chunked`, `read_dwords_single_request`, `read_named`,
`read_timer`, `read_timer_counter`, `read_typed`, `read_words`,
`read_words_chunked`, `read_words_single_request`, `write_bit_in_word`,
`write_dwords_chunked`, `write_dwords_single_request`, `write_typed`,
`write_words_chunked`, `write_words_single_request`, `HostLinkClock`,
`HostLinkConnectionOptions`, `HostLinkTraceDirection`,
`HostLinkTraceFrame`, `HostLinkTransportMode`, `KvModelInfo`, `KvPlcMode`,
`TraceHook`.

## Generated API Details

The crate-level generated API is published by docs.rs for the released crate.
This page keeps the same user-facing operation index in the repository and on
the shared docs site.
