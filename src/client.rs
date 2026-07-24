use crate::address::{
    force_consecutive_device_types, force_device_types, is_direct_bit_device_type,
    mbs_device_types, model_name_for_code, mws_device_types, parse_device, rdc_device_types,
    require_explicit_format, require_no_suffix, validate_device_count, validate_device_span,
    validate_device_type, validate_expansion_buffer_count, validate_expansion_buffer_span,
    wr_device_types, ws_device_types,
};
use crate::error::HostLinkError;
use crate::helpers;
use crate::model::{
    HostLinkClock, HostLinkConnectionOptions, HostLinkMonitorWord, HostLinkTransportMode,
    KvModelInfo, KvPlcMode,
};
use crate::protocol::{
    build_frame, decode_comment_response, decode_response, ensure_success, raw_response_body,
    split_data_tokens,
};
use std::fmt::Write as _;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::Mutex;
use tokio::time::{Instant, timeout_at};

const MAX_TCP_LINE_SIZE: usize = 65_536;
const UDP_RECEIVE_BUFFER_SIZE: usize = MAX_TCP_LINE_SIZE + 2;

pub trait HostLinkPayloadValue {
    fn format_for_suffix(&self, data_format: &str) -> String;

    fn as_integer(&self) -> Option<i128> {
        None
    }

    fn append_to_payload(
        &self,
        data_format: &str,
        output: &mut String,
    ) -> Result<(), HostLinkError> {
        output.push_str(&self.format_for_suffix(data_format));
        Ok(())
    }
}
fn validate_integer_payload(value: i128, data_format: &str) -> Result<(), HostLinkError> {
    let valid = match data_format {
        "" => matches!(value, 0 | 1),
        ".U" | ".H" => (0..=u16::MAX as i128).contains(&value),
        ".S" => (i16::MIN as i128..=i16::MAX as i128).contains(&value),
        ".D" => (0..=u32::MAX as i128).contains(&value),
        ".L" => (i32::MIN as i128..=i32::MAX as i128).contains(&value),
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(HostLinkError::protocol(format!(
            "value {value} is outside the range for data format '{data_format}'"
        )))
    }
}
fn validate_response_tokens(tokens: &[String], data_format: &str) -> Result<(), HostLinkError> {
    if tokens.is_empty() {
        return Err(HostLinkError::protocol("Missing response token"));
    }
    for token in tokens {
        let valid = match data_format {
            "" => matches!(
                token.trim().to_ascii_uppercase().as_str(),
                "0" | "1" | "ON" | "OFF"
            ),
            ".U" => token.parse::<u16>().is_ok(),
            ".S" => token.parse::<i16>().is_ok(),
            ".D" => token.parse::<u32>().is_ok(),
            ".L" => token.parse::<i32>().is_ok(),
            ".H" => {
                (1..=4).contains(&token.len()) && token.bytes().all(|byte| byte.is_ascii_hexdigit())
            }
            _ => false,
        };
        if !valid {
            return Err(HostLinkError::protocol(format!(
                "Invalid response token '{token}' for data format '{data_format}'"
            )));
        }
    }
    Ok(())
}

fn validate_response_token_count(tokens: &[String], expected: usize) -> Result<(), HostLinkError> {
    if tokens.len() != expected {
        return Err(HostLinkError::protocol(format!(
            "Response returned {} values; expected {expected}",
            tokens.len()
        )));
    }
    Ok(())
}

fn read_response_token_count(device_type: &str, data_format: &str) -> usize {
    if matches!(device_type, "T" | "C") {
        return 3;
    }
    if is_direct_bit_device_type(device_type) {
        return match data_format {
            ".U" | ".S" | ".H" => 16,
            ".D" | ".L" => 32,
            _ => 1,
        };
    }
    1
}

macro_rules! impl_payload_for_ints {
    ($($ty:ty),* $(,)?) => {
        $(
            impl HostLinkPayloadValue for $ty {
                fn as_integer(&self) -> Option<i128> {
                    Some(*self as i128)
                }

                fn format_for_suffix(&self, data_format: &str) -> String {
                    let mut value = String::new();
                    let _ = self.append_to_payload(data_format, &mut value);
                    value
                }

                fn append_to_payload(&self, data_format: &str, output: &mut String) -> Result<(), HostLinkError> {
                    validate_integer_payload(*self as i128, data_format)?;
                    if data_format == ".H" {
                        let _ = write!(output, "{:X}", *self as i128);
                    } else {
                        let _ = write!(output, "{}", self);
                    }
                    Ok(())
                }
            }
        )*
    };
}

impl_payload_for_ints!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl HostLinkPayloadValue for f32 {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        self.to_string()
    }

    fn append_to_payload(
        &self,
        _data_format: &str,
        _output: &mut String,
    ) -> Result<(), HostLinkError> {
        Err(HostLinkError::protocol(
            "floating-point values require the high-level F helper",
        ))
    }
}

impl HostLinkPayloadValue for f64 {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        self.to_string()
    }

    fn append_to_payload(
        &self,
        _data_format: &str,
        _output: &mut String,
    ) -> Result<(), HostLinkError> {
        Err(HostLinkError::protocol(
            "floating-point values require the high-level F helper",
        ))
    }
}

impl HostLinkPayloadValue for bool {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        if *self { "1" } else { "0" }.to_owned()
    }

    fn append_to_payload(
        &self,
        data_format: &str,
        output: &mut String,
    ) -> Result<(), HostLinkError> {
        if !data_format.is_empty() {
            return Err(HostLinkError::protocol(
                "bool is valid only for direct-bit access",
            ));
        }
        output.push(if *self { '1' } else { '0' });
        Ok(())
    }
}

