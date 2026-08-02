use crate::address::{
    KvDeviceAddress, bit_bank_logical_number, is_direct_bit_device_type,
    is_native_32bit_device_type, is_optimizable_read_named_device_type, parse_device,
    parse_named_address_parts, uses_bit_bank_address,
};
use crate::error::HostLinkError;
use crate::helpers::{HostLinkValue, parse_bool_token};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadPlanValueKind {
    Unsigned16,
    Signed16,
    Unsigned32,
    Signed32,
    Float32,
    Hex16,
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
    pub(crate) input_count: usize,
    pub(crate) operations: Vec<ReadPlanOperation>,
}

#[derive(Debug, Clone)]
pub(crate) enum ReadPlanOperation {
    Segment(ReadPlanSegment),
    Sequential { index: usize },
}

enum PendingReadPlanGroup {
    Optimized {
        device_type: String,
        mode: ReadPlanSegmentMode,
        requests: Vec<ReadPlanRequest>,
    },
    Sequential {
        index: usize,
    },
}

pub(crate) fn compile_read_named_plan(addresses: &[String]) -> Option<CompiledReadNamedPlan> {
    let mut groups = Vec::new();
    for (index, address) in addresses.iter().enumerate() {
        if let Some(request) = try_parse_optimizable_read_named_request(address, index) {
            let mode = segment_mode_for_kind(request.kind);
            if let Some(requests) = groups.iter_mut().find_map(|group| match group {
                PendingReadPlanGroup::Optimized {
                    device_type,
                    mode: group_mode,
                    requests,
                } if *device_type == request.base_address.device_type && *group_mode == mode => {
                    Some(requests)
                }
                _ => None,
            }) {
                requests.push(request);
            } else {
                groups.push(PendingReadPlanGroup::Optimized {
                    device_type: request.base_address.device_type.clone(),
                    mode,
                    requests: vec![request],
                });
            }
        } else {
            // Fully validated but non-batchable commands (for example COMMENT,
            // native Dword, or direct-bit word views) remain single operations.
            // They no longer disable optimization of unrelated compatible input.
            groups.push(PendingReadPlanGroup::Sequential { index });
        }
    }

    let mut operations = Vec::new();
    for group in groups {
        match group {
            PendingReadPlanGroup::Optimized {
                mode, mut requests, ..
            } => {
                requests.sort_by_key(read_plan_number);
                let mut segments = Vec::new();
                append_group_segments(&mut segments, requests, mode)?;
                operations.extend(segments.into_iter().map(ReadPlanOperation::Segment));
            }
            PendingReadPlanGroup::Sequential { index } => {
                operations.push(ReadPlanOperation::Sequential { index });
            }
        }
    }

    Some(CompiledReadNamedPlan {
        input_count: addresses.len(),
        operations,
    })
}

fn append_group_segments(
    segments: &mut Vec<ReadPlanSegment>,
    requests: Vec<ReadPlanRequest>,
    mode: ReadPlanSegmentMode,
) -> Option<()> {
    let mut pending = Vec::new();
    let mut current_start: Option<KvDeviceAddress> = None;
    let mut current_start_number = 0u32;
    let mut current_end_exclusive = 0u32;

    for request in requests {
        let request_start = read_plan_number(&request);
        let request_end_exclusive =
            request_start.checked_add(get_word_width(request.kind) as u32)?;
        let segment_limit = read_plan_segment_limit(&request.base_address.device_type, mode);
        let exceeds_segment_limit = current_start.is_some()
            && request_end_exclusive - current_start_number > segment_limit as u32;
        let can_append = current_start.is_some()
            && request_start <= current_end_exclusive
            && !exceeds_segment_limit;

        if !can_append {
            if let Some(start_address) = current_start.take() {
                segments.push(ReadPlanSegment {
                    start_address,
                    start_number: current_start_number,
                    count: (current_end_exclusive - current_start_number) as usize,
                    mode,
                    requests: std::mem::take(&mut pending),
                });
            }
            current_start = Some(KvDeviceAddress {
                device_type: request.base_address.device_type.clone(),
                number: request.base_address.number,
                suffix: String::new(),
            });
            current_start_number = request_start;
            current_end_exclusive = request_end_exclusive;
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
            mode,
            requests: pending,
        });
    }
    Some(())
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
        if dtype == "BIT" && is_direct_bit_device_type(&base_address.device_type) {
            (ReadPlanValueKind::DirectBit, 0)
        } else if dtype == "BIT_IN_WORD" {
            (ReadPlanValueKind::BitInWord, bit_index?)
        } else {
            (try_map_read_plan_value_kind(&dtype)?, 0)
        };
    if is_direct_bit_device_type(&base_address.device_type) && kind != ReadPlanValueKind::DirectBit
    {
        // Direct-bit word views return 16/32 tokens per RD. They cannot share the
        // normal word-device RDS plan without shifting the logical values.
        return None;
    }
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
        "H" => Some(ReadPlanValueKind::Hex16),
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

