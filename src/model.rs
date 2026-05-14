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
        Self::from_offset_datetime(now)
    }

    fn from_offset_datetime(now: OffsetDateTime) -> Self {
        let week = now.weekday().number_days_from_sunday();
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

#[cfg(test)]
mod tests {
    use super::HostLinkClock;
    use time::{Date, Month, Time};

    fn clock_for(year: i32, month: Month, day: u8) -> HostLinkClock {
        let date = Date::from_calendar_date(year, month, day).unwrap();
        let time = Time::from_hms(1, 2, 3).unwrap();
        HostLinkClock::from_offset_datetime(date.with_time(time).assume_utc())
    }

    #[test]
    fn clock_uses_sunday_based_weekday() {
        assert_eq!(clock_for(2026, Month::March, 15).week, 0);
        assert_eq!(clock_for(2026, Month::March, 16).week, 1);
        assert_eq!(clock_for(2026, Month::March, 21).week, 6);
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
