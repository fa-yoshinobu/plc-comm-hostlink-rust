//! Async Rust client for KEYENCE KV Host Link.
//!
//! The crate keeps the same high-level helper semantics as the sibling
//! `.NET` and Python libraries in this workspace.

mod address;
mod client;
mod device_ranges;
mod error;
mod helpers;
mod model;
mod protocol;

pub use address::{
    HostLinkAddress, KvDeviceAddress, KvLogicalAddress, normalize_suffix, parse_device,
    parse_logical_address, resolve_effective_format, validate_device_count, validate_device_span,
    validate_device_type, validate_expansion_buffer_count, validate_expansion_buffer_span,
};
pub use client::{
    HostLinkClient, HostLinkClientFactory, HostLinkPayloadValue, QueuedHostLinkClient,
    open_and_connect,
};
pub use device_ranges::{
    KvDeviceRangeCatalog, KvDeviceRangeCategory, KvDeviceRangeEntry, KvDeviceRangeNotation,
    KvDeviceRangeSegment, available_device_range_models, device_range_catalog_for_model,
};
pub use error::{HostLinkError, decode_error_code};
pub use helpers::{
    HostLinkValue, NamedSnapshot, poll, read_comments, read_dwords, read_dwords_chunked,
    read_dwords_single_request, read_named, read_typed, read_words, read_words_chunked,
    read_words_single_request, write_bit_in_word, write_dwords_chunked,
    write_dwords_single_request, write_typed, write_words_chunked, write_words_single_request,
};
pub use model::{
    HostLinkClock, HostLinkConnectionOptions, HostLinkTraceDirection, HostLinkTraceFrame,
    HostLinkTransportMode, KvModelInfo, KvPlcMode, TraceHook,
};
