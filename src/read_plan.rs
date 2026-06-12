use crate::address::{
    KvDeviceAddress, bit_bank_logical_number, is_direct_bit_device_type,
    is_native_32bit_device_type, is_optimizable_read_named_device_type, parse_device,
    parse_named_address_parts, uses_bit_bank_address,
};
use crate::error::HostLinkError;
use crate::helpers::{HostLinkValue, parse_bool_token};
use indexmap::IndexMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadPlanValueKind {
    Unsigned16,
    Signed16,
    Unsigned32,
    Signed32,
    Float32,
    BitInWord,
    DirectBit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadPlanSegmentMode {
    Words,
    DirectBits,
}

#[derive(Debug, Clone)]
pub(crate) struct ReadPlanRequest {
    pub(crate) index: usize,
    pub(crate) address: String,
    pub(crate) base_address: KvDeviceAddress,
    pub(crate) kind: ReadPlanValueKind,
    pub(crate) bit_index: u8,
}

#[derive(Debug, Clone)]
pub(crate) struct ReadPlanSegment {
    pub(crate) start_address: KvDeviceAddress,
    pub(crate) start_number: u32,
    pub(crate) count: usize,
    pub(crate) mode: ReadPlanSegmentMode,
    pub(crate) requests: Vec<ReadPlanRequest>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledReadNamedPlan {
    pub(crate) requests_in_input_order: Vec<ReadPlanRequest>,
    pub(crate) segments: Vec<ReadPlanSegment>,
}

pub(crate) fn compile_read_named_plan(addresses: &[String]) -> Option<CompiledReadNamedPlan> {
    let mut requests_in_input_order = Vec::new();
    let mut requests_by_device_type: IndexMap<String, Vec<ReadPlanRequest>> = IndexMap::new();

    for (index, address) in addresses.iter().enumerate() {
        let request = try_parse_optimizable_read_named_request(address, index)?;
        requests_by_device_type
            .entry(request.base_address.device_type.clone())
            .or_default()
            .push(request.clone());
        requests_in_input_order.push(request);
    }

    let mut segments = Vec::new();
    for bucket in requests_by_device_type.values() {
        let mut sorted = bucket.clone();
        sorted.sort_by_key(|request| {
            (
                read_plan_number(request),
                usize::MAX - get_word_width(request.kind),
            )
        });

        let mut pending = Vec::new();
        let mut current_start: Option<KvDeviceAddress> = None;
        let mut current_start_number = 0u32;
        let mut current_end_exclusive = 0u32;
        let mut current_mode: Option<ReadPlanSegmentMode> = None;

        for request in sorted {
            let request_start = read_plan_number(&request);
            let request_end_exclusive = request_start + get_word_width(request.kind) as u32;
            let request_mode = segment_mode_for_kind(request.kind);
            if current_start.is_none()
                || request_start > current_end_exclusive
                || current_mode != Some(request_mode)
            {
                if let Some(start_address) = current_start.take() {
                    segments.push(ReadPlanSegment {
                        start_address,
                        start_number: current_start_number,
                        count: (current_end_exclusive - current_start_number) as usize,
                        mode: current_mode.unwrap_or(ReadPlanSegmentMode::Words),
                        requests: pending.clone(),
                    });
                    pending.clear();
                }
                current_start = Some(KvDeviceAddress {
                    device_type: request.base_address.device_type.clone(),
                    number: request.base_address.number,
                    suffix: String::new(),
                });
                current_start_number = request_start;
                current_end_exclusive = request_end_exclusive;
                current_mode = Some(request_mode);
            } else if request_end_exclusive > current_end_exclusive {
                current_end_exclusive = request_end_exclusive;
            }
            pending.push(request);
        }

        if let Some(start_address) = current_start {
            segments.push(ReadPlanSegment {
                start_address,
                start_number: current_start_number,
                count: (current_end_exclusive - current_start_number) as usize,
                mode: current_mode.unwrap_or(ReadPlanSegmentMode::Words),
                requests: pending,
            });
        }
    }

    Some(CompiledReadNamedPlan {
        requests_in_input_order,
        segments,
    })
}

fn try_parse_optimizable_read_named_request(
    address: &str,
    index: usize,
) -> Option<ReadPlanRequest> {
    let (base_address, dtype, bit_index) = parse_named_address_parts(address).ok()?;
    let mut base_address = parse_device(&base_address).ok()?;
    if !is_optimizable_read_named_device_type(&base_address.device_type)
        && !is_direct_bit_device_type(&base_address.device_type)
    {
        return None;
    }
    base_address.suffix.clear();

    let (kind, bit_index) =
        if dtype.is_empty() && is_direct_bit_device_type(&base_address.device_type) {
            (ReadPlanValueKind::DirectBit, 0)
        } else if dtype == "BIT_IN_WORD" {
            (ReadPlanValueKind::BitInWord, bit_index.unwrap_or(0))
        } else {
            (try_map_read_plan_value_kind(&dtype)?, 0)
        };
    if is_native_32bit_device_type(&base_address.device_type)
        && matches!(
            kind,
            ReadPlanValueKind::Unsigned32 | ReadPlanValueKind::Signed32
        )
    {
        return None;
    }

    Some(ReadPlanRequest {
        index,
        address: address.to_owned(),
        base_address,
        kind,
        bit_index,
    })
}

fn try_map_read_plan_value_kind(dtype: &str) -> Option<ReadPlanValueKind> {
    match dtype.trim_start_matches('.').to_ascii_uppercase().as_str() {
        "U" => Some(ReadPlanValueKind::Unsigned16),
        "S" => Some(ReadPlanValueKind::Signed16),
        "D" => Some(ReadPlanValueKind::Unsigned32),
        "L" => Some(ReadPlanValueKind::Signed32),
        "F" => Some(ReadPlanValueKind::Float32),
        _ => None,
    }
}

fn segment_mode_for_kind(kind: ReadPlanValueKind) -> ReadPlanSegmentMode {
    when_direct_bit(
        kind,
        ReadPlanSegmentMode::DirectBits,
        ReadPlanSegmentMode::Words,
    )
}

fn when_direct_bit<T>(kind: ReadPlanValueKind, direct: T, other: T) -> T {
    match kind {
        ReadPlanValueKind::DirectBit => direct,
        _ => other,
    }
}

fn get_word_width(kind: ReadPlanValueKind) -> usize {
    match kind {
        ReadPlanValueKind::Unsigned32
        | ReadPlanValueKind::Signed32
        | ReadPlanValueKind::Float32 => 2,
        _ => 1,
    }
}

pub(crate) fn read_plan_number(request: &ReadPlanRequest) -> u32 {
    if request.kind == ReadPlanValueKind::DirectBit
        && uses_bit_bank_address(&request.base_address.device_type)
    {
        bit_bank_logical_number(request.base_address.number)
    } else {
        request.base_address.number
    }
}

pub(crate) fn resolve_planned_value(
    words: &[u16],
    offset: usize,
    kind: ReadPlanValueKind,
    bit_index: u8,
) -> Result<HostLinkValue, HostLinkError> {
    let word = *words
        .get(offset)
        .ok_or_else(|| HostLinkError::protocol("Batched read response was too short"))?;
    let next_word = || {
        words
            .get(offset + 1)
            .copied()
            .ok_or_else(|| HostLinkError::protocol("Batched read response was too short"))
    };

    Ok(match kind {
        ReadPlanValueKind::Unsigned16 => HostLinkValue::U16(word),
        ReadPlanValueKind::Signed16 => HostLinkValue::I16(word as i16),
        ReadPlanValueKind::Unsigned32 => {
            let hi = next_word()? as u32;
            HostLinkValue::U32((word as u32) | (hi << 16))
        }
        ReadPlanValueKind::Signed32 => {
            let hi = next_word()? as u32;
            HostLinkValue::I32(((word as u32) | (hi << 16)) as i32)
        }
        ReadPlanValueKind::Float32 => {
            let hi = next_word()? as u32;
            HostLinkValue::F32(f32::from_bits((word as u32) | (hi << 16)))
        }
        ReadPlanValueKind::BitInWord => HostLinkValue::Bool(((word >> bit_index) & 1) != 0),
        ReadPlanValueKind::DirectBit => {
            return Err(HostLinkError::protocol(
                "Direct bit values must be resolved from bit tokens.",
            ));
        }
    })
}

pub(crate) fn resolve_direct_bit_value(
    tokens: &[String],
    offset: usize,
) -> Result<HostLinkValue, HostLinkError> {
    let token = tokens
        .get(offset)
        .ok_or_else(|| HostLinkError::protocol("Batched direct bit response was too short"))?;
    Ok(HostLinkValue::Bool(parse_bool_token(token)?))
}
