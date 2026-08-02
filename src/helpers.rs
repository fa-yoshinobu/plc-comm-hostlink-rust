use crate::address::{
    KvDeviceAddress, is_direct_bit_device_type, parse_device, parse_named_address_parts,
    rdc_device_types, require_explicit_format, require_float32_eligible_device_type,
    require_no_suffix, validate_device_count, validate_device_span, validate_device_type,
};
use crate::client::{HostLinkClient, HostLinkPayloadValue};
use crate::error::HostLinkError;
use crate::model::HostLinkCommentEncoding;
use crate::read_plan::{
    CompiledReadNamedPlan, ReadPlanOperation, ReadPlanSegmentMode, compile_read_named_plan,
    read_plan_number, resolve_direct_bit_value, resolve_planned_value,
};
use futures_core::Stream;
use indexmap::IndexMap;
use std::collections::HashSet;
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

/// Ordered, all-or-error values returned by one named-read aggregate.
///
/// The aggregate may use more than one PLC request, so this type does not
/// represent or guarantee one PLC-atomic snapshot.
pub type NamedReadResult = IndexMap<String, HostLinkValue>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerCounterValue {
    pub status: u32,
    pub current: u32,
    pub preset: u32,
}

impl TryFrom<HostLinkValue> for u16 {
    type Error = HostLinkError;

    fn try_from(value: HostLinkValue) -> Result<Self, Self::Error> {
        match value {
            HostLinkValue::U16(value) => Ok(value),
            _ => Err(HostLinkError::protocol(
                "HostLinkValue variant cannot be converted to u16 without loss",
            )),
        }
    }
}

