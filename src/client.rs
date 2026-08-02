use crate::address::{
    force_consecutive_device_types, force_device_types, is_direct_bit_device_type,
    mbs_device_types, model_name_for_code, mws_device_types, parse_device, rdc_device_types,
    require_explicit_format, require_no_suffix, validate_device_count, validate_device_span,
    validate_device_type, validate_expansion_buffer_count, validate_expansion_buffer_span,
    wr_device_types, ws_device_types,
};
use crate::error::{HostLinkError, HostLinkOutcomeUnknownReason};
use crate::helpers;
use crate::model::{
    HostLinkClock, HostLinkCommentEncoding, HostLinkConnectionOptions, HostLinkMonitorWord,
    HostLinkTransportMode, KvModelInfo, KvPlcMode, validate_host_input,
};
use crate::protocol::{
    build_frame, comment_response_payload, decode_comment_payload, decode_response,
    ensure_comment_success, ensure_success, raw_response_body, split_data_tokens,
};
use std::net::{IpAddr, SocketAddr};
use std::sync::{
    Arc, RwLock,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket, lookup_host};
use tokio::sync::{Mutex, OwnedMutexGuard, watch};
use tokio::time::{Instant, timeout_at};

const MAX_TCP_LINE_SIZE: usize = 65_536;
const UDP_RECEIVE_BUFFER_SIZE: usize = MAX_TCP_LINE_SIZE + 2;

/// Value that can produce one validated Host Link write token.
///
/// Implementations using the former infallible signature do not compile:
///
/// ```compile_fail
/// use plc_comm_kv_hostlink::{HostLinkError, HostLinkPayloadValue};
/// struct Legacy;
/// impl HostLinkPayloadValue for Legacy {
///     fn format_for_suffix(&self, _suffix: &str) -> String {
///         String::new()
///     }
/// }
/// ```
pub trait HostLinkPayloadValue {
    /// Format one complete low-level numeric payload token.
    ///
    /// Implementations must reject unsupported suffixes and invalid values;
    /// returning a fallback or empty token is not permitted.
    fn format_for_suffix(&self, data_format: &str) -> Result<String, HostLinkError>;

    fn as_bool(&self) -> Option<bool> {
        None
    }

    fn as_integer(&self) -> Option<i128> {
        None
    }

    #[doc(hidden)]
    fn as_float(&self) -> Option<f64> {
        None
    }

    #[doc(hidden)]
    fn as_text(&self) -> Option<&str> {
        None
    }

    fn append_to_payload(
        &self,
        data_format: &str,
        output: &mut String,
    ) -> Result<(), HostLinkError> {
        let token = self.format_for_suffix(data_format)?;
        if token.is_empty() {
            return Err(HostLinkError::protocol(format!(
                "formatter returned an empty token for data format '{data_format}'"
            )));
        }
        output.push_str(&token);
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
            "" => matches!(token.as_str(), "0" | "1" | "ON" | "OFF"),
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

fn validate_packed_direct_bits_u16_token(token: &str) -> Result<(), HostLinkError> {
    let valid = (1..=5).contains(&token.len())
        && token.bytes().all(|byte| byte.is_ascii_digit())
        && token.parse::<u16>().is_ok();
    if valid {
        Ok(())
    } else {
        Err(HostLinkError::protocol(format!(
            "Invalid packed direct-bit MWR token '{token}'; expected 1-5 ASCII decimal digits in 0..=65535"
        )))
    }
}

fn validate_and_normalize_response_tokens(
    tokens: &mut [String],
    data_format: &str,
) -> Result<(), HostLinkError> {
    validate_response_tokens(tokens, data_format)?;
    if data_format == ".H" {
        for token in tokens {
            let value = u16::from_str_radix(token, 16).map_err(|_| {
                HostLinkError::protocol(format!(
                    "Invalid response token '{token}' for data format '{data_format}'"
                ))
            })?;
            *token = format!("{value:04X}");
        }
    }
    Ok(())
}

fn validate_and_normalize_timer_counter_response_tokens(
    tokens: &mut [String],
    data_format: &str,
) -> Result<(), HostLinkError> {
    if !matches!(tokens.first().map(String::as_str), Some("0" | "1")) {
        return Err(HostLinkError::protocol(format!(
            "Invalid timer/counter status token: {}",
            tokens.first().map(String::as_str).unwrap_or("<missing>")
        )));
    }
    validate_and_normalize_response_tokens(&mut tokens[1..], data_format)
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

fn read_response_token_count(device_type: &str, _data_format: &str) -> usize {
    if matches!(device_type, "T" | "C") {
        return 3;
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

                fn format_for_suffix(&self, data_format: &str) -> Result<String, HostLinkError> {
                    validate_integer_payload(*self as i128, data_format)?;
                    if data_format == ".H" {
                        Ok(format!("{:X}", *self as i128))
                    } else {
                        Ok(self.to_string())
                    }
                }
            }
        )*
    };
}

impl_payload_for_ints!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);

impl HostLinkPayloadValue for f32 {
    fn format_for_suffix(&self, data_format: &str) -> Result<String, HostLinkError> {
        Err(HostLinkError::protocol(format!(
            "f32 is not valid for low-level numeric data format '{data_format}'; use the typed F helper"
        )))
    }

    fn as_float(&self) -> Option<f64> {
        Some(f64::from(*self))
    }
}

impl HostLinkPayloadValue for f64 {
    fn format_for_suffix(&self, data_format: &str) -> Result<String, HostLinkError> {
        Err(HostLinkError::protocol(format!(
            "f64 is not valid for low-level numeric data format '{data_format}'; use the typed F helper"
        )))
    }

    fn as_float(&self) -> Option<f64> {
        Some(*self)
    }
}

impl HostLinkPayloadValue for bool {
    fn as_bool(&self) -> Option<bool> {
        Some(*self)
    }

    fn format_for_suffix(&self, data_format: &str) -> Result<String, HostLinkError> {
        if data_format.is_empty() {
            Ok(if *self { "1" } else { "0" }.to_owned())
        } else {
            Err(HostLinkError::protocol(format!(
                "bool is valid only for direct-bit formatting, not '{data_format}'"
            )))
        }
    }
}

impl HostLinkPayloadValue for String {
    fn format_for_suffix(&self, data_format: &str) -> Result<String, HostLinkError> {
        Err(HostLinkError::protocol(format!(
            "String is not valid for low-level numeric data format '{data_format}'"
        )))
    }

    fn as_text(&self) -> Option<&str> {
        Some(self.as_str())
    }
}

impl HostLinkPayloadValue for &str {
    fn format_for_suffix(&self, data_format: &str) -> Result<String, HostLinkError> {
        Err(HostLinkError::protocol(format!(
            "str is not valid for low-level numeric data format '{data_format}'"
        )))
    }

    fn as_text(&self) -> Option<&str> {
        Some(self)
    }
}

impl<T: HostLinkPayloadValue + ?Sized> HostLinkPayloadValue for &T {
    fn as_bool(&self) -> Option<bool> {
        (*self).as_bool()
    }

    fn as_integer(&self) -> Option<i128> {
        (*self).as_integer()
    }

    fn format_for_suffix(&self, data_format: &str) -> Result<String, HostLinkError> {
        (*self).format_for_suffix(data_format)
    }

    fn as_float(&self) -> Option<f64> {
        (*self).as_float()
    }

