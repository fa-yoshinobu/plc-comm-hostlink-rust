use crate::address::{
    force_device_types, mbs_device_types, model_name_for_code, mws_device_types, parse_device,
    rdc_device_types, resolve_effective_format, validate_device_count, validate_device_span,
    validate_device_type, validate_expansion_buffer_count, validate_expansion_buffer_span,
    ws_device_types,
};
use crate::device_ranges::{KvDeviceRangeCatalog, device_range_catalog_for_model};
use crate::error::HostLinkError;
use crate::helpers;
use crate::model::{
    HostLinkClock, HostLinkConnectionOptions, HostLinkTraceDirection, HostLinkTraceFrame,
    HostLinkTransportMode, KvModelInfo, KvPlcMode, TraceHook,
};
use crate::protocol::{
    build_frame, decode_comment_response, decode_response, ensure_success, split_data_tokens,
};
use std::fmt::Write as _;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::Mutex;
use tokio::time::timeout;

pub trait HostLinkPayloadValue {
    fn format_for_suffix(&self, data_format: &str) -> String;

    fn append_to_payload(&self, data_format: &str, output: &mut String) {
        output.push_str(&self.format_for_suffix(data_format));
    }
}

macro_rules! impl_payload_for_ints {
    ($($ty:ty),* $(,)?) => {
        $(
            impl HostLinkPayloadValue for $ty {
                fn format_for_suffix(&self, data_format: &str) -> String {
                    let mut value = String::new();
                    self.append_to_payload(data_format, &mut value);
                    value
                }

                fn append_to_payload(&self, data_format: &str, output: &mut String) {
                    if data_format == ".H" {
                        let _ = write!(output, "{:X}", ((*self as i128) & 0xFFFF));
                    } else {
                        let _ = write!(output, "{}", self);
                    }
                }
            }
        )*
    };
}

impl_payload_for_ints!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl HostLinkPayloadValue for f32 {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        let mut value = String::new();
        self.append_to_payload("", &mut value);
        value
    }

    fn append_to_payload(&self, _data_format: &str, output: &mut String) {
        let _ = write!(output, "{}", self);
    }
}

impl HostLinkPayloadValue for f64 {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        let mut value = String::new();
        self.append_to_payload("", &mut value);
        value
    }

    fn append_to_payload(&self, _data_format: &str, output: &mut String) {
        let _ = write!(output, "{}", self);
    }
}

impl HostLinkPayloadValue for bool {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        let mut value = String::new();
        self.append_to_payload("", &mut value);
        value
    }

    fn append_to_payload(&self, _data_format: &str, output: &mut String) {
        output.push(if *self { '1' } else { '0' });
    }
}

impl HostLinkPayloadValue for String {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        self.trim().to_owned()
    }

    fn append_to_payload(&self, _data_format: &str, output: &mut String) {
        output.push_str(self.trim());
    }
}

impl HostLinkPayloadValue for &str {
    fn format_for_suffix(&self, _data_format: &str) -> String {
        self.trim().to_owned()
    }

    fn append_to_payload(&self, _data_format: &str, output: &mut String) {
        output.push_str(self.trim());
    }
}

impl<T: HostLinkPayloadValue + ?Sized> HostLinkPayloadValue for &T {
    fn format_for_suffix(&self, data_format: &str) -> String {
        (*self).format_for_suffix(data_format)
    }

    fn append_to_payload(&self, data_format: &str, output: &mut String) {
        (*self).append_to_payload(data_format, output);
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
    trace_hook: Option<TraceHook>,
    rx_buf: Vec<u8>,
    rx_start: usize,
    rx_count: usize,
    tcp_read_buf: Vec<u8>,
    udp_read_buf: Vec<u8>,
}

impl HostLinkClient {
    pub fn new(options: HostLinkConnectionOptions) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ClientInner {
                options,
                transport: None,
                trace_hook: None,
                rx_buf: vec![0u8; 4096],
                rx_start: 0,
                rx_count: 0,
                tcp_read_buf: vec![0u8; 8192],
                udp_read_buf: vec![0u8; 4096],
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
        self.inner.lock().await.transport.is_some()
    }

