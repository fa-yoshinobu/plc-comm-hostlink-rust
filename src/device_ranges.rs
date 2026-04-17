use crate::address::{default_format_by_device_type, is_direct_bit_device_type};
use crate::error::HostLinkError;
use crate::model::KvModelInfo;
use std::sync::OnceLock;

const RANGE_CSV_DATA: &str = r#"DeviceType,Base,KV-NANO,KV-NANO(XYM),KV-3000/5000,KV-3000/5000(XYM),KV-7000,KV-7000(XYM),KV-8000,KV-8000(XYM),KV-X500,KV-X500(XYM)
R,10,R00000-R59915,"X0-599F,Y0-599F",R00000-R99915,"X0-999F,Y0-999F",R00000-R199915,"X0-1999F,Y0-1999F",R00000-R199915,"X0-1999F,Y0-1999F",R00000-R199915,"X0-1999F,Y0-1999F"
B,16,B0000-B1FFF,B0000-B1FFF,B0000-B3FFF,B0000-B3FFF,B0000-B7FFF,B0000-B7FFF,B0000-B7FFF,B0000-B7FFF,B0000-B7FFF,B0000-B7FFF
MR,10,MR00000-MR59915,M0-9599,MR00000-MR99915,M0-15999,MR000000-MR399915,M000000-M63999,MR000000-MR399915,M000000-M63999,MR000000-MR399915,M000000-M63999
LR,10,LR00000-LR19915,L0-3199,LR00000-LR99915,L0-15999,LR00000-LR99915,L00000-L15999,LR00000-LR99915,L00000-L15999,LR00000-LR99915,L00000-L15999
CR,10,CR0000-CR8915,CR0000-CR8915,CR0000-CR3915,CR0000-CR3915,CR0000-CR7915,CR0000-CR7915,CR0000-CR7915,CR0000-CR7915,CR0000-CR7915,CR0000-CR7915
CM,10,CM0000-CM8999,CM0000-CM8999,CM0000-CM5999,CM0000-CM5999,CM0000-CM5999,CM0000-CM5999,CM0000-CM7599,CM0000-CM7599,CM0000-CM7599,CM0000-CM7599
T,10,T0000-T0511,T0000-T0511,T0000-T3999,T0000-T3999,T0000-T3999,T0000-T3999,T0000-T3999,T0000-T3999,T0000-T3999,T0000-T3999
C,10,C0000-C0255,C0000-C0255,C0000-C3999,C0000-C3999,C0000-C3999,C0000-C3999,C0000-C3999,C0000-C3999,C0000-C3999,C0000-C3999
DM,10,DM00000-DM32767,D0-32767,DM00000-DM65534,D0-65534,DM00000-DM65534,D00000-D65534,DM00000-DM65534,D00000-D65534,DM00000-DM65534,D00000-D65534
EM,10,-,-,EM00000-EM65534,E0-65534,EM00000-EM65534,E00000-E65534,EM00000-EM65534,E00000-E65534,EM00000-EM65534,E00000-E65534
FM,10,-,-,FM00000-FM32767,F0-32767,FM00000-FM32767,F00000-F32767,FM00000-FM32767,F00000-F32767,FM00000-FM32767,F00000-F32767
ZF,10,-,-,ZF000000-ZF131071,ZF000000-ZF131071,ZF000000-ZF524287,ZF000000-ZF524287,ZF000000-ZF524287,ZF000000-ZF524287,ZF000000-ZF524287,ZF000000-ZF524287
W,16,W0000-W3FFF,W0000-W3FFF,W0000-W3FFF,W0000-W3FFF,W0000-W7FFF,W0000-W7FFF,W0000-W7FFF,W0000-W7FFF,W0000-W7FFF,W0000-W7FFF
TM,10,TM000-TM511,TM000-TM511,TM000-TM511,TM000-TM511,TM000-TM511,TM000-TM511,TM000-TM511,TM000-TM511,TM000-TM511,TM000-TM511
VM,10,VM0-9499,VM0-9499,VM0-49999,VM0-49999,VM0-63999,VM0-63999,VM0-589823,VM0-589823,-,-
VB,16,VB0-1FFF,VB0-1FFF,VB0-3FFF,VB0-3FFF,VB0-F9FF,VB0-F9FF,VB0-F9FF,VB0-F9FF,-,-
Z,10,Z1-12,Z1-12,Z1-12,Z1-12,Z1-12,Z1-12,Z1-12,Z1-12,-,-
CTH,10,CTH0-3,CTH0-3,CTH0-1,CTH0-3,-,-,-,-,-,-
CTC,10,CTC0-7,CTC0-7,CTC0-3,CTC0-3,-,-,-,-,-,-
AT,10,-,-,AT0-7,AT0-7,AT0-7,AT0-7,AT0-7,AT0-7,-,-
"#;

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
    FileRefresh,
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
    pub model: String,
    pub model_code: String,
    pub has_model_code: bool,
    pub requested_model: String,
    pub resolved_model: String,
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