    fn as_text(&self) -> Option<&str> {
        (*self).as_text()
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
    turn: Arc<Mutex<()>>,
    control: Arc<ClientControl>,
    admitted_generation: Option<u64>,
    admitted_timeout: Option<Duration>,
}

pub struct HostLinkClientFactory;

struct ClientControl {
    generation: AtomicU64,
    close_tx: watch::Sender<u64>,
    timeout: RwLock<Duration>,
}

enum Transport {
    Tcp(TcpStream),
    Udp(UdpTransport),
}

struct UdpTransport {
    endpoint: SocketAddr,
    socket: Option<UdpSocket>,
}

#[derive(Clone)]
enum MonitorWordResponseFormat {
    Numeric(String),
    PackedDirectBitsU16,
}

struct ClientInner {
    options: HostLinkConnectionOptions,
    transport: Option<Transport>,
    rx_buf: Vec<u8>,
    rx_start: usize,
    rx_count: usize,
    rx_scan: usize,
    tcp_read_buf: Vec<u8>,
    udp_read_buf: Vec<u8>,
    tcp_scan_bytes: u64,
    tcp_copy_bytes: u64,
    transport_buffer_allocation_count: u64,
    monitor_bit_count: Option<usize>,
    monitor_word_formats: Option<Vec<MonitorWordResponseFormat>>,
    // Set before the first transport await and cleared only after a complete
    // response. If the future is dropped, the next operation replaces the
    // poisoned transport before sending another request.
    exchange_incomplete: bool,
    last_request_may_have_been_sent: bool,
    active_deadline: Option<Instant>,
    traffic_stats: crate::HostLinkTrafficStats,
}

impl HostLinkClient {
    pub fn new(options: HostLinkConnectionOptions) -> Self {
        let (close_tx, _) = watch::channel(0u64);
        let initial_timeout = options.timeout;
        Self {
            inner: Arc::new(Mutex::new(ClientInner {
                options,
                transport: None,
                rx_buf: Vec::new(),
                rx_start: 0,
                rx_count: 0,
                rx_scan: 0,
                tcp_read_buf: Vec::new(),
                udp_read_buf: Vec::new(),
                tcp_scan_bytes: 0,
                tcp_copy_bytes: 0,
                transport_buffer_allocation_count: 0,
                monitor_bit_count: None,
                monitor_word_formats: None,
                exchange_incomplete: false,
                last_request_may_have_been_sent: false,
                active_deadline: None,
                traffic_stats: crate::HostLinkTrafficStats::default(),
            })),
            turn: Arc::new(Mutex::new(())),
            control: Arc::new(ClientControl {
                generation: AtomicU64::new(0),
                close_tx,
                timeout: RwLock::new(initial_timeout),
            }),
            admitted_generation: None,
            admitted_timeout: None,
        }
    }

    fn generation(&self) -> u64 {
        self.admitted_generation
            .unwrap_or_else(|| self.control.generation.load(Ordering::Acquire))
    }

