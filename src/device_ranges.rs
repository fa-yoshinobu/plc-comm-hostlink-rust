use crate::error::HostLinkError;
use encoding_rs::SHIFT_JIS;
use std::sync::OnceLock;

const RANGE_CSV_BYTES: &[u8] = include_bytes!("../range.csv");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvDeviceRangeNotation {
    Decimal,
    Hexadecimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvDeviceRangeSegment {
    pub device: String,
    pub address_range: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvDeviceRangeEntry {
    pub device_type: String,
    pub notation: KvDeviceRangeNotation,
    pub supported: bool,
    pub address_range: Option<String>,
    pub segments: Vec<KvDeviceRangeSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvDeviceRangeCatalog {
    pub requested_model: String,
    pub resolved_model: String,
    pub entries: Vec<KvDeviceRangeEntry>,
}

impl KvDeviceRangeCatalog {
    pub fn entry(&self, device_type: &str) -> Option<&KvDeviceRangeEntry> {
        self.entries
            .iter()
            .find(|entry| entry.device_type.eq_ignore_ascii_case(device_type.trim()))
    }
}

pub fn device_range_catalog_for_model(
    model: impl AsRef<str>,
) -> Result<KvDeviceRangeCatalog, HostLinkError> {
    let requested_model = model.as_ref().trim().to_owned();
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
                "Resolved model column '{resolved_model}' was not found in range.csv."
            ))
        })?;

    let entries = table
        .rows
        .iter()
        .map(|row| build_entry(row, model_index))
        .collect::<Vec<_>>();

    Ok(KvDeviceRangeCatalog {
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
    let csv_text = decode_range_csv()?;
    let mut lines = csv_text.lines().filter(|line| !line.trim().is_empty());
    let header_line = lines
        .next()
        .ok_or_else(|| HostLinkError::protocol("range.csv is empty"))?;
    let headers = parse_csv_line(header_line)?;
    if headers.len() < 3 {
        return Err(HostLinkError::protocol(
            "range.csv must contain at least DeviceType, Base, and one model column",
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
                "range.csv row has {} columns but {} were expected: {line}",
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

fn decode_range_csv() -> Result<String, HostLinkError> {
    let (decoded, _, had_errors) = SHIFT_JIS.decode(RANGE_CSV_BYTES);
    if had_errors {
        return Err(HostLinkError::protocol(
            "range.csv could not be decoded as Shift_JIS",
        ));
    }

    Ok(decoded.into_owned())
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
            "range.csv contains an unterminated quoted field: {line}"
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
            "Unsupported base cell '{base_text}' in range.csv"
        )))
    }
}

fn build_entry(row: &RangeRow, model_index: usize) -> KvDeviceRangeEntry {
    let range_text = row.ranges[model_index].trim();
    let supported = !range_text.is_empty() && range_text != "-";
    let address_range = supported.then(|| range_text.to_owned());
    let segments = address_range
        .as_deref()
        .map(parse_segments)
        .unwrap_or_default()
        .into_iter()
        .map(|(device, address_range)| KvDeviceRangeSegment {
            device: if device.is_empty() {
                row.device_type.clone()
            } else {
                device
            },
            address_range,
        })
        .collect();

    KvDeviceRangeEntry {
        device_type: row.device_type.clone(),
        notation: row.notation,
        supported,
        address_range,
        segments,
    }
}

fn parse_segments(range_text: &str) -> Vec<(String, String)> {
    range_text
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(|segment| (segment_device(segment), segment.to_owned()))
        .collect()
}

fn segment_device(segment: &str) -> String {
    segment
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>()
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
            "Resolved model '{resolved_key}' was not found in range.csv."
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
        KvDeviceRangeNotation, available_device_range_models, device_range_catalog_for_model,
        normalize_model_key,
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

        assert_eq!(entry.notation, KvDeviceRangeNotation::Decimal);
        assert_eq!(entry.address_range.as_deref(), Some("X0-999F,Y0-999F"));
        assert_eq!(entry.segments.len(), 2);
        assert_eq!(entry.segments[0].device, "X");
        assert_eq!(entry.segments[0].address_range, "X0-999F");
        assert_eq!(entry.segments[1].device, "Y");
        assert_eq!(entry.segments[1].address_range, "Y0-999F");

        let dm = catalog.entry("DM").unwrap();
        assert_eq!(dm.segments[0].device, "D");
        assert_eq!(dm.segments[0].address_range, "D0-65534");
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
