use crate::error::HostLinkError;
use std::fmt;

const DEVICE_TYPES_PARSE_ORDER: &[&str] = &[
    "MR", "LR", "CR", "VB", "DM", "EM", "FM", "ZF", "TM", "TC", "TS", "CC", "CS", "AT", "CM", "VM",
    "R", "B", "W", "Z", "T", "C", "X", "Y", "M", "L", "D", "E", "F",
];
const FORCE_DEVICE_TYPES: &[&str] = &["R", "B", "MR", "LR", "CR", "T", "C", "VB"];
const MBS_DEVICE_TYPES: &[&str] = &["R", "B", "MR", "LR", "CR", "T", "C", "VB"];
const MWS_DEVICE_TYPES: &[&str] = &[
    "R", "B", "MR", "LR", "CR", "VB", "DM", "EM", "FM", "W", "TM", "Z", "TC", "TS", "CC", "CS",
    "CM", "VM",
];
const RDC_DEVICE_TYPES: &[&str] = &[
    "R", "B", "MR", "LR", "CR", "DM", "EM", "FM", "ZF", "W", "TM", "Z", "T", "C", "CM", "X",
    "Y", "M", "L", "D", "E", "F",
];
const WS_DEVICE_TYPES: &[&str] = &["T", "C"];

#[derive(Debug, Clone, Copy)]
struct DeviceRange {
    lo: u32,
    hi: u32,
    base: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvDeviceAddress {
    pub device_type: String,
    pub number: u32,
    pub suffix: String,
}

impl KvDeviceAddress {
    pub fn to_text(&self) -> Result<String, HostLinkError> {
        let range = device_range(&self.device_type).ok_or_else(|| {
            HostLinkError::protocol(format!("Unsupported device type: {}", self.device_type))
        })?;
        let number = if range.base == 16 {
            format!("{:X}", self.number)
        } else {
            self.number.to_string()
        };
        Ok(format!("{}{}{}", self.device_type, number, self.suffix))
    }
}

impl fmt::Display for KvDeviceAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.to_text() {
            Ok(text) => write!(f, "{text}"),
            Err(_) => write!(f, "{}{}{}", self.device_type, self.number, self.suffix),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvLogicalAddress {
    pub base_address: KvDeviceAddress,
    pub data_type: String,
    pub bit_index: Option<u8>,
}

impl KvLogicalAddress {
    pub fn is_bit_in_word(&self) -> bool {
        self.bit_index.is_some()
    }

    pub fn to_text(&self) -> Result<String, HostLinkError> {
        let mut base = self.base_address.clone();
        base.suffix.clear();
        let base_text = base.to_text()?;
        if let Some(bit_index) = self.bit_index {
            return Ok(format!("{base_text}.{bit_index:X}"));
        }

        if self.data_type == "U" {
            Ok(base_text)
        } else {
            Ok(format!("{base_text}:{}", self.data_type))
        }
    }
}

pub struct HostLinkAddress;

impl HostLinkAddress {
    pub fn parse(text: &str) -> Result<KvDeviceAddress, HostLinkError> {
        parse_device(text)
    }

    pub fn try_parse(text: &str) -> Option<KvDeviceAddress> {
        parse_device(text).ok()
    }

    pub fn format(address: &KvDeviceAddress) -> Result<String, HostLinkError> {
        address.to_text()
    }

    pub fn normalize(text: &str) -> Result<String, HostLinkError> {
        if let Ok(address) = parse_device(text) {
            return address.to_text();
        }

        parse_logical_address(text)?.to_text()
    }

    pub fn parse_logical(text: &str) -> Result<KvLogicalAddress, HostLinkError> {
        parse_logical_address(text)
    }

    pub fn try_parse_logical(text: &str) -> Option<KvLogicalAddress> {
        parse_logical_address(text).ok()
    }

    pub fn normalize_logical(text: &str) -> Result<String, HostLinkError> {
        parse_logical_address(text)?.to_text()
    }
}

pub(crate) fn model_name_for_code(code: &str) -> &str {
    match code {
        "134" => "KV-N24nn",
        "133" => "KV-N40nn",
        "132" => "KV-N60nn",
        "128" => "KV-NC32T",
        "63" => "KV-X550",
        "61" => "KV-X530",
        "60" => "KV-X520",
        "62" => "KV-X500",
        "59" => "KV-X310",
        "58" => "KV-8000A",
        "57" => "KV-8000",
        "55" => "KV-7500",
        "54" => "KV-7300",
        "53" => "KV-5500",
        "52" => "KV-5000",
        "51" => "KV-3000",
        "50" => "KV-1000",
        "49" => "KV-700 (With expansion memory)",
        "48" => "KV-700 (No expansion memory)",
        _ => "Unknown",
    }
}

pub(crate) fn force_device_types() -> &'static [&'static str] {
    FORCE_DEVICE_TYPES
}

pub(crate) fn mbs_device_types() -> &'static [&'static str] {
    MBS_DEVICE_TYPES
}

