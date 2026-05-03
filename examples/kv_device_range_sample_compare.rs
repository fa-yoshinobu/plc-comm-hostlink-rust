use plc_comm_hostlink::{
    HostLinkConnectionOptions, HostLinkError, HostLinkValue, KvDeviceAddress, KvDeviceRangeEntry,
    KvDeviceRangeSegment, QueuedHostLinkClient, open_and_connect,
};
use std::collections::BTreeSet;
use std::error::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    Bit,
    Word,
    Dword,
}

#[derive(Debug, Default)]
struct Summary {
    passed: usize,
    read_failed: usize,
    write_failed: usize,
    readback_failed: usize,
    restore_failed: usize,
    skipped: usize,
    unsupported: usize,
}

impl Summary {
    fn is_success(&self) -> bool {
        self.read_failed == 0
            && self.write_failed == 0
            && self.readback_failed == 0
            && self.restore_failed == 0
    }
}

#[derive(Debug)]
struct Failure {
    address: String,
    phase: &'static str,
    message: String,
}

#[derive(Debug)]
struct DeviceReport {
    device: String,
    address_range: String,
    value_kind: Option<ValueKind>,
    sample_addresses: Vec<String>,
    failures: Vec<Failure>,
    passed: usize,
    read_failed: usize,
    write_failed: usize,
    readback_failed: usize,
    restore_failed: usize,
    skipped: usize,
    unsupported: usize,
    untested_reason: Option<String>,
}

impl DeviceReport {
    fn new(entry: &KvDeviceRangeEntry) -> Self {
        Self {
            device: entry.device.clone(),
            address_range: entry
                .address_range
                .clone()
                .unwrap_or_else(|| "n/a".to_owned()),
            value_kind: None,
            sample_addresses: Vec::new(),
            failures: Vec::new(),
            passed: 0,
            read_failed: 0,
            write_failed: 0,
            readback_failed: 0,
            restore_failed: 0,
            skipped: 0,
            unsupported: 0,
            untested_reason: None,
        }
    }