    pub async fn timeout(&self) -> Duration {
        self.inner.lock().await.options.timeout
    }

    pub async fn set_timeout(&self, timeout: Duration) {
        self.inner.lock().await.options.timeout = timeout;
    }

    pub async fn append_lf_on_send(&self) -> bool {
        self.inner.lock().await.options.append_lf_on_send
    }

    pub async fn set_append_lf_on_send(&self, value: bool) {
        self.inner.lock().await.options.append_lf_on_send = value;
    }

    pub async fn set_trace_hook(&self, trace_hook: Option<TraceHook>) {
        self.inner.lock().await.trace_hook = trace_hook;
    }

    pub async fn send_raw(&self, body: &str) -> Result<String, HostLinkError> {
        self.inner.lock().await.send_raw(body).await
    }

    pub async fn change_mode(&self, mode: KvPlcMode) -> Result<(), HostLinkError> {
        self.expect_ok(&format!("M{}", mode as u8)).await
    }

    pub async fn clear_error(&self) -> Result<(), HostLinkError> {
        self.expect_ok("ER").await
    }

    pub async fn check_error_no(&self) -> Result<String, HostLinkError> {
        self.send_raw("?E").await
    }

    pub async fn query_model(&self) -> Result<KvModelInfo, HostLinkError> {
        let code = self.send_raw("?K").await?;
        Ok(KvModelInfo {
            model: model_name_for_code(&code).to_owned(),
            code,
        })
    }

    pub async fn read_device_range_catalog(&self) -> Result<KvDeviceRangeCatalog, HostLinkError> {
        let model = self.query_model().await?;
        device_range_catalog_for_model(&model.model)
    }

    pub async fn confirm_operating_mode(&self) -> Result<KvPlcMode, HostLinkError> {
        match self.send_raw("?M").await?.parse::<u8>() {
            Ok(0) => Ok(KvPlcMode::Program),
            Ok(1) => Ok(KvPlcMode::Run),
            _ => Err(HostLinkError::protocol("Unsupported PLC mode response")),
        }
    }

    pub async fn set_time(&self, value: Option<HostLinkClock>) -> Result<(), HostLinkError> {
        let value = value.unwrap_or_else(HostLinkClock::now_local);
        if value.month == 0
            || value.month > 12
            || value.day == 0
            || value.day > 31
            || value.hour > 23
            || value.minute > 59
            || value.second > 59
            || value.week > 6
        {
            return Err(HostLinkError::protocol(
                "Invalid time fields for WRT command",
            ));
        }

        self.expect_ok(&format!(
            "WRT {:02} {:02} {:02} {:02} {:02} {:02} {}",
            value.year, value.month, value.day, value.hour, value.minute, value.second, value.week
        ))
        .await
    }

    pub async fn forced_set(&self, device: &str) -> Result<(), HostLinkError> {
        let mut address = parse_device(device)?;
        validate_device_type("ST", &address.device_type, force_device_types())?;
        address.suffix.clear();
        self.expect_ok(&format!("ST {}", address.to_text()?)).await
    }

    pub async fn forced_reset(&self, device: &str) -> Result<(), HostLinkError> {
        let mut address = parse_device(device)?;
        validate_device_type("RS", &address.device_type, force_device_types())?;
        address.suffix.clear();
        self.expect_ok(&format!("RS {}", address.to_text()?)).await
    }

    pub async fn read(
        &self,
        device: &str,
        data_format: Option<&str>,
    ) -> Result<Vec<String>, HostLinkError> {
        let mut address = parse_device(device)?;
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            address.suffix.clone()
        };
        let suffix = resolve_effective_format(&address.device_type, &suffix);
        validate_device_span(&address.device_type, address.number, &suffix, 1)?;
        address.suffix = suffix;
        let response = self.send_raw(&format!("RD {}", address.to_text()?)).await?;
        Ok(split_data_tokens(&response))
    }

