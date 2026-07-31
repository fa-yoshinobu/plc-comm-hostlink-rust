#![allow(dead_code)]

use plc_comm_kv_hostlink::{
    HostLinkClient, HostLinkConnectionOptions, HostLinkError, HostLinkTransportMode,
    parse_logical_address,
};
use std::collections::BTreeMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::time::sleep;

pub type MonitorResult<T> = Result<T, String>;

#[derive(Clone, Debug)]
pub struct TagSpec {
    pub name: String,
    pub address: String,
}

#[derive(Clone, Debug)]
pub struct PlcEndpoint {
    pub name: String,
    pub host: String,
    pub plc_profile: String,
    pub port: u16,
    pub transport: String,
    pub timeout_ms: u64,
    pub interval: Duration,
}

#[derive(Clone)]
pub struct CsvWriter {
    path: Arc<PathBuf>,
    lock: Arc<Mutex<()>>,
}

impl CsvWriter {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path: Arc::new(path),
            lock: Arc::new(Mutex::new(())),
        }
    }

    async fn write_result(
        &self,
        endpoint: &PlcEndpoint,
        values: &BTreeMap<String, String>,
    ) -> MonitorResult<()> {
        let _guard = self.lock.lock().await;
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let needs_header = !self.path.exists()
            || self
                .path
                .metadata()
                .map_err(|error| error.to_string())?
                .len()
                == 0;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path.as_ref())
            .map_err(|error| error.to_string())?;
        if needs_header {
            writeln!(file, "timestamp,plc,tag,value").map_err(|error| error.to_string())?;
        }
        let timestamp = timestamp();
        for (tag, value) in values {
            writeln!(file, "{timestamp},{},{tag},{value}", endpoint.name)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

pub fn parse_transport(value: &str) -> MonitorResult<String> {
    let transport = value.to_ascii_lowercase();
    match transport.as_str() {
        "tcp" | "udp" => Ok(transport),
        _ => Err("transport must be tcp or udp".to_string()),
    }
}

pub fn parse_tag_spec(value: &str) -> MonitorResult<TagSpec> {
    let (name, address) = if let Some((name, address)) = value.split_once('=') {
        if name.is_empty() || address.is_empty() {
            return Err("expected NAME=ADDRESS".to_string());
        }
        (name.to_string(), address)
    } else {
        (normalize_tag_name(value), value)
    };
    parse_logical_address(address).map_err(|error| error.to_string())?;
    Ok(TagSpec {
        name,
        address: address.to_string(),
    })
}

pub fn parse_positive_duration(value: &str, name: &str) -> MonitorResult<Duration> {
    let seconds = value
        .parse::<f64>()
        .map_err(|error| format!("{name}: {error}"))?;
    positive_duration(seconds, name)
}

pub fn positive_duration(seconds: f64, name: &str) -> MonitorResult<Duration> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return Err(format!("{name} must be a finite number greater than zero"));
    }
    let duration =
        Duration::try_from_secs_f64(seconds).map_err(|error| format!("{name}: {error}"))?;
    if duration.is_zero() {
        return Err(format!("{name} must be greater than zero"));
    }
    Ok(duration)
}

pub fn parse_plc_spec(
    value: &str,
    timeout_ms: u64,
    interval: Duration,
) -> MonitorResult<PlcEndpoint> {
    if timeout_ms == 0 {
        return Err("timeout_ms must be greater than zero".to_string());
    }
    if interval.is_zero() {
        return Err("interval must be greater than zero".to_string());
    }
    let Some((name, rest)) = value.split_once('=') else {
        return Err("expected NAME=HOST,PROFILE,PORT,TRANSPORT".to_string());
    };
    let parts = rest.split(',').map(str::trim).collect::<Vec<_>>();
    if name.is_empty() || parts.len() != 4 || parts.iter().any(|value| value.is_empty()) {
        return Err("expected NAME=HOST,PROFILE,PORT,TRANSPORT".to_string());
    }
    let port = parts[2].parse::<u16>().map_err(|error| error.to_string())?;
    if port == 0 {
        return Err("port must be in 1..=65535".to_string());
    }
    let transport = parse_transport(parts[3])?;
    Ok(PlcEndpoint {
        name: name.to_string(),
        host: parts[0].to_string(),
        plc_profile: parts[1].to_string(),
        port,
        transport,
        timeout_ms,
        interval,
    })
}

