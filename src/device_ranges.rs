use crate::address::{default_format_by_device_type, is_direct_bit_device_type};
use crate::error::HostLinkError;
use crate::plc_profiles::{
    KvHostLinkPlcProfile, normalize_plc_profile, profile_from_name, profiles as plc_profiles,
};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvDeviceRangeNotation {
    Decimal,
    Hexadecimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvDeviceRangeCategory {
    Bit,
    Word,
    TimerCounter,
    Index,
    FileRegister,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvDeviceRangeSegment {
    pub device: String,
    pub category: KvDeviceRangeCategory,
    pub is_bit_device: bool,
    pub notation: KvDeviceRangeNotation,
    pub lower_bound: u32,
    pub upper_bound: Option<u32>,
    pub point_count: Option<u32>,
    pub address_range: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvDeviceRangeEntry {
    pub device: String,
    pub device_type: String,
    pub category: KvDeviceRangeCategory,
    pub is_bit_device: bool,
    pub notation: KvDeviceRangeNotation,
    pub supported: bool,
    pub lower_bound: u32,
    pub upper_bound: Option<u32>,
    pub point_count: Option<u32>,
    pub address_range: Option<String>,
    pub source: String,
    pub notes: Option<String>,
    pub segments: Vec<KvDeviceRangeSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvDeviceRangeCatalog {
    pub plc_profile: String,
    pub model_code: String,
    pub has_model_code: bool,
    pub requested_plc_profile: String,
    pub resolved_plc_profile: String,
    pub entries: Vec<KvDeviceRangeEntry>,
}

impl KvDeviceRangeCatalog {
    pub fn entry(&self, device_type: &str) -> Option<&KvDeviceRangeEntry> {
        let wanted = device_type.trim();
        self.entries
            .iter()
            .find(|entry| entry.device_type.eq_ignore_ascii_case(wanted))
            .or_else(|| {
                self.entries
                    .iter()
                    .find(|entry| entry.device.eq_ignore_ascii_case(wanted))
            })
            .or_else(|| {
                self.entries.iter().find(|entry| {
                    entry
                        .segments
                        .iter()
                        .any(|segment| segment.device.eq_ignore_ascii_case(wanted))
                })
            })
    }
}

pub fn device_range_catalog_for_plc_profile(
    plc_profile: impl AsRef<str>,
) -> Result<KvDeviceRangeCatalog, HostLinkError> {
    build_catalog(plc_profile.as_ref(), None)
}

fn build_catalog(
    requested_plc_profile: &str,
    model_code: Option<&str>,
) -> Result<KvDeviceRangeCatalog, HostLinkError> {
    let requested_plc_profile = normalize_plc_profile(requested_plc_profile)?;

    let table = range_table()?;
    let resolved_profile = range_profile_for_plc_profile(table, &requested_plc_profile)?;
    let model_index = table
        .profiles
        .iter()
        .position(|profile| profile.name == resolved_profile.name)
        .ok_or_else(|| {
            HostLinkError::protocol(format!(
                "Resolved PLC profile '{}' was not found in the embedded device range table.",
                resolved_profile.name
            ))
        })?;

    let entries = table
        .rows
        .iter()
        .map(|row| build_entry(row, model_index, resolved_profile.source_label))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(KvDeviceRangeCatalog {
        plc_profile: resolved_profile.name.to_owned(),
        model_code: model_code.unwrap_or_default().to_owned(),
        has_model_code: model_code.is_some(),
        requested_plc_profile,
        resolved_plc_profile: resolved_profile.name.to_owned(),
        entries,
    })
}

#[derive(Debug, Clone)]
struct RangeTable {
    profiles: &'static [KvHostLinkPlcProfile],
    rows: Vec<RangeRow>,
}

#[derive(Debug, Clone)]
struct RangeRow {
    device_type: String,
    notation: KvDeviceRangeNotation,
    ranges: Vec<String>,
}

static RANGE_TABLE: OnceLock<RangeTable> = OnceLock::new();

fn range_table() -> Result<&'static RangeTable, HostLinkError> {
    Ok(RANGE_TABLE.get_or_init(create_range_table))
}

fn create_range_table() -> RangeTable {
    RangeTable {
        profiles: plc_profiles(),
        rows: vec![
            row(
                "R",
                KvDeviceRangeNotation::Decimal,
                &[
                    "R00000-R59915",
                    "X0-599F,Y0-599F",
                    "R00000-R99915",
                    "X0-999F,Y0-999F",
                    "R00000-R99915",
                    "X0-999F,Y0-999F",
                    "R00000-R199915",
                    "X0-1999F,Y0-1999F",
                    "R00000-R199915",
                    "X0-1999F,Y0-1999F",
                    "R00000-R199915",
                    "X0-1999F,Y0-1999F",
                ],
            ),
            row(
                "B",
                KvDeviceRangeNotation::Hexadecimal,
                &[
                    "B0000-B1FFF",
                    "B0000-B1FFF",
                    "B0000-B3FFF",
                    "B0000-B3FFF",
                    "B0000-B3FFF",
                    "B0000-B3FFF",
                    "B0000-B7FFF",
                    "B0000-B7FFF",
                    "B0000-B7FFF",
                    "B0000-B7FFF",
                    "B0000-B7FFF",
                    "B0000-B7FFF",
                ],
            ),
            row(
                "MR",
                KvDeviceRangeNotation::Decimal,
                &[
                    "MR00000-MR59915",
                    "M0-9599",
                    "MR00000-MR99915",
                    "M0-15999",
                    "MR00000-MR99915",
                    "M0-15999",
                    "MR000000-MR399915",
                    "M000000-M63999",
                    "MR000000-MR399915",
                    "M000000-M63999",
                    "MR000000-MR399915",
                    "M000000-M63999",
                ],
            ),
            row(
                "LR",
                KvDeviceRangeNotation::Decimal,
                &[
                    "LR00000-LR19915",
                    "L0-3199",
                    "LR00000-LR99915",
                    "L0-15999",
                    "LR00000-LR99915",
                    "L0-15999",
                    "LR00000-LR99915",
                    "L00000-L15999",
                    "LR00000-LR99915",
                    "L00000-L15999",
                    "LR00000-LR99915",
                    "L00000-L15999",
                ],
            ),
            row(
                "CR",
                KvDeviceRangeNotation::Decimal,
                &[
                    "CR0000-CR8915",
                    "CR0000-CR8915",
                    "CR0000-CR3915",
                    "CR0000-CR3915",
                    "CR0000-CR3915",
                    "CR0000-CR3915",
                    "CR0000-CR7915",
                    "CR0000-CR7915",
                    "CR0000-CR7915",
                    "CR0000-CR7915",
                    "CR0000-CR7915",
                    "CR0000-CR7915",
                ],
            ),
            row(
                "CM",
                KvDeviceRangeNotation::Decimal,
                &[
                    "CM0000-CM8999",
                    "CM0000-CM8999",
                    "CM0000-CM5999",
                    "CM0000-CM5999",
                    "CM0000-CM5999",
                    "CM0000-CM5999",
                    "CM0000-CM5999",
                    "CM0000-CM5999",
                    "CM0000-CM7599",
                    "CM0000-CM7599",
                    "CM0000-CM7599",
                    "CM0000-CM7599",
                ],
            ),
            row(
                "T",
                KvDeviceRangeNotation::Decimal,
                &[
                    "T0000-T0511",
                    "T0000-T0511",
                    "T0000-T3999",
                    "T0000-T3999",
                    "T0000-T3999",
                    "T0000-T3999",
                    "T0000-T3999",
                    "T0000-T3999",
                    "T0000-T3999",
                    "T0000-T3999",
                    "T0000-T3999",
                    "T0000-T3999",
                ],
            ),
            row(
                "TC",
                KvDeviceRangeNotation::Decimal,
                &[
                    "TC0000-TC0511",
                    "TC0000-TC0511",
                    "TC0000-TC3999",
                    "TC0000-TC3999",
                    "TC0000-TC3999",
                    "TC0000-TC3999",
                    "TC0000-TC3999",
                    "TC0000-TC3999",
                    "TC0000-TC3999",
                    "TC0000-TC3999",
                    "TC0000-TC3999",
                    "TC0000-TC3999",
                ],
            ),
            row(
                "TS",
                KvDeviceRangeNotation::Decimal,
                &[
                    "TS0000-TS0511",
                    "TS0000-TS0511",
                    "TS0000-TS3999",
                    "TS0000-TS3999",
                    "TS0000-TS3999",
                    "TS0000-TS3999",
                    "TS0000-TS3999",
                    "TS0000-TS3999",
                    "TS0000-TS3999",
                    "TS0000-TS3999",
                    "TS0000-TS3999",
                    "TS0000-TS3999",
                ],
            ),
            row(
                "C",
                KvDeviceRangeNotation::Decimal,
                &[
                    "C0000-C0255",
                    "C0000-C0255",
                    "C0000-C3999",
                    "C0000-C3999",
                    "C0000-C3999",
                    "C0000-C3999",
                    "C0000-C3999",
                    "C0000-C3999",
                    "C0000-C3999",
                    "C0000-C3999",
                    "C0000-C3999",
                    "C0000-C3999",
                ],
            ),
            row(
                "CC",
                KvDeviceRangeNotation::Decimal,
                &[
                    "CC0000-CC0255",
                    "CC0000-CC0255",
                    "CC0000-CC3999",
                    "CC0000-CC3999",
                    "CC0000-CC3999",
                    "CC0000-CC3999",
                    "CC0000-CC3999",
                    "CC0000-CC3999",
                    "CC0000-CC3999",
                    "CC0000-CC3999",
                    "CC0000-CC3999",
                    "CC0000-CC3999",
                ],
            ),
            row(
                "CS",
                KvDeviceRangeNotation::Decimal,
                &[
                    "CS0000-CS0255",
                    "CS0000-CS0255",
                    "CS0000-CS3999",
                    "CS0000-CS3999",
                    "CS0000-CS3999",
                    "CS0000-CS3999",
                    "CS0000-CS3999",
                    "CS0000-CS3999",
                    "CS0000-CS3999",
                    "CS0000-CS3999",
                    "CS0000-CS3999",
                    "CS0000-CS3999",
                ],
            ),
            row(
                "DM",
                KvDeviceRangeNotation::Decimal,
                &[
                    "DM00000-DM32767",
                    "D0-32767",
                    "DM00000-DM65534",
                    "D0-65534",
                    "DM00000-DM65534",
                    "D0-65534",
                    "DM00000-DM65534",
                    "D00000-D65534",
                    "DM00000-DM65534",
                    "D00000-D65534",
                    "DM00000-DM65534",
                    "D00000-D65534",
                ],
            ),
            row(
                "EM",
                KvDeviceRangeNotation::Decimal,
                &[
                    "-",
                    "-",
                    "EM00000-EM65534",
                    "E0-65534",
                    "EM00000-EM65534",
                    "E0-65534",
                    "EM00000-EM65534",
                    "E00000-E65534",
                    "EM00000-EM65534",
                    "E00000-E65534",
                    "EM00000-EM65534",
                    "E00000-E65534",
                ],
            ),
            row(
                "FM",
                KvDeviceRangeNotation::Decimal,
                &[
                    "-",
                    "-",
                    "FM00000-FM32767",
                    "F0-32767",
                    "FM00000-FM32767",
                    "F0-32767",
                    "FM00000-FM32767",
                    "F00000-F32767",
                    "FM00000-FM32767",
                    "F00000-F32767",
                    "FM00000-FM32767",
                    "F00000-F32767",
                ],
            ),
            row(
                "ZF",
                KvDeviceRangeNotation::Decimal,
                &[
                    "-",
                    "-",
                    "ZF000000-ZF131071",
                    "ZF000000-ZF131071",
                    "ZF000000-ZF131071",
                    "ZF000000-ZF131071",
                    "ZF000000-ZF524287",
                    "ZF000000-ZF524287",
                    "ZF000000-ZF524287",
                    "ZF000000-ZF524287",
                    "ZF000000-ZF524287",
                    "ZF000000-ZF524287",
                ],
            ),
            row(
                "W",
                KvDeviceRangeNotation::Hexadecimal,
                &[
                    "W0000-W3FFF",
                    "W0000-W3FFF",
                    "W0000-W3FFF",
                    "W0000-W3FFF",
                    "W0000-W3FFF",
                    "W0000-W3FFF",
                    "W0000-W7FFF",
                    "W0000-W7FFF",
                    "W0000-W7FFF",
                    "W0000-W7FFF",
                    "W0000-W7FFF",
                    "W0000-W7FFF",
                ],
            ),
            row(
                "TM",
                KvDeviceRangeNotation::Decimal,
                &[
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                    "TM000-TM511",
                ],
            ),
            row(
                "VM",
                KvDeviceRangeNotation::Decimal,
                &[
                    "VM0-9999",
                    "VM0-9999",
                    "VM0-59999",
                    "VM0-59999",
                    "VM0-59999",
                    "VM0-59999",
                    "VM0-63999",
                    "VM0-63999",
                    "VM0-589823",
                    "VM0-589823",
                    "-",
                    "-",
                ],
            ),
            row(
                "VB",
                KvDeviceRangeNotation::Hexadecimal,
                &[
                    "VB0-1FFF", "VB0-1FFF", "VB0-3FFF", "VB0-3FFF", "VB0-3FFF", "VB0-3FFF",
                    "VB0-F9FF", "VB0-F9FF", "VB0-F9FF", "VB0-F9FF", "-", "-",
                ],
            ),
            row(
                "Z",
                KvDeviceRangeNotation::Decimal,
                &[
                    "Z1-12", "Z1-12", "Z1-12", "Z1-12", "Z1-12", "Z1-12", "Z1-12", "Z1-12",
                    "Z1-23", "Z1-23", "Z1-10", "Z1-10",
                ],
            ),
            row(
                "CTH",
                KvDeviceRangeNotation::Decimal,
                &[
                    "CTH0-3", "CTH0-3", "CTH0-1", "CTH0-1", "CTH0-1", "CTH0-1", "-", "-", "-", "-",
                    "-", "-",
                ],
            ),
            row(
                "CTC",
                KvDeviceRangeNotation::Decimal,
                &[
                    "CTC0-7", "CTC0-7", "CTC0-3", "CTC0-3", "CTC0-3", "CTC0-3", "-", "-", "-", "-",
                    "-", "-",
                ],
            ),
            row(
                "AT",
                KvDeviceRangeNotation::Decimal,
                &[
                    "-", "-", "AT0-7", "AT0-7", "AT0-7", "AT0-7", "AT0-7", "AT0-7", "AT0-7",
                    "AT0-7", "-", "-",
                ],
            ),
        ],
    }
}

fn row(device_type: &str, notation: KvDeviceRangeNotation, ranges: &[&str]) -> RangeRow {
    RangeRow {
        device_type: device_type.to_owned(),
        notation,
        ranges: ranges.iter().map(|range| (*range).to_owned()).collect(),
    }
}

fn build_entry(
    row: &RangeRow,
    model_index: usize,
    resolved_model: &str,
) -> Result<KvDeviceRangeEntry, HostLinkError> {
    let range_text = row.ranges[model_index].trim();
    let supported = !range_text.is_empty() && range_text != "-";
    let address_range = supported.then(|| range_text.to_owned());
    let segments = match address_range.as_deref() {
        Some(text) => parse_segments(row, text)?,
        None => Vec::new(),
    };
    let primary_device = primary_device_name(row, &segments);
    let (category, is_bit_device) = device_metadata(&primary_device);
    let notation = entry_notation(row.notation, &segments);
    let (lower_bound, upper_bound, point_count) = summarize_entry_bounds(&segments);
    let notes = entry_notes(&segments);

    Ok(KvDeviceRangeEntry {
        device: primary_device,
        device_type: row.device_type.clone(),
        category,
        is_bit_device,
        notation,
        supported,
        lower_bound,
        upper_bound,
        point_count,
        address_range,
        source: format!("Embedded device range table ({resolved_model})"),
        notes,
        segments,
    })
}

fn parse_segments(
    row: &RangeRow,
    range_text: &str,
) -> Result<Vec<KvDeviceRangeSegment>, HostLinkError> {
    range_text
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let device = segment_device(segment);
            let device = if device.is_empty() {
                row.device_type.clone()
            } else {
                device
            };
            let (category, is_bit_device) = device_metadata(&device);
            let notation = notation_for_device(row.notation, &device);
            let (lower_bound, upper_bound, point_count) =
                parse_segment_bounds(segment, notation, &device)?;
            Ok(KvDeviceRangeSegment {
                device,
                category,
                is_bit_device,
                notation,
                lower_bound,
                upper_bound,
                point_count,
                address_range: segment.to_owned(),
            })
        })
        .collect()
}

fn segment_device(segment: &str) -> String {
    segment
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
}

fn primary_device_name(row: &RangeRow, segments: &[KvDeviceRangeSegment]) -> String {
    let unique_devices = segments.iter().map(|segment| segment.device.as_str()).fold(
        Vec::<&str>::new(),
        |mut devices, device| {
            if !devices
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(device))
            {
                devices.push(device);
            }
            devices
        },
    );
    if unique_devices.len() == 1 {
        unique_devices[0].to_owned()
    } else {
        row.device_type.clone()
    }
}

