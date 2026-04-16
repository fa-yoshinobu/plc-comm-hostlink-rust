use crate::address::{
    KvDeviceAddress, is_direct_bit_device_type, is_optimizable_read_named_device_type,
    offset_device, parse_device, parse_logical_address, parse_named_address_parts,
    resolve_effective_format, validate_device_count, validate_device_span,
};
use crate::client::{HostLinkClient, HostLinkPayloadValue};
use crate::error::HostLinkError;
use futures_core::Stream;
use indexmap::IndexMap;
use std::str::FromStr;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub enum HostLinkValue {
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
    Text(String),
}

pub type NamedSnapshot = IndexMap<String, HostLinkValue>;

#[derive(Debug, Clone, Copy)]
enum ReadPlanValueKind {
    Unsigned16,
    Signed16,
    Unsigned32,
    Signed32,
    Float32,
    BitInWord,
    DirectBit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadPlanSegmentMode {
    Words,
    DirectBits,
}

#[derive(Debug, Clone)]
struct ReadPlanRequest {
    index: usize,
    address: String,
    base_address: KvDeviceAddress,
    kind: ReadPlanValueKind,
    bit_index: u8,
}

#[derive(Debug, Clone)]
struct ReadPlanSegment {
    start_address: KvDeviceAddress,
    start_number: u32,
    count: usize,
    mode: ReadPlanSegmentMode,
    requests: Vec<ReadPlanRequest>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledReadNamedPlan {
    requests_in_input_order: Vec<ReadPlanRequest>,
    segments: Vec<ReadPlanSegment>,
}

impl From<HostLinkValue> for u16 {
    fn from(value: HostLinkValue) -> Self {
        match value {
            HostLinkValue::U16(value) => value,
            _ => 0,
        }
    }
}

impl HostLinkPayloadValue for HostLinkValue {
    fn format_for_suffix(&self, data_format: &str) -> String {
        let mut value = String::new();
        self.append_to_payload(data_format, &mut value);
        value
    }