pub(crate) fn mws_device_types() -> &'static [&'static str] {
    MWS_DEVICE_TYPES
}

pub(crate) fn rdc_device_types() -> &'static [&'static str] {
    RDC_DEVICE_TYPES
}

pub(crate) fn ws_device_types() -> &'static [&'static str] {
    WS_DEVICE_TYPES
}

pub(crate) fn default_format_by_device_type(device_type: &str) -> &'static str {
    match device_type {
        "R" | "B" | "MR" | "LR" | "CR" | "VB" | "X" | "Y" | "M" | "L" => "",
        "DM" | "EM" | "FM" | "ZF" | "W" | "TM" | "Z" | "AT" | "CM" | "VM" | "D" | "E" | "F" => ".U",
        "T" | "TC" | "TS" | "C" | "CC" | "CS" => ".D",
        _ => "",
    }
}

pub(crate) fn is_optimizable_read_named_device_type(device_type: &str) -> bool {
    default_format_by_device_type(device_type) == ".U"
}

pub(crate) fn offset_device(
    start: &KvDeviceAddress,
    word_offset: u32,
) -> Result<String, HostLinkError> {
    let mut next = start.clone();
    next.number = next
        .number
        .checked_add(word_offset)
        .ok_or_else(|| HostLinkError::protocol("Device offset overflow"))?;
    next.suffix.clear();
    next.to_text()
}

pub(crate) fn parse_named_address_parts(
    address: &str,
) -> Result<(String, String, Option<u8>), HostLinkError> {
    let logical = parse_logical_address(address)?;
    let mut base = logical.base_address;
    base.suffix.clear();
    Ok((base.to_text()?, logical.data_type, logical.bit_index))
}

pub fn normalize_suffix(suffix: impl AsRef<str>) -> Result<String, HostLinkError> {
    let suffix = suffix.as_ref();
    if suffix.is_empty() {
        return Ok(String::new());
    }

    let mut normalized = suffix.trim().to_ascii_uppercase();
    if !normalized.starts_with('.') {
        normalized.insert(0, '.');
    }

    match normalized.as_str() {
        ".U" | ".S" | ".D" | ".L" | ".H" => Ok(normalized),
        _ => Err(HostLinkError::protocol(format!(
            "Unsupported data format suffix: {suffix}"
        ))),
    }
}

pub fn parse_device(text: &str) -> Result<KvDeviceAddress, HostLinkError> {
    parse_device_internal(text, true)
}

fn parse_device_internal(
    text: &str,
    allow_omitted_type: bool,
) -> Result<KvDeviceAddress, HostLinkError> {
    let raw = text.trim().to_ascii_uppercase();
    if raw.is_empty() {
        return Err(HostLinkError::protocol("Device string must not be empty"));
    }

    let (core, suffix) = extract_suffix(&raw)?;
    let (device_type, number_text) = if let Some(device_type) = DEVICE_TYPES_PARSE_ORDER
        .iter()
        .find(|candidate| core.starts_with(**candidate))
    {
        (
            (*device_type).to_owned(),
            core[device_type.len()..].to_owned(),
        )
    } else if allow_omitted_type && core.bytes().all(|byte| byte.is_ascii_digit()) {
        ("R".to_owned(), core.to_owned())
    } else {
        return Err(HostLinkError::protocol(format!(
            "Invalid device string '{text}'. Valid device types: {}.",
            DEVICE_TYPES_PARSE_ORDER.join(", ")
        )));
    };

    if number_text.is_empty() || !number_text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(HostLinkError::protocol(format!(
            "Invalid device number for {device_type}: {number_text}"
        )));
    }

    let range = device_range(&device_type).ok_or_else(|| {
        HostLinkError::protocol(format!("Unsupported device type: {device_type}"))
    })?;

    let number = u32::from_str_radix(&number_text, range.base).map_err(|_| {
        HostLinkError::protocol(format!(
            "Invalid device number for {device_type}: {number_text}"
        ))
    })?;
    if number < range.lo || number > range.hi {
        return Err(HostLinkError::protocol(format!(
            "Device number out of range: {device_type}{number_text} (allowed: {}..{})",
            range.lo, range.hi
        )));
    }

    Ok(KvDeviceAddress {
        device_type,
        number,
        suffix,
    })
}