fn summarize_entry_bounds(segments: &[KvDeviceRangeSegment]) -> (u32, Option<u32>, Option<u32>) {
    let Some(first) = segments.first() else {
        return (0, None, None);
    };
    let all_same = segments.iter().skip(1).all(|segment| {
        segment.lower_bound == first.lower_bound
            && segment.upper_bound == first.upper_bound
            && segment.point_count == first.point_count
    });
    if all_same {
        (first.lower_bound, first.upper_bound, first.point_count)
    } else {
        (first.lower_bound, None, None)
    }
}

fn entry_notation(
    fallback: KvDeviceRangeNotation,
    segments: &[KvDeviceRangeSegment],
) -> KvDeviceRangeNotation {
    let Some(first) = segments.first() else {
        return fallback;
    };
    if segments
        .iter()
        .skip(1)
        .all(|segment| segment.notation == first.notation)
    {
        first.notation
    } else {
        fallback
    }
}

fn entry_notes(segments: &[KvDeviceRangeSegment]) -> Option<String> {
    (segments.len() > 1).then(|| {
        "Published address range expands to multiple alias devices; inspect segments.".to_owned()
    })
}

fn parse_segment_bounds(
    segment: &str,
    notation: KvDeviceRangeNotation,
    default_device: &str,
) -> Result<(u32, Option<u32>, Option<u32>), HostLinkError> {
    let Some((start_text, end_text)) = segment.split_once('-') else {
        return Err(HostLinkError::protocol(format!(
            "Invalid device range segment '{segment}': missing '-' separator."
        )));
    };
    let start = parse_segment_number(start_text, notation, default_device).ok_or_else(|| {
        HostLinkError::protocol(format!(
            "Invalid device range start '{start_text}' in segment '{segment}'."
        ))
    })?;
    let end = parse_segment_number(end_text, notation, default_device).ok_or_else(|| {
        HostLinkError::protocol(format!(
            "Invalid device range end '{end_text}' in segment '{segment}'."
        ))
    })?;
    let point_count = end
        .checked_sub(start)
        .and_then(|distance| distance.checked_add(1))
        .ok_or_else(|| {
            HostLinkError::protocol(format!(
                "Invalid device range bounds in segment '{segment}'."
            ))
        })?;
    Ok((start, Some(end), Some(point_count)))
}