impl HostLinkPayloadValue for HostLinkValue {
    fn as_bool(&self) -> Option<bool> {
        match self {
            HostLinkValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    fn as_integer(&self) -> Option<i128> {
        match self {
            HostLinkValue::U16(value) => Some(i128::from(*value)),
            HostLinkValue::I16(value) => Some(i128::from(*value)),
            HostLinkValue::U32(value) => Some(i128::from(*value)),
            HostLinkValue::I32(value) => Some(i128::from(*value)),
            _ => None,
        }
    }

    fn format_for_suffix(&self, data_format: &str) -> Result<String, HostLinkError> {
        match self {
            HostLinkValue::U16(value) => value.format_for_suffix(data_format),
            HostLinkValue::I16(value) => value.format_for_suffix(data_format),
            HostLinkValue::U32(value) => value.format_for_suffix(data_format),
            HostLinkValue::I32(value) => value.format_for_suffix(data_format),
            HostLinkValue::F32(value) => value.format_for_suffix(data_format),
            HostLinkValue::Bool(value) => value.format_for_suffix(data_format),
            HostLinkValue::Text(value) => value.format_for_suffix(data_format),
        }
    }

    fn as_float(&self) -> Option<f64> {
        match self {
            HostLinkValue::F32(value) => Some(f64::from(*value)),
            _ => None,
        }
    }

    fn as_text(&self) -> Option<&str> {
        match self {
            HostLinkValue::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }

    fn append_to_payload(
        &self,
        data_format: &str,
        output: &mut String,
    ) -> Result<(), HostLinkError> {
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

pub async fn read_comment_bytes(
    client: &HostLinkClient,
    device: &str,
) -> Result<Vec<u8>, HostLinkError> {
    client.read_comment_bytes(device).await
}

pub async fn read_comments(
    client: &HostLinkClient,
    device: &str,
    encoding: HostLinkCommentEncoding,
) -> Result<String, HostLinkError> {
    client.read_comments(device, encoding).await
}

fn validate_read_typed_request(device: &str, dtype: &str) -> Result<(), HostLinkError> {
    let dtype = dtype.trim_start_matches('.').to_ascii_uppercase();
    if dtype.trim().is_empty() {
        return Err(HostLinkError::protocol(
            "dtype is required; specify U, S, D, L, F, H, or BIT.",
        ));
    }
    let parsed = parse_device(device)?;
    if dtype == "F" {
        require_float32_eligible_device_type(&parsed.device_type)?;
    }
    match dtype.as_str() {
        "F" => {
            prepare_read_address(device, Some("U"), 2)?;
        }
        "BIT" => {
            prepare_read_address(device, None, 1)?;
        }
        "U" | "S" | "D" | "L" | "H" => {
            prepare_read_address(device, Some(dtype.as_str()), 1)?;
        }
        other => {
            return Err(HostLinkError::protocol(format!(
                "Unsupported logical data type '{other}'."
            )));
        }
    }
    Ok(())
}

pub(crate) fn validate_named_addresses(addresses: &[String]) -> Result<(), HostLinkError> {
    validate_named_addresses_with_comment_policy(addresses, false).map(|_| ())
}

fn validate_named_addresses_with_comments(addresses: &[String]) -> Result<(), HostLinkError> {
    if !validate_named_addresses_with_comment_policy(addresses, true)? {
        return Err(HostLinkError::protocol(
            "comment encoding was provided but the aggregate contains no COMMENT entry.",
        ));
    }
    Ok(())
}

fn validate_named_addresses_with_comment_policy(
    addresses: &[String],
    allow_comments: bool,
) -> Result<bool, HostLinkError> {
    let mut has_comment = false;
    let mut semantic_keys = HashSet::new();
    for address in addresses {
        let (base_address, dtype, bit_index) = parse_named_address_parts(address)?;
        let parsed = parse_device(&base_address)?;
        match dtype.as_str() {
            "BIT_IN_WORD" => {
                require_bit_in_word_index(address, bit_index)?;
                prepare_read_address(&base_address, Some("U"), 1)?;
            }
            "COMMENT" => {
                if !allow_comments {
                    return Err(HostLinkError::protocol(
                        "COMMENT entries require read_named_with_comment_encoding or poll_with_comment_encoding and an explicit HostLinkCommentEncoding value.",
                    ));
                }
                has_comment = true;
                validate_device_type("RDC", &parsed.device_type, rdc_device_types())?;
                require_no_suffix(&parsed, "RDC")?;
            }
            _ => validate_read_typed_request(&base_address, &dtype)?,
        }
        let semantic_bit_index = (dtype == "BIT_IN_WORD").then_some(bit_index).flatten();
        if !semantic_keys.insert((
            parsed.device_type,
            parsed.number,
            dtype,
            semantic_bit_index,
            1usize,
        )) {
            return Err(HostLinkError::protocol(format!(
                "Named read address '{address}' is semantically duplicated."
            )));
        }
    }
    Ok(has_comment)
}

pub async fn read_typed(
    client: &HostLinkClient,
    device: &str,
    dtype: &str,
) -> Result<HostLinkValue, HostLinkError> {
    validate_read_typed_request(device, dtype)?;
    let (_turn, direct) = client.begin_turn().await?;
    read_typed_impl(&direct, device, dtype).await
}

async fn read_typed_impl(
    client: &HostLinkClient,
    device: &str,
    dtype: &str,
) -> Result<HostLinkValue, HostLinkError> {
    let dtype = dtype.trim_start_matches('.').to_ascii_uppercase();
    if dtype.trim().is_empty() {
        return Err(HostLinkError::protocol(
            "dtype is required; specify U, S, D, L, F, or BIT.",
        ));
    }
    let device = device.trim().to_ascii_uppercase();
    let parsed_device = parse_device(&device)?;
    let direct_bit_device = is_direct_bit_device_type(&parsed_device.device_type);

    if dtype == "F" {
        require_float32_eligible_device_type(&parsed_device.device_type)?;
    }

    if direct_bit_device && dtype != "BIT" {
        let expected = if matches!(dtype.as_str(), "D" | "L") {
            32
        } else {
            16
        };
        let tokens = client.read(&device, Some(&dtype)).await?;
        let packed = pack_direct_bit_tokens(&tokens, expected, &device)?;
        return match dtype.as_str() {
            "U" => Ok(HostLinkValue::U16(packed as u16)),
            "S" => Ok(HostLinkValue::I16(packed as u16 as i16)),
            "D" => Ok(HostLinkValue::U32(packed)),
            "L" => Ok(HostLinkValue::I32(packed as i32)),
            "H" => Ok(HostLinkValue::Text(format!("{:04X}", packed as u16))),
            other => Err(HostLinkError::protocol(format!(
                "Unsupported direct-bit word dtype '{other}'."
            ))),
        };
    }

    match dtype.as_str() {
        "F" => {
            let words = read_words(client, &device, 2).await?;
            let bits = (words[0] as u32) | ((words[1] as u32) << 16);
            Ok(HostLinkValue::F32(f32::from_bits(bits)))
        }
        "S" => {
            if is_timer_counter_composite_device(&device)? {
                let response = read_single_response(client, &device, Some("S")).await?;
                Ok(HostLinkValue::I16(
                    close_on_protocol_error(
                        client,
                        parse_timer_counter_tokens::<i16>(
                            &response,
                            "Invalid signed 16-bit timer/counter response",
                        )
                        .map(|(_, _, preset)| preset),
                    )
                    .await?,
                ))
            } else {
                Ok(HostLinkValue::I16(
                    read_single_parsed(
                        client,
                        &device,
                        Some("S"),
                        "Invalid signed 16-bit response",
                    )
                    .await?,
                ))
            }
        }
        "D" => {
            if is_timer_counter_composite_device(&device)? {
                let response = read_single_response(client, &device, Some("D")).await?;
                Ok(HostLinkValue::U32(
                    close_on_protocol_error(
                        client,
                        parse_timer_counter_tokens::<u32>(
                            &response,
                            "Invalid unsigned 32-bit timer/counter response",
                        )
                        .map(|(_, _, preset)| preset),
                    )
                    .await?,
                ))
            } else {
                Ok(HostLinkValue::U32(
                    read_single_parsed::<u32>(
                        client,
                        &device,
                        Some("D"),
                        "Invalid unsigned 32-bit response",
                    )
                    .await?,
                ))
            }
        }
        "L" => {
            if is_timer_counter_composite_device(&device)? {
                let response = read_single_response(client, &device, Some("L")).await?;
                Ok(HostLinkValue::I32(
                    close_on_protocol_error(
                        client,
                        parse_timer_counter_tokens::<i32>(
                            &response,
                            "Invalid signed 32-bit timer/counter response",
                        )
                        .map(|(_, _, preset)| preset),
                    )
                    .await?,
                ))
            } else {
                Ok(HostLinkValue::I32(
                    read_single_parsed(
                        client,
                        &device,
                        Some("L"),
                        "Invalid signed 32-bit response",
                    )
                    .await?,
                ))
            }
        }
        "U" => {
            if is_timer_counter_composite_device(&device)? {
                let response = read_single_response(client, &device, Some("U")).await?;
                Ok(HostLinkValue::U16(
                    close_on_protocol_error(
                        client,
                        parse_timer_counter_tokens::<u16>(
                            &response,
                            "Invalid unsigned 16-bit timer/counter response",
                        )
                        .map(|(_, _, preset)| preset),
                    )
                    .await?,
                ))
            } else {
                Ok(HostLinkValue::U16(
                    read_single_parsed::<u16>(
                        client,
                        &device,
                        Some("U"),
                        "Invalid unsigned 16-bit response",
                    )
                    .await?,
                ))
            }
        }
        "H" => {
            let response = read_single_response(client, &device, Some("H")).await?;
            let composite = is_timer_counter_composite_device(&device)?;
            let token = if composite {
                close_on_protocol_error(client, parse_timer_counter_hex_preset(&response)).await?
            } else {
                let tokens = crate::protocol::split_data_tokens(&response);
                if tokens.len() != 1 {
                    let error = HostLinkError::protocol(format!(
                        "Expected 1 hexadecimal response token but received {}",
                        tokens.len()
                    ));
                    client.retire_transport().await;
                    return Err(error);
                }
                tokens[0].clone()
            };
            let token = token.trim();
            if !(1..=4).contains(&token.len())
                || !token.chars().all(|character| character.is_ascii_hexdigit())
            {
                let error = HostLinkError::protocol(
                    "Hexadecimal response token must contain 1..=4 hexadecimal digits",
                );
                client.retire_transport().await;
                return Err(error);
            }
            let value = u16::from_str_radix(token, 16).map_err(|_| {
                HostLinkError::protocol(
                    "Hexadecimal response token must contain 1..=4 hexadecimal digits",
                )
            })?;
            Ok(HostLinkValue::Text(format!("{value:04X}")))
        }
        "BIT" => Ok(HostLinkValue::Bool(
            read_single_bool(client, &device, None).await?,
        )),
        other => Err(HostLinkError::protocol(format!(
            "Unsupported logical data type '{other}'."
        ))),
    }
}

pub async fn read_timer_counter(
    client: &HostLinkClient,
    device: &str,
) -> Result<TimerCounterValue, HostLinkError> {
    let address = parse_device(device)?;
    if !matches!(address.device_type.as_str(), "T" | "C") {
        return Err(HostLinkError::protocol(
            "read_timer_counter requires a T or C device.",
        ));
    }

    require_no_suffix(&address, "read_timer_counter")?;
    let target = address.to_text()?;
    let (_turn, direct) = client.begin_turn().await?;
    read_timer_counter_impl(&direct, &target).await
}

async fn read_timer_counter_impl(
    client: &HostLinkClient,
    target: &str,
) -> Result<TimerCounterValue, HostLinkError> {
    let response = read_single_response(client, target, Some("D")).await?;
    let (status, current, preset) = close_on_protocol_error(
        client,
        parse_timer_counter_tokens::<u32>(
            &response,
            "Invalid timer/counter status/current/preset response",
        ),
    )
    .await?;
    Ok(TimerCounterValue {
        status,
        current,
        preset,
    })
}

pub async fn read_timer(
    client: &HostLinkClient,
    device: &str,
) -> Result<TimerCounterValue, HostLinkError> {
    if parse_device(device)?.device_type != "T" {
        return Err(HostLinkError::protocol("read_timer requires a T device."));
    }
    read_timer_counter(client, device).await
}

pub async fn read_counter(
    client: &HostLinkClient,
    device: &str,
) -> Result<TimerCounterValue, HostLinkError> {
    if parse_device(device)?.device_type != "C" {
        return Err(HostLinkError::protocol("read_counter requires a C device."));
    }
    read_timer_counter(client, device).await
}

pub async fn write_typed<T: HostLinkPayloadValue>(
    client: &HostLinkClient,
    device: &str,
    dtype: &str,
    value: &T,
) -> Result<(), HostLinkError> {
    let dtype = dtype.trim_start_matches('.').to_ascii_uppercase();
    if dtype.trim().is_empty() {
        return Err(HostLinkError::protocol(
            "dtype is required; specify U, S, D, L, F, or BIT.",
        ));
    }
    let parsed_device = parse_device(device)?;
    if dtype == "F" {
        require_float32_eligible_device_type(&parsed_device.device_type)?;
    }
    match dtype.as_str() {
        "F" => {
            let single = if let Some(value) = value.as_float() {
                value as f32
            } else if let Some(text) = value.as_text() {
                text.trim()
                    .parse::<f32>()
                    .map_err(|_| HostLinkError::protocol("Invalid float32 input"))?
            } else {
                return Err(HostLinkError::protocol("Invalid float32 input"));
            };
            if !single.is_finite() {
                return Err(HostLinkError::protocol("Float32 input must be finite"));
            }
            let bits = single.to_bits();
            let words = [(bits & 0xFFFF) as u16, (bits >> 16) as u16];
            client.write_consecutive(device, &words, Some("U")).await
        }
        "BIT" => {
            let parsed = value
                .as_bool()
                .ok_or_else(|| HostLinkError::protocol("BIT writes require a bool value"))?;
            client.write(device, parsed, None).await
        }
        "H" => {
            let parsed = if let Some(integer) = value.as_integer() {
                u16::try_from(integer)
                    .map_err(|_| HostLinkError::protocol("Invalid hexadecimal 16-bit input"))?
            } else {
                let token = value
                    .as_text()
                    .ok_or_else(|| HostLinkError::protocol("Invalid hexadecimal 16-bit input"))?;
                u16::from_str_radix(token.trim(), 16)
                    .map_err(|_| HostLinkError::protocol("Invalid hexadecimal 16-bit input"))?
            };
            client.write(device, parsed, Some("H")).await
        }
        "S" | "D" | "L" | "U" => client.write(device, value, Some(dtype.as_str())).await,
        other => Err(HostLinkError::protocol(format!(
            "Unsupported logical data type '{other}'."
        ))),
    }
}

pub(crate) fn parse_bool_token(token: &str) -> Result<bool, HostLinkError> {
    match token {
        "1" | "ON" => Ok(true),
        "0" | "OFF" => Ok(false),
        _ => Err(HostLinkError::protocol(format!(
            "Invalid direct bit response token: {token}"
        ))),
    }
}

pub(crate) fn pack_direct_bit_tokens(
    tokens: &[String],
    expected: usize,
    context: &str,
) -> Result<u32, HostLinkError> {
    if tokens.len() != expected {
        return Err(HostLinkError::protocol(format!(
            "Direct-bit word response for '{context}' returned {} tokens; expected {expected}",
            tokens.len()
        )));
    }
    let mut packed = 0u32;
    for (bit, token) in tokens.iter().enumerate() {
        if parse_bool_token(token)? {
            packed |= 1u32 << bit;
        }
    }
    Ok(packed)
}

fn response_tokens(response_text: &str) -> impl Iterator<Item = &str> {
    response_text
        .split([' ', ','])
        .filter(|token| !token.is_empty())
}

fn is_timer_counter_composite_device(device: &str) -> Result<bool, HostLinkError> {
    let address = parse_device(device)?;
    Ok(matches!(address.device_type.as_str(), "T" | "C"))
}

fn parse_timer_counter_tokens<T: FromStr>(
    response_text: &str,
    invalid_message: &'static str,
) -> Result<(u32, T, T), HostLinkError> {
    let tokens = response_tokens(response_text).collect::<Vec<_>>();
    if tokens.len() != 3 {
        return Err(HostLinkError::protocol(format!(
            "Timer/counter response must contain exactly 3 tokens, received {}",
            tokens.len()
        )));
    }
    let status = match tokens[0] {
        "0" => 0,
        "1" => 1,
        other => {
            return Err(HostLinkError::protocol(format!(
                "Invalid timer/counter status token: {other}"
            )));
        }
    };
    let current = tokens[1]
        .parse::<T>()
        .map_err(|_| HostLinkError::protocol(invalid_message))?;
    let preset = tokens[2]
        .parse::<T>()
        .map_err(|_| HostLinkError::protocol(invalid_message))?;
    Ok((status, current, preset))
}

fn parse_timer_counter_hex_preset(response_text: &str) -> Result<String, HostLinkError> {
    let tokens = response_tokens(response_text).collect::<Vec<_>>();
    if tokens.len() != 3 {
        return Err(HostLinkError::protocol(format!(
            "Timer/counter response must contain exactly 3 tokens, received {}",
            tokens.len()
        )));
    }
    if !matches!(tokens[0], "0" | "1") {
        return Err(HostLinkError::protocol(format!(
            "Invalid timer/counter status token: {}",
            tokens[0]
        )));
    }
    for token in &tokens[1..] {
        if !(1..=4).contains(&token.len())
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
            || u16::from_str_radix(token, 16).is_err()
        {
            return Err(HostLinkError::protocol(
                "Invalid timer/counter hexadecimal 16-bit response",
            ));
        }
    }
    let preset = u16::from_str_radix(tokens[2], 16).map_err(|_| {
        HostLinkError::protocol("Invalid timer/counter hexadecimal 16-bit response")
    })?;
    Ok(format!("{preset:04X}"))
}

fn parse_all_tokens<T: FromStr>(
    response_text: &str,
    invalid_message: &'static str,
) -> Result<Vec<T>, HostLinkError> {
    let mut values = Vec::new();
    for token in response_tokens(response_text) {
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
    let suffix = require_explicit_format(&address, data_format)?;
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
    client
        .send_decoded(&format!("RD {}", address.to_text()?))
        .await
}

async fn read_single_parsed<T: FromStr>(
    client: &HostLinkClient,
    device: &str,
    data_format: Option<&str>,
    invalid_message: &'static str,
) -> Result<T, HostLinkError> {
    let response = read_single_response(client, device, data_format).await?;
    let values =
        close_on_protocol_error(client, parse_all_tokens::<T>(&response, invalid_message)).await?;
    if values.len() != 1 {
        client.retire_transport().await;
        return Err(HostLinkError::protocol(format!(
            "Response returned {} values; expected 1",
            values.len()
        )));
    }
    close_on_protocol_error(
        client,
        values
            .into_iter()
            .next()
            .ok_or_else(|| HostLinkError::protocol("Missing response token")),
    )
    .await
}

async fn read_single_bool(
    client: &HostLinkClient,
    device: &str,
    data_format: Option<&str>,
) -> Result<bool, HostLinkError> {
    let response = read_single_response(client, device, data_format).await?;
    close_on_protocol_error(client, parse_bool_token(&response)).await
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
        .send_decoded(&format!("RDS {} {}", address.to_text()?, count))
        .await?;
    let values =
        close_on_protocol_error(client, parse_all_tokens(&response, invalid_message)).await?;
    if values.len() != count {
        client.retire_transport().await;
        return Err(HostLinkError::protocol(format!(
            "Response returned {} values; expected {count}",
            values.len()
        )));
    }
    Ok(values)
}

async fn close_on_protocol_error<T>(
    client: &HostLinkClient,
    result: Result<T, HostLinkError>,
) -> Result<T, HostLinkError> {
    match result {
        Ok(value) => Ok(value),
        Err(error) => {
            client.retire_transport().await;
            Err(error)
        }
    }
}

pub async fn read_named<S: AsRef<str>>(
    client: &HostLinkClient,
    addresses: &[S],
) -> Result<NamedReadResult, HostLinkError> {
    let addr_list = addresses
        .iter()
        .map(|item| item.as_ref().to_owned())
        .collect::<Vec<_>>();
    if addr_list.is_empty() {
        return Err(HostLinkError::protocol(
            "read_named addresses must not be empty.",
        ));
    }
    validate_named_addresses(&addr_list)?;
    let compiled = compile_read_named_plan(&addr_list);
    read_named_compiled(client, &addr_list, compiled.as_ref(), None).await
}

/// Read a named aggregate with an explicit encoding for at least one comment entry.
///
/// A list without `:COMMENT` rejects the unused encoding before transport.
pub async fn read_named_with_comment_encoding<S: AsRef<str>>(
    client: &HostLinkClient,
    addresses: &[S],
    comment_encoding: HostLinkCommentEncoding,
) -> Result<NamedReadResult, HostLinkError> {
    let addr_list = addresses
        .iter()
        .map(|item| item.as_ref().to_owned())
        .collect::<Vec<_>>();
    if addr_list.is_empty() {
        return Err(HostLinkError::protocol(
            "read_named_with_comment_encoding addresses must not be empty.",
        ));
    }
    validate_named_addresses_with_comments(&addr_list)?;
    let compiled = compile_read_named_plan(&addr_list);
    read_named_compiled(
        client,
        &addr_list,
        compiled.as_ref(),
        Some(comment_encoding),
    )
    .await
}

pub(crate) async fn read_named_compiled(
    client: &HostLinkClient,
    addresses: &[String],
    compiled: Option<&CompiledReadNamedPlan>,
    comment_encoding: Option<HostLinkCommentEncoding>,
) -> Result<NamedReadResult, HostLinkError> {
    let staged = {
        let (_turn, direct) = client.begin_turn().await?;
        if let Some(plan) = compiled {
            execute_read_named_plan(&direct, addresses, plan, comment_encoding).await?
        } else {
            read_named_sequential(&direct, addresses, comment_encoding).await?
        }
    };
    Ok(materialize_named_result(addresses, staged))
}

pub(crate) async fn read_named_sequential(
    client: &HostLinkClient,
    addresses: &[String],
    comment_encoding: Option<HostLinkCommentEncoding>,
) -> Result<Vec<HostLinkValue>, HostLinkError> {
    let mut staged = Vec::with_capacity(addresses.len());
    for address in addresses {
        staged.push(read_named_value(client, address, comment_encoding).await?);
    }
    Ok(staged)
}

async fn read_named_value(
    client: &HostLinkClient,
    address: &str,
    comment_encoding: Option<HostLinkCommentEncoding>,
) -> Result<HostLinkValue, HostLinkError> {
    let (base_address, dtype, bit_index) = parse_named_address_parts(address)?;
    if dtype == "BIT_IN_WORD" {
        let bit_index = require_bit_in_word_index(address, bit_index)?;
        let parsed = parse_device(&base_address)?;
        let word = if is_direct_bit_device_type(&parsed.device_type) {
            let tokens = client.read(&base_address, Some("U")).await?;
            pack_direct_bit_tokens(&tokens, 16, &base_address)? as u16
        } else {
            read_single_parsed::<u16>(
                client,
                &base_address,
                Some("U"),
                "Invalid unsigned 16-bit response",
            )
            .await?
        };
        Ok(HostLinkValue::Bool(((word >> bit_index) & 1) != 0))
    } else if dtype == "COMMENT" {
        let encoding = comment_encoding.ok_or_else(|| {
            HostLinkError::protocol(
                "COMMENT entries require an explicit HostLinkCommentEncoding value.",
            )
        })?;
        Ok(HostLinkValue::Text(
            read_comments(client, &base_address, encoding).await?,
        ))
    } else {
        read_typed_impl(client, &base_address, &dtype).await
    }
}

fn require_bit_in_word_index(address: &str, bit_index: Option<u8>) -> Result<u8, HostLinkError> {
    bit_index.ok_or_else(|| {
        HostLinkError::protocol(format!(
            "Address '{address}' uses BIT_IN_WORD but no bit index was specified. Use '.0' through '.F' notation."
        ))
    })
}

pub(crate) async fn execute_read_named_plan(
    client: &HostLinkClient,
    addresses: &[String],
    plan: &CompiledReadNamedPlan,
    comment_encoding: Option<HostLinkCommentEncoding>,
) -> Result<Vec<HostLinkValue>, HostLinkError> {
    let mut resolved = vec![HostLinkValue::U16(0); plan.input_count];
    for operation in &plan.operations {
        match operation {
            ReadPlanOperation::Segment(segment) => match segment.mode {
                ReadPlanSegmentMode::Words => {
                    let words =
                        read_words(client, &segment.start_address.to_text()?, segment.count)
                            .await?;
                    for request in &segment.requests {
                        let offset = (read_plan_number(request) - segment.start_number) as usize;
                        resolved[request.index] =
                            resolve_planned_value(&words, offset, request.kind, request.bit_index)?;
                    }
                }
                ReadPlanSegmentMode::DirectBits => {
                    let tokens = client
                        .read_consecutive(&segment.start_address.to_text()?, segment.count, None)
                        .await?;
                    for request in &segment.requests {
                        let offset = (read_plan_number(request) - segment.start_number) as usize;
                        resolved[request.index] = resolve_direct_bit_value(&tokens, offset)?;
                    }
                }
            },
            ReadPlanOperation::Sequential { index } => {
                resolved[*index] =
                    read_named_value(client, &addresses[*index], comment_encoding).await?;
            }
        }
    }

    Ok(resolved)
}

fn materialize_named_result(addresses: &[String], staged: Vec<HostLinkValue>) -> NamedReadResult {
    debug_assert_eq!(addresses.len(), staged.len());
    addresses.iter().cloned().zip(staged).collect()
}

pub fn poll<'a, S: AsRef<str> + 'a>(
    client: &'a HostLinkClient,
    addresses: &'a [S],
    interval: Duration,
) -> impl Stream<Item = Result<NamedReadResult, HostLinkError>> + 'a {
    async_stream::try_stream! {
        let addr_list = addresses.iter().map(|item| item.as_ref().to_owned()).collect::<Vec<_>>();
        if addr_list.is_empty() {
            Err(HostLinkError::protocol("poll addresses must not be empty."))?;
        }
        if interval.is_zero() {
            Err(HostLinkError::protocol("poll interval must be greater than zero."))?;
        }
        validate_named_addresses(&addr_list)?;
        let compiled = compile_read_named_plan(&addr_list);
        loop {
            let result = read_named_compiled(client, &addr_list, compiled.as_ref(), None).await?;
            yield result;
            tokio::time::sleep(interval).await;
        }
    }
}

/// Poll a named aggregate with an explicit encoding for at least one comment entry.
///
/// A list without `:COMMENT` rejects the unused encoding before transport.
pub fn poll_with_comment_encoding<'a, S: AsRef<str> + 'a>(
    client: &'a HostLinkClient,
    addresses: &'a [S],
    interval: Duration,
    comment_encoding: HostLinkCommentEncoding,
) -> impl Stream<Item = Result<NamedReadResult, HostLinkError>> + 'a {
    async_stream::try_stream! {
        let addr_list = addresses.iter().map(|item| item.as_ref().to_owned()).collect::<Vec<_>>();
        if addr_list.is_empty() {
            Err(HostLinkError::protocol("poll_with_comment_encoding addresses must not be empty."))?;
        }
        if interval.is_zero() {
            Err(HostLinkError::protocol("poll interval must be greater than zero."))?;
        }
        validate_named_addresses_with_comments(&addr_list)?;
        let compiled = compile_read_named_plan(&addr_list);
        loop {
            let result = read_named_compiled(
                client,
                &addr_list,
                compiled.as_ref(),
                Some(comment_encoding),
            )
            .await?;
            yield result;
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
    prepare_read_address(device, Some("U"), count)?;
    let (_turn, direct) = client.begin_turn().await?;
    read_consecutive_parsed::<u16>(
        &direct,
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
    prepare_read_address(device, Some("D"), count)?;
    let (_turn, direct) = client.begin_turn().await?;
    read_consecutive_parsed::<u32>(
        &direct,
        device,
        count,
        Some("D"),
        "Invalid unsigned 32-bit response",
    )
    .await
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
    client.write_consecutive(device, values, Some("D")).await
}