pub fn parse_logical_address(text: &str) -> Result<KvLogicalAddress, HostLinkError> {
    let raw = text.trim();
    if raw.is_empty() {
        return Err(HostLinkError::protocol("Address must not be empty"));
    }

    if let Some(colon_index) = raw.find(':') {
        let base = parse_device(&raw[..colon_index])?;
        let mut base = base;
        base.suffix.clear();
        return Ok(KvLogicalAddress {
            base_address: base,
            data_type: normalize_dtype(&raw[colon_index + 1..])?,
            bit_index: None,
        });
    }

    if let Some(dot_index) = raw.rfind('.') {
        if let Ok(bit_index) = u8::from_str_radix(&raw[dot_index + 1..], 16) {
            if bit_index <= 15 {
                let mut base = parse_device(&raw[..dot_index])?;
                base.suffix.clear();
                return Ok(KvLogicalAddress {
                    base_address: base,
                    data_type: "BIT_IN_WORD".to_owned(),
                    bit_index: Some(bit_index),
                });
            }
        }
    }

    let mut base = parse_device(raw)?;
    base.suffix.clear();
    Ok(KvLogicalAddress {
        base_address: base,
        data_type: "U".to_owned(),
        bit_index: None,
    })
}

pub fn resolve_effective_format(device_type: &str, suffix: &str) -> String {
    if suffix.is_empty() {
        default_format_by_device_type(device_type).to_owned()
    } else {
        suffix.to_owned()
    }
}

pub fn validate_device_type(
    command: &str,
    device_type: &str,
    allowed_types: &[&str],
) -> Result<(), HostLinkError> {
    if allowed_types.contains(&device_type) {
        Ok(())
    } else {
        Err(HostLinkError::protocol(format!(
            "Command '{command}' does not support device type '{device_type}'. Supported types: {}.",
            allowed_types.join(", ")
        )))
    }
}

pub fn validate_device_count(
    device_type: &str,
    effective_format: &str,
    count: usize,
) -> Result<(), HostLinkError> {
    let is_32_bit = matches!(effective_format, ".D" | ".L");
    let (lo, hi) = match device_type {
        "TM" => (1, if is_32_bit { 256 } else { 512 }),
        "Z" => (1, 12),
        "AT" => (1, 8),
        "T" | "TC" | "TS" | "C" | "CC" | "CS" => (1, 120),
        _ => (1, if is_32_bit { 500 } else { 1000 }),
    };

    if !(lo..=hi).contains(&count) {
        return Err(HostLinkError::protocol(format!(
            "Count {count} is out of range for device type '{device_type}' with format '{effective_format}' (allowed: {lo}..{hi})."
        )));
    }

    Ok(())
}

pub fn validate_device_span(
    device_type: &str,
    start_number: u32,
    effective_format: &str,
    count: usize,
) -> Result<(), HostLinkError> {
    let range = device_range(device_type).ok_or_else(|| {
        HostLinkError::protocol(format!("Unsupported device type: {device_type}"))
    })?;
    if count == 0 {
        return Err(HostLinkError::protocol(
            "count out of range: 0 (allowed: 1..)",
        ));
    }

    let word_width = if matches!(effective_format, ".D" | ".L") {
        2u32
    } else {
        1u32
    };
    let end_number = start_number
        .checked_add((count as u32).saturating_mul(word_width))
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| HostLinkError::protocol("Device span overflow"))?;

    if start_number < range.lo || start_number > range.hi || end_number > range.hi {
        let start_text = format_number(start_number, range.base);
        let end_text = format_number(end_number, range.base);
        return Err(HostLinkError::protocol(format!(
            "Device span out of range: {device_type}{start_text}..{device_type}{end_text} with format '{effective_format}'"
        )));
    }

    Ok(())
}

pub fn validate_expansion_buffer_count(
    effective_format: &str,
    count: usize,
) -> Result<(), HostLinkError> {
    let hi = if matches!(effective_format, ".D" | ".L") {
        500
    } else {
        1000
    };
    if !(1..=hi).contains(&count) {
        return Err(HostLinkError::protocol(format!(
            "Count {count} is out of range for expansion buffer format '{effective_format}' (allowed: 1..{hi})."
        )));
    }
    Ok(())
}

pub fn validate_expansion_buffer_span(
    address: u32,
    effective_format: &str,
    count: usize,
) -> Result<(), HostLinkError> {
    if count == 0 {
        return Err(HostLinkError::protocol(
            "count out of range: 0 (allowed: 1..)",
        ));
    }

    let word_width = if matches!(effective_format, ".D" | ".L") {
        2u32
    } else {
        1u32
    };
    let end_address = address
        .checked_add((count as u32).saturating_mul(word_width))
        .and_then(|value| value.checked_sub(1))
        .ok_or_else(|| HostLinkError::protocol("Expansion buffer span overflow"))?;
    if address > 59_999 || end_address > 59_999 {
        return Err(HostLinkError::protocol(format!(
            "Expansion buffer span out of range: {address}..{end_address} with format '{effective_format}'"
        )));
    }
    Ok(())
}