fn parse_segment_number(
    text: &str,
    notation: KvDeviceRangeNotation,
    default_device: &str,
) -> Option<u32> {
    let normalized = text.trim();
    let trimmed = normalized
        .strip_prefix(default_device)
        .unwrap_or(normalized)
        .trim_start_matches(|ch: char| ch.is_ascii_alphabetic());
    if trimmed.is_empty() {
        return None;
    }
    if matches!(default_device, "X" | "Y") {
        return parse_xym_segment_number(trimmed);
    }
    match notation {
        KvDeviceRangeNotation::Decimal => trimmed.parse().ok(),
        KvDeviceRangeNotation::Hexadecimal => u32::from_str_radix(trimmed, 16).ok(),
    }
}

fn parse_xym_segment_number(text: &str) -> Option<u32> {
    let (bank_text, bit_text) = text.split_at(text.len().saturating_sub(1));
    if !bank_text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let bank = if bank_text.is_empty() {
        0
    } else {
        bank_text.parse::<u32>().ok()?
    };
    let bit = u32::from_str_radix(bit_text, 16).ok()?;
    bank.checked_mul(16)?.checked_add(bit)
}

fn device_metadata(device_type: &str) -> (KvDeviceRangeCategory, bool) {
    if matches!(device_type, "Z") {
        return (KvDeviceRangeCategory::Index, false);
    }
    if matches!(device_type, "ZF") {
        return (KvDeviceRangeCategory::FileRegister, false);
    }
    if matches!(device_type, "T" | "C" | "AT" | "CTH" | "CTC") {
        return (KvDeviceRangeCategory::TimerCounter, false);
    }
    if is_direct_bit_device_type(device_type) {
        return (KvDeviceRangeCategory::Bit, true);
    }
    match default_format_by_device_type(device_type) {
        "" => (KvDeviceRangeCategory::Bit, true),
        _ => (KvDeviceRangeCategory::Word, false),
    }
}