pub fn device_range_catalog_for_model(
    model: impl AsRef<str>,
) -> Result<KvDeviceRangeCatalog, HostLinkError> {
    build_catalog(model.as_ref(), None)
}

pub(crate) fn device_range_catalog_for_query_model(
    model: &KvModelInfo,
) -> Result<KvDeviceRangeCatalog, HostLinkError> {
    build_catalog(&model.model, Some(&model.code))
}

fn build_catalog(
    requested_model: &str,
    model_code: Option<&str>,
) -> Result<KvDeviceRangeCatalog, HostLinkError> {
    let requested_model = requested_model.trim().to_owned();
    if requested_model.is_empty() {
        return Err(HostLinkError::protocol("Model name must not be empty"));
    }

    let table = range_table()?;
    let resolved_model = resolve_model_column(table, &requested_model)?;
    let model_index = table
        .model_headers
        .iter()
        .position(|header| header == resolved_model)
        .ok_or_else(|| {
            HostLinkError::protocol(format!(
                "Resolved model column '{resolved_model}' was not found in the embedded device range table."
            ))
        })?;

    let entries = table
        .rows
        .iter()
        .map(|row| build_entry(row, model_index, resolved_model))
        .collect::<Vec<_>>();

    Ok(KvDeviceRangeCatalog {
        model: resolved_model.to_owned(),
        model_code: model_code.unwrap_or_default().to_owned(),
        has_model_code: model_code.is_some(),
        requested_model,
        resolved_model: resolved_model.to_owned(),
        entries,
    })
}

pub fn available_device_range_models() -> Vec<String> {
    range_table()
        .map(|table| table.model_headers.clone())
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
struct RangeTable {
    model_headers: Vec<String>,
    rows: Vec<RangeRow>,
}

#[derive(Debug, Clone)]
struct RangeRow {
    device_type: String,
    notation: KvDeviceRangeNotation,
    ranges: Vec<String>,
}

static RANGE_TABLE: OnceLock<Result<RangeTable, String>> = OnceLock::new();

fn range_table() -> Result<&'static RangeTable, HostLinkError> {
    RANGE_TABLE
        .get_or_init(|| parse_range_table().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| HostLinkError::protocol(error.clone()))
}

fn parse_range_table() -> Result<RangeTable, HostLinkError> {
    let mut lines = RANGE_CSV_DATA
        .lines()
        .filter(|line| !line.trim().is_empty());
    let header_line = lines
        .next()
        .ok_or_else(|| HostLinkError::protocol("Embedded device range table is empty"))?;
    let headers = parse_csv_line(header_line)?;
    if headers.len() < 3 {
        return Err(HostLinkError::protocol(
            "Embedded device range table must contain at least DeviceType, Base, and one model column",
        ));
    }

    let model_headers = headers[2..]
        .iter()
        .map(|header| header.trim().to_owned())
        .collect::<Vec<_>>();
    let mut rows = Vec::new();

    for line in lines {
        let fields = parse_csv_line(line)?;
        if fields.len() != headers.len() {
            return Err(HostLinkError::protocol(format!(
                "Embedded device range row has {} columns but {} were expected: {line}",
                fields.len(),
                headers.len()
            )));
        }

        rows.push(RangeRow {
            device_type: fields[0].trim().to_owned(),
            notation: notation_from_base(&fields[1])?,
            ranges: fields[2..]
                .iter()
                .map(|value| value.trim().to_owned())
                .collect(),
        });
    }

    Ok(RangeTable {
        model_headers,
        rows,
    })
}

fn parse_csv_line(line: &str) -> Result<Vec<String>, HostLinkError> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.trim_end_matches('\r').chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next();
                } else {
                    in_quotes = !in_quotes;
                }
            }
            ',' if !in_quotes => {
                fields.push(current);
                current = String::new();
            }
            _ => current.push(ch),
        }
    }

    if in_quotes {
        return Err(HostLinkError::protocol(format!(
            "Embedded device range table contains an unterminated quoted field: {line}"
        )));
    }

    fields.push(current);
    Ok(fields)
}