    fn append_to_payload(&self, data_format: &str, output: &mut String) {
        match self {
            HostLinkValue::U16(value) => value.append_to_payload(data_format, output),
            HostLinkValue::I16(value) => value.append_to_payload(data_format, output),
            HostLinkValue::U32(value) => value.append_to_payload(data_format, output),
            HostLinkValue::I32(value) => value.append_to_payload(data_format, output),
            HostLinkValue::F32(value) => value.append_to_payload(data_format, output),
            HostLinkValue::Bool(value) => value.append_to_payload(data_format, output),
            HostLinkValue::Text(value) => value.append_to_payload(data_format, output),
        }
    }
}

pub async fn read_comments(
    client: &HostLinkClient,
    device: &str,
    strip_padding: bool,
) -> Result<String, HostLinkError> {
    client.read_comments(device, strip_padding).await
}

pub async fn read_typed(
    client: &HostLinkClient,
    device: &str,
    dtype: &str,
) -> Result<HostLinkValue, HostLinkError> {
    let (device, dtype) = if dtype.trim().is_empty() {
        let logical = parse_logical_address(device)?;
        (logical.base_address.to_text()?, logical.data_type)
    } else {
        (
            device.trim().to_ascii_uppercase(),
            dtype.trim_start_matches('.').to_ascii_uppercase(),
        )
    };

    match dtype.as_str() {
        "F" => {
            let words = read_words(client, &device, 2).await?;
            let bits = (words[0] as u32) | ((words[1] as u32) << 16);
            Ok(HostLinkValue::F32(f32::from_bits(bits)))
        }
        "S" => Ok(HostLinkValue::I16(
            read_single_parsed(client, &device, Some("S"), "Invalid signed 16-bit response")
                .await?,
        )),
        "D" => Ok(HostLinkValue::U32(
            read_single_parsed::<u32>(
                client,
                &device,
                Some("D"),
                "Invalid unsigned 32-bit response",
            )
            .await?,
        )),
        "L" => Ok(HostLinkValue::I32(
            read_single_parsed(client, &device, Some("L"), "Invalid signed 32-bit response")
                .await?,
        )),
        "U" => Ok(HostLinkValue::U16(
            read_single_parsed::<u16>(
                client,
                &device,
                Some("U"),
                "Invalid unsigned 16-bit response",
            )
            .await?,
        )),
        "" => Ok(HostLinkValue::Bool(
            read_single_bool(client, &device, None).await?,
        )),
        other => Err(HostLinkError::protocol(format!(
            "Unsupported logical data type '{other}'."
        ))),
    }
}

pub async fn write_typed<T: HostLinkPayloadValue>(
    client: &HostLinkClient,
    device: &str,
    dtype: &str,
    value: &T,
) -> Result<(), HostLinkError> {
    match dtype.trim_start_matches('.').to_ascii_uppercase().as_str() {
        "F" => {
            let single = value
                .format_for_suffix("")
                .parse::<f32>()
                .map_err(|_| HostLinkError::protocol("Invalid float32 input"))?;
            let bits = single.to_bits();
            let words = [(bits & 0xFFFF) as u16, (bits >> 16) as u16];
            client.write_consecutive(device, &words, Some("U")).await
        }
        "" => client.write(device, value, None).await,
        "S" | "D" | "L" | "U" => client.write(device, value, Some(dtype)).await,
        other => Err(HostLinkError::protocol(format!(
            "Unsupported logical data type '{other}'."
        ))),
    }
}

fn parse_bool_token(token: &str) -> Result<bool, HostLinkError> {
    let token = token.trim();
    if token == "1" || token.eq_ignore_ascii_case("ON") || token.eq_ignore_ascii_case("TRUE") {
        Ok(true)
    } else if token == "0"
        || token.eq_ignore_ascii_case("OFF")
        || token.eq_ignore_ascii_case("FALSE")
    {
        Ok(false)
    } else {
        Err(HostLinkError::protocol(format!(
            "Invalid direct bit response token: {token}"
        )))
    }
}

fn first_response_token(response_text: &str) -> Result<&str, HostLinkError> {
    response_text
        .split(' ')
        .find(|token| !token.is_empty())
        .ok_or_else(|| HostLinkError::protocol("Missing response token"))
}

fn parse_first_token<T: FromStr>(
    response_text: &str,
    invalid_message: &'static str,
) -> Result<T, HostLinkError> {
    first_response_token(response_text)?
        .parse::<T>()
        .map_err(|_| HostLinkError::protocol(invalid_message))
}

fn parse_all_tokens<T: FromStr>(
    response_text: &str,
    invalid_message: &'static str,
) -> Result<Vec<T>, HostLinkError> {
    let mut values = Vec::new();
    for token in response_text.split(' ').filter(|token| !token.is_empty()) {
        values.push(
            token
                .parse::<T>()
                .map_err(|_| HostLinkError::protocol(invalid_message))?,
        );
    }
    if values.is_empty() {
        return Err(HostLinkError::protocol("Missing response token"));
    }
    Ok(values)
}

fn prepare_read_address(
    device: &str,
    data_format: Option<&str>,
    count: usize,
) -> Result<KvDeviceAddress, HostLinkError> {
    let mut address = parse_device(device)?;
    let suffix = if let Some(data_format) = data_format {
        crate::address::normalize_suffix(data_format)?
    } else {
        address.suffix.clone()
    };
    let suffix = resolve_effective_format(&address.device_type, &suffix);
    if count > 1 {
        validate_device_count(&address.device_type, &suffix, count)?;
    }
    validate_device_span(&address.device_type, address.number, &suffix, count)?;
    address.suffix = suffix;
    Ok(address)
}

async fn read_single_response(
    client: &HostLinkClient,
    device: &str,
    data_format: Option<&str>,
) -> Result<String, HostLinkError> {
    let address = prepare_read_address(device, data_format, 1)?;
    client.send_raw(&format!("RD {}", address.to_text()?)).await
}

async fn read_single_parsed<T: FromStr>(
    client: &HostLinkClient,
    device: &str,
    data_format: Option<&str>,
    invalid_message: &'static str,
) -> Result<T, HostLinkError> {
    let response = read_single_response(client, device, data_format).await?;
    parse_first_token(&response, invalid_message)
}

async fn read_single_bool(
    client: &HostLinkClient,
    device: &str,
    data_format: Option<&str>,
) -> Result<bool, HostLinkError> {
    let response = read_single_response(client, device, data_format).await?;
    parse_bool_token(first_response_token(&response)?)
}

async fn read_consecutive_parsed<T: FromStr>(
    client: &HostLinkClient,
    device: &str,
    count: usize,
    data_format: Option<&str>,
    invalid_message: &'static str,
) -> Result<Vec<T>, HostLinkError> {
    let address = prepare_read_address(device, data_format, count)?;
    let response = client
        .send_raw(&format!("RDS {} {}", address.to_text()?, count))
        .await?;
    parse_all_tokens(&response, invalid_message)
}

pub async fn write_bit_in_word(
    client: &HostLinkClient,
    device: &str,
    bit_index: u8,
    value: bool,
) -> Result<(), HostLinkError> {
    if bit_index > 15 {
        return Err(HostLinkError::protocol("bitIndex must be 0-15."));
    }

    let mut current = read_single_parsed::<u16>(
        client,
        device,
        Some("U"),
        "Invalid unsigned 16-bit response",
    )
    .await?;
    if value {
        current |= 1 << bit_index;
    } else {
        current &= !(1 << bit_index);
    }
    client.write(device, current, Some("U")).await
}

pub async fn read_named<S: AsRef<str>>(
    client: &HostLinkClient,
    addresses: &[S],
) -> Result<NamedSnapshot, HostLinkError> {
    let addr_list = addresses
        .iter()
        .map(|item| item.as_ref().to_owned())
        .collect::<Vec<_>>();
    if addr_list.is_empty() {
        return Ok(NamedSnapshot::new());
    }

    if let Some(plan) = compile_read_named_plan(&addr_list) {
        execute_read_named_plan(client, &plan).await
    } else {
        read_named_sequential(client, &addr_list).await
    }
}

pub(crate) async fn read_named_sequential(
    client: &HostLinkClient,
    addresses: &[String],
) -> Result<NamedSnapshot, HostLinkError> {
    let mut result = NamedSnapshot::new();
    for address in addresses {
        let (base_address, dtype, bit_index) = parse_named_address_parts(address)?;
        if dtype == "BIT_IN_WORD" {
            let word = read_single_parsed::<u16>(
                client,
                &base_address,
                Some("U"),
                "Invalid unsigned 16-bit response",
            )
            .await?;
            let bit_index = bit_index.unwrap_or(0);
            result.insert(
                address.clone(),
                HostLinkValue::Bool(((word >> bit_index) & 1) != 0),
            );
        } else if dtype == "COMMENT" {
            result.insert(
                address.clone(),
                HostLinkValue::Text(read_comments(client, &base_address, true).await?),
            );
        } else {
            result.insert(
                address.clone(),
                read_typed(client, &base_address, &dtype).await?,
            );
        }
    }
    Ok(result)
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
                request.base_address.number,
                usize::MAX - get_word_width(request.kind),
            )
        });

        let mut pending = Vec::new();
        let mut current_start: Option<KvDeviceAddress> = None;
        let mut current_start_number = 0u32;
        let mut current_end_exclusive = 0u32;
        let mut current_mode: Option<ReadPlanSegmentMode> = None;

        for request in sorted {
            let request_start = request.base_address.number;
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

pub(crate) async fn execute_read_named_plan(
    client: &HostLinkClient,
    plan: &CompiledReadNamedPlan,
) -> Result<NamedSnapshot, HostLinkError> {
    let mut resolved = vec![HostLinkValue::U16(0); plan.requests_in_input_order.len()];
    for segment in &plan.segments {
        match segment.mode {
            ReadPlanSegmentMode::Words => {
                let words =
                    read_words(client, &segment.start_address.to_text()?, segment.count).await?;
                for request in &segment.requests {
                    let offset = (request.base_address.number - segment.start_number) as usize;
                    resolved[request.index] =
                        resolve_planned_value(&words, offset, request.kind, request.bit_index)?;
                }
            }
            ReadPlanSegmentMode::DirectBits => {
                let tokens = client
                    .read_consecutive(&segment.start_address.to_text()?, segment.count, None)
                    .await?;
                for request in &segment.requests {
                    let offset = (request.base_address.number - segment.start_number) as usize;
                    resolved[request.index] = resolve_direct_bit_value(&tokens, offset)?;
                }
            }
        }
    }

    let mut result = NamedSnapshot::new();
    for request in &plan.requests_in_input_order {
        result.insert(request.address.clone(), resolved[request.index].clone());
    }
    Ok(result)
}

pub fn poll<'a, S: AsRef<str> + 'a>(
    client: &'a HostLinkClient,
    addresses: &'a [S],
    interval: Duration,
) -> impl Stream<Item = Result<NamedSnapshot, HostLinkError>> + 'a {
    async_stream::try_stream! {
        let addr_list = addresses.iter().map(|item| item.as_ref().to_owned()).collect::<Vec<_>>();
        let compiled = compile_read_named_plan(&addr_list);
        loop {
            let snapshot = if let Some(plan) = &compiled {
                execute_read_named_plan(client, plan).await?
            } else {
                read_named_sequential(client, &addr_list).await?
            };
            yield snapshot;
            tokio::time::sleep(interval).await;
        }
    }
}