fn notation_for_device(
    fallback: KvDeviceRangeNotation,
    device_type: &str,
) -> KvDeviceRangeNotation {
    if matches!(device_type, "B" | "W" | "VB" | "X" | "Y") {
        KvDeviceRangeNotation::Hexadecimal
    } else {
        fallback
    }
}

fn range_profile_for_plc_profile<'a>(
    table: &'a RangeTable,
    plc_profile: &str,
) -> Result<&'a KvHostLinkPlcProfile, HostLinkError> {
    let normalized = profile_from_name(plc_profile)?.name;
    for profile in table.profiles {
        if profile.name == normalized {
            return Ok(profile);
        }
    }
    let supported = table
        .profiles
        .iter()
        .map(|profile| profile.name)
        .collect::<Vec<_>>()
        .join(", ");
    Err(HostLinkError::protocol(format!(
        "Unsupported PLC profile '{plc_profile}'. Supported PLC profiles: {supported}."
    )))
}

#[cfg(test)]
mod tests {
    use super::{
        KvDeviceRangeCategory, KvDeviceRangeNotation, device_range_catalog_for_plc_profile,
        parse_segment_bounds,
    };
    use crate::plc_profiles::{available_plc_profiles, display_name, plc_profile_descriptors};

    #[test]
    fn available_profiles_include_xym_profiles() {
        let profiles = available_plc_profiles();
        assert_eq!(
            profiles,
            vec![
                "keyence:kv-nano",
                "keyence:kv-nano-xym",
                "keyence:kv-3000",
                "keyence:kv-3000-xym",
                "keyence:kv-5000",
                "keyence:kv-5000-xym",
                "keyence:kv-7000",
                "keyence:kv-7000-xym",
                "keyence:kv-8000",
                "keyence:kv-8000-xym",
                "keyence:kv-x500",
                "keyence:kv-x500-xym",
            ]
        );
    }