fn read_plan_segment_limit(device_type: &str, mode: ReadPlanSegmentMode) -> usize {
    if mode == ReadPlanSegmentMode::DirectBits {
        return 1000;
    }
    match device_type {
        "TM" => 512,
        "Z" => 12,
        _ => 1000,
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
        ReadPlanValueKind::Hex16 => HostLinkValue::Text(format!("{word:04X}")),
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

#[cfg(test)]
mod tests {
    use super::{
        CompiledReadNamedPlan, ReadPlanOperation, ReadPlanSegment, compile_read_named_plan,
    };

    fn segments(plan: &CompiledReadNamedPlan) -> Vec<&ReadPlanSegment> {
        plan.operations
            .iter()
            .filter_map(|operation| match operation {
                ReadPlanOperation::Segment(segment) => Some(segment),
                ReadPlanOperation::Sequential { .. } => None,
            })
            .collect()
    }

    #[test]
    fn compiled_segments_sort_each_group_without_changing_public_input_order() {
        let addresses = ["DM10:U", "DM9:U", "DM11:U"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let plan = compile_read_named_plan(&addresses).unwrap();

        assert_eq!(
            segments(&plan)
                .iter()
                .map(|segment| (segment.start_number, segment.count))
                .collect::<Vec<_>>(),
            vec![(9, 3)]
        );
        assert_eq!(plan.input_count, 3);
    }

    #[test]
    fn descending_boundary_starts_a_new_segment_that_can_grow_forward() {
        let addresses = ["DM100:U", "DM0:U", "DM1:U"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let plan = compile_read_named_plan(&addresses).unwrap();

        assert_eq!(
            segments(&plan)
                .iter()
                .map(|segment| (segment.start_number, segment.count))
                .collect::<Vec<_>>(),
            vec![(0, 2), (100, 1)]
        );
    }

    #[test]
    fn repeated_descending_input_is_merged_after_address_sort() {
        let addresses = ["DM100:U", "DM0:U", "DM1:U", "DM50:U", "DM2:U", "DM3:U"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let plan = compile_read_named_plan(&addresses).unwrap();

        assert_eq!(
            segments(&plan)
                .iter()
                .map(|segment| (segment.start_number, segment.count))
                .collect::<Vec<_>>(),
            vec![(0, 4), (50, 1), (100, 1)]
        );
    }

    #[test]
    fn alternating_device_types_use_first_group_appearance_and_minimum_segments() {
        let addresses = ["DM10:U", "MR0:BIT", "DM11:S", "MR1:BIT"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let plan = compile_read_named_plan(&addresses).unwrap();

        let segments = segments(&plan);
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].start_address.device_type, "DM");
        assert_eq!((segments[0].start_number, segments[0].count), (10, 2));
        assert_eq!(segments[1].start_address.device_type, "MR");
        assert_eq!((segments[1].start_number, segments[1].count), (0, 2));
    }

    #[test]
    fn multiword_value_moves_whole_to_next_segment_at_limit() {
        let mut addresses = (0..999)
            .map(|number| format!("DM{number}:U"))
            .collect::<Vec<_>>();
        addresses.push("DM999:D".to_owned());

        let plan = compile_read_named_plan(&addresses).unwrap();
        let segments = segments(&plan);
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[0].start_number, segments[0].count), (0, 999));
        assert_eq!((segments[1].start_number, segments[1].count), (999, 2));
        assert_eq!(segments[1].requests.len(), 1);
        assert_eq!(segments[1].requests[0].index, 999);
    }
}
