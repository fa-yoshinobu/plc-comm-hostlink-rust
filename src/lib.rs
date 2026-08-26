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
mod plc_profiles;
mod protocol;
mod read_plan;

pub use address::{
    HostLinkAddress, KvDeviceAddress, KvLogicalAddress, normalize_suffix, parse_device,
    parse_logical_address, validate_device_count, validate_device_span, validate_device_type,
    validate_expansion_buffer_count, validate_expansion_buffer_span,
};
pub use client::{HostLinkClient, HostLinkClientFactory, HostLinkPayloadValue, open_and_connect};
pub use device_ranges::{
    KvDeviceRangeCatalog, KvDeviceRangeCategory, KvDeviceRangeEntry, KvDeviceRangeNotation,
    KvDeviceRangeSegment, device_range_catalog_for_plc_profile,
};
pub use error::{HostLinkError, HostLinkOutcomeUnknownReason};
#[allow(deprecated)]
pub use helpers::read_words;
pub use helpers::{
    HostLinkValue, NamedReadResult, TimerCounterValue, poll, poll_with_comment_encoding,
    read_bits_single_request, read_comment_bytes, read_comments, read_counter, read_dwords,
    read_dwords_single_request, read_named, read_named_with_comment_encoding, read_timer,
    read_timer_counter, read_typed, read_words_single_request, write_bit_in_expansion_unit_buffer,
    write_bit_in_word, write_bits_single_request, write_dwords_single_request, write_typed,
    write_words_single_request,
};
pub use model::{
    HostLinkClock, HostLinkCommentEncoding, HostLinkConnectionOptions, HostLinkMonitorWord,
    HostLinkTrafficStats, HostLinkTransportMode, KvModelInfo, KvPlcMode,
};
pub use plc_profiles::{
    KvHostLinkPlcProfile, KvHostLinkPlcProfileDescriptor, available_plc_profiles, display_name,
    normalize_plc_profile, plc_profile_descriptors, profile_from_name,
};