    #[test]
    fn embedded_range_catalog_matches_canonical_fixture() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/kv_device_ranges.json")).unwrap();
        let profiles = fixture["profiles"].as_object().unwrap();
        let profile_ids = available_plc_profiles();

        assert_eq!(profiles.len(), profile_ids.len());
        for profile_id in &profile_ids {
            assert!(profiles.contains_key(profile_id));
            assert!(
                profiles[profile_id]["display_name"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
            );
            assert_eq!(
                display_name(profile_id).unwrap(),
                profiles[profile_id]["display_name"].as_str().unwrap()
            );
        }

        for row in fixture["device_range_rows"].as_array().unwrap() {
            let device_type = row["device_type"].as_str().unwrap();
            let ranges = row["ranges"].as_object().unwrap();
            for profile_id in &profile_ids {
                let expected_range = ranges[profile_id].as_str().unwrap();
                let catalog = device_range_catalog_for_plc_profile(profile_id).unwrap();
                let entry = catalog.entry(device_type).unwrap();
                if expected_range == "-" {
                    assert!(!entry.supported);
                    assert!(entry.address_range.is_none());
                } else {
                    assert!(entry.supported);
                    assert_eq!(entry.address_range.as_deref(), Some(expected_range));
                }
            }
        }
    }

    #[test]
    fn profile_descriptors_match_canonical_profile_metadata() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/kv_device_ranges.json")).unwrap();
        let profiles = fixture["profiles"].as_object().unwrap();
        let descriptors = plc_profile_descriptors();