    fn direct_for_generation(&self, generation: u64, timeout: Duration) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            turn: Arc::clone(&self.turn),
            control: Arc::clone(&self.control),
            admitted_generation: Some(generation),
            admitted_timeout: Some(timeout),
        }
    }

    async fn acquire_turn(
        &self,
        generation: u64,
    ) -> Result<Option<OwnedMutexGuard<()>>, HostLinkError> {
        if self.admitted_generation.is_some() {
            if self.control.generation.load(Ordering::Acquire) != generation {
                return Err(HostLinkError::Closed);
            }
            return Ok(None);
        }
        let mut close_rx = self.control.close_tx.subscribe();
        let turn = Arc::clone(&self.turn);
        let guard = tokio::select! {
            guard = turn.lock_owned() => guard,
            changed = close_rx.changed() => {
                let _ = changed;
                return Err(HostLinkError::Closed);
            }
        };
        if self.control.generation.load(Ordering::Acquire) != generation {
            return Err(HostLinkError::Closed);
        }
        Ok(Some(guard))
    }

    pub(crate) async fn begin_turn(
        &self,
    ) -> Result<(Option<OwnedMutexGuard<()>>, HostLinkClient), HostLinkError> {
        let generation = self.generation();
        let timeout = self.admitted_timeout.unwrap_or_else(|| {
            *self
                .control
                .timeout
                .read()
                .expect("timeout snapshot lock poisoned")
        });
        let guard = self.acquire_turn(generation).await?;
        Ok((guard, self.direct_for_generation(generation, timeout)))
    }

    pub async fn connect(options: HostLinkConnectionOptions) -> Result<Self, HostLinkError> {
        let client = Self::new(options);
        client.open().await?;
        Ok(client)
    }

    pub async fn open(&self) -> Result<(), HostLinkError> {
        let (_turn, direct) = self.begin_turn().await?;
        direct.open_direct().await
    }

    pub async fn close(&self) -> Result<(), HostLinkError> {
        let generation = self.control.generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.control.close_tx.send_replace(generation);
        let _turn = Arc::clone(&self.turn).lock_owned().await;
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
        let (_turn, direct) = self.begin_turn().await?;
        direct.inner.lock().await.options.timeout = timeout;
        *self
            .control
            .timeout
            .write()
            .expect("timeout snapshot lock poisoned") = timeout;
        Ok(())
    }

    #[doc(hidden)]
    pub async fn send_raw(&self, body: &str) -> Result<Vec<u8>, HostLinkError> {
        let body = body.to_owned();
        build_frame(&body)?;
        let (_turn, direct) = self.begin_turn().await?;
        direct.send_raw_direct(&body, true).await
    }

    pub(crate) async fn send_decoded(&self, body: &str) -> Result<String, HostLinkError> {
        let body = body.to_owned();
        build_frame(&body)?;
        let (_turn, direct) = self.begin_turn().await?;
        direct.send_decoded_direct(&body, false).await
    }

    async fn open_direct(&self) -> Result<(), HostLinkError> {
        let generation = self.generation();
        if self.control.generation.load(Ordering::Acquire) != generation {
            return Err(HostLinkError::Closed);
        }
        let mut close_rx = self.control.close_tx.subscribe();
        if *close_rx.borrow() != generation {
            return Err(HostLinkError::Closed);
        }
        let mut inner = self.inner.lock().await;
        let timeout = self.admitted_timeout.unwrap_or(inner.options.timeout);
        let result = tokio::select! {
            result = inner.open(timeout) => result,
            changed = close_rx.changed() => {
                let _ = changed;
                Err(HostLinkError::Closed)
            }
        };
        if matches!(result, Err(HostLinkError::Closed)) {
            inner.close();
        }
        result
    }

    async fn send_raw_direct(
        &self,
        body: &str,
        state_changing: bool,
    ) -> Result<Vec<u8>, HostLinkError> {
        let generation = self.generation();
        let mut close_rx = self.control.close_tx.subscribe();
        if self.control.generation.load(Ordering::Acquire) != generation
            || *close_rx.borrow() != generation
        {
            return Err(HostLinkError::Closed);
        }
        let mut inner = self.inner.lock().await;
        inner.last_request_may_have_been_sent = false;
        let timeout = self.admitted_timeout.unwrap_or(inner.options.timeout);
        let result = tokio::select! {
            result = inner.send_raw_bytes(body, timeout) => result,
            changed = close_rx.changed() => {
                let _ = changed;
                Err(HostLinkError::Closed)
            }
        };
        let may_have_been_sent = inner.last_request_may_have_been_sent;
        if result.is_err() {
            inner.retire_failed_transport();
        }
        classify_operation_result(result, state_changing, may_have_been_sent)
    }

    async fn send_decoded_direct(
        &self,
        body: &str,
        state_changing: bool,
    ) -> Result<String, HostLinkError> {
        self.send_decoded_with_direct(body, state_changing, decode_response)
            .await
    }

    async fn send_decoded_with_direct(
        &self,
        body: &str,
        state_changing: bool,
        decoder: fn(&[u8]) -> Result<String, HostLinkError>,
    ) -> Result<String, HostLinkError> {
        let generation = self.generation();
        if self.control.generation.load(Ordering::Acquire) != generation {
            return Err(HostLinkError::Closed);
        }
        let mut close_rx = self.control.close_tx.subscribe();
        if *close_rx.borrow() != generation {
            return Err(HostLinkError::Closed);
        }
        let mut inner = self.inner.lock().await;
        inner.last_request_may_have_been_sent = false;
        let timeout = self.admitted_timeout.unwrap_or(inner.options.timeout);
        let result = tokio::select! {
            result = inner.send_decoded_with(body, decoder, timeout) => result,
            changed = close_rx.changed() => {
                let _ = changed;
                Err(HostLinkError::Closed)
            }
        };
        let may_have_been_sent = inner.last_request_may_have_been_sent;
        if matches!(&result, Err(error) if !matches!(error, HostLinkError::Plc { .. })) {
            inner.retire_failed_transport();
        }
        classify_operation_result(result, state_changing, may_have_been_sent)
    }

    async fn send_comment_with_direct<T, F>(
        &self,
        body: &str,
        decoder: F,
    ) -> Result<T, HostLinkError>
    where
        F: FnOnce(&[u8]) -> Result<T, HostLinkError>,
    {
        let generation = self.generation();
        if self.control.generation.load(Ordering::Acquire) != generation {
            return Err(HostLinkError::Closed);
        }
        let mut close_rx = self.control.close_tx.subscribe();
        if *close_rx.borrow() != generation {
            return Err(HostLinkError::Closed);
        }
        let mut inner = self.inner.lock().await;
        inner.last_request_may_have_been_sent = false;
        let timeout = self.admitted_timeout.unwrap_or(inner.options.timeout);
        let result = tokio::select! {
            result = inner.send_comment_with(body, decoder, timeout) => result,
            changed = close_rx.changed() => {
                let _ = changed;
                Err(HostLinkError::Closed)
            }
        };
        let may_have_been_sent = inner.last_request_may_have_been_sent;
        if matches!(&result, Err(error) if !matches!(error, HostLinkError::Plc { .. })) {
            inner.retire_failed_transport();
        }
        classify_operation_result(result, false, may_have_been_sent)
    }

    pub(crate) async fn retire_transport(&self) {
        self.inner.lock().await.retire_failed_transport();
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
        let (_turn, direct) = self.begin_turn().await?;
        let response = direct.send_decoded_direct("?M", false).await?;
        match response.parse::<u8>() {
            Ok(0) => Ok(KvPlcMode::Program),
            Ok(1) => Ok(KvPlcMode::Run),
            _ => {
                direct.retire_transport().await;
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
        let command = format!("RD {}", address.to_text()?);
        let (_turn, direct) = self.begin_turn().await?;
        let response = direct.send_decoded_direct(&command, false).await?;
        if suffix.is_empty() && response.trim() != response {
            direct.retire_transport().await;
            return Err(HostLinkError::protocol(
                "Direct bit response must not contain surrounding whitespace",
            ));
        }
        let mut tokens = split_data_tokens(&response);
        let expected = read_response_token_count(&address.device_type, &suffix);
        if let Err(error) = validate_response_token_count(&tokens, expected) {
            direct.retire_transport().await;
            return Err(error);
        }
        let response_format =
            if is_direct_bit_device_type(&address.device_type) && suffix.is_empty() {
                ""
            } else {
                suffix.as_str()
            };
        let validation = if matches!(address.device_type.as_str(), "T" | "C") {
            validate_and_normalize_timer_counter_response_tokens(&mut tokens, response_format)
        } else {
            validate_and_normalize_response_tokens(&mut tokens, response_format)
        };
        if let Err(error) = validation {
            direct.retire_transport().await;
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
        let command = format!("RDS {} {}", address.to_text()?, count);
        let (_turn, direct) = self.begin_turn().await?;
        let response = direct.send_decoded_direct(&command, false).await?;
        if suffix.is_empty() && response.trim() != response {
            direct.retire_transport().await;
            return Err(HostLinkError::protocol(
                "Direct bit response must not contain surrounding whitespace",
            ));
        }
        let mut tokens = split_data_tokens(&response);
        if let Err(error) = validate_response_token_count(&tokens, count) {
            direct.retire_transport().await;
            return Err(error);
        }
        if let Err(error) = validate_and_normalize_response_tokens(&mut tokens, &suffix) {
            direct.retire_transport().await;
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
        append_strict_payload(&value, &suffix, &mut command)?;
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
        let (_turn, direct) = self.begin_turn().await?;
        let response = direct.send_decoded_direct(&command, true).await?;
        if response != "OK" {
            let error = HostLinkError::protocol(format!("Expected OK but received {response}"));
            direct.retire_transport().await;
            return Err(HostLinkError::outcome_unknown(
                HostLinkOutcomeUnknownReason::MalformedResponse,
                error,
            ));
        }
        direct.inner.lock().await.monitor_bit_count = Some(devices.len());
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
        let mut formats = Vec::with_capacity(devices.len());
        for entry in devices {
            let (device, data_format, packed_direct_bits_u16) = match entry {
                HostLinkMonitorWord::Numeric {
                    device,
                    data_format,
                } => (device.as_str(), Some(data_format.as_str()), false),
                HostLinkMonitorWord::PackedDirectBitsU16 { device } => {
                    (device.as_str(), None, true)
                }
            };
            let mut address = parse_device(device)?;
            validate_device_type("MWS", &address.device_type, mws_device_types())?;
            let suffix = if packed_direct_bits_u16 {
                require_no_suffix(&address, "packed direct-bit MWS")?;
                if !is_direct_bit_device_type(&address.device_type) {
                    return Err(HostLinkError::protocol(format!(
                        "Packed direct-bit MWS requires a direct-bit device, received '{}'",
                        address.device_type
                    )));
                }
                String::from(".U")
            } else {
                require_explicit_format(&address, data_format)?
            };
            validate_device_span(&address.device_type, address.number, &suffix, 1)?;
            formats.push(if packed_direct_bits_u16 {
                MonitorWordResponseFormat::PackedDirectBitsU16
            } else {
                MonitorWordResponseFormat::Numeric(suffix.clone())
            });
            if !packed_direct_bits_u16 {
                address.suffix = suffix;
            }
            command.push(' ');
            command.push_str(&address.to_text()?);
        }
        let (_turn, direct) = self.begin_turn().await?;
        let response = direct.send_decoded_direct(&command, true).await?;
        if response != "OK" {
            let error = HostLinkError::protocol(format!("Expected OK but received {response}"));
            direct.retire_transport().await;
            return Err(HostLinkError::outcome_unknown(
                HostLinkOutcomeUnknownReason::MalformedResponse,
                error,
            ));
        }
        direct.inner.lock().await.monitor_word_formats = Some(formats);
        Ok(())
    }

    pub async fn read_monitor_bits(&self) -> Result<Vec<String>, HostLinkError> {
        let (_turn, direct) = self.begin_turn().await?;
        let expected =
            direct.inner.lock().await.monitor_bit_count.ok_or_else(|| {
                HostLinkError::protocol("Monitor bits must be registered before MBR")
            })?;
        let response = direct.send_decoded_direct("MBR", false).await?;
        if response.trim() != response {
            direct.retire_transport().await;
            return Err(HostLinkError::protocol(
                "Direct bit response must not contain surrounding whitespace",
            ));
        }
        let tokens = split_data_tokens(&response);
        if let Err(error) = validate_response_token_count(&tokens, expected) {
            direct.retire_transport().await;
            return Err(error);
        }
        if let Err(error) = validate_response_tokens(&tokens, "") {
            direct.retire_transport().await;
            return Err(error);
        }
        Ok(tokens)
    }

    pub async fn read_monitor_words(&self) -> Result<Vec<String>, HostLinkError> {
        let (_turn, direct) = self.begin_turn().await?;
        let formats = direct
            .inner
            .lock()
            .await
            .monitor_word_formats
            .clone()
            .ok_or_else(|| {
                HostLinkError::protocol("Monitor words must be registered before MWR")
            })?;
        let response = direct.send_decoded_direct("MWR", false).await?;
        let mut tokens = if formats
            .iter()
            .any(|format| matches!(format, MonitorWordResponseFormat::PackedDirectBitsU16))
        {
            response.split([' ', ',']).map(ToOwned::to_owned).collect()
        } else {
            split_data_tokens(&response)
        };
        if let Err(error) = validate_response_token_count(&tokens, formats.len()) {
            direct.retire_transport().await;
            return Err(error);
        }
        for (token, response_format) in tokens.iter_mut().zip(&formats) {
            let validation = match response_format {
                MonitorWordResponseFormat::Numeric(data_format) => {
                    validate_and_normalize_response_tokens(std::slice::from_mut(token), data_format)
                }
                MonitorWordResponseFormat::PackedDirectBitsU16 => {
                    validate_packed_direct_bits_u16_token(token)
                }
            };
            if let Err(error) = validation {
                direct.retire_transport().await;
                return Err(error);
            }
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
        let command = format!("RDE {} {}", address.to_text()?, count);
        let (_turn, direct) = self.begin_turn().await?;
        let response = direct.send_decoded_direct(&command, false).await?;
        let mut tokens = split_data_tokens(&response);
        if let Err(error) = validate_response_token_count(&tokens, count) {
            direct.retire_transport().await;
            return Err(error);
        }
        if let Err(error) = validate_and_normalize_response_tokens(&mut tokens, &suffix) {
            direct.retire_transport().await;
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
        append_strict_payload(&value, &suffix, &mut command)?;
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
        let command = format!("URD {unit_no:02} {address}{suffix} {count}");
        let (_turn, direct) = self.begin_turn().await?;
        let response = direct.send_decoded_direct(&command, false).await?;
        let mut tokens = split_data_tokens(&response);
        if let Err(error) = validate_response_token_count(&tokens, count) {
            direct.retire_transport().await;
            return Err(error);
        }
        if let Err(error) = validate_and_normalize_response_tokens(&mut tokens, &suffix) {
            direct.retire_transport().await;
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

    /// Return the exact `RDC` response payload bytes without text decoding.
    ///
    /// Transport CR/LF terminators are excluded. Comment padding bytes,
    /// including trailing ASCII spaces, are preserved. A syntactically valid
    /// PLC error response remains [`HostLinkError::Plc`] and does not retire
    /// the connection.
    pub async fn read_comment_bytes(&self, device: &str) -> Result<Vec<u8>, HostLinkError> {
        let address = parse_device(device)?;
        validate_device_type("RDC", &address.device_type, rdc_device_types())?;
        require_no_suffix(&address, "RDC")?;
        let command = format!("RDC {}", address.to_text()?);
        let (_turn, direct) = self.begin_turn().await?;
        direct
            .send_comment_with_direct(&command, |payload| Ok(payload.to_vec()))
            .await
    }

    /// Read and strictly decode one `RDC` device comment with the selected encoding.
    ///
    /// The library never retries another codec and never replaces malformed
    /// input. Use [`Self::read_comment_bytes`] when the stored encoding is not
    /// known by the application. A syntactically valid PLC error response is
    /// returned as [`HostLinkError::Plc`] without retiring the connection.
    pub async fn read_comments(
        &self,
        device: &str,
        encoding: HostLinkCommentEncoding,
    ) -> Result<String, HostLinkError> {
        let address = parse_device(device)?;
        validate_device_type("RDC", &address.device_type, rdc_device_types())?;
        require_no_suffix(&address, "RDC")?;
        let command = format!("RDC {}", address.to_text()?);
        let (_turn, direct) = self.begin_turn().await?;
        direct
            .send_comment_with_direct(&command, |payload| {
                decode_comment_payload(payload, encoding)
            })
            .await
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

    /// Read a named all-or-error aggregate with semantically unique keys.
    ///
    /// Spelling variants that normalize to the same device, numeric address,
    /// dtype, bit index, and scalar count are rejected before FIFO admission.
    /// Distinct dtype views, bit indices, and overlapping spans remain valid,
    /// and result keys preserve their original input strings.
    pub async fn read_named<S: AsRef<str>>(
        &self,
        addresses: &[S],
    ) -> Result<helpers::NamedReadResult, HostLinkError> {
        helpers::read_named(self, addresses).await
    }

    /// Read a named aggregate that intentionally contains comments.
    ///
    /// At least one address must use `:COMMENT`; otherwise the unused encoding
    /// is rejected before FIFO admission or transport.
    pub async fn read_named_with_comment_encoding<S: AsRef<str>>(
        &self,
        addresses: &[S],
        comment_encoding: HostLinkCommentEncoding,
    ) -> Result<helpers::NamedReadResult, HostLinkError> {
        helpers::read_named_with_comment_encoding(self, addresses, comment_encoding).await
    }

    pub fn poll<'a, S: AsRef<str> + 'a>(
        &'a self,
        addresses: &'a [S],
        interval: Duration,
    ) -> impl futures_core::Stream<Item = Result<helpers::NamedReadResult, HostLinkError>> + 'a
    {
        async_stream::try_stream! {
            let addr_list = addresses.iter().map(|item| item.as_ref().to_owned()).collect::<Vec<_>>();
            if addr_list.is_empty() {
                Err(HostLinkError::protocol("poll addresses must not be empty."))?;
            }
            if interval.is_zero() {
                Err(HostLinkError::protocol("poll interval must be greater than zero."))?;
            }
            helpers::validate_named_addresses(&addr_list)?;
            let compiled = crate::read_plan::compile_read_named_plan(&addr_list);
            loop {
                yield helpers::read_named_compiled(self, &addr_list, compiled.as_ref(), None).await?;
                tokio::time::sleep(interval).await;
            }
        }
    }

    /// Poll a named aggregate that intentionally contains comments.
    ///
    /// At least one address must use `:COMMENT`; otherwise the unused encoding
    /// is rejected before FIFO admission or transport when the stream starts.
    pub fn poll_with_comment_encoding<'a, S: AsRef<str> + 'a>(
        &'a self,
        addresses: &'a [S],
        interval: Duration,
        comment_encoding: HostLinkCommentEncoding,
    ) -> impl futures_core::Stream<Item = Result<helpers::NamedReadResult, HostLinkError>> + 'a
    {
        helpers::poll_with_comment_encoding(self, addresses, interval, comment_encoding)
    }

    pub async fn read_words(&self, device: &str, count: usize) -> Result<Vec<u16>, HostLinkError> {
        helpers::read_words(self, device, count).await
    }

    pub async fn read_dwords(&self, device: &str, count: usize) -> Result<Vec<u32>, HostLinkError> {
        helpers::read_dwords(self, device, count).await
    }

    async fn expect_ok(&self, body: &str) -> Result<(), HostLinkError> {
        let body = body.to_owned();
        build_frame(&body)?;
        let (_turn, direct) = self.begin_turn().await?;
        let response = direct.send_decoded_direct(&body, true).await?;
        if response == "OK" {
            Ok(())
        } else {
            let error = HostLinkError::protocol(format!(
                "Expected 'OK' but received '{response}' for command '{body}'"
            ));
            direct.retire_transport().await;
            Err(HostLinkError::outcome_unknown(
                HostLinkOutcomeUnknownReason::MalformedResponse,
                error,
            ))
        }
    }
}

impl ClientInner {
    async fn open(&mut self, timeout: Duration) -> Result<(), HostLinkError> {
        if self.exchange_incomplete {
            self.retire_failed_transport();
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
        let (rx_buf, tcp_read_buf, udp_read_buf) = match self.options.transport {
            HostLinkTransportMode::Tcp => (
                allocate_zeroed_buffer(4096, "TCP receive accumulator")?,
                allocate_zeroed_buffer(8192, "TCP socket read buffer")?,
                Vec::new(),
            ),
            HostLinkTransportMode::Udp => (
                Vec::new(),
                Vec::new(),
                allocate_zeroed_buffer(UDP_RECEIVE_BUFFER_SIZE, "UDP datagram receive buffer")?,
            ),
        };
        let connect_deadline = checked_deadline(timeout)?;
        let endpoints = resolve_ipv4_endpoints(
            self.options.host.as_str(),
            self.options.port,
            connect_deadline,
        )
        .await?;

        let transport = match self.options.transport {
            HostLinkTransportMode::Tcp => {
                let stream = connect_tcp_ipv4(&endpoints, connect_deadline).await?;
                stream.set_nodelay(true)?;
                Transport::Tcp(stream)
            }
            HostLinkTransportMode::Udp => {
                let endpoint = endpoints[0];
                let socket = create_udp_socket(endpoint, connect_deadline).await?;
                Transport::Udp(UdpTransport {
                    endpoint,
                    socket: Some(socket),
                })
            }
        };

        self.rx_buf = rx_buf;
        self.tcp_read_buf = tcp_read_buf;
        self.udp_read_buf = udp_read_buf;
        self.transport_buffer_allocation_count += 1;
        self.transport = Some(transport);
        self.rx_start = 0;
        self.rx_count = 0;
        self.rx_scan = 0;
        self.last_request_may_have_been_sent = false;
        self.active_deadline = None;
        Ok(())
    }

    fn close(&mut self) {
        self.transport = None;
        self.rx_buf = Vec::new();
        self.rx_start = 0;
        self.rx_count = 0;
        self.rx_scan = 0;
        self.tcp_read_buf = Vec::new();
        self.udp_read_buf = Vec::new();
        self.exchange_incomplete = false;
        self.active_deadline = None;
        self.monitor_bit_count = None;
        self.monitor_word_formats = None;
    }

    fn retire_failed_transport(&mut self) {
        match self.transport.as_mut() {
            Some(Transport::Udp(transport)) => {
                transport.socket = None;
                self.rx_start = 0;
                self.rx_count = 0;
                self.rx_scan = 0;
                self.exchange_incomplete = false;
                self.active_deadline = None;
                self.monitor_bit_count = None;
                self.monitor_word_formats = None;
            }
            _ => self.close(),
        }
    }

    async fn send_raw_bytes(
        &mut self,
        body: &str,
        timeout: Duration,
    ) -> Result<Vec<u8>, HostLinkError> {
        let raw = self.exchange_raw(body, timeout).await?;
        let body = raw_response_body(&raw);
        self.finish_decode_deadline()?;
        Ok(body)
    }

    async fn send_decoded_with<F>(
        &mut self,
        body: &str,
        decoder: F,
        timeout: Duration,
    ) -> Result<String, HostLinkError>
    where
        F: Fn(&[u8]) -> Result<String, HostLinkError>,
    {
        let raw = self.exchange_raw(body, timeout).await?;
        let decoded_result = decoder(&raw);
        self.finish_decode_deadline()?;
        let decoded = match decoded_result {
            Ok(decoded) => decoded,
            Err(error) => {
                self.close();
                return Err(error);
            }
        };
        let result = ensure_success(decoded);
        self.finish_decode_deadline()?;
        result
    }

    async fn send_comment_with<T, F>(
        &mut self,
        body: &str,
        decoder: F,
        timeout: Duration,
    ) -> Result<T, HostLinkError>
    where
        F: FnOnce(&[u8]) -> Result<T, HostLinkError>,
    {
        let raw = self.exchange_raw(body, timeout).await?;
        let decoded_result = comment_response_payload(&raw).and_then(|payload| {
            ensure_comment_success(payload)?;
            decoder(payload)
        });
        self.finish_decode_deadline()?;
        match decoded_result {
            Ok(decoded) => Ok(decoded),
            Err(error @ HostLinkError::Protocol(_)) => {
                self.close();
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    async fn exchange_raw(
        &mut self,
        body: &str,
        timeout: Duration,
    ) -> Result<Vec<u8>, HostLinkError> {
        let frame = build_frame(body)?;
        if self.exchange_incomplete {
            self.retire_failed_transport();
        }
        if self.transport.is_none() {
            return Err(HostLinkError::NotConnected);
        }
        let deadline = checked_deadline(timeout)?;
        self.active_deadline = Some(deadline);
        self.exchange_incomplete = true;
        self.last_request_may_have_been_sent = false;
        let exchange_result = match self.transport.as_mut() {
            Some(Transport::Tcp(stream)) => {
                let preflight = reject_unowned_tcp_response(stream, &mut self.tcp_read_buf, true);
                self.last_request_may_have_been_sent = preflight.is_ok();
                match preflight {
                    Err(error) => Err(error),
                    Ok(()) => match write_all_with_timeout(stream, &frame, deadline).await {
                        Ok(()) => {
                            self.traffic_stats.request_count += 1;
                            self.traffic_stats.tx_bytes += frame.len() as u64;
                            recv_tcp_line(
                                stream,
                                TcpReceiveState {
                                    rx_buf: &mut self.rx_buf,
                                    rx_start: &mut self.rx_start,
                                    rx_count: &mut self.rx_count,
                                    rx_scan: &mut self.rx_scan,
                                    scan_bytes: &mut self.tcp_scan_bytes,
                                    copy_bytes: &mut self.tcp_copy_bytes,
                                },
                                &mut self.tcp_read_buf,
                                deadline,
                            )
                            .await
                        }
                        Err(err) => Err(err),
                    },
                }
            }
            Some(Transport::Udp(transport)) => {
                async {
                    // Move ownership into the exchange future. Dropping/cancelling
                    // that future drops the socket immediately instead of leaving
                    // an indeterminate datagram queue in the logical session.
                    let mut socket = match transport.socket.take() {
                        Some(socket) => socket,
                        None => create_udp_socket(transport.endpoint, deadline).await?,
                    };
                    reject_unowned_udp_response(&socket, &mut self.udp_read_buf)?;
                    self.last_request_may_have_been_sent = true;
                    send_udp_with_timeout(&mut socket, &frame, deadline).await?;
                    self.traffic_stats.request_count += 1;
                    self.traffic_stats.tx_bytes += frame.len() as u64;
                    recv_udp_with_timeout(&mut socket, &mut self.udp_read_buf, deadline).await?;
                    if !matches!(self.udp_read_buf.last(), Some(b'\r' | b'\n')) {
                        return Err(HostLinkError::protocol(
                            "UDP response is missing the required CR/LF terminator",
                        ));
                    }
                    let raw = self.udp_read_buf.clone();
                    let counted_len = raw.len();
                    reject_unowned_udp_response(&socket, &mut self.udp_read_buf)?;
                    transport.socket = Some(socket);
                    Ok((raw, counted_len))
                }
                .await
            }
            None => Err(HostLinkError::connection("transport was not opened")),
        };

        match exchange_result {
            Ok((raw, counted_len)) => {
                self.traffic_stats.rx_bytes += counted_len as u64;
                if raw_response_body(&raw).len() > MAX_TCP_LINE_SIZE {
                    self.retire_failed_transport();
                    return Err(HostLinkError::protocol(format!(
                        "Response line exceeds {MAX_TCP_LINE_SIZE} bytes"
                    )));
                }
                self.exchange_incomplete = false;
                Ok(raw)
            }
            Err(err) => {
                self.retire_failed_transport();
                Err(err)
            }
        }
    }

    fn finish_decode_deadline(&mut self) -> Result<(), HostLinkError> {
        let Some(deadline) = self.active_deadline else {
            return Ok(());
        };
        if Instant::now() >= deadline {
            self.retire_failed_transport();
            return Err(HostLinkError::timeout(
                "transaction deadline expired during response decoding",
            ));
        }
        self.active_deadline = None;
        Ok(())
    }
}

impl HostLinkClientFactory {
    pub async fn open_and_connect(
        options: HostLinkConnectionOptions,
    ) -> Result<HostLinkClient, HostLinkError> {
        if options.host.trim().is_empty() {
            return Err(HostLinkError::protocol("Host must not be empty."));
        }

        let client = HostLinkClient::new(options);
        client.open().await?;
        Ok(client)
    }
}

pub async fn open_and_connect(
    options: HostLinkConnectionOptions,
) -> Result<HostLinkClient, HostLinkError> {
    HostLinkClientFactory::open_and_connect(options).await
}

fn classify_operation_result<T>(
    result: Result<T, HostLinkError>,
    state_changing: bool,
    may_have_been_sent: bool,
) -> Result<T, HostLinkError> {
    result.map_err(|error| {
        if !state_changing || !may_have_been_sent {
            return error;
        }
        let reason = match error {
            HostLinkError::Timeout(_) => HostLinkOutcomeUnknownReason::Timeout,
            HostLinkError::Closed => HostLinkOutcomeUnknownReason::Closed,
            HostLinkError::Transport { .. } => HostLinkOutcomeUnknownReason::Transport,
            HostLinkError::Protocol(_) => HostLinkOutcomeUnknownReason::MalformedResponse,
            HostLinkError::OutcomeUnknown { .. }
            | HostLinkError::NotConnected
            | HostLinkError::Plc { .. } => return error,
        };
        HostLinkError::outcome_unknown(reason, error)
    })
}

fn checked_deadline(duration: Duration) -> Result<Instant, HostLinkError> {
    Instant::now()
        .checked_add(duration)
        .ok_or_else(|| HostLinkError::protocol("timeout is too large to form an absolute deadline"))
}

fn allocate_zeroed_buffer(size: usize, purpose: &str) -> Result<Vec<u8>, HostLinkError> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(size).map_err(|error| {
        HostLinkError::connection(format!(
            "Failed to allocate {purpose} before transport use: {error}"
        ))
    })?;
    buffer.resize(size, 0);
    Ok(buffer)
}

async fn resolve_ipv4_endpoints(
    host: &str,
    port: u16,
    deadline: Instant,
) -> Result<Vec<SocketAddr>, HostLinkError> {
    validate_host_input(host)?;
    let literal_text = host
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(host);
    if let Ok(literal) = literal_text.parse::<IpAddr>() {
        return match literal {
            IpAddr::V4(address) => Ok(vec![SocketAddr::new(IpAddr::V4(address), port)]),
            IpAddr::V6(_) => Err(HostLinkError::protocol(
                "Host Link endpoints are IPv4-only; IPv6 literals are not supported",
            )),
        };
    }

    let resolved = timeout_at(deadline, lookup_host((host, port)))
        .await
        .map_err(|_| HostLinkError::timeout("endpoint resolution"))?
        .map_err(|error| HostLinkError::transport("endpoint resolution failed", error))?;
    select_ipv4_endpoints(host, resolved.collect())
}

fn select_ipv4_endpoints(
    host: &str,
    resolved: Vec<SocketAddr>,
) -> Result<Vec<SocketAddr>, HostLinkError> {
    let endpoints = resolved
        .into_iter()
        .filter(SocketAddr::is_ipv4)
        .collect::<Vec<_>>();
    if endpoints.is_empty() {
        return Err(HostLinkError::connection(format!(
            "Host Link hostname '{host}' did not resolve to an IPv4 address"
        )));
    }
    Ok(endpoints)
}

async fn connect_tcp_ipv4(
    endpoints: &[SocketAddr],
    deadline: Instant,
) -> Result<TcpStream, HostLinkError> {
    let mut last_error = None;
    for endpoint in endpoints {
        match timeout_at(deadline, TcpStream::connect(endpoint)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => last_error = Some(error),
            Err(_) => return Err(HostLinkError::timeout("tcp connect")),
        }
    }
    match last_error {
        Some(error) => Err(HostLinkError::transport(
            "tcp connect failed for every resolved IPv4 endpoint",
            error,
        )),
        None => Err(HostLinkError::connection(
            "tcp connect failed: no IPv4 endpoint",
        )),
    }
}

async fn write_all_with_timeout(
    stream: &mut TcpStream,
    payload: &[u8],
    deadline: Instant,
) -> Result<(), HostLinkError> {
    timeout_at(deadline, stream.write_all(payload))
        .await
        .map_err(|_| HostLinkError::timeout("tcp write"))??;
    Ok(())
}

async fn send_udp_with_timeout(
    socket: &mut UdpSocket,
    payload: &[u8],
    deadline: Instant,
) -> Result<(), HostLinkError> {
    timeout_at(deadline, socket.send(payload))
        .await
        .map_err(|_| HostLinkError::timeout("udp send"))??;
    Ok(())
}

async fn create_udp_socket(
    endpoint: SocketAddr,
    deadline: Instant,
) -> Result<UdpSocket, HostLinkError> {
    let socket = timeout_at(deadline, UdpSocket::bind("0.0.0.0:0"))
        .await
        .map_err(|_| HostLinkError::timeout("udp bind"))??;
    timeout_at(deadline, socket.connect(endpoint))
        .await
        .map_err(|_| HostLinkError::timeout("udp connect"))??;
    Ok(socket)
}

fn reject_unowned_udp_response(
    socket: &UdpSocket,
    udp_read_buf: &mut Vec<u8>,
) -> Result<(), HostLinkError> {
    if udp_read_buf.len() != UDP_RECEIVE_BUFFER_SIZE {
        udp_read_buf.resize(UDP_RECEIVE_BUFFER_SIZE, 0);
    }
    match socket.try_recv(udp_read_buf.as_mut_slice()) {
        Ok(_) => Err(HostLinkError::protocol(
            "Received an unowned UDP response outside the active request",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn reject_unowned_tcp_response(
    stream: &TcpStream,
    tcp_read_buf: &mut [u8],
    eof_is_error: bool,
) -> Result<(), HostLinkError> {
    loop {
        match stream.try_read(tcp_read_buf) {
            Ok(0) if eof_is_error => {
                return Err(HostLinkError::connection("Connection closed by PLC"));
            }
            Ok(0) => return Ok(()),
            Ok(read) => {
                if tcp_read_buf[..read]
                    .iter()
                    .any(|byte| !matches!(byte, b'\r' | b'\n'))
                {
                    return Err(HostLinkError::protocol(
                        "Received an unowned extra TCP response before sending the next request",
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
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
        .map_err(|_| HostLinkError::timeout("udp receive"))??;
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
        append_strict_payload(value, suffix, &mut payload)?;
    }
    Ok(payload)
}

fn append_strict_payload<T: HostLinkPayloadValue + ?Sized>(
    value: &T,
    suffix: &str,
    output: &mut String,
) -> Result<(), HostLinkError> {
    if suffix.is_empty() {
        let value = value
            .as_bool()
            .ok_or_else(|| HostLinkError::protocol("direct-bit writes require a bool value"))?;
        output.push(if value { '1' } else { '0' });
        Ok(())
    } else {
        value.append_to_payload(suffix, output)
    }
}

struct TcpReceiveState<'a> {
    rx_buf: &'a mut Vec<u8>,
    rx_start: &'a mut usize,
    rx_count: &'a mut usize,
    rx_scan: &'a mut usize,
    scan_bytes: &'a mut u64,
    copy_bytes: &'a mut u64,
}

async fn recv_tcp_line(
    stream: &mut TcpStream,
    state: TcpReceiveState<'_>,
    tcp_read_buf: &mut [u8],
    deadline: Instant,
) -> Result<(Vec<u8>, usize), HostLinkError> {
    let TcpReceiveState {
        rx_buf,
        rx_start,
        rx_count,
        rx_scan,
        scan_bytes,
        copy_bytes,
    } = state;
    loop {
        while *rx_count > 0 && matches!(rx_buf[*rx_start], b'\r' | b'\n') {
            *rx_start += 1;
            *rx_count -= 1;
            *rx_scan = (*rx_scan).max(*rx_start);
            *scan_bytes += 1;
        }
        if *rx_start > rx_buf.len() / 2 {
            if *rx_count > 0 {
                rx_buf.copy_within(*rx_start..*rx_start + *rx_count, 0);
                *copy_bytes += *rx_count as u64;
            }
            *rx_scan = (*rx_scan).saturating_sub(*rx_start);
            *rx_start = 0;
        }
        let mut found_idx = None;
        let end = *rx_start + *rx_count;
        while *rx_scan < end {
            let byte = rx_buf[*rx_scan];
            *scan_bytes += 1;
            if matches!(byte, b'\r' | b'\n') {
                found_idx = Some(*rx_scan - *rx_start);
                break;
            }
            *rx_scan += 1;
            if *rx_scan - *rx_start > MAX_TCP_LINE_SIZE {
                return Err(HostLinkError::protocol(format!(
                    "Response line exceeds {MAX_TCP_LINE_SIZE} bytes"
                )));
            }
        }

        if let Some(found_idx) = found_idx {
            if found_idx > MAX_TCP_LINE_SIZE {
                return Err(HostLinkError::protocol(format!(
                    "Response line exceeds {MAX_TCP_LINE_SIZE} bytes"
                )));
            }
            let mut skip = found_idx + 1;
            while skip < *rx_count && matches!(rx_buf[*rx_start + skip], b'\r' | b'\n') {
                *scan_bytes += 1;
                skip += 1;
            }
            if skip < *rx_count {
                return Err(HostLinkError::protocol(
                    "Received more than one non-empty TCP response for one request",
                ));
            }
            reject_unowned_tcp_response(stream, tcp_read_buf, false)?;
            let line = rx_buf[*rx_start..*rx_start + skip].to_vec();
            *copy_bytes += skip as u64;
            let counted_len = found_idx + 1;
            *rx_start += skip;
            *rx_count -= skip;
            *rx_scan = *rx_start;
            if *rx_start > rx_buf.len() / 2 {
                rx_buf.copy_within(*rx_start..*rx_start + *rx_count, 0);
                *copy_bytes += *rx_count as u64;
                *rx_start = 0;
                *rx_scan = 0;
            }
            return Ok((line, counted_len));
        }

        let read = timeout_at(deadline, stream.read(tcp_read_buf))
            .await
            .map_err(|_| HostLinkError::timeout("tcp receive"))??;
        if read == 0 {
            let message = if *rx_count > 0 {
                "Connection closed by PLC before the response terminator"
            } else {
                "Connection closed by PLC"
            };
            return Err(HostLinkError::connection(message));
        }

        if *rx_start + *rx_count + read > rx_buf.len() {
            if *rx_start > 0 && *rx_count > 0 {
                rx_buf.copy_within(*rx_start..*rx_start + *rx_count, 0);
                *copy_bytes += *rx_count as u64;
            }
            *rx_scan = (*rx_scan).saturating_sub(*rx_start);
            *rx_start = 0;
            if *rx_count + read > MAX_TCP_LINE_SIZE + tcp_read_buf.len() {
                return Err(HostLinkError::protocol(format!(
                    "Response line exceeds {MAX_TCP_LINE_SIZE} bytes"
                )));
            }
            if *rx_count + read > rx_buf.len() {
                let new_len = (rx_buf.len().max(1) * 2).max(*rx_count + read);
                let mut grown = allocate_zeroed_buffer(new_len, "TCP receive accumulator growth")?;
                grown[..*rx_count].copy_from_slice(&rx_buf[..*rx_count]);
                *copy_bytes += *rx_count as u64;
                *rx_buf = grown;
            }
        }

        let target = *rx_start + *rx_count;
        rx_buf[target..target + read].copy_from_slice(&tcp_read_buf[..read]);
        *copy_bytes += read as u64;
        *rx_count += read;
    }
}

#[cfg(test)]
mod monitor_word_response_tests {
    use super::validate_packed_direct_bits_u16_token;

    #[test]
    fn packed_u16_wire_grammar_is_distinct_from_general_unsigned_parsing() {
        for token in ["0", "2", "13", "00000", "00002", "00013", "65535"] {
            assert!(
                validate_packed_direct_bits_u16_token(token).is_ok(),
                "token={token:?}"
            );
        }
        for token in [
            "", "+2", "-1", " 2", "2 ", "\t2", "2\t", "2.0", "ON", "２", "000000", "65536",
        ] {
            assert!(
                validate_packed_direct_bits_u16_token(token).is_err(),
                "token={token:?}"
            );
        }
    }
}

#[cfg(test)]
mod endpoint_tests {
    use super::select_ipv4_endpoints;
    use std::net::SocketAddr;

    #[test]
    fn hostname_resolution_requires_at_least_one_ipv4_result() {
        let ipv6_only = vec!["[::1]:8501".parse::<SocketAddr>().unwrap()];
        let error = select_ipv4_endpoints("ipv6-only.example", ipv6_only).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("did not resolve to an IPv4 address")
        );

        let mixed = vec![
            "[::1]:8501".parse::<SocketAddr>().unwrap(),
            "127.0.0.1:8501".parse::<SocketAddr>().unwrap(),
        ];
        assert_eq!(
            select_ipv4_endpoints("mixed.example", mixed).unwrap(),
            vec!["127.0.0.1:8501".parse::<SocketAddr>().unwrap()]
        );
    }
}

#[cfg(test)]
mod performance_tests {
    use super::{HostLinkClient, MAX_TCP_LINE_SIZE, TcpReceiveState, recv_tcp_line};
    use crate::{HostLinkConnectionOptions, HostLinkTransportMode};
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;
    use tokio::net::{TcpListener, TcpStream, UdpSocket};
    use tokio::time::Instant;

    fn options(port: u16, transport: HostLinkTransportMode) -> HostLinkConnectionOptions {
        HostLinkConnectionOptions::new("127.0.0.1", port, transport, "keyence:kv-8000").unwrap()
    }

    #[tokio::test]
    async fn transport_buffers_are_lazy_separated_reused_and_released() {
        let tcp_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let tcp_port = tcp_listener.local_addr().unwrap().port();
        let tcp_client = HostLinkClient::new(options(tcp_port, HostLinkTransportMode::Tcp));
        {
            let inner = tcp_client.inner.lock().await;
            assert_eq!(inner.rx_buf.capacity(), 0);
            assert_eq!(inner.tcp_read_buf.capacity(), 0);
            assert_eq!(inner.udp_read_buf.capacity(), 0);
            assert_eq!(inner.transport_buffer_allocation_count, 0);
        }

        tcp_client.open().await.unwrap();
        let (first_tcp, _) = tcp_listener.accept().await.unwrap();
        {
            let inner = tcp_client.inner.lock().await;
            assert_eq!(inner.rx_buf.len(), 4096);
            assert_eq!(inner.tcp_read_buf.len(), 8192);
            assert_eq!(inner.udp_read_buf.capacity(), 0);
            assert_eq!(inner.transport_buffer_allocation_count, 1);
        }
        tcp_client.open().await.unwrap();
        assert_eq!(
            tcp_client
                .inner
                .lock()
                .await
                .transport_buffer_allocation_count,
            1
        );
        drop(first_tcp);
        tcp_client.close().await.unwrap();
        {
            let inner = tcp_client.inner.lock().await;
            assert_eq!(inner.rx_buf.capacity(), 0);
            assert_eq!(inner.tcp_read_buf.capacity(), 0);
            assert_eq!(inner.udp_read_buf.capacity(), 0);
        }

        let udp_peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let udp_client = HostLinkClient::new(options(
            udp_peer.local_addr().unwrap().port(),
            HostLinkTransportMode::Udp,
        ));
        udp_client.open().await.unwrap();
        {
            let inner = udp_client.inner.lock().await;
            assert_eq!(inner.rx_buf.capacity(), 0);
            assert_eq!(inner.tcp_read_buf.capacity(), 0);
            assert_eq!(inner.udp_read_buf.len(), MAX_TCP_LINE_SIZE + 2);
            assert_eq!(inner.transport_buffer_allocation_count, 1);
        }
        udp_client.close().await.unwrap();
        let inner = udp_client.inner.lock().await;
        assert_eq!(inner.udp_read_buf.capacity(), 0);
    }

    #[tokio::test]
    async fn one_byte_tcp_reads_scan_and_copy_maximum_body_linearly() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let sender = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut response = vec![b'A'; MAX_TCP_LINE_SIZE];
            response.push(b'\r');
            stream.write_all(&response).await.unwrap();
        });
        let mut stream = TcpStream::connect(address).await.unwrap();
        let mut rx_buf = vec![0; 4096];
        let mut rx_start = 0;
        let mut rx_count = 0;
        let mut rx_scan = 0;
        let mut read_buf = [0; 1];
        let mut scan_bytes = 0;
        let mut copy_bytes = 0;

        let (line, counted) = recv_tcp_line(
            &mut stream,
            TcpReceiveState {
                rx_buf: &mut rx_buf,
                rx_start: &mut rx_start,
                rx_count: &mut rx_count,
                rx_scan: &mut rx_scan,
                scan_bytes: &mut scan_bytes,
                copy_bytes: &mut copy_bytes,
            },
            &mut read_buf,
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap();
        sender.await.unwrap();

        assert_eq!(line.len(), MAX_TCP_LINE_SIZE + 1);
        assert_eq!(counted, MAX_TCP_LINE_SIZE + 1);
        assert_eq!(scan_bytes, (MAX_TCP_LINE_SIZE + 1) as u64);
        assert!(
            copy_bytes <= ((MAX_TCP_LINE_SIZE + 1) * 4) as u64,
            "copy_bytes={copy_bytes}"
        );
    }

    #[tokio::test]
    async fn udp_socket_replacement_reuses_the_session_receive_buffer() {
        let peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let port = peer.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            let mut request = [0u8; 32];
            let (_, first_client) = peer.recv_from(&mut request).await.unwrap();
            peer.send_to(b"UNTERMINATED", first_client).await.unwrap();
            let (_, replacement_client) = peer.recv_from(&mut request).await.unwrap();
            peer.send_to(b"OK\r", replacement_client).await.unwrap();
        });
        let client = HostLinkClient::new(options(port, HostLinkTransportMode::Udp));
        client.open().await.unwrap();

        assert!(client.send_raw("FIRST").await.is_err());
        {
            let inner = client.inner.lock().await;
            assert!(inner.udp_read_buf.capacity() >= MAX_TCP_LINE_SIZE + 2);
            assert_eq!(inner.transport_buffer_allocation_count, 1);
        }
        assert_eq!(client.send_raw("SECOND").await.unwrap(), b"OK");
        {
            let inner = client.inner.lock().await;
            assert!(inner.udp_read_buf.capacity() >= MAX_TCP_LINE_SIZE + 2);
            assert_eq!(inner.transport_buffer_allocation_count, 1);
        }
        server.await.unwrap();
    }
}

#[cfg(test)]
mod udp_reuse_tests {
    use super::{UDP_RECEIVE_BUFFER_SIZE, reject_unowned_udp_response};
    use crate::error::HostLinkError;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn queued_extra_datagram_is_rejected_before_socket_reuse() {
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        receiver
            .connect(sender.local_addr().unwrap())
            .await
            .unwrap();
        sender
            .send_to(b"EXTRA\r", receiver.local_addr().unwrap())
            .await
            .unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        let mut buffer = vec![0; UDP_RECEIVE_BUFFER_SIZE];

        assert!(matches!(
            reject_unowned_udp_response(&receiver, &mut buffer),
            Err(HostLinkError::Protocol(_))
        ));
    }
}

#[cfg(test)]
mod payload_value_tests {
    use super::{HostLinkPayloadValue, build_joined_payload};
    use crate::error::HostLinkError;
    use crate::helpers::HostLinkValue;

    macro_rules! assert_integer_type_is_fallible {
        ($ty:ty) => {{
            let value: $ty = 0;
            assert_eq!(value.format_for_suffix("").unwrap(), "0");
            for suffix in [".U", ".S", ".H", ".D", ".L"] {
                assert!(value.format_for_suffix(suffix).is_ok());
            }
            assert!(value.format_for_suffix(".UNKNOWN").is_err());
        }};
    }

    #[test]
    fn every_builtin_integer_type_uses_validated_fallible_formatting() {
        assert_integer_type_is_fallible!(u8);
        assert_integer_type_is_fallible!(u16);
        assert_integer_type_is_fallible!(u32);
        assert_integer_type_is_fallible!(u64);
        assert_integer_type_is_fallible!(usize);
        assert_integer_type_is_fallible!(i8);
        assert_integer_type_is_fallible!(i16);
        assert_integer_type_is_fallible!(i32);
        assert_integer_type_is_fallible!(i64);
        assert_integer_type_is_fallible!(isize);

        assert_eq!(0_i32.format_for_suffix("").unwrap(), "0");
        assert_eq!(1_i32.format_for_suffix("").unwrap(), "1");
        assert!((-1_i32).format_for_suffix("").is_err());
        assert!(2_i32.format_for_suffix("").is_err());

        assert_eq!(0_u16.format_for_suffix(".U").unwrap(), "0");
        assert_eq!(u16::MAX.format_for_suffix(".U").unwrap(), "65535");
        assert!((-1_i32).format_for_suffix(".U").is_err());
        assert!(65_536_u32.format_for_suffix(".U").is_err());

        assert_eq!(i16::MIN.format_for_suffix(".S").unwrap(), "-32768");
        assert_eq!(i16::MAX.format_for_suffix(".S").unwrap(), "32767");
        assert!((-32_769_i32).format_for_suffix(".S").is_err());
        assert!(32_768_i32.format_for_suffix(".S").is_err());

        assert_eq!(0_u16.format_for_suffix(".H").unwrap(), "0");
        assert_eq!(u16::MAX.format_for_suffix(".H").unwrap(), "FFFF");
        assert!((-1_i32).format_for_suffix(".H").is_err());
        assert!(65_536_u32.format_for_suffix(".H").is_err());

        assert_eq!(0_u32.format_for_suffix(".D").unwrap(), "0");
        assert_eq!(u32::MAX.format_for_suffix(".D").unwrap(), "4294967295");
        assert!((-1_i64).format_for_suffix(".D").is_err());
        assert!(4_294_967_296_u64.format_for_suffix(".D").is_err());

        assert_eq!(i32::MIN.format_for_suffix(".L").unwrap(), "-2147483648");
        assert_eq!(i32::MAX.format_for_suffix(".L").unwrap(), "2147483647");
        assert!((-2_147_483_649_i64).format_for_suffix(".L").is_err());
        assert!(2_147_483_648_i64.format_for_suffix(".L").is_err());
    }

    #[test]
    fn incompatible_builtin_types_and_hostlink_values_never_return_fallback_tokens() {
        assert_eq!(true.format_for_suffix("").unwrap(), "1");
        assert!(true.format_for_suffix(".U").is_err());
        assert!(1.25_f32.format_for_suffix(".U").is_err());
        assert!(1.25_f64.format_for_suffix(".U").is_err());
        assert!(String::from("12").format_for_suffix(".U").is_err());
        assert!("12".format_for_suffix(".U").is_err());
        let referenced = &65_536_u32;
        assert!(referenced.format_for_suffix(".U").is_err());
        assert!(HostLinkValue::F32(1.25).format_for_suffix(".U").is_err());
        assert!(
            HostLinkValue::Text("12".to_owned())
                .format_for_suffix(".U")
                .is_err()
        );
        assert!(HostLinkValue::U32(65_536).format_for_suffix(".U").is_err());
    }

    struct CustomValue(bool);

    impl HostLinkPayloadValue for CustomValue {
        fn format_for_suffix(&self, suffix: &str) -> Result<String, HostLinkError> {
            if self.0 && suffix == ".U" {
                Ok("77".to_owned())
            } else {
                Err(HostLinkError::protocol("custom suffix rejected"))
            }
        }
    }

    struct EmptyValue;

    impl HostLinkPayloadValue for EmptyValue {
        fn format_for_suffix(&self, _suffix: &str) -> Result<String, HostLinkError> {
            Ok(String::new())
        }
    }

    #[test]
    fn default_append_and_joined_payload_propagate_original_error_without_partial_token() {
        let mut output = String::from("prefix");
        CustomValue(false)
            .append_to_payload(".U", &mut output)
            .unwrap_err();
        assert_eq!(output, "prefix");
        assert_eq!(CustomValue(true).format_for_suffix(".U").unwrap(), "77");
        assert_eq!(
            build_joined_payload(&[CustomValue(true), CustomValue(true)], ".U").unwrap(),
            "77 77"
        );
        let error =
            build_joined_payload(&[CustomValue(true), CustomValue(false)], ".U").unwrap_err();
        assert!(error.to_string().contains("custom suffix rejected"));

        let mut output = String::from("still unchanged");
        let error = EmptyValue.append_to_payload(".U", &mut output).unwrap_err();
        assert!(error.to_string().contains("empty token"));
        assert_eq!(output, "still unchanged");
        assert!(build_joined_payload(&[EmptyValue], ".U").is_err());
    }
}