impl HostLinkPayloadValue for String {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        self.trim().to_owned()
    }

    fn append_to_payload(
        &self,
        _data_format: &str,
        _output: &mut String,
    ) -> Result<(), HostLinkError> {
        Err(HostLinkError::protocol(
            "text values are not accepted by low-level numeric writes",
        ))
    }
}

impl HostLinkPayloadValue for &str {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        self.trim().to_owned()
    }

    fn append_to_payload(
        &self,
        _data_format: &str,
        _output: &mut String,
    ) -> Result<(), HostLinkError> {
        Err(HostLinkError::protocol(
            "text values are not accepted by low-level numeric writes",
        ))
    }
}

impl<T: HostLinkPayloadValue + ?Sized> HostLinkPayloadValue for &T {
    fn as_integer(&self) -> Option<i128> {
        (*self).as_integer()
    }

    fn format_for_suffix(&self, data_format: &str) -> String {
        (*self).format_for_suffix(data_format)
    }

    fn append_to_payload(
        &self,
        data_format: &str,
        output: &mut String,
    ) -> Result<(), HostLinkError> {
        (*self).append_to_payload(data_format, output)
    }
}

#[derive(Clone)]
pub struct HostLinkClient {
    inner: Arc<Mutex<ClientInner>>,
}

pub struct HostLinkClientFactory;

#[derive(Clone)]
pub struct QueuedHostLinkClient {
    client: HostLinkClient,
    gate: Arc<Mutex<()>>,
}

enum Transport {
    Tcp(TcpStream),
    Udp(UdpSocket),
}

struct ClientInner {
    options: HostLinkConnectionOptions,
    transport: Option<Transport>,
    rx_buf: Vec<u8>,
    rx_start: usize,
    rx_count: usize,
    tcp_read_buf: Vec<u8>,
    udp_read_buf: Vec<u8>,
    monitor_bit_count: Option<usize>,
    monitor_word_count: Option<usize>,
    // Set before the first transport await and cleared only after a complete
    // response. If the future is dropped, the next operation replaces the
    // poisoned transport before sending another request.
    exchange_incomplete: bool,
    traffic_stats: crate::HostLinkTrafficStats,
}