pub async fn read_words(
    client: &HostLinkClient,
    device: &str,
    count: usize,
) -> Result<Vec<u16>, HostLinkError> {
    read_words_single_request(client, device, count).await
}

pub async fn read_dwords(
    client: &HostLinkClient,
    device: &str,
    count: usize,
) -> Result<Vec<u32>, HostLinkError> {
    read_dwords_single_request(client, device, count).await
}

pub async fn read_words_single_request(
    client: &HostLinkClient,
    device: &str,
    count: usize,
) -> Result<Vec<u16>, HostLinkError> {
    if count == 0 {
        return Err(HostLinkError::protocol("count must be 1 or greater."));
    }
    read_consecutive_parsed::<u16>(
        client,
        device,
        count,
        Some("U"),
        "Invalid unsigned 16-bit response",
    )
    .await
}

pub async fn read_dwords_single_request(
    client: &HostLinkClient,
    device: &str,
    count: usize,
) -> Result<Vec<u32>, HostLinkError> {
    if count == 0 {
        return Err(HostLinkError::protocol("count must be 1 or greater."));
    }
    let words = read_words_single_request(client, device, count * 2).await?;
    let mut result = Vec::with_capacity(count);
    for index in 0..count {
        let lo = words[index * 2] as u32;
        let hi = words[(index * 2) + 1] as u32;
        result.push(lo | (hi << 16));
    }
    Ok(result)
}