pub async fn monitor_endpoint(
    endpoint: PlcEndpoint,
    tags: Vec<TagSpec>,
    cycles: Option<usize>,
    initial_backoff: Duration,
    max_backoff: Duration,
    writer: Option<CsvWriter>,
) -> MonitorResult<()> {
    if tags.is_empty() {
        return Err("at least one tag is required".to_string());
    }

    let mut client: Option<HostLinkClient> = None;
    let mut completed = 0_usize;
    let mut backoff = initial_backoff;
    let mut connected_once = false;

    while cycles.is_none_or(|limit| completed < limit) {
        if client.is_none() {
            log_state(
                &endpoint.name,
                "reconnecting",
                &format!(
                    "{} {}:{} profile={}",
                    endpoint.transport, endpoint.host, endpoint.port, endpoint.plc_profile
                ),
            );
            match HostLinkClient::connect(options_for(&endpoint)?).await {
                Ok(new_client) => {
                    client = Some(new_client);
                    log_state(
                        &endpoint.name,
                        if connected_once {
                            "recovered"
                        } else {
                            "connected"
                        },
                        &format!("{} tags", tags.len()),
                    );
                    connected_once = true;
                    backoff = initial_backoff;
                }
                Err(error) if is_retryable_hostlink(&error) => {
                    log_state(
                        &endpoint.name,
                        "reconnecting",
                        &format!(
                            "connect failed: {error}; retry in {:.1}s",
                            backoff.as_secs_f64()
                        ),
                    );
                    sleep(backoff).await;
                    backoff = next_backoff(backoff, max_backoff);
                    continue;
                }
                Err(error) => return Err(error.to_string()),
            }
        }

        let active = client.as_ref().expect("client was just connected");
        match read_values(active, &tags).await {
            Ok(values) => {
                log_state(&endpoint.name, "read", &format_values(&values));
                if let Some(csv_writer) = &writer {
                    csv_writer.write_result(&endpoint, &values).await?;
                }
                completed += 1;
                if cycles.is_none_or(|limit| completed < limit) {
                    sleep(endpoint.interval).await;
                }
            }
            Err(error) if is_retryable_hostlink(&error) => {
                log_state(&endpoint.name, "lost", &error.to_string());
                if let Some(client) = client.take() {
                    let _ = client.close().await;
                }
                log_state(
                    &endpoint.name,
                    "reconnecting",
                    &format!("retry in {:.1}s", backoff.as_secs_f64()),
                );
                sleep(backoff).await;
                backoff = next_backoff(backoff, max_backoff);
            }
            Err(error) => return Err(error.to_string()),
        }
    }

    if let Some(client) = client.take() {
        let _ = client.close().await;
    }
    Ok(())
}

pub fn split_address(address: &str) -> MonitorResult<(&str, &str)> {
    if let Some((device, dtype)) = address.rsplit_once(':') {
        if device.is_empty() || dtype.is_empty() {
            return Err(format!("address must be DEVICE:DTYPE: {address}"));
        }
        Ok((device, dtype))
    } else {
        Ok((address, "U"))
    }
}

pub fn format_endpoint(endpoint: &PlcEndpoint) -> String {
    format!(
        "{}: {} {}:{} profile={} interval={}s",
        endpoint.name,
        endpoint.transport,
        endpoint.host,
        endpoint.port,
        endpoint.plc_profile,
        endpoint.interval.as_secs_f64()
    )
}

pub fn format_tags(tags: &[TagSpec]) -> String {
    tags.iter()
        .map(|tag| format!("{}={}", tag.name, tag.address))
        .collect::<Vec<_>>()
        .join(", ")
}

fn normalize_tag_name(address: &str) -> String {
    address
        .replace(['\\', ':', '.', '-', '/'], "_")
        .to_ascii_lowercase()
}

fn options_for(endpoint: &PlcEndpoint) -> MonitorResult<HostLinkConnectionOptions> {
    if endpoint.timeout_ms == 0 {
        return Err("timeout_ms must be greater than zero".to_string());
    }
    let transport = match endpoint.transport.as_str() {
        "udp" => HostLinkTransportMode::Udp,
        "tcp" => HostLinkTransportMode::Tcp,
        _ => return Err("transport must be tcp or udp".to_string()),
    };
    let mut options = HostLinkConnectionOptions::new(
        endpoint.host.clone(),
        endpoint.port,
        transport,
        &endpoint.plc_profile,
    )
    .map_err(|error| error.to_string())?;
    options.timeout = Duration::from_millis(endpoint.timeout_ms);
    Ok(options)
}

async fn read_values(
    client: &HostLinkClient,
    tags: &[TagSpec],
) -> Result<BTreeMap<String, String>, HostLinkError> {
    let addresses = tags
        .iter()
        .map(|tag| tag.address.as_str())
        .collect::<Vec<_>>();
    let values = client.read_named(&addresses).await?;
    let mut result = BTreeMap::new();
    for tag in tags {
        let value = values.get(&tag.address).ok_or_else(|| {
            HostLinkError::protocol(format!("named read omitted {}", tag.address))
        })?;
        result.insert(tag.name.clone(), format!("{value:?}"));
    }
    Ok(result)
}

fn is_retryable_hostlink(error: &HostLinkError) -> bool {
    matches!(
        error,
        HostLinkError::Transport { .. } | HostLinkError::Timeout(_) | HostLinkError::Closed
    )
}

fn next_backoff(current: Duration, max: Duration) -> Duration {
    Duration::from_secs_f64((current.as_secs_f64() * 2.0).min(max.as_secs_f64()))
}

fn format_values(values: &BTreeMap<String, String>) -> String {
    values
        .iter()
        .map(|(tag, value)| format!("{tag}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn log_state(plc_name: &str, state: &str, message: &str) {
    println!("{} [{plc_name}] [{state}] {message}", timestamp());
}

fn timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:03}", now.as_secs(), now.subsec_millis())
}