impl HostLinkClient {
    pub fn new(options: HostLinkConnectionOptions) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClientInner {
                options,
                transport: None,
                rx_buf: vec![0u8; 4096],
                rx_start: 0,
                rx_count: 0,
                tcp_read_buf: vec![0u8; 8192],
                udp_read_buf: vec![0u8; UDP_RECEIVE_BUFFER_SIZE],
                monitor_bit_count: None,
                monitor_word_count: None,
                exchange_incomplete: false,
                traffic_stats: crate::HostLinkTrafficStats::default(),
            })),
        }
    }

    pub async fn connect(options: HostLinkConnectionOptions) -> Result<Self, HostLinkError> {
        let client = Self::new(options);
        client.open().await?;
        Ok(client)
    }

    pub async fn open(&self) -> Result<(), HostLinkError> {
        self.inner.lock().await.open().await
    }

    pub async fn close(&self) -> Result<(), HostLinkError> {
        self.inner.lock().await.close();
        Ok(())
    }

    pub async fn is_open(&self) -> bool {
        let inner = self.inner.lock().await;
        inner.transport.is_some() && !inner.exchange_incomplete
    }

    pub async fn timeout(&self) -> Duration {
        self.inner.lock().await.options.timeout
    }

    pub async fn plc_profile(&self) -> String {
        self.inner.lock().await.options.plc_profile.clone()
    }

    pub async fn traffic_stats(&self) -> crate::HostLinkTrafficStats {
        self.inner.lock().await.traffic_stats
    }

    pub async fn set_timeout(&self, timeout: Duration) -> Result<(), HostLinkError> {
        if timeout.is_zero() {
            return Err(HostLinkError::protocol("timeout must be greater than zero"));
        }
        checked_deadline(timeout)?;
        self.inner.lock().await.options.timeout = timeout;
        Ok(())
    }

    #[doc(hidden)]
    pub async fn send_raw(&self, body: &str) -> Result<Vec<u8>, HostLinkError> {
        self.inner.lock().await.send_raw_bytes(body).await
    }

    pub(crate) async fn send_decoded(&self, body: &str) -> Result<String, HostLinkError> {
        self.inner.lock().await.send_decoded(body).await
    }

    pub async fn change_mode(&self, mode: KvPlcMode) -> Result<(), HostLinkError> {
        self.expect_ok(&format!("M{}", mode as u8)).await
    }

    pub async fn clear_error(&self) -> Result<(), HostLinkError> {
        self.expect_ok("ER").await
    }

    pub async fn check_error_no(&self) -> Result<String, HostLinkError> {
        self.send_decoded("?E").await
    }

    pub async fn query_model(&self) -> Result<KvModelInfo, HostLinkError> {
        let code = self.send_decoded("?K").await?;
        Ok(KvModelInfo {
            model: model_name_for_code(&code).to_owned(),
            code,
        })
    }

    pub async fn confirm_operating_mode(&self) -> Result<KvPlcMode, HostLinkError> {
        let mut inner = self.inner.lock().await;
        let response = inner.send_decoded("?M").await?;
        match response.parse::<u8>() {
            Ok(0) => Ok(KvPlcMode::Program),
            Ok(1) => Ok(KvPlcMode::Run),
            _ => {
                inner.close();
                Err(HostLinkError::protocol("Unsupported PLC mode response"))
            }
        }
    }

    pub async fn set_time(&self, value: HostLinkClock) -> Result<(), HostLinkError> {
        value.validate()?;

        self.expect_ok(&format!(
            "WRT {:02} {:02} {:02} {:02} {:02} {:02} {}",
            value.year, value.month, value.day, value.hour, value.minute, value.second, value.week
        ))
        .await
    }

    pub async fn forced_set(&self, device: &str) -> Result<(), HostLinkError> {
        let address = parse_device(device)?;
        validate_device_type("ST", &address.device_type, force_device_types())?;
        require_no_suffix(&address, "ST")?;
        self.expect_ok(&format!("ST {}", address.to_text()?)).await
    }

    pub async fn forced_reset(&self, device: &str) -> Result<(), HostLinkError> {
        let address = parse_device(device)?;
        validate_device_type("RS", &address.device_type, force_device_types())?;
        require_no_suffix(&address, "RS")?;
        self.expect_ok(&format!("RS {}", address.to_text()?)).await
    }

    pub async fn read(
        &self,
        device: &str,
        data_format: Option<&str>,
    ) -> Result<Vec<String>, HostLinkError> {
        let mut address = parse_device(device)?;
        let suffix = require_explicit_format(&address, data_format)?;
        validate_device_span(&address.device_type, address.number, &suffix, 1)?;
        address.suffix = suffix.clone();
        let response = self
            .send_decoded(&format!("RD {}", address.to_text()?))
            .await?;
        let tokens = split_data_tokens(&response);
        let expected = read_response_token_count(&address.device_type, &suffix);
        if let Err(error) = validate_response_token_count(&tokens, expected) {
            self.close().await?;
            return Err(error);
        }
        if let Err(error) = validate_response_tokens(&tokens, &suffix) {
            self.close().await?;
            return Err(error);
        }
        Ok(tokens)
    }

    pub async fn read_consecutive(
        &self,
        device: &str,
        count: usize,
        data_format: Option<&str>,
    ) -> Result<Vec<String>, HostLinkError> {
        let mut address = parse_device(device)?;
        let suffix = require_explicit_format(&address, data_format)?;
        validate_device_count(&address.device_type, &suffix, count)?;
        validate_device_span(&address.device_type, address.number, &suffix, count)?;
        address.suffix = suffix.clone();
        let response = self
            .send_decoded(&format!("RDS {} {}", address.to_text()?, count))
            .await?;
        let tokens = split_data_tokens(&response);
        if let Err(error) = validate_response_token_count(&tokens, count) {
            self.close().await?;
            return Err(error);
        }
        if let Err(error) = validate_response_tokens(&tokens, &suffix) {
            self.close().await?;
            return Err(error);
        }
        Ok(tokens)
    }

    pub async fn write<T: HostLinkPayloadValue>(
        &self,
        device: &str,
        value: T,
        data_format: Option<&str>,
    ) -> Result<(), HostLinkError> {
        let mut address = parse_device(device)?;
        let suffix = require_explicit_format(&address, data_format)?;
        validate_device_type("WR", &address.device_type, wr_device_types())?;
        validate_device_span(&address.device_type, address.number, &suffix, 1)?;
        address.suffix = suffix.clone();
        let mut command = String::from("WR ");
        command.push_str(&address.to_text()?);
        command.push(' ');
        value.append_to_payload(&suffix, &mut command)?;
        self.expect_ok(&command).await
    }

    pub async fn write_consecutive<T: HostLinkPayloadValue>(
        &self,
        device: &str,
        values: &[T],
        data_format: Option<&str>,
    ) -> Result<(), HostLinkError> {
        if values.is_empty() {
            return Err(HostLinkError::protocol("values must not be empty"));
        }

        let mut address = parse_device(device)?;
        let suffix = require_explicit_format(&address, data_format)?;
        validate_device_type("WRS", &address.device_type, wr_device_types())?;
        validate_device_count(&address.device_type, &suffix, values.len())?;
        validate_device_span(&address.device_type, address.number, &suffix, values.len())?;
        address.suffix = suffix.clone();
        let payload = build_joined_payload(values, &suffix)?;
        self.expect_ok(&format!(
            "WRS {} {} {}",
            address.to_text()?,
            values.len(),
            payload
        ))
        .await
    }

    pub async fn register_monitor_bits<S: AsRef<str>>(
        &self,
        devices: &[S],
    ) -> Result<(), HostLinkError> {
        if devices.is_empty() {
            return Err(HostLinkError::protocol("At least one device is required"));
        }
        if devices.len() > 120 {
            return Err(HostLinkError::protocol(
                "Maximum 120 devices can be registered",
            ));
        }

        let mut command = String::from("MBS");
        for device in devices {
            let address = parse_device(device.as_ref())?;
            validate_device_type("MBS", &address.device_type, mbs_device_types())?;
            require_no_suffix(&address, "MBS")?;
            command.push(' ');
            command.push_str(&address.to_text()?);
        }
        let mut inner = self.inner.lock().await;
        let response = inner.send_decoded(&command).await?;
        if response != "OK" {
            let error = HostLinkError::protocol(format!("Expected OK but received {response}"));
            inner.close();
            return Err(error);
        }
        inner.monitor_bit_count = Some(devices.len());
        Ok(())
    }

    pub async fn register_monitor_words(
        &self,
        devices: &[HostLinkMonitorWord],
    ) -> Result<(), HostLinkError> {
        if devices.is_empty() {
            return Err(HostLinkError::protocol("At least one device is required"));
        }
        if devices.len() > 120 {
            return Err(HostLinkError::protocol(
                "Maximum 120 devices can be registered",
            ));
        }

        let mut command = String::from("MWS");
        for device in devices {
            let (device, data_format) = match device {
                HostLinkMonitorWord::Numeric {
                    device,
                    data_format,
                } => (device.as_str(), Some(data_format.as_str())),
                HostLinkMonitorWord::DirectBit { device } => (device.as_str(), None),
            };
            let mut address = parse_device(device)?;
            validate_device_type("MWS", &address.device_type, mws_device_types())?;
            let suffix = require_explicit_format(&address, data_format)?;
            validate_device_span(&address.device_type, address.number, &suffix, 1)?;
            address.suffix = suffix.clone();
            command.push(' ');
            command.push_str(&address.to_text()?);
        }
        let mut inner = self.inner.lock().await;
        let response = inner.send_decoded(&command).await?;
        if response != "OK" {
            let error = HostLinkError::protocol(format!("Expected OK but received {response}"));
            inner.close();
            return Err(error);
        }
        inner.monitor_word_count = Some(devices.len());
        Ok(())
    }

    pub async fn read_monitor_bits(&self) -> Result<Vec<String>, HostLinkError> {
        let mut inner = self.inner.lock().await;
        let expected = inner
            .monitor_bit_count
            .ok_or_else(|| HostLinkError::protocol("Monitor bits must be registered before MBR"))?;
        let response = inner.send_decoded("MBR").await?;
        let tokens = split_data_tokens(&response);
        if let Err(error) = validate_response_token_count(&tokens, expected) {
            inner.close();
            return Err(error);
        }
        if let Err(error) = validate_response_tokens(&tokens, "") {
            inner.close();
            return Err(error);
        }
        Ok(tokens)
    }

    pub async fn read_monitor_words(&self) -> Result<Vec<String>, HostLinkError> {
        let mut inner = self.inner.lock().await;
        let expected = inner.monitor_word_count.ok_or_else(|| {
            HostLinkError::protocol("Monitor words must be registered before MWR")
        })?;
        let response = inner.send_decoded("MWR").await?;
        let tokens = split_data_tokens(&response);
        if let Err(error) = validate_response_token_count(&tokens, expected) {
            inner.close();
            return Err(error);
        }
        Ok(tokens)
    }

    pub async fn forced_set_consecutive(
        &self,
        device: &str,
        count: usize,
    ) -> Result<(), HostLinkError> {
        if !(1..=16).contains(&count) {
            return Err(HostLinkError::protocol("count must be 1-16."));
        }
        let address = parse_device(device)?;
        validate_device_type(
            "STS",
            &address.device_type,
            force_consecutive_device_types(),
        )?;
        require_no_suffix(&address, "STS")?;
        self.expect_ok(&format!("STS {} {}", address.to_text()?, count))
            .await
    }

    pub async fn forced_reset_consecutive(
        &self,
        device: &str,
        count: usize,
    ) -> Result<(), HostLinkError> {
        if !(1..=16).contains(&count) {
            return Err(HostLinkError::protocol("count must be 1-16."));
        }
        let address = parse_device(device)?;
        validate_device_type(
            "RSS",
            &address.device_type,
            force_consecutive_device_types(),
        )?;
        require_no_suffix(&address, "RSS")?;
        self.expect_ok(&format!("RSS {} {}", address.to_text()?, count))
            .await
    }

    pub async fn read_consecutive_legacy(
        &self,
        device: &str,
        count: usize,
        data_format: Option<&str>,
    ) -> Result<Vec<String>, HostLinkError> {
        let mut address = parse_device(device)?;
        let suffix = require_explicit_format(&address, data_format)?;
        validate_device_count(&address.device_type, &suffix, count)?;
        validate_device_span(&address.device_type, address.number, &suffix, count)?;
        address.suffix = suffix.clone();
        let response = self
            .send_decoded(&format!("RDE {} {}", address.to_text()?, count))
            .await?;
        let tokens = split_data_tokens(&response);
        if let Err(error) = validate_response_token_count(&tokens, count) {
            self.close().await?;
            return Err(error);
        }
        if let Err(error) = validate_response_tokens(&tokens, &suffix) {
            self.close().await?;
            return Err(error);
        }
        Ok(tokens)
    }

    pub async fn write_consecutive_legacy<T: HostLinkPayloadValue>(
        &self,
        device: &str,
        values: &[T],
        data_format: Option<&str>,
    ) -> Result<(), HostLinkError> {
        if values.is_empty() {
            return Err(HostLinkError::protocol("values must not be empty"));
        }
        let mut address = parse_device(device)?;
        let suffix = require_explicit_format(&address, data_format)?;
        validate_device_type("WRE", &address.device_type, wr_device_types())?;
        validate_device_count(&address.device_type, &suffix, values.len())?;
        validate_device_span(&address.device_type, address.number, &suffix, values.len())?;
        address.suffix = suffix.clone();
        let payload = build_joined_payload(values, &suffix)?;
        self.expect_ok(&format!(
            "WRE {} {} {}",
            address.to_text()?,
            values.len(),
            payload
        ))
        .await
    }

    pub async fn write_set_value<T: HostLinkPayloadValue>(
        &self,
        device: &str,
        value: T,
        data_format: Option<&str>,
    ) -> Result<(), HostLinkError> {
        let mut address = parse_device(device)?;
        validate_device_type("WS", &address.device_type, ws_device_types())?;
        let suffix = require_explicit_format(&address, data_format)?;
        validate_device_count(&address.device_type, &suffix, 1)?;
        validate_device_span(&address.device_type, address.number, &suffix, 1)?;
        address.suffix = suffix.clone();
        let mut command = String::from("WS ");
        command.push_str(&address.to_text()?);
        command.push(' ');
        value.append_to_payload(&suffix, &mut command)?;
        self.expect_ok(&command).await
    }

    pub async fn write_set_value_consecutive<T: HostLinkPayloadValue>(
        &self,
        device: &str,
        values: &[T],
        data_format: Option<&str>,
    ) -> Result<(), HostLinkError> {
        if values.is_empty() {
            return Err(HostLinkError::protocol("values must not be empty"));
        }
        let mut address = parse_device(device)?;
        validate_device_type("WSS", &address.device_type, ws_device_types())?;
        let suffix = require_explicit_format(&address, data_format)?;
        validate_device_count(&address.device_type, &suffix, values.len())?;
        validate_device_span(&address.device_type, address.number, &suffix, values.len())?;
        address.suffix = suffix.clone();
        let payload = build_joined_payload(values, &suffix)?;
        self.expect_ok(&format!(
            "WSS {} {} {}",
            address.to_text()?,
            values.len(),
            payload
        ))
        .await
    }

    pub async fn switch_bank(&self, bank_no: u8) -> Result<(), HostLinkError> {
        if bank_no > 15 {
            return Err(HostLinkError::protocol("bankNo must be 0-15."));
        }
        self.expect_ok(&format!("BE {bank_no}")).await
    }

    pub async fn read_expansion_unit_buffer(
        &self,
        unit_no: u8,
        address: u32,
        count: usize,
        data_format: &str,
    ) -> Result<Vec<String>, HostLinkError> {
        if unit_no > 48 {
            return Err(HostLinkError::protocol("unitNo must be 0-48."));
        }
        if address > 59_999 {
            return Err(HostLinkError::protocol("address must be 0-59999."));
        }
        if data_format.trim().is_empty() {
            return Err(HostLinkError::protocol("data format must not be empty"));
        }
        let suffix = crate::address::normalize_suffix(data_format)?;
        validate_expansion_buffer_count(&suffix, count)?;
        validate_expansion_buffer_span(address, &suffix, count)?;
        let response = self
            .send_decoded(&format!("URD {unit_no:02} {address}{suffix} {count}"))
            .await?;
        let tokens = split_data_tokens(&response);
        if let Err(error) = validate_response_token_count(&tokens, count) {
            self.close().await?;
            return Err(error);
        }
        if let Err(error) = validate_response_tokens(&tokens, &suffix) {
            self.close().await?;
            return Err(error);
        }
        Ok(tokens)
    }

    pub async fn write_expansion_unit_buffer<T: HostLinkPayloadValue>(
        &self,
        unit_no: u8,
        address: u32,
        values: &[T],
        data_format: &str,
    ) -> Result<(), HostLinkError> {
        if values.is_empty() {
            return Err(HostLinkError::protocol("values must not be empty"));
        }
        if unit_no > 48 {
            return Err(HostLinkError::protocol("unitNo must be 0-48."));
        }
        if address > 59_999 {
            return Err(HostLinkError::protocol("address must be 0-59999."));
        }
        if data_format.trim().is_empty() {
            return Err(HostLinkError::protocol("data format must not be empty"));
        }
        let suffix = crate::address::normalize_suffix(data_format)?;
        validate_expansion_buffer_count(&suffix, values.len())?;
        validate_expansion_buffer_span(address, &suffix, values.len())?;
        let payload = build_joined_payload(values, &suffix)?;
        self.expect_ok(&format!(
            "UWR {unit_no:02} {address}{suffix} {} {payload}",
            values.len()
        ))
        .await
    }

    pub async fn read_comments(&self, device: &str) -> Result<String, HostLinkError> {
        let address = parse_device(device)?;
        validate_device_type("RDC", &address.device_type, rdc_device_types())?;
        require_no_suffix(&address, "RDC")?;
        let response = self
            .inner
            .lock()
            .await
            .send_decoded_with(
                &format!("RDC {}", address.to_text()?),
                decode_comment_response,
            )
            .await?;
        Ok(response)
    }

    pub async fn read_typed(
        &self,
        device: &str,
        dtype: &str,
    ) -> Result<helpers::HostLinkValue, HostLinkError> {
        helpers::read_typed(self, device, dtype).await
    }

    pub async fn read_timer_counter(
        &self,
        device: &str,
    ) -> Result<helpers::TimerCounterValue, HostLinkError> {
        helpers::read_timer_counter(self, device).await
    }

    pub async fn read_timer(
        &self,
        device: &str,
    ) -> Result<helpers::TimerCounterValue, HostLinkError> {
        helpers::read_timer(self, device).await
    }

    pub async fn read_counter(
        &self,
        device: &str,
    ) -> Result<helpers::TimerCounterValue, HostLinkError> {
        helpers::read_counter(self, device).await
    }

    pub async fn write_typed<T: HostLinkPayloadValue>(
        &self,
        device: &str,
        dtype: &str,
        value: T,
    ) -> Result<(), HostLinkError> {
        helpers::write_typed(self, device, dtype, &value).await
    }

    pub async fn read_named<S: AsRef<str>>(
        &self,
        addresses: &[S],
    ) -> Result<helpers::NamedSnapshot, HostLinkError> {
        helpers::read_named(self, addresses).await
    }

    pub async fn write_bit_in_word(
        &self,
        device: &str,
        bit_index: u8,
        value: bool,
    ) -> Result<(), HostLinkError> {
        if bit_index > 15 {
            return Err(HostLinkError::protocol("bitIndex must be 0-15."));
        }
        let mut address = parse_device(device)?;
        let suffix = require_explicit_format(&address, Some("U"))?;
        validate_device_span(&address.device_type, address.number, &suffix, 1)?;
        address.suffix = suffix.clone();
        let address_text = address.to_text()?;

        let mut inner = self.inner.lock().await;
        let response = inner.send_decoded(&format!("RD {address_text}")).await?;
        let tokens = split_data_tokens(&response);
        let current = if is_direct_bit_device_type(&address.device_type) {
            helpers::pack_direct_bit_tokens(&tokens, 16, device)? as u16
        } else {
            if tokens.len() != 1 {
                return Err(HostLinkError::protocol(
                    "Bit-in-word read did not return exactly one unsigned word",
                ));
            }
            tokens[0]
                .parse::<u16>()
                .map_err(|_| HostLinkError::protocol("Invalid unsigned 16-bit response"))?
        };
        let next = if value {
            current | (1 << bit_index)
        } else {
            current & !(1 << bit_index)
        };
        let response = inner
            .send_decoded(&format!("WR {address_text} {next}"))
            .await?;
        if response == "OK" {
            Ok(())
        } else {
            let error = HostLinkError::protocol(format!(
                "Expected 'OK' but received '{response}' for bit-in-word write"
            ));
            inner.close();
            Err(error)
        }
    }

    async fn expect_ok(&self, body: &str) -> Result<(), HostLinkError> {
        let mut inner = self.inner.lock().await;
        let response = inner.send_decoded(body).await?;
        if response == "OK" {
            Ok(())
        } else {
            let error = HostLinkError::protocol(format!(
                "Expected 'OK' but received '{response}' for command '{body}'"
            ));
            inner.close();
            Err(error)
        }
    }
}