    pub async fn read_consecutive(
        &self,
        device: &str,
        count: usize,
        data_format: Option<&str>,
    ) -> Result<Vec<String>, HostLinkError> {
        let mut address = parse_device(device)?;
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            address.suffix.clone()
        };
        let suffix = resolve_effective_format(&address.device_type, &suffix);
        validate_device_count(&address.device_type, &suffix, count)?;
        validate_device_span(&address.device_type, address.number, &suffix, count)?;
        address.suffix = suffix;
        let response = self
            .send_raw(&format!("RDS {} {}", address.to_text()?, count))
            .await?;
        Ok(split_data_tokens(&response))
    }

    pub async fn write<T: HostLinkPayloadValue>(
        &self,
        device: &str,
        value: T,
        data_format: Option<&str>,
    ) -> Result<(), HostLinkError> {
        let mut address = parse_device(device)?;
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            address.suffix.clone()
        };
        let suffix = resolve_effective_format(&address.device_type, &suffix);
        validate_device_span(&address.device_type, address.number, &suffix, 1)?;
        address.suffix = suffix.clone();
        let mut command = String::from("WR ");
        command.push_str(&address.to_text()?);
        command.push(' ');
        value.append_to_payload(&suffix, &mut command);
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
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            address.suffix.clone()
        };
        let suffix = resolve_effective_format(&address.device_type, &suffix);
        validate_device_count(&address.device_type, &suffix, values.len())?;
        validate_device_span(&address.device_type, address.number, &suffix, values.len())?;
        address.suffix = suffix.clone();
        let payload = build_joined_payload(values, &suffix);
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
            let mut address = parse_device(device.as_ref())?;
            validate_device_type("MBS", &address.device_type, mbs_device_types())?;
            address.suffix.clear();
            command.push(' ');
            command.push_str(&address.to_text()?);
        }
        self.expect_ok(&command).await
    }

    pub async fn register_monitor_words<S: AsRef<str>>(
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

        let mut command = String::from("MWS");
        for device in devices {
            let mut address = parse_device(device.as_ref())?;
            validate_device_type("MWS", &address.device_type, mws_device_types())?;
            let suffix = resolve_effective_format(&address.device_type, &address.suffix);
            validate_device_span(&address.device_type, address.number, &suffix, 1)?;
            address.suffix = suffix;
            command.push(' ');
            command.push_str(&address.to_text()?);
        }
        self.expect_ok(&command).await
    }

    pub async fn read_monitor_bits(&self) -> Result<Vec<String>, HostLinkError> {
        let response = self.send_raw("MBR").await?;
        Ok(split_data_tokens(&response))
    }

    pub async fn read_monitor_words(&self) -> Result<Vec<String>, HostLinkError> {
        let response = self.send_raw("MWR").await?;
        Ok(split_data_tokens(&response))
    }

    pub async fn forced_set_consecutive(
        &self,
        device: &str,
        count: usize,
    ) -> Result<(), HostLinkError> {
        if !(1..=16).contains(&count) {
            return Err(HostLinkError::protocol("count must be 1-16."));
        }
        let mut address = parse_device(device)?;
        validate_device_type("STS", &address.device_type, force_device_types())?;
        address.suffix.clear();
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
        let mut address = parse_device(device)?;
        validate_device_type("RSS", &address.device_type, force_device_types())?;
        address.suffix.clear();
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
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            address.suffix.clone()
        };
        let suffix = resolve_effective_format(&address.device_type, &suffix);
        validate_device_count(&address.device_type, &suffix, count)?;
        validate_device_span(&address.device_type, address.number, &suffix, count)?;
        address.suffix = suffix;
        let response = self
            .send_raw(&format!("RDE {} {}", address.to_text()?, count))
            .await?;
        Ok(split_data_tokens(&response))
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
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            address.suffix.clone()
        };
        let suffix = resolve_effective_format(&address.device_type, &suffix);
        validate_device_count(&address.device_type, &suffix, values.len())?;
        validate_device_span(&address.device_type, address.number, &suffix, values.len())?;
        address.suffix = suffix.clone();
        let payload = build_joined_payload(values, &suffix);
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
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            resolve_effective_format(&address.device_type, &address.suffix)
        };
        validate_device_span(&address.device_type, address.number, &suffix, 1)?;
        address.suffix = suffix.clone();
        let mut command = String::from("WS ");
        command.push_str(&address.to_text()?);
        command.push(' ');
        value.append_to_payload(&suffix, &mut command);
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
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            resolve_effective_format(&address.device_type, &address.suffix)
        };
        validate_device_span(&address.device_type, address.number, &suffix, values.len())?;
        address.suffix = suffix.clone();
        let payload = build_joined_payload(values, &suffix);
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
        data_format: Option<&str>,
    ) -> Result<Vec<String>, HostLinkError> {
        if unit_no > 48 {
            return Err(HostLinkError::protocol("unitNo must be 0-48."));
        }
        if address > 59_999 {
            return Err(HostLinkError::protocol("address must be 0-59999."));
        }
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            ".U".to_owned()
        };
        validate_expansion_buffer_count(&suffix, count)?;
        validate_expansion_buffer_span(address, &suffix, count)?;
        let response = self
            .send_raw(&format!("URD {unit_no:02} {address} {suffix} {count}"))
            .await?;
        Ok(split_data_tokens(&response))
    }

    pub async fn write_expansion_unit_buffer<T: HostLinkPayloadValue>(
        &self,
        unit_no: u8,
        address: u32,
        values: &[T],
        data_format: Option<&str>,
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
        let suffix = if let Some(data_format) = data_format {
            crate::address::normalize_suffix(data_format)?
        } else {
            ".U".to_owned()
        };
        validate_expansion_buffer_count(&suffix, values.len())?;
        validate_expansion_buffer_span(address, &suffix, values.len())?;
        let payload = build_joined_payload(values, &suffix);
        self.expect_ok(&format!(
            "UWR {unit_no:02} {address} {suffix} {} {payload}",
            values.len()
        ))
        .await
    }

    pub async fn read_comments(
        &self,
        device: &str,
        strip_padding: bool,
    ) -> Result<String, HostLinkError> {
        let mut address = parse_device(device)?;
        validate_device_type("RDC", &address.device_type, rdc_device_types())?;
        address.suffix.clear();
        let response = self
            .inner
            .lock()
            .await
            .send_raw_decoded(
                &format!("RDC {}", address.to_text()?),
                decode_comment_response,
            )
            .await?;
        if strip_padding {
            Ok(response.trim_end_matches(' ').to_owned())
        } else {
            Ok(response)
        }
    }

    pub async fn read_typed(
        &self,
        device: &str,
        dtype: &str,
    ) -> Result<helpers::HostLinkValue, HostLinkError> {
        helpers::read_typed(self, device, dtype).await
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
        helpers::write_bit_in_word(self, device, bit_index, value).await
    }

    async fn expect_ok(&self, body: &str) -> Result<(), HostLinkError> {
        let response = self.send_raw(body).await?;
        if response == "OK" {
            Ok(())
        } else {
            Err(HostLinkError::protocol(format!(
                "Expected 'OK' but received '{response}' for command '{body}'"
            )))
        }
    }
}