pub async fn write_words_single_request(
    client: &HostLinkClient,
    device: &str,
    values: &[u16],
) -> Result<(), HostLinkError> {
    if values.is_empty() {
        return Err(HostLinkError::protocol("values must not be empty"));
    }
    client.write_consecutive(device, values, Some("U")).await
}

pub async fn write_dwords_single_request(
    client: &HostLinkClient,
    device: &str,
    values: &[u32],
) -> Result<(), HostLinkError> {
    if values.is_empty() {
        return Err(HostLinkError::protocol("values must not be empty"));
    }
    let mut words = Vec::with_capacity(values.len() * 2);
    for value in values {
        words.push((value & 0xFFFF) as u16);
        words.push((value >> 16) as u16);
    }
    write_words_single_request(client, device, &words).await
}

pub async fn read_words_chunked(
    client: &HostLinkClient,
    device: &str,
    count: usize,
    max_words_per_request: usize,
) -> Result<Vec<u16>, HostLinkError> {
    validate_chunk_arguments(count, max_words_per_request, "count", "maxWordsPerRequest")?;
    let mut start = parse_device(device)?;
    start.suffix.clear();
    let mut result = vec![0u16; count];
    let mut offset = 0usize;
    while offset < count {
        let chunk_count = max_words_per_request.min(count - offset);
        let chunk_start = offset_device(&start, offset as u32)?;
        let chunk = read_words_single_request(client, &chunk_start, chunk_count).await?;
        result[offset..offset + chunk_count].copy_from_slice(&chunk);
        offset += chunk_count;
    }
    Ok(result)
}