    fn fail(&mut self, address: String, phase: &'static str, message: String) {
        self.failures.push(Failure {
            address,
            phase,
            message,
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().collect::<Vec<_>>();
    let host = args
        .get(1)
        .cloned()
        .or_else(|| std::env::var("HOSTLINK_HOST").ok())
        .unwrap_or_else(|| "192.168.250.100".to_owned());
    let port = args
        .get(2)
        .cloned()
        .or_else(|| std::env::var("HOSTLINK_PORT").ok())
        .unwrap_or_else(|| "8501".to_owned())
        .parse::<u16>()?;
    let sample_points = env_usize("KV_SAMPLE_POINTS", 10);
    let only = env_csv("KV_SAMPLE_ONLY");
    let only_set = only.iter().cloned().collect::<BTreeSet<_>>();

    let mut options = HostLinkConnectionOptions::new(host);
    options.port = port;
    let client = open_and_connect(options).await?;
    let catalog = client.read_device_range_catalog().await?;

    println!(
        "catalog -> model={} resolved_model={} model_code={} sample_points={}",
        catalog.model, catalog.resolved_model, catalog.model_code, sample_points
    );
    if !only.is_empty() {
        println!("only -> {}", only.join(","));
    }

    let mut summary = Summary::default();
    for entry in &catalog.entries {
        if !only_set.is_empty() && !matches_only(entry, &only_set) {
            continue;
        }

        let mut report = DeviceReport::new(entry);
        run_entry(&client, entry, sample_points, &mut summary, &mut report).await;
        print_device_report(&report);
    }

    let _ = client.close().await;
    println!(
        "summary -> passed={} read_failed={} write_failed={} readback_failed={} restore_failed={} skipped={} unsupported={}",
        summary.passed,
        summary.read_failed,
        summary.write_failed,
        summary.readback_failed,
        summary.restore_failed,
        summary.skipped,
        summary.unsupported
    );

    if summary.restore_failed > 0 {
        return Err(make_error("one or more restore operations failed"));
    }
    if !summary.is_success() {
        return Err(make_error("one or more device sample checks failed"));
    }

    Ok(())
}

async fn run_entry(
    client: &QueuedHostLinkClient,
    entry: &KvDeviceRangeEntry,
    sample_points: usize,
    summary: &mut Summary,
    report: &mut DeviceReport,
) {
    if !entry.supported {
        skip_device(summary, report, "unsupported by catalog");
        return;
    }
    if entry.segments.is_empty() {
        skip_device(summary, report, "catalog has no concrete segment");
        return;
    }

    let kind = kind_for(entry);
    report.value_kind = Some(kind);
    let mut any_sample = false;

    for segment in &entry.segments {
        let Some(upper_bound) = effective_upper_bound(segment, kind) else {
            skip_device(summary, report, "open-ended or too narrow range");
            return;
        };
        let lower_bound = effective_lower_bound(segment);
        if lower_bound > upper_bound {
            skip_device(summary, report, "effective sample range is empty");
            return;
        }

        let samples =
            sample_numbers_for_segment(segment, lower_bound, upper_bound, kind, sample_points);
        if samples.is_empty() {
            skip_device(
                summary,
                report,
                "no valid sample address could be generated",
            );
            return;
        }

        for number in samples {
            let address = match format_address(&segment.device, number) {
                Ok(address) => address,
                Err(error) => {
                    summary.unsupported += 1;
                    report.unsupported += 1;
                    report.untested_reason =
                        Some(format!("parser/client does not support {}", segment.device));
                    report.fail(segment.device.clone(), "unsupported", error.to_string());
                    return;
                }
            };

            any_sample = true;
            report.sample_addresses.push(address.clone());
            match exercise_point(client, &address, kind).await {
                Ok(()) => {
                    summary.passed += 1;
                    report.passed += 1;
                }
                Err((phase, message)) if phase == "read" => {
                    summary.read_failed += 1;
                    report.read_failed += 1;
                    report.fail(address, phase, message);
                }
                Err((phase, message)) if phase == "restore" => {
                    summary.restore_failed += 1;
                    report.restore_failed += 1;
                    report.fail(address, phase, message);
                }
                Err((phase, message)) if phase == "readback" => {
                    summary.readback_failed += 1;
                    report.readback_failed += 1;
                    report.fail(address, phase, message);
                }
                Err((phase, message)) => {
                    summary.write_failed += 1;
                    report.write_failed += 1;
                    report.fail(address, phase, message);
                }
            }
        }
    }

    if !any_sample {
        skip_device(
            summary,
            report,
            "no valid sample address could be generated",
        );
    }
}

fn skip_device(summary: &mut Summary, report: &mut DeviceReport, reason: &str) {
    summary.skipped += 1;
    report.skipped += 1;
    report.untested_reason = Some(reason.to_owned());
}

fn print_device_report(report: &DeviceReport) {
    if let Some(reason) = &report.untested_reason {
        if report.sample_addresses.is_empty() {
            println!("UNTESTED {}: {reason}", report.device);
            for failure in &report.failures {
                println!(
                    "FAIL {} {}: {}",
                    failure.phase, failure.address, failure.message
                );
            }
            return;
        }
    }

    println!(
        "DEVICE {} range={} kind={:?} samples={}",
        report.device,
        report.address_range,
        report.value_kind,
        report.sample_addresses.len()
    );
    for address in &report.sample_addresses {
        println!("SAMPLE {address}");
    }
    for failure in &report.failures {
        println!(
            "FAIL {} {}: {}",
            failure.phase, failure.address, failure.message
        );
    }
    println!(
        "DEVICE-SUMMARY {} passed={} read_failed={} write_failed={} readback_failed={} restore_failed={} skipped={} unsupported={}",
        report.device,
        report.passed,
        report.read_failed,
        report.write_failed,
        report.readback_failed,
        report.restore_failed,
        report.skipped,
        report.unsupported
    );
}

async fn exercise_point(
    client: &QueuedHostLinkClient,
    address: &str,
    kind: ValueKind,
) -> Result<(), (&'static str, String)> {
    let original = read_value(client, address, kind)
        .await
        .map_err(|error| ("read", error.to_string()))?;
    let (value_a, value_b) = test_values(address, &original, kind);
    let mut restore_needed = false;

    let test_result: Result<(), (&'static str, String)> = async {
        write_value(client, address, kind, value_a.clone())
            .await
            .map_err(|error| ("write", error.to_string()))?;
        restore_needed = true;
        assert_value(client, address, kind, &value_a).await?;
        write_value(client, address, kind, value_b.clone())
            .await
            .map_err(|error| ("write", error.to_string()))?;
        assert_value(client, address, kind, &value_b).await?;
        Ok(())
    }
    .await;

    let restore_result = if restore_needed {
        Some(write_value(client, address, kind, original).await)
    } else {
        None
    };
    match (test_result, restore_result) {
        (Ok(()), Some(Ok(()))) | (Ok(()), None) => Ok(()),
        (Ok(()), Some(Err(error))) => Err(("restore", error.to_string())),
        (Err((phase, message)), Some(Ok(()))) | (Err((phase, message)), None) => {
            Err((phase, message))
        }
        (Err((_phase, test_error)), Some(Err(restore_error))) => Err((
            "restore",
            format!("{test_error}; restore also failed: {restore_error}"),
        )),
    }
}

async fn assert_value(
    client: &QueuedHostLinkClient,
    address: &str,
    kind: ValueKind,
    expected: &HostLinkValue,
) -> Result<(), (&'static str, String)> {
    let observed = read_value(client, address, kind)
        .await
        .map_err(|error| ("readback", error.to_string()))?;
    if &observed != expected {
        return Err((
            "readback",
            format!("readback mismatch: expected={expected:?} observed={observed:?}"),
        ));
    }
    Ok(())
}

async fn read_value(
    client: &QueuedHostLinkClient,
    address: &str,
    kind: ValueKind,
) -> Result<HostLinkValue, HostLinkError> {
    client.read_typed(address, dtype_for(kind)).await
}

async fn write_value(
    client: &QueuedHostLinkClient,
    address: &str,
    kind: ValueKind,
    value: HostLinkValue,
) -> Result<(), HostLinkError> {
    client.write_typed(address, dtype_for(kind), value).await
}

fn kind_for(entry: &KvDeviceRangeEntry) -> ValueKind {
    if entry.is_bit_device {
        ValueKind::Bit
    } else if matches!(
        entry.device_type.as_str(),
        "T" | "TC" | "TS" | "C" | "CC" | "CS"
    ) {
        ValueKind::Dword
    } else {
        ValueKind::Word
    }
}

fn dtype_for(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Bit => "",
        ValueKind::Word => "U",
        ValueKind::Dword => "D",
    }
}

fn test_values(
    address: &str,
    original: &HostLinkValue,
    kind: ValueKind,
) -> (HostLinkValue, HostLinkValue) {
    match (kind, original) {
        (ValueKind::Bit, HostLinkValue::Bool(value)) => {
            (HostLinkValue::Bool(!*value), HostLinkValue::Bool(*value))
        }
        (ValueKind::Word, HostLinkValue::U16(original)) => {
            let mut a = seeded_u16(address, 0x1111);
            let mut b = seeded_u16(address, 0x2222);
            if a == *original {
                a ^= 0x00FF;
            }
            if b == a {
                b ^= 0xFF00;
            }
            (HostLinkValue::U16(a), HostLinkValue::U16(b))
        }
        (ValueKind::Dword, HostLinkValue::U32(original)) => {
            let mut a = seeded_u32(address, 0x3333);
            let mut b = seeded_u32(address, 0x4444);
            if a == *original {
                a ^= 0x0000_FFFF;
            }
            if b == a {
                b ^= 0xFFFF_0000;
            }
            (HostLinkValue::U32(a), HostLinkValue::U32(b))
        }
        _ => {
            let a = seeded_u16(address, 0x5555);
            let b = seeded_u16(address, 0x6666);
            (HostLinkValue::U16(a), HostLinkValue::U16(b))
        }
    }
}

fn effective_lower_bound(segment: &KvDeviceRangeSegment) -> u32 {
    if segment.device.eq_ignore_ascii_case("R") {
        segment.lower_bound.max(200)
    } else {
        segment.lower_bound
    }
}

fn effective_upper_bound(segment: &KvDeviceRangeSegment, kind: ValueKind) -> Option<u32> {
    let upper = segment.upper_bound?;
    if kind == ValueKind::Dword {
        if upper <= segment.lower_bound {
            return None;
        }
        Some(upper - 1)
    } else {
        Some(upper)
    }
}

fn sample_numbers_for_segment(
    segment: &KvDeviceRangeSegment,
    lower_bound: u32,
    upper_bound: u32,
    kind: ValueKind,
    count: usize,
) -> Vec<u32> {
    let mut selected = BTreeSet::new();
    for number in sample_numbers_between(lower_bound, upper_bound, count) {
        if let Some(number) =
            normalize_sample_number(&segment.device, number, lower_bound, upper_bound, kind)
        {
            selected.insert(number);
        }
    }

    let mut cursor = lower_bound;
    while selected.len() < count && cursor <= upper_bound {
        if let Some(number) =
            normalize_sample_number(&segment.device, cursor, lower_bound, upper_bound, kind)
        {
            selected.insert(number);
        }
        cursor = cursor.saturating_add(1);
        if cursor == u32::MAX {
            break;
        }
    }

    selected.into_iter().collect()
}

fn sample_numbers_between(lower_bound: u32, upper_bound: u32, count: usize) -> Vec<u32> {
    if count == 0 || lower_bound > upper_bound {
        return Vec::new();
    }

    let span = upper_bound - lower_bound;
    if (span as u64) < count as u64 {
        return (lower_bound..=upper_bound).collect();
    }

    let lower = lower_bound as u64;
    let span = span as u64;
    let mut selected = BTreeSet::new();
    for offset in [
        0,
        span,
        span / 2,
        span / 4,
        (span * 3) / 4,
        span / 8,
        (span * 3) / 8,
        (span * 5) / 8,
        (span * 7) / 8,
        1,
        span.saturating_sub(1),
    ] {
        if selected.len() < count {
            selected.insert((lower + offset) as u32);
        }
    }

    for index in 0..count {
        if selected.len() >= count {
            break;
        }
        let denominator = count.saturating_sub(1) as u64;
        let offset = if denominator == 0 {
            0
        } else {
            (index as u64 * span) / denominator
        };
        selected.insert((lower + offset) as u32);
    }

    selected.into_iter().collect()
}

fn normalize_sample_number(
    device_type: &str,
    number: u32,
    lower_bound: u32,
    upper_bound: u32,
    _kind: ValueKind,
) -> Option<u32> {
    let number = number.clamp(lower_bound, upper_bound);
    if !uses_bit_bank_address(device_type) {
        return Some(number);
    }

    nearest_valid_bit_bank_number(number, lower_bound, upper_bound)
}

fn nearest_valid_bit_bank_number(number: u32, lower_bound: u32, upper_bound: u32) -> Option<u32> {
    let bank = number / 100;
    let bit = (number % 100).min(15);
    let mut candidates = Vec::new();
    for candidate_bank in [
        bank,
        bank.saturating_add(1),
        bank.saturating_sub(1),
        lower_bound / 100,
        upper_bound / 100,
    ] {
        for candidate_bit in [bit, 0, 15] {
            let candidate = candidate_bank
                .checked_mul(100)
                .and_then(|base| base.checked_add(candidate_bit))?;
            if candidate >= lower_bound && candidate <= upper_bound && candidate % 100 <= 15 {
                candidates.push(candidate);
            }
        }
    }

    candidates
        .into_iter()
        .min_by_key(|candidate| candidate.abs_diff(number))
}

fn uses_bit_bank_address(device_type: &str) -> bool {
    matches!(device_type, "R" | "MR" | "LR" | "CR")
}

fn format_address(device_type: &str, number: u32) -> Result<String, HostLinkError> {
    KvDeviceAddress {
        device_type: device_type.to_ascii_uppercase(),
        number,
        suffix: String::new(),
    }
    .to_text()
}

fn seeded_u16(label: &str, salt: u32) -> u16 {
    let mut hash = 0x811C9DC5u32 ^ salt;
    for byte in label.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    let value = ((hash & 0xFFFF) as u16) | 1;
    if value == 0 { 1 } else { value }
}

fn seeded_u32(label: &str, salt: u32) -> u32 {
    let high = seeded_u16(label, salt) as u32;
    let low = seeded_u16(label, salt ^ 0xA5A5_5A5A) as u32;
    (high << 16) | low
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

fn env_csv(name: &str) -> Vec<String> {
    std::env::var(name)
        .unwrap_or_default()
        .split(',')
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn matches_only(entry: &KvDeviceRangeEntry, only: &BTreeSet<String>) -> bool {
    only.contains(&entry.device.to_ascii_uppercase())
        || only.contains(&entry.device_type.to_ascii_uppercase())
        || entry
            .segments
            .iter()
            .any(|segment| only.contains(&segment.device.to_ascii_uppercase()))
}

fn make_error(message: impl Into<String>) -> Box<dyn Error> {
    Box::new(std::io::Error::other(message.into()))
}