        assert_eq!(descriptors.len(), profiles.len());
        for descriptor in descriptors {
            let expected = &profiles[descriptor.canonical_name];
            assert_eq!(
                descriptor.display_name,
                expected["display_name"].as_str().unwrap()
            );
            assert!(descriptor.connectable);
            assert_eq!(
                descriptor.base_profile,
                expected
                    .get("base_profile")
                    .and_then(serde_json::Value::as_str)
            );
        }
    }

    #[test]
    fn resolves_plc_profiles_to_range_catalogs() {
        let catalog = device_range_catalog_for_plc_profile("keyence:kv-8000").unwrap();
        assert_eq!(catalog.plc_profile, "keyence:kv-8000");
        assert_eq!(catalog.model_code, "");
        assert!(!catalog.has_model_code);
        assert_eq!(catalog.resolved_plc_profile, "keyence:kv-8000");
        assert_eq!(
            catalog.entry("DM").unwrap().address_range.as_deref(),
            Some("DM00000-DM65534")
        );

        let x_catalog = device_range_catalog_for_plc_profile("keyence:kv-x500").unwrap();
        assert_eq!(x_catalog.resolved_plc_profile, "keyence:kv-x500");
        assert_eq!(
            x_catalog.entry("ZF").unwrap().address_range.as_deref(),
            Some("ZF000000-ZF524287")
        );

        let tm = catalog.entry("TM").unwrap();
        assert_eq!(tm.category, KvDeviceRangeCategory::Word);
        assert!(!tm.is_bit_device);
        assert_eq!(tm.address_range.as_deref(), Some("TM000-TM511"));
    }

    #[test]
    fn xym_catalog_splits_multi_device_ranges_into_segments() {
        let catalog = device_range_catalog_for_plc_profile("keyence:kv-3000-xym").unwrap();
        let entry = catalog.entry("R").unwrap();

        assert_eq!(entry.device, "R");
        assert_eq!(entry.category, KvDeviceRangeCategory::Bit);
        assert!(entry.is_bit_device);
        assert_eq!(entry.notation, KvDeviceRangeNotation::Hexadecimal);
        assert_eq!(entry.lower_bound, 0);
        assert_eq!(entry.upper_bound, Some(999 * 16 + 15));
        assert_eq!(entry.point_count, Some(1_000 * 16));
        assert_eq!(entry.address_range.as_deref(), Some("X0-999F,Y0-999F"));
        assert!(
            entry
                .notes
                .as_deref()
                .unwrap()
                .contains("multiple alias devices")
        );
        assert_eq!(entry.segments.len(), 2);
        assert_eq!(entry.segments[0].device, "X");
        assert_eq!(
            entry.segments[0].notation,
            KvDeviceRangeNotation::Hexadecimal
        );
        assert_eq!(entry.segments[0].lower_bound, 0);
        assert_eq!(entry.segments[0].upper_bound, Some(999 * 16 + 15));
        assert_eq!(entry.segments[0].point_count, Some(1_000 * 16));
        assert_eq!(entry.segments[0].address_range, "X0-999F");
        assert_eq!(entry.segments[1].device, "Y");
        assert_eq!(
            entry.segments[1].notation,
            KvDeviceRangeNotation::Hexadecimal
        );
        assert_eq!(entry.segments[1].lower_bound, 0);
        assert_eq!(entry.segments[1].upper_bound, Some(999 * 16 + 15));
        assert_eq!(entry.segments[1].point_count, Some(1_000 * 16));
        assert_eq!(entry.segments[1].address_range, "Y0-999F");
        assert_eq!(catalog.entry("X").unwrap().device_type, "R");

        let kv8000 = device_range_catalog_for_plc_profile("keyence:kv-8000-xym").unwrap();
        let r = kv8000.entry("R").unwrap();
        assert_eq!(r.upper_bound, Some(1_999 * 16 + 15));
        assert_eq!(r.point_count, Some(2_000 * 16));
        assert_eq!(r.segments[0].upper_bound, Some(1_999 * 16 + 15));
        assert_eq!(r.segments[1].upper_bound, Some(1_999 * 16 + 15));

        let dm = catalog.entry("DM").unwrap();
        assert_eq!(dm.device, "D");
        assert_eq!(dm.category, KvDeviceRangeCategory::Word);
        assert!(!dm.is_bit_device);
        assert_eq!(dm.lower_bound, 0);
        assert_eq!(dm.upper_bound, Some(65534));
        assert_eq!(dm.point_count, Some(65535));
        assert_eq!(dm.notation, KvDeviceRangeNotation::Decimal);
        assert_eq!(dm.segments[0].device, "D");
        assert_eq!(dm.segments[0].address_range, "D0-65534");
        assert_eq!(catalog.entry("D").unwrap().device_type, "DM");

        let fm = catalog.entry("FM").unwrap();
        assert_eq!(fm.device, "F");
        assert_eq!(fm.address_range.as_deref(), Some("F0-32767"));
        assert_eq!(fm.segments[0].device, "F");
        assert_eq!(fm.segments[0].address_range, "F0-32767");
    }

    #[test]
    fn corrected_catalog_typos_are_published_consistently() {
        let nano = device_range_catalog_for_plc_profile("keyence:kv-nano").unwrap();
        assert_eq!(
            nano.entry("CM").unwrap().address_range.as_deref(),
            Some("CM0000-CM8999")
        );

        let xym = device_range_catalog_for_plc_profile("keyence:kv-5000-xym").unwrap();
        assert_eq!(
            xym.entry("CR").unwrap().address_range.as_deref(),
            Some("CR0000-CR3915")
        );

        let kvx = device_range_catalog_for_plc_profile("keyence:kv-x500").unwrap();
        assert_eq!(
            kvx.entry("Z").unwrap().address_range.as_deref(),
            Some("Z1-10")
        );
    }

    #[test]
    fn single_device_ranges_keep_their_device_prefixes() {
        let nano = device_range_catalog_for_plc_profile("keyence:kv-nano").unwrap();
        assert_eq!(
            nano.entry("VM").unwrap().address_range.as_deref(),
            Some("VM0-9999")
        );
        assert_eq!(
            nano.entry("VB").unwrap().address_range.as_deref(),
            Some("VB0-1FFF")
        );
        assert_eq!(
            nano.entry("CTC").unwrap().address_range.as_deref(),
            Some("CTC0-7")
        );

        let kv3000 = device_range_catalog_for_plc_profile("keyence:kv-3000").unwrap();
        assert_eq!(
            kv3000.entry("AT").unwrap().address_range.as_deref(),
            Some("AT0-7")
        );
        assert_eq!(
            kv3000.entry("CTH").unwrap().address_range.as_deref(),
            Some("CTH0-1")
        );

        let kv5000 = device_range_catalog_for_plc_profile("keyence:kv-5000").unwrap();
        assert_eq!(
            kv5000.entry("AT").unwrap().address_range.as_deref(),
            Some("AT0-7")
        );
        assert_eq!(
            kv5000.entry("CTH").unwrap().address_range.as_deref(),
            Some("CTH0-1")
        );
    }

    #[test]
    fn unsupported_entries_remain_present_but_marked_unsupported() {
        let catalog = device_range_catalog_for_plc_profile("keyence:kv-nano").unwrap();
        let em = catalog.entry("EM").unwrap();

        assert!(!em.supported);
        assert!(em.address_range.is_none());
        assert!(em.segments.is_empty());
    }

    #[test]
    fn invalid_range_segment_numbers_are_rejected() {
        let error =
            parse_segment_bounds("DMX-DM10", KvDeviceRangeNotation::Decimal, "DM").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Invalid device range start 'DMX'")
        );
    }

    #[test]
    fn public_profile_input_rejects_legacy_model_names() {
        assert!(device_range_catalog_for_plc_profile("KV-X500").is_err());
        assert!(device_range_catalog_for_plc_profile("KEYENCE:KV-X500").is_err());
        assert!(device_range_catalog_for_plc_profile("keyence:kv-1000").is_err());
    }
}