impl ClientInner {
    async fn open(&mut self) -> Result<(), HostLinkError> {
        if self.exchange_incomplete {
            self.close();
        }
        if self.transport.is_some() {
            return Ok(());
        }
        if self.options.timeout.is_zero() {
            return Err(HostLinkError::protocol("timeout must be greater than zero"));
        }
        if self.options.host.trim().is_empty() || self.options.port == 0 {
            return Err(HostLinkError::protocol("invalid Host Link endpoint"));
        }
        let connect_deadline = checked_deadline(self.options.timeout)?;

        let transport = match self.options.transport {
            HostLinkTransportMode::Tcp => {
                let stream = timeout_at(
                    connect_deadline,
                    TcpStream::connect((self.options.host.as_str(), self.options.port)),
                )
                .await
                .map_err(|_| HostLinkError::connection("tcp connect timed out"))??;
                stream.set_nodelay(true)?;
                Transport::Tcp(stream)
            }
            HostLinkTransportMode::Udp => {
                let socket = UdpSocket::bind("0.0.0.0:0").await?;
                timeout_at(
                    connect_deadline,
                    socket.connect((self.options.host.as_str(), self.options.port)),
                )
                .await
                .map_err(|_| HostLinkError::connection("udp connect timed out"))??;
                Transport::Udp(socket)
            }
        };

        self.transport = Some(transport);
        self.rx_start = 0;
        self.rx_count = 0;
        Ok(())
    }