fn normalize_dtype(text: &str) -> Result<String, HostLinkError> {
    match text
        .trim()
        .trim_start_matches('.')
        .to_ascii_uppercase()
        .as_str()
    {
        "U" => Ok("U".to_owned()),
        "S" => Ok("S".to_owned()),
        "D" => Ok("D".to_owned()),
        "L" => Ok("L".to_owned()),
        "F" => Ok("F".to_owned()),
        "COMMENT" => Ok("COMMENT".to_owned()),
        _ => Err(HostLinkError::protocol(format!(
            "Unsupported logical data type '{text}'."
        ))),
    }
}

fn extract_suffix(raw: &str) -> Result<(&str, String), HostLinkError> {
    if raw.len() >= 2 && raw.as_bytes()[raw.len() - 2] == b'.' {
        let suffix = normalize_suffix(&raw[raw.len() - 2..])?;
        Ok((&raw[..raw.len() - 2], suffix))
    } else {
        Ok((raw, String::new()))
    }
}

fn format_number(value: u32, base: u32) -> String {
    if base == 16 {
        format!("{value:X}")
    } else {
        value.to_string()
    }
}

fn device_range(device_type: &str) -> Option<DeviceRange> {
    let range = match device_type {
        "R" => DeviceRange {
            lo: 0,
            hi: 199_915,
            base: 10,
        },
        "B" => DeviceRange {
            lo: 0,
            hi: 0x7FFF,
            base: 16,
        },
        "MR" => DeviceRange {
            lo: 0,
            hi: 399_915,
            base: 10,
        },
        "LR" => DeviceRange {
            lo: 0,
            hi: 99_915,
            base: 10,
        },
        "CR" => DeviceRange {
            lo: 0,
            hi: 7_915,
            base: 10,
        },
        "VB" => DeviceRange {
            lo: 0,
            hi: 0xF9FF,
            base: 16,
        },
        "DM" => DeviceRange {
            lo: 0,
            hi: 65_534,
            base: 10,
        },
        "EM" => DeviceRange {
            lo: 0,
            hi: 65_534,
            base: 10,
        },
        "FM" => DeviceRange {
            lo: 0,
            hi: 32_767,
            base: 10,
        },
        "ZF" => DeviceRange {
            lo: 0,
            hi: 524_287,
            base: 10,
        },
        "W" => DeviceRange {
            lo: 0,
            hi: 0x7FFF,
            base: 16,
        },
        "TM" => DeviceRange {
            lo: 0,
            hi: 511,
            base: 10,
        },
        "Z" => DeviceRange {
            lo: 1,
            hi: 12,
            base: 10,
        },
        "T" | "TC" | "TS" | "C" | "CC" | "CS" => DeviceRange {
            lo: 0,
            hi: 3_999,
            base: 10,
        },
        "AT" => DeviceRange {
            lo: 0,
            hi: 7,
            base: 10,
        },
        "CM" => DeviceRange {
            lo: 0,
            hi: 7_599,
            base: 10,
        },
        "VM" => DeviceRange {
            lo: 0,
            hi: 589_823,
            base: 10,
        },
        "X" => DeviceRange {
            lo: 0,
            hi: 0x1999F,
            base: 16,
        },
        "Y" => DeviceRange {
            lo: 0,
            hi: 0x63999F,
            base: 16,
        },
        "M" | "L" => DeviceRange {
            lo: 0,
            hi: 15_999,
            base: 10,
        },
        "D" | "E" => DeviceRange {
            lo: 0,
            hi: 65_534,
            base: 10,
        },
        "F" => DeviceRange {
            lo: 0,
            hi: 32_767,
            base: 10,
        },
        _ => return None,
    };
    Some(range)
}

#[cfg(test)]
mod tests {
    use super::{HostLinkAddress, parse_device, parse_logical_address};

    #[test]
    fn parse_device_normalizes_hex_suffix_and_number() {
        let address = parse_device("w1a.h").unwrap();
        assert_eq!(address.device_type, "W");
        assert_eq!(address.number, 0x1A);
        assert_eq!(address.suffix, ".H");
        assert_eq!(address.to_text().unwrap(), "W1A.H");
    }

    #[test]
    fn parse_logical_bit_index_uses_hex_notation() {
        let logical = parse_logical_address("dm100.a").unwrap();
        assert_eq!(logical.to_text().unwrap(), "DM100.A");
        assert_eq!(logical.bit_index, Some(10));
    }

    #[test]
    fn normalize_plain_address_keeps_default_r_omission_rule() {
        assert_eq!(HostLinkAddress::normalize("100").unwrap(), "R100");
    }

    #[test]
    fn parse_logical_comment_address_round_trips() {
        let logical = parse_logical_address("dm100:comment").unwrap();
        assert_eq!(logical.to_text().unwrap(), "DM100:COMMENT");
        assert_eq!(logical.data_type, "COMMENT");
    }
}