pub async fn read_dwords_chunked(
    client: &HostLinkClient,
    device: &str,
    count: usize,
    max_dwords_per_request: usize,
) -> Result<Vec<u32>, HostLinkError> {
    validate_chunk_arguments(
        count,
        max_dwords_per_request,
        "count",
        "maxDwordsPerRequest",
    )?;
    let mut start = parse_device(device)?;
    start.suffix.clear();
    let mut result = vec![0u32; count];
    let mut offset = 0usize;
    while offset < count {
        let chunk_count = max_dwords_per_request.min(count - offset);
        let chunk_start = offset_device(&start, (offset * 2) as u32)?;
        let chunk = read_dwords_single_request(client, &chunk_start, chunk_count).await?;
        result[offset..offset + chunk_count].copy_from_slice(&chunk);
        offset += chunk_count;
    }
    Ok(result)
}

pub async fn write_words_chunked(
    client: &HostLinkClient,
    device: &str,
    values: &[u16],
    max_words_per_request: usize,
) -> Result<(), HostLinkError> {
    if values.is_empty() {
        return Err(HostLinkError::protocol("values must not be empty"));
    }
    validate_chunk_size(max_words_per_request, "maxWordsPerRequest")?;
    let mut start = parse_device(device)?;
    start.suffix.clear();
    let mut offset = 0usize;
    while offset < values.len() {
        let chunk_count = max_words_per_request.min(values.len() - offset);
        let chunk_start = offset_device(&start, offset as u32)?;
        write_words_single_request(client, &chunk_start, &values[offset..offset + chunk_count])
            .await?;
        offset += chunk_count;
    }
    Ok(())
}

pub async fn write_dwords_chunked(
    client: &HostLinkClient,
    device: &str,
    values: &[u32],
    max_dwords_per_request: usize,
) -> Result<(), HostLinkError> {
    if values.is_empty() {
        return Err(HostLinkError::protocol("values must not be empty"));
    }
    validate_chunk_size(max_dwords_per_request, "maxDwordsPerRequest")?;
    let mut start = parse_device(device)?;
    start.suffix.clear();
    let mut offset = 0usize;
    while offset < values.len() {
        let chunk_count = max_dwords_per_request.min(values.len() - offset);
        let chunk_start = offset_device(&start, (offset * 2) as u32)?;
        write_dwords_single_request(client, &chunk_start, &values[offset..offset + chunk_count])
            .await?;
        offset += chunk_count;
    }
    Ok(())
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

fn resolve_planned_value(
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

fn resolve_direct_bit_value(
    tokens: &[String],
    offset: usize,
) -> Result<HostLinkValue, HostLinkError> {
    let token = tokens
        .get(offset)
        .ok_or_else(|| HostLinkError::protocol("Batched direct bit response was too short"))?;
    Ok(HostLinkValue::Bool(parse_bool_token(token)?))
}

fn validate_chunk_arguments(
    count: usize,
    max_per_request: usize,
    count_name: &str,
    chunk_name: &str,
) -> Result<(), HostLinkError> {
    if count == 0 {
        return Err(HostLinkError::protocol(format!(
            "{count_name} must be 1 or greater."
        )));
    }
    validate_chunk_size(max_per_request, chunk_name)
}

fn validate_chunk_size(max_per_request: usize, param_name: &str) -> Result<(), HostLinkError> {
    if max_per_request == 0 {
        return Err(HostLinkError::protocol(format!(
            "{param_name} must be 1 or greater."
        )));
    }
    Ok(())
}