    fn close(&mut self) {
        self.transport = None;
        self.rx_start = 0;
        self.rx_count = 0;
        self.exchange_incomplete = false;
        self.monitor_bit_count = None;
        self.monitor_word_count = None;
    }

    async fn send_raw_bytes(&mut self, body: &str) -> Result<Vec<u8>, HostLinkError> {
        let raw = self.exchange_raw(body).await?;
        Ok(raw_response_body(&raw))
    }

    async fn send_decoded(&mut self, body: &str) -> Result<String, HostLinkError> {
        self.send_decoded_with(body, decode_response).await
    }

    async fn send_decoded_with<F>(
        &mut self,
        body: &str,
        decoder: F,
    ) -> Result<String, HostLinkError>
    where
        F: Fn(&[u8]) -> Result<String, HostLinkError>,
    {
        let raw = self.exchange_raw(body).await?;
        let decoded = match decoder(&raw) {
            Ok(decoded) => decoded,
            Err(error) => {
                self.close();
                return Err(error);
            }
        };
        ensure_success(decoded)
    }

    async fn exchange_raw(&mut self, body: &str) -> Result<Vec<u8>, HostLinkError> {
        let frame = build_frame(body)?;
        if self.exchange_incomplete {
            self.close();
        }
        if self.transport.is_none() {
            return Err(HostLinkError::NotConnected);
        }
        let deadline = checked_deadline(self.options.timeout)?;
        self.exchange_incomplete = true;
        let exchange_result = match self.transport.as_mut() {
            Some(Transport::Tcp(stream)) => {
                match write_all_with_timeout(stream, &frame, deadline).await {
                    Ok(()) => {
                        self.traffic_stats.request_count += 1;
                        self.traffic_stats.tx_bytes += frame.len() as u64;
                        recv_tcp_line(
                            stream,
                            &mut self.rx_buf,
                            &mut self.rx_start,
                            &mut self.rx_count,
                            &mut self.tcp_read_buf,
                            deadline,
                        )
                        .await
                    }
                    Err(err) => Err(err),
                }
            }
            Some(Transport::Udp(socket)) => {
                match send_udp_with_timeout(socket, &frame, deadline).await {
                    Ok(()) => {
                        self.traffic_stats.request_count += 1;
                        self.traffic_stats.tx_bytes += frame.len() as u64;
                        match recv_udp_with_timeout(socket, &mut self.udp_read_buf, deadline).await
                        {
                            Ok(()) if matches!(self.udp_read_buf.last(), Some(b'\r' | b'\n')) => {
                                let raw = self.udp_read_buf.clone();
                                let counted_len = raw.len();
                                Ok((raw, counted_len))
                            }
                            Ok(()) => Err(HostLinkError::protocol(
                                "UDP response is missing the required CR/LF terminator",
                            )),
                            Err(error) => Err(error),
                        }
                    }
                    Err(err) => Err(err),
                }
            }
            None => Err(HostLinkError::connection("transport was not opened")),
        };

        match exchange_result {
            Ok((raw, counted_len)) => {
                self.traffic_stats.rx_bytes += counted_len as u64;
                if raw_response_body(&raw).len() > MAX_TCP_LINE_SIZE {
                    self.close();
                    return Err(HostLinkError::protocol(format!(
                        "Response line exceeds {MAX_TCP_LINE_SIZE} bytes"
                    )));
                }
                self.exchange_incomplete = false;
                Ok(raw)
            }
            Err(err) => {
                self.close();
                Err(err)
            }
        }
    }
}