impl ClientInner {
    async fn open(&mut self) -> Result<(), HostLinkError> {
        if self.transport.is_some() {
            return Ok(());
        }

        let transport = match self.options.transport {
            HostLinkTransportMode::Tcp => {
                let stream = timeout(
                    self.options.timeout,
                    TcpStream::connect((self.options.host.as_str(), self.options.port)),
                )
                .await
                .map_err(|_| HostLinkError::connection("tcp connect timed out"))??;
                stream.set_nodelay(true)?;
                Transport::Tcp(stream)
            }
            HostLinkTransportMode::Udp => {
                let socket = UdpSocket::bind("0.0.0.0:0").await?;
                timeout(
                    self.options.timeout,
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
    }

    async fn send_raw(&mut self, body: &str) -> Result<String, HostLinkError> {
        self.send_raw_decoded(body, decode_response).await
    }

    async fn send_raw_decoded<F>(&mut self, body: &str, decoder: F) -> Result<String, HostLinkError>
    where
        F: Fn(&[u8]) -> Result<String, HostLinkError>,
    {
        self.open().await?;
        let frame = build_frame(body, self.options.append_lf_on_send);
        self.fire_trace(HostLinkTraceDirection::Send, &frame);

        match self.transport.as_mut() {
            Some(Transport::Tcp(stream)) => {
                write_all_with_timeout(stream, &frame, self.options.timeout).await?;
                let raw = recv_tcp_line(
                    stream,
                    &mut self.rx_buf,
                    &mut self.rx_start,
                    &mut self.rx_count,
                    &mut self.tcp_read_buf,
                    self.options.timeout,
                )
                .await?;
                self.fire_trace(HostLinkTraceDirection::Receive, &raw);
                ensure_success(decoder(&raw)?)
            }
            Some(Transport::Udp(socket)) => {
                send_udp_with_timeout(socket, &frame, self.options.timeout).await?;
                recv_udp_with_timeout(socket, &mut self.udp_read_buf, self.options.timeout).await?;
                let raw = &self.udp_read_buf;
                self.fire_trace(HostLinkTraceDirection::Receive, raw);
                ensure_success(decoder(raw)?)
            }
            None => Err(HostLinkError::connection("transport was not opened")),
        }
    }

    fn fire_trace(&self, direction: HostLinkTraceDirection, data: &[u8]) {
        if let Some(trace_hook) = &self.trace_hook {
            trace_hook(HostLinkTraceFrame {
                direction,
                data: data.to_vec(),
                timestamp: SystemTime::now(),
            });
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

    pub async fn open(&self) -> Result<(), HostLinkError> {
        let _guard = self.gate.lock().await;
        self.client.open().await
    }

    pub async fn close(&self) -> Result<(), HostLinkError> {
        let _guard = self.gate.lock().await;
        self.client.close().await
    }

    pub async fn set_trace_hook(&self, trace_hook: Option<TraceHook>) {
        let _guard = self.gate.lock().await;
        self.client.set_trace_hook(trace_hook).await;
    }

    pub async fn execute_async<F, Fut, T>(&self, operation: F) -> Result<T, HostLinkError>
    where
        F: FnOnce(&HostLinkClient) -> Fut,
        Fut: Future<Output = Result<T, HostLinkError>>,
    {
        let _guard = self.gate.lock().await;
        operation(&self.client).await
    }

    pub async fn send_raw(&self, body: &str) -> Result<String, HostLinkError> {
        let _guard = self.gate.lock().await;
        self.client.send_raw(body).await
    }

    pub async fn read_comments(
        &self,
        device: &str,
        strip_padding: bool,
    ) -> Result<String, HostLinkError> {
        let _guard = self.gate.lock().await;
        self.client.read_comments(device, strip_padding).await
    }

    pub async fn read_typed(
        &self,
        device: &str,
        dtype: &str,
    ) -> Result<helpers::HostLinkValue, HostLinkError> {
        let _guard = self.gate.lock().await;
        helpers::read_typed(&self.client, device, dtype).await
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

    pub async fn read_device_range_catalog(&self) -> Result<KvDeviceRangeCatalog, HostLinkError> {
        let _guard = self.gate.lock().await;
        self.client.read_device_range_catalog().await
    }

    pub fn poll<'a, S: AsRef<str> + 'a>(
        &'a self,
        addresses: &'a [S],
        interval: Duration,
    ) -> impl futures_core::Stream<Item = Result<helpers::NamedSnapshot, HostLinkError>> + 'a {
        async_stream::try_stream! {
            let addr_list = addresses.iter().map(|item| item.as_ref().to_owned()).collect::<Vec<_>>();
            let compiled = helpers::compile_read_named_plan(&addr_list);
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

async fn write_all_with_timeout(
    stream: &mut TcpStream,
    payload: &[u8],
    duration: Duration,
) -> Result<(), HostLinkError> {
    timeout(duration, stream.write_all(payload))
        .await
        .map_err(|_| HostLinkError::connection("write timed out"))??;
    Ok(())
}

async fn send_udp_with_timeout(
    socket: &mut UdpSocket,
    payload: &[u8],
    duration: Duration,
) -> Result<(), HostLinkError> {
    timeout(duration, socket.send(payload))
        .await
        .map_err(|_| HostLinkError::connection("write timed out"))??;
    Ok(())
}

async fn recv_udp_with_timeout(
    socket: &mut UdpSocket,
    buffer: &mut Vec<u8>,
    duration: Duration,
) -> Result<(), HostLinkError> {
    if buffer.len() != 4096 {
        buffer.resize(4096, 0);
    }
    let read = timeout(duration, socket.recv(buffer.as_mut_slice()))
        .await
        .map_err(|_| HostLinkError::connection("read timed out"))??;
    buffer.truncate(read);
    Ok(())
}

fn build_joined_payload<T: HostLinkPayloadValue>(values: &[T], suffix: &str) -> String {
    let mut payload = String::new();
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            payload.push(' ');
        }
        value.append_to_payload(suffix, &mut payload);
    }
    payload
}

async fn recv_tcp_line(
    stream: &mut TcpStream,
    rx_buf: &mut Vec<u8>,
    rx_start: &mut usize,
    rx_count: &mut usize,
    tcp_read_buf: &mut [u8],
    duration: Duration,
) -> Result<Vec<u8>, HostLinkError> {
    loop {
        let mut found_idx = None;
        for index in 0..*rx_count {
            let byte = rx_buf[*rx_start + index];
            if matches!(byte, b'\r' | b'\n') {
                found_idx = Some(index);
                break;
            }
        }

        if let Some(found_idx) = found_idx {
            let line = rx_buf[*rx_start..*rx_start + found_idx].to_vec();
            let mut skip = found_idx;
            while skip < *rx_count && matches!(rx_buf[*rx_start + skip], b'\r' | b'\n') {
                skip += 1;
            }
            *rx_start += skip;
            *rx_count -= skip;
            if *rx_start > rx_buf.len() / 2 {
                rx_buf.copy_within(*rx_start..*rx_start + *rx_count, 0);
                *rx_start = 0;
            }
            return Ok(line);
        }

        let read = timeout(duration, stream.read(tcp_read_buf))
            .await
            .map_err(|_| HostLinkError::connection("read timed out"))??;
        if read == 0 {
            if *rx_count > 0 {
                let line = rx_buf[*rx_start..*rx_start + *rx_count].to_vec();
                *rx_start = 0;
                *rx_count = 0;
                return Ok(line);
            }
            return Err(HostLinkError::connection("Connection closed by PLC"));
        }

        if *rx_start + *rx_count + read > rx_buf.len() {
            if *rx_count > 0 {
                rx_buf.copy_within(*rx_start..*rx_start + *rx_count, 0);
            }
            *rx_start = 0;
            if *rx_count + read > rx_buf.len() {
                rx_buf.resize((rx_buf.len() * 2).max(*rx_count + read), 0);
            }
        }

        let target = *rx_start + *rx_count;
        rx_buf[target..target + read].copy_from_slice(&tcp_read_buf[..read]);
        *rx_count += read;
    }
}