fn notation_from_base(base_text: &str) -> Result<KvDeviceRangeNotation, HostLinkError> {
    let normalized = base_text.trim();
    if normalized.starts_with("10") {
        Ok(KvDeviceRangeNotation::Decimal)
    } else if normalized.starts_with("16") {
        Ok(KvDeviceRangeNotation::Hexadecimal)
    } else {
        Err(HostLinkError::protocol(format!(
            "Unsupported base cell '{base_text}' in the embedded device range table"
        )))
    }
}

fn build_entry(row: &RangeRow, model_index: usize, resolved_model: &str) -> KvDeviceRangeEntry {
    let range_text = row.ranges[model_index].trim();
    let supported = !range_text.is_empty() && range_text != "-";
    let address_range = supported.then(|| range_text.to_owned());
    let segments = address_range
        .as_deref()
        .map(|text| parse_segments(row, text))
        .unwrap_or_default();
    let primary_device = primary_device_name(row, &segments);
    let (category, is_bit_device) = device_metadata(&primary_device);
    let notation = entry_notation(row.notation, &segments);
    let (lower_bound, upper_bound, point_count) = summarize_entry_bounds(&segments);
    let notes = entry_notes(&segments);

    KvDeviceRangeEntry {
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
    }
}

fn parse_segments(row: &RangeRow, range_text: &str) -> Vec<KvDeviceRangeSegment> {
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
                parse_segment_bounds(segment, notation, &device);
            KvDeviceRangeSegment {
                device,
                category,
                is_bit_device,
                notation,
                lower_bound,
                upper_bound,
                point_count,
                address_range: segment.to_owned(),
            }
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
) -> (u32, Option<u32>, Option<u32>) {
    let Some((start_text, end_text)) = segment.split_once('-') else {
        return (0, None, None);
    };
    let start = parse_segment_number(start_text, notation, default_device);
    let end = parse_segment_number(end_text, notation, default_device);
    let point_count = start
        .zip(end)
        .and_then(|(lower, upper)| upper.checked_sub(lower))
        .and_then(|distance| distance.checked_add(1));
    (start.unwrap_or(0), end, point_count)
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
    match notation {
        KvDeviceRangeNotation::Decimal => trimmed.parse().ok(),
        KvDeviceRangeNotation::Hexadecimal => u32::from_str_radix(trimmed, 16).ok(),
    }
}

fn device_metadata(device_type: &str) -> (KvDeviceRangeCategory, bool) {
    if matches!(device_type, "Z") {
        return (KvDeviceRangeCategory::Index, false);
    }
    if matches!(device_type, "ZF") {
        return (KvDeviceRangeCategory::FileRefresh, false);
    }
    if matches!(device_type, "T" | "C" | "TM" | "AT" | "CTH" | "CTC") {
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

fn resolve_model_column<'a>(
    table: &'a RangeTable,
    requested_model: &str,
) -> Result<&'a str, HostLinkError> {
    let normalized = normalize_model_key(requested_model);
    if let Some(header) = direct_model_match(table, &normalized) {
        return Ok(header);
    }

    let wants_xym = normalized.ends_with("(XYM)");
    let base_model = normalized.strip_suffix("(XYM)").unwrap_or(&normalized);
    let resolved_family = match base_model {
        value if value.starts_with("KV-NANO") || value.starts_with("KV-N") => "KV-NANO",
        value
            if value.starts_with("KV-3000")
                || value.starts_with("KV-5000")
                || value.starts_with("KV-5500") =>
        {
            "KV-3000/5000"
        }
        value
            if value.starts_with("KV-7000")
                || value.starts_with("KV-7300")
                || value.starts_with("KV-7500") =>
        {
            "KV-7000"
        }
        value if value.starts_with("KV-8000") => "KV-8000",
        value if value.starts_with("KV-X5") || value.starts_with("KV-X3") => "KV-X500",
        _ => {
            let supported = table.model_headers.join(", ");
            return Err(HostLinkError::protocol(format!(
                "Unsupported model '{requested_model}'. Supported range models: {supported}."
            )));
        }
    };

    let resolved_key = if wants_xym {
        format!("{resolved_family}(XYM)")
    } else {
        resolved_family.to_owned()
    };

    direct_model_match(table, &resolved_key).ok_or_else(|| {
        HostLinkError::protocol(format!(
            "Resolved model '{resolved_key}' was not found in the embedded device range table."
        ))
    })
}