impl HostLinkClientFactory {
    pub async fn open_and_connect(
        options: HostLinkConnectionOptions,
    ) -> Result<QueuedHostLinkClient, HostLinkError> {
        if options.host.trim().is_empty() {
            return Err(HostLinkError::protocol("Host must not be empty."));
        }

        let client = HostLinkClient::new(options);
        let queued = QueuedHostLinkClient::new(client);
        queued.open().await?;
        Ok(queued)
    }
}

pub async fn open_and_connect(
    options: HostLinkConnectionOptions,
) -> Result<QueuedHostLinkClient, HostLinkError> {
    HostLinkClientFactory::open_and_connect(options).await
}

impl QueuedHostLinkClient {
    pub fn new(client: HostLinkClient) -> Self {
        Self {
            client,
            gate: Arc::new(Mutex::new(())),
        }
    }

    pub fn inner_client(&self) -> &HostLinkClient {
        &self.client
    }

    pub async fn is_open(&self) -> bool {
        self.client.is_open().await
    }

    pub async fn traffic_stats(&self) -> crate::HostLinkTrafficStats {
        self.client.traffic_stats().await
    }

    pub async fn open(&self) -> Result<(), HostLinkError> {
        let _guard = self.gate.lock().await;
        self.client.open().await
    }

    pub async fn close(&self) -> Result<(), HostLinkError> {
        let _guard = self.gate.lock().await;
        self.client.close().await
    }

