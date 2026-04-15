use std::sync::Arc;
use std::time::SystemTime;
use time::{Month, OffsetDateTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLinkTransportMode {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvPlcMode {
    Program = 0,
    Run = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLinkTraceDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone)]
pub struct HostLinkTraceFrame {
    pub direction: HostLinkTraceDirection,
    pub data: Vec<u8>,
    pub timestamp: SystemTime,
}

pub type TraceHook = Arc<dyn Fn(HostLinkTraceFrame) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvModelInfo {
    pub code: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostLinkClock {
    pub year: u8,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub week: u8,
}

impl HostLinkClock {
    pub fn now_local() -> Self {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let week = now.weekday().number_days_from_sunday() as u8;
        Self {
            year: (now.year() % 100) as u8,
            month: month_to_number(now.month()),
            day: now.day(),
            hour: now.hour(),
            minute: now.minute(),
            second: now.second(),
            week,
        }
    }
}

fn month_to_number(month: Month) -> u8 {
    match month {
        Month::January => 1,
        Month::February => 2,
        Month::March => 3,
        Month::April => 4,
        Month::May => 5,
        Month::June => 6,
        Month::July => 7,
        Month::August => 8,
        Month::September => 9,
        Month::October => 10,
        Month::November => 11,
        Month::December => 12,
    }
}

#[derive(Debug, Clone)]
pub struct HostLinkConnectionOptions {
    pub host: String,
    pub port: u16,
    pub timeout: std::time::Duration,
    pub transport: HostLinkTransportMode,
    pub append_lf_on_send: bool,
}

impl HostLinkConnectionOptions {
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port: 8501,
            timeout: std::time::Duration::from_secs(3),
            transport: HostLinkTransportMode::Tcp,
            append_lf_on_send: false,
        }
    }
}