fn direct_model_match<'a>(table: &'a RangeTable, normalized: &str) -> Option<&'a str> {
    table
        .model_headers
        .iter()
        .find(|header| normalize_model_key(header) == normalized)
        .map(String::as_str)
}

fn normalize_model_key(text: &str) -> String {
    text.trim()
        .trim_end_matches('\0')
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::{
        KvDeviceRangeCategory, KvDeviceRangeNotation, available_device_range_models,
        device_range_catalog_for_model, normalize_model_key,
    };

    #[test]
    fn available_models_include_xym_columns_from_csv() {
        let models = available_device_range_models();
        assert!(models.iter().any(|model| model == "KV-7000"));
        assert!(models.iter().any(|model| model == "KV-7000(XYM)"));
    }

    #[test]
    fn resolves_known_runtime_model_names_to_csv_family_columns() {
        let catalog = device_range_catalog_for_model("KV-8000A").unwrap();
        assert_eq!(catalog.model, "KV-8000");
        assert_eq!(catalog.model_code, "");
        assert!(!catalog.has_model_code);
        assert_eq!(catalog.resolved_model, "KV-8000");
        assert_eq!(
            catalog.entry("DM").unwrap().address_range.as_deref(),
            Some("DM00000-DM65534")
        );

        let x_catalog = device_range_catalog_for_model("KV-X530").unwrap();
        assert_eq!(x_catalog.resolved_model, "KV-X500");
        assert_eq!(
            x_catalog.entry("ZF").unwrap().address_range.as_deref(),
            Some("ZF000000-ZF524287")
        );
    }

    #[test]
    fn xym_catalog_splits_multi_device_ranges_into_segments() {
        let catalog = device_range_catalog_for_model("KV-3000/5000(XYM)").unwrap();
        let entry = catalog.entry("R").unwrap();

        assert_eq!(entry.device, "R");
        assert_eq!(entry.category, KvDeviceRangeCategory::Bit);
        assert!(entry.is_bit_device);
        assert_eq!(entry.notation, KvDeviceRangeNotation::Hexadecimal);
        assert_eq!(entry.lower_bound, 0);
        assert_eq!(entry.upper_bound, Some(0x999F));
        assert_eq!(entry.point_count, Some(0x99A0));
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
        assert_eq!(entry.segments[0].address_range, "X0-999F");
        assert_eq!(entry.segments[1].device, "Y");
        assert_eq!(
            entry.segments[1].notation,
            KvDeviceRangeNotation::Hexadecimal
        );
        assert_eq!(entry.segments[1].address_range, "Y0-999F");
        assert_eq!(catalog.entry("X").unwrap().device_type, "R");

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
        let nano = device_range_catalog_for_model("KV-N24nn").unwrap();
        assert_eq!(
            nano.entry("CM").unwrap().address_range.as_deref(),
            Some("CM0000-CM8999")
        );

        let xym = device_range_catalog_for_model("KV-3000/5000(XYM)").unwrap();
        assert_eq!(
            xym.entry("CR").unwrap().address_range.as_deref(),
            Some("CR0000-CR3915")
        );
    }

    #[test]
    fn single_device_ranges_keep_their_device_prefixes() {
        let nano = device_range_catalog_for_model("KV-N24nn").unwrap();
        assert_eq!(
            nano.entry("VM").unwrap().address_range.as_deref(),
            Some("VM0-9499")
        );
        assert_eq!(
            nano.entry("VB").unwrap().address_range.as_deref(),
            Some("VB0-1FFF")
        );
        assert_eq!(
            nano.entry("CTC").unwrap().address_range.as_deref(),
            Some("CTC0-7")
        );

        let kv3000 = device_range_catalog_for_model("KV-3000/5000").unwrap();
        assert_eq!(
            kv3000.entry("AT").unwrap().address_range.as_deref(),
            Some("AT0-7")
        );
        assert_eq!(
            kv3000.entry("CTH").unwrap().address_range.as_deref(),
            Some("CTH0-1")
        );
    }

    #[test]
    fn unsupported_entries_remain_present_but_marked_unsupported() {
        let catalog = device_range_catalog_for_model("KV-N24nn").unwrap();
        let em = catalog.entry("EM").unwrap();

        assert!(!em.supported);
        assert!(em.address_range.is_none());
        assert!(em.segments.is_empty());
    }

    #[test]
    fn normalize_model_key_removes_whitespace_and_uppercases() {
        assert_eq!(normalize_model_key(" kv-x500 (xym) "), "KV-X500(XYM)");
    }
}