    pub async fn execute_async<F, Fut, T>(&self, operation: F) -> Result<T, HostLinkError>
    where
        F: FnOnce(&HostLinkClient) -> Fut,
        Fut: Future<Output = Result<T, HostLinkError>>,
    {
        let _guard = self.gate.lock().await;
        operation(&self.client).await
    }

    #[doc(hidden)]
    pub async fn send_raw(&self, body: &str) -> Result<Vec<u8>, HostLinkError> {
        let _guard = self.gate.lock().await;
        self.client.send_raw(body).await
    }

    pub async fn read_comments(&self, device: &str) -> Result<String, HostLinkError> {
        let _guard = self.gate.lock().await;
        self.client.read_comments(device).await
    }

    pub async fn read_typed(
        &self,
        device: &str,
        dtype: &str,
    ) -> Result<helpers::HostLinkValue, HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::read_typed(&self.client, device, dtype).await
    }

    pub async fn read_timer_counter(
        &self,
        device: &str,
    ) -> Result<helpers::TimerCounterValue, HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::read_timer_counter(&self.client, device).await
    }

    pub async fn read_timer(
        &self,
        device: &str,
    ) -> Result<helpers::TimerCounterValue, HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::read_timer(&self.client, device).await
    }

    pub async fn read_counter(
        &self,
        device: &str,
    ) -> Result<helpers::TimerCounterValue, HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::read_counter(&self.client, device).await
    }

    pub async fn write_typed<T: HostLinkPayloadValue>(
        &self,
        device: &str,
        dtype: &str,
        value: T,
    ) -> Result<(), HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::write_typed(&self.client, device, dtype, &value).await
    }

    pub async fn write_bit_in_word(
        &self,
        device: &str,
        bit_index: u8,
        value: bool,
    ) -> Result<(), HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::write_bit_in_word(&self.client, device, bit_index, value).await
    }

    pub async fn read_named<S: AsRef<str>>(
        &self,
        addresses: &[S],
    ) -> Result<helpers::NamedSnapshot, HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::read_named(&self.client, addresses).await
    }

    pub fn poll<'a, S: AsRef<str> + 'a>(
        &'a self,
        addresses: &'a [S],
        interval: Duration,
    ) -> impl futures_core::Stream<Item = Result<helpers::NamedSnapshot, HostLinkError>> + 'a {
        async_stream::try_stream! {
            let addr_list = addresses.iter().map(|item| item.as_ref().to_owned()).collect::<Vec<_>>();
            let compiled = crate::read_plan::compile_read_named_plan(&addr_list);
            loop {
                let snapshot = {
                    let _guard = self.gate.lock().await;
                    if let Some(plan) = &compiled {
                        helpers::execute_read_named_plan(&self.client, plan).await?
                    } else {
                        helpers::read_named_sequential(&self.client, &addr_list).await?
                    }
                };
                yield snapshot;
                tokio::time::sleep(interval).await;
            }
        }
    }

    pub async fn read_words(&self, device: &str, count: usize) -> Result<Vec<u16>, HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::read_words(self.inner_client(), device, count).await
    }

    pub async fn read_dwords(&self, device: &str, count: usize) -> Result<Vec<u32>, HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::read_dwords(self.inner_client(), device, count).await
    }
}

