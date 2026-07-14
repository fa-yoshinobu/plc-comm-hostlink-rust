use std::sync::Arc;
use std::time::SystemTime;
use time::{Date, Month, OffsetDateTime, Time};

use crate::error::HostLinkError;
use crate::plc_profiles;

/// Immutable lifetime traffic counters for one Host Link client.
///
/// TCP receive bytes count the body and first CR/LF terminator. UDP receive
/// bytes count the complete datagram.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HostLinkTrafficStats {
    pub request_count: u64,
    pub tx_bytes: u64,
    pub rx_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostLinkTransportMode {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostLinkMonitorWord {
    Numeric { device: String, data_format: String },
    DirectBit { device: String },
}

impl HostLinkMonitorWord {
    pub fn numeric(device: impl Into<String>, data_format: impl Into<String>) -> Self {
        Self::Numeric {
            device: device.into(),
            data_format: data_format.into(),
        }
    }

    pub fn direct_bit(device: impl Into<String>) -> Self {
        Self::DirectBit {
            device: device.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvPlcMode {
    Program = 0,
    Run = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum HostLinkTraceDirection {
    Send,
    Receive,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
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
    pub fn now_local() -> Result<Self, HostLinkError> {
        let now = OffsetDateTime::now_local()
            .map_err(|_| HostLinkError::protocol("Local time offset is unavailable"))?;
        Ok(Self::from_offset_datetime(now))
    }

    pub fn validate(&self) -> Result<(), HostLinkError> {
        if self.year > 99 {
            return Err(HostLinkError::protocol(
                "WRT year must be in the range 0..=99",
            ));
        }
        let month = Month::try_from(self.month)
            .map_err(|_| HostLinkError::protocol("Invalid month for WRT command"))?;
        let date = Date::from_calendar_date(2000 + i32::from(self.year), month, self.day).map_err(
            |_| HostLinkError::protocol("WRT command contains a nonexistent calendar date"),
        )?;
        Time::from_hms(self.hour, self.minute, self.second)
            .map_err(|_| HostLinkError::protocol("Invalid time fields for WRT command"))?;
        let expected_week = date.weekday().number_days_from_sunday();
        if self.week != expected_week {
            return Err(HostLinkError::protocol(format!(
                "WRT weekday {} does not match the calendar date (expected {expected_week})",
                self.week
            )));
        }
        Ok(())
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
    use super::{HostLinkClock, HostLinkConnectionOptions};
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

    #[test]
    fn clock_validation_rejects_nonexistent_dates_and_weekday_mismatch() {
        let nonexistent = HostLinkClock {
            year: 26,
            month: 2,
            day: 30,
            hour: 12,
            minute: 0,
            second: 0,
            week: 1,
        };
        assert!(nonexistent.validate().is_err());

        let mut wrong_weekday = clock_for(2026, Month::March, 16);
        wrong_weekday.week = 2;
        assert!(wrong_weekday.validate().is_err());

        let mut year_overflow = clock_for(2099, Month::December, 31);
        year_overflow.year = 100;
        assert!(year_overflow.validate().is_err());
    }

    #[test]
    fn connection_options_require_canonical_plc_profile() {
        let options = HostLinkConnectionOptions::new(
            "127.0.0.1",
            8501,
            super::HostLinkTransportMode::Tcp,
            "keyence:kv-8000",
        )
        .unwrap();

        assert_eq!(options.plc_profile(), "keyence:kv-8000");
        assert!(
            HostLinkConnectionOptions::new(
                "127.0.0.1",
                8501,
                super::HostLinkTransportMode::Tcp,
                ""
            )
            .is_err()
        );
        assert!(
            HostLinkConnectionOptions::new(
                "127.0.0.1",
                8501,
                super::HostLinkTransportMode::Tcp,
                "KV-8000"
            )
            .is_err()
        );
    }
}

#[derive(Debug, Clone)]
pub struct HostLinkConnectionOptions {
    pub host: String,
    pub port: u16,
    pub timeout: std::time::Duration,
    pub transport: HostLinkTransportMode,
    pub(crate) plc_profile: String,
}

impl HostLinkConnectionOptions {
    pub fn new(
        host: impl Into<String>,
        port: u16,
        transport: HostLinkTransportMode,
        plc_profile: impl AsRef<str>,
    ) -> Result<Self, HostLinkError> {
        let host = host.into();
        if host.trim().is_empty() {
            return Err(HostLinkError::protocol("Host must not be empty"));
        }
        if port == 0 {
            return Err(HostLinkError::protocol(
                "Port must be in the range 1..=65535",
            ));
        }
        Ok(Self {
            host: host.trim().to_owned(),
            port,
            timeout: std::time::Duration::from_secs(3),
            transport,
            plc_profile: normalize_plc_profile(plc_profile)?,
        })
    }

    pub fn plc_profile(&self) -> &str {
        &self.plc_profile
    }

    pub fn set_plc_profile(&mut self, plc_profile: impl AsRef<str>) -> Result<(), HostLinkError> {
        self.plc_profile = normalize_plc_profile(plc_profile)?;
        Ok(())
    }
}

fn normalize_plc_profile(plc_profile: impl AsRef<str>) -> Result<String, HostLinkError> {
    plc_profiles::normalize_plc_profile(plc_profile)
}