fn checked_deadline(duration: Duration) -> Result<Instant, HostLinkError> {
    Instant::now()
        .checked_add(duration)
        .ok_or_else(|| HostLinkError::protocol("timeout is too large to form an absolute deadline"))
}

async fn write_all_with_timeout(
    stream: &mut TcpStream,
    payload: &[u8],
    deadline: Instant,
) -> Result<(), HostLinkError> {
    timeout_at(deadline, stream.write_all(payload))
        .await
        .map_err(|_| HostLinkError::connection("write timed out"))??;
    Ok(())
}

async fn send_udp_with_timeout(
    socket: &mut UdpSocket,
    payload: &[u8],
    deadline: Instant,
) -> Result<(), HostLinkError> {
    timeout_at(deadline, socket.send(payload))
        .await
        .map_err(|_| HostLinkError::connection("write timed out"))??;
    Ok(())
}

async fn recv_udp_with_timeout(
    socket: &mut UdpSocket,
    buffer: &mut Vec<u8>,
    deadline: Instant,
) -> Result<(), HostLinkError> {
    if buffer.len() != UDP_RECEIVE_BUFFER_SIZE {
        // UDP datagrams cannot be continued by another recv call.
        // Keep the buffer large enough for a full datagram to avoid truncating PLC responses.
        buffer.resize(UDP_RECEIVE_BUFFER_SIZE, 0);
    }
    let read = timeout_at(deadline, socket.recv(buffer.as_mut_slice()))
        .await
        .map_err(|_| HostLinkError::connection("read timed out"))??;
    buffer.truncate(read);
    Ok(())
}

fn build_joined_payload<T: HostLinkPayloadValue>(
    values: &[T],
    suffix: &str,
) -> Result<String, HostLinkError> {
    let mut payload = String::new();
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            payload.push(' ');
        }
        value.append_to_payload(suffix, &mut payload)?;
    }
    Ok(payload)
}

async fn recv_tcp_line(
    stream: &mut TcpStream,
    rx_buf: &mut Vec<u8>,
    rx_start: &mut usize,
    rx_count: &mut usize,
    tcp_read_buf: &mut [u8],
    deadline: Instant,
) -> Result<(Vec<u8>, usize), HostLinkError> {
    loop {
        while *rx_count > 0 && matches!(rx_buf[*rx_start], b'\r' | b'\n') {
            *rx_start += 1;
            *rx_count -= 1;
        }
        if *rx_start > rx_buf.len() / 2 {
            if *rx_count > 0 {
                rx_buf.copy_within(*rx_start..*rx_start + *rx_count, 0);
            }
            *rx_start = 0;
        }
        let mut found_idx = None;
        for index in 0..*rx_count {
            let byte = rx_buf[*rx_start + index];
            if matches!(byte, b'\r' | b'\n') {
                found_idx = Some(index);
                break;
            }
        }

        if let Some(found_idx) = found_idx {
            if found_idx > MAX_TCP_LINE_SIZE {
                return Err(HostLinkError::protocol(format!(
                    "Response line exceeds {MAX_TCP_LINE_SIZE} bytes"
                )));
            }
            let mut skip = found_idx;
            while skip < *rx_count && matches!(rx_buf[*rx_start + skip], b'\r' | b'\n') {
                skip += 1;
            }
            let line = rx_buf[*rx_start..*rx_start + skip].to_vec();
            let counted_len = found_idx + 1;
            *rx_start += skip;
            *rx_count -= skip;
            if *rx_start > rx_buf.len() / 2 {
                rx_buf.copy_within(*rx_start..*rx_start + *rx_count, 0);
                *rx_start = 0;
            }
            return Ok((line, counted_len));
        }

        let read = timeout_at(deadline, stream.read(tcp_read_buf))
            .await
            .map_err(|_| HostLinkError::connection("read timed out"))??;
        if read == 0 {
            let message = if *rx_count > 0 {
                "Connection closed by PLC before the response terminator"
            } else {
                "Connection closed by PLC"
            };
            return Err(HostLinkError::connection(message));
        }

        if *rx_start + *rx_count + read > rx_buf.len() {
            if *rx_count > 0 {
                rx_buf.copy_within(*rx_start..*rx_start + *rx_count, 0);
            }
            *rx_start = 0;
            if *rx_count + read > MAX_TCP_LINE_SIZE + tcp_read_buf.len() {
                return Err(HostLinkError::protocol(format!(
                    "Response line exceeds {MAX_TCP_LINE_SIZE} bytes"
                )));
            }
            if *rx_count + read > rx_buf.len() {
                rx_buf.resize((rx_buf.len() * 2).max(*rx_count + read), 0);
            }
        }

        let target = *rx_start + *rx_count;
        rx_buf[target..target + read].copy_from_slice(&tcp_read_buf[..read]);
        *rx_count += read;
        if *rx_count > MAX_TCP_LINE_SIZE {
            let has_terminator =
                (0..*rx_count).any(|index| matches!(rx_buf[*rx_start + index], b'\r' | b'\n'));
            if !has_terminator {
                return Err(HostLinkError::protocol(format!(
                    "Response line exceeds {MAX_TCP_LINE_SIZE} bytes"
                )));
            }
        }
    }
}
